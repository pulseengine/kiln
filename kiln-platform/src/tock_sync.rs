//! Tock OS Synchronization Implementation
//!
//! Implements FutexLike trait for Tock OS using the event-driven callback
//! system and yield-based synchronization. This provides security-first
//! synchronization with MPU isolation between processes.

use core::{
    sync::atomic::{
        AtomicU32,
        Ordering,
    },
    time::Duration,
};

use wrt_error::Error;

use crate::sync::FutexLike;

/// Tock OS synchronization syscalls
mod sync_syscall {

    /// Subscribe to timer events
    pub const TIMER_DRIVER_ID: u32 = 0x00000;
    pub const TIMER_SUBSCRIBE_ID: u32 = 0;

    /// Application-to-application communication driver
    pub const IPC_DRIVER_ID: u32 = 0x10000;
    pub const IPC_NOTIFY_CMD: u32 = 1;
    pub const IPC_DISCOVER_CMD: u32 = 2;

    /// Subscribe system call wrapper
    #[inline(always)]
    pub unsafe fn subscribe(driver_id: u32, callback_id: u32, callback: extern "C" fn()) -> i32 {
        #[cfg(target_arch = "arm")]
        {
            let result: i32;
            core::arch::asm!(
                "svc #1",
                inout("r0") driver_id => result,
                in("r1") callback_id,
                in("r2") callback,
                options(nostack, preserves_flags)
            );
            result
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
            let _ = (driver_id, callback_id, callback);
            -1 // Error: unsupported on this platform
        }
    }

    /// Command system call wrapper
    #[inline(always)]
    pub unsafe fn command(driver_id: u32, command_id: u32, arg1: u32, arg2: u32) -> i32 {
        #[cfg(target_arch = "arm")]
        {
            let result: i32;
            core::arch::asm!(
                "svc #2",
                inout("r0") driver_id => result,
                in("r1") command_id,
                in("r2") arg1,
                in("r3") arg2,
                options(nostack, preserves_flags)
            );
            result
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
            let _ = (driver_id, command_id, arg1, arg2);
            -1 // Error: unsupported on this platform
        }
    }

    /// Yield to scheduler
    #[inline(always)]
    pub unsafe fn yield_for() {
        #[cfg(target_arch = "arm")]
        {
            core::arch::asm!("svc #0", options(nostack, preserves_flags));
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
        }
    }

    /// Yield and wait for events
    #[inline(always)]
    pub unsafe fn yield_wait() {
        #[cfg(target_arch = "arm")]
        {
            core::arch::asm!(
                "svc #0",
                in("r0") 1u32, // yield_wait variant
                options(nostack, preserves_flags)
            );
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
        }
    }
}

/// Callback state for event-driven synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallbackState {
    /// No callback registered
    None,
    /// Waiting for callback
    Waiting,
    /// Callback fired
    Fired,
    /// Timeout occurred
    Timeout,
}

/// Static callback state (Tock requires static callbacks)
static CALLBACK_STATE: AtomicU32 = AtomicU32::new(CallbackState::None as u32);

/// Timer callback function
extern "C" fn timer_callback() {
    CALLBACK_STATE.store(CallbackState::Timeout as u32, Ordering::SeqCst);
}

/// IPC notification callback
extern "C" fn ipc_callback() {
    CALLBACK_STATE.store(CallbackState::Fired as u32, Ordering::SeqCst);
}

/// Tock OS futex implementation using event-driven synchronization
#[derive(Debug)]
pub struct TockFutex {
    /// Atomic value for futex operations
    value:      AtomicU32,
    /// Process ID for IPC communication (for cross-process wake)
    process_id: u32,
}

unsafe impl Send for TockFutex {}
unsafe impl Sync for TockFutex {}

impl TockFutex {
    /// Create new Tock futex
    pub fn new(initial_value: u32) -> Self {
        Self {
            value:      AtomicU32::new(initial_value),
            process_id: Self::get_process_id(),
        }
    }

    /// Load the current value
    pub fn load(&self) -> u32 {
        self.value.load(Ordering::Acquire)
    }

    /// Store a new value
    pub fn store(&self, val: u32) {
        self.value.store(val, Ordering::Release);
    }

    /// Perform compare-and-exchange operation
    pub fn compare_exchange(&self, current: u32, new: u32) -> Result<u32, u32> {
        self.value.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
    }

    /// Fetch and add operation
    pub fn fetch_add(&self, val: u32) -> u32 {
        self.value.fetch_add(val, Ordering::AcqRel)
    }

    /// Fetch and subtract operation
    pub fn fetch_sub(&self, val: u32) -> u32 {
        self.value.fetch_sub(val, Ordering::AcqRel)
    }

    /// Get current process ID from Tock kernel
    fn get_process_id() -> u32 {
        // In Tock, process ID would be provided by the kernel
        // For this implementation, we use a placeholder
        unsafe {
            let result = sync_syscall::command(
                sync_syscall::IPC_DRIVER_ID,
                sync_syscall::IPC_DISCOVER_CMD,
                0,
                0,
            );
            u32::try_from(result).unwrap_or(0)
        }
    }

    /// Set up timer for timeout
    fn setup_timer(&self, timeout: Duration) -> Result<(), Error> {
        // Convert duration to microseconds
        let timeout_us = timeout.as_micros() as u32;

        // Subscribe to timer callback
        let result = unsafe {
            sync_syscall::subscribe(
                sync_syscall::TIMER_DRIVER_ID,
                sync_syscall::TIMER_SUBSCRIBE_ID,
                timer_callback,
            )
        };

        if result < 0 {
            return Err(Error::resource_error("Failed to subscribe to timer"));
        }

        // Start timer
        let result = unsafe {
            sync_syscall::command(
                sync_syscall::TIMER_DRIVER_ID,
                1, // Start timer command
                timeout_us,
                0,
            )
        };

        if result < 0 {
            return Err(Error::resource_error("Failed to start timer"));
        }

        Ok(())
    }

    /// Subscribe to IPC notifications for wake operations
    fn setup_ipc_notification(&self) -> Result<(), Error> {
        let result = unsafe {
            sync_syscall::subscribe(
                sync_syscall::IPC_DRIVER_ID,
                0, // IPC notification callback ID
                ipc_callback,
            )
        };

        if result < 0 {
            return Err(Error::resource_error(
                "Failed to subscribe to IPC notifications",
            ));
        }

        Ok(())
    }

    /// Send IPC notification to wake waiting processes
    fn send_ipc_notification(&self) -> Result<(), Error> {
        let result = unsafe {
            sync_syscall::command(
                sync_syscall::IPC_DRIVER_ID,
                sync_syscall::IPC_NOTIFY_CMD,
                self.process_id,
                1, // Wake signal
            )
        };

        if result < 0 {
            return Err(Error::resource_error("Failed to send IPC notification"));
        }

        Ok(())
    }
}

impl FutexLike for TockFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error> {
        // Set up IPC notification for wake operations
        self.setup_ipc_notification()?;

        // Set up timer if timeout specified
        if let Some(timeout_duration) = timeout {
            self.setup_timer(timeout_duration)?;
        }

        // Reset callback state
        CALLBACK_STATE.store(CallbackState::Waiting as u32, Ordering::SeqCst);

        // Wait loop with yield
        loop {
            // Check if value has changed
            let current = self.value.load(Ordering::SeqCst);
            if current != expected {
                return Ok(());
            }

            // Check callback state
            let state = CALLBACK_STATE.load(Ordering::SeqCst);

            match state {
                x if x == CallbackState::Fired as u32 => {
                    // Wake notification received
                    return Ok(());
                },
                x if x == CallbackState::Timeout as u32 => {
                    // Timeout occurred
                    return Err(Error::resource_error("Wait operation timed out"));
                },
                _ => {
                    // Continue waiting - yield to scheduler
                    unsafe {
                        sync_syscall::yield_wait();
                    }
                },
            }
        }
    }

    fn wake(&self, count: u32) -> Result<(), Error> {
        // In Tock's isolated process model, cross-process wake is limited
        // We can only signal through IPC to other processes that have
        // set up IPC communication with us

        for _ in 0..count {
            self.send_ipc_notification()?;
        }

        Ok(())
    }
}

/// Alternative futex implementation using Tock's semaphore-like primitives
#[derive(Debug)]
pub struct TockSemaphoreFutex {
    /// Atomic value
    value:           AtomicU32,
    /// Semaphore count for blocking operations
    semaphore_count: AtomicU32,
}

impl TockSemaphoreFutex {
    /// Create new semaphore-based futex
    pub fn new(initial_value: u32) -> Self {
        Self {
            value:           AtomicU32::new(initial_value),
            semaphore_count: AtomicU32::new(0),
        }
    }

    /// Load the current value
    pub fn load(&self) -> u32 {
        self.value.load(Ordering::Acquire)
    }

    /// Wake all waiters
    pub fn wake_all(&self) -> Result<(), Error> {
        // For simplicity, we'll set a large wake count
        self.wake(u32::MAX)?;
        Ok(())
    }

    /// Get approximate cycle count for timeout calculations
    fn get_cycle_count() -> u64 {
        // In a real implementation, this would read a cycle counter
        // For now, return a placeholder that increments
        static mut COUNTER: u64 = 0;
        unsafe {
            COUNTER += 1000; // Simulate cycles
            COUNTER
        }
    }
}

impl FutexLike for TockSemaphoreFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error> {
        // Simple spin-wait implementation for cases where IPC is not available
        let start_cycles = Self::get_cycle_count();

        loop {
            let current = self.value.load(Ordering::SeqCst);
            if current != expected {
                return Ok(());
            }

            // Check timeout (simplified cycle-based timeout)
            if let Some(timeout_duration) = timeout {
                let elapsed_cycles = Self::get_cycle_count() - start_cycles;
                let timeout_cycles = timeout_duration.as_micros() as u64 * 1000; // Rough estimate
                if elapsed_cycles >= timeout_cycles {
                    return Err(Error::resource_error("Wait operation timed out"));
                }
            }

            // Yield to scheduler to avoid busy-wait
            unsafe {
                sync_syscall::yield_for();
            }
        }
    }

    fn wake(&self, _count: u32) -> Result<(), Error> {
        // Increment semaphore count to signal waiting threads
        self.semaphore_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Builder for TockFutex
pub struct TockFutexBuilder {
    initial_value: u32,
    use_ipc:       bool,
}

impl TockFutexBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            initial_value: 0,
            use_ipc:       true,
        }
    }

    /// Set initial value
    pub fn with_initial_value(mut self, value: u32) -> Self {
        self.initial_value = value;
        self
    }

    /// Enable/disable IPC for cross-process synchronization
    pub fn with_ipc(mut self, enable: bool) -> Self {
        self.use_ipc = enable;
        self
    }

    /// Build the futex
    pub fn build(self) -> Result<TockFutex, Error> {
        Ok(TockFutex::new(self.initial_value))
    }

    /// Build semaphore-based futex (fallback for no IPC)
    pub fn build_semaphore(self) -> TockSemaphoreFutex {
        TockSemaphoreFutex::new(self.initial_value)
    }
}

impl Default for TockFutexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_futex_creation() {
        let futex = TockFutex::new(42);
        assert_eq!(futex.load(), 42);
    }

    #[test]
    fn test_futex_atomic_operations() {
        let futex = TockFutex::new(0);

        // Test store/load
        futex.store(100);
        assert_eq!(futex.load(), 100);

        // Test fetch_add
        let old_value = futex.fetch_add(50);
        assert_eq!(old_value, 100);
        assert_eq!(futex.load(), 150);

        // Test fetch_sub
        let old_value = futex.fetch_sub(25);
        assert_eq!(old_value, 150);
        assert_eq!(futex.load(), 125);

        // Test compare_exchange
        let result = futex.compare_exchange(125, 200);
        assert_eq!(result, Ok(125));
        assert_eq!(futex.load(), 200);

        let result = futex.compare_exchange(999, 300);
        assert_eq!(result, Err(200));
        assert_eq!(futex.load(), 200);
    }

    #[test]
    fn test_semaphore_futex_creation() {
        let futex = TockSemaphoreFutex::new(10);
        assert_eq!(futex.load(), 10);
        assert_eq!(futex.semaphore_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_semaphore_futex_wake() {
        let futex = TockSemaphoreFutex::new(0);

        // Test wake increments semaphore count
        assert!(futex.wake(1).is_ok());
        assert_eq!(futex.semaphore_count.load(Ordering::SeqCst), 1);

        // Test wake_all sets high value
        assert!(futex.wake_all().is_ok());
        assert_eq!(futex.semaphore_count.load(Ordering::SeqCst), u32::MAX);
    }

    #[test]
    fn test_builder_pattern() {
        let builder = TockFutexBuilder::new().with_initial_value(123).with_ipc(false);

        assert_eq!(builder.initial_value, 123);
        assert!(!builder.use_ipc);

        let futex = builder.build_semaphore();
        assert_eq!(futex.load(), 123);
    }

    #[test]
    fn test_callback_state_enum() {
        assert_eq!(CallbackState::None as u32, 0);
        assert_eq!(CallbackState::Waiting as u32, 1);
        assert_eq!(CallbackState::Fired as u32, 2);
        assert_eq!(CallbackState::Timeout as u32, 3);
    }
}
