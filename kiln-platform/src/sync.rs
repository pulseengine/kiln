// WRT - wrt-platform
// Module: Platform Synchronization Primitives
// SW-REQ-ID: REQ_PLATFORM_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides traits and implementations for platform-specific synchronization.

extern crate alloc;

// Binary std/no_std choice
#[cfg(feature = "std")]
pub use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
pub use alloc::sync::Arc;
use core::fmt::Debug;
// Re-export atomic types for platform use
pub use core::sync::atomic::{
    AtomicU32,
    AtomicU64,
    AtomicUsize,
    Ordering,
};
// Re-export Duration for platform use
pub use core::time::Duration;
#[cfg(feature = "std")]
pub use std::sync::{
    Condvar,
    Mutex,
    MutexGuard,
    RwLock,
};

#[cfg(not(feature = "std"))]
pub use wrt_sync::{
    WrtMutex as Mutex,
    WrtMutexGuard as MutexGuard,
    WrtRwLock as RwLock,
};

use crate::prelude::Result;

/// Provide a simple Condvar alternative for non-std builds
///
/// This is a minimal implementation that provides the Condvar API
/// but returns errors for operations that require std functionality.
#[cfg(not(feature = "std"))]
pub struct Condvar;

#[cfg(not(feature = "std"))]
impl Condvar {
    /// Create a new condition variable
    pub fn new() -> Self {
        Self
    }

    /// Wait on the condition variable (not supported in no_std)
    pub fn wait<'a, T>(&self, _guard: MutexGuard<'a, T>) -> Result<MutexGuard<'a, T>> {
        Err(wrt_error::Error::runtime_not_implemented(
            "Condvar not supported in no_std",
        ))
    }

    /// Notify one waiting thread (no-op in no_std)
    pub fn notify_one(&self) {}

    /// Notify all waiting threads (no-op in no_std)
    pub fn notify_all(&self) {}
}

/// A trait abstracting futex-like operations.
///
/// This trait provides a minimal set of operations similar to those offered by
/// futexes on Linux, allowing for thread synchronization based on an atomic U32
/// value.
pub trait FutexLike: Send + Sync + Debug {
    /// Atomically checks if the futex value is `expected`, and if so, sleeps
    /// until woken by `wake` or until a timeout occurs.
    ///
    /// # Arguments
    ///
    /// * `expected`: The value the futex is expected to hold.
    /// * `timeout`: An optional duration to wait. If `None`, waits
    ///   indefinitely.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the wait was successful (either woken or value changed).
    ///
    /// # Errors
    ///
    /// * Returns `Err(Error::Concurrency(...))` if a lock was poisoned.
    /// * Returns `Err(Error::System(...))` if a timeout occurred.
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<()>;

    /// Wakes `count` waiters blocked on this futex.
    ///
    /// # Arguments
    ///
    /// * `count`: The number of waiters to wake. `u32::MAX` can be used to wake
    ///   all.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the wake signal was sent.
    ///
    /// # Errors
    ///
    /// * Returns `Err(Error::Concurrency(...))` if a lock was poisoned
    ///   (relevant for fallback).
    fn wake(&self, count: u32) -> Result<()>;
}

/// `Result` type for timeout-based operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutResult {
    /// The operation completed before the timeout expired.
    Notified,
    /// The operation timed out.
    TimedOut,
}

/// A simple spin-wait implementation of the `FutexLike` trait.
///
/// This implementation is not as efficient as platform-specific implementations
/// for real-world use, but it works on any platform including embedded systems.
/// It's intended as a fallback when more efficient implementations are not
/// available.
#[derive(Debug)]
pub struct SpinFutex {
    /// The atomic value used for synchronization
    value:    core::sync::atomic::AtomicU32,
    /// Padding to ensure the value is on its own cache line
    _padding: [u8; 60], // 64 - sizeof(AtomicU32)
}

impl SpinFutex {
    /// Creates a new `SpinFutex` with the given initial value.
    pub fn new(initial_value: u32) -> Self {
        Self {
            value:    core::sync::atomic::AtomicU32::new(initial_value),
            _padding: [0; 60],
        }
    }

    /// Gets the current value.
    pub fn get(&self) -> u32 {
        self.value.load(core::sync::atomic::Ordering::Acquire)
    }

    /// Sets a new value.
    pub fn set(&self, new_value: u32) {
        self.value.store(new_value, core::sync::atomic::Ordering::Release);
    }
}

/// Builder for `SpinFutex`.
#[derive(Debug)]
pub struct SpinFutexBuilder {
    initial_value: u32,
}

impl Default for SpinFutexBuilder {
    fn default() -> Self {
        Self { initial_value: 0 }
    }
}

impl SpinFutexBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the initial value for the futex.
    pub fn with_initial_value(mut self, value: u32) -> Self {
        self.initial_value = value;
        self
    }

    /// Builds and returns a configured `SpinFutex`.
    pub fn build(self) -> SpinFutex {
        SpinFutex::new(self.initial_value)
    }
}

impl FutexLike for SpinFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<()> {
        use core::sync::atomic::Ordering;

        // Check if the current value matches the expected value
        if self.value.load(Ordering::Acquire) != expected {
            // Value has changed, no need to wait
            return Ok(());
        }

        // Calculate timeout in iterations (very rough estimation)
        let max_iterations = match timeout {
            Some(duration) if duration.as_micros() == 0 => 1, // Just check once
            Some(duration) => {
                // Rough estimate: 1000 iterations ~= 1ms on a typical system
                // This is obviously not accurate but gives us some scaling
                (duration.as_micros() as u64) * 1000 / 1000
            },
            None => u64::MAX, // Effectively infinite
        };

        // Spin until value changes or timeout
        for _ in 0..max_iterations {
            // Small delay to reduce CPU usage
            for _ in 0..100 {
                core::hint::spin_loop();
            }

            // Check if value has changed
            if self.value.load(Ordering::Acquire) != expected {
                return Ok(());
            }
        }

        // If we get here, we've timed out
        if timeout.is_some() {
            // Return a system error for timeout
            Err(wrt_error::Error::system_error("Operation timed out"))
        } else {
            // Should never reach here with infinite timeout
            Ok(())
        }
    }

    fn wake(&self, _count: u32) -> Result<()> {
        // For a spin-wait implementation, we don't need to do anything
        // except ensure memory visibility
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use core::time::Duration;

    use wrt_error::{
        ErrorCategory,
        ErrorSource,
    };

    use super::*;

    #[test]
    fn test_spin_futex() {
        let futex = SpinFutex::new(42);

        // Test get and set
        assert_eq!(futex.get(), 42);
        futex.set(123);
        assert_eq!(futex.get(), 123);

        // Test wait and wake
        futex.set(0);
        // Value is 0, expected is 1, should return immediately
        let result = futex.wait(1, Some(Duration::from_micros(0)));
        assert!(result.is_ok());

        // Value is 0, expected is 0, should wait until timeout
        let result = futex.wait(0, Some(Duration::from_micros(1)));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category(), ErrorCategory::System);

        // Test wake (doesn't do much in SpinFutex but should not fail)
        futex.wake(1).expect("Wake should succeed");
        futex.wake(u32::MAX).expect("Wake all should succeed");
    }

    #[test]
    fn test_spin_futex_builder() {
        let futex = SpinFutexBuilder::new().with_initial_value(42).build();

        assert_eq!(futex.get(), 42);
    }
}
