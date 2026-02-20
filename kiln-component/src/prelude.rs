//! Prelude module for wrt-component
//!
//! This module provides a unified set of imports for both std and no_std
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Core imports for std environments

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
pub use alloc::{
    boxed::Box,
    collections::{BTreeMap as HashMap, BTreeSet as HashSet},
    format,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
pub use core::{
    any::Any,
    array,
    cmp::{Eq, Ord, PartialEq, PartialOrd},
    convert::{From, Into, TryFrom, TryInto},
    fmt,
    fmt::{Debug, Display, Write as FmtWrite},
    iter,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    result, slice, str,
    time::Duration,
};
// Re-export from std when the std feature is enabled
#[cfg(feature = "std")]
pub use std::{
    boxed::Box,
    collections::{HashMap, HashSet},
    fmt as std_fmt, format, io,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};

// Always use wrt_sync for consistent Mutex/RwLock behavior across std/no_std
#[cfg(feature = "std")]
pub use wrt_sync::{Mutex, RwLock};

#[cfg(feature = "decoder")]
pub use wrt_decoder::decode_no_alloc;
#[cfg(feature = "decoder")]
pub use wrt_decoder::decoder_no_alloc;
// Re-export from wrt-decoder
#[cfg(feature = "decoder")]
pub use wrt_decoder::{
    component::decode::decode_component,
    // Note: component::parse is private, use decode_component() instead
    component::validation,
};

// Placeholder for DecodedComponent until proper import is fixed
pub type DecodedComponent = u32;

// Placeholder for ResourceHandle - seems to be defined in multiple places
pub type ResourceHandle = u32;
// Note: sections moved to decoder_no_alloc or not available
// Re-export from wrt-error
pub use wrt_error::{Error, ErrorCategory, Result, codes, kinds};

// Re-export from wrt-format
pub use wrt_format::component::ValType as FormatValType;
// Import component builders and resource builders with proper feature gates
#[cfg(feature = "std")]
pub use wrt_foundation::builder::ResourceItemBuilder;
#[cfg(feature = "std")]
pub use wrt_foundation::component_builder::{
    ComponentTypeBuilder, ExportBuilder, ImportBuilder, NamespaceBuilder,
};
#[cfg(not(feature = "std"))]
pub use wrt_foundation::component_value::{ComponentValue, ValType};
// Re-export component_value for both std and no_std
#[cfg(feature = "std")]
pub use wrt_foundation::MemoryProvider;
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::{ComponentValue, ValType};
// Binary std/no_std choice - remove conflicting type aliases
#[cfg(not(feature = "std"))]
pub use wrt_foundation::{
    MemoryProvider,
    bounded_collections::{BoundedMap, BoundedSet},
};

// Unified type aliases for std/no_std compatibility
#[cfg(not(feature = "std"))]
pub type ComponentVec<T> = wrt_foundation::collections::StaticVec<T, 64>;

#[cfg(feature = "std")]
pub type ComponentVec<T> = Vec<T>;

// Re-export from wrt-foundation
pub use wrt_foundation::{
    // Common types
    ExternType,
    bounded::{BoundedStack, BoundedString, BoundedVec, MAX_WASM_NAME_LENGTH},
    // Budget management
    budget_aware_provider::CrateId,
    // Builtin types
    builtin::BuiltinType,
    component::ComponentType,
    // Resource types
    resource::{ResourceOperation, ResourceType},
    safe_managed_alloc,
    // Memory providers
    safe_memory::NoStdProvider,
    // SafeMemory types
    safe_memory::{SafeMemoryHandler, SafeSlice, SafeStack},
    types::{BlockType, FuncType, GlobalType, MemoryType, TableType, ValueType},
    values::Value,
    // Verification types
    verification::VerificationLevel,
};
// Re-export from wrt-host
pub use wrt_host::{
    builder::HostBuilder,
    callback::{CallbackRegistry, CallbackType},
    function::{CloneableFn, HostFunctionHandler},
    host::BuiltinHost,
};
// Re-export from wrt-intercept - commented out until available
// pub use wrt_intercept::{
//     // Builtin interceptors
//     builtins::{BeforeBuiltinResult, BuiltinInterceptor, BuiltinSerialization,
// InterceptContext},     InterceptionResult,
//
//     // Core interception types
//     LinkInterceptor,
//     LinkInterceptorStrategy,
//     Modification,
// };
// Import synchronization primitives for no_std
#[cfg(not(feature = "std"))]
pub use wrt_sync::{Mutex, RwLock};

// Include debug logging macro (crate-internal only)
// pub use crate::debug_println;
// Re-export Instant for no_std environments
pub use crate::resources::Instant;
// Additional no_std specific re-exports
// Re-export from this crate conditionally based on std/no_std
#[cfg(feature = "std")]
pub use crate::{
    // Builtins
    builtins::{BuiltinHandler, BuiltinRegistry},
    // Canonical ABI
    canonical_abi::CanonicalABI,
    components::ComponentRegistry,
    // Component model core types
    components::{Component, ExternValue, FunctionValue, GlobalValue, MemoryValue, TableValue},
    // Execution context
    // execution::{TimeBoundedConfig, TimeBoundedContext, TimeBoundedOutcome},
    // Export/Import
    // export::Export,
    // export_map::{ExportMap, SafeExportMap},
    // Factory and instance
    // factory::ComponentFactory,
    // Host and namespace
    // host::Host,
    // import::Import,
    // import_map::{ImportMap, SafeImportMap},
    // instance::InstanceValue,
    // namespace::Namespace,
    // Resources
    resources::{
        // BufferPool,
        MemoryStrategy,
        ResourceManager,
        // ResourceOperation as RuntimeResourceOperation,
        ResourceTable,
    },
    // String encoding
    string_encoding::StringEncoding,
    // Memory strategies
    // strategies::memory::{
    //     BoundedCopyStrategy, FullIsolationStrategy, MemoryOptimizationStrategy,
    // ZeroCopyStrategy, },
    // Type conversion
    // type_conversion::{
    //     format_to_types_extern_type, format_val_type_to_value_type,
    //     format_valtype_to_types_valtype, types_to_format_extern_type,
    //     types_valtype_to_format_valtype, value_type_to_format_val_type, IntoFormatType,
    //     IntoRuntimeType,
    // },
    // Types and values
    types::{ComponentInstance, TaskId},
    // values::{
    //     component_to_core_value, core_to_component_value, deserialize_component_value,
    //     serialize_component_value,
    // },
};
// Re-export from this crate for no_std environments
#[cfg(not(feature = "std"))]
pub use crate::{
    // Builtins
    builtins::{BuiltinHandler, BuiltinRegistry},
    // Canonical ABI
    canonical_abi::CanonicalABI,
    components::ComponentRegistry,
    // Component model core types
    components::{Component, ExternValue, FunctionValue, GlobalValue, MemoryValue, TableValue},
    // component_value_no_std::{
    //     convert_format_to_valtype, convert_valtype_to_format, serialize_component_value_no_std,
    // },
    // Execution context
    // execution::{TimeBoundedConfig, TimeBoundedContext, TimeBoundedOutcome},
    // Export/Import
    // export::Export,
    // export_map::{ExportMap, SafeExportMap},
    // Factory and instance
    // factory::ComponentFactory,
    // Host and namespace
    // host::Host,
    // import::Import,
    // import_map::{ImportMap, SafeImportMap},
    // instance_no_std::{InstanceCollection, InstanceValue, InstanceValueBuilder},
    // namespace::Namespace,
    // Resources
    resources::{
        // BoundedBufferPool,
        // MemoryStrategy,  // Commented out due to resource_table_no_std being disabled
        // ResourceArena,
        ResourceManager,
        // ResourceOperation as RuntimeResourceOperation, ResourceStrategyNoStd,
        // ResourceTable,  // Commented out due to resource_table_no_std being disabled
    },
    // String encoding
    string_encoding::StringEncoding,
    // Memory strategies
    // strategies::memory::{
    //     BoundedCopyStrategy, FullIsolationStrategy, MemoryOptimizationStrategy,
    // ZeroCopyStrategy, },
    // Type conversion
    // type_conversion::{
    //     format_to_types_extern_type, format_val_type_to_value_type,
    //     format_valtype_to_types_valtype, types_to_format_extern_type,
    //     types_valtype_to_format_valtype, value_type_to_format_val_type, IntoFormatType,
    //     IntoRuntimeType,
    // },
    // Types and values
    types::{ComponentInstance, TaskId},
};
// ComponentValue already imported above

// Re-export ComponentProvider for convenience
pub use crate::bounded_component_infra::ComponentProvider;

/// Unified ComponentValue type - generic over provider
pub type WrtComponentValue<P> = ComponentValue<P>;

/// Unified ValType - generic over provider
pub type WrtValType<P> = wrt_foundation::component_value::ValType<P>;

/// Unified ComponentType - generic over provider
pub type WrtComponentType<P> = wrt_foundation::component::ComponentType<P>;

/// Unified ExternType - generic over provider
pub type WrtExternType<P> = wrt_foundation::ExternType<P>;

// Re-export from wrt_runtime for types used in wrt-component
pub use wrt_runtime::stackless::EngineState;

// Type aliases for compatibility
pub type Limits = wrt_foundation::types::Limits;

// Re-export ExportKind from parser_integration (component-specific)
// This is the component model ExportKind with index fields
pub use crate::parser_integration::ExportKind;
