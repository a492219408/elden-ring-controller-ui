//! ABI 模拟测试；不能替代真实游戏对原生实现的动态验证。
use super::*;
const BASENAME: &[u8] = b"02_040_optionsetting.gfx";
const ALIGNED_GFX_LENGTH: usize = Movie::Options.length().next_multiple_of(16);

// 原生 File ABI 回归测试；Options 不再接受或打开供体。
unsafe fn replace_file(state: &State, source: *mut c_void, path: &[u8]) -> Replacement {
    unsafe { replace_movie(state, source, path, Movie::Options) }
}
unsafe fn read_and_rewind(file: &File) -> (ReadOutcome, ReadReport) {
    unsafe { read_movie(file, Movie::Options) }
}
fn supported_file_length(kind: FileKind, length: i64) -> bool {
    supported_movie_length(kind, length, Movie::Options)
}
fn expected_options(layout: ControllerLayout) -> Vec<u8> {
    gfx::build_options_gfx(layout).unwrap().0
}
use std::{
    alloc::{Layout, alloc, dealloc},
    sync::{
        Arc,
        atomic::{AtomicI32, AtomicUsize},
    },
};

#[repr(C)]
struct MockHeap {
    vtable: usize,
    table: [usize; 13],
    memory_vtable: usize,
    calls: AtomicUsize,
    allocations: AtomicUsize,
    frees: AtomicUsize,
    fail_at: usize,
}
unsafe extern "C" fn mock_alloc(heap: *mut c_void, length: usize, _: *const c_void) -> *mut u8 {
    let state = unsafe { &*(heap as *const MockHeap) };
    let index = state.calls.fetch_add(1, Ordering::Relaxed) + 1;
    if index == state.fail_at {
        return ptr::null_mut();
    }
    let allocation = unsafe { alloc(Layout::from_size_align(length + 16, 16).unwrap()) };
    if allocation.is_null() {
        return allocation;
    }
    unsafe {
        allocation.cast::<usize>().write(length);
        allocation.add(8).cast::<usize>().write(heap as usize);
    }
    state.allocations.fetch_add(1, Ordering::Relaxed);
    unsafe { allocation.add(16) }
}
unsafe extern "C" fn mock_free(heap: *mut c_void, pointer: *mut c_void) {
    let allocation = unsafe { (pointer as *mut u8).sub(16) };
    let size = unsafe { allocation.cast::<usize>().read() };
    assert_eq!(
        unsafe { allocation.add(8).cast::<usize>().read() },
        heap as usize
    );
    unsafe { &*(heap as *const MockHeap) }
        .frees
        .fetch_add(1, Ordering::Relaxed);
    unsafe { dealloc(allocation, Layout::from_size_align(size + 16, 16).unwrap()) };
}
unsafe extern "C" fn mock_add(value: *mut i32, delta: i32) -> i32 {
    unsafe { AtomicI32::from_ptr(value) }.fetch_add(delta, Ordering::SeqCst)
}
#[repr(C)]
struct MockMemory {
    vtable: usize,
    references: AtomicI32,
    padding: u32,
    path: usize,
    buffer: *const u8,
    length: i32,
    cursor: i32,
    valid: u8,
    tail: [u8; 7],
}
unsafe extern "C" fn mock_construct(
    object: *mut u8,
    path: *const usize,
    buffer: *const u8,
    length: i32,
) -> *mut c_void {
    let heap = unsafe { object.sub(8).cast::<*const MockHeap>().read() };
    let path = unsafe { *path };
    unsafe { mock_add((path + 8) as *mut i32, 1) };
    unsafe {
        object.cast::<MockMemory>().write(MockMemory {
            vtable: (*heap).memory_vtable,
            references: AtomicI32::new(1),
            padding: 0,
            path,
            buffer,
            length,
            cursor: 0,
            valid: 1,
            tail: [0; 7],
        })
    };
    object.cast()
}
unsafe extern "C" fn memory_delete(pointer: *mut c_void, flags: u32) -> *mut c_void {
    assert_eq!(flags, 1);
    let file = unsafe { &*(pointer as *const MockMemory) };
    assert_eq!(file.references.load(Ordering::Relaxed), 0);
    let heap = unsafe { (pointer as *const u8).sub(8).cast::<*mut c_void>().read() };
    if unsafe { mock_add((file.path + 8) as *mut i32, -1) } == 1 {
        unsafe { mock_free(heap, file.path as *mut c_void) };
    }
    unsafe { mock_free(heap, pointer) };
    pointer
}
unsafe extern "C" fn memory_valid(pointer: *mut c_void) -> u8 {
    unsafe { (*(pointer as *const MockMemory)).valid }
}
unsafe extern "C" fn memory_tell(pointer: *mut c_void) -> i64 {
    unsafe { (*(pointer as *const MockMemory)).cursor as i64 }
}
unsafe extern "C" fn memory_length(pointer: *mut c_void) -> i64 {
    unsafe { (*(pointer as *const MockMemory)).length as i64 }
}
unsafe extern "C" fn memory_read(pointer: *mut c_void, output: *mut u8, size: i32) -> i32 {
    let file = unsafe { &mut *(pointer as *mut MockMemory) };
    let size = size.min(file.length - file.cursor);
    unsafe {
        ptr::copy_nonoverlapping(file.buffer.add(file.cursor as usize), output, size as usize)
    };
    file.cursor += size;
    size
}
unsafe extern "C" fn memory_seek(pointer: *mut c_void, offset: i64, origin: i32) -> i64 {
    assert_eq!(origin, 0);
    unsafe {
        (*(pointer as *mut MockMemory)).cursor = offset as i32;
    }
    offset
}

#[repr(C)]
struct MockSource {
    vtable: usize,
    references: AtomicI32,
    padding: u32,
    bytes: Vec<u8>,
    valid: u8,
    cursor: usize,
    tell_calls: usize,
    length_calls: usize,
    seek_calls: usize,
    read_calls: usize,
    forced_read: Option<i32>,
    chunk: usize,
    stop_after: usize,
    fail_seek: bool,
    seek_without_moving: bool,
    deleted: Arc<AtomicUsize>,
}
unsafe extern "C" fn source_valid(pointer: *mut c_void) -> u8 {
    unsafe { (*(pointer as *const MockSource)).valid }
}
unsafe extern "C" fn source_tell(pointer: *mut c_void) -> i64 {
    let file = unsafe { &mut *(pointer as *mut MockSource) };
    file.tell_calls += 1;
    file.cursor as i64
}
unsafe extern "C" fn source_length(pointer: *mut c_void) -> i64 {
    let file = unsafe { &mut *(pointer as *mut MockSource) };
    file.length_calls += 1;
    file.bytes.len() as i64
}
unsafe extern "C" fn source_read(pointer: *mut c_void, output: *mut u8, size: i32) -> i32 {
    let file = unsafe { &mut *(pointer as *mut MockSource) };
    file.read_calls += 1;
    if let Some(result) = file.forced_read {
        return result;
    }
    if file.read_calls > file.stop_after {
        return 0;
    }
    let size = (size as usize)
        .min(file.chunk)
        .min(file.bytes.len() - file.cursor);
    unsafe { ptr::copy_nonoverlapping(file.bytes.as_ptr().add(file.cursor), output, size) };
    file.cursor += size;
    size as i32
}
unsafe extern "C" fn source_seek(pointer: *mut c_void, offset: i64, origin: i32) -> i64 {
    assert_eq!(origin, 0);
    let file = unsafe { &mut *(pointer as *mut MockSource) };
    file.seek_calls += 1;
    if file.fail_seek {
        return -1;
    }
    if !file.seek_without_moving {
        file.cursor = offset as usize;
    }
    offset
}
unsafe extern "C" fn source_delete(pointer: *mut c_void, flags: u32) -> *mut c_void {
    assert_eq!(flags, 1);
    let file = unsafe { Box::from_raw(pointer as *mut MockSource) };
    assert_eq!(file.references.load(Ordering::Relaxed), 0);
    file.deleted.fetch_add(1, Ordering::Relaxed);
    pointer
}
struct Fixture {
    memory_table: Box<[usize; 20]>,
    sys_table: Box<[usize; 20]>,
    heap: Box<MockHeap>,
    heap_slot: Box<usize>,
}
impl Fixture {
    fn new(fail_at: usize) -> Self {
        let mut memory_table = Box::new([memory_valid as *const () as usize; 20]);
        for (index, function) in [
            (0, memory_delete as *const ()),
            (2, memory_valid as *const ()),
            (5, memory_tell as *const ()),
            (7, memory_length as *const ()),
            (10, memory_read as *const ()),
            (15, memory_seek as *const ()),
        ] {
            memory_table[index] = function as usize;
        }
        let mut sys_table = Box::new([source_valid as *const () as usize; 20]);
        for (index, function) in [
            (0, source_delete as *const ()),
            (2, source_valid as *const ()),
            (5, source_tell as *const ()),
            (7, source_length as *const ()),
            (10, source_read as *const ()),
            (15, source_seek as *const ()),
        ] {
            sys_table[index] = function as usize;
        }
        let mut heap = Box::new(MockHeap {
            vtable: 0,
            table: [0; 13],
            memory_vtable: memory_table.as_ptr() as usize,
            calls: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            frees: AtomicUsize::new(0),
            fail_at,
        });
        heap.table[10] = mock_alloc as *const () as usize;
        heap.table[12] = mock_free as *const () as usize;
        heap.vtable = heap.table.as_ptr() as usize;
        let heap_slot = Box::new(&*heap as *const MockHeap as usize);
        Self {
            memory_table,
            sys_table,
            heap,
            heap_slot,
        }
    }
    fn state(&self, layout: ControllerLayout) -> State {
        State {
            api: NativeFileTargets {
                opener: 0,
                constructor: mock_construct as *const () as usize,
                refcount_add: mock_add as *const () as usize,
                heap_slot: &*self.heap_slot as *const usize as usize,
                memory_vtable: self.memory_table.as_ptr() as usize,
                sys_vtable: self.sys_table.as_ptr() as usize,
                memory_methods: *self.memory_table,
                sys_methods: *self.sys_table,
            },
            layout,
            diagnostics: false,
            patched: OnceLock::new(),
            key_patched: OnceLock::new(),
            read_reports: std::array::from_fn(|_| OnceLock::new()),
            bridge_disabled: AtomicBool::new(false),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
    fn source(&self, bytes: Vec<u8>) -> (Box<MockSource>, Arc<AtomicUsize>) {
        let deleted = Arc::new(AtomicUsize::new(0));
        (
            Box::new(MockSource {
                vtable: self.sys_table.as_ptr() as usize,
                references: AtomicI32::new(1),
                padding: 0,
                bytes,
                valid: 1,
                cursor: 0,
                tell_calls: 0,
                length_calls: 0,
                seek_calls: 0,
                read_calls: 0,
                forced_read: None,
                chunk: usize::MAX,
                stop_after: usize::MAX,
                fail_seek: false,
                seek_without_moving: false,
                deleted: deleted.clone(),
            }),
            deleted,
        )
    }
    fn all_freed(&self) {
        assert_eq!(
            self.heap.allocations.load(Ordering::Relaxed),
            self.heap.frees.load(Ordering::Relaxed)
        );
    }
}
fn returned(result: Replacement) -> *mut c_void {
    match result {
        Replacement::Return(pointer) => pointer,
        Replacement::Reopen => panic!("unexpected reopen"),
    }
}
fn report(state: &State, kind: FileKind, reason: ReadReason) -> &ReadObservation {
    state.read_reports[kind as usize * READ_REASONS + reason as usize]
        .get()
        .unwrap()
}

#[test]
fn path_matching_is_bounded_and_exact() {
    for name in [
        "data0:/menu/win/02_040_optionsetting.gfx",
        "menu:/win/02_040_OPTIONSETTING.GFX",
        r"data0:\menu\win\02_040_optionsetting.gfx",
    ] {
        assert_eq!(target_path(name.as_bytes()), Some(Movie::Options));
    }
    for name in [
        "x02_040_optionsetting.gfx",
        "02_040_optionsetting.gfx.bak",
        "02_040_optionsetting.gfx/other",
        "02_040_optionsetting.gfx?x=1",
        "",
        "02_040_other.gfx",
        "data0:/menu/02_040_optionsetting.gfx",
        "02_040_optionsetting.gfx",
    ] {
        assert_eq!(target_path(name.as_bytes()), None);
    }
    let name = b"menu:/win/02_040_optionsetting.gfx\0";
    let mut output = [0; MAX_PATH];
    assert_eq!(
        read_path(name.as_ptr() as usize, &mut output),
        Some(name.len() - 1)
    );
    assert_eq!(read_path(1, &mut output), None);
    assert_eq!(
        read_path([1u8; MAX_PATH].as_ptr() as usize, &mut output),
        None
    );
}

#[test]
fn both_layouts_round_trip_through_native_abi_and_release_every_object() {
    assert_eq!(mem::size_of::<MockMemory>(), 0x30);
    for layout in [
        ControllerLayout::DualSense,
        ControllerLayout::DualShock4,
        ControllerLayout::XboxSeries,
    ] {
        let fixture = Fixture::new(0);
        let state = fixture.state(layout);
        for _ in 0..3 {
            let (mut source, deleted) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
            source.chunk = 1024;
            let source = Box::into_raw(source).cast();
            let pointer = returned(unsafe {
                replace_file(&state, source, b"menu:/win/02_040_optionsetting.gfx")
            });
            assert_ne!(pointer, source);
            assert_eq!(deleted.load(Ordering::Relaxed), 1);
            let file = File::inspect(pointer, &state.api).unwrap();
            let mut bytes = vec![0; expected_options(layout).len()];
            assert_eq!(
                unsafe { memory_read(pointer, bytes.as_mut_ptr(), bytes.len() as i32) },
                bytes.len() as i32
            );
            assert_eq!(bytes, gfx::build_options_gfx(layout).unwrap().0);
            assert_eq!(
                unsafe { memory_tell(pointer) },
                expected_options(layout).len() as i64
            );
            assert_eq!(unsafe { memory_seek(pointer, 0, 0) }, 0);
            unsafe { file.release() };
        }
        assert_eq!(state.counters[BUILT].load(Ordering::Relaxed), 1);
        assert_eq!(state.counters[RETURNED].load(Ordering::Relaxed), 3);
        assert_eq!(fixture.heap.allocations.load(Ordering::Relaxed), 6);
        fixture.all_freed();
        assert_eq!(
            state.patched.get().unwrap().len(),
            expected_options(layout).len()
        );
        let observation = report(&state, FileKind::SysFile, ReadReason::Complete);
        assert_eq!(observation.report.bytes_read, gfx::OFFICIAL_GFX_LENGTH);
        assert_eq!(
            observation.report.calls,
            gfx::OFFICIAL_GFX_LENGTH.div_ceil(1024)
        );
        assert_eq!(observation.report.rewind, Some(0));
        assert_eq!(observation.report.position_after_rewind, Some(0));
        assert_eq!(&observation.report.header, &gfx::OFFICIAL_OPTIONS_GFX[..16]);
        assert_eq!(
            observation.report.input_sha256,
            Some(gfx::OFFICIAL_GFX_SHA256)
        );
    }
}

#[test]
fn only_the_observed_memory_envelope_is_added_to_the_length_gate() {
    assert_eq!(ALIGNED_GFX_LENGTH, 44016);
    assert!(supported_file_length(FileKind::MemoryFile, 44016));
    assert!(!supported_file_length(FileKind::SysFile, 44016));
    for kind in [FileKind::MemoryFile, FileKind::SysFile] {
        assert!(supported_file_length(kind, 44007));
        for length in [-1, 0, 44006, 44008, 44015, 44017, 44032, 46401, i64::MAX] {
            assert!(!supported_file_length(kind, length));
        }
    }
}

#[test]
fn aligned_memory_files_patch_both_layouts_without_reading_or_relying_on_the_tail() {
    for layout in [
        ControllerLayout::DualSense,
        ControllerLayout::DualShock4,
        ControllerLayout::XboxSeries,
    ] {
        for tail in [0, 0xa5] {
            let fixture = Fixture::new(0);
            let state = fixture.state(layout);
            let mut original = gfx::OFFICIAL_OPTIONS_GFX.to_vec();
            original.resize(ALIGNED_GFX_LENGTH, tail);
            let saved = original.clone();
            let source = unsafe { create_memory_file(&state.api, BASENAME, &original) }.unwrap();
            let original_file = File::inspect(source, &state.api).unwrap();
            // Retain one extra native reference to verify the original cursor and ownership.
            assert_eq!(unsafe { mock_add((source as *mut u8).add(8).cast(), 1) }, 1);
            let replacement = returned(unsafe { replace_file(&state, source, BASENAME) });
            assert_ne!(replacement, source);
            assert_eq!(unsafe { memory_tell(source) }, 0);
            assert_eq!(unsafe { memory_length(source) }, ALIGNED_GFX_LENGTH as i64);
            assert_eq!(original, saved);
            let observation = report(&state, FileKind::MemoryFile, ReadReason::Complete);
            assert_eq!(observation.report.length, Some(ALIGNED_GFX_LENGTH as i64));
            assert_eq!(observation.report.bytes_read, gfx::OFFICIAL_GFX_LENGTH);
            assert_eq!(observation.report.calls, 1);
            assert_eq!(
                observation.report.input_sha256,
                Some(gfx::OFFICIAL_GFX_SHA256)
            );
            assert!(
                observation
                    .log_line()
                    .contains("declared_length=44007; input_sha256=170996C2")
            );
            let mut output = vec![0; expected_options(layout).len()];
            assert_eq!(
                unsafe { memory_read(replacement, output.as_mut_ptr(), output.len() as i32) },
                output.len() as i32
            );
            assert_eq!(output, gfx::build_options_gfx(layout).unwrap().0);
            assert_eq!(state.counters[RETURNED].load(Ordering::Relaxed), 1);
            unsafe { original_file.release() };
            unsafe { File::inspect(replacement, &state.api).unwrap().release() };
            fixture.all_freed();
        }
    }
}

#[test]
fn aligned_memory_with_changed_header_or_body_is_still_rejected_by_the_full_hash() {
    for offset in [4, 100] {
        let fixture = Fixture::new(0);
        let state = fixture.state(ControllerLayout::DualSense);
        let mut original = gfx::OFFICIAL_OPTIONS_GFX.to_vec();
        original[offset] ^= 1;
        original.resize(ALIGNED_GFX_LENGTH, 0xa5);
        let saved = original.clone();
        let source = unsafe { create_memory_file(&state.api, BASENAME, &original) }.unwrap();
        assert_eq!(
            returned(unsafe { replace_file(&state, source, BASENAME) }),
            source
        );
        assert_eq!(original, saved);
        assert_eq!(unsafe { memory_tell(source) }, 0);
        assert!(state.patched.get().is_none());
        assert_eq!(state.counters[HASH_REJECT].load(Ordering::Relaxed), 1);
        assert_eq!(state.counters[RETURNED].load(Ordering::Relaxed), 0);
        let observation = report(&state, FileKind::MemoryFile, ReadReason::Complete);
        assert_eq!(
            observation.report.input_sha256,
            Some(sha256::digest(&original[..gfx::OFFICIAL_GFX_LENGTH]))
        );
        assert_ne!(
            observation.report.input_sha256,
            Some(gfx::OFFICIAL_GFX_SHA256)
        );
        unsafe { File::inspect(source, &state.api).unwrap().release() };
        fixture.all_freed();
    }
}

#[test]
fn other_native_envelopes_and_aligned_sysfiles_remain_unread() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    for length in [44006, 44008, 44015, 44017, 44032, 46401] {
        let mut bytes = gfx::OFFICIAL_OPTIONS_GFX.to_vec();
        bytes.resize(length, 0);
        let source = unsafe { create_memory_file(&state.api, BASENAME, &bytes) }.unwrap();
        let file = File::inspect(source, &state.api).unwrap();
        let (outcome, observation) = unsafe { read_and_rewind(&file) };
        assert!(matches!(outcome, ReadOutcome::Unchanged));
        assert_eq!(observation.reason, ReadReason::LengthMismatch);
        assert_eq!(observation.calls, 0);
        assert_eq!(observation.rewind, None);
        assert_eq!(unsafe { memory_tell(source) }, 0);
        unsafe { file.release() };
    }
    let (source, _) = fixture.source(vec![0; ALIGNED_GFX_LENGTH]);
    let source = Box::into_raw(source);
    assert_eq!(
        returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
        source.cast()
    );
    assert_eq!(unsafe { (*source).read_calls + (*source).seek_calls }, 0);
    unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
    fixture.all_freed();
}

#[test]
fn foreign_gfx_is_returned_unchanged_even_after_a_successful_cached_build() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    state
        .patched
        .set(
            gfx::build_options_gfx(ControllerLayout::DualSense)
                .unwrap()
                .0
                .into_boxed_slice(),
        )
        .unwrap();
    let mut bytes = gfx::OFFICIAL_OPTIONS_GFX.to_vec();
    bytes[100] ^= 1;
    let (source, deleted) = fixture.source(bytes.clone());
    let source = Box::into_raw(source);
    assert_eq!(
        returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
        source.cast()
    );
    assert_eq!(unsafe { &(*source).bytes }, &bytes);
    assert_eq!(unsafe { (*source).cursor }, 0);
    assert_eq!(deleted.load(Ordering::Relaxed), 0);
    assert_eq!(state.counters[HASH_REJECT].load(Ordering::Relaxed), 1);
    unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
    fixture.all_freed();
}

#[test]
fn short_reads_zero_progress_and_wrong_lengths_preserve_originals() {
    for scenario in 0..4 {
        let fixture = Fixture::new(0);
        let state = fixture.state(ControllerLayout::DualSense);
        let (mut source, _) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
        match scenario {
            0 => {
                source.chunk = 10;
            } // bounded at 64 calls
            1 => {
                source.chunk = 1024;
                source.stop_after = 1;
            }
            2 => {
                source.bytes.pop();
            }
            _ => {
                source.cursor = 9;
            }
        }
        let original_cursor = source.cursor;
        let source = Box::into_raw(source);
        assert_eq!(
            returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
            source.cast()
        );
        assert_eq!(unsafe { (*source).cursor }, original_cursor);
        assert!(unsafe { (*source).read_calls } <= MAX_READ_CALLS);
        assert_eq!(state.counters[READ_REJECT].load(Ordering::Relaxed), 1);
        assert!(state.patched.get().is_none());
        let reason = [
            ReadReason::ReadLimit,
            ReadReason::ReadStopped,
            ReadReason::LengthMismatch,
            ReadReason::PositionMismatch,
        ][scenario];
        let observation = report(&state, FileKind::SysFile, reason);
        match scenario {
            0 => {
                assert_eq!(observation.report.calls, MAX_READ_CALLS);
                assert_eq!(observation.report.bytes_read, 640);
                assert_eq!(observation.report.last_read, Some(10));
            }
            1 => {
                assert_eq!(observation.report.calls, 2);
                assert_eq!(observation.report.bytes_read, 1024);
                assert_eq!(observation.report.last_read, Some(0));
            }
            2 => {
                assert_eq!(
                    observation.report.length,
                    Some(gfx::OFFICIAL_GFX_LENGTH as i64 - 1)
                );
                assert_eq!(unsafe { (*source).read_calls }, 0);
                assert_eq!(unsafe { (*source).seek_calls }, 0);
            }
            _ => {
                assert_eq!(observation.report.position, Some(9));
                assert_eq!(observation.report.length, None);
                assert_eq!(unsafe { (*source).length_calls }, 0);
                assert_eq!(unsafe { (*source).read_calls }, 0);
                assert_eq!(unsafe { (*source).seek_calls }, 0);
            }
        }
        unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
    }
}

#[test]
fn failed_rewind_releases_consumed_original_and_requests_one_fresh_open() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, deleted) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
    source.fail_seek = true;
    let source = Box::into_raw(source).cast();
    assert!(matches!(
        unsafe { replace_file(&state, source, BASENAME) },
        Replacement::Reopen
    ));
    assert_eq!(deleted.load(Ordering::Relaxed), 1);
    assert_eq!(state.counters[REOPEN].load(Ordering::Relaxed), 1);
    assert!(state.patched.get().is_none());
    let observation = report(&state, FileKind::SysFile, ReadReason::RewindFailed);
    assert_eq!(observation.report.rewind, Some(-1));
    assert_eq!(observation.report.position_after_rewind, None);
}

#[test]
fn invalid_file_is_reported_without_querying_or_reading_it() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, deleted) = fixture.source(Vec::new());
    source.valid = 0;
    let source = Box::into_raw(source);
    for _ in 0..3 {
        assert_eq!(
            returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
            source.cast()
        );
    }
    let observation = report(&state, FileKind::SysFile, ReadReason::Invalid);
    assert_eq!(observation.report.valid, 0);
    assert_eq!(observation.report.position, None);
    assert_eq!(observation.report.length, None);
    assert_eq!(observation.report.calls, 0);
    assert_eq!(
        unsafe {
            (*source).tell_calls
                + (*source).length_calls
                + (*source).seek_calls
                + (*source).read_calls
        },
        0
    );
    assert_eq!(deleted.load(Ordering::Relaxed), 0);
    assert_eq!(fixture.heap.calls.load(Ordering::Relaxed), 0);
    let mut previous = [0; COUNTERS];
    let lines = progress_lines(&state, &mut previous);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("reason=invalid; source=SysFile"));
    assert!(lines[1].contains("path=\"02_040_optionsetting.gfx\""));
    assert!(lines[1].contains("valid=0; position=not-queried; length=not-queried"));
    assert!(lines[1].ends_with("header=not-read"));
    assert!(progress_lines(&state, &mut previous).is_empty());
    unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
}

#[test]
fn alternate_length_is_reported_without_relaxing_the_input_gate() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (source, _) = fixture.source(vec![0; 46401]);
    let source = Box::into_raw(source);
    assert_eq!(
        returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
        source.cast()
    );
    let observation = report(&state, FileKind::SysFile, ReadReason::LengthMismatch);
    assert_eq!(observation.report.length, Some(46401));
    assert_eq!(observation.report.position, Some(0));
    assert_eq!(unsafe { (*source).read_calls + (*source).seek_calls }, 0);
    assert!(
        observation
            .log_line()
            .contains("length=46401; expected=44007")
    );
    assert!(state.patched.get().is_none());
    unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
}

#[test]
fn invalid_read_counts_are_reported_and_rewound_without_trusting_the_buffer() {
    for (read_count, reason) in [
        (-1, ReadReason::ReadStopped),
        (
            gfx::OFFICIAL_GFX_LENGTH as i32 + 1,
            ReadReason::ReadCountOutOfRange,
        ),
    ] {
        let fixture = Fixture::new(0);
        let state = fixture.state(ControllerLayout::DualSense);
        let (mut source, _) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
        source.forced_read = Some(read_count);
        let source = Box::into_raw(source);
        assert_eq!(
            returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
            source.cast()
        );
        let observation = report(&state, FileKind::SysFile, reason);
        assert_eq!(observation.report.calls, 1);
        assert_eq!(observation.report.last_read, Some(read_count));
        assert_eq!(observation.report.bytes_read, 0);
        assert_eq!(observation.report.header_length, 0);
        assert_eq!(observation.report.rewind, Some(0));
        assert_eq!(observation.report.position_after_rewind, Some(0));
        unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
    }
}

#[test]
fn successful_seek_return_with_wrong_position_is_reported_as_reopen() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, deleted) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
    source.seek_without_moving = true;
    assert!(matches!(
        unsafe { replace_file(&state, Box::into_raw(source).cast(), BASENAME) },
        Replacement::Reopen
    ));
    let observation = report(&state, FileKind::SysFile, ReadReason::RewindFailed);
    assert_eq!(observation.report.rewind, Some(0));
    assert_eq!(
        observation.report.position_after_rewind,
        Some(gfx::OFFICIAL_GFX_LENGTH as i64)
    );
    assert_eq!(deleted.load(Ordering::Relaxed), 1);
}

#[test]
fn a_partial_header_contains_only_confirmed_read_bytes() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, _) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
    source.chunk = 4;
    source.stop_after = 1;
    let source = Box::into_raw(source).cast();
    assert_eq!(
        returned(unsafe { replace_file(&state, source, BASENAME) }),
        source
    );
    let observation = report(&state, FileKind::SysFile, ReadReason::ReadStopped);
    assert_eq!(observation.report.header_length, 4);
    assert!(observation.log_line().ends_with("header=4746580B"));
    unsafe { File::inspect(source, &state.api).unwrap().release() };
}

#[test]
fn background_observes_new_reports_even_if_counts_were_already_sampled() {
    assert_eq!(ReadReason::RewindFailed as usize + 1, READ_REASONS);
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, _) = fixture.source(Vec::new());
    source.valid = 0;
    let source = Box::into_raw(source).cast();
    let file = File::inspect(source, &state.api).unwrap();
    let (_, report) = unsafe { read_and_rewind(&file) };
    let mut previous = [0; COUNTERS];
    state.count(READ_REJECT);
    assert_eq!(progress_lines(&state, &mut previous).len(), 1);
    state.record_read(file.kind, BASENAME, report);
    assert_eq!(progress_lines(&state, &mut previous).len(), 2);
    // Repeated observations do not replace the first path or produce another log record.
    state.record_read(file.kind, b"other/path/02_040_optionsetting.gfx", report);
    assert!(progress_lines(&state, &mut previous).is_empty());
    state.record_read(FileKind::MemoryFile, BASENAME, report);
    let lines = progress_lines(&state, &mut previous);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("source=MemoryFile"));
    unsafe { file.release() };
}

#[test]
fn allocation_failures_unwind_owned_allocations_and_keep_original_open() {
    for fail_at in [1, 2] {
        let fixture = Fixture::new(fail_at);
        let state = fixture.state(ControllerLayout::DualSense);
        let (source, deleted) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
        let source = Box::into_raw(source);
        assert_eq!(
            returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
            source.cast()
        );
        assert_eq!(unsafe { (*source).cursor }, 0);
        assert_eq!(deleted.load(Ordering::Relaxed), 0);
        assert_eq!(state.counters[BRIDGE_REJECT].load(Ordering::Relaxed), 1);
        fixture.all_freed();
        unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
    }
}

#[test]
fn unknown_types_and_changed_vtables_are_not_called() {
    let mut fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let (mut source, _) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
    source.vtable = 1;
    let source = Box::into_raw(source);
    assert_eq!(
        returned(unsafe { replace_file(&state, source.cast(), BASENAME) }),
        source.cast()
    );
    assert_eq!(unsafe { (*source).read_calls }, 0);
    unsafe {
        (*source).vtable = fixture.sys_table.as_ptr() as usize;
    }
    let old = fixture.sys_table[10];
    fixture.sys_table[10] = 1;
    assert!(File::inspect(source.cast(), &state.api).is_none());
    fixture.sys_table[10] = old;
    unsafe { File::inspect(source.cast(), &state.api).unwrap().release() };
}

#[test]
fn a_missing_heap_returns_original_and_never_calls_an_allocator() {
    let mut fixture = Fixture::new(0);
    *fixture.heap_slot = 0;
    let state = fixture.state(ControllerLayout::DualSense);
    let (source, _) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
    let source = Box::into_raw(source).cast();
    assert_eq!(
        returned(unsafe { replace_file(&state, source, BASENAME) }),
        source
    );
    assert_eq!(fixture.heap.calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.counters[BRIDGE_REJECT].load(Ordering::Relaxed), 1);
    unsafe { File::inspect(source, &state.api).unwrap().release() };
}

#[test]
fn extra_native_references_delay_destruction_without_freeing_the_gfx_buffer() {
    let layout = ControllerLayout::DualSense;
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    let bytes = gfx::build_options_gfx(ControllerLayout::DualSense)
        .unwrap()
        .0;
    let pointer = unsafe { create_memory_file(&state.api, BASENAME, &bytes) }.unwrap();
    let file = File::inspect(pointer, &state.api).unwrap();
    assert_eq!(
        unsafe { mock_add((pointer as *mut u8).add(8).cast(), 1) },
        1
    );
    unsafe { file.release() };
    assert_eq!(fixture.heap.frees.load(Ordering::Relaxed), 0);
    unsafe { file.release() };
    fixture.all_freed();
    assert_eq!(bytes.len(), expected_options(layout).len());
}

#[test]
fn concurrent_opens_share_one_immutable_movie_but_have_independent_file_cursors() {
    let fixture = Fixture::new(0);
    let state = fixture.state(ControllerLayout::DualSense);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..3 {
                    let (source, deleted) = fixture.source(gfx::OFFICIAL_OPTIONS_GFX.to_vec());
                    let pointer = returned(unsafe {
                        replace_file(&state, Box::into_raw(source).cast(), BASENAME)
                    });
                    assert_eq!(deleted.load(Ordering::Relaxed), 1);
                    assert_eq!(unsafe { memory_tell(pointer) }, 0);
                    let mut buffer = [0; 8];
                    assert_eq!(unsafe { memory_read(pointer, buffer.as_mut_ptr(), 8) }, 8);
                    assert_eq!(&buffer[..], &state.patched.get().unwrap()[..8]);
                    unsafe { File::inspect(pointer, &state.api).unwrap().release() };
                }
            });
        }
    });
    assert_eq!(state.counters[BUILT].load(Ordering::Relaxed), 1);
    assert_eq!(state.counters[RETURNED].load(Ordering::Relaxed), 24);
    fixture.all_freed();
}

#[test]
fn only_the_two_pc_display_movies_are_replacement_targets() {
    for path in [
        "data0:/menu/02_160_keyconfiguration.gfx",
        "menu:/02_160_keyconfiguration.gfx",
        r"data0:\MENU\02_160_KEYCONFIGURATION.GFX",
    ] {
        assert_eq!(target_path(path.as_bytes()), Some(Movie::KeyConfig));
    }
    assert_eq!(
        target_path(b"data0:/menu/win/02_040_optionsetting.gfx"),
        Some(Movie::Options)
    );
    assert_eq!(target_path(b"data0:/menu/02_040_optionsetting.gfx"), None);
    assert_eq!(
        target_path(b"D:/Projects/EldenRingUnpack/02_040_optionsetting.gfx"),
        None
    );
}

#[test]
fn key_configuration_uses_its_own_cache_and_never_opens_a_donor() {
    for layout in [
        ControllerLayout::DualSense,
        ControllerLayout::DualShock4,
        ControllerLayout::XboxSeries,
    ] {
        let fixture = Fixture::new(0);
        let state = fixture.state(layout);
        for _ in 0..3 {
            let (source, released) = fixture.source(gfx::OFFICIAL_KEY_GFX.to_vec());
            let replacement = returned(unsafe {
                replace_movie(
                    &state,
                    Box::into_raw(source).cast(),
                    b"data0:/menu/02_160_keyconfiguration.gfx",
                    Movie::KeyConfig,
                )
            });
            let expected = gfx::patch(Movie::KeyConfig, gfx::OFFICIAL_KEY_GFX, layout).unwrap();
            let mut output = vec![0; expected.len()];
            assert_eq!(
                unsafe { memory_read(replacement, output.as_mut_ptr(), output.len() as i32) },
                output.len() as i32
            );
            assert_eq!(output, expected);
            assert_eq!(released.load(Ordering::Relaxed), 1);
            unsafe { File::inspect(replacement, &state.api).unwrap().release() };
        }
        assert!(state.patched.get().is_none());
        assert!(state.key_patched.get().is_some());
        assert_eq!(state.counters[BUILT].load(Ordering::Relaxed), 1);
        assert_eq!(state.counters[KEY_RETURNED].load(Ordering::Relaxed), 3);
        let mut changed = gfx::OFFICIAL_KEY_GFX.to_vec();
        changed[400] ^= 1;
        let (source, _) = fixture.source(changed);
        let source = Box::into_raw(source).cast();
        assert_eq!(
            returned(unsafe {
                replace_movie(
                    &state,
                    source,
                    b"data0:/menu/02_160_keyconfiguration.gfx",
                    Movie::KeyConfig,
                )
            }),
            source
        );
        unsafe { File::inspect(source, &state.api).unwrap().release() };
        fixture.all_freed();
    }
}

#[test]
fn alignment_exception_is_independently_bounded_for_each_verified_movie() {
    for kind in [Movie::Options, Movie::KeyConfig, Movie::CommonOptions] {
        let exact = kind.length() as i64;
        let aligned = kind.length().next_multiple_of(16) as i64;
        assert!(supported_movie_length(FileKind::MemoryFile, exact, kind));
        assert!(supported_movie_length(FileKind::MemoryFile, aligned, kind));
        assert!(supported_movie_length(FileKind::SysFile, exact, kind));
        assert_eq!(
            supported_movie_length(FileKind::SysFile, aligned, kind),
            exact == aligned
        );
        for wrong in [-1, 0, exact - 1, exact + 1, aligned + 1, aligned + 16] {
            assert!(!supported_movie_length(FileKind::MemoryFile, wrong, kind));
        }
    }
}

#[test]
fn both_movies_work_in_either_order_without_a_secondary_file_opener() {
    for layout in [
        ControllerLayout::DualSense,
        ControllerLayout::DualShock4,
        ControllerLayout::XboxSeries,
    ] {
        for order in [
            [Movie::Options, Movie::KeyConfig],
            [Movie::KeyConfig, Movie::Options],
        ] {
            let fixture = Fixture::new(0);
            let state = fixture.state(layout);
            assert_eq!(state.api.opener, 0); // Cannot call any donor opener, even accidentally.
            for _ in 0..2 {
                for kind in order {
                    let bytes = if kind == Movie::Options {
                        gfx::OFFICIAL_OPTIONS_GFX
                    } else {
                        gfx::OFFICIAL_KEY_GFX
                    };
                    let (source, released) = fixture.source(bytes.to_vec());
                    let output = returned(unsafe {
                        replace_movie(&state, Box::into_raw(source).cast(), b"test.gfx", kind)
                    });
                    let expected = gfx::patch(kind, bytes, layout).unwrap();
                    let mut got = vec![0; expected.len()];
                    assert_eq!(
                        unsafe { memory_read(output, got.as_mut_ptr(), got.len() as i32) },
                        got.len() as i32
                    );
                    assert_eq!(got, expected);
                    assert_eq!(released.load(Ordering::Relaxed), 1);
                    unsafe { File::inspect(output, &state.api).unwrap().release() };
                }
            }
            assert!(state.patched.get().is_some());
            assert!(state.key_patched.get().is_some());
            assert_eq!(state.counters[BUILT].load(Ordering::Relaxed), 2);
            assert_eq!(state.counters[RETURNED].load(Ordering::Relaxed), 4);
            fixture.all_freed();
        }
    }
}
