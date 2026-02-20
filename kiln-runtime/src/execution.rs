//! WebAssembly Execution Context and Statistics
//!
//! This module provides the core execution context for WebAssembly modules,
//! including execution statistics tracking, resource monitoring, and execution
//! state management.
//!
//! # Core Components
//!
//! - `ExecutionContext`: Main execution state including value stack and call
//!   frames
//! - `ExecutionStatistics`: Performance metrics and resource usage tracking
//! - Stack depth management with configurable limits
//! - Integration with the interpreter for instruction execution
//!
//! # Safety
//!
//! All execution operations are bounds-checked and memory-safe, preventing
//! stack overflows and maintaining WebAssembly's sandboxing guarantees.

// alloc is imported in lib.rs with proper feature gates

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::format;
// Import format! macro for string formatting
#[cfg(feature = "std")]
use std::format;

use crate::prelude::{
    str,
    Debug,
    Error,
    ErrorCategory,
    Ord,
    Result,
};

/// Structure to track execution statistics
#[derive(Debug, Default, Clone)]
pub struct ExecutionStats {
    /// Number of instructions executed
    pub instructions_executed:    u64,
    /// Memory usage in bytes
    pub memory_usage:             usize,
    /// Maximum stack depth reached
    pub max_stack_depth:          usize,
    /// Number of function calls
    pub function_calls:           u64,
    /// Number of memory reads
    pub memory_reads:             u64,
    /// Number of memory writes
    pub memory_writes:            u64,
    /// Execution time in microseconds
    pub execution_time_us:        u64,
    /// Gas used (if metering is enabled)
    pub gas_used:                 u64,
    /// Gas limit (if metering is enabled)
    pub gas_limit:                u64,
    /// Number of SIMD operations executed
    pub simd_operations_executed: u64,
}

impl ExecutionStats {
    /// Create a new instance with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all statistics to zero
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Increment the instruction count
    pub fn increment_instructions(&mut self, count: u64) {
        self.instructions_executed = self.instructions_executed.saturating_add(count);
    }

    /// Update memory usage
    pub fn update_memory_usage(&mut self, bytes: usize) {
        self.memory_usage = self.memory_usage.saturating_add(bytes);
    }

    /// Update maximum stack depth
    pub fn update_stack_depth(&mut self, depth: usize) {
        self.max_stack_depth = self.max_stack_depth.max(depth);
    }

    /// Increment function call count
    pub fn increment_function_calls(&mut self, count: u64) {
        self.function_calls = self.function_calls.saturating_add(count);
    }

    /// Increment memory read count
    pub fn increment_memory_reads(&mut self, count: u64) {
        self.memory_reads = self.memory_reads.saturating_add(count);
    }

    /// Increment memory write count
    pub fn increment_memory_writes(&mut self, count: u64) {
        self.memory_writes = self.memory_writes.saturating_add(count);
    }

    /// Update execution time
    pub fn update_execution_time(&mut self, time_us: u64) {
        self.execution_time_us = self.execution_time_us.saturating_add(time_us);
    }

    /// Check if gas limit is exceeded
    #[must_use]
    pub fn is_gas_exceeded(&self) -> bool {
        self.gas_limit > 0 && self.gas_used >= self.gas_limit
    }

    /// Use gas and check if limit is exceeded
    pub fn use_gas(&mut self, amount: u64) -> Result<()> {
        self.gas_used = self.gas_used.saturating_add(amount);

        if self.is_gas_exceeded() {
            return Err(Error::runtime_execution_error("Gas limit exceeded"));
        }

        Ok(())
    }

    /// Set gas limit
    pub fn set_gas_limit(&mut self, limit: u64) {
        self.gas_limit = limit;
    }
}

/// Execution context containing state for a running WebAssembly instance
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Execution statistics
    pub stats:              ExecutionStats,
    /// Whether execution is currently trapped
    pub trapped:            bool,
    /// Current function depth
    pub function_depth:     usize,
    /// Maximum allowed function depth
    pub max_function_depth: usize,
}

impl ExecutionContext {
    /// Create a new execution context
    #[must_use]
    pub fn new(max_function_depth: usize) -> Self {
        Self {
            stats: ExecutionStats::default(),
            trapped: false,
            function_depth: 0,
            max_function_depth,
        }
    }

    /// Create execution context with platform-aware limits
    #[must_use]
    pub fn new_with_limits(max_function_depth: usize) -> Self {
        Self::new(max_function_depth)
    }

    /// Create execution context from platform limits
    #[must_use]
    pub fn from_platform_limits(platform_limits: &wrt_foundation::PlatformLimits) -> Self {
        let max_depth = platform_limits.max_stack / (8 * 64); // Estimate stack depth
        Self::new(max_depth.max(16)) // Minimum depth of 16
    }

    /// Enter a function
    pub fn enter_function(&mut self) -> Result<()> {
        self.function_depth += 1;

        if self.function_depth > self.max_function_depth {
            self.trapped = true;
            return Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::CALL_STACK_EXHAUSTED,
                "Function call depth exceeded maximum limit",
            ));
        }

        self.stats.increment_function_calls(1);
        self.stats.update_stack_depth(self.function_depth);

        Ok(())
    }

    /// Exit a function
    pub fn exit_function(&mut self) {
        if self.function_depth > 0 {
            self.function_depth -= 1;
        }
    }

    /// Check if execution is trapped
    #[must_use]
    pub fn is_trapped(&self) -> bool {
        self.trapped
    }

    /// Set trapped state
    pub fn set_trapped(&mut self, trapped: bool) {
        self.trapped = trapped;
    }
}

/// Placeholder for call frame information
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Function index
    pub function_index: u32,
    /// Program counter
    pub pc:             usize,
    /// Local variables count
    pub locals_count:   u32,
}

impl CallFrame {
    /// Create a new call frame
    #[must_use]
    pub fn new(function_index: u32, pc: usize, locals_count: u32) -> Self {
        Self {
            function_index,
            pc,
            locals_count,
        }
    }
}

/// Placeholder for instrumentation point
#[derive(Debug, Clone)]
pub struct InstrumentationPoint {
    /// Location in code
    pub location:   usize,
    /// Type of instrumentation
    pub point_type: wrt_foundation::bounded::BoundedString<64>,
}

impl InstrumentationPoint {
    /// Create a new instrumentation point
    pub fn new(location: usize, point_type: &str) -> Result<Self> {
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;
        let bounded_point_type: wrt_foundation::bounded::BoundedString<
            64,
        > = wrt_foundation::bounded::BoundedString::from_str_truncate(point_type)
            .unwrap_or_else(|_| {
                wrt_foundation::bounded::BoundedString::from_str_truncate("").unwrap()
            });
        Ok(Self {
            location,
            point_type: bounded_point_type,
        })
    }
}
