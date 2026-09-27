#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

mod compat;
mod config;
mod gfx;
mod hook;
mod logger;
mod mapping;
mod native_file;
mod native_tabs;
mod scaleform;
mod sha256;
mod windows;

use config::{Config, ControllerLayout};
use logger::Logger;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::atomic::{AtomicPtr, AtomicU8, Ordering},
    time::Instant,
};
use windows::Module;

const DLL_PROCESS_ATTACH: u32 = 1;
const INITIALIZER_WAIT_TIMEOUT_MS: u32 = 30_000;
// 0 = not started, 1 = running, 2 = succeeded, 3 = failed.
static INIT_STATE: AtomicU8 = AtomicU8::new(0);
static DLL_MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[unsafe(no_mangle)]
/// Windows loader entry point.
///
/// # Safety
/// Only called by the Windows loader with the standard DllMain ABI.
pub unsafe extern "system" fn DllMain(module: Module, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        DLL_MODULE.store(module, Ordering::Release);
        unsafe { windows::disable_thread_notifications(module) };
        start_initialization();
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn elden_ring_controller_ui_init() -> bool {
    start_initialization();
    let started = Instant::now();
    loop {
        match INIT_STATE.load(Ordering::Acquire) {
            2 => return true,
            3 => return false,
            _ if started.elapsed().as_millis() >= u128::from(INITIALIZER_WAIT_TIMEOUT_MS) => {
                return false;
            }
            _ => windows::sleep(1),
        }
    }
}

fn start_initialization() {
    if INIT_STATE
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    if !unsafe { windows::spawn_thread(initialization_thread) } {
        INIT_STATE.store(3, Ordering::Release);
    }
}

unsafe extern "system" fn initialization_thread(_: *mut c_void) -> u32 {
    let succeeded = match catch_unwind(AssertUnwindSafe(initialize)) {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            logger::global_line(format!("INITIALIZATION FAILURE: {error}"));
            false
        }
        // Release 使用 panic=abort；此分支仅能记录可展开构建中的 panic，不能捕获访问违例。
        Err(_) => {
            logger::global_line("INITIALIZATION FAILURE: unexpected Rust panic");
            false
        }
    };
    INIT_STATE.store(if succeeded { 2 } else { 3 }, Ordering::Release);
    0
}

fn initialize() -> Result<(), String> {
    let module = DLL_MODULE.load(Ordering::Acquire);
    if module.is_null() {
        return Err("DLL module handle is unavailable".to_owned());
    }
    let dll_path = unsafe { windows::module_path(module) }?;
    let logger = Logger::new(&dll_path.with_extension("log"))?;
    logger::install_global(&logger)?;
    logger.line(format!(
        "EldenRingControllerUI {} starting from {}",
        env!("CARGO_PKG_VERSION"),
        dll_path.display()
    ));
    let exe_path = unsafe { windows::module_path(std::ptr::null_mut()) }?;
    if !is_elden_ring(&exe_path) {
        let error = format!("refusing unexpected executable: {}", exe_path.display());
        logger.line(&error);
        return Err(error);
    }
    let (config, created) = Config::load_or_create(&dll_path.with_extension("ini"))?;
    if created {
        logger.line("created default configuration");
    }
    logger.line(format!(
        "configuration: layout={}, diagnostics={}; native UI presets; input and icon-display preference remain game-controlled",
        config.layout.as_str(),
        config.diagnostics
    ));
    let metadata = std::fs::metadata(&exe_path).map_err(|error| error.to_string())?;
    let exe_hash = sha256::file(&exe_path).map_err(|error| error.to_string())?;
    logger.line(format!(
        "target executable: {} ({} bytes, SHA-256={})",
        exe_path.display(),
        metadata.len(),
        sha256::to_hex(&exe_hash)
    ));
    let targets = match unsafe { compat::resolve(metadata.len(), &exe_hash) } {
        Ok(targets) => targets,
        Err(error) => {
            logger.line(format!(
                "COMPATIBILITY FAILURE: {error}; no hooks were installed"
            ));
            return Err(error);
        }
    };
    logger.line(format!(
        "verified executable profile: App Ver. {}",
        targets.app_version
    ));
    if config.layout == ControllerLayout::Original {
        logger.line("original layout selected; no game hooks installed");
        return Ok(());
    }
    let mappings = mapping::RuntimeMappings::new(config.layout, config.diagnostics);
    unsafe { scaleform::install(&targets, mappings, config.layout) }?;
    logger.line("native UI presets: native Help/SelectKey; independent PC overview recipe; native tab frame selection; no donor opens, input hooks, atlas writes or embedded movies");
    logger.line(format!(
        "initialization completed: layout={}",
        config.layout.as_str()
    ));
    Ok(())
}

fn is_elden_ring(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("eldenring.exe"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_target_name_case_insensitively() {
        assert!(is_elden_ring(Path::new(r"C:\Game\ELDENRING.EXE")));
        assert!(!is_elden_ring(Path::new(
            r"C:\Game\start_protected_game.exe"
        )));
    }
}
