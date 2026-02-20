// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Prelude module for wrt-foundation
//!
//! This module provides safety-level-aware imports that automatically select
//! appropriate types based on enabled safety features. It implements the
//! four-layer safety architecture with type selection based on safety
//! integrity levels rather than just std/no_std choices.

// ============================================================================
// SAFETY-LEVEL-AWARE TYPE SELECTION
// ============================================================================
// Type selection based on four-layer safety architecture:
// Layer 1: Memory Management Strategy (static/bounded/managed/std-allocation)
// Layer 4: Safety Integrity Levels (ASIL-D → static, ASIL-C → bounded, etc.)

// Compile-time safety validation removed - conflicts resolved by
// strategy-specific capabilities

// Core traits and types available in all safety levels

// Managed allocation requires alloc crate for dynamic collections (only when
// std is not available)
#[cfg(all(
    feature = "managed-dynamic-alloc",
    not(feature = "std-allocation"),
    not(feature = "bounded-allocation"),
    not(feature = "static-allocation")
))]
extern crate alloc;

// managed-allocation imports (ASIL-A/B, DAL-D, SIL-1/2, Class A) - only when
// std is not available
#[cfg(all(
    feature = "managed-dynamic-alloc",
    not(feature = "std-allocation"),
    not(feature = "bounded-allocation"),
    not(feature = "static-allocation")
))]
pub use alloc::{
    boxed::Box,
    collections::{
        BTreeMap,
        BTreeSet,
    },
    format,
    string::{
        String,
        ToString,
    },
    vec,
    vec::Vec,
};
pub use core::{
    any::Any,
    clone::Clone,
    cmp::{
        Eq,
        Ord,
        PartialEq,
        PartialOrd,
    },
    convert::{
        TryFrom,
        TryInto,
    },
    default::Default,
    fmt::{
        self,
        Debug,
        Display,
        Write,
    },
    hash::Hash,
    marker::{
        Copy,
        PhantomData,
        Sized,
    },
    mem,
    ops::{
        Deref,
        DerefMut,
    },
    slice,
    str,
};
// std-allocation imports (QM, DAL-E - non-safety-critical)
#[cfg(feature = "std-allocation")]
pub use std::{
    boxed::Box,
    collections::{
        BTreeMap,
        BTreeSet,
        HashMap,
        HashSet,
    },
    format,
    string::{
        String,
        ToString,
    },
    sync::Arc,
    vec,
    vec::Vec,
};

// Always use wrt_sync for consistent Mutex/RwLock behavior
#[cfg(feature = "std-allocation")]
pub use wrt_sync::{
    Mutex,
    RwLock,
};

// bounded-allocation and static-allocation alternatives
// For bounded-allocation: Use bounded collections with compile-time capacity limits
// For static-allocation: Use static arrays only (no dynamic collections)
#[cfg(feature = "use-hashbrown")]
pub use hashbrown::HashMap as BHashMap;
// Safety-level-aware choice
// Dynamic collections (HashSet, Arc) are only available for managed-allocation and
// std-allocation Higher safety levels use bounded or static alternatives

// Re-export from wrt_error - this is the standard Result type for WRT
pub use wrt_error::prelude::*;
pub use wrt_error::{
    codes,
    kinds,
    Error,
    ErrorCategory,
    Result,
};

// Feature-gated re-exports that can't be included in the main use block
#[cfg(feature = "std")]
pub use crate::component_builder::{
    ComponentTypeBuilder,
    ExportBuilder,
    ImportBuilder,
    NamespaceBuilder,
};
// Modern memory system convenience functions already imported above

// Safety-level-aware conversion functions (only available with std feature)
#[cfg(all(
    feature = "std",
    any(feature = "managed-dynamic-alloc", feature = "std-allocation")
))]
pub use crate::conversion::{
    ref_type_to_val_type,
    val_type_to_ref_type,
};
// Re-export from wrt_sync, only if the feature is active
// #[cfg(feature = "wrt-sync")] // Or a more specific feature if wrt-sync is always a dep

// Re-export platform-specific memory builders if the feature is enabled
// Memory builders removed in clean architecture
// #[cfg(feature = "platform-memory")]
// pub use crate::memory_builder::{LinearMemoryBuilder, PalMemoryProviderBuilder};
// Safety-level-aware hashmap selection
#[cfg(all(feature = "static-allocation", not(feature = "std-allocation")))]
pub use crate::no_std_hashmap::SimpleHashMap;
// Re-export from this crate
pub use crate::{
    // ASIL testing framework
    asil_testing::{
        get_asil_tests,
        get_test_statistics,
        get_tests_by_asil,
        get_tests_by_category,
        register_asil_test,
        AsilTestMetadata,
        TestCategory,
        TestStatistics,
    },
    // Atomic memory operations
    atomic_memory::{
        AtomicMemoryExt,
        AtomicMemoryOps,
    },
    // Bounded collections
    bounded::{
        BoundedStack,
        BoundedString,
        BoundedVec,
        CapacityError,
        WasmName,
    },
    bounded_collections::{
        BoundedDeque,
        BoundedMap,
        BoundedQueue,
        BoundedSet,
    },
    // Builder patterns
    builder::{
        BoundedBuilder,
        MemoryBuilder,
        ResourceBuilder,
        ResourceItemBuilder,
        StringBuilder,
    },
    // Builtin types
    builtin::BuiltinType,
    // Component model types
    component::{
        ComponentType,
        Export,
        ExternType,
        Import,
        ImportKey,
        InstanceType, // Limits,
        Namespace,
        ResourceType,
    },
    memory_coordinator::{
        AllocationId,
        CrateIdentifier,
        GenericMemoryCoordinator,
    },
    // Resource types
    resource::ResourceOperation,
    // Safety-level-aware features
    safety_features::{
        allocation::MEMORY_STRATEGY,
        runtime::{
            current_safety_level,
            has_capability,
            max_allocation_size,
        },
        standards::{
            AsilLevel,
            DalLevel,
            SafetyStandardMapping,
            SilLevel,
        },
    },
    // Safe memory types (SafeMemoryHandler, SafeSlice, SafeStack are already here from direct
    // re-exports) Sections (SectionId, SectionType, Section are usually handled by decoder)
    // Binary std/no_std choice
    // safe_memory::NoStdProvider, // Re-exported below to avoid duplicate
    // Safety system types
    safety_system::{
        SafeMemoryAllocation,
        SafetyContext,
        SafetyGuard,
    },
    // Validation traits (moved to traits module to break circular dependency)
    traits::{
        BoundedCapacity,
        Checksummed,
        Validatable, /* ValidationContext,
                      * ValidationError and ValidationResult will be re-added when validation
                      * module is restored */
    },
    // Traits
    traits::{
        FromFormat,
        ToFormat,
    },
    // Common types (BlockType, FuncType, GlobalType, MemoryType, TableType, ValueType are already
    // here)
    types::{
        // BlockType,
        FuncType,
        GlobalType,
        Limits,
        MemoryType,
        RefType,
        TableType,
        ValueType,
    },
    // New unified types from Agent A deliverables (simplified)
    unified_types_simple::{
        DefaultTypes,
        DesktopTypes,
        EmbeddedTypes,
        PlatformCapacities,
        SafetyCriticalTypes,
        UnifiedTypes,
    },
    // Value representations
    values::{
        FloatBits32,
        FloatBits64,
        Value,
    },
    // Verification types
    verification::{
        Checksum,
        VerificationLevel,
    },
    // Direct re-exports for convenience (original list)
    // ResourceType, // Already covered by component::* above
    SafeMemoryHandler,
    SafeSlice,
};
// Safety-level-aware component system
// Component builders only available with std feature and allocation levels
#[cfg(all(
    feature = "std",
    any(feature = "managed-dynamic-alloc", feature = "std-allocation")
))]
pub use crate::{
    // Component builders
    component_value::{
        ComponentValue,
        ValType,
    },
    component_value_store::{
        ComponentValueStore,
        ValueRef,
    },
    component_value_store_builder::ComponentValueStoreBuilder,
};
// Memory system core exports
pub use crate::{
    // Modern memory system types
    generic_memory_guard::GenericMemoryGuard,
    // Core memory allocation macro
    safe_managed_alloc,
    // Memory provider types
    safe_memory::Provider as MemoryProvider,
    wrt_memory_system::CapabilityWrtFactory,
};

// Binary std/no_std choice
/// Maximum number of arguments/results for WebAssembly functions
pub const MAX_WASM_FUNCTION_PARAMS: usize = 128;

// ============================================================================
// SAFETY-LEVEL-AWARE TYPE ALIASES
// ============================================================================
// Type selection based on safety integrity levels

/// Safety capacity limits based on enabled safety features
pub const MAX_STATIC_CAPACITY: usize = 16; // ASIL-D: 16KB / sizeof(T)
pub const MAX_BOUNDED_CAPACITY: usize = 64; // ASIL-C: 64KB / sizeof(T)

/// Safety-level-aware function argument vector with proper precedence.
///
/// Precedence hierarchy: std-allocation > managed-dynamic-alloc > bounded-allocation > static-allocation.
/// Standard allocation (QM, DAL-E) - highest precedence
#[cfg(feature = "std-allocation")]
pub type ArgVec<T> = Vec<T>;

/// Managed allocation (ASIL-A/B, DAL-D, SIL-1/2, Class A) - second precedence
#[cfg(all(feature = "managed-dynamic-alloc", not(feature = "std-allocation")))]
pub type ArgVec<T> = Vec<T>;

/// Bounded allocation (ASIL-C, DAL-B, SIL-3, Class B) - third precedence
#[cfg(all(
    feature = "bounded-allocation",
    not(feature = "std-allocation"),
    not(feature = "managed-dynamic-alloc")
))]
pub type ArgVec<T> =
    BoundedVec<T, MAX_WASM_FUNCTION_PARAMS, NoStdProvider<{ MAX_WASM_FUNCTION_PARAMS * 16 }>>;

/// Static allocation (ASIL-D, DAL-A, SIL-4, Class C) - lowest precedence
#[cfg(all(
    feature = "static-allocation",
    not(feature = "std-allocation"),
    not(feature = "managed-dynamic-alloc"),
    not(feature = "bounded-allocation")
))]
pub type ArgVec<T> = [T; MAX_WASM_FUNCTION_PARAMS];

/// Fallback when no allocation strategy is specified
#[cfg(not(any(
    feature = "static-allocation",
    feature = "bounded-allocation",
    feature = "managed-dynamic-alloc",
    feature = "std-allocation"
)))]
pub type ArgVec<T> =
    BoundedVec<T, MAX_WASM_FUNCTION_PARAMS, NoStdProvider<{ MAX_WASM_FUNCTION_PARAMS * 16 }>>;

// ============================================================================
// SAFETY-LEVEL-AWARE CAPABILITY SYSTEM
// ============================================================================
// Capability system selection based on safety requirements

// Static allocation environments - minimal capability context
// Bounded allocation environments - bounded capability system
#[cfg(all(
    feature = "bounded-allocation",
    not(feature = "static-allocation"),
    not(all(
        any(feature = "std", feature = "alloc"),
        any(feature = "managed-dynamic-alloc", feature = "std-allocation")
    ))
))]
pub use crate::capabilities::CapabilityAwareProvider;
#[cfg(all(
    feature = "static-allocation",
    not(any(feature = "std", feature = "alloc"))
))]
pub use crate::capabilities::NoStdCapabilityContext as CapabilityContext;
// Managed and standard allocation - full capability system (requires alloc)
#[cfg(all(
    any(feature = "std", feature = "alloc"),
    any(feature = "managed-dynamic-alloc", feature = "std-allocation")
))]
pub use crate::capabilities::{
    CapabilityAwareProvider,
    CapabilityProviderFactory,
    ProviderCapabilityExt,
};
// Re-export NoStdProvider only once
pub use crate::safe_memory::NoStdProvider;
// Verified allocator with GlobalAlloc and scope support
pub use crate::verified_allocator::{
    ScopeGuard,
    ScopeInfo,
    VerifiedAllocator,
    TOTAL_HEAP_SIZE,
    MAX_MODULE_SIZE,
    MAX_SCOPES,
};
pub use crate::{
    // Budget management
    budget_aware_provider::CrateId,
    // Capability system
    capabilities::{
        CapabilityMask,
        MemoryCapability,
        MemoryGuard,
        MemoryOperation,
        MemoryOperationType,
        MemoryRegion,
    },
    // Memory initialization
    memory_init::MemoryInitializer,
};
