// #![allow(unsafe_code)] // Allow unsafe for UnsafeCell, raw pointers, and
// Send/Sync impls

// WRT - wrt-sync
// Module: OnceCell - A one-time initialization primitive
// SW-REQ-ID: REQ_CONCURRENCY_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! A cell that can be initialized at most once.
//!
//! This module provides a `WrtOnce<T>` type that allows for safe, one-time
//! initialization of a value, typically used for global statics.

use core::{
    cell::UnsafeCell,
    fmt,
    mem::MaybeUninit,
};

use crate::mutex::WrtMutex; // Using the WrtMutex from this crate. Removed WrtMutexGuard as
                            // it's not used directly here.

/// A synchronization primitive which can be written to only once.
///
/// This type is analogous to `std::sync::OnceLock` or
/// `once_cell::sync::OnceCell`.
///
/// # Safety
/// This implementation uses `UnsafeCell` to store the actual data `T` once
/// initialized. The `WrtMutex<bool>` (`initialized_lock`) is used to ensure
/// that the initialization logic runs at most once. After initialization, the
/// data is considered immutable and references can be safely given out.
pub struct WrtOnce<T> {
    mutex:       WrtMutex<()>, // Used as a traditional lock for the initialization check
    data:        UnsafeCell<MaybeUninit<T>>,
    initialized: AtomicBool, // Tracks if `data` is initialized
}

/// # Safety
/// This implementation of `Send` for `WrtOnce<T>` is safe if `T` is `Send`,
/// because access to the inner `UnsafeCell<MaybeUninit<T>>` data is
/// synchronized. During initialization, the `mutex` ensures exclusive access.
/// After initialization (`initialized` is true), the data is treated as
/// immutable and accessed via shared references (`&T`), which is safe. The
/// `AtomicBool` (`initialized`) is also `Send`.
unsafe impl<T: Send> Send for WrtOnce<T> {}

/// # Safety
/// This implementation of `Sync` for `WrtOnce<T>` is safe if `T` is `Send +
/// Sync`. `Send` is required because `T` is stored and potentially moved across
/// threads during initialization. `Sync` is required because after
/// initialization, `&T` can be shared across threads. Access to the inner
/// `UnsafeCell<MaybeUninit<T>>` data is synchronized: during initialization,
/// the `mutex` ensures exclusive access. After initialization (`initialized` is
/// true), the data is accessed via `&T`. If `T` is `Sync`, then `&T` is `Send`,
/// allowing `&WrtOnce<T>` to be `Sync`. The `AtomicBool` (`initialized`) is
/// also `Sync`.
unsafe impl<T: Send + Sync> Sync for WrtOnce<T> {}

impl<T> WrtOnce<T> {
    /// Creates a new, empty `WrtOnce`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mutex:       WrtMutex::new(()),
            data:        UnsafeCell::new(MaybeUninit::uninit()),
            initialized: AtomicBool::new(false),
        }
    }

    /// Gets the reference to the underlying value, initializing it if
    /// necessary.
    ///
    /// If `WrtOnce` is already initialized, this method returns immediately.
    /// Otherwise, it calls `f` to initialize the value, and then returns a
    /// reference. If multiple threads attempt to initialize the same
    /// `WrtOnce` concurrently, only one thread will execute `f()`, and
    /// others will block until initialization is complete.
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // Fast path: check if already initialized without locking.
        // Acquire ordering ensures that if we see `true`, the write to `data`
        // by the initializing thread is also visible.
        if self.initialized.load(Ordering::Acquire) {
            // Safety: `initialized` is true, so `data` has been initialized.
            // The Acquire load synchronizes with the Release store by the initializing
            // thread. It's safe to assume `data` contains a valid `T`.
            // # Safety
            // This `unsafe` block calls `get_unchecked`. It is safe because
            // `self.initialized` was loaded with `Ordering::Acquire` and found
            // to be true. This synchronizes with the `Ordering::Release` store
            // in `get_or_init` after the data is written, ensuring that the
            // data is initialized and visible.
            return unsafe { self.get_unchecked() };
        }

        // Slow path: acquire lock and attempt initialization.
        let _guard = self.mutex.lock(); // Lock to ensure only one thread initializes.

        // Double-check: another thread might have initialized while we were waiting for
        // the lock.
        if self.initialized.load(Ordering::Relaxed) {
            // Relaxed is fine here, guarded by mutex.
            // Safety: Same as above fast path, but now under lock.
            // # Safety
            // This `unsafe` block calls `get_unchecked`. It is safe because
            // `self.initialized` is checked under the protection of
            // `self.mutex`. If true, the data has been initialized by another
            // thread. The lock ensures memory visibility.
            return unsafe { self.get_unchecked() };
        }

        // We are the thread to initialize it.
        let value = f();
        // # Safety
        // This `unsafe` block writes to the `UnsafeCell<MaybeUninit<T>>` via
        // `get().write()`. It is safe because:
        // 1. The `self.mutex` is held, ensuring exclusive access for initialization.
        // 2. `self.initialized` is false at this point (checked before and under lock),
        //    so we are the designated initializer.
        // 3. `MaybeUninit::write` is the correct way to initialize a `MaybeUninit`
        //    value.
        unsafe {
            // Write the initialized value.
            (*self.data.get()).write(value);
        }

        // Mark as initialized. Release ordering ensures that the write to `data`
        // happens-before any subsequent Acquire load of `initialized` by other threads.
        self.initialized.store(true, Ordering::Release);

        // Safety: We just initialized it.
        // # Safety
        // This `unsafe` block calls `get_unchecked`. It is safe because we have just
        // written to `self.data` and set `self.initialized` to true within the
        // critical section protected by `self.mutex`.
        unsafe { self.get_unchecked() }
    }

    /// Gets a reference to the underlying value if it is initialized.
    ///
    /// Returns `None` if the `WrtOnce` has not been initialized yet.
    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(Ordering::Acquire) {
            // Safety: `initialized` is true, so `data` has been initialized.
            // # Safety
            // This `unsafe` block calls `get_unchecked`. It is safe for the same reasons
            // as in the fast path of `get_or_init`: `self.initialized` is true (checked
            // with `Ordering::Acquire`), ensuring data is initialized and visible.
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the `WrtOnce<T>` has been initialized prior to
    /// calling this. This is typically ensured by checking
    /// `initialized.load(Ordering::Acquire)` first, or by virtue of program
    /// logic (e.g., after `get_or_init` has completed).
    #[inline]
    unsafe fn get_unchecked(&self) -> &T {
        // Safety: The caller guarantees that `data` is initialized.
        // `MaybeUninit::assume_init_ref` is the correct way to get a reference
        // from an initialized `MaybeUninit`.
        // # Safety
        // Edition 2024 requires explicit unsafe blocks in unsafe functions.
        // This block accesses the `UnsafeCell<MaybeUninit<T>>` and calls
        // `assume_init_ref()`. This is safe because the method contract of
        // `get_unchecked` (an `unsafe fn`) requires the caller to guarantee
        // that the data has been initialized.
        unsafe { (*self.data.get()).assume_init_ref() }
    }
}

impl<T> Default for WrtOnce<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for WrtOnce<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get() {
            Some(v) => f.debug_struct("WrtOnce").field("data", v).finish(),
            None => f.debug_struct("WrtOnce").field("data", &"<uninitialized>").finish(),
        }
    }
}

// It's good practice to ensure Drop is handled, though for `MaybeUninit<T>`
// where T might need dropping, if WrtOnce itself is dropped, the T inside
// MaybeUninit should be dropped if it was initialized.
impl<T> Drop for WrtOnce<T> {
    fn drop(&mut self) {
        // If initialized, take the value and drop it.
        // Relaxed ordering is fine for the load, as `drop` has exclusive access.
        if *self.initialized.get_mut() {
            // get_mut provides safe access to the bool
            // # Safety
            // This `unsafe` block accesses the `UnsafeCell<MaybeUninit<T>>` and calls
            // `assume_init_drop()`. It is safe because:
            // 1. `&mut self` guarantees exclusive access during drop.
            // 2. `*self.initialized.get_mut()` is true, meaning the data was initialized.
            // 3. `assume_init_drop()` is the correct way to drop an initialized value held
            //    in `MaybeUninit` when the `MaybeUninit` itself is being dropped or its
            //    lifetime ends.
            unsafe {
                // Safety: We have exclusive access (`&mut self`) and `initialized` is true,
                // so `data` contains an initialized `T`.
                (*self.data.get_mut()).assume_init_drop();
            }
        }
    }
}

// Re-import core items used by the module that might not be in scope otherwise
// in no_std
use core::sync::atomic::{
    AtomicBool,
    Ordering,
};
