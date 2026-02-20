//! Execution-related types shared across the WRT ecosystem
//!
//! This module contains execution configuration and statistics types that are
//! used by both the runtime and component model implementations.

/// ASIL execution mode configuration
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ASILExecutionMode {
    /// Quality Management (no safety requirements)
    #[default]
    QM,
    /// ASIL Level A (lowest automotive safety integrity level)
    AsilA,
    /// ASIL Level B (medium automotive safety integrity level)
    AsilB,
    /// ASIL Level C (high automotive safety integrity level)
    AsilC,
    /// ASIL Level D (highest automotive safety integrity level)
    AsilD,
}

/// ASIL execution configuration
#[derive(Debug, Clone, Default)]
pub struct ASILExecutionConfig {
    /// ASIL execution mode
    pub mode:           ASILExecutionMode,
    /// Fuel limit for bounded execution
    pub fuel_limit:     u64,
    /// Memory limit in bytes
    pub memory_limit:   usize,
    /// Maximum call stack depth
    pub max_call_depth: u32,
}

impl ASILExecutionConfig {
    /// Create a new ASIL execution configuration
    pub fn new(mode: ASILExecutionMode) -> Self {
        let (fuel_limit, memory_limit, max_call_depth) = match mode {
            ASILExecutionMode::QM => (u64::MAX, usize::MAX, 1024),
            ASILExecutionMode::AsilA => (1_000_000, 16 * 1024 * 1024, 512),
            ASILExecutionMode::AsilB => (500_000, 8 * 1024 * 1024, 256),
            ASILExecutionMode::AsilC => (100_000, 4 * 1024 * 1024, 128),
            ASILExecutionMode::AsilD => (50_000, 2 * 1024 * 1024, 64),
        };

        Self {
            mode,
            fuel_limit,
            memory_limit,
            max_call_depth,
        }
    }

    /// Get the fuel limit for this configuration
    pub fn fuel_limit(&self) -> u64 {
        self.fuel_limit
    }

    /// Get the memory limit for this configuration
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }

    /// Get the maximum call depth for this configuration
    pub fn max_call_depth(&self) -> u32 {
        self.max_call_depth
    }
}

/// Execution statistics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// Number of instructions executed
    pub instructions_executed:  u64,
    /// Amount of fuel consumed
    pub fuel_consumed:          u64,
    /// Peak memory usage in bytes
    pub peak_memory_usage:      usize,
    /// Number of function calls made
    pub function_calls:         u32,
    /// Maximum call stack depth reached
    pub max_call_depth_reached: u32,
    /// Execution time in microseconds
    pub execution_time_us:      u64,
}

impl ExecutionStats {
    /// Create new empty execution statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all statistics to zero
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Add instruction count
    pub fn add_instructions(&mut self, count: u64) {
        self.instructions_executed = self.instructions_executed.saturating_add(count);
    }

    /// Add fuel consumption
    pub fn add_fuel(&mut self, fuel: u64) {
        self.fuel_consumed = self.fuel_consumed.saturating_add(fuel);
    }

    /// Update peak memory usage if current usage is higher
    pub fn update_peak_memory(&mut self, current_usage: usize) {
        if current_usage > self.peak_memory_usage {
            self.peak_memory_usage = current_usage;
        }
    }

    /// Record a function call
    pub fn record_function_call(&mut self, call_depth: u32) {
        self.function_calls = self.function_calls.saturating_add(1);
        if call_depth > self.max_call_depth_reached {
            self.max_call_depth_reached = call_depth;
        }
    }

    /// Set execution time
    pub fn set_execution_time(&mut self, time_us: u64) {
        self.execution_time_us = time_us;
    }
}

/// Extract resource limits configuration from WebAssembly binary
///
/// This function attempts to extract ASIL-compliant resource limits
/// from a WebAssembly binary's custom sections.
///
/// # Arguments
///
/// * `binary` - The WebAssembly binary data
/// * `asil_mode` - The target ASIL execution mode
///
/// # Returns
///
/// Returns `Ok(Some(config))` if resource limits are found and valid,
/// `Ok(None)` if no resource limits are found, or `Err` if the binary is
/// invalid.
pub fn extract_resource_limits_from_binary(
    _binary: &[u8],
    asil_mode: ASILExecutionMode,
) -> wrt_error::Result<Option<ASILExecutionConfig>> {
    // TODO: Implement actual resource limits extraction from custom sections
    // For now, return a default configuration based on ASIL mode
    Ok(Some(ASILExecutionConfig::new(asil_mode)))
}
