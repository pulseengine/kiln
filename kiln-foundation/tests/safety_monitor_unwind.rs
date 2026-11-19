//! The process-global safety monitor stays usable after a closure panics.
//!
//! Integration test (own process) so the library is built without cfg(test)
//! and with_safety_monitor uses the global monitor and its lock.

#![cfg(feature = "std")]

use std::{sync::mpsc, time::Duration};

use kiln_foundation::safety_monitor::with_safety_monitor;

#[test]
fn a_panic_inside_the_monitor_releases_it() {
    let unwound = std::panic::catch_unwind(|| {
        with_safety_monitor(|_| panic!("closure panics while holding the monitor"))
    });
    assert!(unwound.is_err(), "the closure's panic must propagate");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        with_safety_monitor(|m| m.record_allocation(1));
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(Duration::from_secs(10)).is_ok(),
        "the monitor must be acquirable again after a panic inside it"
    );
}
