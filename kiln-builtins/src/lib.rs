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

use core::sync::atomic::{AtomicU32, AtomicPtr, Ordering};

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
#[unsafe(no_mangle)]
pub extern "C" fn __meld_dispatch_import(import_index: u32) -> u32 {
    let p = PLATFORM.load(Ordering::Acquire);
    if p.is_null() {
        // No platform — stub mode, return 0
        return 0;
    }
    let _platform = unsafe { &*p };

    // Phase 2: use import table to map index → WASI function, dispatch.
    // For now, return 0 (stub).
    let _ = import_index;
    0
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
    unsafe { &raw const __wasm_memory_start as *mut u8 }
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
pub extern "C" fn cabi_realloc(
    old_ptr: u32,
    old_size: u32,
    align: u32,
    new_size: u32,
) -> u32 {
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

        match BUMP_PTR.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
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
/// Used internally by WASI dispatch handlers to access platform callbacks.
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

    fn reset() {
        BUMP_PTR.store(0, Ordering::SeqCst);
        PLATFORM.store(core::ptr::null_mut(), Ordering::SeqCst);
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
        unsafe extern "C" fn test_write(_fd: u32, _data: *const u8, _len: u32) -> i32 { 0 }
        unsafe extern "C" fn test_clock() -> u64 { 0 }
        unsafe extern "C" fn test_random(_buf: *mut u8, _len: u32) -> i32 { 0 }
        unsafe extern "C" fn test_exit(_code: i32) -> ! { loop {} }

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
        unsafe extern "C" fn nop_clock() -> u64 { 42 }
        unsafe extern "C" fn nop_random(_buf: *mut u8, _len: u32) -> i32 { 0 }
        unsafe extern "C" fn nop_exit(_code: i32) -> ! { loop {} }

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
        unsafe extern "C" fn mock_clock() -> u64 { 123_456_789 }
        unsafe extern "C" fn nop_write(_fd: u32, _data: *const u8, _len: u32) -> i32 { 0 }
        unsafe extern "C" fn nop_random(_buf: *mut u8, _len: u32) -> i32 { 0 }
        unsafe extern "C" fn nop_exit(_code: i32) -> ! { loop {} }

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
