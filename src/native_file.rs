//! 两个显示界面的原生 MemoryFile 桥接。无磁盘资源、临时 GFX 或自造 File vtable。
use crate::{
    compat::{self, NativeFileTargets},
    config::ControllerLayout,
    gfx::{self, Movie},
    hook::PreparedHook,
    logger, sha256, windows,
};
use std::{
    ffi::c_void,
    mem, ptr,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
    },
    time::Instant,
};

type OpenFile = unsafe extern "C" fn(*mut c_void, *const u8, i32, i32) -> *mut c_void;
type Read = unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32;
type Query = unsafe extern "C" fn(*mut c_void) -> i64;
type Seek = unsafe extern "C" fn(*mut c_void, i64, i32) -> i64;
type Valid = unsafe extern "C" fn(*mut c_void) -> u8;
type RefCountAdd = unsafe extern "C" fn(*mut i32, i32) -> i32;
type Delete = unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void;
type Allocate = unsafe extern "C" fn(*mut c_void, usize, *const c_void) -> *mut u8;
type Free = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Construct = unsafe extern "C" fn(*mut u8, *const usize, *const u8, i32) -> *mut c_void;

const MAX_PATH: usize = 512;
const MAX_READ_CALLS: usize = 64;
pub const COUNTERS: usize = 12;
const MATCH: usize = 0;
const RETURNED: usize = 1;
const BUILT: usize = 2;
const TYPE_REJECT: usize = 3;
const READ_REJECT: usize = 4;
const HASH_REJECT: usize = 5;
const BRIDGE_REJECT: usize = 6;
const REOPEN: usize = 7;
const FLAGS_REJECT: usize = 8;
const MAX_US: usize = 9;
const READ_REPORT_MASK: usize = 10;
const KEY_RETURNED: usize = 11;
const READ_REASONS: usize = 8;
const READ_REPORTS: usize = READ_REASONS * 2 * 2;

struct State {
    api: NativeFileTargets,
    layout: ControllerLayout,
    diagnostics: bool,
    patched: OnceLock<Box<[u8]>>,
    key_patched: OnceLock<Box<[u8]>>,
    read_reports: [OnceLock<ReadObservation>; READ_REPORTS],
    bridge_disabled: AtomicBool,
    counters: [AtomicU64; COUNTERS],
}
impl State {
    fn count(&self, index: usize) {
        self.counters[index].fetch_add(1, Ordering::Relaxed);
    }
    fn record_read(&self, kind: FileKind, path: &[u8], report: ReadReport) {
        let index = report.movie as usize * READ_REASONS * 2
            + kind as usize * READ_REASONS
            + report.reason as usize;
        if self.read_reports[index].get().is_some() {
            return;
        }
        // A fixed number of small snapshots; no formatting, logging or extra File calls here.
        let mut observation = ReadObservation {
            kind,
            report,
            path: [0; MAX_PATH],
            path_length: path.len().min(MAX_PATH),
        };
        observation.path[..observation.path_length]
            .copy_from_slice(&path[..observation.path_length]);
        if self.read_reports[index].set(observation).is_ok() {
            self.counters[READ_REPORT_MASK].fetch_or(1 << index, Ordering::Release);
        }
    }
}
static STATE: OnceLock<State> = OnceLock::new();
static ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

pub unsafe fn prepare(
    api: NativeFileTargets,
    layout: ControllerLayout,
    diagnostics: bool,
) -> Result<PreparedHook, String> {
    let hook = unsafe {
        PreparedHook::prepare(
            api.opener as *mut u8,
            &compat::FILE_OPENER_PROLOGUE,
            open_hook as *const c_void,
        )?
    };
    STATE
        .set(State {
            api,
            layout,
            diagnostics,
            patched: OnceLock::new(),
            key_patched: OnceLock::new(),
            read_reports: std::array::from_fn(|_| OnceLock::new()),
            bridge_disabled: AtomicBool::new(false),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
        })
        .map_err(|_| "native file bridge was already initialized".to_owned())?;
    ORIGINAL.store(hook.trampoline(), Ordering::Release);
    Ok(hook)
}

unsafe extern "C" fn open_hook(
    opener: *mut c_void,
    filename: *const u8,
    flags: i32,
    mode: i32,
) -> *mut c_void {
    let original: OpenFile = unsafe { mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    let Some(state) = STATE.get() else {
        return unsafe { original(opener, filename, flags, mode) };
    };
    let mut path = [0u8; MAX_PATH];
    let Some(length) = read_path(filename as usize, &mut path) else {
        return unsafe { original(opener, filename, flags, mode) };
    };
    let Some(kind) = target_path(&path[..length]) else {
        return unsafe { original(opener, filename, flags, mode) };
    };
    state.count(MATCH);
    if !matches!(flags, 1 | 33) {
        state.count(FLAGS_REJECT);
        return unsafe { original(opener, filename, flags, mode) };
    }
    // Always open through the game's original path, so another mod's GFX is never silently ignored.
    let source = unsafe { original(opener, filename, flags, mode) };
    let started = state.diagnostics.then(Instant::now);
    let result = unsafe { replace_movie(state, source, &path[..length], kind) };
    if let Some(started) = started {
        state.counters[MAX_US].fetch_max(
            started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
    }
    match result {
        Replacement::Return(file) => file,
        // Only reached after releasing a known file whose cursor could not be restored. The
        // trampoline bypasses this hook and supplies an untouched new original, without recursion.
        Replacement::Reopen => unsafe { original(opener, filename, flags, mode) },
    }
}

enum Replacement {
    Return(*mut c_void),
    Reopen,
}

unsafe fn replace_movie(
    state: &State,
    source: *mut c_void,
    path: &[u8],
    kind: Movie,
) -> Replacement {
    if state.bridge_disabled.load(Ordering::Acquire) {
        return Replacement::Return(source);
    }
    let Some(file) = File::inspect(source, &state.api) else {
        state.count(TYPE_REJECT);
        return Replacement::Return(source);
    };
    let (outcome, mut report) = unsafe { read_movie(&file, kind) };
    if let ReadOutcome::Bytes(bytes) = &outcome {
        report.input_sha256 = Some(sha256::digest(bytes));
    }
    state.record_read(file.kind, path, report);
    let bytes = match outcome {
        ReadOutcome::Bytes(bytes) => bytes,
        ReadOutcome::Unchanged => {
            state.count(READ_REJECT);
            return Replacement::Return(source);
        }
        ReadOutcome::Reopen => {
            state.count(REOPEN);
            unsafe { file.release() };
            return Replacement::Reopen;
        }
    };
    if report.input_sha256 != Some(kind.hash()) {
        state.count(HASH_REJECT);
        return Replacement::Return(source);
    }
    let cache = match kind {
        Movie::Options => &state.patched,
        Movie::KeyConfig => &state.key_patched,
        #[cfg(all(test, feature = "game-fixtures"))]
        Movie::CommonOptions => return Replacement::Return(source),
    };
    if cache.get().is_none() {
        let Ok(patched) = gfx::patch(kind, &bytes, state.layout) else {
            state.count(HASH_REJECT);
            return Replacement::Return(source);
        };
        // Concurrent first opens can build independently; no caller waits on a builder lock.
        if cache.set(patched.into_boxed_slice()).is_ok() {
            state.count(BUILT);
        }
    }
    let Some(patched) = cache.get() else {
        return Replacement::Return(source);
    };
    match unsafe { create_memory_file(&state.api, path, patched) } {
        Ok(replacement) => {
            unsafe { file.release() };
            state.count(RETURNED);
            if kind == Movie::KeyConfig {
                state.count(KEY_RETURNED);
            }
            Replacement::Return(replacement)
        }
        Err(error) => {
            // A surprising constructor result cannot safely be destroyed as a guessed type.
            // Stop attempting the bridge to bound any quarantined allocation to this attempt.
            if error == CreateError::UnexpectedObject {
                state.bridge_disabled.store(true, Ordering::Release);
            }
            state.count(BRIDGE_REJECT);
            Replacement::Return(source)
        }
    }
}

fn read_path(address: usize, output: &mut [u8; MAX_PATH]) -> Option<usize> {
    for offset in (0..MAX_PATH).step_by(32) {
        let block = &mut output[offset..offset + 32];
        if windows::read_memory_into(address.checked_add(offset)?, block) {
            if let Some(end) = block.iter().position(|byte| *byte == 0) {
                return Some(offset + end);
            }
        } else {
            // A short valid name may end at a page boundary. Never dereference across that page.
            for (index, slot) in block.iter_mut().enumerate() {
                *slot = windows::read_memory::<1>(address.checked_add(offset + index)?)?[0];
                if *slot == 0 {
                    return Some(offset + index);
                }
            }
        }
    }
    None
}
fn target_path(path: &[u8]) -> Option<Movie> {
    // The frequent non-target path incurs no allocation or global state lookup beyond this hook.
    let equal = |a: &[u8], b: &[u8]| {
        a.len() == b.len()
            && a.iter().zip(b).all(|(a, b)| {
                (if *a == b'\\' {
                    b'/'
                } else {
                    a.to_ascii_lowercase()
                }) == *b
            })
    };
    let suffix = |tail: &[u8]| {
        path.len()
            .checked_sub(tail.len())
            .is_some_and(|start| equal(&path[start..], tail))
    };
    if suffix(b"/menu/win/02_040_optionsetting.gfx")
        || equal(path, b"menu/win/02_040_optionsetting.gfx")
        || equal(path, b"menu:/win/02_040_optionsetting.gfx")
    {
        Some(Movie::Options)
    } else if suffix(b"/menu/02_160_keyconfiguration.gfx")
        || equal(path, b"menu/02_160_keyconfiguration.gfx")
        || equal(path, b"menu:/02_160_keyconfiguration.gfx")
    {
        Some(Movie::KeyConfig)
    } else {
        None
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileKind {
    MemoryFile,
    SysFile,
}

struct File {
    pointer: *mut c_void,
    kind: FileKind,
    methods: [usize; 20],
    refcount_add: usize,
}
impl File {
    fn inspect(pointer: *mut c_void, api: &NativeFileTargets) -> Option<Self> {
        let header = windows::read_memory::<16>(pointer as usize)?;
        let vtable = usize::from_le_bytes(header[..8].try_into().ok()?);
        let count = i32::from_le_bytes(header[8..12].try_into().ok()?);
        if !(1..=0x10000).contains(&count) || !(pointer as usize).is_multiple_of(8) {
            return None;
        }
        let (kind, methods) = if vtable == api.memory_vtable {
            (FileKind::MemoryFile, api.memory_methods)
        } else if vtable == api.sys_vtable {
            (FileKind::SysFile, api.sys_methods)
        } else {
            return None;
        };
        let table = windows::read_memory::<160>(vtable)?;
        if !table
            .chunks_exact(8)
            .zip(methods)
            .all(|(bytes, method)| usize::from_le_bytes(bytes.try_into().unwrap()) == method)
        {
            return None;
        }
        if !unsafe { windows::range_is_writable(pointer, 16) } {
            return None;
        }
        Some(Self {
            pointer,
            kind,
            methods,
            refcount_add: api.refcount_add,
        })
    }
    unsafe fn release(&self) {
        let add: RefCountAdd = unsafe { mem::transmute(self.refcount_add) };
        if unsafe { add((self.pointer as *mut u8).add(8).cast(), -1) } == 1 {
            let delete: Delete = unsafe { mem::transmute(self.methods[0]) };
            unsafe { delete(self.pointer, 1) };
        }
    }
}

enum ReadOutcome {
    Bytes(Vec<u8>),
    Unchanged,
    Reopen,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadReason {
    Complete,
    Invalid,
    PositionMismatch,
    LengthMismatch,
    ReadStopped,
    ReadCountOutOfRange,
    ReadLimit,
    RewindFailed,
}
impl ReadReason {
    fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Invalid => "invalid",
            Self::PositionMismatch => "position_mismatch",
            Self::LengthMismatch => "length_mismatch",
            Self::ReadStopped => "read_stopped",
            Self::ReadCountOutOfRange => "read_count_out_of_range",
            Self::ReadLimit => "read_limit",
            Self::RewindFailed => "rewind_failed",
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct ReadReport {
    movie: Movie,
    reason: ReadReason,
    valid: u8,
    position: Option<i64>,
    length: Option<i64>,
    calls: usize,
    bytes_read: usize,
    last_read: Option<i32>,
    rewind: Option<i64>,
    position_after_rewind: Option<i64>,
    header: [u8; 16],
    header_length: usize,
    input_sha256: Option<[u8; 32]>,
}
struct ReadObservation {
    kind: FileKind,
    report: ReadReport,
    path: [u8; MAX_PATH],
    path_length: usize,
}
impl ReadObservation {
    fn log_line(&self) -> String {
        let report = &self.report;
        let value =
            |value: Option<i64>| value.map_or_else(|| "not-queried".into(), |n| n.to_string());
        let header = if report.header_length == 0 {
            "not-read".to_owned()
        } else {
            report.header[..report.header_length]
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>()
        };
        let declared_length = (report.header_length >= 8)
            .then(|| i64::from(u32::from_le_bytes(report.header[4..8].try_into().unwrap())));
        let input_hash = report
            .input_sha256
            .as_ref()
            .map_or_else(|| "not-hashed".into(), sha256::to_hex);
        format!(
            "memory GFX read: reason={}; source={:?}; path={:?}; valid={}; position={}; length={}; expected={}; aligned_memory={}; calls={}; bytes={}; last_read={}; rewind={}; position_after_rewind={}; declared_length={}; input_sha256={}; header={}",
            report.reason.label(),
            self.kind,
            String::from_utf8_lossy(&self.path[..self.path_length]),
            report.valid,
            value(report.position),
            value(report.length),
            report.movie.length(),
            report.movie.length().next_multiple_of(16),
            report.calls,
            report.bytes_read,
            value(report.last_read.map(i64::from)),
            value(report.rewind),
            value(report.position_after_rewind),
            value(declared_length),
            input_hash,
            header
        )
    }
}

fn supported_movie_length(kind: FileKind, length: i64, movie: Movie) -> bool {
    length == movie.length() as i64
        || (kind == FileKind::MemoryFile && length == movie.length().next_multiple_of(16) as i64)
}

unsafe fn read_movie(file: &File, movie: Movie) -> (ReadOutcome, ReadReport) {
    let valid: Valid = unsafe { mem::transmute(file.methods[2]) };
    let mut report = ReadReport {
        movie,
        reason: ReadReason::Invalid,
        valid: unsafe { valid(file.pointer) },
        position: None,
        length: None,
        calls: 0,
        bytes_read: 0,
        last_read: None,
        rewind: None,
        position_after_rewind: None,
        header: [0; 16],
        header_length: 0,
        input_sha256: None,
    };
    if report.valid == 0 {
        return (ReadOutcome::Unchanged, report);
    }
    let tell: Query = unsafe { mem::transmute(file.methods[5]) };
    let length: Query = unsafe { mem::transmute(file.methods[7]) };
    report.position = Some(unsafe { tell(file.pointer) });
    if report.position != Some(0) {
        report.reason = ReadReason::PositionMismatch;
        return (ReadOutcome::Unchanged, report);
    }
    let reported_length = unsafe { length(file.pointer) };
    report.length = Some(reported_length);
    if !supported_movie_length(file.kind, reported_length, movie) {
        report.reason = ReadReason::LengthMismatch;
        return (ReadOutcome::Unchanged, report);
    }
    let read: Read = unsafe { mem::transmute(file.methods[10]) };
    let seek: Seek = unsafe { mem::transmute(file.methods[15]) };
    // The GFX header is part of the exact official hash. Do not read the native envelope's tail,
    // assume it is zero-filled, or allow its size to bypass the logical content fingerprint.
    let mut bytes = vec![0; movie.length()];
    let mut offset = 0;
    report.reason = ReadReason::ReadLimit;
    for _ in 0..MAX_READ_CALLS {
        let remaining = bytes.len() - offset;
        let read_count = unsafe {
            read(
                file.pointer,
                bytes.as_mut_ptr().add(offset),
                remaining as i32,
            )
        };
        report.calls += 1;
        report.last_read = Some(read_count);
        if read_count <= 0 {
            report.reason = ReadReason::ReadStopped;
            break;
        }
        if read_count as usize > remaining {
            report.reason = ReadReason::ReadCountOutOfRange;
            break;
        }
        offset += read_count as usize;
        if offset == bytes.len() {
            report.reason = ReadReason::Complete;
            break;
        }
    }
    report.bytes_read = offset;
    report.header_length = offset.min(report.header.len());
    report.header[..report.header_length].copy_from_slice(&bytes[..report.header_length]);
    report.rewind = Some(unsafe { seek(file.pointer, 0, 0) });
    if report.rewind == Some(0) {
        report.position_after_rewind = Some(unsafe { tell(file.pointer) });
    }
    if report.rewind != Some(0) || report.position_after_rewind != Some(0) {
        report.reason = ReadReason::RewindFailed;
        return (ReadOutcome::Reopen, report);
    }
    if offset == bytes.len() {
        (ReadOutcome::Bytes(bytes), report)
    } else {
        (ReadOutcome::Unchanged, report)
    }
}

struct Heap {
    pointer: *mut c_void,
    allocate: Allocate,
    free: Free,
}
impl Heap {
    fn get(api: &NativeFileTargets) -> Option<Self> {
        let pointer = usize::from_le_bytes(windows::read_memory::<8>(api.heap_slot)?);
        let table = usize::from_le_bytes(windows::read_memory::<8>(pointer)?);
        let methods = windows::read_memory::<0x68>(table)?;
        let allocate = usize::from_le_bytes(methods[0x50..0x58].try_into().ok()?);
        let free = usize::from_le_bytes(methods[0x60..0x68].try_into().ok()?);
        if !unsafe {
            windows::address_is_executable(allocate as *const c_void)
                && windows::address_is_executable(free as *const c_void)
        } {
            return None;
        }
        Some(Self {
            pointer: pointer as *mut c_void,
            allocate: unsafe { mem::transmute::<usize, Allocate>(allocate) },
            free: unsafe { mem::transmute::<usize, Free>(free) },
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CreateError {
    HeapUnavailable,
    Allocation,
    UnexpectedObject,
}
unsafe fn create_memory_file(
    api: &NativeFileTargets,
    path: &[u8],
    bytes: &[u8],
) -> Result<*mut c_void, CreateError> {
    let heap = Heap::get(api).ok_or(CreateError::HeapUnavailable)?;
    unsafe { create_on_heap(api, &heap, path, bytes) }
}
unsafe fn create_on_heap(
    api: &NativeFileTargets,
    heap: &Heap,
    path: &[u8],
    bytes: &[u8],
) -> Result<*mut c_void, CreateError> {
    if path.is_empty()
        || path.len() >= MAX_PATH
        || path.contains(&0)
        || bytes.is_empty()
        || bytes.len() > i32::MAX as usize
    {
        return Err(CreateError::Allocation);
    }
    let object = unsafe { (heap.allocate)(heap.pointer, 0x30, ptr::null()) };
    if object.is_null() {
        return Err(CreateError::Allocation);
    }
    let descriptor = unsafe { (heap.allocate)(heap.pointer, path.len() + 0x10, ptr::null()) };
    if descriptor.is_null() {
        unsafe { (heap.free)(heap.pointer, object.cast()) };
        return Err(CreateError::Allocation);
    }
    // Mirrors 141142CC0: length u64, reference count i32, UTF-8 bytes at +0x0C.
    // Both allocations come from the game's global Scaleform heap, not Rust's allocator.
    unsafe {
        ptr::write_bytes(object, 0, 0x30);
        ptr::write_bytes(descriptor, 0, path.len() + 0x10);
        descriptor.cast::<u64>().write(path.len() as u64);
        descriptor.add(8).cast::<i32>().write(1);
        ptr::copy_nonoverlapping(path.as_ptr(), descriptor.add(12), path.len());
    }
    let path_string = descriptor as usize;
    let construct: Construct = unsafe { mem::transmute(api.constructor) };
    let result = unsafe { construct(object, &path_string, bytes.as_ptr(), bytes.len() as i32) };
    let add: RefCountAdd = unsafe { mem::transmute(api.refcount_add) };
    if unsafe { add(descriptor.add(8).cast(), -1) } == 1 {
        unsafe { (heap.free)(heap.pointer, descriptor.cast()) };
    }
    let snapshot =
        windows::read_memory::<0x30>(object as usize).ok_or(CreateError::UnexpectedObject)?;
    if result != object.cast()
        || !matches_constructed_file(&snapshot, api.memory_vtable, path_string, bytes)
    {
        return Err(CreateError::UnexpectedObject);
    }
    Ok(result)
}
fn matches_constructed_file(object: &[u8; 0x30], vtable: usize, path: usize, bytes: &[u8]) -> bool {
    let pointer = |offset| usize::from_le_bytes(object[offset..offset + 8].try_into().unwrap());
    pointer(0) == vtable
        && object[8..12] == 1i32.to_le_bytes()
        && pointer(0x10) == path
        && pointer(0x18) == bytes.as_ptr() as usize
        && object[0x20..0x24] == (bytes.len() as i32).to_le_bytes()
        && object[0x24..0x28] == [0; 4]
        && object[0x28] == 1
}

pub fn log_progress(previous: &mut [u64; COUNTERS]) {
    let Some(state) = STATE.get() else {
        return;
    };
    for line in progress_lines(state, previous) {
        logger::global_line(line);
    }
}

fn progress_lines(state: &State, previous: &mut [u64; COUNTERS]) -> Vec<String> {
    let values = std::array::from_fn(|index| state.counters[index].load(Ordering::Acquire));
    if values == *previous {
        return Vec::new();
    }
    let new_reports = values[READ_REPORT_MASK] & !previous[READ_REPORT_MASK];
    *previous = values;
    let mut lines = vec![format!(
        "memory GFX: matches={}, returned={}, builds={}, type_reject={}, read_reject={}, hash_reject={}, bridge_reject={}, reopen={}, flags_reject={}, max_bridge_us={}; options_buffer={} bytes; key_buffer={} bytes; key_returned={}; donor_reads=removed; disabled={}",
        values[MATCH],
        values[RETURNED],
        values[BUILT],
        values[TYPE_REJECT],
        values[READ_REJECT],
        values[HASH_REJECT],
        values[BRIDGE_REJECT],
        values[REOPEN],
        values[FLAGS_REJECT],
        values[MAX_US],
        state.patched.get().map_or(0, |bytes| bytes.len()),
        state.key_patched.get().map_or(0, |bytes| bytes.len()),
        values[KEY_RETURNED],
        state.bridge_disabled.load(Ordering::Acquire)
    )];
    for (index, slot) in state.read_reports.iter().enumerate() {
        if new_reports & (1 << index) != 0
            && let Some(observation) = slot.get()
        {
            lines.push(observation.log_line());
        }
    }
    lines
}

#[cfg(all(test, feature = "game-fixtures"))]
#[path = "native_file_tests.rs"]
mod tests;
