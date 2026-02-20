// #![allow(unsafe_code)] // Allow unsafe for UnsafeCell and Send/Sync impls

// use crate::prelude::*;
use crate::prelude::{
    fmt,
    AtomicBool,
    Deref,
    DerefMut,
    Ordering,
    UnsafeCell,
};

/// A simple, non-reentrant spinlock mutex suitable for `no_std` environments.
///
/// WARNING: This is a basic implementation. It does not handle contention well
/// (it just spins aggressively) and lacks features like deadlock detection or
/// poisoning. Use with caution and consider alternatives if contention is
/// expected to be high.
pub struct WrtMutex<T: ?Sized> {
    locked: AtomicBool,
    data:   UnsafeCell<T>,
}

/// A guard that provides mutable access to the data protected by a `WrtMutex`.
///
/// When the guard is dropped, the mutex is unlocked.
#[clippy::has_significant_drop]
pub struct WrtMutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a WrtMutex<T>,
}

// Implementations

// Allow the mutex to be shared across threads.
/// # Safety
/// Access to the `UnsafeCell` data is protected by the atomic `locked` flag.
/// The `Send` trait is safe because the `WrtMutex` ensures that only one thread
/// can access the data at a time (if `T` is `Send`).
unsafe impl<T: ?Sized + Send> Send for WrtMutex<T> {}
/// # Safety
/// Access to the `UnsafeCell` data is protected by the atomic `locked` flag.
/// The `Sync` trait is safe because the `WrtMutex` ensures that all accesses
/// (read or write) are synchronized through the lock (if `T` is `Send`).
/// If `T` is also `Sync`, then `&WrtMutex<T>` can be safely shared.
unsafe impl<T: ?Sized + Send> Sync for WrtMutex<T> {}

impl<T> WrtMutex<T> {
    /// Creates a new `WrtMutex` protecting the given data.
    #[inline]
    pub const fn new(data: T) -> Self {
        WrtMutex {
            locked: AtomicBool::new(false),
            data:   UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> WrtMutex<T> {
    /// Acquires the lock, spinning until it is available.
    ///
    /// This function will block the current execution context until the lock is
    /// acquired.
    ///
    /// # Returns
    ///
    /// A guard that allows mutable access to the protected data.
    ///
    /// # Panics
    ///
    /// This function does not panic.
    /// Safety impact: [LOW|MEDIUM|HIGH] - [Brief explanation of the safety
    /// implication] Tracking: WRTQ-XXX (qualification requirement tracking
    /// ID).
    #[inline]
    pub fn lock(&self) -> WrtMutexGuard<'_, T> {
        // Spin until the lock is acquired.
        // Use compare_exchange_weak for potentially better performance on some
        // platforms.
        // - Acquire ordering on success: Ensures that subsequent reads of the data
        //   happen *after* the lock is acquired.
        // - Relaxed ordering on failure: We don't need guarantees on failure, just
        //   retry.
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Hint to the CPU that we are spinning.
            core::hint::spin_loop();
        }
        WrtMutexGuard { mutex: self }
    }

    // Optional: Implement try_lock if needed later
    // pub fn try_lock(&self) -> Option<WrtMutexGuard<'_, T>> {
    // if self
    // .locked
    // .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    // .is_ok()
    // {
    // Some(WrtMutexGuard { mutex: self })
    // } else {
    // None
    // }
    // }

    // Optional: Implement into_inner if needed later
    // pub fn into_inner(self) -> T where T: Sized {
    // Note: This consumes the mutex. Ensure no guards exist.
    // This is simpler without poisoning checks.
    // self.data.into_inner()
    // }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for WrtMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Attempt a non-blocking check for Debug representation if possible,
        // otherwise indicate locked status. Avoids deadlocking Debug.
        // For safety-critical code, Debug should not access potentially-locked data
        // or create race conditions. Edition 2024's stricter lifetime rules
        // correctly reject the previous unsafe dereference pattern.
        // Always show a safe representation without accessing the data.
        if self.locked.load(Ordering::Relaxed) {
            f.debug_struct("WrtMutex").field("data", &"<locked>").finish()
        } else {
            f.debug_struct("WrtMutex").field("data", &"<unlocked>").finish()
        }
    }
}

// Guard implementation

impl<T: ?Sized> Deref for WrtMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // # Safety
        // This `unsafe` block dereferences the raw pointer from `UnsafeCell::get()`.
        // It is safe because a `WrtMutexGuard` can only be created if the
        // associated `WrtMutex` is locked. The existence of the guard guarantees
        // exclusive (for `&mut`) or shared (for `&`) access to the data.
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for WrtMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // # Safety
        // This `unsafe` block dereferences the raw pointer from `UnsafeCell::get()`
        // for mutable access. It is safe because a `WrtMutexGuard` can only
        // be created if the associated `WrtMutex` is locked. The existence of the
        // guard guarantees exclusive access to the data.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for WrtMutexGuard<'_, T> {
    /// Releases the lock when the guard goes out of scope.
    #[inline]
    fn drop(&mut self) {
        // Release the lock.
        // - Release ordering: Ensures that all writes to the data *before* this point
        //   are visible to other threads *after* they acquire the lock.
        self.mutex.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    // use crate::prelude::*;
    // For std-specific parts of tests, ensure std imports are scoped or handled by
    // feature flags.
    #[cfg(feature = "std")]
    use std::{
        sync::Arc,
        thread,
    };

    use super::*;

    #[test]
    fn test_mutex_creation() {
        let mutex = WrtMutex::new(42);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }

    #[test]
    #[cfg(any(feature = "std", feature = "dynamic-allocation"))]
    fn test_mutex_modification() {
        let mutex = WrtMutex::new(vec![1, 2, 3]);
        {
            let mut guard = mutex.lock();
            guard.push(4);
        }
        let guard = mutex.lock();
        assert_eq!(*guard, vec![1, 2, 3, 4]);
    }

    #[test]
    #[cfg(any(feature = "std", feature = "dynamic-allocation"))]
    fn test_mutex_multiple_locks() {
        let mutex = WrtMutex::new(String::from("test"));
        {
            let mut guard = mutex.lock();
            guard.push_str("_1");
        }
        {
            let mut guard = mutex.lock();
            guard.push_str("_2");
        }
        let guard = mutex.lock();
        assert_eq!(*guard, "test_1_2");
    }

    #[test]
    fn test_mutex_send_sync() {
        // This test verifies that WrtMutex implements Send and Sync
        // by checking trait bounds (compile-time check)
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WrtMutex<i32>>();
    }

    #[test]
    fn test_mutex_guard_drop() {
        let mutex = WrtMutex::new(42);
        {
            let mut guard = mutex.lock();
            *guard = 100;
        } // guard is dropped here, releasing the lock
        let guard = mutex.lock(); // Should be able to re-acquire lock
        assert_eq!(*guard, 100);
    }

    // Basic concurrency test (requires std for threading)
    #[cfg(feature = "std")]
    #[test]
    fn test_mutex_concurrency() {
        let mutex = Arc::new(WrtMutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let mutex_clone = Arc::clone(&mutex);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = mutex_clone.lock();
                    *guard += 1;
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, 10 * 1000);
    }
}
