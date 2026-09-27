# SPDX-License-Identifier: GPL-3.0-only
"""公开源码边界与独立打包；不读取私有库或游戏样本。"""
import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'packaging/source-files.txt'
PRIVATE_PARTS = {
    '.git', '.private', 'private', 'local', 'docs', 'state', 'agent-context',
    'publishing', 'analysis', '.analysis', 'assets', '截图', 'experiments',
    'gallery', 'nexusmods', 'github', 'dist', 'target', '__pycache__',
}
PRIVATE_EXTENSIONS = {
    '.gfx', '.dds', '.tga', '.tpf', '.dcx', '.fmg', '.bin', '.dll', '.exe',
    '.pdb', '.log', '.dmp', '.png', '.jpg', '.zip', '.gpr', '.pyc', '.bbcode',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_path(name):
    require(isinstance(name, str) and name and name.isascii(), f'Invalid source path: {name!r}')
    require('\\' not in name and ':' not in name and '\x00' not in name, f'Unsafe source path: {name}')
    path = PurePosixPath(name)
    require(not path.is_absolute() and str(path) == name and '..' not in path.parts, f'Unsafe source path: {name}')
    lowered = [part.lower() for part in path.parts]
    require(all(part.rstrip(' .') == part and all(ord(char) >= 32 for char in part) for part in path.parts),
            f'Windows path alias/control character: {name!r}')
    require(not any(part == 'agents.md' or part in PRIVATE_PARTS or part.endswith('-private') for part in lowered),
            f'Private path in public source: {name}')
    require(path.suffix.lower() not in PRIVATE_EXTENSIONS, f'Private/binary source member: {name}')
    return path


def parse_manifest(text):
    names = [line.strip() for line in text.splitlines() if line.strip() and not line.lstrip().startswith('#')]
    require(len(names) == len(set(names)), 'Duplicate source manifest entries')
    require(len(names) == len({name.lower() for name in names}), 'Case-colliding source manifest entries')
    for name in names:
        check_path(name)
    require(MANIFEST in names and 'Cargo.toml' in names and 'src/lib.rs' in names, 'Incomplete source manifest')
    return sorted(names)


def read_sources(root):
    names = parse_manifest((root / MANIFEST).read_text('utf-8-sig'))
    result = {}
    for name in names:
        current = root
        for part in PurePosixPath(name).parts:
            current = current / part
            metadata = current.lstat()
            require(not stat.S_ISLNK(metadata.st_mode) and not getattr(metadata, 'st_file_attributes', 0) & 0x400,
                    f'Symlink/reparse point in source: {name}')
        require(current.is_file(), f'Not a source file: {name}')
        result[name] = current.read_bytes()
    return result


def git(root, *args):
    return subprocess.check_output(['git', '-c', f'safe.directory={root.as_posix()}', '-C', str(root), *args])


def check_git(root, names):
    if not os.path.lexists(root / '.git'):
        return None
    top = Path(git(root, 'rev-parse', '--show-toplevel').decode('utf-8').strip()).resolve()
    require(top == root.resolve(), 'Unexpected Git root')
    tracked = set(git(root, 'ls-files', '-z').decode('utf-8').rstrip('\0').split('\0'))
    require(tracked == set(names), f'Tracked files differ from public manifest: {sorted(tracked ^ set(names))}')
    require(not git(root, 'status', '--porcelain', '--untracked-files=normal'), 'Commit a clean source tree before packaging')
    # 扫描本地所有可达分支／标签；不只检查当前工作树。
    for revision in git(root, 'rev-list', '--all').decode('ascii').splitlines():
        records = git(root, 'ls-tree', '-r', '-z', revision).split(b'\0')
        for record in filter(None, records):
            metadata, raw_name = record.split(b'\t', 1)
            mode, kind, _ = metadata.split()
            require(kind == b'blob' and mode in (b'100644', b'100755'), 'Symlink/submodule in public history')
            check_path(raw_name.decode('utf-8'))
    return git(root, 'rev-parse', 'HEAD').decode('ascii').strip()


def validate(root):
    files = read_sources(root)
    revision = check_git(root, files)
    cargo = tomllib.loads(files['Cargo.toml'].decode('utf-8-sig'))
    require(cargo['package']['license'] == 'GPL-3.0-only', 'Wrong source license')
    return files, cargo['package']['version'], revision


def pack(root, output):
    files, version, revision = validate(root)
    output.mkdir(parents=True, exist_ok=True)
    prefix = f'elden-ring-controller-ui-{version}-source'
    archive = output / (prefix + '.zip')
    checksum = Path(str(archive) + '.sha256')
    require(not archive.exists() and not checksum.exists(), 'Source archive/checksum already exists; use a fresh directory')
    with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zipped:
        for name, data in files.items():
            entry = zipfile.ZipInfo(prefix + '/' + name, date_time=(1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(entry, data)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    with checksum.open('x', encoding='ascii', newline='\n') as stream:
        stream.write(f'{digest}  {archive.name}\n')
    print(f'Corresponding source: {archive} ({len(files)} files; commit={revision or "standalone source"})')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('check', 'pack'))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.action == 'pack':
        require(args.output is not None, 'pack requires --output')
        pack(ROOT, args.output)
    else:
        files, version, revision = validate(ROOT)
        print(f'Public source verified: {len(files)} files, version={version}, commit={revision or "standalone source"}')


if __name__ == '__main__':
    main()
