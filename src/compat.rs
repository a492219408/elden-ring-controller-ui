use std::{ffi::c_void, mem};

use crate::windows;

const EXE_117_SIZE: u64 = 87_024_720;
const EXE_117_SHA256: [u8; 32] = [
    0xD1, 0xA8, 0x40, 0x83, 0xC6, 0xC7, 0xC7, 0x90, 0x21, 0x62, 0xFF, 0x09, 0x8F, 0x7D, 0x86, 0x81,
    0x28, 0x39, 0xAA, 0x6B, 0x35, 0x75, 0x95, 0x93, 0x98, 0x85, 0x7E, 0x53, 0x9C, 0x48, 0x81, 0x34,
];

const EXE_1171_SIZE: u64 = 87_042_128;
const EXE_1171_SHA256: [u8; 32] = [
    0x1A, 0x35, 0x47, 0x10, 0x13, 0x27, 0xF6, 0x5D, 0x0C, 0x76, 0xDA, 0x2F, 0x91, 0x90, 0xAC, 0x0A,
    0xA6, 0x68, 0x71, 0xEA, 0x42, 0xBA, 0xE2, 0xAE, 0xCC, 0x61, 0xE1, 0x1A, 0x8B, 0x59, 0x78, 0x91,
];

// 两版独立 EXE 样本证明这些数据地址和页签指令不变；不能把代码偏移套到数据地址上。
const TAB_SELECT_RVAS: [usize; 2] = [0x0080_8C20, 0x0096_8040];
pub const TAB_SELECT_INSTRUCTIONS: [[u8; 9]; 2] = [
    [0x8B, 0x15, 0x12, 0xBA, 0x28, 0x02, 0x83, 0xC2, 0x14],
    [0x8B, 0x15, 0xF2, 0xC5, 0x12, 0x02, 0x83, 0xC2, 0x1E],
];
const SCALEFORM_HEAP_RVA: usize = 0x0459_72D0;
const MEMORY_FILE_VTABLE_RVA: usize = 0x02BA_7D70;
const SYS_FILE_VTABLE_RVA: usize = 0x02CC_5CA0;

pub const FILE_OPENER_PROLOGUE: [u8; 16] = [
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8D, 0x6C, 0x24, 0xD9,
];
const MEMORY_FILE_CTOR_PROLOGUE: [u8; 19] = [
    0x48, 0x89, 0x4C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x30, 0x48, 0xC7, 0x44, 0x24, 0x20, 0xFE,
    0xFF, 0xFF, 0xFF,
];
const REFCOUNT_ADD_PROLOGUE: [u8; 7] = [0xF0, 0x0F, 0xC1, 0x11, 0x8B, 0xC2, 0xC3];
const MEMORY_FILE_METHODS_117: [usize; 20] = [
    0xCE96C0, 0xCE98F0, 0xCE9AC0, 0xCE9AD0, 0xCE9BE0, 0xCE9B10, 0xCE9900, 0xCE9AE0, 0xCE98E0,
    0xCE9BF0, 0xCE9B20, 0xCE9BB0, 0xCE97B0, 0xCE98B0, 0xCE9B70, 0xCE9AF0, 0xCE9BA0, 0xCE97C0,
    0xCE97E0, 0xCE97D0,
];
const SYS_FILE_METHODS_117: [usize; 20] = [
    0x11BB220, 0x11BB3D0, 0x11BB0F0, 0x11BB440, 0x11BB500, 0x11BB490, 0xFC7BD0, 0x1067F30,
    0x11BB0D0, 0x1068520, 0xFC81D0, 0x11BB4E0, 0x11BB300, 0x11BB390, 0x11BB4C0, 0x11BB470,
    0xCE9BA0, 0x11BB320, 0x11BB370, 0x11BB120,
];

// 逐项核对两张 vtable 及对应函数后记录，不在运行时按差值猜测未支持版本。
const MEMORY_FILE_METHODS_1171: [usize; 20] = [
    0xCE9730, 0xCE9960, 0xCE9B30, 0xCE9B40, 0xCE9C50, 0xCE9B80, 0xCE9970, 0xCE9B50, 0xCE9950,
    0xCE9C60, 0xCE9B90, 0xCE9C20, 0xCE9820, 0xCE9920, 0xCE9BE0, 0xCE9B60, 0xCE9C10, 0xCE9830,
    0xCE9850, 0xCE9840,
];
const SYS_FILE_METHODS_1171: [usize; 20] = [
    0x11BB290, 0x11BB440, 0x11BB160, 0x11BB4B0, 0x11BB570, 0x11BB500, 0xFC7C40, 0x1067FA0,
    0x11BB140, 0x1068590, 0xFC8240, 0x11BB550, 0x11BB370, 0x11BB400, 0x11BB530, 0x11BB4E0,
    0xCE9C10, 0x11BB390, 0x11BB3E0, 0x11BB190,
];

#[derive(Debug)]
struct Build {
    app_version: &'static str,
    exe_size: u64,
    exe_sha256: [u8; 32],
    texture_resolver: usize,
    external_texture_resolver: usize,
    file_opener: usize,
    memory_file_ctor: usize,
    refcount_add: usize,
    memory_methods: [usize; 20],
    sys_methods: [usize; 20],
}

const BUILDS: [Build; 2] = [
    Build {
        app_version: "1.17",
        exe_size: EXE_117_SIZE,
        exe_sha256: EXE_117_SHA256,
        texture_resolver: 0x00D6_61D0,
        external_texture_resolver: 0x00D6_5970,
        file_opener: 0x00D6_D210,
        memory_file_ctor: 0x00CE_9280,
        refcount_add: 0x0114_29B0,
        memory_methods: MEMORY_FILE_METHODS_117,
        sys_methods: SYS_FILE_METHODS_117,
    },
    Build {
        app_version: "1.17.1",
        exe_size: EXE_1171_SIZE,
        exe_sha256: EXE_1171_SHA256,
        texture_resolver: 0x00D6_6240,
        external_texture_resolver: 0x00D6_59E0,
        file_opener: 0x00D6_D280,
        memory_file_ctor: 0x00CE_92F0,
        refcount_add: 0x0114_2A20,
        memory_methods: MEMORY_FILE_METHODS_1171,
        sys_methods: SYS_FILE_METHODS_1171,
    },
];

fn select_build(exe_size: u64, exe_hash: &[u8; 32]) -> Result<&'static Build, String> {
    BUILDS
        .iter()
        .find(|build| build.exe_size == exe_size && &build.exe_sha256 == exe_hash)
        .ok_or_else(|| {
            format!(
                "unsupported eldenring.exe (size={exe_size}, SHA-256={})",
                crate::sha256::to_hex(exe_hash)
            )
        })
}

#[derive(Clone, Copy)]
pub struct NativeFileTargets {
    pub opener: usize,
    pub constructor: usize,
    pub refcount_add: usize,
    pub heap_slot: usize,
    pub memory_vtable: usize,
    pub sys_vtable: usize,
    pub memory_methods: [usize; 20],
    pub sys_methods: [usize; 20],
}

pub const TEXTURE_RESOLVER_PROLOGUE: [u8; 18] = [
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8D, 0x6C,
    0x24, 0xE1,
];
pub const EXTERNAL_TEXTURE_RESOLVER_PROLOGUE: [u8; 18] = [
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8D, 0x6C,
    0x24, 0xE1,
];
pub struct Targets {
    pub app_version: &'static str,
    pub texture_resolver: *mut u8,
    pub external_texture_resolver: *mut u8,
    pub native_file: NativeFileTargets,
    pub tab_selectors: [*mut u8; 2],
}

pub unsafe fn resolve(exe_size: u64, exe_hash: &[u8; 32]) -> Result<Targets, String> {
    let build = select_build(exe_size, exe_hash)?;

    let image = unsafe { LoadedImage::main_module()? };
    let texture_resolver = unsafe {
        image.resolve_verified(
            build.texture_resolver,
            &TEXTURE_RESOLVER_PROLOGUE,
            "Scaleform texture resolver",
        )?
    };
    let external_texture_resolver = unsafe {
        image.resolve_verified(
            build.external_texture_resolver,
            &EXTERNAL_TEXTURE_RESOLVER_PROLOGUE,
            "Scaleform external-image texture resolver",
        )?
    };
    let opener = unsafe {
        image.resolve_verified(
            build.file_opener,
            &FILE_OPENER_PROLOGUE,
            "Scaleform file opener",
        )?
    } as usize;
    let constructor = unsafe {
        image.resolve_verified(
            build.memory_file_ctor,
            &MEMORY_FILE_CTOR_PROLOGUE,
            "native MemoryFile constructor",
        )?
    } as usize;
    let refcount_add = unsafe {
        image.resolve_verified(
            build.refcount_add,
            &REFCOUNT_ADD_PROLOGUE,
            "native refcount increment",
        )?
    } as usize;
    let memory_methods = image.verify_vtable(MEMORY_FILE_VTABLE_RVA, &build.memory_methods)?;
    let sys_methods = image.verify_vtable(SYS_FILE_VTABLE_RVA, &build.sys_methods)?;
    if !image
        .sections
        .iter()
        .any(|section| section.contains(SCALEFORM_HEAP_RVA, 8))
    {
        return Err("Scaleform heap slot is outside the PE image".to_owned());
    }
    Ok(Targets {
        app_version: build.app_version,
        texture_resolver,
        external_texture_resolver,
        tab_selectors: [
            unsafe {
                image.resolve_verified(
                    TAB_SELECT_RVAS[0],
                    &TAB_SELECT_INSTRUCTIONS[0],
                    "game-options native tab selector",
                )?
            },
            unsafe {
                image.resolve_verified(
                    TAB_SELECT_RVAS[1],
                    &TAB_SELECT_INSTRUCTIONS[1],
                    "controller-settings native tab selector",
                )?
            },
        ],
        native_file: NativeFileTargets {
            opener,
            constructor,
            refcount_add,
            heap_slot: image.base as usize + SCALEFORM_HEAP_RVA,
            memory_vtable: image.base as usize + MEMORY_FILE_VTABLE_RVA,
            sys_vtable: image.base as usize + SYS_FILE_VTABLE_RVA,
            memory_methods,
            sys_methods,
        },
    })
}

#[derive(Clone)]
struct Section {
    rva: usize,
    size: usize,
    characteristics: u32,
}

impl Section {
    fn contains(&self, rva: usize, length: usize) -> bool {
        rva >= self.rva
            && rva
                .checked_add(length)
                .zip(self.rva.checked_add(self.size))
                .is_some_and(|(end, section_end)| end <= section_end)
    }

    fn executable(&self) -> bool {
        self.characteristics & 0x2000_0000 != 0
    }
}

struct LoadedImage {
    base: *mut u8,
    size: usize,
    sections: Vec<Section>,
}

impl LoadedImage {
    fn verify_vtable(&self, rva: usize, methods: &[usize; 20]) -> Result<[usize; 20], String> {
        if !self
            .sections
            .iter()
            .any(|section| !section.executable() && section.contains(rva, 160))
        {
            return Err(format!("file vtable {rva:X} is outside PE data sections"));
        }
        let actual = windows::read_memory::<160>(self.base as usize + rva)
            .ok_or_else(|| format!("file vtable {rva:X} is unreadable"))?;
        let mut resolved = [0; 20];
        for (index, method) in methods.iter().enumerate() {
            resolved[index] = self.base as usize + method;
            if usize::from_le_bytes(actual[index * 8..index * 8 + 8].try_into().unwrap())
                != resolved[index]
                || !self
                    .sections
                    .iter()
                    .any(|section| section.executable() && section.contains(*method, 1))
            {
                return Err(format!("file vtable {rva:X} slot {index} mismatch"));
            }
        }
        Ok(resolved)
    }

    unsafe fn main_module() -> Result<Self, String> {
        const MAX_IMAGE_SIZE: usize = 1024 * 1024 * 1024;
        let base = unsafe { windows::main_module()? }.cast::<u8>();
        if unsafe { read_u16(base, 0)? } != 0x5A4D {
            return Err("main module has no MZ signature".to_owned());
        }
        let nt = unsafe { read_u32(base, 0x3C)? } as usize;
        if nt > 1024 * 1024 || unsafe { read_u32(base, nt)? } != 0x0000_4550 {
            return Err("main module has no usable PE signature".to_owned());
        }
        let section_count = unsafe { read_u16(base, nt + 6)? } as usize;
        if section_count == 0 || section_count > 96 {
            return Err(format!("invalid PE section count {section_count}"));
        }
        let optional_size = unsafe { read_u16(base, nt + 20)? } as usize;
        let optional = nt + 24;
        if optional_size < 112 || unsafe { read_u16(base, optional)? } != 0x20B {
            return Err("main module is not PE32+".to_owned());
        }
        let size = unsafe { read_u32(base, optional + 56)? } as usize;
        let headers_size = unsafe { read_u32(base, optional + 60)? } as usize;
        if !(4096..=MAX_IMAGE_SIZE).contains(&size) || headers_size == 0 || headers_size > size {
            return Err(format!("invalid loaded PE image size {size}"));
        }
        let section_table = optional
            .checked_add(optional_size)
            .ok_or_else(|| "PE section-table offset overflow".to_owned())?;
        let table_length = section_count
            .checked_mul(40)
            .ok_or_else(|| "PE section-table length overflow".to_owned())?;
        if section_table + table_length > headers_size
            || !unsafe { windows::range_is_readable(base.add(section_table).cast(), table_length) }
        {
            return Err("PE section table is outside readable headers".to_owned());
        }

        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let header = section_table + index * 40;
            let virtual_size = unsafe { read_u32(base, header + 8)? } as usize;
            let rva = unsafe { read_u32(base, header + 12)? } as usize;
            let raw_size = unsafe { read_u32(base, header + 16)? } as usize;
            let characteristics = unsafe { read_u32(base, header + 36)? };
            let mapped_size = virtual_size.max(raw_size);
            if mapped_size == 0 {
                continue;
            }
            if rva.checked_add(mapped_size).is_none_or(|end| end > size) {
                return Err(format!(
                    "PE section {index} extends outside the loaded image"
                ));
            }
            sections.push(Section {
                rva,
                size: mapped_size,
                characteristics,
            });
        }
        Ok(Self {
            base,
            size,
            sections,
        })
    }

    unsafe fn resolve_verified(
        &self,
        rva: usize,
        expected: &[u8],
        name: &str,
    ) -> Result<*mut u8, String> {
        if rva
            .checked_add(expected.len())
            .is_none_or(|end| end > self.size)
            || !self
                .sections
                .iter()
                .any(|section| section.executable() && section.contains(rva, expected.len()))
        {
            return Err(format!(
                "{name} RVA 0x{rva:08X} is outside executable PE sections"
            ));
        }
        let address = unsafe { self.base.add(rva) };
        if !unsafe { windows::range_is_readable(address.cast(), expected.len()) }
            || !unsafe { windows::address_is_executable(address.cast()) }
        {
            return Err(format!(
                "{name} target memory is not readable executable code"
            ));
        }
        let actual = unsafe { std::slice::from_raw_parts(address, expected.len()) };
        if actual != expected {
            return Err(format!(
                "{name} prologue does not match the verified executable profile"
            ));
        }
        Ok(address)
    }
}

unsafe fn read_bytes(base: *mut u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
    let address = unsafe { base.add(offset) };
    if !unsafe { windows::range_is_readable(address.cast::<c_void>(), length) } {
        return Err(format!(
            "PE header range 0x{offset:X}+0x{length:X} is unreadable"
        ));
    }
    Ok(unsafe { std::slice::from_raw_parts(address, length) }.to_vec())
}

unsafe fn read_u16(base: *mut u8, offset: usize) -> Result<u16, String> {
    let bytes = unsafe { read_bytes(base, offset, mem::size_of::<u16>())? };
    Ok(u16::from_le_bytes(bytes.try_into().expect("u16 range")))
}

unsafe fn read_u32(base: *mut u8, offset: usize) -> Result<u32, String> {
    let bytes = unsafe { read_bytes(base, offset, mem::size_of::<u32>())? };
    Ok(u32::from_le_bytes(bytes.try_into().expect("u32 range")))
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::*;

    #[test]
    fn fingerprints_and_stolen_instruction_lengths_are_fixed() {
        assert_eq!(EXE_117_SIZE, 87_024_720);
        assert_eq!(EXE_1171_SIZE, 87_042_128);
        assert_eq!(TEXTURE_RESOLVER_PROLOGUE.len(), 18);
        assert_eq!(EXTERNAL_TEXTURE_RESOLVER_PROLOGUE.len(), 18);
        assert_eq!(FILE_OPENER_PROLOGUE.len(), 16);
        for build in &BUILDS {
            assert_eq!(crate::sha256::to_hex(&build.exe_sha256).len(), 64);
        }
    }

    #[test]
    fn exact_fingerprints_select_independent_version_profiles() {
        for expected in &BUILDS {
            let actual = select_build(expected.exe_size, &expected.exe_sha256).unwrap();
            assert_eq!(actual.app_version, expected.app_version);
            assert_eq!(actual.file_opener, expected.file_opener);
            assert_eq!(actual.memory_methods, expected.memory_methods);
            assert_eq!(actual.sys_methods, expected.sys_methods);
        }
    }

    #[test]
    fn unknown_or_crossed_fingerprints_fail_before_accessing_loaded_image() {
        for build in &BUILDS {
            for i in 0..32 {
                let mut wrong = build.exe_sha256;
                wrong[i] ^= 1;
                assert!(unsafe { resolve(build.exe_size, &wrong) }.is_err());
            }
            assert!(unsafe { resolve(build.exe_size + 1, &build.exe_sha256) }.is_err());
        }
        assert!(select_build(EXE_117_SIZE, &EXE_1171_SHA256).is_err());
        assert!(select_build(EXE_1171_SIZE, &EXE_117_SHA256).is_err());
    }

    #[test]
    fn new_profile_does_not_reuse_legacy_code_addresses() {
        let old = &BUILDS[0];
        let new = &BUILDS[1];
        for (a, b) in [
            (old.texture_resolver, new.texture_resolver),
            (old.external_texture_resolver, new.external_texture_resolver),
            (old.file_opener, new.file_opener),
            (old.memory_file_ctor, new.memory_file_ctor),
            (old.refcount_add, new.refcount_add),
        ] {
            assert_eq!(b - a, 0x70);
        }
        for (a, b) in old
            .memory_methods
            .iter()
            .zip(&new.memory_methods)
            .chain(old.sys_methods.iter().zip(&new.sys_methods))
        {
            assert_eq!(b - a, 0x70);
        }
        assert_eq!(TAB_SELECT_RVAS, [0x808C20, 0x968040]);
        assert_eq!(SCALEFORM_HEAP_RVA, 0x45972D0);
    }

    #[test]
    #[ignore = "requires ERCUI_ELDENRING_EXE pointing to a verified read-only 1.17/1.17.1 sample"]
    fn verified_prologues_match_the_real_executable_fixture() {
        let path = env::var_os("ERCUI_ELDENRING_EXE").expect("ERCUI_ELDENRING_EXE");
        let bytes = fs::read(path).expect("read eldenring.exe");
        let build = select_build(bytes.len() as u64, &crate::sha256::digest(&bytes))
            .expect("verified executable fingerprint");
        assert_eq!(
            raw_pe_rva(
                &bytes,
                build.texture_resolver,
                TEXTURE_RESOLVER_PROLOGUE.len()
            ),
            &TEXTURE_RESOLVER_PROLOGUE
        );
        assert_eq!(
            raw_pe_rva(
                &bytes,
                build.external_texture_resolver,
                EXTERNAL_TEXTURE_RESOLVER_PROLOGUE.len()
            ),
            &EXTERNAL_TEXTURE_RESOLVER_PROLOGUE
        );
        for (rva, prologue) in [
            (build.file_opener, FILE_OPENER_PROLOGUE.as_slice()),
            (build.memory_file_ctor, MEMORY_FILE_CTOR_PROLOGUE.as_slice()),
            (build.refcount_add, REFCOUNT_ADD_PROLOGUE.as_slice()),
            (TAB_SELECT_RVAS[0], TAB_SELECT_INSTRUCTIONS[0].as_slice()),
            (TAB_SELECT_RVAS[1], TAB_SELECT_INSTRUCTIONS[1].as_slice()),
        ] {
            assert_eq!(raw_pe_rva(&bytes, rva, prologue.len()), prologue);
        }
        for (rva, methods) in [
            (MEMORY_FILE_VTABLE_RVA, build.memory_methods),
            (SYS_FILE_VTABLE_RVA, build.sys_methods),
        ] {
            for (index, method) in methods.into_iter().enumerate() {
                let entry = raw_pe_rva(&bytes, rva + index * 8, 8);
                assert_eq!(
                    u64::from_le_bytes(entry.try_into().unwrap()),
                    0x140000000 + method as u64
                );
            }
        }
    }

    fn raw_pe_rva(bytes: &[u8], rva: usize, length: usize) -> &[u8] {
        let nt = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
        assert_eq!(&bytes[nt..nt + 4], b"PE\0\0");
        let section_count = u16::from_le_bytes(bytes[nt + 6..nt + 8].try_into().unwrap()) as usize;
        let optional_size =
            u16::from_le_bytes(bytes[nt + 20..nt + 22].try_into().unwrap()) as usize;
        let table = nt + 24 + optional_size;
        for index in 0..section_count {
            let header = table + index * 40;
            let virtual_address =
                u32::from_le_bytes(bytes[header + 12..header + 16].try_into().unwrap()) as usize;
            let raw_size =
                u32::from_le_bytes(bytes[header + 16..header + 20].try_into().unwrap()) as usize;
            let raw_offset =
                u32::from_le_bytes(bytes[header + 20..header + 24].try_into().unwrap()) as usize;
            if rva >= virtual_address && rva + length <= virtual_address + raw_size {
                let offset = raw_offset + rva - virtual_address;
                return &bytes[offset..offset + length];
            }
        }
        panic!("RVA 0x{rva:08X} is not backed by a raw PE section");
    }
}
