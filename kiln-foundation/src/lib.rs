//! Core type definitions, traits, and utilities for the WebAssembly Runtime
//! (WRT).
//!
//! This crate provides foundational data structures and functionalities used
//! across the WRT ecosystem, ensuring type safety, memory safety, and
//! consistent error handling. It supports three configurations:
//! - `std`: Full standard library support
//! - `no_std` + `alloc`: No standard library but with allocation
//! - `no_std` + `no_alloc`: Pure no_std without any allocation
//!
//! # Feature Flags
//!
//! - `std`: Enables standard library support (implies `alloc`)
//! - `alloc`: Enables allocation support for `no_std` environments
//! - Default: Pure `no_std` without allocation

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)] // Changed from forbid to deny to allow specific unsafe blocks when justified
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(hidden_glob_reexports)]
// Allow clippy warnings that would require substantial refactoring
#![allow(clippy::needless_continue)]
#![allow(clippy::if_not_else)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::map_identity)]
#![allow(clippy::expect_used)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::float_arithmetic)]
#![allow(clippy::unimplemented)]
#![allow(clippy::useless_attribute)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::never_loop)]
#![allow(clippy::while_immutable_condition)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
#![allow(clippy::duplicated_attributes)]
#![allow(clippy::multiple_bound_locations)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(clippy::unused_self)]
#![allow(clippy::ptr_as_ptr)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::inline_always)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::panic)]
#![allow(clippy::single_match_else)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::match_same_arms)]
// Allow all pedantic clippy warnings for now to focus on core functionality
#![allow(clippy::pedantic)]
#![allow(clippy::identity_op)]

/// Version of the wrt-foundation crate (from wrt-helper)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Helper macro to check if a feature is enabled (from wrt-helper)
#[macro_export]
macro_rules! has_feature {
    ($feature:expr) => {
        cfg!(feature = $feature)
    };
}

// Core library is always available
extern crate core;

#[cfg(feature = "std")]
extern crate std;

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// External crates
use wrt_error::{
    codes,
    kinds,
    Error,
    ErrorCategory,
    Result,
};

// WRT - wrt-foundation
// SW-REQ-ID: REQ_MEM_SAFETY_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// SW-REQ-ID: REQ_LANG_RUST_PROJECT_SETUP_001
// SW-REQ-ID: REQ_LANG_RUST_EDITION_001

#[forbid(clippy::unwrap_used, clippy::panic)]
#[warn(clippy::pedantic, clippy::nursery)]
// Note: clippy::expect_used is allowed at crate level (line 53) for documented safety cases
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::return_self_not_must_use)]
#[allow(clippy::doc_markdown)]
// Prelude module for consistent imports across std and no_std environments
pub mod prelude;
pub mod safety_features;

// NEW: Static collections (heapless-inspired, self-contained)
// SW-REQ-ID: REQ_RESOURCE_001 (static allocation)
pub mod collections;

// Execution-related shared types
pub mod execution;

// Runtime memory budget limits for safety-critical operations
pub mod runtime_limits;

// Platform-specific WebAssembly limits for bounded allocation
pub mod limits;

// Allocation tracing for understanding memory patterns (development only)
pub mod allocation_tracing;
pub use allocation_tracing::Phase as AllocationPhase;

// Tracing support for debugging complex execution flows
#[cfg(feature = "tracing")]
pub mod tracing;

// Re-export common types from prelude
pub use prelude::*;

// Core memory system macro - the primary allocation interface
/// Safe managed memory allocation with automatic cleanup
///
/// This is the primary interface for memory allocation in WRT. It provides:
/// - Automatic budget verification
/// - RAII-based cleanup
/// - Capability-based access control
/// - Compile-time size validation where possible
///
/// # Examples
///
/// ```rust
/// use wrt_foundation::{
///     safe_managed_alloc,
///     CrateId,
/// };
///
/// // Basic allocation
/// let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;
///
/// // Use with bounded collections
/// let mut vec = BoundedVec::new(provider)?;
/// vec.push(42)?;
/// // Memory automatically cleaned up on drop
/// ```
#[macro_export]
macro_rules! safe_managed_alloc {
    ($size:expr, $crate_id:expr) => {{
        // Ensure memory system is initialized
        let _ = $crate::memory_init::MemoryInitializer::initialize();

        // Use capability-based allocation
        $crate::wrt_memory_system::CapabilityWrtFactory::create_provider::<$size>($crate_id)
    }};
}

// Re-export bounded slice functionality
pub use bounded_slice::{
    BoundedSlice,
    BoundedVecSliceExt,
};

/// `Result` type alias for WRT operations using `wrt_error::Error`
///
/// **DEPRECATED**: Use `wrt_error::Result<T>` directly instead.
/// This type alias is maintained for backward compatibility but will be
/// removed.
// WrtResult type alias has been removed - use wrt_error::Result<T> directly

// Core modules - always available in all configurations
/// Atomic memory operations with integrated checksumming
pub mod atomic_memory;
/// Bounded collections for memory safety
pub mod bounded;
/// Binary std/no_std choice
pub mod bounded_collections;
/// Bounded slice abstraction for safe slice-like access
pub mod bounded_slice;
/// Direct storage map for hot-path execution (no serialization)
pub mod direct_map;
/// Binary std/no_std choice
pub mod builder;
/// WebAssembly Component Model built-in types
pub mod builtin;
// /// String conversion traits for component model integration
// pub mod string_conversion;
/// WebAssembly Component Model types
pub mod component;
/// Type conversion utilities
pub mod conversion;
/// Float representation utilities
pub mod float_repr;
/// Operation tracking and fuel metering
pub mod operations;
/// Resource management
pub mod resource;
/// Safe memory access primitives
pub mod safe_memory;
/// WebAssembly section definitions
pub mod sections;
/// Shared memory support for multi-threading
pub mod shared_memory;
/// Common traits for type conversions
pub mod traits;
/// Core WebAssembly types
pub mod types;
/// Validation utilities
pub mod validation;
/// WebAssembly value representations
pub mod values;
/// Verification and integrity checking
pub mod verification;
/// Verified memory allocator with GlobalAlloc and scope support
pub mod verified_allocator;
/// Formal verification using Kani
#[cfg(any(doc, kani))]
pub mod verify;

// Modern Memory Management System
/// Compile-time budget verification system
pub mod budget_verification;
pub mod compile_time_bounds;
/// Generic RAII memory guard - automatic cleanup
pub mod generic_memory_guard;
/// Generic provider factory - budget-aware allocation
pub mod generic_provider_factory;
/// Generic memory coordination system - works with any project
pub mod memory_coordinator;
/// WRT-specific memory system implementation
pub mod wrt_memory_system;
// Validated collections disabled - architectural decision to keep API simple
// Use standard bounded collections instead for better maintainability
// #[cfg(feature = "std")]
// pub mod validated_collections;
/// Compile-time memory enforcement system - prevents bypass
pub mod enforcement;
/// Formal verification with KANI - mathematical proofs
pub mod formal_verification;
/// Hierarchical budget system for complex allocations
pub mod hierarchical_budgets;
/// Zero-configuration convenience macros
pub mod macros;
/// Modern memory initialization system - zero-config setup
pub mod memory_init;
/// Compile-time memory sizing strategy
pub mod memory_sizing;
/// Memory system monitoring and telemetry
pub mod monitoring;

// Clean Architecture - Provider-Free Types
pub mod clean_core_types;
/// Clean type definitions without provider parameters
pub mod clean_types;
/// Safe allocation API without unsafe code
pub mod safe_allocation;
/// Type factory pattern for allocation boundary management
pub mod type_factory;

// WRT Compile-Time Allocator System
/// Revolutionary compile-time memory allocation system with A+ safety
/// compliance
pub mod allocator;

// Legacy Support & Compatibility
/// Budget-aware provider factory for global memory coordination
pub mod budget_aware_provider;
/// Budget provider compatibility layer
pub mod budget_provider;

// Non-Memory Foundation Modules
/// ASIL-tagged testing framework for safety verification
pub mod asil_testing;
/// Platform Abstraction Interface (PAI) for cross-platform safety-critical
/// runtime
pub mod platform_abstraction;
/// ASIL-aware safety primitives for safety-critical applications
pub mod safety_system;
/// Unified type system with platform-configurable bounded collections
/// (simplified)
pub mod unified_types_simple;

// Capability-driven memory architecture
pub mod capabilities;
// Capability-aware allocators for heap types
pub mod capability_allocators;
// Platform abstraction for safe atomic operations
pub mod platform_atomic;
// Systematic fault detection for memory safety (ASIL-A)
pub mod fault_detection;
// Runtime safety monitoring for production deployments (ASIL-A)
pub mod safety_monitor;
// Production telemetry and logging infrastructure (ASIL-A)
pub mod telemetry;
// Heap-based memory provider to avoid stack overflow
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod heap_provider;

// Binary std/no_std choice
#[cfg(feature = "std")]
/// Builder patterns for Component Model types
pub mod component_builder;
#[cfg(feature = "std")]
/// Store for component model types
pub mod component_type_store;
#[cfg(feature = "std")]
/// WebAssembly Component Model value types
pub mod component_value;
#[cfg(feature = "std")]
pub mod component_value_store;
#[cfg(feature = "std")]
/// Builder pattern for component value store
pub mod component_value_store_builder;

// Platform-specific modules
#[cfg(feature = "platform-memory")]
/// Linear memory implementation using PageAllocator.
pub mod linear_memory;
// Memory system modules removed - now using clean architecture
// #[cfg(feature = "platform-memory")]
// /// Memory builder patterns for platform-backed memory types
// pub mod memory_builder;
// #[cfg(feature = "platform-memory")]
// /// Runtime memory module
// pub mod runtime_memory;

// Binary std/no_std choice
#[cfg(not(feature = "std"))]
/// No-std hash map implementation
pub mod no_std_hashmap;
// pub mod no_std_compat;

// Re-export the most important types - core types always available
pub use atomic_memory::{
    AtomicMemoryExt,
    AtomicMemoryOps,
};
pub use bounded::{
    BoundedStack,
    BoundedString,
    BoundedVec,
    CapacityError,
    WasmName,
};
// Alloc-dependent re-exports
#[cfg(feature = "std")]
pub use bounded_collections::BoundedBitSet;
pub use bounded_collections::{
    BoundedDeque,
    BoundedMap,
    BoundedQueue,
    BoundedSet,
};
pub use builder::{
    BoundedBuilder,
    MemoryBuilder,
    ResourceBuilder,
    ResourceItemBuilder,
    ResourceTypeBuilder,
    StringBuilder,
};
pub use builtin::BuiltinType;
pub use component::{
    ComponentType,
    ExternType,
    InstanceType,
    Namespace,
    ResourceType,
};
#[cfg(feature = "std")]
pub use component_builder::{
    ComponentTypeBuilder,
    ExportBuilder,
    ImportBuilder,
    NamespaceBuilder,
};
#[cfg(feature = "std")]
pub use component_type_store::{
    ComponentTypeStore,
    TypeRef,
};
#[cfg(feature = "std")]
pub use component_value::ComponentValue;
#[cfg(feature = "std")]
pub use component_value_store::{
    ComponentValueStore,
    ValueRef,
};
#[cfg(feature = "std")]
pub use component_value_store_builder::ComponentValueStoreBuilder;
#[cfg(feature = "std")]
pub use conversion::{
    ref_type_to_val_type,
    val_type_to_ref_type,
};
pub use float_repr::{
    FloatBits32,
    FloatBits64,
};
pub use operations::{
    global_fuel_consumed,
    global_operation_summary,
    record_global_operation,
    reset_global_operations,
    Summary as OperationSummary,
    Tracking as OperationTracking,
    Type as OperationType,
};
#[cfg(feature = "std")]
pub use safe_memory::StdMemoryProvider;
pub use safe_memory::{
    NoStdProvider,
    Provider as MemoryProvider,
    SafeMemoryHandler,
    Slice as SafeSlice,
    SliceMut as SafeSliceMut,
    Stats as MemoryStats,
};

// Type aliases for backward compatibility
pub type DefaultMemoryProvider<const N: usize> = NoStdProvider<N>;
pub type WrtVec<T, const CAPACITY: usize, P> = BoundedVec<T, CAPACITY, P>;
// Re-export crate identifiers
pub use budget_aware_provider::CrateId;
// Re-export budget provider types
#[allow(deprecated)]
pub use budget_provider::BudgetProvider;
// Re-export modern memory system types
pub use budget_verification::{
    CRATE_BUDGETS,
    TOTAL_MEMORY_BUDGET,
};
// Re-export clean core types (provider-free core WebAssembly types)
#[cfg(any(feature = "std", feature = "alloc"))]
pub use clean_core_types::{
    CoreFuncType as CleanCoreFuncType,
    CoreGlobalType as CleanCoreGlobalType,
    CoreMemoryType as CleanCoreMemoryType,
    CoreTableType as CleanCoreTableType,
};
// Re-export clean types (provider-free) - only when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use clean_types::{
    Case as CleanCase,
    ComponentType as CleanComponentType,
    ComponentTypeDefinition as CleanComponentTypeDefinition,
    Enum as CleanEnum,
    ExternType as CleanExternType,
    Field as CleanField,
    Flags as CleanFlags,
    FuncType as CleanFuncType,
    GlobalType as CleanGlobalType,
    InstanceType as CleanInstanceType,
    Limits as CleanLimits,
    MemoryType as CleanMemoryType,
    Record as CleanRecord,
    RefType as CleanRefType,
    Result_ as CleanResult,
    TableType as CleanTableType,
    Tuple as CleanTuple,
    ValType as CleanValType,
    Value as CleanValue,
    Variant as CleanVariant,
};
pub use compile_time_bounds::{
    CollectionBoundsValidator,
    CompileTimeBoundsValidator,
    MemoryLayoutValidator,
    ResourceLimitsValidator,
    StackBoundsValidator,
    SystemBoundsValidator,
};
// Capability-based memory system replaces legacy global coordinator
// Use memory_init::get_global_capability_context() for modern memory management

#[allow(deprecated)]
pub use enforcement::{
    AllocationToken,
    EnforcedAllocation,
    MemoryManaged,
};
pub use generic_memory_guard::{
    GenericMemoryGuard,
    ManagedMemoryProvider,
    MemoryCoordinator,
};
// Re-export hierarchical budget types
pub use hierarchical_budgets::{
    HierarchicalBudget,
    MemoryPriority,
};
pub use memory_coordinator::{
    AllocationId,
    CrateIdentifier,
    GenericMemoryCoordinator,
};
pub use memory_init::MemoryInitializer;
// Re-export monitoring types
pub use monitoring::{
    convenience as monitoring_convenience,
    MemoryMonitor,
    MemoryStatistics,
    SystemHealth,
    SystemReport,
};
// Re-export platform abstraction std types
#[cfg(feature = "std")]
pub use platform_abstraction::SystemTimeProvider;
// Re-export platform abstraction types
pub use platform_abstraction::{
    current_time_ns,
    get_platform_limits,
    get_platform_services,
    // Factory functions
    initialize_platform_services,
    CounterTimeProvider,
    // Core platform types
    PlatformLimits,
    PlatformServices,
    TimeProvider,
};
// Re-export safety system types
pub use safety_system::{
    AgricultureLevel,
    // Traditional ASIL types
    AsilLevel,
    // Additional safety standard levels
    DalLevel,
    MedicalClass,
    RailwaySil,
    SafeMemoryAllocation,
    SafetyContext,
    SafetyError,
    SafetyGuard,
    SafetyLevel,
    // Universal safety system types
    SafetyStandard,
    SafetyStandardConversion,
    SafetyStandardType,
    SeverityScore,
    SilLevel,
    UniversalSafetyContext,
};
pub use traits::{
    BoundedCapacity,
    Checksummed,
    FromFormat,
    ToFormat,
    Validatable,
};
#[cfg(feature = "std")]
pub use traits::{HostImportHandler, MemoryAccessor, SliceMemory};
// Re-export type factory types - only when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use type_factory::{
    ComponentFactory1M,
    ComponentFactory64K,
    ComponentFactory8K,
    ComponentTypeFactory,
    FactoryBuilder,
    RuntimeFactory1M,
    RuntimeFactory64K,
    RuntimeFactory8K,
    RuntimeTypeFactory,
    TypeConverter,
    TypeFactory,
};
pub use types::{
    // Core types
    BlockType,
    DataMode,
    ElementMode,
    FuncType,
    GlobalType,
    Limits,
    MemArg,
    MemoryType,
    RefType,
    TableType,
    ValueType,
    // GC types (WebAssembly 3.0)
    ArrayType,
    CompositeType,
    FieldType,
    HeapType,
    GcRefType,
    PackedType,
    StorageType,
    StructType,
    SubType,
    // GC constants
    MAX_STRUCT_FIELDS,
    MAX_ARRAY_ELEMENTS,
    MAX_SUPERTYPES,
    MAX_REC_GROUP_SIZE,
};
// Re-export unified types for backward compatibility and new functionality
pub use unified_types_simple::{
    DefaultTypes,
    DesktopTypes,
    EmbeddedTypes,
    PlatformCapacities,
    SafetyCriticalTypes,
    UnifiedTypes,
};
pub use values::Value;
pub use verification::{
    Checksum,
    VerificationLevel,
};
// Re-export runtime limits for safety-critical configurations
pub use runtime_limits::{
    // Resource limits
    AGGREGATE_RESOURCES_LIMIT,
    RECORD_FIELDS_LIMIT,
    RESOURCE_TABLE_LIMIT,
    // Execution limits
    FUNCTIONS_PER_MODULE_LIMIT,
    MODULE_SIZE_LIMIT,
    STACK_DEPTH_LIMIT,
};
// Re-export capability-based memory factory and deprecated coordinator for compatibility
pub use wrt_memory_system::CapabilityWrtFactory;

// Note: Macros exported with #[macro_export] are available at the crate root
// create_foundation_provider!, create_runtime_provider!,
// create_component_provider!, get_recommended_size!
// create_shared_foundation_provider!, create_shared_runtime_provider!,
// create_shared_component_provider! auto_provider!, auto_shared_provider!,
// small_provider!, medium_provider!, large_provider! monitor_operation!

/// The WebAssembly binary format magic number: \0asm
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// The WebAssembly Core Specification version this runtime aims to support.
/// Version 2.0 includes features like multi-value returns, reference types,
/// etc.
pub const WASM_VERSION: u32 = 2;

// Component model feature re-exports
#[cfg(feature = "component-model-values")]
pub use component_value::ValType;

// Component Model async types (always available when component-model-async is
// enabled)
#[cfg(feature = "component-model-async")]
/// Component Model async types (future, stream, error-context)
pub mod async_types;

// Async support modules
#[cfg(feature = "async-api")]
/// Bridge between Component Model async and Rust async
pub mod async_bridge;
#[cfg(feature = "async-api")]
/// Simple async executor support
pub mod async_executor_simple;

// Component Model async re-exports
// Async API re-exports
#[cfg(feature = "async-api")]
pub use async_bridge::with_async as with_async_bridge;
#[cfg(all(feature = "async-api", feature = "component-model-async"))]
pub use async_bridge::{
    ComponentAsyncExt,
    ComponentFutureBridge,
    ComponentStreamBridge,
};
#[cfg(feature = "async-api")]
pub use async_executor_simple::{
    is_using_fallback,
    with_async,
    AsyncRuntime,
    ExecutorError,
};
#[cfg(feature = "component-model-async")]
pub use async_types::{
    ComponentFuture,
    ComponentFutureStatus,
    ComponentStream,
    ErrorContext,
    FutureHandle,
    StreamHandle,
    StreamState,
};

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-foundation is being tested in
// isolation #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bounded::BoundedVec,
        budget_aware_provider::CrateId,
        safe_managed_alloc,
        safe_memory::{
            NoStdProvider,
            SafeMemoryHandler,
        },
        traits::BoundedCapacity,
    };

    // Helper function to initialize memory system for tests
    fn init_test_memory_system() {
        // Memory system is automatically initialized
    }


    // TODO: Add comprehensive tests for all public functionality in
    // wrt-foundation, ensuring coverage for different VerificationLevels,
    // std/no_std features, and edge cases for component model types, value
    // conversions, etc. Specific modules like bounded.rs, safe_memory.rs,
    // math_ops.rs, etc., should have their own detailed test suites as
    // well.
}
