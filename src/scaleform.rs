use crate::{
    compat::{self, Targets},
    config::ControllerLayout,
    hook::{CodeChange, PreparedHook, commit_transaction},
    logger,
    mapping::{MAX_ENTRIES, RuntimeMappings},
    native_file, native_tabs, windows,
};
use std::{
    ffi::c_void,
    mem, ptr,
    sync::{
        OnceLock,
        atomic::{AtomicPtr, Ordering},
    },
    time::Instant,
};

type TextureResolver =
    unsafe extern "C" fn(*mut c_void, *mut *mut c_void, *const u16, u64) -> *mut c_void;
static MAPPINGS: OnceLock<RuntimeMappings> = OnceLock::new();
static ORIGINAL_TEXTURE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_EXTERNAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

pub unsafe fn install(
    targets: &Targets,
    mappings: RuntimeMappings,
    layout: ControllerLayout,
) -> Result<(), String> {
    let tabs = unsafe { native_tabs::prepare(targets.tab_selectors, layout)? };
    let file = unsafe { native_file::prepare(targets.native_file, layout, mappings.diagnostics)? };
    let texture_hooks = !mappings.entries.is_empty();
    MAPPINGS
        .set(mappings)
        .map_err(|_| "mappings already initialized".to_owned())?;
    let mut hooks = Vec::new();
    if texture_hooks {
        let texture = unsafe {
            PreparedHook::prepare(
                targets.texture_resolver,
                &compat::TEXTURE_RESOLVER_PROLOGUE,
                texture_hook as *const c_void,
            )?
        };
        let external = unsafe {
            PreparedHook::prepare(
                targets.external_texture_resolver,
                &compat::EXTERNAL_TEXTURE_RESOLVER_PROLOGUE,
                external_hook as *const c_void,
            )?
        };
        ORIGINAL_TEXTURE.store(texture.trampoline(), Ordering::Release);
        ORIGINAL_EXTERNAL.store(external.trampoline(), Ordering::Release);
        hooks.push(texture);
        hooks.push(external);
    }
    hooks.push(file);
    let changes: Vec<&dyn CodeChange> = hooks
        .iter()
        .map(|hook| hook as &dyn CodeChange)
        .chain(tabs.iter().map(|patch| patch as &dyn CodeChange))
        .collect();
    unsafe { commit_transaction(&changes) }?;
    logger::global_line(format!(
        "installed {} UI-only hooks; no atlas mutation, input hooks, or per-frame UI work",
        hooks.len()
    ));
    let frames = native_tabs::frames(layout);
    logger::global_line(format!(
        "native tab selectors: local_sites=2; game_options_frame={}; controller_settings_frame={}; shared_constant_unchanged=true",
        frames[0], frames[1]
    ));
    if !unsafe { windows::spawn_thread(diagnostics_thread) } {
        logger::global_line("diagnostic worker unavailable; UI hooks will not write logs");
    }
    Ok(())
}
unsafe extern "C" fn texture_hook(
    repo: *mut c_void,
    output: *mut *mut c_void,
    name: *const u16,
    flags: u64,
) -> *mut c_void {
    let original: TextureResolver =
        unsafe { mem::transmute(ORIGINAL_TEXTURE.load(Ordering::Acquire)) };
    unsafe { resolve(original, MAPPINGS.get(), repo, output, name, flags) }
}
unsafe extern "C" fn external_hook(
    repo: *mut c_void,
    output: *mut *mut c_void,
    name: *const u16,
    flags: u64,
) -> *mut c_void {
    let original: TextureResolver =
        unsafe { mem::transmute(ORIGINAL_EXTERNAL.load(Ordering::Acquire)) };
    unsafe { resolve(original, MAPPINGS.get(), repo, output, name, flags) }
}
unsafe fn resolve(
    original: TextureResolver,
    mappings: Option<&RuntimeMappings>,
    repo: *mut c_void,
    output: *mut *mut c_void,
    name: *const u16,
    flags: u64,
) -> *mut c_void {
    let Some(mappings) = mappings else {
        return unsafe { original(repo, output, name, flags) };
    };
    let started = mappings.diagnostics.then(Instant::now);
    let Some(index) = mappings.decide(name) else {
        return unsafe { original(repo, output, name, flags) };
    };
    let entry = &mappings.entries[index];
    let game_start = mappings.diagnostics.then(Instant::now);
    let mut result = unsafe { original(repo, output, entry.target_wide.as_ptr(), flags) };
    let missing = windows::read_memory::<8>(output as usize).is_some_and(|value| value == [0; 8]);
    if missing {
        result = unsafe { original(repo, output, name, flags) };
    }
    let game_us = game_start.map_or(0, elapsed_us);
    if let Some(started) = started {
        entry.stats.record(
            missing,
            elapsed_us(started).saturating_sub(game_us),
            game_us,
        );
    }
    result
}
fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}
unsafe extern "system" fn diagnostics_thread(_: *mut c_void) -> u32 {
    let Some(mappings) = MAPPINGS.get() else {
        return 0;
    };
    let mut last = [0u64; MAX_ENTRIES];
    let mut last_gfx = [0; native_file::COUNTERS];
    loop {
        windows::sleep(1000);
        native_file::log_progress(&mut last_gfx);
        for (entry, previous) in mappings.entries.iter().zip(last.iter_mut()) {
            let calls = entry.stats.calls.load(Ordering::Acquire);
            if calls == *previous {
                continue;
            }
            *previous = calls;
            logger::global_line(format!(
                "key-guide name: {} -> {}; calls={}, fallbacks={}, max_hook_us={}, max_game_us={}",
                entry.source,
                entry.target,
                calls,
                entry.stats.failures.load(Ordering::Relaxed),
                entry.stats.max_hook_us.load(Ordering::Relaxed),
                entry.stats.max_game_us.load(Ordering::Relaxed)
            ));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Probe {
        names: Vec<String>,
        flags: Vec<u64>,
        absent: bool,
    }
    unsafe extern "C" fn resolver(
        repo: *mut c_void,
        output: *mut *mut c_void,
        name: *const u16,
        flags: u64,
    ) -> *mut c_void {
        let probe = unsafe { &mut *repo.cast::<Probe>() };
        let mut text = [0; 260];
        let length = windows::read_wide_name(name as usize, &mut text).unwrap();
        probe.names.push(String::from_utf16_lossy(&text[..length]));
        probe.flags.push(flags);
        unsafe {
            *output = if probe.absent && probe.names.len() == 1 {
                ptr::null_mut()
            } else {
                repo
            };
        }
        output.cast()
    }
    #[test]
    fn display_only_forwarding_preserves_original_calls_and_falls_back_with_the_exact_source_path()
    {
        let mappings = RuntimeMappings::new(ControllerLayout::DualSense, false);
        for (name, expected, absent) in [
            ("KG_Key_A", vec!["KG_Key_A"], false),
            ("KG_Mouse_L", vec!["KG_Mouse_L"], false),
            ("KG_L1", vec!["KG_PS5_L1"], false),
            (
                "menu:/KG_L1.tga",
                vec!["KG_PS5_L1", "menu:/KG_L1.tga"],
                true,
            ),
        ] {
            let mut probe = Probe {
                names: Vec::new(),
                flags: Vec::new(),
                absent,
            };
            let repo = (&mut probe as *mut Probe).cast();
            let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
            let mut output = ptr::null_mut();
            assert_eq!(
                unsafe {
                    resolve(
                        resolver,
                        Some(&mappings),
                        repo,
                        &mut output,
                        wide.as_ptr(),
                        0x1234,
                    )
                },
                (&mut output as *mut *mut c_void).cast()
            );
            assert_eq!(output, repo);
            assert_eq!(probe.names, expected);
            assert!(probe.flags.iter().all(|v| *v == 0x1234));
        }
    }
}
