//! Core module to component adapter
//!
//! This module provides adaptation between WebAssembly core modules and
//! components, allowing core modules to be used within the component model
//! ecosystem.

#[cfg(not(feature = "std"))]
use alloc::{format, vec};
#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_foundation::RefType;
use wrt_foundation::collections::StaticVec as BoundedVec;
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
use wrt_foundation::types::FuncType as WrtFuncType;
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    BoundedString, bounded::MAX_WASM_NAME_LENGTH, budget_aware_provider::CrateId,
    safe_managed_alloc, safe_memory::NoStdProvider,
};

#[cfg(not(feature = "std"))]
// For no_std, use a simpler ComponentValue representation
use crate::types::Value as ComponentValue;
use crate::{
    bounded_component_infra::ComponentProvider,
    canonical_abi::CanonicalABI,
    components::Component,
    execution_engine::ComponentExecutionEngine,
    prelude::*,
    types::{ValType, Value},
};

use crate::export::Export;

#[cfg(feature = "std")]
use crate::components::component::{
    ExternValue, FunctionValue, GlobalValue, MemoryValue, TableValue,
};
#[cfg(not(feature = "std"))]
use crate::components::component_no_std::{
    ExternValue, FunctionValue, GlobalValue, MemoryValue, TableValue,
};

/// Maximum number of adapted functions in no_std environments
const MAX_ADAPTED_FUNCTIONS: usize = 256;

/// Adapter that wraps a core WebAssembly module for use in components
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoreModuleAdapter {
    /// Module name/identifier
    #[cfg(feature = "std")]
    pub name: String,
    #[cfg(not(any(feature = "std",)))]
    pub name: BoundedString<64>,

    /// Function adapters
    #[cfg(feature = "std")]
    pub functions: Vec<FunctionAdapter>,
    #[cfg(not(any(feature = "std",)))]
    pub functions: BoundedVec<FunctionAdapter, MAX_ADAPTED_FUNCTIONS>,

    /// Memory adapters
    #[cfg(feature = "std")]
    pub memories: Vec<MemoryAdapter>,
    #[cfg(not(any(feature = "std",)))]
    pub memories: BoundedVec<MemoryAdapter, 16>,

    /// Table adapters
    #[cfg(feature = "std")]
    pub tables: Vec<TableAdapter>,
    #[cfg(not(any(feature = "std",)))]
    pub tables: BoundedVec<TableAdapter, 16>,

    /// Global adapters
    #[cfg(feature = "std")]
    pub globals: Vec<GlobalAdapter>,
    #[cfg(not(any(feature = "std",)))]
    pub globals: BoundedVec<GlobalAdapter, 64>,
}

/// Adapter for core module functions
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionAdapter {
    /// Core function index
    pub core_index: u32,
    /// Component function signature
    pub component_signature: WrtComponentType<ComponentProvider>,
    /// Core function signature (WebAssembly types)
    pub core_signature: CoreFunctionSignature,
    /// Adaptation mode
    pub mode: AdaptationMode,
}

/// Core WebAssembly function signature
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreFunctionSignature {
    /// Parameter types (WebAssembly core types)
    #[cfg(feature = "std")]
    pub params: Vec<CoreValType>,
    #[cfg(not(any(feature = "std",)))]
    pub params: BoundedVec<CoreValType, 32>,
    /// Result types (WebAssembly core types)
    #[cfg(feature = "std")]
    pub results: Vec<CoreValType>,
    #[cfg(not(any(feature = "std",)))]
    pub results: BoundedVec<CoreValType, 8>,
}

/// WebAssembly core value types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoreValType {
    #[default]
    /// 32-bit integer
    I32,
    /// 64-bit integer
    I64,
    /// 32-bit float
    F32,
    /// 64-bit float
    F64,
    /// 128-bit vector (SIMD)
    V128,
    /// Function reference
    FuncRef,
    /// External reference
    ExternRef,
}

/// Function adaptation mode
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AdaptationMode {
    #[default]
    /// Direct mapping (no adaptation needed)
    Direct,
    /// Lift core types to component types
    Lift,
    /// Lower component types to core types
    Lower,
    /// Bidirectional adaptation
    Bidirectional,
}

/// Memory adapter
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryAdapter {
    /// Core memory index
    pub core_index: u32,
    /// Memory limits
    pub limits: MemoryLimits,
    /// Shared flag
    pub shared: bool,
}

/// Memory limits
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryLimits {
    /// Minimum size in pages
    pub min: u32,
    /// Maximum size in pages (if any)
    pub max: Option<u32>,
}

/// Table adapter
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableAdapter {
    /// Core table index
    pub core_index: u32,
    /// Element type
    pub element_type: CoreValType,
    /// Table limits
    pub limits: TableLimits,
}

/// Table limits
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableLimits {
    /// Minimum size
    pub min: u32,
    /// Maximum size (if any)
    pub max: Option<u32>,
}

/// Global adapter
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalAdapter {
    /// Core global index
    pub core_index: u32,
    /// Global type
    pub global_type: CoreValType,
    /// Mutability
    pub mutable: bool,
}

impl CoreModuleAdapter {
    /// Create a new core module adapter
    #[cfg(feature = "std")]
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: Vec::new(),
            memories: Vec::new(),
            tables: Vec::new(),
            globals: Vec::new(),
        }
    }

    /// Create a new core module adapter (no_std version)
    #[cfg(not(any(feature = "std",)))]
    pub fn new(name: BoundedString<64>) -> core::result::Result<Self, Error> {
        Ok(Self {
            name,
            functions: BoundedVec::new(),
            memories: BoundedVec::new(),
            tables: BoundedVec::new(),
            globals: BoundedVec::new(),
        })
    }

    /// Add a function adapter
    pub fn add_function(&mut self, adapter: FunctionAdapter) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.functions.push(adapter);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.functions
                .push(adapter)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many function adapters"))
        }
    }

    /// Add a memory adapter
    pub fn add_memory(&mut self, adapter: MemoryAdapter) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.memories.push(adapter);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.memories
                .push(adapter)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many memory adapters"))
        }
    }

    /// Add a table adapter
    pub fn add_table(&mut self, adapter: TableAdapter) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.tables.push(adapter);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.tables
                .push(adapter)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many table adapters"))
        }
    }

    /// Add a global adapter
    pub fn add_global(&mut self, adapter: GlobalAdapter) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.globals.push(adapter);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.globals
                .push(adapter)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many global adapters"))
        }
    }

    /// Get function adapter by index
    pub fn get_function(&self, index: u32) -> Option<&FunctionAdapter> {
        self.functions.get(index as usize)
    }

    /// Get memory adapter by index
    pub fn get_memory(&self, index: u32) -> Option<&MemoryAdapter> {
        self.memories.get(index as usize)
    }

    /// Get table adapter by index
    pub fn get_table(&self, index: u32) -> Option<&TableAdapter> {
        self.tables.get(index as usize)
    }

    /// Get global adapter by index
    pub fn get_global(&self, index: u32) -> Option<&GlobalAdapter> {
        self.globals.get(index as usize)
    }

    /// Convert this adapter to a component
    pub fn to_component(&self) -> Result<Component> {
        #[cfg(feature = "std")]
        let mut component =
            Component::new(crate::components::component::WrtComponentType::default());
        #[cfg(not(feature = "std"))]
        let mut component = Component::new()?;

        // Convert function adapters to component functions
        for func_adapter in &self.functions {
            // Add the function to the component
            // This is simplified - in reality would need more complex conversion
            // Create an Export from the function adapter
            #[cfg(feature = "std")]
            let name = format!("func_{}", func_adapter.core_index);
            #[cfg(not(feature = "std"))]
            let name = format!("func_{}", func_adapter.core_index);

            let export = Export {
                name,
                ty: wrt_format::component::ExternType::Function {
                    params: vec![],
                    results: vec![],
                },
                value: ExternValue::Function(FunctionValue {
                    ty: wrt_foundation::types::FuncType::default(),
                    export_name: {
                        let name_str = format!("func_{}", func_adapter.core_index);
                        #[cfg(feature = "std")]
                        let export_name = name_str;
                        #[cfg(not(feature = "std"))]
                        let export_name = BoundedString::from_str_truncate(&name_str)?;
                        export_name
                    },
                }),
                kind: ExportKind::Function {
                    function_index: func_adapter.core_index,
                },
                attributes: HashMap::new(),
                integrity_hash: None,
            };
            component.add_function(export)?;
        }

        // Convert memory adapters to component memories
        for mem_adapter in &self.memories {
            let export = Export {
                name: format!("memory_{}", mem_adapter.core_index),
                ty: wrt_format::component::ExternType::Type(mem_adapter.core_index),
                value: ExternValue::Memory(MemoryValue::new(wrt_foundation::types::MemoryType {
                    limits: wrt_foundation::types::Limits {
                        min: mem_adapter.limits.min,
                        max: mem_adapter.limits.max,
                    },
                    shared: mem_adapter.shared,
                    memory64: false,
                })?),
                kind: ExportKind::Value {
                    value_index: mem_adapter.core_index,
                },
                attributes: HashMap::new(),
                integrity_hash: None,
            };
            component.add_memory(export)?;
        }

        // Convert table adapters to component tables
        for table_adapter in &self.tables {
            #[cfg(feature = "std")]
            let table_value = TableValue {
                ty: wrt_foundation::types::TableType {
                    limits: wrt_foundation::types::Limits {
                        min: table_adapter.limits.min,
                        max: table_adapter.limits.max,
                    },
                    element_type: RefType::Funcref,
                },
                table: wrt_runtime::table::Table::new(wrt_foundation::types::TableType {
                    limits: wrt_foundation::types::Limits {
                        min: table_adapter.limits.min,
                        max: table_adapter.limits.max,
                    },
                    element_type: RefType::Funcref,
                })?,
            };

            #[cfg(not(feature = "std"))]
            let table_value = TableValue {
                ty: wrt_foundation::types::TableType {
                    limits: wrt_foundation::types::Limits {
                        min: table_adapter.limits.min,
                        max: table_adapter.limits.max,
                    },
                    element_type: RefType::Funcref,
                },
                table: BoundedVec::new(),
            };

            let export = Export {
                name: format!("table_{}", table_adapter.core_index),
                ty: wrt_format::component::ExternType::Type(table_adapter.core_index),
                value: ExternValue::Table(table_value),
                kind: ExportKind::Value {
                    value_index: table_adapter.core_index,
                },
                attributes: HashMap::new(),
                integrity_hash: None,
            };
            component.add_table(export)?;
        }

        // Convert global adapters to component globals
        for global_adapter in &self.globals {
            #[cfg(feature = "std")]
            let global_value = GlobalValue {
                ty: wrt_foundation::types::GlobalType {
                    mutable: global_adapter.mutable,
                    value_type: wrt_foundation::types::ValueType::I32,
                },
                global: wrt_runtime::global::Global::new(
                    wrt_foundation::types::ValueType::I32,
                    global_adapter.mutable,
                    wrt_foundation::Value::I32(0),
                )?,
            };

            #[cfg(not(feature = "std"))]
            let global_value = GlobalValue {
                ty: wrt_foundation::types::GlobalType {
                    mutable: global_adapter.mutable,
                    value_type: wrt_foundation::types::ValueType::I32,
                },
                value: wrt_foundation::Value::I32(0),
            };

            let export = Export {
                name: format!("global_{}", global_adapter.core_index),
                ty: wrt_format::component::ExternType::Type(global_adapter.core_index),
                value: ExternValue::Global(global_value),
                kind: ExportKind::Value {
                    value_index: global_adapter.core_index,
                },
                attributes: HashMap::new(),
                integrity_hash: None,
            };
            component.add_global(export)?;
        }

        Ok(component)
    }

    /// Convert core type to component type
    fn core_type_to_component_type(
        &self,
        core_type: CoreValType,
    ) -> Result<WrtComponentType<ComponentProvider>> {
        // Create a provider for the component type
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        // All core types map to Unit for this simplified implementation
        match core_type {
            CoreValType::I32 => WrtComponentType::unit(provider),
            CoreValType::I64 => WrtComponentType::unit(provider),
            CoreValType::F32 => WrtComponentType::unit(provider),
            CoreValType::F64 => WrtComponentType::unit(provider),
            CoreValType::V128 => WrtComponentType::unit(provider),
            CoreValType::FuncRef => WrtComponentType::unit(provider),
            CoreValType::ExternRef => WrtComponentType::unit(provider),
        }
    }

    /// Adapt a core function call to component model
    pub fn adapt_function_call(
        &self,
        func_index: u32,
        args: &[Value],
        engine: &mut ComponentExecutionEngine,
    ) -> Result<Value> {
        let adapter = self.get_function(func_index).ok_or_else(|| {
            wrt_error::Error::runtime_function_not_found("Function adapter not found")
        })?;

        match adapter.mode {
            AdaptationMode::Direct => {
                // Direct call - no adaptation needed
                self.call_core_function_direct(adapter.core_index, args, engine)
            },
            AdaptationMode::Lift => {
                // Lower component args to core args, call, then lift result
                let core_args = self.lower_args_to_core(args, &adapter.core_signature)?;
                let core_result =
                    self.call_core_function_direct(adapter.core_index, &core_args, engine)?;
                self.lift_result_to_component(core_result, &adapter.component_signature)
            },
            AdaptationMode::Lower => {
                // Already have core args, call directly
                let core_result =
                    self.call_core_function_direct(adapter.core_index, args, engine)?;
                self.lift_result_to_component(core_result, &adapter.component_signature)
            },
            AdaptationMode::Bidirectional => {
                // Full bidirectional adaptation
                let core_args = self.lower_args_to_core(args, &adapter.core_signature)?;
                let core_result =
                    self.call_core_function_direct(adapter.core_index, &core_args, engine)?;
                self.lift_result_to_component(core_result, &adapter.component_signature)
            },
        }
    }

    /// Call a core function directly
    fn call_core_function_direct(
        &self,
        _core_index: u32,
        args: &[Value],
        _engine: &mut ComponentExecutionEngine,
    ) -> Result<Value> {
        // Simplified implementation - in reality would call actual core module
        if let Some(first_arg) = args.first() {
            Ok(first_arg.clone())
        } else {
            Ok(Value::U32(0))
        }
    }

    /// Lower component arguments to core arguments
    fn lower_args_to_core(
        &self,
        args: &[Value],
        _core_signature: &CoreFunctionSignature,
    ) -> Result<Vec<Value>> {
        // Simplified lowering - in reality would use canonical ABI
        #[cfg(feature = "std")]
        {
            Ok(args.to_vec())
        }
        #[cfg(not(feature = "std"))]
        {
            let mut result = Vec::new();
            for arg in args {
                result.push(arg.clone());
            }
            Ok(result)
        }
    }

    /// Lift core result to component result
    fn lift_result_to_component(
        &self,
        result: Value,
        _component_signature: &WrtComponentType<ComponentProvider>,
    ) -> Result<Value> {
        // Simplified lifting - in reality would use canonical ABI
        Ok(result)
    }
}

impl FunctionAdapter {
    /// Create a new function adapter
    pub fn new(
        core_index: u32,
        component_signature: WrtComponentType<ComponentProvider>,
        core_signature: CoreFunctionSignature,
        mode: AdaptationMode,
    ) -> Self {
        Self {
            core_index,
            component_signature,
            core_signature,
            mode,
        }
    }

    /// Check if this adapter needs canonical ABI processing
    pub fn needs_canonical_abi(&self) -> bool {
        matches!(
            self.mode,
            AdaptationMode::Lift | AdaptationMode::Lower | AdaptationMode::Bidirectional
        )
    }
}

impl CoreFunctionSignature {
    /// Create a new core function signature
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            params: Vec::new(),
            #[cfg(not(feature = "std"))]
            params: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            results: Vec::new(),
            #[cfg(not(feature = "std"))]
            results: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
        })
    }

    /// Add a parameter type
    pub fn add_param(&mut self, param_type: CoreValType) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.params.push(param_type);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.params
                .push(param_type)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many parameters"))
        }
    }

    /// Add a result type
    pub fn add_result(&mut self, result_type: CoreValType) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.results.push(result_type);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            self.results
                .push(result_type)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many results"))
        }
    }
}

impl Default for CoreFunctionSignature {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            #[cfg(feature = "std")]
            params: Vec::new(),
            #[cfg(not(feature = "std"))]
            params: BoundedVec::new(),
            #[cfg(feature = "std")]
            results: Vec::new(),
            #[cfg(not(feature = "std"))]
            results: BoundedVec::new(),
        })
    }
}

impl MemoryAdapter {
    /// Create a new memory adapter
    pub fn new(core_index: u32, min: u32, max: Option<u32>, shared: bool) -> Self {
        Self {
            core_index,
            limits: MemoryLimits { min, max },
            shared,
        }
    }
}

impl TableAdapter {
    /// Create a new table adapter
    pub fn new(core_index: u32, element_type: CoreValType, min: u32, max: Option<u32>) -> Self {
        Self {
            core_index,
            element_type,
            limits: TableLimits { min, max },
        }
    }
}

impl GlobalAdapter {
    /// Create a new global adapter
    pub fn new(core_index: u32, global_type: CoreValType, mutable: bool) -> Self {
        Self {
            core_index,
            global_type,
            mutable,
        }
    }
}

impl fmt::Display for CoreValType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreValType::I32 => write!(f, "i32"),
            CoreValType::I64 => write!(f, "i64"),
            CoreValType::F32 => write!(f, "f32"),
            CoreValType::F64 => write!(f, "f64"),
            CoreValType::V128 => write!(f, "v128"),
            CoreValType::FuncRef => write!(f, "funcref"),
            CoreValType::ExternRef => write!(f, "externref"),
        }
    }
}

impl fmt::Display for AdaptationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdaptationMode::Direct => write!(f, "direct"),
            AdaptationMode::Lift => write!(f, "lift"),
            AdaptationMode::Lower => write!(f, "lower"),
            AdaptationMode::Bidirectional => write!(f, "bidirectional"),
        }
    }
}

// Implement required traits for BoundedVec compatibility
use wrt_foundation::traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream};

// Macro to implement basic traits for simple types
macro_rules! impl_basic_traits {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                // Simple checksum without unsafe code
                0u32.update_checksum(checksum);
            }
        }

        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                &self,
                writer: &mut WriteStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<()> {
                // Simple stub implementation
                Ok(())
            }
        }

        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                _reader: &mut ReadStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<Self> {
                Ok($default_val)
            }
        }
    };
}

// Apply macro to all adapter types
impl_basic_traits!(FunctionAdapter, FunctionAdapter::default());
impl_basic_traits!(MemoryAdapter, MemoryAdapter::default());
impl_basic_traits!(TableAdapter, TableAdapter::default());
impl_basic_traits!(GlobalAdapter, GlobalAdapter::default());
impl_basic_traits!(MemoryLimits, MemoryLimits::default());
impl_basic_traits!(TableLimits, TableLimits::default());
impl_basic_traits!(CoreValType, CoreValType::default());
impl_basic_traits!(AdaptationMode, AdaptationMode::default());
impl_basic_traits!(CoreFunctionSignature, CoreFunctionSignature::default());
