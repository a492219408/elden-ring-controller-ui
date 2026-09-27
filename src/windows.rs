use std::{
    ffi::{OsString, c_void},
    mem,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
};

pub type Module = *mut c_void;
pub type Handle = *mut c_void;
pub type ThreadStart = unsafe extern "system" fn(*mut c_void) -> u32;

const PAGE_NOACCESS: u32 = 0x01;
const PAGE_READONLY: u32 = 0x02;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_WRITECOPY: u32 = 0x08;
const PAGE_EXECUTE: u32 = 0x10;
const PAGE_EXECUTE_READ: u32 = 0x20;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
const PAGE_GUARD: u32 = 0x100;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;

#[repr(C)]
struct MemoryBasicInformation {
    base_address: *mut c_void,
    allocation_base: *mut c_void,
    allocation_protect: u32,
    partition_id: u16,
    region_size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(object: Handle) -> i32;
    fn CreateThread(
        thread_attributes: *const c_void,
        stack_size: usize,
        start_address: Option<ThreadStart>,
        parameter: *mut c_void,
        creation_flags: u32,
        thread_id: *mut u32,
    ) -> Handle;
    fn DisableThreadLibraryCalls(module: Module) -> i32;
    fn FlushInstructionCache(process: Handle, base_address: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetModuleFileNameW(module: Module, filename: *mut u16, size: u32) -> u32;
    fn GetModuleHandleW(module_name: *const u16) -> Module;
    fn ReadProcessMemory(
        process: Handle,
        source: *const c_void,
        destination: *mut c_void,
        size: usize,
        copied: *mut usize,
    ) -> i32;
    fn Sleep(milliseconds: u32);
    fn VirtualAlloc(
        address: *mut c_void,
        size: usize,
        allocation_type: u32,
        protect: u32,
    ) -> *mut c_void;
    #[cfg(test)]
    fn VirtualFree(address: *mut c_void, size: usize, free_type: u32) -> i32;
    fn VirtualProtect(
        address: *mut c_void,
        size: usize,
        new_protect: u32,
        old_protect: *mut u32,
    ) -> i32;
    fn VirtualQuery(
        address: *const c_void,
        information: *mut MemoryBasicInformation,
        length: usize,
    ) -> usize;
}

pub unsafe fn disable_thread_notifications(module: Module) {
    let _ = unsafe { DisableThreadLibraryCalls(module) };
}

pub unsafe fn spawn_thread(start: ThreadStart) -> bool {
    let handle = unsafe {
        CreateThread(
            ptr::null(),
            0,
            Some(start),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )
    };
    if handle.is_null() {
        return false;
    }
    let _ = unsafe { CloseHandle(handle) };
    true
}

pub fn sleep(milliseconds: u32) {
    unsafe { Sleep(milliseconds) };
}

// Snapshot game-owned memory into local buffers. Unlike the old per-field VirtualQuery path,
// this does not enumerate memory regions and cannot dereference an unreadable game pointer.
pub fn read_memory<const N: usize>(address: usize) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    read_memory_into(address, &mut bytes).then_some(bytes)
}

pub fn read_memory_into(address: usize, bytes: &mut [u8]) -> bool {
    if address == 0 || bytes.is_empty() || address.checked_add(bytes.len()).is_none() {
        return false;
    }
    let mut copied = 0;
    unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const c_void,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            &mut copied,
        ) != 0
            && copied == bytes.len()
    }
}

pub fn read_wide_name(address: usize, output: &mut [u16]) -> Option<usize> {
    if output.is_empty() || output.len() > 260 || address == 0 {
        return None;
    }
    let mut bytes = [0u8; 520];
    if read_memory_into(address, &mut bytes[..output.len() * 2]) {
        for (i, slot) in output.iter_mut().enumerate() {
            *slot = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
            if *slot == 0 {
                return Some(i);
            }
        }
        return None;
    }
    // A valid short name may end just before an unreadable page. Stop at its first terminator.
    for (i, slot) in output.iter_mut().enumerate() {
        *slot = u16::from_le_bytes(read_memory::<2>(address.checked_add(i * 2)?)?);
        if *slot == 0 {
            return Some(i);
        }
    }
    None
}

pub unsafe fn main_module() -> Result<Module, String> {
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if module.is_null() {
        Err("GetModuleHandleW(NULL) failed".to_owned())
    } else {
        Ok(module)
    }
}

pub unsafe fn module_path(module: Module) -> Result<PathBuf, String> {
    let mut capacity = 512usize;
    loop {
        let mut buffer = vec![0u16; capacity];
        let length = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), capacity as u32) };
        if length == 0 {
            return Err("GetModuleFileNameW failed".to_owned());
        }
        if (length as usize) < capacity - 1 {
            buffer.truncate(length as usize);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        capacity *= 2;
        if capacity > 32_768 {
            return Err("module path exceeds the Windows path limit".to_owned());
        }
    }
}

pub unsafe fn range_is_readable(address: *const c_void, size: usize) -> bool {
    if address.is_null() || size == 0 {
        return false;
    }
    let start = address as usize;
    let Some(end) = start.checked_add(size) else {
        return false;
    };
    let mut current = start;
    while current < end {
        let Some(information) = (unsafe { query_memory(current as *const c_void) }) else {
            return false;
        };
        if information.state != MEM_COMMIT
            || information.protect & (PAGE_NOACCESS | PAGE_GUARD) != 0
            || information.protect
                & (PAGE_READONLY
                    | PAGE_READWRITE
                    | PAGE_WRITECOPY
                    | PAGE_EXECUTE_READ
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY)
                == 0
        {
            return false;
        }
        let region_start = information.base_address as usize;
        let Some(region_end) = region_start.checked_add(information.region_size) else {
            return false;
        };
        if region_end <= current {
            return false;
        }
        current = region_end.min(end);
    }
    true
}

pub unsafe fn range_is_writable(address: *const c_void, size: usize) -> bool {
    if address.is_null() || size == 0 {
        return false;
    }
    let start = address as usize;
    let Some(end) = start.checked_add(size) else {
        return false;
    };
    let mut current = start;
    while current < end {
        let Some(information) = (unsafe { query_memory(current as *const c_void) }) else {
            return false;
        };
        if information.state != MEM_COMMIT
            || information.protect & (PAGE_NOACCESS | PAGE_GUARD) != 0
            || information.protect
                & (PAGE_READWRITE
                    | PAGE_WRITECOPY
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY)
                == 0
        {
            return false;
        }
        let region_start = information.base_address as usize;
        let Some(region_end) = region_start.checked_add(information.region_size) else {
            return false;
        };
        if region_end <= current {
            return false;
        }
        current = region_end.min(end);
    }
    true
}

pub unsafe fn address_is_executable(address: *const c_void) -> bool {
    let Some(information) = (unsafe { query_memory(address) }) else {
        return false;
    };
    information.state == MEM_COMMIT
        && information.protect & (PAGE_NOACCESS | PAGE_GUARD) == 0
        && information.protect
            & (PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY)
            != 0
}

unsafe fn query_memory(address: *const c_void) -> Option<MemoryBasicInformation> {
    if address.is_null() {
        return None;
    }
    let mut information = MemoryBasicInformation {
        base_address: ptr::null_mut(),
        allocation_base: ptr::null_mut(),
        allocation_protect: 0,
        partition_id: 0,
        region_size: 0,
        state: 0,
        protect: 0,
        kind: 0,
    };
    let queried = unsafe {
        VirtualQuery(
            address,
            &mut information,
            mem::size_of::<MemoryBasicInformation>(),
        )
    };
    (queried == mem::size_of::<MemoryBasicInformation>()).then_some(information)
}

pub unsafe fn allocate_executable(size: usize) -> Result<*mut u8, String> {
    if size == 0 {
        return Err("cannot allocate an empty trampoline".to_owned());
    }
    let allocation = unsafe {
        VirtualAlloc(
            ptr::null_mut(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    }
    .cast::<u8>();
    if allocation.is_null() {
        return Err("VirtualAlloc(PAGE_READWRITE) failed".to_owned());
    }
    Ok(allocation)
}

pub unsafe fn protect_execute_read(address: *mut u8, size: usize) -> Result<(), String> {
    if address.is_null() || size == 0 {
        return Err("invalid executable memory range".to_owned());
    }
    let mut old_protect = 0u32;
    if unsafe { VirtualProtect(address.cast(), size, PAGE_EXECUTE_READ, &mut old_protect) } == 0 {
        return Err("VirtualProtect(PAGE_EXECUTE_READ) failed".to_owned());
    }
    if unsafe { FlushInstructionCache(GetCurrentProcess(), address.cast(), size) } == 0 {
        return Err("FlushInstructionCache failed for trampoline".to_owned());
    }
    Ok(())
}

pub unsafe fn write_code(address: *mut u8, bytes: &[u8]) -> Result<(), String> {
    if address.is_null() || bytes.is_empty() {
        return Err("invalid code patch range".to_owned());
    }
    let mut old_protect = 0u32;
    if unsafe {
        VirtualProtect(
            address.cast(),
            bytes.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    } == 0
    {
        return Err("VirtualProtect(PAGE_EXECUTE_READWRITE) failed".to_owned());
    }
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), address, bytes.len()) };
    let flush_ok =
        unsafe { FlushInstructionCache(GetCurrentProcess(), address.cast(), bytes.len()) } != 0;
    let mut ignored = 0u32;
    let restore_ok =
        unsafe { VirtualProtect(address.cast(), bytes.len(), old_protect, &mut ignored) } != 0;
    if !flush_ok {
        return Err("FlushInstructionCache failed after code patch".to_owned());
    }
    if !restore_ok {
        return Err("VirtualProtect failed to restore code protection".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_at_an_unreadable_page_boundary_are_copied_safely() {
        let memory = unsafe { allocate_executable(8192).unwrap() };
        let name = unsafe { memory.add(4096 - 4) };
        unsafe {
            ptr::copy_nonoverlapping([b'A', 0, 0, 0].as_ptr(), name, 4);
        }
        let mut protection = 0;
        assert_ne!(
            unsafe {
                VirtualProtect(
                    memory.add(4096).cast(),
                    4096,
                    PAGE_NOACCESS,
                    &mut protection,
                )
            },
            0
        );
        let mut output = [0; 260];
        let result = read_wide_name(name as usize, &mut output);
        let invalid = read_wide_name(unsafe { memory.add(4096) } as usize, &mut [0; 260]);
        assert_ne!(unsafe { VirtualFree(memory.cast(), 0, 0x8000) }, 0);
        assert_eq!(result, Some(1));
        assert_eq!(output[0], u16::from(b'A'));
        assert!(invalid.is_none());
    }
}
