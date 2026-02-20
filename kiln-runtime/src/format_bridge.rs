//! Format bridge interface
//!
//! This module provides clean interfaces for receiving format data from
//! wrt-format and converting it to runtime representations. It establishes
//! the runtime side of the boundary between format and runtime layers.

// alloc is imported in lib.rs with proper feature gates

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use alloc::vec::Vec;

use wrt_foundation::{
    traits::{
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    MemoryProvider,
};

use crate::prelude::*;
// For no_std without alloc, use type aliases with concrete providers
#[cfg(not(any(feature = "std", feature = "alloc")))]
type Vec<T> = wrt_foundation::BoundedVec<T, 16, wrt_foundation::NoStdProvider<1024>>;

/// Trait for types that can be initialized from format representations
pub trait FromFormatBridge<FormatData> {
    /// Initialize from format data with runtime context
    fn from_format_bridge(format_data: FormatData, context: &RuntimeContext) -> Result<Self>
    where
        Self: Sized;
}

/// Runtime context for format bridge operations
#[derive(Debug, Clone)]
pub struct RuntimeContext {
    /// Memory providers available
    pub memory_providers:         Vec<RuntimeMemoryProvider>,
    /// Table providers available  
    pub table_providers:          Vec<RuntimeTableProvider>,
    /// Maximum initialization steps allowed
    pub max_initialization_steps: usize,
    /// ASIL level constraints
    pub asil_constraints:         ASILConstraints,
}

/// Memory provider for runtime operations
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMemoryProvider {
    /// Memory index
    pub index:        u32,
    /// Current size in pages
    pub current_size: u32,
    /// Maximum size in pages
    pub max_size:     Option<u32>,
    /// Whether memory is shared
    pub is_shared:    bool,
}

impl Checksummable for RuntimeMemoryProvider {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.index.update_checksum(checksum);
        self.current_size.update_checksum(checksum);
        self.max_size.update_checksum(checksum);
        self.is_shared.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeMemoryProvider {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        self.current_size.to_bytes_with_provider(writer, provider)?;
        self.max_size.to_bytes_with_provider(writer, provider)?;
        self.is_shared.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for RuntimeMemoryProvider {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let current_size = u32::from_bytes_with_provider(reader, provider)?;
        let max_size = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let is_shared = bool::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            index,
            current_size,
            max_size,
            is_shared,
        })
    }
}

/// Table provider for runtime operations
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTableProvider {
    /// Table index
    pub index:        u32,
    /// Current size in elements
    pub current_size: u32,
    /// Maximum size in elements
    pub max_size:     Option<u32>,
    /// Element type
    pub element_type: RuntimeRefType,
}

impl Checksummable for RuntimeTableProvider {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.index.update_checksum(checksum);
        self.current_size.update_checksum(checksum);
        self.max_size.update_checksum(checksum);
        self.element_type.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeTableProvider {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        self.current_size.to_bytes_with_provider(writer, provider)?;
        self.max_size.to_bytes_with_provider(writer, provider)?;
        self.element_type.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for RuntimeTableProvider {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let current_size = u32::from_bytes_with_provider(reader, provider)?;
        let max_size = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let element_type = RuntimeRefType::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            index,
            current_size,
            max_size,
            element_type,
        })
    }
}

/// Runtime reference type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum RuntimeRefType {
    /// Function reference
    #[default]
    FuncRef,
    /// External reference
    ExternRef,
}


impl Checksummable for RuntimeRefType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            RuntimeRefType::FuncRef => 0u8,
            RuntimeRefType::ExternRef => 1u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeRefType {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &P,
    ) -> Result<()> {
        let discriminant = match self {
            RuntimeRefType::FuncRef => 0u8,
            RuntimeRefType::ExternRef => 1u8,
        };
        writer.write_u8(discriminant)?;
        Ok(())
    }
}

impl FromBytes for RuntimeRefType {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &P,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(RuntimeRefType::FuncRef),
            1 => Ok(RuntimeRefType::ExternRef),
            _ => Err(Error::parse_error("Invalid RuntimeRefType discriminant")),
        }
    }
}

/// ASIL safety constraints
#[derive(Debug, Clone)]
pub struct ASILConstraints {
    /// ASIL level (A, B, C, D)
    pub level: ASILLevel,
    /// Maximum memory allocation allowed
    pub max_memory_allocation: usize,
    /// Maximum initialization time allowed (in microseconds)
    pub max_initialization_time_us: u64,
    /// Whether dynamic allocation is allowed
    pub allow_dynamic_allocation: bool,
}

/// ASIL safety level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ASILLevel {
    /// ASIL-A (lowest safety requirements)
    A,
    /// ASIL-B (moderate safety requirements)
    B,
    /// ASIL-C (high safety requirements)
    C,
    /// ASIL-D (highest safety requirements)
    D,
}

/// Runtime data segment initialized from format bridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDataSegment {
    /// Segment index
    pub index:                u32,
    /// Memory index for active segments
    pub memory_index:         Option<u32>,
    /// Evaluated offset (runtime computed)
    pub evaluated_offset:     Option<u32>,
    /// Data bytes (owned by runtime)
    pub data:                 Vec<u8>,
    /// Initialization state
    pub initialization_state: InitializationState,
    /// Runtime handle for tracking
    pub runtime_handle:       u32,
}

impl Default for RuntimeDataSegment {
    fn default() -> Self {
        Self {
            index:                0,
            memory_index:         None,
            evaluated_offset:     None,
            data:                 {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    Vec::new()
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    wrt_foundation::BoundedVec::new(wrt_foundation::NoStdProvider::<1024>::default()).unwrap_or_default()
                }
            },
            initialization_state: InitializationState::Pending,
            runtime_handle:       0,
        }
    }
}

impl Checksummable for RuntimeDataSegment {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.index.update_checksum(checksum);
        self.memory_index.update_checksum(checksum);
        self.evaluated_offset.update_checksum(checksum);
        // Checksum data length and first few bytes for efficiency
        (self.data.len() as u32).update_checksum(checksum);
        if !self.data.is_empty() {
            let sample_len = core::cmp::min(self.data.len(), 32);
            for (i, byte) in self.data.iter().enumerate() {
                if i >= sample_len {
                    break;
                }
                byte.update_checksum(checksum);
            }
        }
        self.initialization_state.update_checksum(checksum);
        self.runtime_handle.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeDataSegment {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        self.memory_index.to_bytes_with_provider(writer, provider)?;
        self.evaluated_offset.to_bytes_with_provider(writer, provider)?;
        (self.data.len() as u32).to_bytes_with_provider(writer, provider)?;
        for byte in &self.data {
            #[cfg(any(feature = "std", feature = "alloc"))]
            writer.write_u8(*byte)?;
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            writer.write_u8(byte)?;
        }
        self.initialization_state.to_bytes_with_provider(writer, provider)?;
        self.runtime_handle.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for RuntimeDataSegment {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let memory_index = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let evaluated_offset = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let data_len = u32::from_bytes_with_provider(reader, provider)? as usize;

        let data = {
            #[cfg(any(feature = "std", feature = "alloc"))]
            {
                let mut vec = Vec::with_capacity(data_len);
                for _ in 0..data_len {
                    vec.push(reader.read_u8()?);
                }
                vec
            }
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            {
                let data_provider = wrt_foundation::NoStdProvider::<1024>::default();
                let mut vec = wrt_foundation::BoundedVec::new(data_provider)
                    .map_err(|_| Error::parse_error("Failed to create data vector"))?;
                for _ in 0..data_len {
                    vec.push(reader.read_u8()?).map_err(|_| {
                        Error::parse_error("Data segment too large for bounded vector")
                    })?;
                }
                vec
            }
        };

        let initialization_state = InitializationState::from_bytes_with_provider(reader, provider)?;
        let runtime_handle = u32::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            index,
            memory_index,
            evaluated_offset,
            data,
            initialization_state,
            runtime_handle,
        })
    }
}

/// Runtime element segment initialized from format bridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeElementSegment {
    /// Segment index
    pub index:                u32,
    /// Table index for active segments
    pub table_index:          Option<u32>,
    /// Evaluated offset (runtime computed)
    pub evaluated_offset:     Option<u32>,
    /// Element data (function indices or evaluated expressions)
    pub elements:             Vec<RuntimeElement>,
    /// Initialization state
    pub initialization_state: InitializationState,
    /// Runtime handle for tracking
    pub runtime_handle:       u32,
}

impl Default for RuntimeElementSegment {
    fn default() -> Self {
        Self {
            index:                0,
            table_index:          None,
            evaluated_offset:     None,
            elements:             {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    Vec::new()
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    wrt_foundation::BoundedVec::new(wrt_foundation::NoStdProvider::<1024>::default()).unwrap_or_default()
                }
            },
            initialization_state: InitializationState::Pending,
            runtime_handle:       0,
        }
    }
}

impl Checksummable for RuntimeElementSegment {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.index.update_checksum(checksum);
        self.table_index.update_checksum(checksum);
        self.evaluated_offset.update_checksum(checksum);
        (self.elements.len() as u32).update_checksum(checksum);
        for element in &self.elements {
            element.update_checksum(checksum);
        }
        self.initialization_state.update_checksum(checksum);
        self.runtime_handle.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeElementSegment {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        self.table_index.to_bytes_with_provider(writer, provider)?;
        self.evaluated_offset.to_bytes_with_provider(writer, provider)?;
        (self.elements.len() as u32).to_bytes_with_provider(writer, provider)?;
        for element in &self.elements {
            element.to_bytes_with_provider(writer, provider)?;
        }
        self.initialization_state.to_bytes_with_provider(writer, provider)?;
        self.runtime_handle.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for RuntimeElementSegment {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let table_index = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let evaluated_offset = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let elements_len = u32::from_bytes_with_provider(reader, provider)? as usize;

        let elements = {
            #[cfg(any(feature = "std", feature = "alloc"))]
            {
                let mut vec = Vec::with_capacity(elements_len);
                for _ in 0..elements_len {
                    vec.push(RuntimeElement::from_bytes_with_provider(reader, provider)?);
                }
                vec
            }
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            {
                let elements_provider = wrt_foundation::NoStdProvider::<1024>::default();
                let mut vec = wrt_foundation::BoundedVec::new(elements_provider)
                    .map_err(|_| Error::parse_error("Failed to create elements vector"))?;
                for _ in 0..elements_len {
                    vec.push(RuntimeElement::from_bytes_with_provider(reader, provider)?).map_err(
                        |_| Error::parse_error("Element segment too large for bounded vector"),
                    )?;
                }
                vec
            }
        };

        let initialization_state = InitializationState::from_bytes_with_provider(reader, provider)?;
        let runtime_handle = u32::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            index,
            table_index,
            evaluated_offset,
            elements,
            initialization_state,
            runtime_handle,
        })
    }
}

/// Runtime element data
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeElement {
    /// Function index
    FunctionIndex(u32),
    /// Evaluated reference (from expression)
    EvaluatedRef(RuntimeReference),
}

impl Default for RuntimeElement {
    fn default() -> Self {
        RuntimeElement::FunctionIndex(0)
    }
}

impl Checksummable for RuntimeElement {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            RuntimeElement::FunctionIndex(_) => 0u8,
            RuntimeElement::EvaluatedRef(_) => 1u8,
        };
        discriminant.update_checksum(checksum);

        match self {
            RuntimeElement::FunctionIndex(index) => index.update_checksum(checksum),
            RuntimeElement::EvaluatedRef(ref_) => ref_.update_checksum(checksum),
        }
    }
}

impl ToBytes for RuntimeElement {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match self {
            RuntimeElement::FunctionIndex(index) => {
                writer.write_u8(0)?;
                index.to_bytes_with_provider(writer, provider)?;
            },
            RuntimeElement::EvaluatedRef(ref_) => {
                writer.write_u8(1)?;
                ref_.to_bytes_with_provider(writer, provider)?;
            },
        }
        Ok(())
    }
}

impl FromBytes for RuntimeElement {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => {
                let index = u32::from_bytes_with_provider(reader, provider)?;
                Ok(RuntimeElement::FunctionIndex(index))
            },
            1 => {
                let ref_ = RuntimeReference::from_bytes_with_provider(reader, provider)?;
                Ok(RuntimeElement::EvaluatedRef(ref_))
            },
            _ => Err(Error::parse_error("Invalid RuntimeElement discriminant")),
        }
    }
}

/// Runtime reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReference {
    /// Reference type
    pub ref_type: RuntimeRefType,
    /// Runtime handle to the referenced object
    pub handle:   u32,
}

impl Checksummable for RuntimeReference {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.ref_type.update_checksum(checksum);
        self.handle.update_checksum(checksum);
    }
}

impl ToBytes for RuntimeReference {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.ref_type.to_bytes_with_provider(writer, provider)?;
        self.handle.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for RuntimeReference {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let ref_type = RuntimeRefType::from_bytes_with_provider(reader, provider)?;
        let handle = u32::from_bytes_with_provider(reader, provider)?;
        Ok(Self { ref_type, handle })
    }
}

/// Initialization state for runtime segments
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationState {
    /// Not yet initialized
    Pending,
    /// Currently being initialized
    InProgress,
    /// Successfully initialized
    Completed,
    /// Initialization failed
    Failed,
}

impl Checksummable for InitializationState {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            InitializationState::Pending => 0u8,
            InitializationState::InProgress => 1u8,
            InitializationState::Completed => 2u8,
            InitializationState::Failed => 3u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for InitializationState {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &P,
    ) -> Result<()> {
        let discriminant = match self {
            InitializationState::Pending => 0u8,
            InitializationState::InProgress => 1u8,
            InitializationState::Completed => 2u8,
            InitializationState::Failed => 3u8,
        };
        writer.write_u8(discriminant)?;
        Ok(())
    }
}

impl FromBytes for InitializationState {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &P,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(InitializationState::Pending),
            1 => Ok(InitializationState::InProgress),
            2 => Ok(InitializationState::Completed),
            3 => Ok(InitializationState::Failed),
            _ => Err(Error::parse_error(
                "Invalid InitializationState discriminant",
            )),
        }
    }
}

/// Runtime module initialization manager
#[derive(Debug)]
pub struct RuntimeModuleInitializer {
    /// Runtime context
    pub context:          RuntimeContext,
    /// Data segments being managed
    pub data_segments:    Vec<RuntimeDataSegment>,
    /// Element segments being managed
    pub element_segments: Vec<RuntimeElementSegment>,
    /// Start function index
    pub start_function:   Option<u32>,
    /// Current initialization step
    pub current_step:     usize,
    /// Total steps required
    pub total_steps:      usize,
}

impl RuntimeModuleInitializer {
    /// Create new initializer with runtime context
    pub fn new(context: RuntimeContext) -> Self {
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            Self {
                context,
                data_segments: Vec::new(),
                element_segments: Vec::new(),
                start_function: None,
                current_step: 0,
                total_steps: 0,
            }
        }
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            let provider = wrt_foundation::NoStdProvider::<1024>::default();
            Self {
                context,
                data_segments: wrt_foundation::BoundedVec::new(provider.clone())
                    .unwrap_or_default(),
                element_segments: wrt_foundation::BoundedVec::new(provider).unwrap_or_default(),
                start_function: None,
                current_step: 0,
                total_steps: 0,
            }
        }
    }

    /// Initialize from format module runtime data
    pub fn initialize_from_format(&mut self, runtime_data: FormatModuleRuntimeData) -> Result<()> {
        // Validate ASIL constraints
        self.validate_asil_constraints(&runtime_data)?;

        // Set total steps
        self.total_steps = runtime_data.estimated_initialization_steps;

        // Initialize data segments
        for (index, data_extraction) in runtime_data.data_extractions.into_iter().enumerate() {
            let runtime_segment =
                self.create_runtime_data_segment(index as u32, data_extraction)?;
            self.data_segments.push(runtime_segment);
        }

        // Initialize element segments
        for (index, element_extraction) in runtime_data.element_extractions.into_iter().enumerate()
        {
            let runtime_segment =
                self.create_runtime_element_segment(index as u32, element_extraction)?;
            self.element_segments.push(runtime_segment);
        }

        // Set start function
        self.start_function = runtime_data.start_function;

        Ok(())
    }

    /// Execute initialization process
    pub fn execute_initialization(&mut self) -> Result<()> {
        // Initialize active data segments - use indices to avoid borrowing conflicts
        #[cfg(any(feature = "std", feature = "alloc"))]
        let mut active_indices = Vec::new();
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        let mut active_indices = {
            let provider = wrt_foundation::NoStdProvider::<1024>::default();
            Vec::new(provider).map_err(|_| {
                Error::runtime_execution_error("Failed to create active indices vector")
            })?
        };

        for (idx, segment) in self.data_segments.iter().enumerate() {
            if segment.memory_index.is_some() {
                #[cfg(any(feature = "std", feature = "alloc"))]
                active_indices.push(idx);
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                active_indices.push(idx).map_err(|_| {
                    Error::runtime_execution_error("Failed to add index to active indices")
                })?;
            }
        }

        for idx in active_indices {
            if let Some(segment) = self.data_segments.get_mut(idx) {
                Self::initialize_data_segment_static(segment)?;
                self.current_step += 1;
            }
        }

        // Initialize active element segments - use indices to avoid borrowing conflicts
        #[cfg(any(feature = "std", feature = "alloc"))]
        let mut active_element_indices = Vec::new();
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        let mut active_element_indices = {
            let provider = wrt_foundation::NoStdProvider::<1024>::default();
            Vec::new(provider).map_err(|_| {
                Error::runtime_execution_error("Failed to create active element indices vector")
            })?
        };

        for (idx, segment) in self.element_segments.iter().enumerate() {
            if segment.table_index.is_some() {
                #[cfg(any(feature = "std", feature = "alloc"))]
                active_element_indices.push(idx);
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                active_element_indices.push(idx).map_err(|_| {
                    Error::runtime_execution_error("Failed to add index to active element indices")
                })?;
            }
        }

        for idx in active_element_indices {
            if let Some(segment) = self.element_segments.get_mut(idx) {
                Self::initialize_element_segment_static(segment)?;
                self.current_step += 1;
            }
        }

        // Call start function if present
        if let Some(start_fn) = self.start_function {
            self.call_start_function(start_fn)?;
            self.current_step += 1;
        }

        Ok(())
    }

    /// Validate ASIL constraints
    fn validate_asil_constraints(&self, runtime_data: &FormatModuleRuntimeData) -> Result<()> {
        // Check initialization step limits
        if runtime_data.estimated_initialization_steps > self.context.max_initialization_steps {
            return Err(Error::resource_exhausted(
                "Too many initialization steps for ASIL constraints",
            ));
        }

        // Check memory allocation limits for ASIL-D
        if matches!(self.context.asil_constraints.level, ASILLevel::D) {
            let total_data_size: usize =
                runtime_data.data_extractions.iter().map(|e| e.data_size).sum();

            if total_data_size > self.context.asil_constraints.max_memory_allocation {
                return Err(Error::resource_exhausted(
                    "Data size exceeds ASIL-D memory limits",
                ));
            }
        }

        Ok(())
    }

    /// Create runtime data segment from format extraction
    fn create_runtime_data_segment(
        &self,
        index: u32,
        extraction: FormatDataExtraction,
    ) -> Result<RuntimeDataSegment> {
        Ok(RuntimeDataSegment {
            index,
            memory_index: extraction.memory_index,
            evaluated_offset: None, // Will be computed during initialization
            data: {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    Vec::new()
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    wrt_foundation::BoundedVec::new(wrt_foundation::NoStdProvider::<1024>::default()).unwrap_or_default()
                }
            },
            initialization_state: InitializationState::Pending,
            runtime_handle: self.generate_runtime_handle(),
        })
    }

    /// Create runtime element segment from format extraction
    fn create_runtime_element_segment(
        &self,
        index: u32,
        extraction: FormatElementExtraction,
    ) -> Result<RuntimeElementSegment> {
        Ok(RuntimeElementSegment {
            index,
            table_index: extraction.table_index,
            evaluated_offset: None, // Will be computed during initialization
            elements: {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    Vec::new()
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    wrt_foundation::BoundedVec::new(wrt_foundation::NoStdProvider::<1024>::default()).unwrap_or_default()
                }
            },
            initialization_state: InitializationState::Pending,
            runtime_handle: self.generate_runtime_handle(),
        })
    }

    /// Initialize a data segment
    fn initialize_data_segment(&mut self, segment: &mut RuntimeDataSegment) -> Result<()> {
        Self::initialize_data_segment_static(segment)
    }

    /// Initialize a data segment (static version to avoid borrowing conflicts)
    fn initialize_data_segment_static(segment: &mut RuntimeDataSegment) -> Result<()> {
        segment.initialization_state = InitializationState::InProgress;

        // Runtime-specific initialization logic would go here
        // For now, just mark as completed
        segment.initialization_state = InitializationState::Completed;

        Ok(())
    }

    /// Initialize an element segment
    fn initialize_element_segment(&mut self, segment: &mut RuntimeElementSegment) -> Result<()> {
        Self::initialize_element_segment_static(segment)
    }

    /// Initialize an element segment (static version to avoid borrowing
    /// conflicts)
    fn initialize_element_segment_static(segment: &mut RuntimeElementSegment) -> Result<()> {
        segment.initialization_state = InitializationState::InProgress;

        // Runtime-specific initialization logic would go here
        // For now, just mark as completed
        segment.initialization_state = InitializationState::Completed;

        Ok(())
    }

    /// Call start function
    fn call_start_function(&mut self, _start_fn: u32) -> Result<()> {
        // Runtime-specific start function execution would go here
        Ok(())
    }

    /// Generate unique runtime handle
    fn generate_runtime_handle(&self) -> u32 {
        // Simple handle generation (in real implementation would use proper allocation)
        (self.data_segments.len() + self.element_segments.len()) as u32
    }
}

/// Format module runtime data (received from wrt-format bridge)
#[derive(Debug, Clone)]
pub struct FormatModuleRuntimeData {
    /// Start function index
    pub start_function:                 Option<u32>,
    /// Data extraction results
    pub data_extractions:               Vec<FormatDataExtraction>,
    /// Element extraction results
    pub element_extractions:            Vec<FormatElementExtraction>,
    /// Estimated initialization steps
    pub estimated_initialization_steps: usize,
}

/// Format data extraction (received from wrt-format bridge)
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct FormatDataExtraction {
    /// Memory index for active segments
    pub memory_index:            Option<u32>,
    /// Raw offset expression bytes
    pub offset_expr_bytes:       Vec<u8>,
    /// Data size
    pub data_size:               usize,
    /// Whether initialization is required
    pub requires_initialization: bool,
}


impl Checksummable for FormatDataExtraction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.memory_index.update_checksum(checksum);
        (self.offset_expr_bytes.len() as u32).update_checksum(checksum);
        for byte in &self.offset_expr_bytes {
            byte.update_checksum(checksum);
        }
        (self.data_size as u32).update_checksum(checksum);
        self.requires_initialization.update_checksum(checksum);
    }
}

impl ToBytes for FormatDataExtraction {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.memory_index.to_bytes_with_provider(writer, provider)?;
        (self.offset_expr_bytes.len() as u32).to_bytes_with_provider(writer, provider)?;
        for byte in &self.offset_expr_bytes {
            #[cfg(any(feature = "std", feature = "alloc"))]
            writer.write_u8(*byte)?;
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            writer.write_u8(byte)?;
        }
        (self.data_size as u32).to_bytes_with_provider(writer, provider)?;
        self.requires_initialization.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for FormatDataExtraction {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let memory_index = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let bytes_len = u32::from_bytes_with_provider(reader, provider)? as usize;

        let offset_expr_bytes = {
            #[cfg(any(feature = "std", feature = "alloc"))]
            {
                let mut vec = Vec::with_capacity(bytes_len);
                for _ in 0..bytes_len {
                    vec.push(reader.read_u8()?);
                }
                vec
            }
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            {
                let bytes_provider = wrt_foundation::NoStdProvider::<1024>::default();
                let mut vec = wrt_foundation::BoundedVec::new(bytes_provider)
                    .map_err(|_| Error::parse_error("Failed to create offset bytes vector"))?;
                for _ in 0..bytes_len {
                    vec.push(reader.read_u8()?).map_err(|_| {
                        Error::parse_error("Offset expression too large for bounded vector")
                    })?;
                }
                vec
            }
        };

        let data_size = u32::from_bytes_with_provider(reader, provider)? as usize;
        let requires_initialization = bool::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            memory_index,
            offset_expr_bytes,
            data_size,
            requires_initialization,
        })
    }
}

/// Format element extraction (received from wrt-format bridge)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatElementExtraction {
    /// Table index for active segments
    pub table_index:             Option<u32>,
    /// Raw offset expression bytes
    pub offset_expr_bytes:       Vec<u8>,
    /// Element initialization type
    pub init_data_type:          FormatElementInitType,
    /// Whether initialization is required
    pub requires_initialization: bool,
}

impl Default for FormatElementExtraction {
    fn default() -> Self {
        Self {
            table_index:             None,
            offset_expr_bytes:       {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    Vec::new()
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    wrt_foundation::BoundedVec::new(wrt_foundation::NoStdProvider::<1024>::default()).unwrap_or_default()
                }
            },
            init_data_type:          FormatElementInitType::FunctionIndices,
            requires_initialization: false,
        }
    }
}

impl Checksummable for FormatElementExtraction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.table_index.update_checksum(checksum);
        (self.offset_expr_bytes.len() as u32).update_checksum(checksum);
        for byte in &self.offset_expr_bytes {
            byte.update_checksum(checksum);
        }
        self.init_data_type.update_checksum(checksum);
        self.requires_initialization.update_checksum(checksum);
    }
}

impl ToBytes for FormatElementExtraction {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.table_index.to_bytes_with_provider(writer, provider)?;
        (self.offset_expr_bytes.len() as u32).to_bytes_with_provider(writer, provider)?;
        for byte in &self.offset_expr_bytes {
            #[cfg(any(feature = "std", feature = "alloc"))]
            writer.write_u8(*byte)?;
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            writer.write_u8(byte)?;
        }
        self.init_data_type.to_bytes_with_provider(writer, provider)?;
        self.requires_initialization.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for FormatElementExtraction {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let table_index = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let bytes_len = u32::from_bytes_with_provider(reader, provider)? as usize;

        let offset_expr_bytes = {
            #[cfg(any(feature = "std", feature = "alloc"))]
            {
                let mut vec = Vec::with_capacity(bytes_len);
                for _ in 0..bytes_len {
                    vec.push(reader.read_u8()?);
                }
                vec
            }
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            {
                let bytes_provider = wrt_foundation::NoStdProvider::<1024>::default();
                let mut vec = wrt_foundation::BoundedVec::new(bytes_provider)
                    .map_err(|_| Error::parse_error("Failed to create offset bytes vector"))?;
                for _ in 0..bytes_len {
                    vec.push(reader.read_u8()?).map_err(|_| {
                        Error::parse_error("Offset expression too large for bounded vector")
                    })?;
                }
                vec
            }
        };

        let init_data_type = FormatElementInitType::from_bytes_with_provider(reader, provider)?;
        let requires_initialization = bool::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            table_index,
            offset_expr_bytes,
            init_data_type,
            requires_initialization,
        })
    }
}

/// Format element initialization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatElementInitType {
    /// Function indices
    FunctionIndices,
    /// Expression bytes
    ExpressionBytes,
}

impl Checksummable for FormatElementInitType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            FormatElementInitType::FunctionIndices => 0u8,
            FormatElementInitType::ExpressionBytes => 1u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for FormatElementInitType {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &P,
    ) -> Result<()> {
        let discriminant = match self {
            FormatElementInitType::FunctionIndices => 0u8,
            FormatElementInitType::ExpressionBytes => 1u8,
        };
        writer.write_u8(discriminant)?;
        Ok(())
    }
}

impl FromBytes for FormatElementInitType {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &P,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(FormatElementInitType::FunctionIndices),
            1 => Ok(FormatElementInitType::ExpressionBytes),
            _ => Err(Error::parse_error(
                "Invalid FormatElementInitType discriminant",
            )),
        }
    }
}

impl Default for RuntimeContext {
    fn default() -> Self {
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            Self {
                memory_providers:         Vec::new(),
                table_providers:          Vec::new(),
                max_initialization_steps: 1000,
                asil_constraints:         ASILConstraints::default(),
            }
        }
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            let provider = wrt_foundation::NoStdProvider::<1024>::default();
            Self {
                memory_providers:         wrt_foundation::BoundedVec::new(provider.clone())
                    .unwrap_or_default(),
                table_providers:          wrt_foundation::BoundedVec::new(provider)
                    .unwrap_or_default(),
                max_initialization_steps: 1000,
                asil_constraints:         ASILConstraints::default(),
            }
        }
    }
}

impl Default for ASILConstraints {
    fn default() -> Self {
        Self {
            level: ASILLevel::A,
            max_memory_allocation: 1024 * 1024, // 1MB default
            max_initialization_time_us: 10_000, // 10ms default
            allow_dynamic_allocation: true,
        }
    }
}
