//! Component and core module instantiation types
//!
//! This module contains runtime-specific types for instantiating components
//! and core modules. These types handle the actual instantiation process
//! and runtime state management.

use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    traits::{
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
    MemoryProvider,
};

use crate::prelude::*;

/// Maximum instantiation arguments per component
const MAX_INSTANTIATION_ARGS: usize = 256;

/// Memory provider for instantiation data
type InstantiationProvider = wrt_foundation::safe_memory::NoStdProvider<4096>;

/// Runtime component instantiation data
#[derive(Debug, Clone)]
pub struct ComponentInstantiation {
    /// Component index to instantiate
    pub component_idx: u32,
    /// Runtime instantiation arguments
    pub args: BoundedVec<RuntimeInstantiateArg, MAX_INSTANTIATION_ARGS, InstantiationProvider>,
    /// Runtime state for the instantiation
    pub runtime_state: InstantiationState,
}

/// Runtime core module instantiation data
#[derive(Debug, Clone)]
pub struct CoreModuleInstantiation {
    /// Module index to instantiate
    pub module_idx:    u32,
    /// Runtime instantiation arguments
    pub args: BoundedVec<RuntimeCoreInstantiateArg, MAX_INSTANTIATION_ARGS, InstantiationProvider>,
    /// Runtime state for the instantiation
    pub runtime_state: InstantiationState,
}

/// Runtime instantiation argument for components
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeInstantiateArg {
    /// Name of the argument
    pub name:         BoundedString<256>,
    /// Runtime reference to the provided value
    pub runtime_ref:  RuntimeReference,
    /// Validation state
    pub is_validated: bool,
}

/// Runtime instantiation argument for core modules
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCoreInstantiateArg {
    /// Name of the argument
    pub name:                 BoundedString<256>,
    /// Runtime instance index that provides the value
    pub runtime_instance_idx: u32,
    /// Validation state
    pub is_validated:         bool,
}

/// Runtime reference to an instantiation argument
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeReference {
    /// Sort of the reference
    pub sort:           RuntimeSort,
    /// Runtime index within the sort
    pub runtime_idx:    u32,
    /// Handle to the runtime object
    pub runtime_handle: u32,
}

/// Runtime sort for instantiation references
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeSort {
    /// Core function reference
    #[default]
    CoreFunction,
    /// Core table reference
    CoreTable,
    /// Core memory reference
    CoreMemory,
    /// Core global reference
    CoreGlobal,
    /// Core type reference
    CoreType,
    /// Core module reference
    CoreModule,
    /// Core instance reference
    CoreInstance,
    /// Component function reference
    Function,
    /// Component value reference
    Value,
    /// Component type reference
    Type,
    /// Component reference
    Component,
    /// Component instance reference
    Instance,
}

/// Runtime instantiation state
#[derive(Debug, Clone)]
pub struct InstantiationState {
    /// Whether instantiation has started
    pub is_started:          bool,
    /// Whether instantiation is complete
    pub is_complete:         bool,
    /// Runtime error if instantiation failed
    pub error_state:         Option<InstantiationError>,
    /// Runtime resources allocated during instantiation
    pub allocated_resources: BoundedVec<u32, 64, InstantiationProvider>,
}

/// Runtime instantiation error
#[derive(Debug, Clone)]
pub struct InstantiationError {
    /// Error message
    pub message:        &'static str,
    /// Error code
    pub code:           u16,
    /// Failed argument index (if applicable)
    pub failed_arg_idx: Option<u32>,
}

impl ComponentInstantiation {
    /// Create new component instantiation
    pub fn new(component_idx: u32) -> Self {
        Self {
            component_idx,
            args: BoundedVec::new(InstantiationProvider::default()).unwrap_or_default(),
            runtime_state: InstantiationState::default(),
        }
    }

    /// Add instantiation argument
    pub fn add_arg(
        &mut self,
        name: BoundedString<256>,
        runtime_ref: RuntimeReference,
    ) -> Result<()> {
        let arg = RuntimeInstantiateArg {
            name,
            runtime_ref,
            is_validated: false,
        };

        self.args
            .push(arg)
            .map_err(|_| Error::runtime_execution_error("Argument capacity exceeded"))
    }

    /// Start instantiation process
    pub fn start_instantiation(&mut self) -> Result<()> {
        if self.runtime_state.is_started {
            return Err(Error::invalid_state_error("Instantiation already started"));
        }

        self.runtime_state.is_started = true;
        Ok(())
    }

    /// Complete instantiation process
    pub fn complete_instantiation(&mut self) -> Result<()> {
        if !self.runtime_state.is_started {
            return Err(Error::runtime_execution_error("Instantiation not started"));
        }

        self.runtime_state.is_complete = true;
        Ok(())
    }
}

impl CoreModuleInstantiation {
    /// Create new core module instantiation
    pub fn new(module_idx: u32) -> Self {
        Self {
            module_idx,
            args: BoundedVec::new(InstantiationProvider::default()).unwrap_or_default(),
            runtime_state: InstantiationState::default(),
        }
    }

    /// Add core instantiation argument
    pub fn add_core_arg(
        &mut self,
        name: BoundedString<256>,
        runtime_instance_idx: u32,
    ) -> Result<()> {
        let arg = RuntimeCoreInstantiateArg {
            name,
            runtime_instance_idx,
            is_validated: false,
        };

        self.args.push(arg).map_err(|_| {
            Error::new(
                ErrorCategory::Memory,
                wrt_error::codes::CAPACITY_EXCEEDED,
                "Argument capacity exceeded",
            )
        })
    }

    /// Start core instantiation process
    pub fn start_core_instantiation(&mut self) -> Result<()> {
        if self.runtime_state.is_started {
            return Err(Error::runtime_execution_error(
                "Core instantiation already started",
            ));
        }

        self.runtime_state.is_started = true;
        Ok(())
    }

    /// Complete core instantiation process
    pub fn complete_core_instantiation(&mut self) -> Result<()> {
        if !self.runtime_state.is_started {
            return Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Core instantiation not started",
            ));
        }

        self.runtime_state.is_complete = true;
        Ok(())
    }
}

impl Default for ComponentInstantiation {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Default for CoreModuleInstantiation {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Default for InstantiationState {
    fn default() -> Self {
        Self {
            is_started:          false,
            is_complete:         false,
            error_state:         None,
            allocated_resources: BoundedVec::new(InstantiationProvider::default())
                .unwrap_or_default(),
        }
    }
}

// Trait implementations for RuntimeInstantiateArg
impl Checksummable for RuntimeInstantiateArg {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(self.name.as_str().unwrap_or("").as_bytes());
        checksum.update_slice(&self.runtime_ref.runtime_idx.to_le_bytes());
        checksum.update_slice(&[if self.is_validated { 1 } else { 0 }]);
    }
}

impl ToBytes for RuntimeInstantiateArg {
    fn serialized_size(&self) -> usize {
        self.name.len() + 4 + 1 // name + runtime_idx + bool
    }

    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(self.name.as_str()?.as_bytes())?;
        writer.write_all(&self.runtime_ref.runtime_idx.to_le_bytes())?;
        writer.write_all(&[if self.is_validated { 1 } else { 0 }])?;
        Ok(())
    }
}

impl FromBytes for RuntimeInstantiateArg {
    fn from_bytes_with_provider<P: MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // Simple implementation - in real usage this would parse the bytes
        // For now, return default instance
        Ok(Self::default())
    }
}

// Trait implementations for RuntimeCoreInstantiateArg
impl Checksummable for RuntimeCoreInstantiateArg {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(self.name.as_str().unwrap_or("").as_bytes());
        checksum.update_slice(&self.runtime_instance_idx.to_le_bytes());
        checksum.update_slice(&[if self.is_validated { 1 } else { 0 }]);
    }
}

impl ToBytes for RuntimeCoreInstantiateArg {
    fn serialized_size(&self) -> usize {
        self.name.len() + 4 + 1
    }

    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(self.name.as_str()?.as_bytes())?;
        writer.write_all(&self.runtime_instance_idx.to_le_bytes())?;
        writer.write_all(&[if self.is_validated { 1 } else { 0 }])?;
        Ok(())
    }
}

impl FromBytes for RuntimeCoreInstantiateArg {
    fn from_bytes_with_provider<P: MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

// Trait implementations for RuntimeReference
impl Checksummable for RuntimeReference {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&(self.sort as u32).to_le_bytes());
        checksum.update_slice(&self.runtime_idx.to_le_bytes());
        checksum.update_slice(&self.runtime_handle.to_le_bytes());
    }
}

impl ToBytes for RuntimeReference {
    fn serialized_size(&self) -> usize {
        12 // 4 + 4 + 4
    }

    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(&(self.sort as u32).to_le_bytes())?;
        writer.write_all(&self.runtime_idx.to_le_bytes())?;
        writer.write_all(&self.runtime_handle.to_le_bytes())?;
        Ok(())
    }
}

impl FromBytes for RuntimeReference {
    fn from_bytes_with_provider<P: MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}
