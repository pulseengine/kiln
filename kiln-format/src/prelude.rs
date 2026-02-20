//! Prelude module for kiln-format
//!
//! This module provides a unified set of imports for both std and no_std
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the Kiln project and simplify imports in
//! individual modules.

// Core imports for both std and no_std environments
// Import synchronization primitives for no_std
// For a no_std implementation, we should use appropriate no_std compatible sync
// primitives since kiln-sync is not implemented yet, we use spin-based locks as
// a placeholder
#[cfg(not(feature = "std"))]
pub use core::cell::{
    Cell,
    RefCell,
};
pub use core::{
    any::Any,
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
    fmt,
    fmt::{
        Debug,
        Display,
    },
    marker::PhantomData,
    mem,
    ops::{
        Deref,
        DerefMut,
    },
    slice,
    str,
};

// Re-export from kiln-error
pub use kiln_error::{
    codes,
    kinds,
    Error,
    ErrorCategory,
    FromError,
    Result,
    ToErrorCategory,
};
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use kiln_foundation::component_value::{
    ComponentValue,
    ValType,
};
#[cfg(feature = "std")]
pub use kiln_foundation::safe_memory::StdProvider as StdMemoryProvider;
// Re-export additional clean types when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use kiln_foundation::CleanExternType;
// Conditional imports for safety features
#[cfg(feature = "safety")]
pub use kiln_foundation::MemoryProvider;
// No-std memory provider
#[cfg(all(feature = "safety", not(feature = "std")))]
pub use kiln_foundation::NoStdProvider as NoStdMemoryProvider;
// Re-export clean types from kiln-foundation
pub use kiln_foundation::{
    // Legacy types for compatibility
    types::{
        BlockType,
        RefType,
        ValueType,
    },
    values::Value,
    // Verification types
    verification::VerificationLevel,
    BoundedStack,
    // SafeMemory types
    SafeMemoryHandler,
    SafeSlice,
};
#[cfg(not(any(feature = "std")))]
pub use kiln_foundation::{
    BoundedMap,
    BoundedString,
    BoundedVec,
};
// Clean types without provider parameters - only when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use kiln_foundation::{
    CleanFuncType,
    CleanGlobalType,
    CleanMemoryType,
    CleanTableType,
    CleanValType,
    CleanValue,
};
// Re-export type factory types - only when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use kiln_foundation::{
    ComponentTypeFactory,
    RuntimeTypeFactory,
    TypeFactory,
};

// Re-export from this crate's modules
pub use crate::{
    // Binary module constants and functions
    binary::{
        read_leb128_u32,
        read_string,
        WASM_MAGIC,
        WASM_VERSION,
    },
    // Conversion utilities
    conversion::{
        block_type_to_format_block_type,
        convert,
        format_block_type_to_block_type,
        format_limits_to_kiln_limits,
        format_value_type,
        format_value_type as value_type_to_byte,
        parse_value_type,
        validate,
        validate_format,
        validate_option,
        kiln_limits_to_format_limits,
    },
    // Error conversion utilities
    error::{
        parse_error,
        runtime_error,
        to_kiln_error,
        type_error,
        validation_error,
        kiln_runtime_error,
        kiln_type_error,
        kiln_validation_error,
        IntoError,
    },
    // Section constants
    section::{
        CODE_ID,
        CUSTOM_ID,
        DATA_COUNT_ID,
        DATA_ID,
        ELEMENT_ID,
        EXPORT_ID,
        FUNCTION_ID,
        GLOBAL_ID,
        IMPORT_ID,
        MEMORY_ID,
        START_ID,
        TABLE_ID,
        TYPE_ID,
    },
};
// Re-export collection types for no_std
#[cfg(not(any(feature = "std")))]
pub use crate::{
    WasmString,
    WasmVec,
};

// Helper functions for memory safety

/// Create a SafeSlice from a byte slice
#[cfg(feature = "safety")]
pub fn safe_slice(data: &[u8]) -> kiln_error::Result<kiln_foundation::safe_memory::SafeSlice<'_>> {
    kiln_foundation::safe_memory::SafeSlice::new(data)
}

/// Create a SafeSlice with specific verification level
#[cfg(feature = "safety")]
pub fn safe_slice_with_verification(
    data: &[u8],
    level: kiln_foundation::verification::VerificationLevel,
) -> kiln_error::Result<kiln_foundation::safe_memory::SafeSlice<'_>> {
    kiln_foundation::safe_memory::SafeSlice::with_verification_level(data, level)
}

/// Create a memory provider from a byte slice (changed from Vec<u8>)
#[cfg(all(feature = "safety", feature = "std"))] // StdMemoryProvider likely needs std
pub fn memory_provider(data: &[u8]) -> kiln_foundation::safe_memory::StdProvider {
    // StdMemoryProvider::new takes Vec, this needs adjustment or StdMemoryProvider
    // needs a from_slice For now, let's assume StdMemoryProvider can be created
    // from a slice or this function is std-only. This function is problematic
    // if StdMemoryProvider strictly needs owned Vec. A proper fix would be for
    // StdMemoryProvider to have a method that takes a slice if appropriate,
    // or this helper should be cfg-gated more strictly or use a different provider
    // for no_std. Tentatively, creating a Vec here if std is available.
    kiln_foundation::safe_memory::StdProvider::new(data.to_vec())
}

/// Create a memory provider with specific capacity
#[cfg(all(feature = "safety", feature = "std"))] // StdMemoryProvider likely needs std
pub fn memory_provider_with_capacity(capacity: usize) -> kiln_foundation::safe_memory::StdProvider {
    kiln_foundation::safe_memory::StdProvider::with_capacity(capacity)
}

// Factory function for creating providers using capability system
#[cfg(not(feature = "std"))]
pub fn create_format_provider<const N: usize>() -> kiln_error::Result<
    kiln_foundation::capabilities::CapabilityAwareProvider<kiln_foundation::NoStdProvider<N>>,
> {
    use kiln_foundation::{
        capability_context,
        safe_capability_alloc,
        CrateId,
    };
    let context: kiln_error::Result<_> = capability_context!(dynamic(CrateId::Format, N));
    let context = context?;
    safe_capability_alloc!(context, CrateId::Format, N)
}

/// The prelude trait
pub trait Prelude {}

/// Standard prelude for the format library
pub mod std_prelude {
    // External crate imports
    // Result type
    pub use kiln_error::Result;
    // Binary std/no_std choice
    #[cfg(feature = "std")]
    pub use kiln_foundation::component_value::ValType;
    // Base types from kiln_foundation - fix incorrect paths
    pub use kiln_foundation::{
        // These types appear to be from the component module
        component::ComponentType,
        // SafeMemory types
        safe_memory::{
            SafeMemoryHandler,
            SafeSlice,
            SafeStack,
        },
        // Import correctly from types module
        types::{
            BlockType,
            FuncType,
            GlobalType,
            Limits,
            MemoryType,
            RefType,
            TableType,
            ValueType,
        },
        // Verification
        verification::VerificationLevel,
    };

    // Explicitly re-export conversion utilities
    pub use crate::conversion::{
        block_type_to_format_block_type,
        convert,
        format_block_type_to_block_type,
        format_limits_to_kiln_limits,
        format_value_type,
        format_value_type as value_type_to_byte,
        parse_value_type,
        validate,
        validate_format,
        validate_option,
        kiln_limits_to_format_limits,
    };
    // Error handling
    pub use crate::error::{
        parse_error,
        runtime_error,
        to_kiln_error,
        type_error,
        validation_error,
        kiln_runtime_error,
        kiln_type_error,
        kiln_validation_error,
        IntoError,
    };
    // Format types - fix incorrect modules
    pub use crate::{
        binary,
        types::FormatBlockType,
        validation::Validatable,
    };
    #[cfg(feature = "std")]
    pub use crate::{
        component::Component,
        module::Module,
    };
}

#[cfg(feature = "std")]
impl Prelude for crate::component::Component {}

/// No-std prelude for the format library
pub mod no_std_prelude {
    // Re-export collection types for no_std
    // External crate imports
    // Base error types from kiln_error
    pub use kiln_error::{
        codes,
        kinds,
        Error,
        ErrorCategory,
        FromError,
        Result,
        ToErrorCategory,
    };
    // Binary std/no_std choice
    #[cfg(feature = "std")]
    pub use kiln_foundation::component_value::ValType;
    // Base types from kiln_foundation - fix incorrect paths
    pub use kiln_foundation::{
        // These types appear to be from the component module
        component::ComponentType,
        // SafeMemory types
        safe_memory::{
            SafeMemoryHandler,
            SafeSlice,
            SafeStack,
        },
        // Import correctly from types module
        types::{
            BlockType,
            FuncType,
            GlobalType,
            Limits,
            MemoryType,
            RefType,
            TableType,
            ValueType,
        },
        // Verification
        verification::VerificationLevel,
    };
    #[cfg(not(any(feature = "std")))]
    pub use kiln_foundation::{
        BoundedMap,
        BoundedString,
        BoundedVec,
    };

    // Explicitly re-export conversion utilities
    pub use crate::conversion::{
        block_type_to_format_block_type,
        convert,
        format_block_type_to_block_type,
        format_limits_to_kiln_limits,
        format_value_type,
        format_value_type as value_type_to_byte,
        parse_value_type,
        validate,
        validate_format,
        validate_option,
        kiln_limits_to_format_limits,
    };
    // Error handling
    pub use crate::error::{
        parse_error,
        runtime_error,
        to_kiln_error,
        type_error,
        validation_error,
        kiln_runtime_error,
        kiln_type_error,
        kiln_validation_error,
        IntoError,
    };
    // Format types - fix incorrect modules
    pub use crate::{
        binary,
        types::FormatBlockType,
        validation::Validatable,
    };
    #[cfg(feature = "std")]
    pub use crate::{
        component::Component,
        module::Module,
    };
    #[cfg(not(any(feature = "std")))]
    pub use crate::{
        WasmString,
        WasmVec,
    };
}
