//! # kilnd binary entry point
//!
//! Thin wrapper around the [`kilnd`] library. A binary — not a library — must
//! own the process-level items: the no_std `#[global_allocator]` and
//! `#[panic_handler]`, and the `fn main` entry points. The actual daemon logic
//! lives in the library ([`kilnd::run`]) so it can be exercised by integration
//! tests (issue #377).

#![deny(unsafe_code)]

// Simple global allocator for no_std mode — a static buffer. Must be defined by
// the binary, not the library.
#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
use linked_list_allocator::LockedHeap;

#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
static mut HEAP: [u8; 64 * 1024] = [0; 64 * 1024]; // 64KB heap

/// std entry point. Runs the daemon on a thread with a larger stack — module
/// parsing and type validation can recurse deeply, needing more than the
/// platform default. 8MB works on Linux/macOS/VxWorks; override with the
/// `KILN_STACK_SIZE` env var (in bytes).
#[cfg(feature = "std")]
fn main() -> kiln_error::Result<()> {
    let stack_size: usize = std::env::var("KILN_STACK_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8 * 1024 * 1024); // 8MB default

    std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(kilnd::run)
        .expect("Failed to spawn main thread")
        .join()
        .expect("Main thread panicked")
}

/// no_std entry point (embedded / bare-metal): initialize the allocator, then
/// run a minimal embedded execution flow against demo module data.
#[cfg(not(feature = "std"))]
fn main() {
    use kilnd::{KilndConfig, KilndEngine};

    // Initialize the allocator if available.
    #[cfg(feature = "enable-panic-handler")]
    {
        #[allow(unsafe_code)] // Required for allocator initialization
        unsafe {
            ALLOCATOR.lock().init(HEAP.as_mut_ptr(), HEAP.len());
        }
    }

    // In no_std mode, module data typically comes from embedded storage. For
    // this demo we use a minimal WASM header.
    const DEMO_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6D, // WASM magic
        0x01, 0x00, 0x00, 0x00, // Version 1
    ];

    let mut config = KilndConfig::default();
    config.module_data = Some(DEMO_MODULE);
    config.function_name = Some("start");
    config.max_fuel = 1000; // Conservative for embedded
    config.max_memory = 4096; // 4KB for embedded

    let mut engine = match KilndEngine::new(config) {
        Ok(engine) => engine,
        // Engine creation failed — enter an error loop (no console in no_std).
        Err(_) => loop {
            core::hint::spin_loop();
        },
    };

    if engine.execute_module().is_err() {
        // Execution failed — enter an error loop (panic-like behavior).
        loop {
            core::hint::spin_loop();
        }
    }
}

/// Panic handler for no_std builds. Real embedded systems would write to status
/// registers, trigger a reset, or flash an error LED.
#[cfg(all(not(feature = "std"), not(test), feature = "enable-panic-handler"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
