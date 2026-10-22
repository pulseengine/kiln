//! Concurrent first-use of the global memory system.
//!
//! This is an integration test (own process) so the one-time initialization
//! has not already happened: `initialize()` must not report success to a
//! second caller before the global capability context exists.

#![cfg(feature = "std")]

use std::sync::{
    Arc,
    Barrier,
};

use kiln_foundation::memory_init::{
    get_global_capability_context,
    MemoryInitializer,
};

#[test]
fn concurrent_first_initialization_publishes_context_before_success() {
    const THREADS: usize = 16;
    let barrier = Arc::new(Barrier::new(THREADS));

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                MemoryInitializer::initialize()?;
                // Once initialize() has returned Ok, the context must be
                // readable by this thread.
                get_global_capability_context().map(|_| ())
            })
        })
        .collect();

    for handle in handles {
        handle
            .join()
            .expect("thread panicked")
            .expect("initialize() returned Ok but the global context was not available");
    }
}
