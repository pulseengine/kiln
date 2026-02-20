// WRT - wrt-runtime
// Module: Core WebAssembly Runtime
// SW-REQ-ID: REQ_001
// SW-REQ-ID: REQ_002
// SW-REQ-ID: REQ_MEM_SAFETY_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Runtime (WRT) - Runtime Implementation
//!
//! This crate provides the core runtime types and implementations for
//! WebAssembly, shared between both the core WebAssembly and Component Model
//! implementations.
//!
//! # Safety
//!
//! Most modules forbid unsafe code. Only specific modules that require direct
//! memory access (atomic operations, wait queues) allow unsafe code with
//! documented safety invariants.

#![cfg_attr(not(feature = "std"), no_std)]
// Note: unsafe_code is allowed selectively in specific modules that need it
// Lints configured in Cargo.toml

// Import std when available
#[cfg(feature = "std")]
extern crate std;

// Binary std/no_std choice
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Re-export prelude module publicly
pub use prelude::*;

// Test module for clean architecture migration
pub mod clean_runtime_tests;

// Core modules
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod atomic_execution;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod atomic_memory_model;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod atomic_runtime;
pub mod cfi_engine;
pub mod core_types;
pub mod execution;
#[cfg(test)]
mod execution_tests;
/// Format bridge interface
pub mod format_bridge;
pub mod func;
pub mod gc;
pub mod global;
pub mod memory;

// WebAssembly bulk memory operations runtime
pub mod bulk_memory;

// WebAssembly 3.0 multi-memory runtime
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod multi_memory;

// WebAssembly 3.0 shared memory runtime
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod shared_memory;

// WebAssembly SIMD runtime
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod simd_runtime;

// Simplified type system - CRITICAL COMPILATION FIX
pub mod simple_types;
pub mod unified_types;

// WASI Preview2 host implementation (std-only due to OS dependencies)
#[cfg(feature = "std")]
pub mod wasip2_host;

// Component model integration
pub mod capability_integration;
pub mod component_unified;
pub mod memory_adapter;
pub mod memory_config_adapter;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod memory_helpers;
/// WebAssembly module representation and management
pub mod module;
pub mod module_instance;
pub mod prelude;
pub mod stackless;
pub mod table;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod thread_manager;
pub mod type_conversion;
pub mod types;

// Platform-aware runtime and unified memory management
pub mod platform_runtime;

// Bounded infrastructure for static memory allocation
pub mod bounded_runtime_infra;

// Smart runtime provider that prevents stack overflow
pub mod runtime_provider;

// Capability-based execution engine
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod engine;

// Engine factory pattern for architecture refactoring
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod engine_factory;

// Comprehensive testing infrastructure
#[cfg(feature = "std")]
pub mod testing_framework;

// Instruction parser for bytecode to instruction conversion
pub mod instruction_parser;
#[cfg(test)]
mod instruction_parser_tests;

// Temporary stub modules for parallel development
mod component_stubs;
mod foundation_stubs;

// Runtime state and resource management
pub mod component;
pub mod resources;
pub mod state;

// Import platform abstractions from wrt-foundation
// Re-export commonly used types
#[cfg(any(feature = "std", feature = "alloc"))]
pub use atomic_execution::{
    AtomicExecutionStats,
    AtomicMemoryContext,
};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use atomic_memory_model::{
    AtomicMemoryModel,
    ConsistencyValidationResult,
    DataRaceReport,
    MemoryModelPerformanceMetrics,
    MemoryOrderingPolicy,
    OrderingViolationReport,
};
pub use cfi_engine::{
    CfiEngineStatistics,
    CfiExecutionEngine,
    CfiExecutionResult,
    CfiViolationPolicy,
    CfiViolationType,
    ExecutionResult,
};
pub use core_types::{
    CallFrame,
    ComponentExecutionState,
    ExecutionContext,
};
pub use execution::ExecutionStats;
pub use func::Function as RuntimeFunction;
pub use global::Global;
#[cfg(any(feature = "std", feature = "alloc"))]
pub use memory::Memory;
pub use memory_adapter::{
    MemoryAdapter,
    SafeMemoryAdapter,
    StdMemoryProvider,
};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use memory_helpers::ArcMemoryExt;
pub use prelude::FuncType;
pub use table::Table;
pub use wrt_foundation::platform_abstraction;

/// The WebAssembly memory page size (64KiB)
pub const PAGE_SIZE: usize = 65536;

/// Component Model trait definitions for runtime interfaces
#[cfg(feature = "std")]
pub mod component_traits;

// Internal modules
#[cfg(test)]
mod tests;

// Re-export trait definitions
#[cfg(feature = "std")]
pub use component_traits::HostImportHandler;

// Panic handler is provided by the main binary crate to avoid conflicts

// Panic handler is provided by wrt-platform when needed
