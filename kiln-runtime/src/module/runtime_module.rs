//! Runtime-specific module types and functionality
//!
//! This module contains runtime-specific data that extends the pure format
//! definitions from wrt-format. It handles execution concerns like start
//! functions and active segment initialization.

use crate::prelude::*;

/// Runtime-specific module data that extends format module
#[derive(Debug, Clone)]
pub struct RuntimeModuleData {
    /// Start function index for execution entry point
    pub start_function: Option<u32>,
    
    /// Active data segments that need runtime initialization
    pub active_data_segments: Vec<ActiveDataSegment>,
    
    /// Active element segments that need runtime initialization
    pub active_element_segments: Vec<ActiveElementSegment>,
}

/// Active data segment with runtime initialization info
#[derive(Debug, Clone)]
pub struct ActiveDataSegment {
    /// Memory index (0 in MVP)
    pub memory_index: u32,
    
    /// Offset expression to evaluate at runtime
    pub offset_expr: Vec<u8>,
    
    /// Index into the format module's data segments
    pub data_index: u32,
}

/// Active element segment with runtime initialization info
#[derive(Debug, Clone)]
pub struct ActiveElementSegment {
    /// Table index
    pub table_index: u32,
    
    /// Offset expression to evaluate at runtime
    pub offset_expr: Vec<u8>,
    
    /// Index into the format module's element segments
    pub element_index: u32,
}

impl RuntimeModuleData {
    /// Create new runtime module data
    pub fn new() -> Self {
        Self {
            start_function: None,
            active_data_segments: Vec::new(),
            active_element_segments: Vec::new(),
        }
    }
    
    /// Extract runtime data from a format module during conversion
    #[cfg(feature = "format")]
    pub fn from_format_module(module: &wrt_format::module::Module) -> Self {
        let mut runtime_data = Self::new();
        
        // Extract start function
        runtime_data.start_function = module.start;
        
        // Extract active data segments
        for (idx, data) in module.data.iter().enumerate() {
            if let wrt_format::pure_format_types::PureDataMode::Active { memory_index, .. } = &data.mode {
                runtime_data.active_data_segments.push(ActiveDataSegment {
                    memory_index: *memory_index,
                    offset_expr: data.offset.to_vec(),
                    data_index: idx as u32,
                };
            }
        }
        
        // Extract active element segments
        for (idx, elem) in module.elements.iter().enumerate() {
            if let wrt_format::pure_format_types::PureElementMode::Active { table_index, .. } = &elem.mode {
                runtime_data.active_element_segments.push(ActiveElementSegment {
                    table_index: *table_index,
                    offset_expr: elem.offset_expr.to_vec(),
                    element_index: idx as u32,
                };
            }
        }
        
        runtime_data
    }
}

impl Default for RuntimeModuleData {
    fn default() -> Self {
        Self::new()
    }
}