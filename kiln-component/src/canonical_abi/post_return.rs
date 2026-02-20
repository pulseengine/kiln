//! Post-return implementation for WebAssembly Component Model MVP
//!
//! This module implements the post-return canonical ABI functionality required
//! by the latest Component Model MVP specification for proper resource cleanup
//! and memory management after component function calls.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_error::{Error, ErrorCategory, Result};
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
use wrt_foundation::{
    budget_aware_provider::CrateId, collections::StaticVec as BoundedVec, safe_managed_alloc,
    safe_memory::NoStdProvider, traits::DefaultMemoryProvider, values::Value,
};

// Import prelude for std/no_std compatibility
use crate::prelude::*;
#[cfg(not(feature = "std"))]
use crate::types::Value as ComponentValue;
use crate::{
    bounded_component_infra::ComponentProvider,
    canonical_abi::canonical_options::CanonicalOptions,
    instance::Instance,
    // memory::Memory, // Module not available
    runtime::Runtime,
};

// Placeholder types for missing imports
pub type Memory = ();

/// Trait for executing cleanup functions
///
/// This trait allows the runtime layer to provide function execution capability
/// to the component layer without creating a dependency cycle.
pub trait PostReturnExecutor {
    /// Execute a post-return cleanup function
    ///
    /// # Arguments
    /// * `func_index` - The index of the function to call
    /// * `args` - The arguments to pass to the function
    ///
    /// # Returns
    /// Ok(()) on success, Err on failure
    fn execute_cleanup(&mut self, func_index: u32, args: &[Value]) -> Result<()>;
}

/// No-op executor for when no runtime is available
#[derive(Debug, Default)]
pub struct NoOpExecutor;

impl PostReturnExecutor for NoOpExecutor {
    fn execute_cleanup(&mut self, _func_index: u32, _args: &[Value]) -> Result<()> {
        // No-op: Used when actual execution is not available
        Ok(())
    }
}

/// Registry for managing post-return cleanup operations
#[derive(Debug, Default)]
pub struct PostReturnRegistry {
    entries: Vec<PostReturnEntry>,
}

impl PostReturnRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: PostReturnEntry) {
        self.entries.push(entry);
    }

    pub fn execute_cleanup(&mut self) -> Result<()> {
        // Placeholder implementation
        self.entries.clear();
        Ok(())
    }
}

/// Post-return cleanup entry tracking resources that need cleanup
#[derive(Debug, Clone)]
pub struct PostReturnEntry {
    /// Function index to call for cleanup
    pub func_index: u32,
    /// Arguments to pass to the cleanup function
    #[cfg(feature = "std")]
    pub args: Vec<ComponentValue<DefaultMemoryProvider>>,
    #[cfg(not(feature = "std"))]
    pub args: BoundedVec<ComponentValue, 16>,
    /// Priority of this cleanup operation (higher = more critical)
    pub priority: CleanupPriority,
    /// Resource type being cleaned up
    pub resource_type: ResourceType,
}

/// Priority levels for post-return cleanup operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CleanupPriority {
    /// Low priority - optional cleanup
    Low = 0,
    /// Normal priority - standard cleanup
    Normal = 1,
    /// High priority - important cleanup
    High = 2,
    /// Critical priority - must not fail
    Critical = 3,
}

/// Type of resource being cleaned up
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Memory allocations
    Memory,
    /// File handles
    File,
    /// Network connections
    Network,
    /// Component instances
    Component,
    /// Stream handles
    Stream,
    /// Future handles
    Future,
    /// Generic resource
    Generic,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Memory => write!(f, "memory"),
            ResourceType::File => write!(f, "file"),
            ResourceType::Network => write!(f, "network"),
            ResourceType::Component => write!(f, "component"),
            ResourceType::Stream => write!(f, "stream"),
            ResourceType::Future => write!(f, "future"),
            ResourceType::Generic => write!(f, "generic"),
        }
    }
}

/// Post-return execution context
#[derive(Debug)]
pub struct PostReturnContext {
    /// Cleanup entries pending execution
    #[cfg(feature = "std")]
    entries: Vec<PostReturnEntry>,
    #[cfg(not(feature = "std"))]
    entries: BoundedVec<PostReturnEntry, 256>,

    /// Whether post-return is currently executing
    is_executing: bool,

    /// Error recovery mode
    error_recovery: ErrorRecoveryMode,

    /// Statistics for post-return operations
    stats: PostReturnStats,
}

/// Error recovery strategy for post-return operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorRecoveryMode {
    /// Continue executing remaining cleanup operations on error
    Continue,
    /// Stop on first error
    StopOnError,
    /// Best effort - try all operations but collect errors
    BestEffort,
}

/// Statistics for post-return operations
#[derive(Debug, Default, Clone)]
pub struct PostReturnStats {
    /// Total number of cleanup operations executed
    pub operations_executed: u64,
    /// Number of successful operations
    pub operations_successful: u64,
    /// Number of failed operations
    pub operations_failed: u64,
    /// Total time spent in post-return (microseconds)
    pub total_time_us: u64,
    /// Maximum single operation time (microseconds)
    pub max_operation_time_us: u64,
}

impl Default for PostReturnContext {
    fn default() -> Self {
        // For Default trait, we prefer to panic rather than return invalid state
        // This ensures capability-based allocation is always properly initialized
        Self::new().expect("Failed to create PostReturnContext with proper capability allocation")
    }
}

impl PostReturnContext {
    /// Create a new post-return context
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            entries: Vec::new(),
            #[cfg(not(feature = "std"))]
            entries: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            is_executing: false,
            error_recovery: ErrorRecoveryMode::BestEffort,
            stats: PostReturnStats::default(),
        })
    }

    /// Add a cleanup entry to be executed during post-return
    pub fn add_cleanup(&mut self, entry: PostReturnEntry) -> Result<()> {
        if self.is_executing {
            return Err(Error::runtime_invalid_state(
                "cannot_add_cleanup_during_post_return",
            ));
        }

        #[cfg(feature = "std")]
        {
            self.entries.push(entry);
        }
        #[cfg(not(feature = "std"))]
        {
            self.entries
                .push(entry)
                .map_err(|_| Error::resource_exhausted("post_return_cleanup_queue_full"))?;
        }

        // Sort by priority (highest first)
        #[cfg(feature = "std")]
        {
            self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
        #[cfg(not(feature = "std"))]
        {
            // Simple bubble sort for no_std
            let len = self.entries.len();
            for i in 0..len {
                for j in 0..len - 1 - i {
                    if self.entries.get(j).unwrap().priority
                        < self.entries.get(j + 1).unwrap().priority
                    {
                        // Manual swap for BoundedVec
                        let temp = self.entries.get(j).unwrap().clone();
                        *self.entries.get_mut(j).unwrap() =
                            self.entries.get(j + 1).unwrap().clone();
                        *self.entries.get_mut(j + 1).unwrap() = temp;
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute all pending post-return cleanup operations (legacy, uses no-op executor)
    pub fn execute_post_return(
        &mut self,
        instance: &mut Instance,
        memory: &mut Memory,
        options: &CanonicalOptions,
    ) -> Result<()> {
        // Use a no-op executor for backward compatibility
        let mut executor = NoOpExecutor;
        self.execute_post_return_with_executor(instance, memory, options, &mut executor)
    }

    /// Execute all pending post-return cleanup operations with a provided executor
    ///
    /// This method allows the runtime layer to provide actual function execution
    /// capability through the `PostReturnExecutor` trait.
    pub fn execute_post_return_with_executor<E: PostReturnExecutor>(
        &mut self,
        instance: &mut Instance,
        memory: &mut Memory,
        options: &CanonicalOptions,
        executor: &mut E,
    ) -> Result<()> {
        if self.is_executing {
            return Err(Error::runtime_invalid_state(
                "post_return_already_executing",
            ));
        }

        if !options.has_post_return() {
            return Ok(()); // No post-return function configured
        }

        self.is_executing = true;
        let start_time = current_time_us();
        let mut errors = Vec::new();

        // Execute each cleanup operation in priority order
        let entries = mem::take(&mut self.entries);
        for entry in entries {
            let operation_start = current_time_us();

            match self.execute_cleanup_entry_with_executor(instance, memory, &entry, executor) {
                Ok(()) => {
                    self.stats.operations_successful += 1;
                },
                Err(e) => {
                    self.stats.operations_failed += 1;
                    errors.push((entry.resource_type, e));

                    match self.error_recovery {
                        ErrorRecoveryMode::StopOnError => break,
                        ErrorRecoveryMode::Continue | ErrorRecoveryMode::BestEffort => continue,
                    }
                },
            }

            let operation_time = current_time_us() - operation_start;
            if operation_time > self.stats.max_operation_time_us {
                self.stats.max_operation_time_us = operation_time;
            }
            self.stats.operations_executed += 1;
        }

        self.stats.total_time_us += current_time_us() - start_time;
        self.is_executing = false;

        // Handle collected errors based on recovery mode
        if !errors.is_empty() && self.error_recovery == ErrorRecoveryMode::StopOnError {
            let (_resource_type, _error) = errors.into_iter().next().unwrap();
            return Err(Error::runtime_execution_error("Post-return cleanup failed"));
        }

        Ok(())
    }

    /// Execute a single cleanup entry (legacy, uses no-op)
    fn execute_cleanup_entry(
        &self,
        instance: &mut Instance,
        memory: &mut Memory,
        entry: &PostReturnEntry,
    ) -> Result<()> {
        let mut executor = NoOpExecutor;
        self.execute_cleanup_entry_with_executor(instance, memory, entry, &mut executor)
    }

    /// Execute a single cleanup entry with a provided executor
    fn execute_cleanup_entry_with_executor<E: PostReturnExecutor>(
        &self,
        _instance: &mut Instance,
        _memory: &mut Memory,
        entry: &PostReturnEntry,
        executor: &mut E,
    ) -> Result<()> {
        // Convert ComponentValue args to raw values for function call
        let mut raw_args = Vec::new();

        #[cfg(feature = "std")]
        {
            for arg in &entry.args {
                raw_args.push(self.component_value_to_raw(arg)?);
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for i in 0..entry.args.len() {
                if let Some(arg) = entry.args.get(i) {
                    raw_args.push(self.component_value_to_raw(arg)?);
                }
            }
        }

        // Call the cleanup function via the executor
        executor.execute_cleanup(entry.func_index, &raw_args)
    }

    /// Convert ComponentValue to raw value for function calls
    #[cfg(feature = "std")]
    fn component_value_to_raw(
        &self,
        value: &ComponentValue<DefaultMemoryProvider>,
    ) -> Result<Value> {
        match value {
            ComponentValue::Bool(b) => Ok(Value::I32(if *b { 1 } else { 0 })),
            ComponentValue::S8(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::U8(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S16(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::U16(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S32(v) => Ok(Value::I32(*v)),
            ComponentValue::U32(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S64(v) => Ok(Value::I64(*v)),
            ComponentValue::U64(v) => Ok(Value::I64(*v as i64)),
            ComponentValue::F32(v) => Ok(Value::F32(wrt_foundation::FloatBits32::from_bits(
                v.to_bits(),
            ))),
            ComponentValue::F64(v) => Ok(Value::F64(wrt_foundation::FloatBits64::from_bits(
                v.to_bits(),
            ))),
            ComponentValue::String(s) => {
                // For strings, we typically pass pointer and length
                // This is a simplified version - real implementation would need
                // proper memory management
                Ok(Value::I32(s.as_ptr() as i32))
            },
            _ => Err(Error::runtime_execution_error(
                "unsupported_component_value_type",
            )),
        }
    }

    #[cfg(not(feature = "std"))]
    fn component_value_to_raw(&self, value: &ComponentValue) -> Result<Value> {
        match value {
            ComponentValue::Bool(b) => Ok(Value::I32(if *b { 1 } else { 0 })),
            ComponentValue::S8(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::U8(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S16(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::U16(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S32(v) => Ok(Value::I32(*v)),
            ComponentValue::U32(v) => Ok(Value::I32(*v as i32)),
            ComponentValue::S64(v) => Ok(Value::I64(*v)),
            ComponentValue::U64(v) => Ok(Value::I64(*v as i64)),
            ComponentValue::F32(v) => Ok(Value::F32(wrt_foundation::FloatBits32::from_bits(
                v.to_bits(),
            ))),
            ComponentValue::F64(v) => Ok(Value::F64(wrt_foundation::FloatBits64::from_bits(
                v.to_bits(),
            ))),
            #[cfg(feature = "std")]
            ComponentValue::String(s) => {
                // For strings, we typically pass pointer and length
                // This is a simplified version - real implementation would need
                // proper memory management
                Ok(Value::I32(s.as_ptr() as i32))
            },
            #[cfg(not(feature = "std"))]
            ComponentValue::String(s) => {
                if let Ok(str_ref) = s.as_str() {
                    Ok(Value::I32(str_ref.as_ptr() as i32))
                } else {
                    Err(Error::runtime_execution_error("Invalid string"))
                }
            },
            _ => Err(Error::runtime_execution_error(
                "unsupported_component_value_type",
            )),
        }
    }

    /// Set error recovery mode
    pub fn set_error_recovery(&mut self, mode: ErrorRecoveryMode) {
        self.error_recovery = mode;
    }

    /// Get current statistics
    pub fn stats(&self) -> &PostReturnStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = PostReturnStats::default();
    }

    /// Check if post-return is currently executing
    pub fn is_executing(&self) -> bool {
        self.is_executing
    }

    /// Get number of pending cleanup operations
    pub fn pending_operations(&self) -> usize {
        self.entries.len()
    }
}

/// Helper functions for post-return management
pub mod helpers {
    use super::*;

    /// Create a memory cleanup entry
    pub fn create_memory_cleanup(func_index: u32, ptr: u32, size: u32) -> Result<PostReturnEntry> {
        Ok(PostReturnEntry {
            func_index,
            #[cfg(feature = "std")]
            args: vec![ComponentValue::U32(ptr), ComponentValue::U32(size)],
            #[cfg(not(feature = "std"))]
            args: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                let mut args = BoundedVec::new().unwrap();
                args.push(ComponentValue::U32(ptr))?;
                args.push(ComponentValue::U32(size))?;
                args
            },
            priority: CleanupPriority::High,
            resource_type: ResourceType::Memory,
        })
    }

    /// Create a resource handle cleanup entry
    pub fn create_resource_cleanup(
        func_index: u32,
        handle: u32,
        resource_type: ResourceType,
    ) -> Result<PostReturnEntry> {
        Ok(PostReturnEntry {
            func_index,
            #[cfg(feature = "std")]
            args: vec![ComponentValue::U32(handle)],
            #[cfg(not(feature = "std"))]
            args: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                let mut args = BoundedVec::new().unwrap();
                args.push(ComponentValue::U32(handle))?;
                args
            },
            priority: CleanupPriority::Critical,
            resource_type,
        })
    }

    /// Create a component cleanup entry
    pub fn create_component_cleanup(func_index: u32, instance_id: u32) -> Result<PostReturnEntry> {
        Ok(PostReturnEntry {
            func_index,
            #[cfg(feature = "std")]
            args: vec![ComponentValue::U32(instance_id)],
            #[cfg(not(feature = "std"))]
            args: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                let mut args = BoundedVec::new().unwrap();
                args.push(ComponentValue::U32(instance_id))?;
                args
            },
            priority: CleanupPriority::Normal,
            resource_type: ResourceType::Component,
        })
    }
}

/// Get current time in microseconds (platform-specific)
fn current_time_us() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
    }
    #[cfg(not(feature = "std"))]
    {
        // ASIL-D safe: Use atomic counter instead of unsafe static mut
        use core::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}
