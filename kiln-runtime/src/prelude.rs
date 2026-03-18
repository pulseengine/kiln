//! Prelude module for kiln-runtime
//!
//! This module provides a unified set of imports for the runtime.
//! The interpreter is std-only per the RFC #46 architecture decision.

// Re-export from std
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

// Re-export from kiln-error for error handling
pub use kiln_error::prelude::{
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

// Re-export from kiln-format
pub use kiln_format::component::Component as FormatComponent;
pub use kiln_format::{
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
pub use kiln_foundation::clean_core_types::{
    CoreGlobalType,
    CoreMemoryType,
    CoreTableType,
};
pub use kiln_foundation::clean_types::ExternType as CleanExternType;
pub use kiln_foundation::component::ComponentType;

// Import required traits
pub use kiln_foundation::traits::{
    Checksummable,
    FromBytes,
    ToBytes,
};

// Re-export core types from kiln_foundation
pub use kiln_foundation::types::CustomSection;
pub use kiln_foundation::{
    prelude::{
        BoundedStack,
        BoundedVec,
        ComponentValue,
        ResourceType,
        SafeMemoryHandler,
        SafeSlice,
        ValType as ComponentValType,
        ValueType,
        VerificationLevel,
    },
    safe_memory::SafeStack,
    traits::BoundedCapacity,
    types::{
        DataSegment,
        ElementSegment,
        Limits,
        RefValue,
    },
    MemoryStats,
};

// Use foundation Value directly for runtime
pub use kiln_foundation::{
    types::FuncType as CleanFuncType,
    types::ValueType as CleanValType,
    GlobalType as CleanGlobalType,
    MemoryType as CleanMemoryType,
    TableType as CleanTableType,
    Value as CleanValue,
};

/// Core function type
pub type CoreFuncType = kiln_foundation::types::FuncType;
/// Type alias for WebAssembly function types
pub type FuncType = kiln_foundation::types::FuncType;
/// Type alias for WebAssembly memory types
pub type MemoryType = CoreMemoryType;
/// Type alias for WebAssembly table types
pub type TableType = CoreTableType;
/// Type alias for WebAssembly global types
pub type GlobalType = CoreGlobalType;
/// Type alias for WebAssembly values
pub type Value = CleanValue;
/// Type alias for WebAssembly external types
pub type ExternType = CleanExternType;
/// Default memory provider factory with 64KB allocation capacity
pub type DefaultProviderFactory = kiln_foundation::type_factory::RuntimeFactory64K;
/// Factory for internal allocation
pub type DefaultFactory = kiln_foundation::type_factory::RuntimeFactory64K;
/// Runtime function type alias
pub type RuntimeFuncType = FuncType;
/// Runtime string type alias
pub type RuntimeString = String;

// Re-export from kiln-host
pub use kiln_host::prelude::CallbackRegistry as HostFunctionRegistry;
pub use kiln_host::prelude::HostFunctionHandler as HostFunction;

pub use kiln_instructions::{
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

/// Unified instruction type for WebAssembly operations
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum Instruction {
    /// No operation instruction
    #[default]
    Nop,
    /// Arithmetic operation instruction
    Arithmetic(ArithmeticOp),
    /// Control flow operation instruction
    Control(ControlOp),
    /// Function call instruction
    Call(u32),
}

impl kiln_foundation::traits::Checksummable for Instruction {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        match self {
            Instruction::Nop => {
                checksum.update_slice(&[0u8]);
            },
            Instruction::Arithmetic(_op) => {
                checksum.update_slice(&[1u8]);
            },
            Instruction::Control(_op) => {
                checksum.update_slice(&[2u8]);
            },
            Instruction::Call(func_idx) => {
                checksum.update_slice(&[3u8]);
                checksum.update_slice(&func_idx.to_le_bytes());
            },
        }
    }
}

impl kiln_foundation::traits::ToBytes for Instruction {
    fn serialized_size(&self) -> usize {
        1 + match self {
            Instruction::Nop => 0,
            Instruction::Arithmetic(_) => 4,
            Instruction::Control(_) => 4,
            Instruction::Call(_) => 4,
        }
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
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

impl kiln_foundation::traits::FromBytes for Instruction {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
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
            _ => Err(kiln_error::Error::runtime_execution_error(
                "Unsupported instruction discriminant",
            )),
        }
    }
}

// Re-export from kiln-intercept
pub use kiln_intercept::prelude::{
    LinkInterceptor as InterceptorRegistry,
    LinkInterceptorStrategy as InterceptStrategy,
};

// Execution related types
pub use crate::execution::{
    ExecutionContext,
    ExecutionStats,
};

pub use crate::global::Global;

pub use crate::module::{
    Data,
    Element,
    Export,
    ExportItem,
    ExportKind,
    Import,
    OtherExport,
};

pub use crate::{
    memory::Memory,
    module::Module as RuntimeModule,
    module_instance::ModuleInstance as RuntimeModuleInstance,
    table::Table,
};
