// WRT - wrt-component
// Module: WebAssembly Component Model Implementation
// SW-REQ-ID: REQ_019
// SW-REQ-ID: REQ_002
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![deny(unsafe_code)] // Rule 2 - deny instead of forbid to allow opt-in for required unsafe (e.g., Waker creation)

//! Component Model implementation for the WebAssembly Runtime (WRT).
//!
//! This crate provides an implementation of the WebAssembly Component Model,
//! enabling composition and interoperability between WebAssembly modules
//! with shared-nothing linking.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "kani", feature(kani))]
// Lints configured in Cargo.toml
#![allow(deprecated)]

// extern crate declarations
#[cfg(not(feature = "std"))]
extern crate alloc;

// Debug macro for both std and no_std environments
// When tracing is enabled, use tracing::trace! for structured logging
#[cfg(feature = "tracing")]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        wrt_foundation::tracing::trace!($($arg)*);
    };
}

#[cfg(not(feature = "tracing"))]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        // No-op when tracing is not enabled
    };
}

// Export our prelude module for consistent imports
pub mod prelude;

// Bounded infrastructure for static memory allocation
pub mod bounded_component_infra;
// Disabled until stub modules (foundation_stubs, platform_stubs, runtime_stubs) are implemented
// pub mod bounded_resource_management;

// Core component modules
pub mod adapter;
pub mod agent_registry;
pub mod async_;
pub mod blast_zone;
pub mod builtins;
pub mod call_context;
pub mod canonical_abi;
pub mod canonical_executor;
pub mod components;
pub mod cross_component_calls;
pub mod cross_component_communication;
pub mod cross_component_resource_sharing;
pub mod resource_limits_loader;

// Module aliases for compatibility with existing imports
pub use components::component_instantiation;
pub use cross_component_communication as component_communication;
pub mod execution;
pub mod execution_engine;
pub mod export;
pub mod generative_types;
pub mod handle_representation;
pub mod import;
pub mod instance; // Consolidated: contains both std and no_std implementations
pub mod instantiation;
pub mod linker;
pub mod memory_layout;
#[cfg(feature = "safety-critical")]
pub mod memory_limits;
pub mod parser;
pub mod parser_integration;
pub mod platform_component;
// Note: Stub modules contain syntax errors but are needed by other modules
pub mod foundation_stubs;
pub mod platform_stubs;
pub mod post_return;
pub mod resource_management;
pub mod resources;
pub mod runtime;
pub mod runtime_stubs;
pub mod strategies;
pub mod string_encoding;
pub mod type_bounds;
pub mod type_conversion;
pub mod types;
pub mod unified_execution_agent;
pub mod unified_execution_agent_stubs;
pub mod values;
pub mod virtualization;

// Module aliases for commonly expected imports
pub use memory_layout as memory;

// Async support features enabled via feature flags

// Threading support
#[cfg(feature = "component-model-threading")]
pub mod threading;

// Include verification module when the kani feature is enabled
#[cfg(feature = "kani")]
pub mod verify;

// Essential re-exports only
pub use blast_zone::{
    BlastZoneConfig, BlastZoneManager, ContainmentPolicy, IsolationLevel, ZoneHealth,
};
pub use builtins::{BuiltinHandler, BuiltinRegistry};
pub use canonical_abi::canonical::CanonicalABI;
pub use components::component::ComponentType;
// Re-export MemoryProvider from foundation for type parameters
pub use wrt_foundation::MemoryProvider;
// Add placeholder types for commonly missing imports (when not imported from
// component modules)
pub type ResourceHandle = u32;
pub type TypeId = u32;
// Component types based on feature flags
#[cfg(feature = "std")]
pub use components::component::{
    Component, ExternValue, FunctionValue, GlobalValue, MemoryValue, TableValue,
};
#[cfg(not(feature = "std"))]
pub use components::component_no_std::{
    Component, ComponentBuilder, ExternValue, FunctionValue, GlobalValue, MemoryValue,
    RuntimeInstance, TableValue, WrtComponentType, WrtComponentTypeBuilder,
};
// Component registry (consolidated implementation for both std and no_std)
pub use components::component_registry::ComponentRegistry;
pub use resources::{
    DynamicQuotaManager, QuotaNode, QuotaNodeType, QuotaPolicy, QuotaRequest, QuotaResourceType,
    QuotaResponse, QuotaStatus,
};
// Canonical type definitions for ASIL-D compliance
pub use types::{ComponentInstance, ComponentInstanceId, ComponentInstanceState};

// Make the debug_println macro available to submodules
// (Defined above using tracing::trace! when tracing feature is enabled)
pub(crate) use debug_println;
