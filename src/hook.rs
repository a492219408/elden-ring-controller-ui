use std::{ffi::c_void, ptr};

use crate::windows;

const ABSOLUTE_JUMP_SIZE: usize = 14;

pub struct PreparedHook {
    target: *mut u8,
    original: Vec<u8>,
    replacement: *const c_void,
    trampoline: *mut u8,
}

impl PreparedHook {
    pub unsafe fn prepare(
        target: *mut u8,
        expected: &[u8],
        replacement: *const c_void,
    ) -> Result<Self, String> {
        if target.is_null() || replacement.is_null() || expected.len() < ABSOLUTE_JUMP_SIZE {
            return Err("invalid inline-hook arguments".to_owned());
        }
        if !unsafe { windows::range_is_readable(target.cast(), expected.len()) }
            || !unsafe { windows::address_is_executable(target.cast()) }
        {
            return Err("hook target is not readable executable memory".to_owned());
        }
        let actual = unsafe { std::slice::from_raw_parts(target, expected.len()) };
        if actual != expected {
            return Err(format!(
                "hook target prologue differs (expected {}, found {})",
                hex(expected),
                hex(actual)
            ));
        }

        let trampoline_size = expected.len() + ABSOLUTE_JUMP_SIZE;
        let trampoline = unsafe { windows::allocate_executable(trampoline_size) }?;
        unsafe { ptr::copy_nonoverlapping(expected.as_ptr(), trampoline, expected.len()) };
        let return_address = unsafe { target.add(expected.len()) };
        let tail = absolute_jump(return_address.cast());
        unsafe {
            ptr::copy_nonoverlapping(
                tail.as_ptr(),
                trampoline.add(expected.len()),
                ABSOLUTE_JUMP_SIZE,
            )
        };
        unsafe { windows::protect_execute_read(trampoline, trampoline_size) }?;

        Ok(Self {
            target,
            original: expected.to_vec(),
            replacement,
            trampoline,
        })
    }

    pub fn trampoline(&self) -> *mut c_void {
        self.trampoline.cast()
    }

    pub unsafe fn commit(&self) -> Result<(), String> {
        let actual = unsafe { std::slice::from_raw_parts(self.target, self.original.len()) };
        if actual != self.original {
            return Err("hook target changed between preparation and commit".to_owned());
        }
        let mut patch = vec![0x90; self.original.len()];
        patch[..ABSOLUTE_JUMP_SIZE].copy_from_slice(&absolute_jump(self.replacement));
        if let Err(error) = unsafe { windows::write_code(self.target, &patch) } {
            // A flush/protection failure can happen after copying the jump. Restore this target
            // too; the transaction caller only knows about earlier successful commits.
            let rollback = unsafe { windows::write_code(self.target, &self.original) };
            return Err(format!(
                "hook write failed: {error}; current-target rollback={rollback:?}"
            ));
        }
        Ok(())
    }

    pub unsafe fn rollback(&self) -> Result<(), String> {
        unsafe { windows::write_code(self.target, &self.original) }
    }
}

pub trait CodeChange {
    unsafe fn commit(&self) -> Result<(), String>;
    unsafe fn rollback(&self) -> Result<(), String>;
}
impl CodeChange for PreparedHook {
    unsafe fn commit(&self) -> Result<(), String> {
        unsafe { self.commit() }
    }
    unsafe fn rollback(&self) -> Result<(), String> {
        unsafe { self.rollback() }
    }
}

/// A small, verified instruction replacement with no trampoline or callback.
pub struct PreparedPatch {
    target: *mut u8,
    original: Vec<u8>,
    replacement: Vec<u8>,
}
impl PreparedPatch {
    pub unsafe fn prepare(
        target: *mut u8,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<Self, String> {
        if target.is_null()
            || expected.is_empty()
            || expected.len() != replacement.len()
            || !unsafe { windows::range_is_readable(target.cast(), expected.len()) }
            || !unsafe { windows::address_is_executable(target.cast()) }
        {
            return Err("invalid local code-patch target".to_owned());
        }
        if unsafe { std::slice::from_raw_parts(target, expected.len()) } != expected {
            return Err("local code-patch instructions differ".to_owned());
        }
        Ok(Self {
            target,
            original: expected.to_vec(),
            replacement: replacement.to_vec(),
        })
    }
}
impl CodeChange for PreparedPatch {
    unsafe fn commit(&self) -> Result<(), String> {
        if unsafe { std::slice::from_raw_parts(self.target, self.original.len()) } != self.original
        {
            return Err("local code-patch target changed before commit".to_owned());
        }
        if let Err(error) = unsafe { windows::write_code(self.target, &self.replacement) } {
            let rollback = unsafe { windows::write_code(self.target, &self.original) };
            return Err(format!(
                "local code-patch write failed: {error}; rollback={rollback:?}"
            ));
        }
        Ok(())
    }
    unsafe fn rollback(&self) -> Result<(), String> {
        unsafe { windows::write_code(self.target, &self.original) }
    }
}

pub unsafe fn commit_transaction(hooks: &[&dyn CodeChange]) -> Result<(), String> {
    for (index, hook) in hooks.iter().enumerate() {
        if let Err(error) = unsafe { hook.commit() } {
            let mut failures = Vec::new();
            for previous in (0..index).rev() {
                if let Err(rollback) = unsafe { hooks[previous].rollback() } {
                    failures.push(format!("{previous}: {rollback}"));
                }
            }
            return Err(format!(
                "hook transaction failed at {index}: {error}; earlier rollback failures={failures:?}"
            ));
        }
    }
    Ok(())
}

fn absolute_jump(destination: *const c_void) -> [u8; ABSOLUTE_JUMP_SIZE] {
    let mut bytes = [0u8; ABSOLUTE_JUMP_SIZE];
    bytes[..6].copy_from_slice(&[0xFF, 0x25, 0, 0, 0, 0]);
    bytes[6..].copy_from_slice(&(destination as usize as u64).to_le_bytes());
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualFree(address: *mut c_void, size: usize, kind: u32) -> i32;
    }

    #[test]
    fn a_late_prologue_conflict_rolls_back_earlier_hooks_without_overwriting_the_conflict() {
        let memory = unsafe { windows::allocate_executable(64) }.unwrap();
        let expected = [0x90; 16];
        unsafe {
            ptr::write_bytes(memory, 0x90, 64);
            windows::protect_execute_read(memory, 64).unwrap();
        }
        let first =
            unsafe { PreparedHook::prepare(memory, &expected, memory.add(48).cast()) }.unwrap();
        let second =
            unsafe { PreparedHook::prepare(memory.add(16), &expected, memory.add(48).cast()) }
                .unwrap();
        unsafe {
            windows::write_code(memory.add(16), &[0xCC; 16]).unwrap();
        }
        assert!(unsafe { commit_transaction(&[&first, &second]) }.is_err());
        assert_eq!(
            windows::read_memory::<16>(memory as usize).unwrap(),
            expected
        );
        assert_eq!(
            windows::read_memory::<16>(memory as usize + 16).unwrap(),
            [0xCC; 16]
        );
        unsafe {
            assert_ne!(VirtualFree(first.trampoline.cast(), 0, 0x8000), 0);
            assert_ne!(VirtualFree(second.trampoline.cast(), 0, 0x8000), 0);
            assert_ne!(VirtualFree(memory.cast(), 0, 0x8000), 0);
        }
    }

    #[test]
    fn absolute_jump_is_rip_indirect_and_contains_destination() {
        let jump = absolute_jump(0x1234_5678_9ABC_DEF0usize as *const c_void);
        assert_eq!(&jump[..6], &[0xFF, 0x25, 0, 0, 0, 0]);
        assert_eq!(
            u64::from_le_bytes(jump[6..].try_into().unwrap()),
            0x1234_5678_9ABC_DEF0
        );
    }

    #[test]
    fn mixed_transaction_rolls_back_hooks_and_local_patches_on_a_late_conflict() {
        let memory = unsafe { windows::allocate_executable(96) }.unwrap();
        unsafe {
            ptr::write_bytes(memory, 0x90, 96);
            windows::protect_execute_read(memory, 96).unwrap();
        }
        let first =
            unsafe { PreparedHook::prepare(memory, &[0x90; 16], memory.add(80).cast()) }.unwrap();
        let middle =
            unsafe { PreparedPatch::prepare(memory.add(32), &[0x90; 9], &[0xCC; 9]) }.unwrap();
        let last =
            unsafe { PreparedHook::prepare(memory.add(48), &[0x90; 16], memory.add(80).cast()) }
                .unwrap();
        unsafe {
            windows::write_code(memory.add(48), &[0xCC; 16]).unwrap();
        }
        assert!(unsafe { commit_transaction(&[&first, &middle, &last]) }.is_err());
        assert_eq!(
            windows::read_memory::<16>(memory as usize).unwrap(),
            [0x90; 16]
        );
        assert_eq!(
            windows::read_memory::<9>(memory as usize + 32).unwrap(),
            [0x90; 9]
        );
        assert_eq!(
            windows::read_memory::<16>(memory as usize + 48).unwrap(),
            [0xCC; 16]
        );
        unsafe {
            assert_ne!(VirtualFree(first.trampoline.cast(), 0, 0x8000), 0);
            assert_ne!(VirtualFree(last.trampoline.cast(), 0, 0x8000), 0);
            assert_ne!(VirtualFree(memory.cast(), 0, 0x8000), 0);
        }
    }
}
