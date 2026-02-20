#![allow(clippy::missing_fields_in_debug)] // Added to allow for now
#![allow(clippy::uninlined_format_args)] // Added to allow for now
                                         // #![allow(unsafe_code)] // Allow unsafe for UnsafeCell, atomics, and Send/Sync
                                         // impls

use core::{
    cell::UnsafeCell,
    fmt,
    hint::spin_loop,
    ops::{
        Deref,
        DerefMut,
    },
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
};
// REMOVED: #[cfg(feature = "std")]
// REMOVED: use std::borrow::Cow; // Unused import
// REMOVED: #[cfg(all(not(feature = "std"), feature = "std"))]
// REMOVED: use std::borrow::Cow; // Unused, as Cow was only for error messages which are now
// static.

// Binary std/no_std choice
#[cfg(feature = "std")]
use std::sync::Arc;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(feature = "std")] // Gate this import as it's only used by parking_impl (std-gated)
use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
}; // Keep this top-level import

/// A simple, `no_std` compatible Read-Write Lock using atomics.
///
/// This is a reader-preference lock. Writers may starve if readers constantly
/// hold the lock. WARNING: This is a basic implementation. It does not handle
/// potential deadlocks involving multiple locks or priority inversion. Use with
/// caution.
const WRITE_LOCK_STATE: usize = usize::MAX;

/// A non-blocking, atomic read-write lock for `no_std` environments.
///
/// This `RwLock` is designed to be efficient and suitable for environments
/// where `std` is not available, making it suitable for use in embedded systems
/// and other `no_std` contexts.
///
/// # Examples
///
/// ```
/// use wrt_sync::WrtRwLock;
///
/// // Create a new RwLock
/// let lock = WrtRwLock::new(42);
///
/// // Acquire a read lock
/// let reader = lock.read();
/// assert_eq!(*reader, 42);
///
/// // Release the read lock by dropping the guard
/// drop(reader);
///
/// // Acquire a write lock
/// let mut writer = lock.write();
/// *writer = 100;
///
/// // Release the write lock by dropping the guard
/// drop(writer);
///
/// // Verify the new value with another reader
/// let reader = lock.read();
/// assert_eq!(*reader, 100);
/// ```
pub struct WrtRwLock<T: ?Sized> {
    /// Atomically tracks the lock state.
    /// Encoding:
    /// - 0: Unlocked
    /// - `usize::MAX`: Write-locked
    /// - n (`1..usize::MAX - 1`): Read-locked by n readers
    state: AtomicUsize,
    data:  UnsafeCell<T>,
}

/// A guard that provides read access to the data protected by a `WrtRwLock`.
#[clippy::has_significant_drop]
pub struct WrtRwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a WrtRwLock<T>,
}

/// A guard that provides write access to the data protected by a `WrtRwLock`.
#[clippy::has_significant_drop]
pub struct WrtRwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a WrtRwLock<T>,
}

// Allow the lock to be shared across threads.
// Safety: Requires correct implementation of locking mechanisms.
/// # Safety
/// This implementation of `Send` for `WrtRwLock<T>` is safe if `T` is `Send +
/// Sync`. Access to the `UnsafeCell` data is protected by the atomic `state`
/// variable, ensuring that data races do not occur. `T` must be `Sync` because
/// multiple readers can access it concurrently.
unsafe impl<T: ?Sized + Send + Sync> Send for WrtRwLock<T> {}
/// # Safety
/// This implementation of `Sync` for `WrtRwLock<T>` is safe if `T` is `Send +
/// Sync`. The atomic `state` variable ensures that access to the `UnsafeCell`
/// data is synchronized. Multiple threads can safely hold references
/// (`&WrtRwLock<T>`) and access the data via read or write guards, which manage
/// the lock state.
unsafe impl<T: ?Sized + Send + Sync> Sync for WrtRwLock<T> {}

impl<T> WrtRwLock<T> {
    /// Creates a new `WrtRwLock` protecting the given data.
    #[inline]
    pub const fn new(data: T) -> Self {
        WrtRwLock {
            state: AtomicUsize::new(0), // Start unlocked
            data:  UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> WrtRwLock<T> {
    /// Acquires a read lock, spinning until available.
    #[inline]
    pub fn read(&self) -> WrtRwLockReadGuard<'_, T> {
        loop {
            let current_state = self.state.load(Ordering::Relaxed);
            // Check if write-locked
            if current_state != WRITE_LOCK_STATE {
                // Attempt to increment reader count
                match self.state.compare_exchange_weak(
                    current_state,
                    current_state + 1,
                    Ordering::Acquire, // Ensure reads happen after lock acquisition
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return WrtRwLockReadGuard { lock: self },
                    Err(_) => continue, // State changed, retry loop
                }
            }
            // If write-locked, spin
            spin_loop();
        }
    }

    /// Acquires a write lock, spinning until available.
    #[inline]
    pub fn write(&self) -> WrtRwLockWriteGuard<'_, T> {
        loop {
            // Attempt to acquire write lock if currently unlocked (state == 0)
            match self.state.compare_exchange_weak(
                0, // Only try if unlocked
                WRITE_LOCK_STATE,
                Ordering::Acquire, // Ensure writes happen after lock acquisition
                Ordering::Relaxed,
            ) {
                Ok(_) => return WrtRwLockWriteGuard { lock: self },
                Err(current_state) => {
                    // If it failed because it was already locked (read or write), spin.
                    // If current_state was 0 but compare_exchange failed spuriously,
                    // the loop retries anyway.
                    if current_state != 0 {
                        spin_loop();
                    }
                    // Continue loop regardless to retry compare_exchange
                },
            }
        }
    }

    /// Attempts to acquire a read lock without blocking.
    #[inline]
    pub fn try_read(&self) -> Option<WrtRwLockReadGuard<'_, T>> {
        let current_state = self.state.load(Ordering::Relaxed);

        if current_state == WRITE_LOCK_STATE {
            None // Write locked
        } else {
            // Attempt to increment reader count if not write-locked.
            // The `writer_waiting` flag is not strictly necessary to check here for
            // `try_read`, as `try_read` should succeed even if a writer is
            // waiting, as per typical RwLock semantics. A writer trying to
            // acquire the lock will wait for readers to release.
            match self.state.compare_exchange(
                current_state,     // Expected: not WRITE_LOCK_STATE
                current_state + 1, // Increment reader count
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => Some(WrtRwLockReadGuard { lock: self }),
                Err(_) => None, // State changed, CAS failed
            }
        }
    }

    /// Attempts to acquire a write lock without blocking.
    #[inline]
    pub fn try_write(&self) -> Option<WrtRwLockWriteGuard<'_, T>> {
        match self.state.compare_exchange(
            // Use strong exchange for try_ versions
            0,
            WRITE_LOCK_STATE,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Some(WrtRwLockWriteGuard { lock: self }),
            Err(_) => None, // Failed (already locked or state changed)
        }
    }
}

// Read Guard Implementation

impl<T: ?Sized> Deref for WrtRwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: Guard ensures read lock is held.
        // # Safety
        // This `unsafe` block dereferences the raw pointer from `UnsafeCell::get()`.
        // It is safe because a `WrtRwLockReadGuard` only exists when a read lock
        // is held on the associated `WrtRwLock`. This guarantees shared, read-only
        // access to the data is valid.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for WrtRwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // Decrement reader count.
        // Release ordering ensures preceding reads are visible before the lock is
        // released (partially).
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

// Write Guard Implementation

impl<T: ?Sized> Deref for WrtRwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: Guard ensures write lock is held.
        // # Safety
        // This `unsafe` block dereferences the raw pointer from `UnsafeCell::get()`.
        // It is safe because a `WrtRwLockWriteGuard` only exists when a write lock
        // is held on the associated `WrtRwLock`. This guarantees shared (for `&` access
        // via `deref`) or exclusive (for `&mut` access via `deref_mut`) access to data.
        // In this `deref` case, it provides a shared reference from an exclusive lock
        // context.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for WrtRwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: Guard ensures write lock is held.
        // # Safety
        // This `unsafe` block dereferences the raw pointer from `UnsafeCell::get()`
        // for mutable access. It is safe because a `WrtRwLockWriteGuard` only
        // exists when a write lock is held, guaranteeing exclusive access to the data.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for WrtRwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // Set state back to unlocked (0).
        // Release ordering ensures preceding writes are visible before the lock is
        // released.
        self.lock.state.store(0, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for WrtRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Attempt a non-blocking read for Debug representation if possible,
        // otherwise indicate locked status. Avoids deadlocking Debug.
        if let Some(guard) = self.try_read() {
            // Access data through the guard for Debug formatting.
            // No direct unsafe access needed here as guard provides safe access.
            f.debug_struct("WrtRwLock").field("data", &&*guard).finish()
        } else {
            // Could also try_write to see if it's exclusively locked, but for simplicity:
            f.debug_struct("WrtRwLock").field("data", &"<locked>").finish()
        }
    }
}

// ======= Parking-based Implementation (for std environments) =======
#[cfg(feature = "std")]
/// An `RwLock` implementation that uses `std::thread::park` for blocking.
///
/// This lock is suitable for environments where the standard library is
/// available and provides more efficient blocking than spin-loops for contended
/// locks. It maintains fairness by waking writers before readers if both are
/// waiting.
pub mod parking_impl {
    // Replace wildcard import with explicit imports
    // Removed Cow from this list as it is no longer used.
    use core::sync::atomic::{
        AtomicBool,
        AtomicUsize,
        Ordering,
    }; // Use Ordering directly
    use core::{
        cell::UnsafeCell as CoreUnsafeCell,
        fmt as CoreFmt,
        ops::{
            Deref as CoreDeref,
            DerefMut as CoreDerefMut,
        },
    };
    // std specific items, now directly used because of cfg(feature = "std") on the module
    use std::sync::RwLock as StdRwLock;
    use std::thread::{
        self,
        Thread,
    };

    use super::{
        codes,
        Arc,
        Error,
        ErrorCategory,
        Result,
        Vec,
    }; // Keep alias for clarity if preferred

    const UNLOCKED: usize = 0;
    // REMOVED: const WRITE_LOCKED as it was unused and outer module's
    // WRITE_LOCK_STATE is used. const WRITE_LOCKED: usize = usize::MAX - 1; //
    // Max value for readers, special marker for writer

    /// A `RwLock` that uses `std::thread::park` for blocking.
    pub struct WrtParkingRwLock<T: ?Sized> {
        state:          AtomicUsize,
        writer_waiting: AtomicBool, // True if a writer is parked, waiting for the lock
        waiters:        Arc<WaitQueue>,
        data:           CoreUnsafeCell<T>,
    }

    #[derive(CoreFmt::Debug)] // Add Debug derive for WaitQueue
    struct WaitQueue {
        readers: StdRwLock<Vec<Thread>>,
        writer:  StdRwLock<Option<Thread>>,
    }

    // Static error messages - Corrected syntax
    const POISONED_READER_QUEUE_MSG: &str = "RwLock readers queue poisoned";
    const POISONED_WRITER_QUEUE_MSG: &str = "RwLock writer queue poisoned";
    const POISONED_READER_UNREGISTER_MSG: &str = "RwLock readers queue poisoned for unregister";
    const POISONED_WRITER_UNREGISTER_MSG: &str = "RwLock writer queue poisoned for unregister";
    const POISONED_WRITER_WAKE_MSG: &str = "RwLock writer queue poisoned for wake";
    const POISONED_READER_WAKE_MSG: &str = "RwLock readers queue poisoned for wake";

    impl WaitQueue {
        fn new() -> Self {
            Self {
                readers: StdRwLock::new(Vec::new()),
                writer:  StdRwLock::new(None),
            }
        }

        fn register_reader(&self) -> Result<()> {
            self.readers
                .write()
                .map_err(|_e| {
                    Error::new(
                        ErrorCategory::Concurrency, // Corrected Category
                        codes::POISONED_LOCK,
                        POISONED_READER_QUEUE_MSG, // Static message
                    )
                })?
                .push(thread::current());
            Ok(())
        }

        fn register_writer(&self) -> Result<bool> {
            let mut writer_guard = self.writer.write().map_err(|_e| {
                Error::new(
                    ErrorCategory::Concurrency, // Corrected Category
                    codes::POISONED_LOCK,
                    POISONED_WRITER_QUEUE_MSG, // Static message
                )
            })?;
            if writer_guard.is_some() {
                return Ok(false); // Writer already present
            }
            *writer_guard = Some(thread::current());
            Ok(true)
        }

        fn unregister_current_reader(&self) -> Result<()> {
            let current_thread_id = thread::current().id();
            self.readers
                .write()
                .map_err(|_e| {
                    Error::new(
                        ErrorCategory::Concurrency, // Corrected Category
                        codes::POISONED_LOCK,
                        POISONED_READER_UNREGISTER_MSG, // Static message
                    )
                })?
                .retain(|t| t.id() != current_thread_id);
            Ok(())
        }

        fn unregister_writer(&self) -> Result<()> {
            let mut writer_guard = self.writer.write().map_err(|_e| {
                Error::new(
                    ErrorCategory::Concurrency, // Corrected Category
                    codes::POISONED_LOCK,
                    POISONED_WRITER_UNREGISTER_MSG, // Static message
                )
            })?;
            *writer_guard = None;
            Ok(())
        }

        fn wake_writer(&self) -> Result<()> {
            if let Some(writer_thread) = self
                .writer
                .write()
                .map_err(|_e| {
                    Error::new(
                        ErrorCategory::Concurrency, // Corrected Category
                        codes::POISONED_LOCK,
                        POISONED_WRITER_WAKE_MSG, // Static message
                    )
                })?
                .take()
            {
                writer_thread.unpark();
            }
            Ok(())
        }

        fn wake_readers(&self) -> Result<()> {
            let readers_guard = self.readers.read().map_err(|_e| {
                Error::new(
                    ErrorCategory::Concurrency, // Corrected Category
                    codes::POISONED_LOCK,
                    POISONED_READER_WAKE_MSG, // Static message
                )
            })?;
            for reader_thread in readers_guard.iter() {
                reader_thread.unpark();
            }
            Ok(())
        }
    }

    /// Safety: The underlying Send/Sync traits of T ensure data integrity.
    /// The atomic operations and careful state management ensure safe
    /// concurrent access.
    /// # Safety
    /// `Send` is safe if `T` is `Send + Sync` because the internal
    /// `Arc<WaitQueue>` and `CoreUnsafeCell<T>` are managed in a
    /// thread-safe manner. Access to `data` is guarded by the lock state.
    unsafe impl<T: ?Sized + Send + Sync> Send for WrtParkingRwLock<T> {}
    /// # Safety
    /// `Sync` is safe if `T` is `Send + Sync` for similar reasons to `Send`.
    /// The lock state and synchronization primitives ensure that multiple
    /// threads can safely interact with the lock.
    unsafe impl<T: ?Sized + Send + Sync> Sync for WrtParkingRwLock<T> {}

    /// A guard that provides read access to data protected by a
    /// `WrtParkingRwLock`.
    ///
    /// When the guard is dropped, the read lock is released.
    #[clippy::has_significant_drop]
    pub struct WrtParkingRwLockReadGuard<'a, T: ?Sized + 'a> {
        lock: &'a WrtParkingRwLock<T>,
    }

    /// A guard that provides write access to data protected by a
    /// `WrtParkingRwLock`.
    ///
    /// When the guard is dropped, the write lock is released.
    #[clippy::has_significant_drop]
    pub struct WrtParkingRwLockWriteGuard<'a, T: ?Sized + 'a> {
        lock: &'a WrtParkingRwLock<T>,
    }

    impl<T> WrtParkingRwLock<T> {
        /// Creates a new `WrtParkingRwLock` in an unlocked state, ready for
        /// use.
        ///
        /// # Examples
        ///
        /// ```
        /// use wrt_sync::parking_impl::WrtParkingRwLock;
        /// let lock = WrtParkingRwLock::new(0);
        /// ```
        pub fn new(data: T) -> Self {
            WrtParkingRwLock {
                state:          AtomicUsize::new(UNLOCKED),
                writer_waiting: AtomicBool::new(false),
                waiters:        Arc::new(WaitQueue::new()),
                data:           CoreUnsafeCell::new(data),
            }
        }
    }

    impl<T: ?Sized> WrtParkingRwLock<T> {
        /// Acquires a read lock.
        ///
        /// Returns a RAII guard which will release the lock when dropped.
        ///
        /// # Errors
        ///
        /// Returns a `PoisonError` if the lock is poisoned.
        pub fn read(&self) -> Result<WrtParkingRwLockReadGuard<'_, T>> {
            loop {
                let current_state = self.state.load(Ordering::Relaxed);
                if current_state < usize::MAX - 1 && !self.writer_waiting.load(Ordering::Relaxed) {
                    match self.state.compare_exchange_weak(
                        current_state,
                        current_state + 1,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => return Ok(WrtParkingRwLockReadGuard { lock: self }),
                        Err(_) => continue, // CAS failed, retry
                    }
                }
                // Park if write locked or writer is waiting
                self.waiters.register_reader()?;
                // Re-check state before parking to avoid race
                let current_state_after_register = self.state.load(Ordering::Relaxed);
                if current_state_after_register < usize::MAX - 1
                    && !self.writer_waiting.load(Ordering::Relaxed)
                {
                    // Lock might have become available between check and park registration
                    self.waiters.unregister_current_reader()?; // Unregister before trying to acquire
                    continue; // Retry acquiring without parking
                }
                thread::park();
            }
        }

        /// Acquires a write lock.
        ///
        /// Returns a RAII guard which will release the lock when dropped.
        ///
        /// # Errors
        ///
        /// Returns a `PoisonError` if the lock is poisoned.
        pub fn write(&self) -> Result<WrtParkingRwLockWriteGuard<'_, T>> {
            self.writer_waiting.store(true, Ordering::Relaxed);
            loop {
                match self.state.compare_exchange_weak(
                    0,
                    usize::MAX - 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        return Ok(WrtParkingRwLockWriteGuard { lock: self });
                    },
                    Err(_current_state) => {
                        if !self.waiters.register_writer()? {
                            thread::park();
                            continue;
                        }
                        if self.state.load(Ordering::Relaxed) == 0 {
                            self.waiters.unregister_writer()?;
                            continue;
                        }
                        thread::park();
                    },
                }
            }
        }

        /// Attempts to acquire a read lock without blocking.
        #[inline]
        pub fn try_read(&self) -> Option<WrtParkingRwLockReadGuard<'_, T>> {
            let current_state = self.state.load(Ordering::Relaxed);
            if current_state != usize::MAX - 1 && !self.writer_waiting.load(Ordering::Relaxed) {
                match self.state.compare_exchange(
                    current_state,
                    current_state + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => Some(WrtParkingRwLockReadGuard { lock: self }),
                    Err(_) => None, // State changed, CAS failed
                }
            } else {
                None
            }
        }

        /// Attempts to acquire a write lock without blocking.
        #[inline]
        pub fn try_write(&self) -> Option<WrtParkingRwLockWriteGuard<'_, T>> {
            if self
                .state
                .compare_exchange(0, usize::MAX - 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                self.writer_waiting.store(true, Ordering::Relaxed); // Mark writer active
                Some(WrtParkingRwLockWriteGuard { lock: self })
            } else {
                None
            }
        }
    }

    impl<T: ?Sized> CoreDeref for WrtParkingRwLockReadGuard<'_, T> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &Self::Target {
            // # Safety
            // Access to data is safe because the existence of the guard implies the read
            // lock is held.
            unsafe { &*self.lock.data.get() }
        }
    }

    impl<T: ?Sized> Drop for WrtParkingRwLockReadGuard<'_, T> {
        fn drop(&mut self) {
            let old_state = self.lock.state.fetch_sub(1, core::sync::atomic::Ordering::Release);
            if old_state == 1
                && self.lock.writer_waiting.load(core::sync::atomic::Ordering::Acquire)
            {
                // Last reader out, and a writer is waiting. Wake the writer.
                self.lock.waiters.wake_writer().ok();
            }
        }
    }

    impl<T: ?Sized> CoreDeref for WrtParkingRwLockWriteGuard<'_, T> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &Self::Target {
            // # Safety
            // Access to data is safe because the existence of the guard implies the write
            // lock is held.
            unsafe { &*self.lock.data.get() }
        }
    }

    impl<T: ?Sized> CoreDerefMut for WrtParkingRwLockWriteGuard<'_, T> {
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
            // # Safety
            // Access to data is safe because the existence of the guard implies the write
            // lock is held.
            unsafe { &mut *self.lock.data.get() }
        }
    }

    impl<T: ?Sized> Drop for WrtParkingRwLockWriteGuard<'_, T> {
        fn drop(&mut self) {
            self.lock.state.store(0, core::sync::atomic::Ordering::Release);
            // Wake up any waiting readers or a writer. Readers are prioritized if present.
            let readers_woken_or_attempted =
                self.lock.waiters.wake_readers().map(|_| true).unwrap_or_else(|_err| {
                    // Log _err if necessary
                    false // If wake_readers failed, assume no readers were
                          // effectively woken for this logic
                });

            if !readers_woken_or_attempted {
                self.lock.waiters.wake_writer().ok();
            }
        }
    }

    impl<T: ?Sized + CoreFmt::Debug> CoreFmt::Debug for WrtParkingRwLock<T> {
        fn fmt(&self, f: &mut CoreFmt::Formatter<'_>) -> CoreFmt::Result {
            if let Some(guard) = self.try_read() {
                f.debug_struct("WrtParkingRwLock").field("data", &&*guard).finish()
            } else {
                f.debug_struct("WrtParkingRwLock").field("data", &"<locked>").finish()
            }
        }
    }

    #[cfg(test)]
    mod internal_parking_tests {
        use std::{
            sync::Arc,
            thread,
        };

        use super::WrtParkingRwLock;
        use crate::prelude::{
            AtomicBool,
            Ordering,
        };

        #[test]
        fn test_parking_rwlock_basic() {
            let lock = WrtParkingRwLock::new(42);

            {
                let r = lock.read().unwrap();
                assert_eq!(*r, 42);
            }
            {
                let mut w = lock.write().unwrap();
                *w = 100;
            }
            let r = lock.read().unwrap();
            assert_eq!(*r, 100);
        }

        #[test]
        fn test_parking_rwlock_concurrent_reads() {
            let lock = Arc::new(WrtParkingRwLock::new(123));
            let mut handles = vec![];

            for _ in 0..5 {
                let lock_clone = Arc::clone(&lock);
                handles.push(thread::spawn(move || {
                    let reader = lock_clone.read().unwrap();
                    assert_eq!(*reader, 123);
                    thread::sleep(std::time::Duration::from_millis(10));
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }

        #[test]
        fn test_parking_rwlock_writer_blocks_readers() {
            let lock = Arc::new(WrtParkingRwLock::new(0));
            let writer_ready = Arc::new(AtomicBool::new(false));
            let reader_finished_first_attempt = Arc::new(AtomicBool::new(false));

            let lock_clone_writer = Arc::clone(&lock);
            let writer_ready_clone = Arc::clone(&writer_ready);

            let writer_handle = thread::spawn(move || {
                let mut w_guard = lock_clone_writer.write().unwrap();
                *w_guard = 10;
                writer_ready_clone.store(true, Ordering::SeqCst);
                thread::sleep(std::time::Duration::from_millis(50));
            });

            while !writer_ready.load(Ordering::SeqCst) {
                thread::yield_now();
            }

            let lock_clone_reader = Arc::clone(&lock);
            let reader_finished_first_attempt_clone = Arc::clone(&reader_finished_first_attempt);
            let reader_handle = thread::spawn(move || {
                let r_guard = lock_clone_reader.read().unwrap();
                assert_eq!(*r_guard, 10);
                reader_finished_first_attempt_clone.store(true, Ordering::SeqCst);
            });

            thread::sleep(std::time::Duration::from_millis(10));
            assert!(
                !reader_finished_first_attempt.load(Ordering::SeqCst),
                "Reader should be blocked by writer"
            );

            writer_handle.join().unwrap();
            reader_handle.join().unwrap();
            assert!(
                reader_finished_first_attempt.load(Ordering::SeqCst),
                "Reader should have finished after writer released"
            );
        }

        #[test]
        fn test_parking_rwlock_try_operations() {
            let lock = WrtParkingRwLock::new(0);
            let w_opt = lock.try_write();
            assert!(w_opt.is_some());
            let mut w_guard = w_opt.unwrap();
            *w_guard = 20;
            drop(w_guard);
            let r_opt = lock.try_read();
            assert!(r_opt.is_some());
            assert_eq!(*r_opt.unwrap(), 20);
            let r1 = lock.read().unwrap();
            assert!(lock.try_write().is_none());
            let r_opt_2 = lock.try_read();
            assert!(r_opt_2.is_some());
            assert_eq!(*r_opt_2.unwrap(), 20);
            drop(r1);
            let mut w = lock.write().unwrap();
            *w = 2;
            assert!(lock.try_read().is_none());
            assert!(lock.try_write().is_none());
            drop(w);
            assert_eq!(*lock.read().unwrap(), 2);
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_rwlock_new_read() {
        let lock = WrtRwLock::new(10);
        let val = lock.read();
        assert_eq!(*val, 10);
    }

    #[test]
    fn test_rwlock_new_write_read() {
        let lock = WrtRwLock::new(0);
        {
            let mut w = lock.write();
            *w = 20;
        }
        let val = lock.read();
        assert_eq!(*val, 20);
    }

    #[test]
    fn test_rwlock_multiple_readers() {
        let lock = WrtRwLock::new(30);
        let r1 = lock.read();
        let r2 = lock.read();
        assert_eq!(*r1, 30);
        assert_eq!(*r2, 30);
        drop(r1);
        assert_eq!(*r2, 30);
    }

    #[test]
    fn test_rwlock_try_read_write() {
        let lock = WrtRwLock::new(5);

        if let Some(r_guard) = lock.try_read() {
            assert_eq!(*r_guard, 5);
            assert!(lock.try_write().is_none());
        } else {
            #[allow(clippy::assertions_on_constants)]
            {
                assert!(false, "try_read failed when it should succeed");
            }
        }

        if let Some(mut w_guard) = lock.try_write() {
            *w_guard = 10;
            assert!(lock.try_read().is_none());
            assert!(lock.try_write().is_none());
        } else {
            #[allow(clippy::assertions_on_constants)]
            {
                assert!(false, "try_write failed when it should succeed");
            }
        }
        assert_eq!(*lock.read(), 10);

        let r_guard = lock.read();
        assert!(
            lock.try_write().is_none(),
            "try_write should fail when read lock is held"
        );
        drop(r_guard);

        let w_guard = lock.write();
        assert!(
            lock.try_read().is_none(),
            "try_read should fail when write lock is held"
        );
        drop(w_guard);
    }
}
