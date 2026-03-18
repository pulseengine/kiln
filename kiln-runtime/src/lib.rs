// Kiln - kiln-runtime
// Module: Core WebAssembly Runtime
// SW-REQ-ID: REQ_001
// SW-REQ-ID: REQ_002
// SW-REQ-ID: REQ_MEM_SAFETY_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Runtime (Kiln) - Runtime Implementation
//!
//! This crate provides the core runtime types and implementations for
//! WebAssembly, shared between both the core WebAssembly and Component Model
//! implementations.
//!
//! The interpreter is std-only per the RFC #46 architecture decision.
//! The no_std path is via kiln-builtins for synth-compiled code.
//!
//! # Safety
//!
//! Most modules forbid unsafe code. Only specific modules that require direct
//! memory access (atomic operations, wait queues) allow unsafe code with
//! documented safety invariants.

// Note: unsafe_code is allowed selectively in specific modules that need it
// Lints configured in Cargo.toml

extern crate std;
extern crate alloc;

// Re-export prelude module publicly
pub use prelude::*;

// Test module for clean architecture migration
pub mod clean_runtime_tests;

// Core modules
pub mod atomic_execution;
pub mod atomic_memory_model;
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
pub mod multi_memory;

// WebAssembly 3.0 shared memory runtime
pub mod shared_memory;

// WebAssembly SIMD runtime
pub mod simd_runtime;

// Simplified type system - CRITICAL COMPILATION FIX
pub mod simple_types;
pub mod unified_types;

// WASI Preview2 host implementation
pub mod wasip2_host;

// Component model integration
pub mod capability_integration;
pub mod component_unified;
pub mod memory_adapter;
pub mod memory_config_adapter;
pub mod memory_helpers;
/// WebAssembly module representation and management
pub mod module;
pub mod module_instance;
pub mod prelude;
pub mod stackless;
pub mod table;
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
pub mod engine;

// Engine factory pattern for architecture refactoring
pub mod engine_factory;

// Comprehensive testing infrastructure
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

// Import platform abstractions from kiln-foundation
// Re-export commonly used types
pub use atomic_execution::{
    AtomicExecutionStats,
    AtomicMemoryContext,
};
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
pub use memory::Memory;
pub use memory_adapter::{
    MemoryAdapter,
    SafeMemoryAdapter,
    StdMemoryProvider,
};
pub use memory_helpers::ArcMemoryExt;
pub use prelude::FuncType;
pub use table::Table;
