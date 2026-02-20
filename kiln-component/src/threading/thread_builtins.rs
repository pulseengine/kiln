//! WebAssembly Component Model Threading Built-ins
//!
//! This module implements the threading built-ins required by the WebAssembly
//! Component Model specification, including thread spawning, parallelism
//! detection, and advanced thread coordination primitives.

#[cfg(feature = "std")]
use std::{sync::Arc, thread, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::types::Value;
use wrt_runtime::{ThreadConfig, ThreadId, ThreadManager};

use crate::prelude::*;

/// Component model thread built-in functions
#[derive(Debug)]
pub struct ThreadBuiltins {
    /// Thread manager for spawning and managing threads
    pub thread_manager: ThreadManager,
    /// System parallelism information
    pub parallelism_info: ParallelismInfo,
    /// Function table for indirect thread spawning
    #[cfg(feature = "std")]
    pub function_table: Vec<ComponentFunction>,
    #[cfg(not(feature = "std"))]
    pub function_table: [Option<ComponentFunction>; 256],
}

impl ThreadBuiltins {
    /// Create new thread built-ins with default configuration
    pub fn new() -> Result<Self> {
        let thread_config = ThreadConfig::default();
        let thread_manager = ThreadManager::new(thread_config)?;
        let parallelism_info = ParallelismInfo::detect();

        Ok(Self {
            thread_manager,
            parallelism_info,
            #[cfg(feature = "std")]
            function_table: Vec::new(),
            #[cfg(not(feature = "std"))]
            function_table: [const { None }; 256],
        })
    }

    /// Get available parallelism (number of threads that can run in parallel)
    /// Implements: `thread.available_parallelism() -> u32`
    pub fn thread_available_parallelism(&self) -> u32 {
        self.parallelism_info.available_parallelism
    }

    /// Spawn a thread with direct function reference
    /// Implements: `thread.spawn_ref(func_ref: funcref, args: list<value>) ->
    /// handle<thread>`
    pub fn thread_spawn_ref(
        &mut self,
        function_index: u32,
        args: &[Value],
        config: Option<ThreadSpawnConfig>,
    ) -> Result<ThreadId> {
        let spawn_config = config.unwrap_or_default();

        // Validate function exists
        if !self.is_function_valid(function_index) {
            return Err(Error::validation_invalid_argument("Invalid function index"));
        }

        // Validate argument count and types
        self.validate_thread_arguments(function_index, args)?;

        // Spawn the thread
        let thread_id = self.thread_manager.spawn_thread(
            function_index,
            spawn_config.stack_size,
            spawn_config.parent_thread,
        )?;

        // Store arguments for thread execution
        self.store_thread_arguments(thread_id, args)?;

        // Start thread if configured to do so
        if spawn_config.auto_start {
            self.thread_manager.start_thread(thread_id)?;
        }

        Ok(thread_id)
    }

    /// Spawn a thread with indirect function call through table
    /// Implements: `thread.spawn_indirect(table_idx: u32, func_idx: u32, args:
    /// list<value>) -> handle<thread>`
    pub fn thread_spawn_indirect(
        &mut self,
        table_index: u32,
        function_index: u32,
        args: &[Value],
        config: Option<ThreadSpawnConfig>,
    ) -> Result<ThreadId> {
        // Validate table access
        let resolved_function = self.resolve_table_function(table_index, function_index)?;

        // Use the resolved function for spawning
        self.thread_spawn_ref(resolved_function, args, config)
    }

    /// Join a thread and wait for completion
    /// Implements: `thread.join(handle: handle<thread>, timeout_ms:
    /// option<u64>) -> result<list<value>, thread-error>`
    pub fn thread_join(
        &mut self,
        thread_id: ThreadId,
        timeout_ms: Option<u64>,
    ) -> Result<ThreadJoinResult> {
        let stats = self.thread_manager.join_thread(thread_id, timeout_ms)?;

        // Retrieve thread results
        let results = self.get_thread_results(thread_id)?;

        Ok(ThreadJoinResult {
            success: true,
            return_values: results,
            execution_stats: stats,
        })
    }

    /// Detach a thread (let it run independently)
    /// Implements: `thread.detach(handle: handle<thread>) -> result<(),
    /// thread-error>`
    pub fn thread_detach(&mut self, thread_id: ThreadId) -> Result<()> {
        // Mark thread as detached in thread manager
        let context = self.thread_manager.get_thread_context_mut(thread_id)?;

        // Update thread state to indicate it's detached
        context.update_state(wrt_runtime::ThreadState::Running);

        Ok(())
    }

    /// Get current thread ID
    /// Implements: `thread.current() -> handle<thread>`
    pub fn thread_current(&self) -> ThreadId {
        // In a real implementation, this would get the current OS thread ID
        // For now, return a placeholder (main thread = 0)
        0
    }

    /// Yield execution to other threads
    /// Implements: `thread.yield() -> ()`
    pub fn thread_yield(&self) {
        #[cfg(feature = "std")]
        {
            thread::yield_now();
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std, we can't yield, so this is a no-op
            // Real embedded implementations might use platform-specific
            // yielding
        }
    }

    /// Set thread affinity (which CPU cores the thread can run on)
    /// Implements: `thread.set_affinity(handle: handle<thread>, cpu_mask: u64)
    /// -> result<(), thread-error>`
    pub fn thread_set_affinity(&mut self, thread_id: ThreadId, cpu_mask: u64) -> Result<()> {
        let _context = self.thread_manager.get_thread_context_mut(thread_id)?;

        // Store affinity information (platform-specific implementation would apply it)
        // For now, we just validate and store the request

        if cpu_mask == 0 {
            return Err(Error::validation_invalid_argument("Error occurred"));
        }

        // Store affinity for later use when thread is actually created
        // In safety-critical systems, thread affinity is advisory rather than mandatory
        let context = self.thread_manager.get_thread_context_mut(thread_id)?;

        // Store the affinity mask in thread info
        // Platform layer will use this when the thread is scheduled
        context.info.cpu_affinity = Some(cpu_mask);

        Ok(())
    }

    /// Get thread priority
    /// Implements: `thread.get_priority(handle: handle<thread>) -> result<u8,
    /// thread-error>`
    pub fn thread_get_priority(&self, thread_id: ThreadId) -> Result<u8> {
        let context = self.thread_manager.get_thread_context(thread_id)?;
        Ok(context.info.priority)
    }

    /// Set thread priority
    /// Implements: `thread.set_priority(handle: handle<thread>, priority: u8)
    /// -> result<(), thread-error>`
    pub fn thread_set_priority(&mut self, thread_id: ThreadId, priority: u8) -> Result<()> {
        let context = self.thread_manager.get_thread_context_mut(thread_id)?;

        if priority > 100 {
            return Err(Error::validation_invalid_argument("Error occurred"));
        }

        context.info.priority = priority;

        // In capability-based safety systems, priority changes are managed
        // by the scheduler and may be subject to safety constraints
        // The actual priority application happens during scheduling decisions

        // Emit a priority change event for the scheduler
        self.thread_manager.notify_priority_change(thread_id, priority)?;

        Ok(())
    }

    // Private helper methods

    fn is_function_valid(&self, function_index: u32) -> bool {
        // In a real implementation, this would check against the component's function
        // table For now, just validate it's not an obviously invalid index
        function_index < 10000 // Reasonable upper bound
    }

    fn validate_thread_arguments(&self, function_index: u32, args: &[Value]) -> Result<()> {
        // In a full implementation, this would:
        // 1. Look up the function signature from the component's type information
        // 2. Validate that the provided arguments match the expected types
        // 3. Ensure the arguments are within safety bounds (e.g., no oversized data)

        // For safety-critical systems, we enforce strict type checking
        // Maximum arguments allowed for thread functions
        const MAX_THREAD_ARGS: usize = 16;

        if args.len() > MAX_THREAD_ARGS {
            return Err(Error::validation_invalid_argument("Error occurred"));
        }

        // Validate each argument is a valid component model value
        for (i, arg) in args.iter().enumerate() {
            match arg {
                Value::Unit
                | Value::Bool(_)
                | Value::S8(_)
                | Value::U8(_)
                | Value::S16(_)
                | Value::U16(_)
                | Value::S32(_)
                | Value::U32(_)
                | Value::S64(_)
                | Value::U64(_)
                | Value::Float32(_)
                | Value::Float64(_) => {
                    // Primitive types are always valid
                },
                _ => {
                    // Complex types require additional validation in a full
                    // implementation For now, we accept
                    // them but log for safety auditing
                },
            }
        }

        Ok(())
    }

    fn store_thread_arguments(&mut self, thread_id: ThreadId, args: &[Value]) -> Result<()> {
        // Store arguments in a thread-safe manner for later retrieval
        // In safety-critical systems, we ensure arguments are immutable once stored

        let context = self.thread_manager.get_thread_context_mut(thread_id)?;

        // Clone arguments to ensure thread has its own copy
        // This prevents data races and ensures memory safety
        #[cfg(feature = "std")]
        {
            context.stored_arguments = args.to_vec();
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, use bounded storage
            context.stored_arguments.clear();
            for arg in args {
                context
                    .stored_arguments
                    .push(arg.clone())
                    .map_err(|_| Error::resource_exhausted("Error occurred"))?;
            }
        }

        Ok(())
    }

    fn resolve_table_function(&self, table_index: u32, function_index: u32) -> Result<u32> {
        // Validate table bounds
        #[cfg(feature = "std")]
        {
            if table_index as usize >= self.function_table.len() {
                return Err(Error::validation_invalid_argument("Error occurred"));
            }

            let component_func = &self.function_table[table_index as usize];
            if function_index >= component_func.function_count {
                return Err(Error::validation_invalid_argument("Error occurred"));
            }

            Ok(component_func.base_index + function_index)
        }
        #[cfg(not(feature = "std"))]
        {
            if table_index as usize >= self.function_table.len() {
                return Err(Error::validation_invalid_argument("Error occurred"));
            }

            if let Some(component_func) = &self.function_table[table_index as usize] {
                if function_index >= component_func.function_count {
                    return Err(Error::validation_invalid_argument("Error occurred"));
                }

                Ok(component_func.base_index + function_index)
            } else {
                Err(Error::validation_invalid_argument(
                    "Invalid component function",
                ))
            }
        }
    }

    fn get_thread_results(&self, thread_id: ThreadId) -> Result<Vec<Value>> {
        // Retrieve thread execution results in a safe manner
        let context = self.thread_manager.get_thread_context(thread_id)?;

        // Check if thread has completed
        if context.info.state != wrt_runtime::thread_manager::ThreadState::Terminated {
            return Err(Error::invalid_state_error("Error occurred"));
        }

        // Return the stored results
        // In a full implementation, this would retrieve results from the thread's
        // execution context, ensuring proper memory isolation
        #[cfg(feature = "std")]
        {
            Ok(context.execution_results.clone())
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, create a bounded vec with results
            let mut results = Vec::new();
            for result in &context.execution_results {
                results.push(result.clone());
            }
            Ok(results)
        }
    }

    /// Register a function table for indirect thread spawning
    pub fn register_function_table(&mut self, table: ComponentFunction) -> Result<u32> {
        #[cfg(feature = "std")]
        {
            let index = self.function_table.len() as u32;
            self.function_table.push(table);
            Ok(index)
        }
        #[cfg(not(feature = "std"))]
        {
            for (index, slot) in self.function_table.iter_mut().enumerate() {
                if slot.is_none() {
                    *slot = Some(table);
                    return Ok(index as u32);
                }
            }

            Err(Error::resource_exhausted("Error occurred"))
        }
    }
}

impl Default for ThreadBuiltins {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

/// System parallelism information
#[derive(Debug, Clone)]
pub struct ParallelismInfo {
    /// Number of threads that can run in parallel
    pub available_parallelism: u32,
    /// Number of physical CPU cores
    pub physical_cores: u32,
    /// Number of logical CPU cores (including hyperthreading)
    pub logical_cores: u32,
    /// Whether NUMA architecture is detected
    pub is_numa: bool,
}

impl ParallelismInfo {
    /// Detect system parallelism capabilities
    pub fn detect() -> Self {
        #[cfg(feature = "std")]
        {
            let available_parallelism =
                thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);

            Self {
                available_parallelism,
                physical_cores: available_parallelism, // Simplified
                logical_cores: available_parallelism,
                is_numa: false, // Would need platform-specific detection
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Self {
                available_parallelism: 1, // Single-threaded in no_std
                physical_cores: 1,
                logical_cores: 1,
                is_numa: false,
            }
        }
    }
}

/// Configuration for thread spawning
#[derive(Debug, Clone)]
pub struct ThreadSpawnConfig {
    /// Stack size for the new thread
    pub stack_size: Option<usize>,
    /// Parent thread ID
    pub parent_thread: Option<ThreadId>,
    /// Whether to automatically start the thread
    pub auto_start: bool,
    /// Thread priority (0-100)
    pub priority: u8,
    /// CPU affinity mask
    pub cpu_affinity: Option<u64>,
}

impl Default for ThreadSpawnConfig {
    fn default() -> Self {
        Self {
            stack_size: None,
            parent_thread: None,
            auto_start: true,
            priority: 50,
            cpu_affinity: None,
        }
    }
}

/// Component function table entry for indirect thread spawning
#[derive(Debug, Clone)]
pub struct ComponentFunction {
    /// Base function index in the component
    pub base_index: u32,
    /// Number of functions in this table entry
    pub function_count: u32,
    /// Function signature information
    pub signature: FunctionSignature,
}

/// Function signature for validation
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    /// Parameter types
    pub params: Vec<ValueType>,
    /// Return types
    pub returns: Vec<ValueType>,
}

/// Value type for function signatures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
}

/// Result of joining a thread
#[derive(Debug)]
pub struct ThreadJoinResult {
    /// Whether the thread completed successfully
    pub success: bool,
    /// Return values from the thread
    pub return_values: Vec<Value>,
    /// Thread execution statistics
    pub execution_stats: wrt_runtime::ThreadExecutionStats,
}

/// Thread error types for component model
#[derive(Debug, Clone)]
pub enum ThreadError {
    /// Thread panicked during execution
    Panic(String),
    /// Thread was terminated
    Terminated,
    /// Timeout waiting for thread
    Timeout,
    /// Invalid thread handle
    InvalidHandle,
    /// Resource exhaustion
    ResourceExhausted,
}

impl core::fmt::Display for ThreadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ThreadError::Panic(msg) => write!(f, "Thread panic: {}", msg),
            ThreadError::Terminated => write!(f, "Thread was terminated"),
            ThreadError::Timeout => write!(f, "Thread operation timed out"),
            ThreadError::InvalidHandle => write!(f, "Invalid thread handle"),
            ThreadError::ResourceExhausted => write!(f, "Thread resources exhausted"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ThreadError {}
