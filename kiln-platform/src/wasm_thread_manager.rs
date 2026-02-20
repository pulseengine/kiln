//! Safe WebAssembly thread manager with resource quotas and monitoring.
//!
//! This module provides a high-level thread management system that enforces
//! safety constraints and resource limits for WebAssembly thread execution.

extern crate alloc;

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::String,
    sync::Arc,
    vec::Vec,
};
use core::{
    sync::atomic::{
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
    create_thread_pool,
    ExecutionMonitor,
    PlatformThreadPool,
    ResourceTracker,
    ThreadHandle,
    ThreadHealth,
    ThreadPoolConfig,
    ThreadPriority,
    ThreadSpawnRequest,
    ThreadingLimits,
    WasmTask,
};

/// Type alias for complex executor function type
type ExecutorFn = Arc<dyn Fn(u32, Vec<u8>) -> Result<Vec<u8>> + Send + Sync>;

/// WebAssembly module information for thread tracking
#[derive(Debug, Clone)]
pub struct WasmModuleInfo {
    /// Module ID
    pub id:               u64,
    /// Module name
    pub name:             String,
    /// Maximum allowed threads
    pub max_threads:      usize,
    /// Memory limit for all threads
    pub memory_limit:     usize,
    /// CPU quota per thread
    pub cpu_quota:        Duration,
    /// Default thread priority
    pub default_priority: ThreadPriority,
}

/// Thread execution result
#[derive(Debug, Clone)]
pub enum ThreadExecutionResult {
    /// Thread completed successfully
    Success(Vec<u8>),
    /// Thread failed with error
    Error(String),
    /// Thread was cancelled
    Cancelled,
    /// Thread timed out
    Timeout,
}

/// Thread information for tracking
#[derive(Debug)]
struct ThreadInfo {
    /// Thread handle
    handle:      ThreadHandle,
    /// Module that spawned this thread
    module_id:   u64,
    /// Function being executed
    function_id: u32,
    /// Thread spawn time
    spawn_time:  std::time::Instant,
    /// Deadline if any
    deadline:    Option<std::time::Instant>,
    /// Binary std/no_std choice
    stack_size:  usize,
}

/// Simple execution monitor implementation
pub struct SimpleExecutionMonitor {
    /// Tracked threads
    threads: Arc<WrtRwLock<BTreeMap<u64, ThreadMonitorInfo>>>,
    /// Monitoring enabled
    enabled: bool,
}

#[derive(Debug, Clone)]
struct ThreadMonitorInfo {
    spawn_time:     std::time::Instant,
    deadline:       Option<std::time::Instant>,
    last_heartbeat: std::time::Instant,
    cpu_quota:      Duration,
    cpu_used:       Duration,
}

impl SimpleExecutionMonitor {
    /// Create new execution monitor
    pub fn new() -> Self {
        Self {
            threads: Arc::new(WrtRwLock::new(BTreeMap::new())),
            enabled: true,
        }
    }
}

impl ExecutionMonitor for SimpleExecutionMonitor {
    fn track_thread(&self, handle: &ThreadHandle) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let info = ThreadMonitorInfo {
            spawn_time:     std::time::Instant::now(),
            deadline:       None,
            last_heartbeat: std::time::Instant::now(),
            cpu_quota:      Duration::from_secs(60), // Default 1 minute
            cpu_used:       Duration::from_secs(0),
        };

        self.threads.write().insert(handle.id(), info);
        Ok(())
    }

    fn check_thread_health(&self, id: u64) -> Result<ThreadHealth> {
        let threads = self.threads.read();
        let info = threads
            .get(&id)
            .ok_or_else(|| Error::runtime_execution_error("Thread not found"))?;

        let now = std::time::Instant::now();

        // Check deadline
        if let Some(deadline) = info.deadline {
            if now > deadline {
                return Ok(ThreadHealth::LifetimeExceeded);
            }
        }

        // Check CPU quota
        if info.cpu_used > info.cpu_quota {
            return Ok(ThreadHealth::CpuQuotaExceeded);
        }

        // Check heartbeat (simple unresponsive check)
        if now.duration_since(info.last_heartbeat) > Duration::from_secs(30) {
            return Ok(ThreadHealth::Unresponsive);
        }

        Ok(ThreadHealth::Healthy)
    }

    fn kill_unhealthy_thread(&self, id: u64) -> Result<()> {
        // Remove from tracking
        self.threads.write().remove(&id);
        // In a real implementation, we would forcibly terminate the thread
        // This is platform-specific and dangerous, so we just remove from tracking
        Ok(())
    }
}

/// Safe WebAssembly thread manager
pub struct WasmThreadManager {
    /// Platform-specific thread pool
    pool:             Box<dyn PlatformThreadPool>,
    /// Resource tracker for quotas
    resource_tracker: Arc<ResourceTracker>,
    /// Execution monitor
    monitor:          Arc<dyn ExecutionMonitor>,
    /// Active threads
    threads:          Arc<WrtRwLock<BTreeMap<u64, ThreadInfo>>>,
    /// Module registry
    modules:          Arc<WrtRwLock<BTreeMap<u64, WasmModuleInfo>>>,
    /// Next thread ID
    next_thread_id:   AtomicU64,
    /// Shutdown flag
    shutdown:         WrtMutex<bool>,
    /// Task executor function
    executor:         ExecutorFn,
}

impl core::fmt::Debug for WasmThreadManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WasmThreadManager")
            .field("pool", &"<Box<dyn PlatformThreadPool>>")
            .field("resource_tracker", &self.resource_tracker)
            .field("monitor", &"<Arc<dyn ExecutionMonitor>>")
            .field("threads", &"<BTreeMap>")
            .field("modules", &"<BTreeMap>")
            .field("next_thread_id", &self.next_thread_id)
            .field("shutdown", &self.shutdown)
            .field("executor", &"<ExecutorFn>")
            .finish()
    }
}

impl WasmThreadManager {
    /// Create new WebAssembly thread manager
    pub fn new(
        _config: ThreadPoolConfig,
        limits: ThreadingLimits,
        executor: ExecutorFn,
    ) -> Result<Self> {
        let pool = create_thread_pool(&_config)?;
        let resource_tracker = Arc::new(ResourceTracker::new(limits));
        let monitor = Arc::new(SimpleExecutionMonitor::new());

        Ok(Self {
            pool,
            resource_tracker,
            monitor,
            threads: Arc::new(WrtRwLock::new(BTreeMap::new())),
            modules: Arc::new(WrtRwLock::new(BTreeMap::new())),
            next_thread_id: AtomicU64::new(1),
            shutdown: WrtMutex::new(false),
            executor,
        })
    }

    /// Register a WebAssembly module
    pub fn register_module(&self, module: WasmModuleInfo) -> Result<()> {
        self.modules.write().insert(module.id, module);
        Ok(())
    }

    /// Unregister a WebAssembly module
    pub fn unregister_module(&self, module_id: u64) -> Result<()> {
        // Cancel all threads for this module
        self.cancel_module_threads(module_id)?;

        // Remove module
        self.modules.write().remove(&module_id);
        Ok(())
    }

    /// Spawn a WebAssembly thread with safety checks
    pub fn spawn_thread(&self, request: &ThreadSpawnRequest) -> Result<u64> {
        // Check if shutting down
        if *self.shutdown.lock() {
            return Err(Error::runtime_execution_error(
                "Thread manager is shutting down",
            ));
        }

        // Get module info
        let module = {
            let modules = self.modules.read();
            modules
                .get(&request.module_id)
                .cloned()
                .ok_or_else(|| Error::new(ErrorCategory::Validation, 1, "Module not found"))?
        };

        // Binary std/no_std choice
        if !self.resource_tracker.can_allocate_thread(request)? {
            return Err(Error::runtime_execution_error(
                "Thread allocation limit exceeded",
            ));
        }

        // Get thread ID
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::AcqRel);

        // Determine stack size
        let stack_size = request
            .stack_size
            .unwrap_or(2 * 1024 * 1024) // 2MB default
            .max(64 * 1024); // Minimum 64KB

        // Calculate deadline
        let deadline = module
            .cpu_quota
            .checked_add(Duration::from_secs(10))
            .map(|d| std::time::Instant::now() + d);

        // Create WASM task
        let priority = request.priority.unwrap_or(module.default_priority);
        let task = WasmTask {
            id: thread_id,
            function_id: request.function_id,
            args: request.args.clone(),
            priority,
            stack_size: Some(stack_size),
            memory_limit: Some(module.memory_limit / module.max_threads.max(1)),
            cpu_affinity: None, // Could be configured per module
            deadline: Some(module.cpu_quota),
        };

        // Allocate resources
        self.resource_tracker.allocate_thread(request.module_id, stack_size)?;

        // Spawn thread on platform pool
        let handle = match self.pool.spawn_wasm_thread(task) {
            Ok(handle) => handle,
            Err(e) => {
                // Release resources on failure
                self.resource_tracker.release_thread(request.module_id, stack_size);
                return Err(e);
            },
        };

        // Track with monitor
        self.monitor.track_thread(&handle)?;

        // Store thread info
        let thread_info = ThreadInfo {
            handle,
            module_id: request.module_id,
            function_id: request.function_id,
            spawn_time: std::time::Instant::now(),
            deadline,
            stack_size,
        };

        self.threads.write().insert(thread_id, thread_info);

        Ok(thread_id)
    }

    /// Join a thread and get its result
    pub fn join_thread(&self, thread_id: u64) -> Result<ThreadExecutionResult> {
        // Remove thread from tracking
        let thread_info = {
            let mut threads = self.threads.write();
            threads
                .remove(&thread_id)
                .ok_or_else(|| Error::new(ErrorCategory::Validation, 1, "Thread not found"))?
        };

        // Release resources
        self.resource_tracker
            .release_thread(thread_info.module_id, thread_info.stack_size);

        // Check if thread exceeded deadline
        if let Some(deadline) = thread_info.deadline {
            if std::time::Instant::now() > deadline {
                return Ok(ThreadExecutionResult::Timeout);
            }
        }

        // Join the thread
        match thread_info.handle.join() {
            Ok(data) => Ok(ThreadExecutionResult::Success(data)),
            Err(e) => Ok(ThreadExecutionResult::Error(e.to_string())),
        }
    }

    /// Check if a thread is still running
    pub fn is_thread_running(&self, thread_id: u64) -> Result<bool> {
        let threads = self.threads.read();
        let thread_info = threads
            .get(&thread_id)
            .ok_or_else(|| Error::runtime_execution_error("Thread not found"))?;

        Ok(thread_info.handle.is_running())
    }

    /// Cancel a specific thread
    pub fn cancel_thread(&self, thread_id: u64) -> Result<()> {
        // In a real implementation, we would send a cancellation signal
        // For now, we just remove it from tracking
        let mut threads = self.threads.write();
        if let Some(thread_info) = threads.remove(&thread_id) {
            self.resource_tracker
                .release_thread(thread_info.module_id, thread_info.stack_size);
        }
        Ok(())
    }

    /// Cancel all threads for a specific module
    pub fn cancel_module_threads(&self, module_id: u64) -> Result<()> {
        let mut threads = self.threads.write();
        let to_remove: Vec<u64> = threads
            .iter()
            .filter(|(_, info)| info.module_id == module_id)
            .map(|(id, _)| *id)
            .collect();

        for thread_id in to_remove {
            if let Some(thread_info) = threads.remove(&thread_id) {
                self.resource_tracker
                    .release_thread(thread_info.module_id, thread_info.stack_size);
            }
        }

        Ok(())
    }

    /// Perform health check on all threads
    pub fn health_check(&self) -> Result<Vec<(u64, ThreadHealth)>> {
        let threads = self.threads.read();
        let mut results = Vec::new();

        for (thread_id, _) in threads.iter() {
            match self.monitor.check_thread_health(*thread_id) {
                Ok(health) => results.push((*thread_id, health)),
                Err(_) => results.push((*thread_id, ThreadHealth::Unresponsive)),
            }
        }

        Ok(results)
    }

    /// Kill unhealthy threads
    pub fn kill_unhealthy_threads(&self) -> Result<usize> {
        let health_results = self.health_check()?;
        let mut killed = 0;

        for (thread_id, health) in health_results {
            match health {
                ThreadHealth::CpuQuotaExceeded
                | ThreadHealth::LifetimeExceeded
                | ThreadHealth::Deadlocked
                | ThreadHealth::Unresponsive => {
                    if self.cancel_thread(thread_id).is_ok() {
                        let _ = self.monitor.kill_unhealthy_thread(thread_id);
                        killed += 1;
                    }
                },
                ThreadHealth::Healthy => {},
            }
        }

        Ok(killed)
    }

    /// Get thread manager statistics
    pub fn get_stats(&self) -> ThreadManagerStats {
        let threads = self.threads.read();
        let pool_stats = self.pool.get_stats();

        ThreadManagerStats {
            total_threads: threads.len(),
            pool_stats,
            modules_registered: self.modules.read().len(),
        }
    }

    /// Shutdown the thread manager
    pub fn shutdown(&mut self, timeout: Duration) -> Result<()> {
        *self.shutdown.lock() = true;

        // Cancel all threads
        let thread_ids: Vec<u64> = self.threads.read().keys().copied().collect();
        for thread_id in thread_ids {
            let _ = self.cancel_thread(thread_id);
        }

        // Shutdown thread pool
        self.pool.shutdown(timeout)?;

        Ok(())
    }

    /// Serialize component values to bytes (simplified)
    fn serialize_component_values(&self, values: &[u8]) -> Result<Vec<u8>> {
        // Simple pass-through for now
        Ok(values.to_vec())
    }

    /// Deserialize component values from bytes (simplified)
    fn deserialize_component_values(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Simple pass-through for now
        Ok(data.to_vec())
    }
}

/// Thread manager statistics
#[derive(Debug, Clone)]
pub struct ThreadManagerStats {
    /// Total active threads
    pub total_threads:      usize,
    /// Thread pool statistics
    pub pool_stats:         crate::threading::ThreadPoolStats,
    /// Number of registered modules
    pub modules_registered: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_executor() -> ExecutorFn {
        Arc::new(|_function_id, args| Ok(args))
    }

    #[test]
    fn test_wasm_thread_manager_creation() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let manager = WasmThreadManager::new(config, limits, executor);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_module_registration() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let manager = WasmThreadManager::new(config, limits, executor).unwrap();

        let module = WasmModuleInfo {
            id:               1,
            name:             "test_module".to_string(),
            max_threads:      4,
            memory_limit:     64 * 1024 * 1024,
            cpu_quota:        Duration::from_secs(60),
            default_priority: ThreadPriority::Normal,
        };

        assert!(manager.register_module(module).is_ok());
        assert!(manager.unregister_module(1).is_ok());
    }

    #[test]
    fn test_serialization() {
        let config = ThreadPoolConfig::default();
        let limits = ThreadingLimits::default();
        let executor = create_test_executor();

        let manager = WasmThreadManager::new(config, limits, executor).unwrap();

        let data = vec![1, 2, 3, 4];
        let serialized = manager.serialize_component_values(&data).unwrap();
        let deserialized = manager.deserialize_component_values(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }
}
