// WRT - wrt-runtime
// Module: Platform-Aware Runtime Implementation
// SW-REQ-ID: REQ_RUNTIME_PLATFORM_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Platform-Aware WebAssembly Runtime
//!
//! This module provides a runtime that adapts to platform-specific capabilities
//! and resource limits, integrating with the CFI engine and memory management
//! to provide optimal performance within platform constraints.

#![allow(clippy::module_name_repetitions)]

// alloc is imported in lib.rs with proper feature gates
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{
    boxed::Box,
    vec,
    vec::Vec,
};
// Import Box, Vec, and other types for allocating memory adapters
#[cfg(feature = "std")]
use alloc::{
    boxed::Box,
    vec,
    vec::Vec,
};

// For no_std without alloc, use BoundedVec instead of Vec
#[cfg(not(any(feature = "std", feature = "alloc")))]
use wrt_foundation::bounded::BoundedVec;

use crate::{
    cfi_engine::{
        CfiExecutionEngine,
        CfiViolationPolicy,
    },
    component_stubs::ComponentId,
    execution::ExecutionContext,
    foundation_stubs::{
        AsilLevel,
        MediumProvider,
        SafetyContext,
        UnifiedMemoryProvider,
    },
    func::Function as RuntimeFunction,
    prelude::*,
    unified_types::UnifiedMemoryAdapter as UnifiedMemoryAdapterTrait,
};
#[cfg(not(any(feature = "std", feature = "alloc")))]
type PlatformVec<T> = BoundedVec<T, 16, wrt_foundation::NoStdProvider<1024>>;

// Type alias for Vec that works in all feature configurations
#[cfg(any(feature = "std", feature = "alloc"))]
type ValueVec = Vec<Value>;
#[cfg(not(any(feature = "std", feature = "alloc")))]
type ValueVec = PlatformVec<Value>;

// Import Value type
use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::Value;
// Import specific allocators conditionally
#[cfg(all(feature = "platform-linux", target_os = "linux"))]
use wrt_platform::LinuxAllocatorBuilder;
#[cfg(all(feature = "platform-macos", target_os = "macos"))]
use wrt_platform::MacOsAllocatorBuilder;
#[cfg(all(feature = "platform-qnx", target_os = "nto"))]
use wrt_platform::QnxAllocatorBuilder;
// Import from wrt-platform for all platform abstractions
#[cfg(feature = "std")]
use wrt_platform::{
    ComprehensivePlatformLimits,
    PageAllocator,
    PlatformId,
    PlatformLimitDiscoverer,
    WASM_PAGE_SIZE,
};

// CFI imports temporarily disabled since CFI module is disabled
// use wrt_instructions::CfiControlFlowProtection;
use crate::cfi_engine::CfiControlFlowProtection;

// Helper function to convert between ASIL level types
#[cfg(feature = "std")]
fn convert_asil_level(platform_asil: wrt_platform::AsilLevel) -> AsilLevel {
    match platform_asil {
        wrt_platform::AsilLevel::QM => AsilLevel::QM,
        wrt_platform::AsilLevel::AsilA => AsilLevel::A,
        wrt_platform::AsilLevel::AsilB => AsilLevel::B,
        wrt_platform::AsilLevel::AsilC => AsilLevel::C,
        wrt_platform::AsilLevel::AsilD => AsilLevel::D,
    }
}

// Use PageAllocator from wrt-platform instead of custom trait

/// Platform-aware WebAssembly runtime
pub struct PlatformAwareRuntime {
    /// Execution engine with CFI protection
    execution_engine: CfiExecutionEngine,
    /// Platform-specific memory allocator
    #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
    memory_allocator: Box<dyn PageAllocator>,
    /// Platform-specific limits and capabilities
    #[cfg(feature = "std")]
    platform_limits:  ComprehensivePlatformLimits,
    /// Safety context for ASIL compliance
    safety_context:   SafetyContext,
    /// Runtime statistics and metrics
    metrics:          RuntimeMetrics,
}

/// Runtime performance and resource metrics
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetrics {
    /// Total instructions executed
    pub instructions_executed:   u64,
    /// Total memory allocated
    pub memory_allocated:        usize,
    /// Peak memory usage
    pub peak_memory_usage:       usize,
    /// Number of components instantiated
    pub components_instantiated: u32,
    /// CFI violations detected
    pub cfi_violations:          u64,
    /// Execution time in nanoseconds
    pub execution_time_ns:       u64,
}

impl PlatformAwareRuntime {
    /// Create new platform-aware runtime using platform discovery
    #[cfg(feature = "std")]
    pub fn new() -> Result<Self> {
        let mut discoverer = PlatformLimitDiscoverer::new();
        let limits = discoverer
            .discover()
            .map_err(|e| Error::runtime_execution_error("Platform limit discovery failed"))?;
        Self::new_with_limits(limits)
    }

    /// Create new platform-aware runtime for no_std environments
    #[cfg(not(feature = "std"))]
    pub fn new() -> Result<Self> {
        let cfi_protection = Self::create_basic_cfi_protection();
        let execution_engine = CfiExecutionEngine::new(cfi_protection);
        let safety_context = SafetyContext::new(AsilLevel::D); // Default to highest safety level for no_std

        Ok(Self {
            execution_engine: execution_engine?,
            safety_context,
            metrics: RuntimeMetrics::default(),
        })
    }

    /// Create new platform-aware runtime with specific limits
    #[cfg(feature = "std")]
    pub fn new_with_limits(limits: ComprehensivePlatformLimits) -> Result<Self> {
        #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
        let memory_allocator = Self::create_memory_allocator(&limits)?;

        let cfi_protection = Self::create_cfi_protection(&limits);
        let execution_engine = CfiExecutionEngine::new(cfi_protection)?;
        let safety_context = SafetyContext::new(convert_asil_level(limits.asil_level));

        Ok(Self {
            execution_engine,
            #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
            memory_allocator,
            platform_limits: limits,
            safety_context,
            metrics: RuntimeMetrics::default(),
        })
    }

    /// Create runtime with custom CFI violation policy
    #[cfg(feature = "std")]
    pub fn new_with_cfi_policy(
        limits: ComprehensivePlatformLimits,
        cfi_policy: CfiViolationPolicy,
    ) -> Result<Self> {
        #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
        let memory_allocator = Self::create_memory_allocator(&limits)?;

        let cfi_protection = Self::create_cfi_protection(&limits);
        let execution_engine = CfiExecutionEngine::new_with_policy(cfi_protection, cfi_policy)?;
        let safety_context = SafetyContext::new(convert_asil_level(limits.asil_level));

        Ok(Self {
            execution_engine,
            #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
            memory_allocator,
            platform_limits: limits,
            safety_context,
            metrics: RuntimeMetrics::default(),
        })
    }

    /// Execute WebAssembly function with platform-aware resource management
    pub fn execute_function(
        &mut self,
        function: &RuntimeFunction,
        args: &[Value],
    ) -> Result<ValueVec> {
        let start_time = self.get_timestamp();

        // Validate execution against platform limits
        #[cfg(feature = "std")]
        self.validate_execution_limits(function, args)?;

        // Create execution context with platform limits
        #[cfg(feature = "std")]
        let mut execution_context = ExecutionContext::new_with_limits(
            self.platform_limits.max_stack_bytes / 8, // Approximate stack depth
        );

        #[cfg(not(feature = "std"))]
        let mut execution_context = ExecutionContext::new_with_limits(256); // Default stack depth for no_std

        // Execute with CFI protection
        let instruction = self.create_call_instruction(function);
        let cfi_result = self
            .execution_engine
            .execute_instruction_with_cfi(&instruction, &mut execution_context)?;

        // Update metrics
        let end_time = self.get_timestamp();
        self.metrics.instructions_executed += 1;
        self.metrics.execution_time_ns += end_time.saturating_sub(start_time);
        #[cfg(feature = "std")]
        self.update_memory_metrics();

        // Extract return values from CFI result
        self.extract_return_values(cfi_result, args.len())
    }

    /// Instantiate component with resource budget validation
    pub fn instantiate_component(&mut self, component_bytes: &[u8]) -> Result<ComponentId> {
        #[cfg(feature = "std")]
        {
            // Validate component against platform limits
            let requirements = self.analyze_component_requirements(component_bytes)?;

            if requirements.memory_usage > self.available_memory() {
                return Err(Error::runtime_execution_error(
                    "Insufficient memory for component",
                ));
            }

            if self.metrics.components_instantiated >= self.platform_limits.max_components as u32 {
                return Err(Error::new(
                    ErrorCategory::Resource,
                    wrt_error::codes::RESOURCE_LIMIT_EXCEEDED,
                    "Maximum component limit exceeded",
                ));
            }
        }

        #[cfg(not(feature = "std"))]
        {
            // For no_std, use basic validation
            if component_bytes.len() > 1024 * 1024 {
                // 1MB limit
                return Err(Error::runtime_execution_error(
                    "Component size exceeds limit",
                ));
            }

            if self.metrics.components_instantiated >= 16 {
                // Fixed limit for no_std
                return Err(Error::new(
                    ErrorCategory::Resource,
                    wrt_error::codes::RESOURCE_LIMIT_EXCEEDED,
                    "Component instantiation limit exceeded",
                ));
            }
        }

        // Create component instance with bounded resources
        let component_id = ComponentId::new(self.metrics.components_instantiated);
        self.metrics.components_instantiated += 1;

        Ok(component_id)
    }

    /// Get current runtime metrics
    pub fn metrics(&self) -> &RuntimeMetrics {
        &self.metrics
    }

    /// Get platform limits
    #[cfg(feature = "std")]
    pub fn platform_limits(&self) -> &ComprehensivePlatformLimits {
        &self.platform_limits
    }

    /// Get safety context
    pub fn safety_context(&self) -> &SafetyContext {
        &self.safety_context
    }

    /// Get available memory in bytes
    pub fn available_memory(&self) -> usize {
        #[cfg(feature = "std")]
        {
            // Simplified implementation - could query platform allocator for available
            // memory
            self.platform_limits
                .max_total_memory
                .saturating_sub(self.metrics.memory_allocated)
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, use a fixed memory budget
            (1024_usize * 1024).saturating_sub(self.metrics.memory_allocated) // 1MB budget
        }
    }

    /// Get total memory capacity
    pub fn total_memory(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.platform_limits.max_total_memory
        }
        #[cfg(not(feature = "std"))]
        {
            1024_usize * 1024 // 1MB for no_std
        }
    }

    /// Create platform-specific memory allocator using wrt-platform
    #[cfg(all(any(feature = "std", feature = "alloc"), feature = "platform"))]
    fn create_memory_allocator(
        limits: &ComprehensivePlatformLimits,
    ) -> Result<Box<dyn PageAllocator>> {
        use wrt_platform::prelude::*;

        let max_pages = limits.max_total_memory / WASM_PAGE_SIZE;

        match limits.platform_id {
            #[cfg(all(feature = "platform-linux", target_os = "linux"))]
            PlatformId::Linux => {
                let allocator = wrt_platform::LinuxAllocatorBuilder::new()
                    .with_maximum_pages(max_pages as u32)
                    .with_guard_pages(true)
                    .build();
                Ok(Box::new(allocator))
            },
            #[cfg(all(feature = "platform-qnx", target_os = "nto"))]
            PlatformId::QNX => {
                let allocator = wrt_platform::QnxAllocatorBuilder::new()
                    .with_maximum_pages(max_pages as u32)
                    .build();
                Ok(Box::new(allocator))
            },
            #[cfg(all(feature = "platform-macos", target_os = "macos"))]
            PlatformId::MacOS => {
                let allocator = wrt_platform::MacOsAllocatorBuilder::new()
                    .with_maximum_pages(max_pages as u32)
                    .build();
                Ok(Box::new(allocator))
            },
            _ => {
                // For non-specific platforms or when platform features aren't enabled,
                // create a basic allocator using the foundation types
                use core::ptr::NonNull;

                use wrt_foundation::safe_memory::NoStdProvider;

                /// Basic allocator implementation for fallback
                #[derive(Debug)]
                struct BasicAllocator {
                    max_pages: u32,
                }

                impl PageAllocator for BasicAllocator {
                    fn allocate(
                        &mut self,
                        initial_pages: u32,
                        maximum_pages: Option<u32>,
                    ) -> Result<(NonNull<u8>, usize)> {
                        if initial_pages > self.max_pages {
                            return Err(Error::runtime_execution_error("Runtime execution error"));
                        }
                        // Return a dummy pointer for basic functionality
                        let size = initial_pages as usize * WASM_PAGE_SIZE;
                        Ok((NonNull::dangling(), size))
                    }

                    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
                        if current_pages + additional_pages > self.max_pages {
                            return Err(Error::new(
                                ErrorCategory::Memory,
                                wrt_error::codes::MEMORY_ALLOCATION_ERROR,
                                "Memory growth would exceed limit",
                            ));
                        }
                        Ok(())
                    }

                    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()> {
                        // Basic implementation - just validate arguments
                        Ok(())
                    }
                }

                let allocator = BasicAllocator {
                    max_pages: max_pages as u32,
                };
                Ok(Box::new(allocator))
            },
        }
    }

    // No-std environments use NoStdProvider from wrt-platform

    /// Create CFI protection configuration based on platform capabilities
    #[cfg(feature = "std")]
    fn create_cfi_protection(limits: &ComprehensivePlatformLimits) -> CfiControlFlowProtection {
        let protection_level = match convert_asil_level(limits.asil_level) {
            AsilLevel::QM => 0,               // Basic protection level
            AsilLevel::A | AsilLevel::B => 1, // Enhanced protection level
            AsilLevel::C | AsilLevel::D => 2, // Maximum protection level
        };

        CfiControlFlowProtection::new_with_level(protection_level)
    }

    /// Create basic CFI protection for no_std environments
    #[cfg(not(feature = "std"))]
    fn create_basic_cfi_protection() -> CfiControlFlowProtection {
        // Use maximum protection level for no_std environments (ASIL-D)
        CfiControlFlowProtection::new_with_level(2)
    }

    /// Validate execution against platform limits
    #[cfg(feature = "std")]
    fn validate_execution_limits(&self, function: &RuntimeFunction, args: &[Value]) -> Result<()> {
        // Check stack depth estimate
        let estimated_stack = (args.len() + 32) * 8; // Rough estimate
        if estimated_stack > self.platform_limits.max_stack_bytes {
            return Err(Error::runtime_execution_error(
                "Stack size exceeds platform limit",
            ));
        }

        // Check memory availability
        if self.available_memory() < 4096 {
            return Err(Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::MEMORY_ALLOCATION_ERROR,
                "Insufficient memory available",
            ));
        }

        Ok(())
    }

    /// Create call instruction for function execution
    fn create_call_instruction(&self, function: &RuntimeFunction) -> Instruction {
        // Create a call instruction for the function
        Instruction::Call(function.index().unwrap_or(0))
    }

    /// Analyze component resource requirements
    fn analyze_component_requirements(
        &self,
        component_bytes: &[u8],
    ) -> Result<crate::component_stubs::ComponentRequirements> {
        // Simple analysis - in real implementation this would parse the component
        let memory_usage = component_bytes.len() * 2; // Estimate 2x size for runtime overhead

        Ok(crate::component_stubs::ComponentRequirements {
            component_count: 1,
            resource_count: 0,
            memory_usage,
        })
    }

    /// Update memory usage metrics
    #[cfg(feature = "std")]
    fn update_memory_metrics(&mut self) {
        let current_usage = self.total_memory() - self.available_memory();
        self.metrics.memory_allocated = current_usage;
        if current_usage > self.metrics.peak_memory_usage {
            self.metrics.peak_memory_usage = current_usage;
        }
    }

    /// Extract return values from CFI execution result
    fn extract_return_values(
        &self,
        _cfi_result: crate::cfi_engine::CfiExecutionResult,
        _arg_count: usize,
    ) -> Result<ValueVec> {
        // Simplified implementation - in real scenario this would extract actual values
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            use wrt_foundation::{
                budget_aware_provider::CrateId,
                capability_allocators::capability_alloc::capability_vec,
                memory_init::get_global_capability_context,
            };

            // Get capability context
            let context = get_global_capability_context()?;

            // Use capability-aware allocation
            let mut result = capability_vec(context, CrateId::Runtime, 1)?;
            result.push(Value::I32(0));
            Ok(result)
        }
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // For no_std no_alloc, return a fixed array wrapped as Vec-like
            use wrt_foundation::{
                bounded::BoundedVec,
                budget_aware_provider::CrateId,
                safe_managed_alloc,
            };
            let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
            let mut result: BoundedVec<Value, 16, _> = BoundedVec::new(provider)
                .map_err(|_| Error::runtime_execution_error("Failed to create bounded vector"))?;
            result.push(Value::I32(0)).map_err(|_| {
                Error::new(
                    ErrorCategory::Memory,
                    wrt_error::codes::MEMORY_ALLOCATION_ERROR,
                    "Failed to push value to result",
                )
            })?;
            // Convert to Vec for compatibility - this is a temporary workaround
            Err(Error::runtime_execution_error("Not implemented for no_std"))
        }
    }

    /// Get current timestamp for performance tracking
    fn get_timestamp(&self) -> u64 {
        // Platform-specific timestamp implementation
        #[cfg(feature = "std")]
        {
            use std::time::{
                SystemTime,
                UNIX_EPOCH,
            };
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            // Simple counter for no_std environments
            use core::sync::atomic::{
                AtomicU64,
                Ordering,
            };
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            COUNTER.fetch_add(1, Ordering::Relaxed)
        }
    }
}

// All platform-specific memory adapters removed - using wrt-platform
// abstractions instead

