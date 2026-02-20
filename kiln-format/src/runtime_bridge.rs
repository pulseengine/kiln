//! Runtime bridge interface
//!
//! This module provides clean interfaces for converting between pure format
//! representations and runtime types. It establishes the boundary between
//! the format layer (wrt-format) and the runtime layer (wrt-runtime).

#![allow(clippy::needless_borrow)] // Many false positives in iterator patterns

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    traits::BoundedCapacity,
};

use crate::{
    prelude::*,
    pure_format_types::*,
};

// Simplified type aliases - use Vec when available
#[cfg(feature = "std")]
type OffsetExprBytes = Vec<u8>;
#[cfg(feature = "std")]
type DataExtractionVec = Vec<RuntimeDataExtraction>;
#[cfg(feature = "std")]
type ElementExtractionVec = Vec<RuntimeElementExtraction>;
#[cfg(feature = "std")]
type DataInitVec = Vec<(usize, DataInitializationHint)>;
#[cfg(feature = "std")]
type ElementInitVec = Vec<(usize, ElementInitializationHint)>;

// For no_std, use bounded vectors
#[cfg(not(feature = "std"))]
type OffsetExprBytes =
    wrt_foundation::bounded::BoundedVec<u8, 1024, wrt_foundation::NoStdProvider<8192>>;
#[cfg(not(feature = "std"))]
type DataExtractionVec = wrt_foundation::bounded::BoundedVec<
    RuntimeDataExtraction,
    512,
    wrt_foundation::NoStdProvider<8192>,
>;
#[cfg(not(feature = "std"))]
type ElementExtractionVec = wrt_foundation::bounded::BoundedVec<
    RuntimeElementExtraction,
    512,
    wrt_foundation::NoStdProvider<8192>,
>;
#[cfg(not(feature = "std"))]
type DataInitVec = wrt_foundation::bounded::BoundedVec<
    (usize, DataInitializationHint),
    512,
    wrt_foundation::NoStdProvider<8192>,
>;
#[cfg(not(feature = "std"))]
type ElementInitVec = wrt_foundation::bounded::BoundedVec<
    (usize, ElementInitializationHint),
    512,
    wrt_foundation::NoStdProvider<8192>,
>;

// Helper functions to reduce deprecation warnings by centralizing pattern
// matching
/// Check if a data segment is active (helper to reduce deprecation warnings)
fn is_data_active(data: &crate::pure_format_types::PureDataSegment) -> bool {
    data.is_active()
}

/// Check if an element segment is active (helper to reduce deprecation
/// warnings)
fn is_element_active(element: &crate::pure_format_types::PureElementSegment) -> bool {
    element.is_active()
}

/// Extract table index from element segment (helper to reduce deprecation
/// warnings)
fn get_element_table_index(element: &crate::pure_format_types::PureElementSegment) -> Option<u32> {
    element.table_index()
}

/// Get element segment type (helper to reduce deprecation warnings)
fn get_element_segment_type(
    element: &crate::pure_format_types::PureElementSegment,
) -> ElementSegmentType {
    match element.mode {
        crate::pure_format_types::PureElementMode::Active { .. } => ElementSegmentType::Active,
        crate::pure_format_types::PureElementMode::Passive => ElementSegmentType::Passive,
        crate::pure_format_types::PureElementMode::Declared => ElementSegmentType::Declared,
    }
}

/// Trait for types that can be converted to runtime representations
pub trait ToRuntime<RuntimeType> {
    /// Convert to runtime type, possibly with additional context
    fn to_runtime(&self) -> Result<RuntimeType>;
}

/// Trait for types that can be converted from format representations
pub trait FromFormat<FormatType> {
    /// Convert from format type to runtime type
    fn from_format(format: &FormatType) -> Result<Self>
    where
        Self: Sized;
}

/// Bridge interface for data segments
pub struct DataSegmentBridge;

impl DataSegmentBridge {
    /// Extract runtime initialization data from pure format data segment
    pub fn extract_runtime_data(segment: &PureDataSegment) -> RuntimeDataExtraction {
        // Convert Vec to appropriate type for no_std
        #[cfg(feature = "std")]
        let offset_expr_bytes = segment.offset_expr_bytes.clone();
        #[cfg(not(feature = "std"))]
        let offset_expr_bytes = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            let mut bounded_vec = OffsetExprBytes::new(provider).unwrap();
            for byte in &segment.offset_expr_bytes {
                bounded_vec.push(*byte).unwrap();
            }
            bounded_vec
        };

        RuntimeDataExtraction {
            is_active: segment.is_active(),
            memory_index: segment.memory_index(),
            offset_expr_bytes,
            data_size: segment.data_bytes.len(),
            requires_initialization: segment.is_active(),
        }
    }

    /// Create runtime initialization hint for data segment
    pub fn create_initialization_hint(segment: &PureDataSegment) -> DataInitializationHint {
        DataInitializationHint {
            segment_type:             if segment.is_active() {
                DataSegmentType::Active
            } else {
                DataSegmentType::Passive
            },
            memory_target:            segment.memory_index(),
            offset_evaluation_needed: segment.is_active() && !segment.offset_expr_bytes.is_empty(),
            data_bytes_ref:           DataBytesReference {
                start_offset:  0,
                length:        segment.data_bytes.len(),
                requires_copy: true,
            },
        }
    }
}

/// Bridge interface for element segments
pub struct ElementSegmentBridge;

impl ElementSegmentBridge {
    /// Extract runtime initialization data from pure format element segment
    pub fn extract_runtime_data(segment: &PureElementSegment) -> RuntimeElementExtraction {
        // Convert Vec to appropriate type for no_std
        #[cfg(feature = "std")]
        let offset_expr_bytes = segment.offset_expr_bytes.clone();
        #[cfg(not(feature = "std"))]
        let offset_expr_bytes = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            let mut bounded_vec = OffsetExprBytes::new(provider).unwrap();
            for byte in &segment.offset_expr_bytes {
                bounded_vec.push(*byte).unwrap();
            }
            bounded_vec
        };

        RuntimeElementExtraction {
            is_active: segment.is_active(),
            table_index: segment.table_index(),
            element_type: segment.element_type.clone(),
            offset_expr_bytes,
            init_data_type: match &segment.init_data {
                PureElementInit::FunctionIndices(_) => ElementInitType::FunctionIndices,
                PureElementInit::ExpressionBytes(_) => ElementInitType::ExpressionBytes,
            },
            requires_initialization: segment.is_active(),
        }
    }

    /// Create runtime initialization hint for element segment
    pub fn create_initialization_hint(segment: &PureElementSegment) -> ElementInitializationHint {
        ElementInitializationHint {
            segment_type:             match segment.mode {
                PureElementMode::Active { .. } => ElementSegmentType::Active,
                PureElementMode::Passive => ElementSegmentType::Passive,
                PureElementMode::Declared => ElementSegmentType::Declared,
            },
            table_target:             segment.table_index(),
            offset_evaluation_needed: segment.is_active() && !segment.offset_expr_bytes.is_empty(),
            element_count:            match &segment.init_data {
                PureElementInit::FunctionIndices(indices) => indices.len(),
                PureElementInit::ExpressionBytes(exprs) => exprs.len(),
            },
        }
    }
}

/// Runtime data extraction result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeDataExtraction {
    /// Whether this is an active segment requiring initialization
    pub is_active:               bool,
    /// Memory index for active segments
    pub memory_index:            Option<u32>,
    /// Raw offset expression bytes (for runtime evaluation)
    pub offset_expr_bytes:       OffsetExprBytes,
    /// Size of data in bytes
    pub data_size:               usize,
    /// Whether runtime initialization is required
    pub requires_initialization: bool,
}

/// Runtime element extraction result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeElementExtraction {
    /// Whether this is an active segment requiring initialization
    pub is_active:               bool,
    /// Table index for active segments
    pub table_index:             Option<u32>,
    /// Element type
    pub element_type:            crate::types::RefType,
    /// Raw offset expression bytes (for runtime evaluation)
    pub offset_expr_bytes:       OffsetExprBytes,
    /// Type of initialization data
    pub init_data_type:          ElementInitType,
    /// Whether runtime initialization is required
    pub requires_initialization: bool,
}

/// Data segment type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSegmentType {
    /// Active segment (requires runtime initialization)
    #[default]
    Active,
    /// Passive segment (available for memory.init)
    Passive,
}

/// Element segment type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementSegmentType {
    /// Active segment (requires runtime initialization)
    #[default]
    Active,
    /// Passive segment (available for table.init)
    Passive,
    /// Declared segment (available for linking)
    Declared,
}

/// Element initialization data type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementInitType {
    /// Function indices
    #[default]
    FunctionIndices,
    /// Expression bytes (for runtime evaluation)
    ExpressionBytes,
}

/// Data initialization hint for runtime
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataInitializationHint {
    /// Type of data segment
    pub segment_type:             DataSegmentType,
    /// Target memory index
    pub memory_target:            Option<u32>,
    /// Whether offset expression evaluation is needed
    pub offset_evaluation_needed: bool,
    /// Reference to data bytes
    pub data_bytes_ref:           DataBytesReference,
}

/// Element initialization hint for runtime
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementInitializationHint {
    /// Type of element segment
    pub segment_type:             ElementSegmentType,
    /// Target table index
    pub table_target:             Option<u32>,
    /// Whether offset expression evaluation is needed
    pub offset_evaluation_needed: bool,
    /// Number of elements to initialize
    pub element_count:            usize,
}

impl wrt_foundation::traits::Checksummable for ElementSegmentType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let value = match self {
            ElementSegmentType::Active => 0u8,
            ElementSegmentType::Passive => 1u8,
            ElementSegmentType::Declared => 2u8,
        };
        checksum.update_slice(&[value]);
    }
}

impl wrt_foundation::traits::ToBytes for ElementSegmentType {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        let value = match self {
            ElementSegmentType::Active => 0u8,
            ElementSegmentType::Passive => 1u8,
            ElementSegmentType::Declared => 2u8,
        };
        writer.write_all(&[value])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ElementSegmentType {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        match bytes[0] {
            0 => Ok(ElementSegmentType::Active),
            1 => Ok(ElementSegmentType::Passive),
            2 => Ok(ElementSegmentType::Declared),
            _ => Err(Error::runtime_execution_error(
                "Invalid element segment type discriminant",
            )),
        }
    }
}

impl wrt_foundation::traits::Checksummable for ElementInitializationHint {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.segment_type.update_checksum(checksum);
        if let Some(target) = self.table_target {
            checksum.update_slice(&target.to_le_bytes());
        }
        checksum.update_slice(&[self.offset_evaluation_needed as u8]);
        checksum.update_slice(&self.element_count.to_le_bytes());
    }
}

impl wrt_foundation::traits::ToBytes for ElementInitializationHint {
    fn serialized_size(&self) -> usize {
        16 // Simplified size
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.segment_type.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&self.table_target.unwrap_or(0).to_le_bytes())?;
        writer.write_all(&[self.offset_evaluation_needed as u8])?;
        writer.write_all(&self.element_count.to_le_bytes())?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ElementInitializationHint {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let segment_type = ElementSegmentType::from_bytes_with_provider(reader, provider)?;

        let mut target_bytes = [0u8; 4];
        reader.read_exact(&mut target_bytes)?;
        let table_target_val = u32::from_le_bytes(target_bytes);
        let table_target = if table_target_val == 0 { None } else { Some(table_target_val) };

        let mut bool_bytes = [0u8; 1];
        reader.read_exact(&mut bool_bytes)?;
        let offset_evaluation_needed = bool_bytes[0] != 0;

        let mut count_bytes = [0u8; 8];
        reader.read_exact(&mut count_bytes)?;
        let element_count = usize::from_le_bytes(count_bytes);

        Ok(Self {
            segment_type,
            table_target,
            offset_evaluation_needed,
            element_count,
        })
    }
}

/// Reference to data bytes (avoids copying large data)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataBytesReference {
    /// Start offset in the data
    pub start_offset:  usize,
    /// Length of data
    pub length:        usize,
    /// Whether runtime needs to copy the data
    pub requires_copy: bool,
}

/// Bridge for converting entire modules
pub struct ModuleBridge;

impl ModuleBridge {
    /// Extract all runtime initialization data from a module
    pub fn extract_module_runtime_data(module: &crate::module::Module) -> ModuleRuntimeData {
        // Convert module data segments to runtime extractions
        #[cfg(feature = "std")]
        let mut data_extractions = DataExtractionVec::new();
        #[cfg(not(feature = "std"))]
        let mut data_extractions = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            DataExtractionVec::new(provider).unwrap()
        };
        for data_segment in module.data.iter() {
            // Convert module::Data to runtime extraction
            let extraction = RuntimeDataExtraction {
                is_active:               is_data_active(&data_segment),
                memory_index:            data_segment.memory_index(),
                offset_expr_bytes:       {
                    #[cfg(feature = "std")]
                    {
                        data_segment.offset_expr_bytes.clone()
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
                        let mut bounded = OffsetExprBytes::new(provider).unwrap();
                        for byte in &data_segment.offset_expr_bytes {
                            bounded.push(*byte).unwrap();
                        }
                        bounded
                    }
                },
                data_size:               data_segment.data_bytes.len(),
                requires_initialization: is_data_active(&data_segment),
            };
            #[cfg(feature = "std")]
            data_extractions.push(extraction);
            #[cfg(not(feature = "std"))]
            data_extractions.push(extraction).unwrap();
        }

        // Convert module element segments to runtime extractions
        #[cfg(feature = "std")]
        let mut element_extractions = ElementExtractionVec::new();
        #[cfg(not(feature = "std"))]
        let mut element_extractions = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            ElementExtractionVec::new(provider).unwrap()
        };
        for element_segment in module.elements.iter() {
            let extraction = RuntimeElementExtraction {
                is_active:               is_element_active(&element_segment),
                table_index:             get_element_table_index(&element_segment),
                element_type:            element_segment.element_type.clone(),
                offset_expr_bytes:       {
                    #[cfg(feature = "std")]
                    {
                        element_segment.offset_expr_bytes.clone()
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
                        let mut bounded = OffsetExprBytes::new(provider).unwrap();
                        for byte in &element_segment.offset_expr_bytes {
                            bounded.push(*byte).unwrap();
                        }
                        bounded
                    }
                },
                init_data_type:          match &element_segment.init_data {
                    crate::pure_format_types::PureElementInit::FunctionIndices(_) => {
                        ElementInitType::FunctionIndices
                    },
                    crate::pure_format_types::PureElementInit::ExpressionBytes(_) => {
                        ElementInitType::ExpressionBytes
                    },
                },
                requires_initialization: is_element_active(&element_segment),
            };
            #[cfg(feature = "std")]
            element_extractions.push(extraction);
            #[cfg(not(feature = "std"))]
            element_extractions.push(extraction).unwrap();
        }

        ModuleRuntimeData {
            start_function: module.start,
            data_extractions,
            element_extractions,
            requires_initialization: module.start.is_some()
                || module.data.iter().any(|d| is_data_active(&d))
                || module.elements.iter().any(|e| is_element_active(&e)),
        }
    }

    /// Create initialization plan for a module
    pub fn create_initialization_plan(module: &crate::module::Module) -> ModuleInitializationPlan {
        // Create data initialization hints directly from module data
        #[cfg(feature = "std")]
        let mut data_hints = Vec::new();
        #[cfg(not(feature = "std"))]
        let mut data_hints = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            DataInitVec::new(provider).unwrap()
        };
        for (index, data_segment) in module.data.iter().enumerate() {
            let hint = DataInitializationHint {
                segment_type:             if data_segment.is_active() {
                    DataSegmentType::Active
                } else {
                    DataSegmentType::Passive
                },
                memory_target:            data_segment.memory_index(),
                offset_evaluation_needed: data_segment.is_active(),
                data_bytes_ref:           DataBytesReference {
                    start_offset:  0,
                    length:        data_segment.data_bytes.len(),
                    requires_copy: true,
                },
            };
            #[cfg(feature = "std")]
            data_hints.push((index, hint));
            #[cfg(not(feature = "std"))]
            data_hints.push((index, hint)).unwrap();
        }

        // Create element initialization hints directly from module elements
        #[cfg(feature = "std")]
        let mut element_hints = Vec::new();
        #[cfg(not(feature = "std"))]
        let mut element_hints = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            ElementInitVec::new(provider).unwrap()
        };
        for (index, element_segment) in module.elements.iter().enumerate() {
            let hint = ElementInitializationHint {
                segment_type:             get_element_segment_type(&element_segment),
                table_target:             get_element_table_index(&element_segment),
                offset_evaluation_needed: is_element_active(&element_segment),
                element_count:            match &element_segment.init_data {
                    crate::pure_format_types::PureElementInit::FunctionIndices(indices) => {
                        indices.len()
                    },
                    crate::pure_format_types::PureElementInit::ExpressionBytes(exprs) => {
                        exprs.len()
                    },
                },
            };
            #[cfg(feature = "std")]
            element_hints.push((index, hint));
            #[cfg(not(feature = "std"))]
            element_hints.push((index, hint)).unwrap();
        }

        ModuleInitializationPlan {
            start_function:                 module.start,
            data_initialization_order:      data_hints,
            element_initialization_order:   element_hints,
            estimated_initialization_steps: calculate_initialization_steps(module),
        }
    }
}

/// Complete runtime data for a module
#[derive(Debug, Clone)]
pub struct ModuleRuntimeData {
    /// Start function index
    pub start_function:          Option<u32>,
    /// Runtime data for all data segments
    pub data_extractions:        DataExtractionVec,
    /// Runtime data for all element segments
    pub element_extractions:     ElementExtractionVec,
    /// Whether any initialization is required
    pub requires_initialization: bool,
}

/// Initialization plan for a module
#[derive(Debug, Clone)]
pub struct ModuleInitializationPlan {
    /// Start function to call after initialization
    pub start_function:                 Option<u32>,
    /// Data segments with their initialization hints (index, hint)
    pub data_initialization_order:      DataInitVec,
    /// Element segments with their initialization hints (index, hint)
    pub element_initialization_order:   ElementInitVec,
    /// Estimated number of initialization steps
    pub estimated_initialization_steps: usize,
}

/// Calculate estimated initialization steps for planning
fn calculate_initialization_steps(module: &crate::module::Module) -> usize {
    let mut steps = 0;

    // Count active data segments (each requires offset evaluation + memory.init)
    steps += module.data.iter().filter(|d| is_data_active(d)).count() * 2;

    // Count active element segments (each requires offset evaluation + table.init)
    steps += module.elements.iter().filter(|e| is_element_active(e)).count() * 2;

    // Add start function call if present
    if module.start.is_some() {
        steps += 1;
    }

    steps
}

/// Add missing trait implementations for DataSegmentType
impl wrt_foundation::traits::Checksummable for DataSegmentType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let value = match self {
            DataSegmentType::Active => 0u8,
            DataSegmentType::Passive => 1u8,
        };
        checksum.update_slice(&[value]);
    }
}

impl wrt_foundation::traits::ToBytes for DataSegmentType {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        let value = match self {
            DataSegmentType::Active => 0u8,
            DataSegmentType::Passive => 1u8,
        };
        writer.write_all(&[value])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for DataSegmentType {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        match bytes[0] {
            0 => Ok(DataSegmentType::Active),
            1 => Ok(DataSegmentType::Passive),
            _ => Err(Error::runtime_execution_error(
                "Invalid data segment type discriminant",
            )),
        }
    }
}

/// Add missing trait implementations for DataBytesReference
impl wrt_foundation::traits::Checksummable for DataBytesReference {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.start_offset.to_le_bytes());
        checksum.update_slice(&self.length.to_le_bytes());
        checksum.update_slice(&[self.requires_copy as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for DataBytesReference {
    fn serialized_size(&self) -> usize {
        17 // 8 + 8 + 1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(&self.start_offset.to_le_bytes())?;
        writer.write_all(&self.length.to_le_bytes())?;
        writer.write_all(&[self.requires_copy as u8])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for DataBytesReference {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut offset_bytes = [0u8; 8];
        reader.read_exact(&mut offset_bytes)?;
        let start_offset = usize::from_le_bytes(offset_bytes);

        let mut length_bytes = [0u8; 8];
        reader.read_exact(&mut length_bytes)?;
        let length = usize::from_le_bytes(length_bytes);

        let mut bool_bytes = [0u8; 1];
        reader.read_exact(&mut bool_bytes)?;
        let requires_copy = bool_bytes[0] != 0;

        Ok(Self {
            start_offset,
            length,
            requires_copy,
        })
    }
}

/// Add missing trait implementations for DataInitializationHint
impl wrt_foundation::traits::Checksummable for DataInitializationHint {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.segment_type.update_checksum(checksum);
        if let Some(target) = self.memory_target {
            checksum.update_slice(&target.to_le_bytes());
        }
        checksum.update_slice(&[self.offset_evaluation_needed as u8]);
        self.data_bytes_ref.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for DataInitializationHint {
    fn serialized_size(&self) -> usize {
        22 // Simplified size: 1 + 4 + 1 + 17 = 23, rounded down
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.segment_type.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&self.memory_target.unwrap_or(0).to_le_bytes())?;
        writer.write_all(&[self.offset_evaluation_needed as u8])?;
        self.data_bytes_ref.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for DataInitializationHint {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let segment_type = DataSegmentType::from_bytes_with_provider(reader, provider)?;

        let mut target_bytes = [0u8; 4];
        reader.read_exact(&mut target_bytes)?;
        let memory_target_val = u32::from_le_bytes(target_bytes);
        let memory_target = if memory_target_val == 0 { None } else { Some(memory_target_val) };

        let mut bool_bytes = [0u8; 1];
        reader.read_exact(&mut bool_bytes)?;
        let offset_evaluation_needed = bool_bytes[0] != 0;

        let data_bytes_ref = DataBytesReference::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            segment_type,
            memory_target,
            offset_evaluation_needed,
            data_bytes_ref,
        })
    }
}

/// Add missing trait implementations for ElementInitType
impl wrt_foundation::traits::Checksummable for ElementInitType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let value = match self {
            ElementInitType::FunctionIndices => 0u8,
            ElementInitType::ExpressionBytes => 1u8,
        };
        checksum.update_slice(&[value]);
    }
}

impl wrt_foundation::traits::ToBytes for ElementInitType {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        let value = match self {
            ElementInitType::FunctionIndices => 0u8,
            ElementInitType::ExpressionBytes => 1u8,
        };
        writer.write_all(&[value])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ElementInitType {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        match bytes[0] {
            0 => Ok(ElementInitType::FunctionIndices),
            1 => Ok(ElementInitType::ExpressionBytes),
            _ => Err(Error::new(
                ErrorCategory::Parse,
                0x3003,
                "Invalid element init type discriminant",
            )),
        }
    }
}

/// Add minimal trait implementations for RuntimeDataExtraction
impl wrt_foundation::traits::Checksummable for RuntimeDataExtraction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[self.is_active as u8]);
        if let Some(index) = self.memory_index {
            checksum.update_slice(&index.to_le_bytes());
        }
        checksum.update_slice(&self.data_size.to_le_bytes());
        checksum.update_slice(&[self.requires_initialization as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for RuntimeDataExtraction {
    fn serialized_size(&self) -> usize {
        16 // Simplified
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(&[self.is_active as u8])?;
        writer.write_all(&self.memory_index.unwrap_or(0).to_le_bytes())?;
        writer.write_all(&self.data_size.to_le_bytes())?;
        writer.write_all(&[self.requires_initialization as u8])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for RuntimeDataExtraction {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut bool_bytes = [0u8; 1];
        reader.read_exact(&mut bool_bytes)?;
        let is_active = bool_bytes[0] != 0;

        let mut index_bytes = [0u8; 4];
        reader.read_exact(&mut index_bytes)?;
        let memory_index_val = u32::from_le_bytes(index_bytes);
        let memory_index = if memory_index_val == 0 { None } else { Some(memory_index_val) };

        let mut size_bytes = [0u8; 8];
        reader.read_exact(&mut size_bytes)?;
        let data_size = usize::from_le_bytes(size_bytes);

        reader.read_exact(&mut bool_bytes)?;
        let requires_initialization = bool_bytes[0] != 0;

        // Create empty Vec for offset_expr_bytes
        #[cfg(feature = "std")]
        let offset_expr_bytes = std::vec::Vec::new();
        #[cfg(not(feature = "std"))]
        let offset_expr_bytes = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            OffsetExprBytes::new(provider)?
        };

        Ok(Self {
            is_active,
            memory_index,
            offset_expr_bytes,
            data_size,
            requires_initialization,
        })
    }
}

/// Add minimal trait implementations for RuntimeElementExtraction
impl wrt_foundation::traits::Checksummable for RuntimeElementExtraction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[self.is_active as u8]);
        if let Some(index) = self.table_index {
            checksum.update_slice(&index.to_le_bytes());
        }
        self.init_data_type.update_checksum(checksum);
        checksum.update_slice(&[self.requires_initialization as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for RuntimeElementExtraction {
    fn serialized_size(&self) -> usize {
        16 // Simplified
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(&[self.is_active as u8])?;
        writer.write_all(&self.table_index.unwrap_or(0).to_le_bytes())?;
        self.init_data_type.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&[self.requires_initialization as u8])?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for RuntimeElementExtraction {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut bool_bytes = [0u8; 1];
        reader.read_exact(&mut bool_bytes)?;
        let is_active = bool_bytes[0] != 0;

        let mut index_bytes = [0u8; 4];
        reader.read_exact(&mut index_bytes)?;
        let table_index_val = u32::from_le_bytes(index_bytes);
        let table_index = if table_index_val == 0 { None } else { Some(table_index_val) };

        let init_data_type = ElementInitType::from_bytes_with_provider(reader, provider)?;

        reader.read_exact(&mut bool_bytes)?;
        let requires_initialization = bool_bytes[0] != 0;

        // Create defaults for complex fields
        #[cfg(feature = "std")]
        let offset_expr_bytes = std::vec::Vec::new();
        #[cfg(not(feature = "std"))]
        let offset_expr_bytes = {
            let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
            OffsetExprBytes::new(provider)?
        };

        Ok(Self {
            is_active,
            table_index,
            element_type: crate::types::RefType::Funcref, // Default
            offset_expr_bytes,
            init_data_type,
            requires_initialization,
        })
    }
}
