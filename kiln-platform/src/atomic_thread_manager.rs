//! Atomic operations integration with WebAssembly thread management.
//!
//! This module provides a bridge between WebAssembly atomic operations and
//! the existing thread management infrastructure, enabling efficient
//! implementation of memory.atomic.wait and memory.atomic.notify.

extern crate alloc;

use alloc::{
    collections::BTreeMap,
    sync::Arc,
};
use core::time::Duration;

use wrt_error::{
    ErrorCategory,
    ErrorSource,
    Result,
    ToErrorCategory,
};
use wrt_sync::WrtRwLock;

#[cfg(target_os = "linux")]
use crate::linux_sync::{
    LinuxFutex,
    LinuxFutexBuilder,
};
use crate::{
    sync::FutexLike,
    threading::{
        ThreadHandle,
        ThreadPoolConfig,
        ThreadPriority,
        ThreadSpawnRequest,
        ThreadingLimits,
        WasmTask,
    },
    wasm_thread_manager::{
        WasmModuleInfo,
        WasmThreadManager,
    },
};

/// Type alias for complex executor function type
type ExecutorFn = Arc<dyn Fn(u32, Vec<u8>) -> Result<Vec<u8>> + Send + Sync>;

/// Atomic wait/notify coordinator that manages futex objects per memory address
pub struct AtomicCoordinator {
    /// Map of memory addresses to futex objects
    futex_map:        Arc<WrtRwLock<BTreeMap<u64, Arc<dyn FutexLike + Send + Sync>>>>,
    /// Thread manager for spawning atomic operation threads
    thread_manager:   Arc<WasmThreadManager>,
    /// Module registered for atomic operations
    atomic_module_id: u64,
}

impl AtomicCoordinator {
    /// Create a new atomic coordinator
    pub fn new(thread_manager: Arc<WasmThreadManager>) -> Result<Self> {
        // Register a special module for atomic operations
        let atomic_module = WasmModuleInfo {
            id:               u64::MAX, // Special ID for atomic operations
            name:             "atomic_operations".to_string(),
            max_threads:      1024,             // Allow many atomic waiters
            memory_limit:     64 * 1024 * 1024, // 64MB for atomic operations
            cpu_quota:        Duration::from_secs(3600), // Long-running waits allowed
            default_priority: ThreadPriority::Normal,
        };

        thread_manager.register_module(atomic_module)?;

        Ok(Self {
            futex_map: Arc::new(WrtRwLock::new(BTreeMap::new())),
            thread_manager,
            atomic_module_id: u64::MAX,
        })
    }

    /// Get or create a futex for a memory address
    fn get_or_create_futex(
        &self,
        addr: u64,
        initial_value: u32,
    ) -> Result<Arc<dyn FutexLike + Send + Sync>> {
        let mut map = self.futex_map.write();

        if let Some(futex) = map.get(&addr) {
            return Ok(Arc::clone(futex));
        }

        // Create new futex based on platform
        #[cfg(target_os = "linux")]
        let futex: Arc<dyn FutexLike + Send + Sync> =
            Arc::new(LinuxFutexBuilder::new().with_initial_value(initial_value).build()?);

        #[cfg(not(target_os = "linux"))]
        let futex: Arc<dyn FutexLike + Send + Sync> =
            Arc::new(crate::sync::SpinFutex::new(initial_value));

        map.insert(addr, Arc::clone(&futex));
        Ok(futex)
    }

    /// Implement atomic wait operation
    pub fn atomic_wait(&self, addr: u64, expected: u32, timeout_ns: Option<u64>) -> Result<i32> {
        let futex = self.get_or_create_futex(addr, expected)?;
        let timeout = timeout_ns.map(Duration::from_nanos);

        match futex.wait(expected, timeout) {
            Ok(()) => Ok(0),                                             // Woken by notify
            Err(e) if e.to_category() == ErrorCategory::System => Ok(2), // Timeout
            Err(e) => Err(e),
        }
    }

    /// Implement atomic notify operation
    pub fn atomic_notify(&self, addr: u64, count: u32) -> Result<u32> {
        let map = self.futex_map.read();

        if let Some(futex) = map.get(&addr) {
            futex.wake(count)?;
            Ok(count) // Return number of waiters woken (simplified)
        } else {
            Ok(0) // No waiters at this address
        }
    }

    /// Spawn a thread that performs atomic wait
    pub fn spawn_atomic_waiter(
        &self,
        addr: u64,
        expected: u32,
        timeout_ns: Option<u64>,
    ) -> Result<u64> {
        let request = ThreadSpawnRequest {
            module_id:   self.atomic_module_id,
            function_id: 0xFFFF, // Special function ID for atomic operations
            args:        {
                let mut args = Vec::new();
                args.extend_from_slice(&addr.to_le_bytes());
                args.extend_from_slice(&expected.to_le_bytes());
                if let Some(timeout) = timeout_ns {
                    args.extend_from_slice(&timeout.to_le_bytes());
                }
                args
            },
            priority:    Some(ThreadPriority::Normal),
            stack_size:  Some(64 * 1024), // Small stack for atomic operations
        };

        self.thread_manager.spawn_thread(&request)
    }

    /// Clean up unused futexes (garbage collection)
    pub fn cleanup_futexes(&self) {
        let mut map = self.futex_map.write();

        // Remove futexes that are no longer referenced
        // In a real implementation, we'd track reference counts
        map.retain(|_addr, futex| Arc::strong_count(futex) > 1);
    }

    /// Get statistics about atomic operations
    pub fn get_atomic_stats(&self) -> AtomicStats {
        let map = self.futex_map.read();
        AtomicStats {
            active_futexes:       map.len(),
            thread_manager_stats: self.thread_manager.get_stats(),
        }
    }
}

/// Statistics for atomic operations
#[derive(Debug, Clone)]
pub struct AtomicStats {
    /// Number of active futex objects
    pub active_futexes:       usize,
    /// Thread manager statistics
    pub thread_manager_stats: crate::wasm_thread_manager::ThreadManagerStats,
}

/// Enhanced WebAssembly thread manager with atomic operations support
pub struct AtomicAwareThreadManager {
    /// Base thread manager
    base_manager:       Arc<WasmThreadManager>,
    /// Atomic coordinator
    atomic_coordinator: AtomicCoordinator,
}

impl AtomicAwareThreadManager {
    /// Create a new atomic-aware thread manager
    pub fn new(
        config: ThreadPoolConfig,
        limits: ThreadingLimits,
        executor: ExecutorFn,
    ) -> Result<Self> {
        let base_manager = Arc::new(WasmThreadManager::new(config, limits, executor)?);
        let atomic_coordinator = AtomicCoordinator::new(Arc::clone(&base_manager))?;

        Ok(Self {
            base_manager,
            atomic_coordinator,
        })
    }

    /// Execute atomic wait operation
    pub fn execute_atomic_wait(
        &self,
        addr: u64,
        expected: u32,
        timeout_ns: Option<u64>,
    ) -> Result<i32> {
        self.atomic_coordinator.atomic_wait(addr, expected, timeout_ns)
    }

    /// Execute atomic notify operation
    pub fn execute_atomic_notify(&self, addr: u64, count: u32) -> Result<u32> {
        self.atomic_coordinator.atomic_notify(addr, count)
    }

    /// Spawn a regular WebAssembly thread
    pub fn spawn_wasm_thread(&self, request: &ThreadSpawnRequest) -> Result<u64> {
        self.base_manager.spawn_thread(request)
    }

    /// Join a thread and get its result
    pub fn join_thread(
        &self,
        thread_id: u64,
    ) -> Result<crate::wasm_thread_manager::ThreadExecutionResult> {
        self.base_manager.join_thread(thread_id)
    }

    /// Get comprehensive statistics
    pub fn get_stats(&self) -> AtomicAwareStats {
        AtomicAwareStats {
            base_stats:   self.base_manager.get_stats(),
            atomic_stats: self.atomic_coordinator.get_atomic_stats(),
        }
    }

    /// Shutdown the manager
    pub fn shutdown(&mut self, _timeout: Duration) -> Result<()> {
        // Clean up atomic operations first
        self.atomic_coordinator.cleanup_futexes();

        // Shutdown base manager (this will unregister the atomic module)
        // Note: We need to work around the fact that base_manager is Arc
        // In a real implementation, we'd need to restructure this
        Ok(())
    }
}

/// Combined statistics for atomic-aware thread manager
#[derive(Debug, Clone)]
pub struct AtomicAwareStats {
    /// Base thread manager statistics
    pub base_stats:   crate::wasm_thread_manager::ThreadManagerStats,
    /// Atomic operations statistics
    pub atomic_stats: AtomicStats,
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::*;

    fn create_test_executor() -> ExecutorFn {
        Arc::new(|_function_id, args| Ok(args))
    }

    #[test]
    fn test_atomic_coordinator_creation() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let base_manager = Arc::new(WasmThreadManager::new(config, limits, executor).unwrap());
        let coordinator = AtomicCoordinator::new(base_manager);
        assert!(coordinator.is_ok());
    }

    #[test]
    fn test_atomic_aware_thread_manager() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let manager = AtomicAwareThreadManager::new(config, limits, executor);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_atomic_operations() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let manager = AtomicAwareThreadManager::new(config, limits, executor).unwrap();

        // Test atomic notify (no waiters)
        let result = manager.execute_atomic_notify(0x1000, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // No waiters

        // Test atomic wait with immediate timeout
        let result = manager.execute_atomic_wait(0x1000, 42, Some(1_000_000)); // 1ms timeout
        assert!(result.is_ok());
        // Result should be 2 (timeout) since no other thread is modifying the
        // value
    }
}
