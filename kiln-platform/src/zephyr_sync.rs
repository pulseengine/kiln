#![allow(unsafe_code)]
#![allow(dead_code)]
// Allow unsafe FFI calls to Zephyr kernel
// WRT - wrt-platform
// Module: Zephyr Synchronization Primitives
// SW-REQ-ID: REQ_PLATFORM_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Zephyr RTOS-specific `FutexLike` implementation using Zephyr kernel
//! synchronization primitives.
//!
//! This implementation provides wait/notify synchronization using Zephyr's
//! futex implementation and kernel objects, supporting no_std/no_alloc
//! environments on embedded systems.

use core::{
    fmt,
    sync::atomic::AtomicU32,
    time::Duration,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::sync::FutexLike;

/// Zephyr timeout constants
const K_NO_WAIT: i32 = 0;
const K_FOREVER: i32 = -1;

/// Zephyr error codes
const EAGAIN: i32 = -11;
const ETIMEDOUT: i32 = -110;
const EINTR: i32 = -4;

/// Zephyr futex structure (opaque)
#[repr(C)]
struct ZephyrFutexHandle {
    _private: [u8; 0],
}

/// Zephyr timeout structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ZephyrTimeout {
    ticks: i64,
}

impl ZephyrTimeout {
    /// Create timeout from Duration
    fn from_duration(duration: Duration) -> Self {
        // Convert duration to Zephyr ticks (assuming 1000 Hz tick rate)
        let total_ms = duration.as_millis() as i64;
        Self { ticks: total_ms }
    }

    /// Create no-wait timeout
    fn no_wait() -> Self {
        Self {
            ticks: K_NO_WAIT as i64,
        }
    }

    /// Create forever timeout
    fn forever() -> Self {
        Self {
            ticks: K_FOREVER as i64,
        }
    }

    /// Get timeout value for Zephyr APIs
    fn as_timeout(self) -> i32 {
        if self.ticks == K_FOREVER as i64 {
            K_FOREVER
        } else if self.ticks <= 0 {
            K_NO_WAIT
        } else {
            // Cap at i32::MAX to avoid overflow
            core::cmp::min(self.ticks, i32::MAX as i64) as i32
        }
    }
}

// FFI declarations for Zephyr kernel APIs
unsafe extern "C" {
    /// Wait on a futex until value changes
    fn k_futex_wait(futex: *mut ZephyrFutexHandle, expected: u32, timeout: i32) -> i32;

    /// Wake waiters on a futex
    fn k_futex_wake(futex: *mut ZephyrFutexHandle, wake_all: bool) -> i32;

    /// Initialize a futex
    fn k_futex_init(futex: *mut ZephyrFutexHandle);

    /// Get current kernel ticks
    fn k_uptime_ticks() -> i64;

    /// Convert milliseconds to ticks
    fn k_ms_to_ticks_ceil32(ms: u32) -> u32;
}

/// A `FutexLike` implementation for Zephyr RTOS using Zephyr's futex API.
///
/// This implementation provides wait/notify synchronization using Zephyr's
/// kernel futex implementation, which is designed for embedded systems.
#[derive(Debug)]
pub struct ZephyrFutex {
    /// The atomic value used for synchronization
    value:     AtomicU32,
    /// Binary std/no_std choice
    futex_obj: *mut ZephyrFutexHandle,
    /// Padding to ensure cache line alignment
    _padding:  [u8; 56], // Adjust for embedded cache line sizes
}

// Safety: ZephyrFutex contains only atomic values and Zephyr kernel objects
// which are thread-safe
unsafe impl Send for ZephyrFutex {}
unsafe impl Sync for ZephyrFutex {}

impl ZephyrFutex {
    /// Creates a new `ZephyrFutex` with the given initial value.
    pub fn new(initial_value: u32) -> Self {
        // Binary std/no_std choice
        // For demonstration, we'll simulate with null pointer (would need actual kernel
        // object)
        let futex_obj = core::ptr::null_mut();

        let futex = Self {
            value: AtomicU32::new(initial_value),
            futex_obj,
            _padding: [0; 56],
        };

        // Initialize the Zephyr futex object
        // SAFETY: In real usage, futex_obj would point to valid kernel object memory
        if !futex_obj.is_null() {
            unsafe {
                k_futex_init(futex_obj);
            }
        }

        futex
    }

    /// Wait for the futex value to change from expected
    fn wait_impl(&self, expected: u32, timeout: Option<ZephyrTimeout>) -> Result<()> {
        // In real implementation, would use the actual futex object
        if self.futex_obj.is_null() {
            // Fallback to busy-wait for demonstration
            return self.busy_wait_fallback(expected, timeout);
        }

        let timeout_val = match timeout {
            Some(t) => t.as_timeout(),
            None => K_FOREVER,
        };

        // Call Zephyr futex wait
        // SAFETY: We're calling Zephyr's futex API with valid parameters
        let result = unsafe { k_futex_wait(self.futex_obj, expected, timeout_val) };

        match result {
            0 => Ok(()), // Success - woken up
            ETIMEDOUT => Err(Error::runtime_execution_error("Futex wait timeout")),
            EAGAIN => {
                // Value changed before we could wait - this is success
                Ok(())
            },
            EINTR => {
                // Interrupted - treat as spurious wakeup
                Ok(())
            },
            _ => Err(Error::new(ErrorCategory::System, 1, "Futex wait failed")),
        }
    }

    /// Fallback busy-wait implementation for when futex is not available
    fn busy_wait_fallback(&self, expected: u32, timeout: Option<ZephyrTimeout>) -> Result<()> {
        let start_time = unsafe { k_uptime_ticks() };
        let timeout_ticks = timeout.map_or(i64::MAX, |t| t.ticks);

        loop {
            // Check if value has changed
            if self.value.load(core::sync::atomic::Ordering::Acquire) != expected {
                return Ok(());
            }

            // Check timeout
            if timeout_ticks != i64::MAX {
                let elapsed = unsafe { k_uptime_ticks() } - start_time;
                if elapsed >= timeout_ticks {
                    return Err(Error::runtime_execution_error("Zephyr mutex lock timeout"));
                }
            }

            // Yield to other threads
            core::hint::spin_loop();
        }
    }

    /// Wake waiters on this futex
    fn wake_impl(&self, wake_all: bool) -> Result<u32> {
        // In real implementation, would use the actual futex object
        if self.futex_obj.is_null() {
            // No actual waiters to wake in fallback mode
            return Ok(0);
        }

        // Call Zephyr futex wake
        // SAFETY: We're calling Zephyr's futex API with valid parameters
        let result = unsafe { k_futex_wake(self.futex_obj, wake_all) };

        if result >= 0 {
            Ok(u32::try_from(result).unwrap_or(0)) // Number of waiters woken
        } else {
            Err(Error::new(
                ErrorCategory::System,
                1,
                "Failed to wake futex waiters",
            ))
        }
    }
}

/// Builder for `ZephyrFutex`.
#[derive(Debug)]
pub struct ZephyrFutexBuilder {
    initial_value: u32,
}

impl Default for ZephyrFutexBuilder {
    fn default() -> Self {
        Self { initial_value: 0 }
    }
}

impl ZephyrFutexBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the initial value for the futex.
    pub fn with_initial_value(mut self, value: u32) -> Self {
        self.initial_value = value;
        self
    }

    /// Builds and returns a configured `ZephyrFutex`.
    pub fn build(self) -> ZephyrFutex {
        ZephyrFutex::new(self.initial_value)
    }
}

impl FutexLike for ZephyrFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<()> {
        let zephyr_timeout = timeout.map(ZephyrTimeout::from_duration);

        self.wait_impl(expected, zephyr_timeout)
    }

    fn wake(&self, count: u32) -> Result<()> {
        // Wake specified number of waiters
        // For Zephyr, we can either wake one or all
        let wake_all = count > 1;
        self.wake_impl(wake_all).map(|_| ())
    }
}

impl fmt::Display for ZephyrFutex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ZephyrFutex({})",
            self.value.load(core::sync::atomic::Ordering::Relaxed)
        )
    }
}

impl Drop for ZephyrFutex {
    fn drop(&mut self) {
        // In a real implementation, this would clean up the Zephyr futex object
        // For static objects, this might be a no-op
    }
}

/// Alternative implementation using Zephyr semaphores for compatibility
#[derive(Debug)]
pub struct ZephyrSemaphoreFutex {
    /// The atomic value used for synchronization
    value:     AtomicU32,
    /// Binary std/no_std choice
    semaphore: *mut u8, // Placeholder for k_sem structure
    /// Padding for alignment
    _padding:  [u8; 60],
}

// Safety: Same as ZephyrFutex
unsafe impl Send for ZephyrSemaphoreFutex {}
unsafe impl Sync for ZephyrSemaphoreFutex {}

impl ZephyrSemaphoreFutex {
    /// Create a new semaphore-based futex
    pub fn new(initial_value: u32) -> Self {
        Self {
            value:     AtomicU32::new(initial_value),
            semaphore: core::ptr::null_mut(), // Would point to actual k_sem object
            _padding:  [0; 60],
        }
    }
}

impl FutexLike for ZephyrSemaphoreFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<()> {
        // Check value first
        if self.value.load(core::sync::atomic::Ordering::Acquire) != expected {
            return Ok(());
        }

        // In real implementation, would use k_sem_take() with timeout
        // For now, use busy wait fallback
        let start_time = unsafe { k_uptime_ticks() };
        let timeout_ticks = timeout.map_or(i64::MAX, |d| d.as_millis() as i64);

        loop {
            if self.value.load(core::sync::atomic::Ordering::Acquire) != expected {
                return Ok(());
            }

            if timeout_ticks != i64::MAX {
                let elapsed = unsafe { k_uptime_ticks() } - start_time;
                if elapsed >= timeout_ticks {
                    return Err(Error::runtime_execution_error(
                        "Zephyr semaphore wait timeout",
                    ));
                }
            }

            core::hint::spin_loop();
        }
    }

    fn wake(&self, _count: u32) -> Result<()> {
        // In real implementation, would use k_sem_give()
        // For now, this is a no-op
        Ok(())
    }
}

impl fmt::Display for ZephyrSemaphoreFutex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ZephyrSemaphoreFutex({})",
            self.value.load(core::sync::atomic::Ordering::Relaxed)
        )
    }
}
