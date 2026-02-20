//! Prelude module for wrt-runtime
//!
//! This module provides a unified set of imports for both std and `no_std`
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Core imports for both std and no_std environments

#[cfg(not(feature = "std"))]
extern crate alloc;

// Binary std/no_std choice
// Note: NoStdProvider import removed - use safe_managed_alloc! instead

// Platform-aware collection type aliases that adapt to target platform
// capabilities Note: These type aliases are now generic over the provider type
// P Users must specify their own provider when using these types
/// `BoundedHashMap` type for `no_std` environments with bounded capacity
#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub type BoundedHashMap<K, V, P> = wrt_foundation::bounded_collections::BoundedMap<K, V, 128, P>;

/// `BoundedHashSet` type for `no_std` environments with bounded capacity  
#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub type BoundedHashSet<T, P> = wrt_foundation::bounded_collections::BoundedSet<T, 128, P>;

// Platform-aware string and vector types
#[cfg(not(feature = "std"))]
pub use wrt_foundation::bounded::BoundedString;

// Helper macro to create Vec
/// Create a new Vec for `no_std` environments
#[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! vec_new {
    () => {
        Vec::new()
    };
}

// Helper function to create Vec with capacity
/// Create a Vec with specified capacity for `no_std` environments
#[cfg(not(feature = "std"))]
#[must_use]
pub fn vec_with_capacity<T>(capacity: usize) -> alloc::vec::Vec<T> {
    alloc::vec::Vec::with_capacity(capacity)
}

// Add vec! macro for no_std environments without alloc
/// Vec creation macro for pure `no_std` environments without alloc
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
#[macro_export]
macro_rules! vec {
    () => {
        Vec::new()
    };
    ($elem:expr; $n:expr) => {
        {
            let mut v = Vec::new();
            for _ in 0..$n {
                v.push($elem);
            }
            v
        }
    };
    ($($x:expr),*) => {
        {
            let mut v = Vec::new();
            $(v.push($x);)*
            v
        }
    };
}

// Simple format! implementation for no_std mode using a fixed buffer
/// Simplified format macro for `no_std` environments
#[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! format {
    ($fmt:expr) => {{
        $fmt
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        $fmt // Simplified - just return the format string for no_std
    }};
}

// Re-export the macros for no_std
// Note: format macro is provided by alloc when available

// Helper functions for Option<Value> conversion
/// Convert Option<Value> to Option<i32> for `no_std` environments
#[cfg(not(feature = "std"))]
pub fn option_value_as_i32(value: &Option<wrt_foundation::Value>) -> Option<i32> {
    match value {
        Some(wrt_foundation::Value::I32(val)) => Some(*val),
        _ => None,
    }
}

/// Convert Option<Value> to Option<i64> for `no_std` environments
#[cfg(not(feature = "std"))]
pub fn option_value_as_i64(value: &Option<wrt_foundation::Value>) -> Option<i64> {
    match value {
        Some(wrt_foundation::Value::I64(val)) => Some(*val),
        _ => None,
    }
}

/// Convert Option<Value> to Option<f32> for `no_std` environments
#[cfg(not(feature = "std"))]
pub fn option_value_as_f32(value: &Option<wrt_foundation::Value>) -> Option<f32> {
    match value {
        Some(wrt_foundation::Value::F32(val)) => Some(val.value()),
        _ => None,
    }
}

/// Convert Option<Value> to Option<f64> for `no_std` environments
#[cfg(not(feature = "std"))]
pub fn option_value_as_f64(value: &Option<wrt_foundation::Value>) -> Option<f64> {
    match value {
        Some(wrt_foundation::Value::F64(val)) => Some(val.value()),
        _ => None,
    }
}

// ToString is provided by alloc when available

// Arc and Mutex for no_std with alloc
#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub use alloc::sync::Arc;

// For pure no_std without alloc, use reference wrapper
/// Arc-like reference wrapper for pure `no_std` environments without alloc
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Arc<T> {
    /// Inner value being wrapped
    inner: T,
}

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
impl<T> Arc<T> {
    /// Create a new Arc wrapper for pure `no_std` environments
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }

    /// Compare Arc pointers (always returns false in `no_std` mode)
    pub fn ptr_eq(_this: &Self, _other: &Self) -> bool {
        // In no_std mode, we can't do pointer comparison, so just return false
        false
    }
}

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
impl<T: PartialEq> PartialEq for Arc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
impl<T: Eq> Eq for Arc<T> {}

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
impl<T> core::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
// Re-export from alloc when available but not std
#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub use alloc::{
    boxed::Box,
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
    sync::atomic::{
        AtomicUsize,
        Ordering as AtomicOrdering,
    },
};
// Re-export from std when the std feature is enabled
#[cfg(feature = "std")]
pub use std::{
    boxed::Box,
    collections::{
        HashMap,
        HashSet,
    },
    format,
    string::{
        String,
        ToString,
    },
    sync::{
        Arc,
        Mutex,
        MutexGuard,
        RwLock,
        RwLockReadGuard,
        RwLockWriteGuard,
    },
    vec,
    vec::Vec,
};

// Provide String and ToString for pure no_std environments
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub use wrt_foundation::bounded::BoundedString as String;

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub trait ToString {
    fn to_string(&self) -> RuntimeString;
}

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
impl ToString for &str {
    fn to_string(&self) -> RuntimeString {
        RuntimeString::try_from_str(self).unwrap_or_else(|_| {
            // If conversion fails, create empty string
            RuntimeString::try_from_str("").unwrap()
        })
    }
}

// Remove duplicate definitions - Vec and String are already defined above

// Re-export from wrt-decoder (aliased to avoid name clashes)
// Component module is temporarily disabled in wrt-decoder
// #[cfg(feature = "std")]
// pub use wrt_decoder::component::Component as DecoderComponentDefinition;
// Re-export from wrt-instructions for instruction types
// Decoder imports are optional and may not be available
// pub use wrt_decoder::instructions::Instruction;
// pub use wrt_decoder::prelude::Module as DecoderModule;
// Re-export from wrt-error for error handling
pub use wrt_error::prelude::{
    kinds::{
        self,
        ComponentError,
        InvalidType,
        OutOfBoundsError,
        ParseError,
        ResourceError,
        RuntimeError,
        ValidationError,
    },
    Error,
    ErrorCategory,
    Result,
};
// Re-export from wrt-format for format specifications (aliased to avoid name clashes)
#[cfg(feature = "std")]
pub use wrt_format::component::Component as FormatComponent;
#[cfg(feature = "std")]
pub use wrt_format::{
    module::{
        Element as FormatElement,
        Export as FormatExport,
        ExportKind as FormatExportKind,
        Function as FormatFunction,
        Global as FormatGlobal,
        Import as FormatImport,
        ImportDesc as FormatImportDesc,
        Memory as FormatMemory,
        Table as FormatTable,
    },
    pure_format_types::PureDataSegment as FormatData,
    section::CustomSection as FormatCustomSection,
};
// Clean core WebAssembly types (for runtime use)
#[cfg(any(feature = "std", feature = "alloc"))]
pub use wrt_foundation::clean_core_types::{
    CoreGlobalType,
    CoreMemoryType,
    CoreTableType,
};
// For ExternType, use the clean_types version which doesn't have provider parameters
#[cfg(any(feature = "std", feature = "alloc"))]
pub use wrt_foundation::clean_types::ExternType as CleanExternType;
// Re-export from wrt-foundation for core types
#[cfg(feature = "std")]
pub use wrt_foundation::component::ComponentType;
// Import required traits (these should already be implemented by wrt_foundation::Value)
pub use wrt_foundation::traits::{
    Checksummable,
    FromBytes,
    ToBytes,
};
// Re-export core types from wrt_foundation instead of wrt_format
pub use wrt_foundation::types::{
    CustomSection, /* Assuming this is the intended replacement for FormatCustomSection
                    * Add other direct re-exports from wrt_foundation::types if they were
                    * previously from wrt_format::module e.g., DataSegment,
                    * ElementSegment, Export, GlobalType, Import, MemoryType, TableType,
                    * FuncType For now, only replacing what was directly
                    * aliased or used in a way that implies a direct replacement need. */
};
pub use wrt_foundation::{
    prelude::{
        BoundedStack,
        BoundedVec,
        ResourceType,
        SafeMemoryHandler,
        SafeSlice,
        ValueType,
        VerificationLevel,
    },
    safe_memory::SafeStack,
    traits::BoundedCapacity, // Add trait for len(), is_empty(), etc.
    types::{
        DataSegment,
        ElementSegment,
        Limits,
        RefValue,
    },
    MemoryStats,
};
// Use foundation Value directly for runtime (not clean_types)
// This ensures compatibility with BoundedVec and bounded collections
pub use wrt_foundation::{
    types::FuncType as CleanFuncType,
    types::ValueType as CleanValType,
    GlobalType as CleanGlobalType,
    MemoryType as CleanMemoryType,
    TableType as CleanTableType,
    Value as CleanValue,
};

/// Core function type from wrt_foundation.
#[cfg(any(feature = "std", feature = "alloc"))]
pub type CoreFuncType = wrt_foundation::types::FuncType;

// Fallback for no_std environments - provide core types
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub use wrt_foundation::types::{
    GlobalType as CoreGlobalType,
    MemoryType as CoreMemoryType,
    TableType as CoreTableType,
};

#[cfg(not(any(feature = "std", feature = "alloc")))]
pub type CoreFuncType = wrt_foundation::types::FuncType;

// Public type aliases using clean CORE types (not component types)
/// Type alias for WebAssembly function types
#[cfg(any(feature = "std", feature = "alloc"))]
pub type FuncType = wrt_foundation::types::FuncType;
/// Type alias for WebAssembly memory types
#[cfg(any(feature = "std", feature = "alloc"))]
pub type MemoryType = CoreMemoryType;
/// Type alias for WebAssembly table types
#[cfg(any(feature = "std", feature = "alloc"))]
pub type TableType = CoreTableType;
/// Type alias for WebAssembly global types
#[cfg(any(feature = "std", feature = "alloc"))]
pub type GlobalType = CoreGlobalType;
// Note: ValType doesn't need aliasing - ValueType from core is already used
/// Type alias for WebAssembly values
#[cfg(any(feature = "std", feature = "alloc"))]
pub type Value = CleanValue;
/// Type alias for WebAssembly external types
#[cfg(any(feature = "std", feature = "alloc"))]
pub type ExternType = CleanExternType;

// Factory for internal allocation when needed
#[cfg(any(feature = "std", feature = "alloc"))]
pub use wrt_foundation::type_factory::RuntimeFactory64K as DefaultFactory;
// Fallback for no-alloc environments - use legacy provider-based types temporarily
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub use wrt_foundation::types::{
    FuncType,
    GlobalType,
    MemoryType,
    TableType,
    ValueType as ValType,
};

// Default provider factory for capability-based allocation
/// Default memory provider factory with 64KB allocation capacity
#[cfg(any(feature = "std", feature = "alloc"))]
pub type DefaultProviderFactory = wrt_foundation::type_factory::RuntimeFactory64K;

/// Default memory provider factory for pure no_std (smaller capacity)
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub type DefaultProviderFactory = wrt_foundation::type_factory::RuntimeFactory8K;

/// Runtime function type alias for consistency
#[cfg(any(feature = "std", feature = "alloc"))]
pub type RuntimeFuncType = FuncType;
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub type RuntimeFuncType = wrt_foundation::types::FuncType;

/// Runtime string type alias for consistency
#[cfg(feature = "std")]
pub type RuntimeString = String;
#[cfg(not(feature = "std"))]
pub type RuntimeString = wrt_foundation::bounded::BoundedString<256>;

// Safety-critical wrapper types for runtime (deterministic, verifiable)
// SIMD execution integration

// Binary std/no_std choice
#[cfg(feature = "std")]
pub use wrt_foundation::prelude::{
    ComponentValue,
    ValType as ComponentValType,
};
// Re-export from wrt-host (for runtime host interaction items)
#[cfg(feature = "std")]
pub use wrt_host::prelude::CallbackRegistry as HostFunctionRegistry;
#[cfg(feature = "std")]
pub use wrt_host::prelude::HostFunctionHandler as HostFunction;
pub use wrt_instructions::{
    arithmetic_ops::ArithmeticOp,
    control_ops::{
        BranchTarget as Label,
        ControlOp,
    },
    instruction_traits::PureInstruction as InstructionExecutor,
};

pub use crate::module::{
    GlobalWrapper as RuntimeGlobal,
    MemoryWrapper as RuntimeMemory,
    TableWrapper as RuntimeTable,
};

// Temporary instruction type until unified enum is available
/// Unified instruction type for WebAssembly operations
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum Instruction {
    /// No operation instruction
    #[default]
    Nop, // Unit variant for default
    /// Arithmetic operation instruction
    Arithmetic(ArithmeticOp),
    /// Control flow operation instruction
    Control(ControlOp),
    /// Function call instruction (simplified for compatibility)
    Call(u32),
    // Add other variants as needed
}


// Implement required traits for Instruction
impl wrt_foundation::traits::Checksummable for Instruction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Instruction::Nop => {
                checksum.update_slice(&[0u8]); // Variant discriminant for Nop
            },
            Instruction::Arithmetic(op) => {
                checksum.update_slice(&[1u8]); // Variant discriminant
                                               // ArithmeticOp would need to
                                               // implement Checksummable
            },
            Instruction::Control(op) => {
                checksum.update_slice(&[2u8]); // Variant discriminant
                                               // ControlOp would need to
                                               // implement Checksummable
            },
            Instruction::Call(func_idx) => {
                checksum.update_slice(&[3u8]); // Variant discriminant
                checksum.update_slice(&func_idx.to_le_bytes());
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for Instruction {
    fn serialized_size(&self) -> usize {
        1 + match self {
            Instruction::Nop => 0,           // No additional data
            Instruction::Arithmetic(_) => 4, // Placeholder size
            Instruction::Control(_) => 4,    // Placeholder size
            Instruction::Call(_) => 4,       // Function index size
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        match self {
            Instruction::Nop => writer.write_all(&[0u8])?,
            Instruction::Arithmetic(_) => writer.write_all(&[1u8])?,
            Instruction::Control(_) => writer.write_all(&[2u8])?,
            Instruction::Call(func_idx) => {
                writer.write_all(&[3u8])?;
                writer.write_all(&func_idx.to_le_bytes())?;
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for Instruction {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut discriminant = [0u8; 1];
        reader.read_exact(&mut discriminant)?;
        match discriminant[0] {
            0 => Ok(Instruction::Nop),
            1 => Ok(Instruction::Arithmetic(ArithmeticOp::default())),
            2 => Ok(Instruction::Control(ControlOp::default())),
            3 => {
                let mut func_bytes = [0u8; 4];
                reader.read_exact(&mut func_bytes)?;
                let func_idx = u32::from_le_bytes(func_bytes);
                Ok(Instruction::Call(func_idx))
            },
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Unsupported instruction discriminant",
            )),
        }
    }
}
// Re-export from wrt-intercept (for runtime interception items)
pub use wrt_intercept::prelude::{
    LinkInterceptor as InterceptorRegistry,
    LinkInterceptorStrategy as InterceptStrategy,
};
// Binary std/no_std choice
#[cfg(not(feature = "std"))]
pub use wrt_sync::{
    WrtMutex as Mutex,
    WrtMutexGuard as MutexGuard,
    WrtRwLock as RwLock,
    WrtRwLockReadGuard as RwLockReadGuard,
    WrtRwLockWriteGuard as RwLockWriteGuard,
};

// Execution related types defined in wrt-runtime
pub use crate::execution::{
    ExecutionContext,
    ExecutionStats,
}; /* Removed ExecutionResult as
    * it's not defined in
    * execution.rs */
// --- Local definitions from wrt-runtime ---
// These are types defined within wrt-runtime itself, re-exported for convenience if used
// widely.

// Core runtime structures
// pub use crate::func::Function; // Removed: func.rs only re-exports FuncType from
// wrt_foundation, which is already in prelude. RuntimeFunction (from module.rs) is the primary
// Function struct for the runtime.
pub use crate::global::Global;
// Adapters and helpers if they are part of the public API exported by this prelude
// Temporarily disabled - memory_adapter module is disabled
// pub use crate::memory_adapter::MemoryAdapter;
// Module items specific to wrt-runtime module structure
pub use crate::module::{
    Data,
    Element,
    Export,
    ExportItem,
    ExportKind,
    Import,
    OtherExport,
};
// Stackless execution engine components - temporarily disabled
// pub use crate::stackless::{
//     StacklessCallbackRegistry, StacklessEngine, StacklessExecutionState, StacklessFrame,
//     StacklessStack,
// };
pub use crate::{
    memory::Memory,
    module::Module as RuntimeModule,
    module_instance::ModuleInstance as RuntimeModuleInstance,
    table::Table,
};

// The following re-exports from wrt_format are removed as wrt-runtime should
// not depend on wrt-format. pub use wrt_format::module::{ // REMOVED
// DataMode as FormatDataMode, Element as FormatElement, Export as FormatExport,
// // ... and others }; // REMOVED
// pub use wrt_format::section::CustomSection as FormatCustomSection; // REMOVED
// pub use wrt_format::component::Component as FormatComponent; // REMOVED
// (Component model types should come from wrt_component or wrt_foundation if
// foundational)
