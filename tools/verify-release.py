# SPDX-License-Identifier: GPL-3.0-only
"""无第三方依赖的公开发布检查；不加载 DLL，不需要游戏文件。"""
import argparse
import hashlib
import json
import re
import struct
import tomllib
import zipfile
from pathlib import Path, PurePosixPath
from source_policy import MANIFEST, check_path, parse_manifest

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_FILES = {"EldenRingControllerUI.me3", "README.txt", "README.zh-CN.txt", "LICENSE.txt",
                 "Rust-COPYRIGHT-library.html",
                 "NOTICE.txt", "THIRD_PARTY_NOTICES.txt", "natives/EldenRingControllerUI.dll",
                 "natives/EldenRingControllerUI.ini"}
GAME_HASHES = {
    44007: "170996C2376BB14675FE1BB308C3CE82C28BEC0DEAFBFC8E40BD8FEF0E99E4B4",
    46401: "4F80A029D8D6BDB7C6C893F13704E44C592C670C5ADFC8BA6B11C793FB5E0392",
    55592: "693D6B509C01A4758BE63A17C55D043C0133FA2684B3A53CF5F21875202FF137",
}

def check(condition, message):
    if not condition:
        raise ValueError(message)

def digest(data):
    return hashlib.sha256(data).hexdigest().upper()

def pe_symbols(data):
    def unpack(fmt, at):
        return struct.unpack_from("<" + fmt, data, at)[0]
    check(data[:2] == b"MZ", "DLL lacks MZ")
    nt = unpack("I", 0x3c)
    check(data[nt:nt+4] == b"PE\0\0" and unpack("H", nt+4) == 0x8664, "DLL is not PE x64")
    opt = nt+24
    check(unpack("H", opt) == 0x20b and unpack("I", opt+108) >= 14, "invalid PE optional header")
    table = opt + unpack("H", nt+20)
    sections = []
    for i in range(unpack("H", nt+6)):
        s = table+i*40
        sections.append((unpack("I", s+12), unpack("I", s+16), unpack("I", s+20)))
    def rva(value, size=1):
        for base, length, raw in sections:
            if base <= value and value+size <= base+length and raw+value-base+size <= len(data):
                return raw+value-base
        raise ValueError(f"unmapped PE RVA {value:x}")
    def string(value):
        at = rva(value)
        end = data.find(b"\0", at, at+4096)
        check(end >= at, "unterminated PE string")
        return data[at:end].decode("ascii")
    exports = []
    export_rva = unpack("I", opt+112)
    check(export_rva != 0, "no exports")
    directory = rva(export_rva, 40)
    names = unpack("I", directory+24)
    check(names < 4096, "excessive export count")
    pointers = rva(unpack("I", directory+32), names*4)
    for i in range(names):
        exports.append(string(unpack("I", pointers+i*4)))
    imports = {}
    import_rva = unpack("I", opt+120)
    check(import_rva != 0, "no imports")
    for i in range(512):
        at = rva(import_rva+i*20, 20)
        original, _, _, name, first = struct.unpack_from("<IIIII", data, at)
        if not any(data[at:at+20]):
            break
        module = string(name)
        symbols = []
        for j in range(8192):
            thunk = unpack("Q", rva((original or first)+j*8, 8))
            if not thunk:
                break
            symbols.append(f"ordinal:{thunk & 0xffff}" if thunk & (1 << 63) else string(thunk+2))
        else:
            raise ValueError("unterminated import thunks")
        imports[module] = symbols
    else:
        raise ValueError("unterminated imports")
    check(not unpack("I", opt+112+13*8), "unexpected delay imports")
    return exports, imports

def verify_runtime(package, version):
    files = {p.relative_to(package).as_posix(): p for p in package.rglob("*") if p.is_file()}
    check(set(files) == RUNTIME_FILES, f"unexpected runtime inventory: {set(files) ^ RUNTIME_FILES}")
    check(not (package / "assets").exists(), "assets directory in release")
    profile = tomllib.loads(files["EldenRingControllerUI.me3"].read_text("utf-8-sig"))
    check(profile.get("start_online") is False and not profile.get("packages"), "unsafe profile")
    check(len(profile["natives"]) == 1, "expected exactly one native")
    native = profile["natives"][0]
    check(native["path"] == "natives/EldenRingControllerUI.dll" and native["load_early"] is True
          and native["initializer"] == {"function": "elden_ring_controller_ui_init"}, "invalid native profile")
    for shipped, source in [("LICENSE.txt", "LICENSE"), ("NOTICE.txt", "NOTICE"),
                            ("THIRD_PARTY_NOTICES.txt", "THIRD_PARTY_NOTICES.md"),
                            ("Rust-COPYRIGHT-library.html", "third_party/Rust-COPYRIGHT-library.html")]:
        check(files[shipped].read_bytes() == (ROOT / source).read_bytes(), f"stale {shipped}")
    notice = files["NOTICE.txt"].read_text("utf-8-sig")
    check("GPL-3.0-only" in notice and "Luna(a492219408)" in notice and
          f"elden-ring-controller-ui-{version}-source.zip" in notice, "license/author/source notice mismatch")
    for name in ["README.txt", "README.zh-CN.txt"]:
        check(version in files[name].read_text("utf-8-sig"), f"stale version in {name}")
    dll = files["natives/EldenRingControllerUI.dll"].read_bytes()
    check(version.encode() in dll, "DLL version not present")
    at = 0
    while (at := dll.find(b"GFX\x0b", at)) >= 0:
        if at+8 <= len(dll):
            length = struct.unpack_from("<I", dll, at+4)[0]
            if length in GAME_HASHES:
                check(digest(dll[at:at+length]) != GAME_HASHES[length], "embedded full game fixture")
        at += 4
    exports, imports = pe_symbols(dll)
    check(set(exports) == {"DllMain", "elden_ring_controller_ui_init"}, f"unexpected exports: {exports}")
    check(not re.search(r"xinput|dinput|hid\.dll|steam_api|GetRawInput|SendInput|WriteProcessMemory|winhttp|wininet|ws2_32", json.dumps(imports), re.I),
          "unexpected input, network or obsolete atlas-write import")
    return {"version": version, "files": {n: {"length": p.stat().st_size, "sha256": digest(p.read_bytes())}
                                           for n, p in sorted(files.items())}, "exports": exports, "imports": imports}

def verify_source(archive, version):
    prefix = f"elden-ring-controller-ui-{version}-source/"
    with zipfile.ZipFile(archive) as z:
        names = z.namelist()
        check(len(names) == len(set(names)), "duplicate ZIP names")
        required = {"Cargo.toml", "Cargo.lock", "src/lib.rs", "src/overview_data.rs",
                    "scripts/build.ps1", "scripts/package.ps1", "scripts/package-source.ps1", "tools/verify-release.py",
                    MANIFEST, "scripts/check-source.ps1", "tools/source_policy.py", "tools/test_source_policy.py",
                    "tools/generate-overview-presets.py", "scripts/verify-samples.ps1",
                    "EldenRingControllerUI.ini", "LICENSE", "NOTICE", "third_party/Rust-COPYRIGHT-library.html",
                    ".github/workflows/ci.yml", ".github/workflows/release.yml"}
        relative = set()
        for name in names:
            check(name.startswith(prefix) and "\\" not in name, f"invalid archive root: {name}")
            path = PurePosixPath(name[len(prefix):])
            check_path(name[len(prefix):])
            check(not path.is_absolute() and ".." not in path.parts, f"unsafe ZIP path: {name}")
            check(not any(p in {".git", "assets", "analysis", "截图", "dist", "target", "experiments", "gallery"} for p in path.parts), f"private data in source: {name}")
            check(path.suffix.lower() not in {".gfx", ".dds", ".tga", ".tpf", ".dcx", ".fmg", ".bin", ".dll", ".exe", ".log", ".dmp", ".png"}, f"binary in source: {name}")
            relative.add(str(path))
        check(required <= relative, f"missing source files: {required-relative}")
        expected = set(parse_manifest(z.read(prefix+MANIFEST).decode("utf-8-sig")))
        check(relative == expected, f"source inventory differs from manifest: {relative ^ expected}")
        cargo = tomllib.loads(z.read(prefix+"Cargo.toml").decode("utf-8-sig"))
        check(cargo["package"]["version"] == version and cargo["package"]["license"] == "GPL-3.0-only", "source metadata mismatch")
        for name in relative:
            check(z.read(prefix+name) == (ROOT / name).read_bytes(), f"stale source member: {name}")
    return {"file": archive.name, "sha256": digest(archive.read_bytes()), "files": len(names)}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text("utf-8-sig"))
    check(cargo["package"]["license"] == "GPL-3.0-only", "wrong Cargo license")
    check(cargo["package"]["authors"] == ["Luna(a492219408)"], "wrong author")
    result = verify_runtime(args.package, cargo["package"]["version"])
    if args.source:
        result["source"] = verify_source(args.source, result["version"])
    output = json.dumps(result, indent=2, ensure_ascii=False)
    if args.report:
        with args.report.open("x", encoding="utf-8") as report:
            report.write(output+"\n")
    print(output)
    print("Release verified without loading DLL or game.")

if __name__ == "__main__":
    main()
