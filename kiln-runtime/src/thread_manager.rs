//! WebAssembly Thread Management System
//!
//! This module implements WebAssembly 3.0 thread spawning and management,
//! providing safe, efficient multi-threaded execution of WebAssembly modules
//! with proper isolation and resource management.

// alloc is imported in lib.rs with proper feature gates

use core::sync::atomic::AtomicU32;

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
#[cfg(feature = "std")]
use wrt_platform::threading::{
    Thread,
    ThreadHandle,
    ThreadSpawnOptions,
};

use crate::{
    bounded_runtime_infra::{
        new_thread_map,
        new_thread_vec,
        BoundedThreadMap,
        BoundedThreadVec,
        RuntimeProvider,
        MAX_MANAGED_THREADS,
    },
    prelude::{
        str,
        BoundedCapacity,
        Debug,
        Eq,
        PartialEq,
    },
};

// For no_std builds, provide dummy types
/// Thread representation for `no_std` environments
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct Thread {
    /// Thread identifier
    pub id: ThreadId,
}

/// Thread handle for `no_std` environments
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct ThreadHandle {
    /// Thread identifier
    pub id: ThreadId,
}

/// Thread spawn options for `no_std` environments
#[cfg(not(feature = "std"))]
pub struct ThreadSpawnOptions {
    /// Optional stack size for the thread
    pub stack_size: Option<usize>,
    /// Optional thread priority
    pub priority:   Option<i32>,
    /// Optional thread name
    pub name:       Option<&'static str>,
}

#[cfg(not(feature = "std"))]
impl ThreadHandle {
    /// Terminate the thread (not supported in `no_std` mode)
    pub fn terminate(&self) -> Result<()> {
        Err(Error::not_supported_unsupported_operation(
            "Thread termination not supported in no_std mode",
        ))
    }

    /// Join thread with timeout (not supported in `no_std` mode)
    pub fn join_timeout(&self, _timeout: core::time::Duration) -> Result<()> {
        Err(Error::not_supported_unsupported_operation(
            "Thread join with timeout not supported in no_std mode",
        ))
    }

    /// Join thread (not supported in `no_std` mode)
    pub fn join(&self) -> Result<()> {
        Err(Error::not_supported_unsupported_operation(
            "Thread join not supported in no_std mode",
        ))
    }
}

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::{
    sync::Arc,
    thread,
};

/// Thread identifier for WebAssembly threads
pub type ThreadId = u32;

/// Thread execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadState {
    /// Thread is ready to run but not yet started
    Ready,
    /// Thread is currently running
    Running,
    /// Thread is blocked waiting for synchronization
    Blocked,
    /// Thread has completed successfully
    Completed,
    /// Thread has terminated with an error
    Failed,
    /// Thread has been terminated
    Terminated,
}

impl ThreadState {
    /// Check if thread is completed
    #[must_use]
    pub fn is_completed(&self) -> bool {
        matches!(
            self,
            ThreadState::Completed | ThreadState::Failed | ThreadState::Terminated
        )
    }
}

/// Thread configuration and limits
#[derive(Debug, Clone)]
pub struct ThreadConfig {
    /// Maximum number of threads that can be spawned
    pub max_threads:        usize,
    /// Default stack size for new threads (in bytes)
    pub default_stack_size: usize,
    /// Maximum stack size allowed (in bytes)
    pub max_stack_size:     usize,
    /// Thread priority (0-100, higher is more priority)
    pub priority:           u8,
    /// Enable thread-local storage
    pub enable_tls:         bool,
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            max_threads:        128,
            default_stack_size: 1024 * 1024,     // 1MB
            max_stack_size:     8 * 1024 * 1024, // 8MB
            priority:           50,
            enable_tls:         true,
        }
    }
}

/// Information about a WebAssembly thread
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    /// Unique thread identifier
    pub thread_id:      ThreadId,
    /// Current thread state
    pub state:          ThreadState,
    /// Function index being executed
    pub function_index: u32,
    /// Thread stack size
    pub stack_size:     usize,
    /// Thread priority
    pub priority:       u8,
    /// Parent thread ID (if spawned by another thread)
    pub parent_thread:  Option<ThreadId>,
    /// Thread creation timestamp (nanoseconds since epoch)
    pub created_at:     u64,
    /// Thread completion timestamp (if completed)
    pub completed_at:   Option<u64>,
}

impl ThreadInfo {
    /// Create new thread info
    #[must_use]
    pub fn new(
        thread_id: ThreadId,
        function_index: u32,
        stack_size: usize,
        priority: u8,
        parent_thread: Option<ThreadId>,
    ) -> Self {
        Self {
            thread_id,
            state: ThreadState::Ready,
            function_index,
            stack_size,
            priority,
            parent_thread,
            created_at: wrt_foundation::current_time_ns(),
            completed_at: None,
        }
    }

    /// Check if thread is active (running or ready)
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            ThreadState::Ready | ThreadState::Running | ThreadState::Blocked
        )
    }

    /// Check if thread has completed
    #[must_use]
    pub fn is_completed(&self) -> bool {
        matches!(
            self.state,
            ThreadState::Completed | ThreadState::Failed | ThreadState::Terminated
        )
    }

    /// Get thread execution duration in nanoseconds
    #[must_use]
    pub fn execution_duration(&self) -> Option<u64> {
        self.completed_at.map(|end| end.saturating_sub(self.created_at))
    }
}

/// Thread execution context with isolated state
#[derive(Debug)]
pub struct ThreadExecutionContext {
    /// Thread information
    pub info:          ThreadInfo,
    /// Platform thread handle (not cloneable, so optional)
    pub handle:        Option<ThreadHandle>,
    /// Thread-local memory state
    pub local_memory:  Option<crate::Memory>,
    /// Thread-local global state using bounded collections
    pub local_globals: BoundedThreadVec<crate::Global>,
    /// Execution statistics
    pub stats:         ThreadExecutionStats,
}

impl ThreadExecutionContext {
    /// Create new thread execution context
    pub fn new(info: ThreadInfo) -> Result<Self> {
        Ok(Self {
            info,
            handle: None,
            local_memory: None,
            local_globals: new_thread_vec()?,
            stats: ThreadExecutionStats::new(),
        })
    }

    /// Update thread state
    pub fn update_state(&mut self, new_state: ThreadState) {
        self.info.state = new_state;
        if new_state.is_completed() {
            self.info.completed_at = Some(wrt_platform::time::current_time_ns());
        }
    }

    /// Get thread execution duration
    pub fn execution_duration(&self) -> Option<u64> {
        self.info.execution_duration()
    }
}

/// Thread execution statistics
#[derive(Debug, Clone)]
pub struct ThreadExecutionStats {
    /// Number of instructions executed
    pub instructions_executed: u64,
    /// Number of function calls made
    pub function_calls:        u64,
    /// Number of memory operations performed
    pub memory_operations:     u64,
    /// Number of atomic operations performed
    pub atomic_operations:     u64,
    /// Peak memory usage (bytes)
    pub peak_memory_usage:     usize,
    /// Number of context switches
    pub context_switches:      u64,
}

impl ThreadExecutionStats {
    /// Create new thread execution statistics
    #[must_use]
    pub fn new() -> Self {
        Self {
            instructions_executed: 0,
            function_calls:        0,
            memory_operations:     0,
            atomic_operations:     0,
            peak_memory_usage:     0,
            context_switches:      0,
        }
    }

    /// Record instruction execution
    pub fn record_instruction(&mut self) {
        self.instructions_executed += 1;
    }

    /// Record function call
    pub fn record_function_call(&mut self) {
        self.function_calls += 1;
    }

    /// Record memory operation
    pub fn record_memory_operation(&mut self) {
        self.memory_operations += 1;
    }

    /// Record atomic operation
    pub fn record_atomic_operation(&mut self) {
        self.atomic_operations += 1;
    }

    /// Update peak memory usage
    pub fn update_memory_usage(&mut self, current_usage: usize) {
        if current_usage > self.peak_memory_usage {
            self.peak_memory_usage = current_usage;
        }
    }

    /// Record context switch
    pub fn record_context_switch(&mut self) {
        self.context_switches += 1;
    }
}

impl Default for ThreadExecutionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// WebAssembly thread manager
#[derive(Debug)]
pub struct ThreadManager {
    /// Thread configuration
    pub config:     ThreadConfig,
    /// Active thread contexts
    /// In std mode, use Vec to avoid stack overflow with large arrays
    #[cfg(feature = "std")]
    threads: Vec<Option<ThreadExecutionContext>>,
    #[cfg(not(feature = "std"))]
    threads: [Option<ThreadExecutionContext>; MAX_MANAGED_THREADS],
    /// Next thread ID to assign
    next_thread_id: ThreadId,
    /// Thread manager statistics
    pub stats:      ThreadManagerStats,
}

impl ThreadManager {
    /// Create new thread manager
    pub fn new(config: ThreadConfig) -> Result<Self> {
        Ok(Self {
            config,
            #[cfg(feature = "std")]
            threads: (0..MAX_MANAGED_THREADS).map(|_| None).collect(),
            #[cfg(not(feature = "std"))]
            threads: [const { None }; MAX_MANAGED_THREADS],
            next_thread_id: 1, // Thread ID 0 is reserved for main thread
            stats: ThreadManagerStats::new(),
        })
    }

    /// Spawn a new WebAssembly thread
    pub fn spawn_thread(
        &mut self,
        function_index: u32,
        stack_size: Option<usize>,
        parent_thread: Option<ThreadId>,
    ) -> Result<ThreadId> {
        // Check thread limits
        if self.active_thread_count() >= self.config.max_threads {
            return Err(Error::resource_exhausted("Maximum thread limit reached"));
        }

        // Validate stack size
        let stack_size = stack_size.unwrap_or(self.config.default_stack_size);
        if stack_size > self.config.max_stack_size {
            return Err(Error::validation_error(
                "Stack size exceeds maximum allowed",
            ));
        }

        // Generate thread ID
        let thread_id = self.next_thread_id;
        self.next_thread_id += 1;

        // Create thread info
        let thread_info = ThreadInfo::new(
            thread_id,
            function_index,
            stack_size,
            self.config.priority,
            parent_thread,
        );

        // Create thread execution context
        let context = ThreadExecutionContext::new(thread_info)?;

        // Store thread context using bounded map
        // Store thread context in array
        if thread_id as usize >= MAX_MANAGED_THREADS {
            return Err(Error::runtime_execution_error("Runtime execution error"));
        }
        self.threads[thread_id as usize] = Some(context);

        self.stats.threads_spawned += 1;

        Ok(thread_id)
    }

    /// Start thread execution
    pub fn start_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        let context = self.get_thread_context_mut(thread_id)?;

        if context.info.state != ThreadState::Ready {
            return Err(Error::runtime_execution_error("Thread not in ready state"));
        }

        // Create thread spawn options
        let thread_priority = match context.info.priority {
            0..=20 => wrt_platform::threading::ThreadPriority::Idle,
            21..=40 => wrt_platform::threading::ThreadPriority::Low,
            41..=60 => wrt_platform::threading::ThreadPriority::Normal,
            61..=80 => wrt_platform::threading::ThreadPriority::High,
            81..=100 => wrt_platform::threading::ThreadPriority::Realtime,
            _ => wrt_platform::threading::ThreadPriority::Normal, // fallback
        };

        let spawn_options = ThreadSpawnOptions {
            stack_size: Some(context.info.stack_size),
            priority:   Some(thread_priority),
            name:       Some("wasm-thread".to_string()),
        };

        // Spawn platform thread (feature-gated)
        #[cfg(feature = "std")]
        let handle = wrt_platform::threading::spawn_thread(spawn_options, move || {
            // Thread execution logic would go here
            // This is a placeholder for the actual WebAssembly execution
            Ok(())
        })
        .map_err(|_| Error::runtime_execution_error("Failed to spawn platform thread"))?;

        #[cfg(not(feature = "std"))]
        let handle = ThreadHandle { id: thread_id };

        context.handle = Some(handle);
        context.update_state(ThreadState::Running);

        self.stats.threads_started += 1;

        Ok(())
    }

    /// Terminate a thread
    pub fn terminate_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        let context = self.get_thread_context_mut(thread_id)?;

        if let Some(handle) = &context.handle {
            // Request thread termination
            handle
                .terminate()
                .map_err(|_| Error::runtime_execution_error("Failed to terminate thread"))?;
        }

        context.update_state(ThreadState::Terminated);
        self.stats.threads_terminated += 1;

        Ok(())
    }

    /// Join a thread (wait for completion)
    pub fn join_thread(
        &mut self,
        thread_id: ThreadId,
        timeout_ms: Option<u64>,
    ) -> Result<ThreadExecutionStats> {
        let stats_clone = {
            let context = self.get_thread_context_mut(thread_id)?;

            if let Some(handle) = context.handle.take() {
                // Wait for thread completion
                let result = if let Some(timeout) = timeout_ms {
                    let duration = core::time::Duration::from_millis(timeout);
                    handle.join_timeout(duration).map(|opt| opt.unwrap_or_default())
                } else {
                    handle.join()
                };

                if let Ok(_result_data) = result {
                    context.update_state(ThreadState::Completed);
                } else {
                    context.update_state(ThreadState::Failed);
                    return Err(Error::runtime_execution_error("Thread join failed"));
                }
            }

            context.stats.clone()
        };

        // Update stats after the borrow of context ends
        self.stats.threads_completed += 1;

        Ok(stats_clone)
    }

    /// Get thread information
    pub fn get_thread_info(&self, thread_id: ThreadId) -> Result<&ThreadInfo> {
        let context = self.get_thread_context(thread_id)?;
        Ok(&context.info)
    }

    /// Get all active threads
    #[cfg(feature = "std")]
    pub fn get_active_threads(&self) -> Vec<ThreadId> {
        self.threads
            .iter()
            .enumerate()
            .filter_map(|(index, context_opt)| {
                if let Some(context) = context_opt {
                    if context.info.is_active() {
                        Some(context.info.thread_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get number of active threads
    pub fn active_thread_count(&self) -> usize {
        self.threads.iter().filter(|t| t.is_some()).count()
    }

    /// Cleanup completed threads
    pub fn cleanup_completed_threads(&mut self) -> usize {
        let initial_count = self.thread_count();

        #[cfg(feature = "std")]
        {
            // For arrays, we need to manually implement retain-like behavior
            for i in 0..self.threads.len() {
                if let Some(context) = &self.threads[i] {
                    if !context.info.is_active() {
                        self.threads[i] = None;
                    }
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // Binary std/no_std choice
            let mut write_idx = 0;
            for read_idx in 0..self.threads.len() {
                if let Some(context) = &self.threads[read_idx] {
                    if context.info.is_active() {
                        if write_idx != read_idx {
                            // Move active thread to write position
                            // This is a simplified approach - in practice might
                            // need more sophisticated cleanup
                        }
                        write_idx += 1;
                    }
                }
            }
            // Truncate to remove completed threads (simplified)
        }

        initial_count - self.thread_count()
    }

    /// Get total thread count
    pub fn thread_count(&self) -> usize {
        self.threads.iter().filter(|slot| slot.is_some()).count()
    }

    // Private helper methods

    fn get_thread_context(&self, thread_id: ThreadId) -> Result<&ThreadExecutionContext> {
        self.threads
            .get(thread_id as usize)
            .and_then(|opt| opt.as_ref())
            .ok_or_else(|| Error::runtime_execution_error("Thread context not found"))
    }

    /// Get mutable reference to thread execution context
    pub fn get_thread_context_mut(
        &mut self,
        thread_id: ThreadId,
    ) -> Result<&mut ThreadExecutionContext> {
        self.threads
            .get_mut(thread_id as usize)
            .and_then(|opt| opt.as_mut())
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::INVALID_ARGUMENT,
                    "Invalid thread ID",
                )
            })
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new(ThreadConfig::default()).unwrap_or_else(|_| {
            // Create a minimal thread manager with very limited resources
            Self {
                #[cfg(feature = "std")]
                threads:        (0..MAX_MANAGED_THREADS).map(|_| None).collect(),
                #[cfg(not(feature = "std"))]
                threads:        [const { None }; MAX_MANAGED_THREADS],
                next_thread_id: 1,
                config:         ThreadConfig {
                    max_threads:        1,
                    default_stack_size: 64 * 1024,
                    max_stack_size:     64 * 1024,
                    priority:           50,
                    enable_tls:         false,
                },
                stats:          ThreadManagerStats::new(),
            }
        })
    }
}

/// Thread manager statistics
#[derive(Debug, Clone)]
pub struct ThreadManagerStats {
    /// Total number of threads spawned
    pub threads_spawned:         u64,
    /// Total number of threads started
    pub threads_started:         u64,
    /// Total number of threads completed successfully
    pub threads_completed:       u64,
    /// Total number of threads that failed
    pub threads_failed:          u64,
    /// Total number of threads terminated
    pub threads_terminated:      u64,
    /// Peak concurrent thread count
    pub peak_concurrent_threads: usize,
}

impl ThreadManagerStats {
    fn new() -> Self {
        Self {
            threads_spawned:         0,
            threads_started:         0,
            threads_completed:       0,
            threads_failed:          0,
            threads_terminated:      0,
            peak_concurrent_threads: 0,
        }
    }

    /// Get thread success rate (0.0 to 1.0)
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total_completed = self.threads_completed + self.threads_failed;
        if total_completed == 0 {
            0.0
        } else {
            self.threads_completed as f64 / total_completed as f64
        }
    }

    /// Check if thread management is healthy
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.success_rate() > 0.95 && self.threads_spawned > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_config_default() {
        let config = ThreadConfig::default();
        assert_eq!(config.max_threads, 128);
        assert_eq!(config.default_stack_size, 1024 * 1024);
        assert!(config.enable_tls);
    }

    #[test]
    fn test_thread_info_creation() {
        let info = ThreadInfo::new(1, 42, 1024 * 1024, 50, None);
        assert_eq!(info.thread_id, 1);
        assert_eq!(info.function_index, 42);
        assert_eq!(info.state, ThreadState::Ready);
        assert!(info.is_active());
        assert!(!info.is_completed());
    }

    #[test]
    fn test_thread_manager_creation() {
        let config = ThreadConfig::default();
        let manager = ThreadManager::new(config).unwrap();
        assert_eq!(manager.thread_count(), 0);
        assert_eq!(manager.active_thread_count(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_thread_spawning() {
        let mut manager = ThreadManager::default();

        let thread_id = manager.spawn_thread(42, Some(2 * 1024 * 1024), None).unwrap();
        assert_eq!(thread_id, 1);
        assert_eq!(manager.thread_count(), 1);
        assert_eq!(manager.active_thread_count(), 1);

        let info = manager.get_thread_info(thread_id).unwrap();
        assert_eq!(info.function_index, 42);
        assert_eq!(info.stack_size, 2 * 1024 * 1024);
    }

    #[test]
    fn test_thread_stats() {
        let mut stats = ThreadExecutionStats::new();
        stats.record_instruction();
        stats.record_function_call();
        stats.record_atomic_operation();
        stats.update_memory_usage(1024);

        assert_eq!(stats.instructions_executed, 1);
        assert_eq!(stats.function_calls, 1);
        assert_eq!(stats.atomic_operations, 1);
        assert_eq!(stats.peak_memory_usage, 1024);
    }

    #[test]
    fn test_manager_stats() {
        let stats = ThreadManagerStats::new();
        assert_eq!(stats.success_rate(), 0.0);
        assert!(!stats.is_healthy());
    }
}
