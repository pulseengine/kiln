//! The process-global safety monitor under concurrent use.
//!
//! Unit tests inside kiln-foundation see a per-thread monitor (cfg(test)), so
//! only an integration test (its own process, the library built without
//! cfg(test)) reaches the global monitor and its lock.

#![cfg(feature = "std")]

use std::sync::{Arc, Barrier};

use kiln_foundation::safety_monitor::with_safety_monitor;

#[test]
fn concurrent_updates_through_the_global_monitor_are_not_lost() {
    const THREADS: usize = 8;
    const ITERATIONS: u64 = 50_000;

    let before = with_safety_monitor(|m| m.get_safety_report().total_allocations);
    let barrier = Arc::new(Barrier::new(THREADS));

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..ITERATIONS {
                    with_safety_monitor(|m| m.record_allocation(1));
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("worker thread must not panic");
    }

    let after = with_safety_monitor(|m| m.get_safety_report().total_allocations);
    assert_eq!(
        after - before,
        THREADS as u64 * ITERATIONS,
        "every record_allocation made under the monitor lock must be counted"
    );
}
