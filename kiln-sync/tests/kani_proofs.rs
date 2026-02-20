//! Integration tests using Kani verification for wrt-sync.
//!
//! This file contains integration tests that verify the behavior of the
//! synchronization primitives using the Kani verification framework. These
//! tests only run when the 'kani' feature is enabled and require the Kani
//! verifier to be installed.

#![cfg(feature = "kani")]

use kani;
use wrt_sync::*;

// --- WrtMutex Harnesses ---

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))] // Allow a few loop iterations for spinlock
fn verify_mutex_new_lock_unlock() {
    let m = WrtMutex::new(10);
    {
        let mut guard = m.lock();
        *guard += 5;
        assert_eq!(*guard, 15);
        // Test Debug impl while locked (indirectly)
        // Note: Kani might not fully verify the Debug output itself,
        // but checks that calling Debug doesn't panic or cause UB.
        println!("Locked Mutex: {:?}", m);
    } // guard drops here

    {
        let guard = m.lock();
        assert_eq!(*guard, 15);
    } // guard drops here
      // Test Debug impl while unlocked
    println!("Unlocked Mutex: {:?}", m);
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_mutex_guard_deref() {
    let m = WrtMutex::new(42);
    let guard = m.lock();
    assert_eq!(*guard, 42);
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_mutex_guard_deref_mut() {
    let m = WrtMutex::new(42);
    {
        let mut guard = m.lock();
        assert_eq!(*guard, 42);
        *guard = 99;
        assert_eq!(*guard, 99);
    }
    {
        let guard = m.lock();
        assert_eq!(*guard, 99);
    }
}

// --- WrtRwLock Harnesses ---

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_rwlock_new_write_read() {
    let lock = WrtRwLock::new(0);
    {
        let mut writer = lock.write();
        *writer = 100;
        assert_eq!(*writer, 100);
        // Test Debug impl while write-locked
        println!("Write Locked RwLock: {:?}", lock);
    } // writer drops
    {
        let data = lock.read();
        assert_eq!(*data, 100);
        // Test Debug impl while read-locked
        println!("Read Locked RwLock: {:?}", lock);
    } // reader drops
      // Test Debug impl while unlocked
    println!("Unlocked RwLock: {:?}", lock);
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(6))] // More unwinding for multiple locks/spins
fn verify_rwlock_new_read_read() {
    let lock = WrtRwLock::new(55);
    let r1 = lock.read();
    let r2 = lock.read();
    assert_eq!(*r1, 55);
    assert_eq!(*r2, 55);
    drop(r1);
    drop(r2);
    // Should be able to acquire write lock now
    {
        let mut writer = lock.write();
        *writer = 66;
    }
    // Verify write
    {
        let reader = lock.read();
        assert_eq!(*reader, 66);
    }
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_rwlock_read_guard_deref() {
    let lock = WrtRwLock::new(123);
    let guard = lock.read();
    assert_eq!(*guard, 123);
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_rwlock_write_guard_derefmut() {
    let lock = WrtRwLock::new(123);
    {
        let mut guard = lock.write();
        assert_eq!(*guard, 123);
        *guard = 456;
        assert_eq!(*guard, 456);
    }
    {
        let guard = lock.read();
        assert_eq!(*guard, 456);
    }
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(3))]
fn verify_rwlock_write_unlocks_for_read() {
    let lock = WrtRwLock::new(77);
    {
        let mut w1 = lock.write();
        *w1 = 88;
    } // Drop w1
    {
        let r1 = lock.read(); // Should succeed
        assert_eq!(*r1, 88);
    }
}

#[cfg_attr(feature = "kani", kani::proof)]
#[cfg_attr(feature = "kani", kani::unwind(6))]
fn verify_rwlock_read_unlocks_for_write() {
    let lock = WrtRwLock::new(11);
    {
        let r1 = lock.read();
        assert_eq!(*r1, 11);
        // Create another read guard to ensure count > 1 logic is exercised if
        // applicable
        {
            let r2 = lock.read();
            assert_eq!(*r2, 11);
        } // r2 drops
    } // r1 drops
    {
        let mut w1 = lock.write(); // Should succeed
        assert_eq!(*w1, 11);
        *w1 = 22;
    }
    {
        let r_final = lock.read();
        assert_eq!(*r_final, 22);
    }
}

// Note: Verifying blocking behaviour (e.g., read blocks write) directly with
// Kani is tricky because Kani explores one execution path at a time
// sequentially. The best we can do is verify state transitions, as done in the
// standard tests. Kani *will* verify that the spin loops don't cause memory
// safety issues or panics within the unwinding bound.
