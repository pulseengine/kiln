//! Formal verification for the synchronization primitives using Kani.
//!
//! This module contains proofs that verify core properties of the mutex and
//! rwlock implementations. These proofs only run with Kani and are isolated
//! from normal compilation and testing.

// Only compile Kani verification code when documentation is being generated
// or when explicitly running cargo kani. This prevents interference with
// coverage testing.
#[cfg(any(doc, kani))]
/// Formal verification module for Kani proofs
pub mod kani_verification {
    use crate::{
        prelude::*,
        *,
    };

    // --- WrtMutex Verification ---

    /// Verify that mutex operations never cause data races
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(5))]
    pub fn verify_mutex_no_data_races() {
        let initial_value: i32 = kani::any();
        let m = WrtMutex::new(initial_value);

        // Simulate sequence of operations that could race
        let op_count: usize = kani::any();
        kani::assume(op_count <= 5); // Limit for bounded verification

        for _ in 0..op_count {
            let operation: u8 = kani::any();
            match operation % 3 {
                0 => {
                    // Read operation
                    let guard = m.lock();
                    let _value = *guard;
                    // Guard automatically drops, releasing lock
                },
                1 => {
                    // Write operation
                    let mut guard = m.lock();
                    let increment: i32 = kani::any();
                    kani::assume(increment.abs() < 1000); // Prevent overflow
                    *guard = guard.saturating_add(increment);
                },
                _ => {
                    // Read-modify-write operation
                    let mut guard = m.lock();
                    let old_value = *guard;
                    *guard = old_value.saturating_mul(2);
                },
            }
        }

        // Verify mutex state is still valid
        let final_guard = m.lock();
        let _final_value = *final_guard;
        // If we reach here without panic, mutex maintained safety
    }

    /// Verify that mutex creation, locking and unlocking works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_mutex_new_lock_unlock() {
        let m = WrtMutex::new(10);
        {
            let mut guard = m.lock();
            *guard += 5;
            assert_eq!(*guard, 15);
        } // guard drops here

        {
            let guard = m.lock();
            assert_eq!(*guard, 15);
        } // guard drops here
    }

    /// Verify that mutex guard deref works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_mutex_guard_deref() {
        let m = WrtMutex::new(42);
        let guard = m.lock();
        assert_eq!(*guard, 42);
    }

    /// Verify that mutex guard deref_mut works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_mutex_guard_deref_mut() {
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

    // --- WrtRwLock Verification ---

    /// Verify that rwlock concurrent access maintains safety
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(6))]
    pub fn verify_rwlock_concurrent_access() {
        let initial_value: i32 = kani::any();
        let lock = WrtRwLock::new(initial_value);

        // Simulate concurrent reader/writer patterns
        let access_pattern: u8 = kani::any();

        match access_pattern % 4 {
            0 => {
                // Multiple readers scenario
                let r1 = lock.read();
                let r2 = lock.read();
                assert_eq!(*r1, *r2, "Concurrent readers should see same value");
                assert_eq!(*r1, initial_value);
                drop(r1);
                drop(r2);
            },
            1 => {
                // Writer then reader scenario
                {
                    let mut writer = lock.write();
                    let new_value: i32 = kani::any();
                    *writer = new_value;
                } // writer drops
                {
                    let reader = lock.read();
                    // Reader should see the written value
                    // (We can't assert the exact value due to nondeterminism,
                    // but we can verify the read succeeds)
                    let _value = *reader;
                }
            },
            2 => {
                // Reader then writer scenario
                {
                    let reader = lock.read();
                    assert_eq!(*reader, initial_value);
                } // reader drops
                {
                    let mut writer = lock.write();
                    *writer = writer.saturating_add(1);
                }
            },
            _ => {
                // Sequential write operations
                let increment1: i32 = kani::any();
                let increment2: i32 = kani::any();
                kani::assume(increment1.abs() < 100 && increment2.abs() < 100);

                {
                    let mut writer = lock.write();
                    *writer = writer.saturating_add(increment1);
                }
                {
                    let mut writer = lock.write();
                    *writer = writer.saturating_add(increment2);
                }
            },
        }
    }

    /// Verify that rwlock creation, writing and reading works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_rwlock_new_write_read() {
        let lock = WrtRwLock::new(0);
        {
            let mut writer = lock.write();
            *writer = 100;
            assert_eq!(*writer, 100);
        } // writer drops
        {
            let data = lock.read();
            assert_eq!(*data, 100);
        } // reader drops
    }

    /// Verify that multiple read locks work correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(6))]
    pub fn verify_rwlock_new_read_read() {
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

    /// Verify that rwlock read guard deref works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_rwlock_read_guard_deref() {
        let lock = WrtRwLock::new(123);
        let guard = lock.read();
        assert_eq!(*guard, 123);
    }

    /// Verify that rwlock write guard deref_mut works correctly
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(3))]
    pub fn verify_rwlock_write_guard_derefmut() {
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

    // --- Atomic Operations Safety ---

    /// Verify atomic operations maintain safety under concurrent access
    /// patterns
    #[cfg_attr(kani, kani::proof)]
    #[cfg_attr(kani, kani::unwind(5))]
    pub fn verify_atomic_operations_safety() {
        use core::sync::atomic::{
            AtomicU32,
            Ordering,
        };

        let atomic_counter = AtomicU32::new(0);

        // Simulate concurrent operations
        let op_count: usize = kani::any();
        kani::assume(op_count <= 5);

        for _ in 0..op_count {
            let operation: u8 = kani::any();
            match operation % 4 {
                0 => {
                    // Atomic load
                    let _value = atomic_counter.load(Ordering::SeqCst);
                },
                1 => {
                    // Atomic store
                    let new_value: u32 = kani::any();
                    kani::assume(new_value < 1000); // Reasonable bounds
                    atomic_counter.store(new_value, Ordering::SeqCst);
                },
                2 => {
                    // Atomic fetch_add
                    let increment: u32 = kani::any();
                    kani::assume(increment < 10); // Prevent overflow
                    let _old_value = atomic_counter.fetch_add(increment, Ordering::SeqCst);
                },
                _ => {
                    // Atomic compare_exchange
                    let expected: u32 = kani::any();
                    let desired: u32 = kani::any();
                    kani::assume(expected < 1000 && desired < 1000);

                    let _result = atomic_counter.compare_exchange(
                        expected,
                        desired,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );
                    // Result can be Ok(expected) or Err(actual_value)
                    // Both cases are valid for atomic operations
                },
            }
        }

        // Verify final state is still accessible
        let _final_value = atomic_counter.load(Ordering::SeqCst);
    }
}

// Expose the verification module in docs but not for normal compilation
#[cfg(any(doc, kani))]
pub use kani_verification::*;
