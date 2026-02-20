//! Generic thread pool implementation for unsupported platforms.
//!
//! This module provides a fallback thread pool implementation using std::thread
//! for platforms that don't have specialized implementations.
//!
//! This module is only available when the `std` feature is enabled.

extern crate alloc;

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    sync::Arc,
    vec::Vec,
};
use core::{
    sync::atomic::{
        AtomicBool,
        AtomicU64,
        Ordering,
    },
    time::Duration,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use wrt_sync::{
    WrtMutex,
    WrtRwLock,
};

use crate::threading::{
    CpuSet,
    PlatformThreadHandle,
    PlatformThreadPool,
    ThreadHandle,
    ThreadPoolConfig,
    ThreadPoolStats,
    ThreadStats,
    WasmTask,
};

/// Generic thread handle using std::thread
#[cfg(feature = "std")]
struct GenericThreadHandle {
    /// Thread join handle
    handle:  Option<std::thread::JoinHandle<Result<Vec<u8>>>>,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Thread statistics
    stats:   Arc<WrtMutex<ThreadStats>>,
}

#[cfg(feature = "std")]
impl PlatformThreadHandle for GenericThreadHandle {
    fn join(mut self: Box<Self>) -> Result<Vec<u8>> {
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(result) => result,
                Err(_) => Err(Error::runtime_execution_error(
                    "Thread panicked during execution",
                )),
            }
        } else {
            Err(Error::new(
                ErrorCategory::Runtime,
                1,
                "No thread handle available to join",
            ))
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    fn get_stats(&self) -> Result<ThreadStats> {
        Ok(self.stats.lock().clone())
    }

    fn terminate(&self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        Ok(())
    }

    fn join_timeout(&self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        // For generic implementation, we can't easily implement timeout join
        // Return None to indicate timeout not supported
        Ok(None)
    }
}

/// Generic thread pool implementation
#[cfg(feature = "std")]
pub struct GenericThreadPool {
    /// Configuration
    config:         ThreadPoolConfig,
    /// Active threads
    active_threads: Arc<WrtRwLock<BTreeMap<u64, Box<dyn PlatformThreadHandle>>>>,
    /// Thread statistics
    stats:          Arc<WrtMutex<ThreadPoolStats>>,
    /// Next thread ID
    next_thread_id: AtomicU64,
    /// Shutdown flag
    shutdown:       AtomicBool,
    /// Task executor
    executor:       Arc<dyn Fn(WasmTask) -> Result<Vec<u8>> + Send + Sync>,
}

#[cfg(feature = "std")]
impl GenericThreadPool {
    /// Create new generic thread pool
    pub fn new(config: ThreadPoolConfig) -> Result<Self> {
        // Create a simple executor for now
        let executor = Arc::new(|_task: WasmTask| -> Result<Vec<u8>> { Ok(vec![]) });

        Ok(Self {
            config,
            active_threads: Arc::new(WrtRwLock::new(BTreeMap::new())),
            stats: Arc::new(WrtMutex::new(ThreadPoolStats::default())),
            next_thread_id: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            executor,
        })
    }

    /// Set the task executor
    pub fn set_executor<F>(&mut self, executor: F)
    where
        F: Fn(WasmTask) -> Result<Vec<u8>> + Send + Sync + 'static,
    {
        self.executor = Arc::new(executor);
    }
}

#[cfg(feature = "std")]
impl PlatformThreadPool for GenericThreadPool {
    fn configure(&mut self, config: ThreadPoolConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn spawn_wasm_thread(&self, task: WasmTask) -> Result<ThreadHandle> {
        // Check if shutting down
        if self.shutdown.load(Ordering::Acquire) {
            return Err(Error::runtime_execution_error(
                "Thread pool is shutting down",
            ));
        }

        // Check thread limit
        let active_count = self.active_threads.read().len();
        if active_count >= self.config.max_threads {
            return Err(Error::new(
                ErrorCategory::Resource,
                1,
                "Thread pool has reached maximum thread limit",
            ));
        }

        // Get thread ID
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::AcqRel);

        // Create shared state
        let running = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(WrtMutex::new(ThreadStats::default()));

        // Clone for thread
        let task_clone = task.clone();
        let executor = self.executor.clone();
        let running_clone = running.clone();

        // Create thread name
        let thread_name = format!("{}-{}", self.config.name_prefix, thread_id);

        // Spawn thread
        let handle = std::thread::Builder::new()
            .name(thread_name)
            .stack_size(
                task.stack_size.unwrap_or(self.config.stack_size).max(64 * 1024), // Minimum 64KB
            )
            .spawn(move || {
                // Mark as running
                running_clone.store(true, Ordering::Release);

                // Execute the task
                let result = executor(task_clone);

                // Mark as not running
                running_clone.store(false, Ordering::Release);

                result
            })
            .map_err(|_| Error::runtime_execution_error("Failed to spawn thread"))?;

        // Create platform handle
        let platform_handle = Box::new(GenericThreadHandle {
            handle: Some(handle),
            running,
            stats,
        });

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.active_threads += 1;
            stats.total_spawned += 1;
        }

        Ok(ThreadHandle::new(thread_id, platform_handle))
    }

    fn get_stats(&self) -> ThreadPoolStats {
        self.stats.lock().clone()
    }

    fn shutdown(&mut self, timeout: Duration) -> Result<()> {
        // Set shutdown flag
        self.shutdown.store(true, Ordering::Release);

        // Wait for threads to complete
        let start = std::time::Instant::now();
        while !self.active_threads.read().is_empty() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }
}

// Clone implementation for WasmTask (needed for generic thread pool)
impl Clone for WasmTask {
    fn clone(&self) -> Self {
        Self {
            id:           self.id,
            function_id:  self.function_id,
            args:         self.args.clone(),
            priority:     self.priority,
            stack_size:   self.stack_size,
            memory_limit: self.memory_limit,
            cpu_affinity: self.cpu_affinity.clone(),
            deadline:     self.deadline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threading::ThreadPriority;

    #[test]
    fn test_generic_thread_pool_basic() {
        let config = ThreadPoolConfig {
            max_threads: 4,
            priority_range: (ThreadPriority::Low, ThreadPriority::High),
            ..Default::default()
        };

        let mut pool = GenericThreadPool::new(config).unwrap();

        // Set a test executor
        pool.set_executor(|task| Ok(task.args));

        // Spawn a thread
        let task = WasmTask {
            id:           1,
            function_id:  100,
            args:         vec![1, 2, 3, 4],
            priority:     ThreadPriority::Normal,
            stack_size:   None,
            memory_limit: None,
            cpu_affinity: None,
            deadline:     None,
        };

        let handle = pool.spawn_wasm_thread(task).unwrap();

        // Join and verify result
        let result = handle.join().unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_spawned, 1);
    }

    #[test]
    fn test_generic_thread_pool_limits() {
        let config = ThreadPoolConfig {
            max_threads: 1, // Very small limit
            ..Default::default()
        };

        let pool = GenericThreadPool::new(config).unwrap();

        let task = WasmTask {
            id:           1,
            function_id:  100,
            args:         vec![],
            priority:     ThreadPriority::Normal,
            stack_size:   None,
            memory_limit: None,
            cpu_affinity: None,
            deadline:     None,
        };

        // First thread should succeed
        let _handle1 = pool.spawn_wasm_thread(task.clone()).unwrap();

        // Second thread should fail due to limit
        let result2 = pool.spawn_wasm_thread(task);
        assert!(result2.is_err());
    }
}
