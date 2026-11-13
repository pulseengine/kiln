//! # kiln-builtins
//!
//! Runtime builtins for synth-compiled WebAssembly on embedded targets.
//!
//! This crate provides the C ABI surface that synth-compiled ELF binaries
//! call into for host intrinsics, memory management, and WASI dispatch.
//! It is `no_std` compatible and targets bare-metal / RTOS environments
//! (Cortex-M via gale/Zephyr).
//!
//! ## Architecture
//!
//! ```text
//! synth-compiled code ──→ kiln-builtins ──→ Platform callbacks ──→ gale/Zephyr
//! ```
//!
//! Platform operations (I/O, clocks, random) are provided via a [`Platform`]
//! struct of C-compatible function pointers. This supports two paths:
//!
//! 1. **C shim** (today): callbacks call Zephyr C APIs directly via FFI
//! 2. **Verified Rust** (gradual): callbacks route through gale's formally
//!    verified kernel primitives, then down to Zephyr C for hardware access
//!
//! Both paths coexist — migrate one callback at a time as gale modules
//! become available, without changing kiln-builtins itself.
//!
//! ## Functions
//!
//! - [`__meld_dispatch_import`] — Routes import calls to WASI/host handlers
//! - [`__meld_get_memory_base`] — Returns linear memory base address
//! - [`cabi_realloc`] — Canonical ABI memory allocator
//! - [`__kiln_init`] — Initialize platform + heap before WASM execution

#![cfg_attr(not(test), no_std)]

use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // If platform is initialized, try to write panic info to stderr
    let p = PLATFORM.load(Ordering::Acquire);
    if !p.is_null() {
        let platform = unsafe { &*p };
        let msg = b"panic in kiln-builtins\n";
        unsafe { (platform.write)(2, msg.as_ptr(), msg.len() as u32) };
    }
    loop {}
}

// ============================================================================
// Platform abstraction
// ============================================================================

/// Platform callbacks for hardware/OS operations.
///
/// C-compatible function pointers — no generics, no vtable, no `dyn`.
/// Set once at firmware init via [`__kiln_init`], then immutable.
///
/// ## Migration pattern
///
/// Each callback starts as a thin C FFI shim to Zephyr:
/// ```c
/// int32_t my_write(uint32_t fd, const uint8_t* data, uint32_t len) {
///     return uart_fifo_fill(uart_dev, data, (int)len);
/// }
/// ```
///
/// When gale provides a verified Rust implementation (e.g., `gale::pipe`),
/// replace that one callback with a Rust function that routes through gale's
/// verified state tracking, then calls the Zephyr C API for hardware:
/// ```rust,ignore
/// extern "C" fn gale_write(fd: u32, data: *const u8, len: u32) -> i32 {
///     // gale::Pipe::write_check — verified bounds (ASIL-D)
///     // then: zephyr_sys::uart_fifo_fill — hardware I/O
/// }
/// ```
///
/// The rest of the Platform stays as C shims. Migrate one at a time.
#[repr(C)]
pub struct Platform {
    /// Write bytes to an output stream.
    ///
    /// - `fd`: 1 = stdout, 2 = stderr, other = application-defined
    /// - `data`: pointer to byte buffer
    /// - `len`: number of bytes to write
    /// - Returns: bytes written (>= 0) or negative error code
    pub write: unsafe extern "C" fn(fd: u32, data: *const u8, len: u32) -> i32,

    /// Read monotonic clock in nanoseconds since boot.
    pub clock_ns: unsafe extern "C" fn() -> u64,

    /// Fill buffer with random bytes.
    ///
    /// - Returns: 0 on success, negative error code on failure
    pub random: unsafe extern "C" fn(buf: *mut u8, len: u32) -> i32,

    /// Terminate execution with the given exit code.
    ///
    /// For RTOS: abort the current thread/task.
    /// For bare-metal: enter infinite loop.
    /// Must not return.
    pub exit: unsafe extern "C" fn(code: i32) -> !,
}

/// Global platform pointer — set by [`__kiln_init`], read by dispatch.
static PLATFORM: AtomicPtr<Platform> = AtomicPtr::new(core::ptr::null_mut());

/// Bump allocator pointer for cabi_realloc.
static BUMP_PTR: AtomicU32 = AtomicU32::new(0);

// ============================================================================
// Initialization
// ============================================================================

/// Initialize kiln-builtins with platform callbacks and heap start.
///
/// Must be called before any WASM execution. Typically called from the
/// firmware's `main()` or RTOS task init.
///
/// # Safety
///
/// `platform` must point to a valid `Platform` struct that outlives all
/// WASM execution (typically `&'static Platform`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __kiln_init(platform: *mut Platform, heap_start: u32) {
    PLATFORM.store(platform, Ordering::Release);
    BUMP_PTR.store(heap_start, Ordering::Release);
}

/// Initialize only the bump allocator (for backward compatibility).
#[unsafe(no_mangle)]
pub extern "C" fn __kiln_builtins_init(heap_start: u32) {
    BUMP_PTR.store(heap_start, Ordering::Release);
}

// ============================================================================
// Import dispatch
// ============================================================================

/// Called by synth-compiled code to dispatch an import call.
///
/// Synth generates: `MOV R0, #import_index; BL __meld_dispatch_import`
///
/// The import index maps to a WASI or host function via the import table
/// generated by meld (`--emit-import-map`).
///
/// ## Dispatch protocol
///
/// Arguments and return values are passed through WASM linear memory.
/// The caller (synth-compiled code) sets up arguments in memory before
/// the call. The dispatcher reads them, performs the operation via
/// Platform callbacks, and writes results back to memory.
/// Import dispatch kind — determined from the import table generated by meld.
///
/// The import table is a linker-provided array mapping import indices to
/// their operation kind. Synth embeds this in `.meld_import_table`.
#[repr(u8)]
pub enum ImportKind {
    /// P3 async built-in: [task-return]N
    TaskReturn = 1,
    /// P3 async built-in: [context-get-N]
    ContextGet = 2,
    /// P3 async built-in: [context-set-N]
    ContextSet = 3,
    /// P3 async built-in: [waitable-set-new]
    WaitableSetNew = 4,
    /// P3 async built-in: [waitable-set-poll]
    WaitableSetPoll = 5,
    /// P3 async built-in: [waitable-set-drop]
    WaitableSetDrop = 6,
    /// P3 async built-in: [waitable-join]
    WaitableJoin = 7,
    /// P3 async built-in: [task-cancel]
    TaskCancel = 8,
    /// WASI function call
    Wasi = 16,
    /// Unknown/stub
    Unknown = 0,
}

// ── P3 Async Task Protocol State ──────────────────────────────────────
// Synchronous implementation for single-threaded interpreter and
// embedded targets. Matches the semantics in kiln-runtime's
// try_dispatch_p3_builtin.

/// Task-local context slot (one per component instance, slot 0).
static P3_CONTEXT_SLOT: AtomicU32 = AtomicU32::new(0);

/// Last task-return value (captured by [task-return]N).
/// For embedded, we store a single i32 result. Complex return types
/// use memory pointers.
static P3_TASK_RETURN_VALUE: AtomicU32 = AtomicU32::new(0);

/// Monotonic waitable set handle counter.
static P3_NEXT_WAITABLE_SET: AtomicU32 = AtomicU32::new(1);

/// Called by synth-compiled code to dispatch an import call.
///
/// Synth generates: `MOV R0, #import_index; BL __meld_dispatch_import`
///
/// The import index maps to a WASI or host function via the import table
/// generated by meld (`--emit-import-map`).
///
/// ## Dispatch protocol
///
/// Arguments and return values are passed through WASM linear memory.
/// The caller (synth-compiled code) sets up arguments in memory before
/// the call. The dispatcher reads them, performs the operation via
/// Platform callbacks, and writes results back to memory.
///
/// ## P3 Async Built-in Dispatch
///
/// For P3 async builtins, the import table entry identifies the operation
/// kind. The dispatcher handles them synchronously:
/// - `[task-return]N`: captures result value, no-op for sync execution
/// - `[context-get-0]`: returns stored context slot value
/// - `[context-set-0]`: stores context slot value
/// - `[waitable-set-new]`: returns incrementing handle
/// - `[waitable-set-poll/drop/join]`, `[task-cancel]`: refuse (see
///   [`__p3_waitable_set_poll`] and friends), they are not no-ops (SR-83)
#[unsafe(no_mangle)]
pub extern "C" fn __meld_dispatch_import(import_index: u32) -> u32 {
    let p = PLATFORM.load(Ordering::Acquire);
    if p.is_null() {
        // No platform — stub mode, return 0
        return 0;
    }
    let _platform = unsafe { &*p };

    // Phase 2: use import table to map index → operation, dispatch.
    //
    // The import table is not yet wired up (needs linker symbol from meld).
    // For now, return 0 as a safe default.
    //
    // When wired up, the dispatch will be:
    //   let kind = import_table[import_index].kind;
    //   match kind {
    //       ImportKind::ContextGet => p3_context_get(),
    //       ImportKind::ContextSet => p3_context_set(arg0),
    //       ImportKind::TaskReturn => p3_task_return(arg0),
    //       ImportKind::WaitableSetNew => p3_waitable_set_new(),
    //       ImportKind::WaitableSetPoll => __p3_waitable_set_poll(..), // refuses
    //       ImportKind::WaitableSetDrop => __p3_waitable_set_drop(..), // refuses
    //       ImportKind::WaitableJoin => __p3_waitable_join(..),        // refuses
    //       ImportKind::TaskCancel => __p3_task_cancel(),              // refuses
    //       ImportKind::Wasi => dispatch_wasi(import_index, platform),
    //       _ => 0,
    //   }
    let _ = import_index;
    0
}

// ── P3 Built-in Implementations ───────────────────────────────────────
// These are called by __meld_dispatch_import once the import table is
// wired up. They can also be called directly for testing.

/// P3 [context-get-0]: returns the stored task-local context value.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_context_get() -> u32 {
    P3_CONTEXT_SLOT.load(Ordering::Acquire)
}

/// P3 [context-set-0]: stores a task-local context value.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_context_set(value: u32) {
    P3_CONTEXT_SLOT.store(value, Ordering::Release);
}

/// P3 [task-return]N: captures the task's return value.
/// For synchronous execution, this just stores the value.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_task_return(value: u32) {
    P3_TASK_RETURN_VALUE.store(value, Ordering::Release);
}

/// P3 [waitable-set-new]: allocates a new waitable set handle.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_waitable_set_new() -> u32 {
    P3_NEXT_WAITABLE_SET.fetch_add(1, Ordering::AcqRel)
}

// ── Unbacked P3 built-ins (SR-83) ─────────────────────────────────────
// Nothing backs these until kiln-async does (SR-84). Returning 0 would claim a
// poll found no event, a join happened or a task was cancelled when none did,
// so each refuses the way this crate ends execution it cannot continue: it
// writes a message naming the built-in to stderr through the platform, then
// calls the platform's `exit`, which must not return.

/// Exit code passed to the platform when an unbacked P3 built-in is called.
pub const P3_NOT_IMPLEMENTED_EXIT: i32 = 70;

/// The refusal message for an unbacked P3 built-in, or `None` for a kind this
/// crate serves.
fn p3_refusal_message(kind: &ImportKind) -> Option<&'static [u8]> {
    match kind {
        ImportKind::WaitableSetPoll => {
            Some(b"kiln-builtins: [waitable-set-poll] is not implemented\n")
        },
        ImportKind::WaitableSetDrop => {
            Some(b"kiln-builtins: [waitable-set-drop] is not implemented\n")
        },
        ImportKind::WaitableJoin => Some(b"kiln-builtins: [waitable-join] is not implemented\n"),
        ImportKind::TaskCancel => Some(b"kiln-builtins: [task-cancel] is not implemented\n"),
        ImportKind::TaskReturn
        | ImportKind::ContextGet
        | ImportKind::ContextSet
        | ImportKind::WaitableSetNew
        | ImportKind::Wasi
        | ImportKind::Unknown => None,
    }
}

/// Writes `message` to stderr through the platform. Returns whether a
/// platform was registered to receive it.
fn p3_report_refusal(message: &[u8]) -> bool {
    let p = PLATFORM.load(Ordering::Acquire);
    if p.is_null() {
        return false;
    }
    let platform = unsafe { &*p };
    unsafe { (platform.write)(2, message.as_ptr(), message.len() as u32) };
    true
}

/// Reports the refusal and ends execution. With no platform registered there
/// is nothing to report through, so it halts like the panic handler does.
fn p3_refuse(kind: ImportKind) -> ! {
    if let Some(message) = p3_refusal_message(&kind) {
        if p3_report_refusal(message) {
            let p = PLATFORM.load(Ordering::Acquire);
            let platform = unsafe { &*p };
            unsafe { (platform.exit)(P3_NOT_IMPLEMENTED_EXIT) }
        }
    }
    loop {
        core::hint::spin_loop();
    }
}

/// P3 `[waitable-set-poll]`: refuses; no async runtime backs it yet.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_waitable_set_poll(_set: u32, _result_ptr: u32) -> u32 {
    p3_refuse(ImportKind::WaitableSetPoll)
}

/// P3 `[waitable-set-drop]`: refuses; no async runtime backs it yet.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_waitable_set_drop(_set: u32) {
    p3_refuse(ImportKind::WaitableSetDrop)
}

/// P3 `[waitable-join]`: refuses; no async runtime backs it yet.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_waitable_join(_waitable: u32, _set: u32) {
    p3_refuse(ImportKind::WaitableJoin)
}

/// P3 `[task-cancel]`: refuses; no async runtime backs it yet.
#[unsafe(no_mangle)]
pub extern "C" fn __p3_task_cancel() {
    p3_refuse(ImportKind::TaskCancel)
}

// ============================================================================
// Memory intrinsics
// ============================================================================

/// Returns the base address of WASM linear memory.
///
/// The linker provides `__wasm_memory_start` as the base of the
/// linear memory section in the ELF binary.
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn __meld_get_memory_base() -> *mut u8 {
    unsafe extern "C" {
        static __wasm_memory_start: u8;
    }
    &raw const __wasm_memory_start as *mut u8
}

/// Canonical ABI allocator for linear memory.
///
/// Called by adapter code (generated by meld) to allocate space for
/// strings, lists, records, and other compound types. Uses a simple
/// bump allocator within linear memory bounds.
///
/// # Parameters
/// - `old_ptr`: Previous allocation pointer (0 for new allocation)
/// - `old_size`: Size of previous allocation (0 for new allocation)
/// - `align`: Required alignment (must be power of 2)
/// - `new_size`: Requested allocation size in bytes
///
/// # Returns
/// Offset from memory base of the allocated region, or 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn cabi_realloc(old_ptr: u32, old_size: u32, align: u32, new_size: u32) -> u32 {
    let _ = (old_ptr, old_size);

    if new_size == 0 {
        return 0;
    }

    // Align must be power of 2
    let align = if align == 0 { 1 } else { align };

    loop {
        let current = BUMP_PTR.load(Ordering::Acquire);
        if current == 0 {
            // Not initialized
            return 0;
        }

        // Align up: (current + align - 1) & !(align - 1)
        let aligned = (current.wrapping_add(align - 1)) & !(align - 1);
        let next = match aligned.checked_add(new_size) {
            Some(n) => n,
            None => return 0, // Overflow — allocation would exceed u32 range
        };

        match BUMP_PTR.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return aligned,
            Err(_) => continue, // Retry on contention
        }
    }
}

// ============================================================================
// Platform access (for use by future WASI handlers)
// ============================================================================

/// Get the current platform, if initialized.
///
/// Test-only accessor for the registered platform callbacks.
#[cfg(test)]
#[inline]
fn platform() -> Option<&'static Platform> {
    let p = PLATFORM.load(Ordering::Acquire);
    if p.is_null() { None } else { Some(unsafe { &*p }) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // The builtins keep process-global state (bump pointer, platform, P3
    // slots), so tests that reset and then use it must not interleave. The
    // first `reset()` on a test's thread takes this lock and keeps the guard
    // in a thread-local until that thread (the test) ends.
    static GLOBAL_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    std::thread_local! {
        static HELD: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> =
            const { std::cell::RefCell::new(None) };
    }

    fn reset() {
        HELD.with(|held| {
            let mut held = held.borrow_mut();
            if held.is_none() {
                // A poisoned lock only means another test panicked; the state
                // is reset right below, so the guard is still sound to take.
                *held = Some(
                    GLOBAL_STATE_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                );
            }
        });
        BUMP_PTR.store(0, Ordering::SeqCst);
        PLATFORM.store(core::ptr::null_mut(), Ordering::SeqCst);
        P3_CONTEXT_SLOT.store(0, Ordering::SeqCst);
        P3_TASK_RETURN_VALUE.store(0, Ordering::SeqCst);
        P3_NEXT_WAITABLE_SET.store(1, Ordering::SeqCst);
    }

    // -- P3 async builtin tests --

    #[test]
    fn test_p3_context_get_set() {
        reset();
        assert_eq!(__p3_context_get(), 0);
        __p3_context_set(42);
        assert_eq!(__p3_context_get(), 42);
        __p3_context_set(0);
        assert_eq!(__p3_context_get(), 0);
    }

    #[test]
    fn test_p3_task_return() {
        reset();
        assert_eq!(P3_TASK_RETURN_VALUE.load(Ordering::SeqCst), 0);
        __p3_task_return(55); // fibonacci(10)
        assert_eq!(P3_TASK_RETURN_VALUE.load(Ordering::SeqCst), 55);
    }

    #[test]
    fn test_p3_waitable_set_new() {
        reset();
        let h1 = __p3_waitable_set_new();
        let h2 = __p3_waitable_set_new();
        assert_eq!(h1, 1);
        assert_eq!(h2, 2);
        assert_ne!(h1, h2);
    }

    // -- unbacked P3 built-ins (SR-83) --

    const UNBACKED: [(&str, &str); 4] = [
        ("poll", "[waitable-set-poll]"),
        ("drop", "[waitable-set-drop]"),
        ("join", "[waitable-join]"),
        ("cancel", "[task-cancel]"),
    ];

    // rivet: verifies SR-83
    #[test]
    fn test_unbacked_kinds_have_a_message_naming_them() {
        let kinds = [
            ImportKind::WaitableSetPoll,
            ImportKind::WaitableSetDrop,
            ImportKind::WaitableJoin,
            ImportKind::TaskCancel,
        ];
        for (kind, (_, name)) in kinds.iter().zip(UNBACKED) {
            let message = p3_refusal_message(kind).expect("unbacked kind must refuse");
            let message = core::str::from_utf8(message).unwrap();
            assert!(message.contains(name), "{message:?} must name {name}");
        }
    }

    // rivet: verifies SR-83
    #[test]
    fn test_served_kinds_do_not_refuse() {
        for kind in [
            ImportKind::TaskReturn,
            ImportKind::ContextGet,
            ImportKind::ContextSet,
            ImportKind::WaitableSetNew,
        ] {
            assert!(p3_refusal_message(&kind).is_none());
        }
    }

    /// Child half of `test_unbacked_builtins_refuse_and_exit`: calls the
    /// built-in named by `KILN_P3_CHILD` against a platform whose `exit`
    /// ends the process. Does nothing when run as an ordinary test.
    #[test]
    fn test_p3_refusal_child() {
        let Ok(which) = std::env::var("KILN_P3_CHILD") else {
            return;
        };
        unsafe extern "C" fn write(_fd: u32, data: *const u8, len: u32) -> i32 {
            use std::io::Write;
            let bytes = unsafe { core::slice::from_raw_parts(data, len as usize) };
            let _ = std::io::stderr().write_all(bytes);
            len as i32
        }
        unsafe extern "C" fn clock() -> u64 {
            0
        }
        unsafe extern "C" fn random(_buf: *mut u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn exit(code: i32) -> ! {
            std::process::exit(code)
        }
        reset();
        let platform = Box::leak(Box::new(Platform {
            write,
            clock_ns: clock,
            random,
            exit,
        }));
        unsafe { __kiln_init(platform, 0x1000) };
        match which.as_str() {
            "poll" => {
                __p3_waitable_set_poll(1, 0);
            },
            "drop" => __p3_waitable_set_drop(1),
            "join" => __p3_waitable_join(1, 2),
            _ => __p3_task_cancel(),
        }
        // Reaching here means the built-in returned.
        std::process::exit(0);
    }

    // rivet: verifies SR-83
    #[test]
    fn test_unbacked_builtins_refuse_and_exit() {
        let exe = std::env::current_exe().unwrap();
        for (which, name) in UNBACKED {
            let out = std::process::Command::new(&exe)
                .args(["--exact", "tests::test_p3_refusal_child", "--nocapture"])
                .env("KILN_P3_CHILD", which)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert_eq!(
                out.status.code(),
                Some(P3_NOT_IMPLEMENTED_EXIT),
                "{name} must exit through the platform\n{stderr}"
            );
            assert!(stderr.contains(name), "{name} must be named\n{stderr}");
        }
    }

    // -- cabi_realloc tests --

    #[test]
    fn test_cabi_realloc_uninitialized_returns_zero() {
        reset();
        assert_eq!(cabi_realloc(0, 0, 1, 16), 0);
    }

    #[test]
    fn test_cabi_realloc_zero_size_returns_zero() {
        reset();
        __kiln_builtins_init(0x1000);
        assert_eq!(cabi_realloc(0, 0, 1, 0), 0);
    }

    #[test]
    fn test_cabi_realloc_basic_allocation() {
        reset();
        __kiln_builtins_init(0x1000);
        let ptr = cabi_realloc(0, 0, 1, 16);
        assert_eq!(ptr, 0x1000);
        let ptr2 = cabi_realloc(0, 0, 1, 8);
        assert_eq!(ptr2, 0x1010);
    }

    #[test]
    fn test_cabi_realloc_alignment() {
        reset();
        __kiln_builtins_init(0x1001); // Misaligned start
        let ptr = cabi_realloc(0, 0, 8, 4);
        assert_eq!(ptr, 0x1008); // Aligned up to 8
        let ptr2 = cabi_realloc(0, 0, 4, 4);
        assert_eq!(ptr2, 0x100C); // 0x1008 + 4 = 0x100C, already 4-aligned
    }

    #[test]
    fn test_cabi_realloc_overflow_returns_zero() {
        reset();
        __kiln_builtins_init(u32::MAX - 10);
        assert_eq!(cabi_realloc(0, 0, 1, 100), 0);
    }

    // -- dispatch tests --

    #[test]
    fn test_dispatch_import_no_platform() {
        reset();
        assert_eq!(__meld_dispatch_import(0), 0);
        assert_eq!(__meld_dispatch_import(42), 0);
        assert_eq!(__meld_dispatch_import(u32::MAX), 0);
    }

    #[test]
    fn test_dispatch_import_with_platform() {
        reset();
        unsafe extern "C" fn test_write(_fd: u32, _data: *const u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn test_clock() -> u64 {
            0
        }
        unsafe extern "C" fn test_random(_buf: *mut u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn test_exit(_code: i32) -> ! {
            loop {}
        }

        let mut p = Platform {
            write: test_write,
            clock_ns: test_clock,
            random: test_random,
            exit: test_exit,
        };
        unsafe { __kiln_init(&mut p, 0x2000) };
        // Still returns 0 in Phase 1 stub mode
        assert_eq!(__meld_dispatch_import(0), 0);
        // But platform is now accessible
        assert!(platform().is_some());
        // And heap is initialized
        assert_eq!(cabi_realloc(0, 0, 1, 8), 0x2000);
    }

    // -- platform tests --

    #[test]
    fn test_platform_write_callback() {
        use core::sync::atomic::AtomicI32;
        static WRITE_CALLED: AtomicI32 = AtomicI32::new(0);

        unsafe extern "C" fn mock_write(_fd: u32, _data: *const u8, len: u32) -> i32 {
            WRITE_CALLED.store(len as i32, Ordering::SeqCst);
            len as i32
        }
        unsafe extern "C" fn nop_clock() -> u64 {
            42
        }
        unsafe extern "C" fn nop_random(_buf: *mut u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn nop_exit(_code: i32) -> ! {
            loop {}
        }

        reset();
        WRITE_CALLED.store(0, Ordering::SeqCst);

        let mut p = Platform {
            write: mock_write,
            clock_ns: nop_clock,
            random: nop_random,
            exit: nop_exit,
        };
        unsafe { __kiln_init(&mut p, 0x1000) };

        let plat = platform().unwrap();
        let msg = b"hello";
        let ret = unsafe { (plat.write)(1, msg.as_ptr(), msg.len() as u32) };
        assert_eq!(ret, 5);
        assert_eq!(WRITE_CALLED.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_platform_clock_callback() {
        unsafe extern "C" fn mock_clock() -> u64 {
            123_456_789
        }
        unsafe extern "C" fn nop_write(_fd: u32, _data: *const u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn nop_random(_buf: *mut u8, _len: u32) -> i32 {
            0
        }
        unsafe extern "C" fn nop_exit(_code: i32) -> ! {
            loop {}
        }

        reset();
        let mut p = Platform {
            write: nop_write,
            clock_ns: mock_clock,
            random: nop_random,
            exit: nop_exit,
        };
        unsafe { __kiln_init(&mut p, 0x1000) };

        let plat = platform().unwrap();
        assert_eq!(unsafe { (plat.clock_ns)() }, 123_456_789);
    }
}

// ============================================================================
// Kani proofs
// ============================================================================

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn cabi_realloc_never_overflows() {
        let heap_start: u32 = kani::any();
        let align: u32 = kani::any();
        let new_size: u32 = kani::any();

        kani::assume(heap_start > 0);
        kani::assume(align <= 4096);
        kani::assume(align == 0 || (align & (align - 1)) == 0);

        BUMP_PTR.store(heap_start, Ordering::SeqCst);
        let result = cabi_realloc(0, 0, align, new_size);

        if result != 0 {
            assert!(result >= heap_start);
            let effective_align = if align == 0 { 1 } else { align };
            assert!(result % effective_align == 0);
            assert!(result as u64 + new_size as u64 <= u32::MAX as u64 + 1);
        }
    }

    #[kani::proof]
    fn cabi_realloc_alignment_correct() {
        let heap_start: u32 = kani::any();
        kani::assume(heap_start > 0 && heap_start < 0x1000_0000);

        let align_shift: u32 = kani::any();
        kani::assume(align_shift <= 8);
        let align = 1u32 << align_shift;

        let new_size: u32 = kani::any();
        kani::assume(new_size > 0 && new_size <= 1024);

        BUMP_PTR.store(heap_start, Ordering::SeqCst);
        let result = cabi_realloc(0, 0, align, new_size);

        if result != 0 {
            assert!(result % align == 0, "Result not properly aligned");
        }
    }

    #[kani::proof]
    fn dispatch_import_returns_zero_without_platform() {
        PLATFORM.store(core::ptr::null_mut(), Ordering::SeqCst);
        let idx: u32 = kani::any();
        assert_eq!(__meld_dispatch_import(idx), 0);
    }
}
