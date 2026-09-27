//! 仅修改两个页签构造点的外观偏移；不改共享常量、全局跳帧函数或输入逻辑。
use crate::{compat, config::ControllerLayout, hook::PreparedPatch};

pub fn frames(layout: ControllerLayout) -> [u8; 2] {
    let offset = offset(layout);
    [20 + offset, 30 + offset]
}
fn offset(layout: ControllerLayout) -> u8 {
    match layout {
        ControllerLayout::Original => 0,
        ControllerLayout::DualShock4 => 1,
        ControllerLayout::DualSense => 3,
        ControllerLayout::XboxSeries => 4,
    }
}
fn instructions(layout: ControllerLayout, original: &[u8; 9]) -> [u8; 9] {
    let mut result = *original;
    // mov edx, [rip + display_constant] -> mov edx, immediate; nop.
    // The following add edx, 20/30 is preserved, including its flags and instruction boundary.
    result[..6].copy_from_slice(&[0xBA, offset(layout), 0, 0, 0, 0x90]);
    result
}
pub unsafe fn prepare(
    targets: [*mut u8; 2],
    layout: ControllerLayout,
) -> Result<[PreparedPatch; 2], String> {
    if layout == ControllerLayout::Original {
        return Err("original layout must not install tab patches".to_owned());
    }
    let make = |i: usize| unsafe {
        PreparedPatch::prepare(
            targets[i],
            &compat::TAB_SELECT_INSTRUCTIONS[i],
            &instructions(layout, &compat::TAB_SELECT_INSTRUCTIONS[i]),
        )
    };
    Ok([make(0)?, make(1)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hook::{CodeChange, commit_transaction},
        windows,
    };
    use std::{ffi::c_void, mem};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualFree(address: *mut c_void, size: usize, kind: u32) -> i32;
    }

    #[test]
    fn local_instructions_select_native_frames_and_execute_without_any_game_callbacks() {
        for (layout, expected) in [
            (ControllerLayout::DualShock4, [21, 31]),
            (ControllerLayout::DualSense, [23, 33]),
            (ControllerLayout::XboxSeries, [24, 34]),
        ] {
            assert_eq!(frames(layout), expected);
            let memory = unsafe { windows::allocate_executable(64) }.unwrap();
            for i in 0..2 {
                let mut stub = compat::TAB_SELECT_INSTRUCTIONS[i].to_vec();
                stub.extend_from_slice(&[0x8B, 0xC2, 0xC3]); // mov eax, edx; ret
                unsafe {
                    windows::write_code(memory.add(i * 16), &stub).unwrap();
                }
            }
            unsafe {
                windows::protect_execute_read(memory, 64).unwrap();
            }
            let patches = unsafe { prepare([memory, memory.add(16)], layout) }.unwrap();
            unsafe {
                commit_transaction(&[&patches[0], &patches[1]]).unwrap();
            }
            for (i, frame) in expected.iter().enumerate() {
                let execute: unsafe extern "C" fn() -> u32 =
                    unsafe { mem::transmute(memory.add(i * 16)) };
                assert_eq!(unsafe { execute() }, u32::from(*frame));
                unsafe {
                    patches[i].rollback().unwrap();
                }
                assert_eq!(
                    windows::read_memory::<9>(memory as usize + i * 16).unwrap(),
                    compat::TAB_SELECT_INSTRUCTIONS[i]
                );
            }
            unsafe {
                assert_ne!(VirtualFree(memory.cast(), 0, 0x8000), 0);
            }
        }
        assert_eq!(frames(ControllerLayout::Original), [20, 30]);
        assert!(unsafe { prepare([std::ptr::null_mut(); 2], ControllerLayout::Original) }.is_err());
    }

    #[test]
    fn conflicting_second_tab_rolls_back_the_first_without_overwriting_foreign_code() {
        let memory = unsafe { windows::allocate_executable(64) }.unwrap();
        unsafe {
            windows::write_code(memory, &compat::TAB_SELECT_INSTRUCTIONS[0]).unwrap();
            windows::write_code(memory.add(16), &compat::TAB_SELECT_INSTRUCTIONS[1]).unwrap();
        }
        unsafe {
            windows::protect_execute_read(memory, 64).unwrap();
        }
        let patches =
            unsafe { prepare([memory, memory.add(16)], ControllerLayout::DualSense) }.unwrap();
        unsafe {
            windows::write_code(memory.add(16), &[0xCC; 9]).unwrap();
        }
        assert!(unsafe { commit_transaction(&[&patches[0], &patches[1]]) }.is_err());
        assert_eq!(
            windows::read_memory::<9>(memory as usize).unwrap(),
            compat::TAB_SELECT_INSTRUCTIONS[0]
        );
        assert_eq!(
            windows::read_memory::<9>(memory as usize + 16).unwrap(),
            [0xCC; 9]
        );
        unsafe {
            assert_ne!(VirtualFree(memory.cast(), 0, 0x8000), 0);
        }
    }
}
