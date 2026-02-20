//! Runtime connector
//!
//! This module demonstrates how to use the clean interface between
//! wrt-format and wrt-runtime, showing the complete flow from format
//! parsing to runtime initialization.

use crate::prelude::*;
use crate::runtime_bridge::*;
use crate::module::Module;

#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(not(feature = "std"))]
use wrt_foundation::safe_memory::NoStdProvider;

// Type aliases for no_std mode
#[cfg(not(feature = "std"))]
type DefaultProvider = NoStdProvider<4096>;
#[cfg(not(feature = "std"))]
type Vec<T> = crate::WasmVec<T, DefaultProvider>;

use wrt_foundation::traits::BoundedCapacity;

/// Connector that bridges format and runtime layers
pub struct FormatRuntimeConnector;

impl FormatRuntimeConnector {
    /// Complete flow: parse format -> extract runtime data -> prepare for runtime
    pub fn prepare_module_for_runtime(module: &Module) -> Result<RuntimePreparationResult> {
        // Phase 1: Extract runtime data using format bridge
        let module_runtime_data = ModuleBridge::extract_module_runtime_data(module);
        
        // Phase 2: Create initialization plan
        let initialization_plan = ModuleBridge::create_initialization_plan(module);
        
        // Store requires_initialization before moving data
        let requires_initialization = module_runtime_data.requires_initialization;
        
        // Phase 3: Convert to runtime-compatible format
        let runtime_compatible_data = Self::convert_to_runtime_format(module_runtime_data)?;
        
        // Phase 4: Create runtime initialization guide
        let initialization_guide = Self::create_runtime_guide(initialization_plan)?;
        
        Ok(RuntimePreparationResult {
            runtime_data: runtime_compatible_data,
            initialization_guide,
            requires_runtime_initialization: requires_initialization,
            estimated_memory_usage: Self::estimate_memory_usage(module),
        })
    }
    
    /// Convert format runtime data to runtime-compatible format
    fn convert_to_runtime_format(data: ModuleRuntimeData) -> Result<RuntimeCompatibleData> {
        let mut runtime_data_segments = Vec::new();
        let mut runtime_element_segments = Vec::new();
        
        // Convert data extractions
        for extraction in data.data_extractions {
            runtime_data_segments.push(RuntimeDataSegmentInfo {
                memory_index: extraction.memory_index,
                offset_expr_bytes: extraction.offset_expr_bytes,
                data_size: extraction.data_size,
                requires_offset_evaluation: !extraction.offset_expr_bytes.is_empty(),
                initialization_priority: if extraction.is_active { 
                    InitializationPriority::High 
                } else { 
                    InitializationPriority::Low 
                },
            });
        }
        
        // Convert element extractions
        for extraction in data.element_extractions {
            runtime_element_segments.push(RuntimeElementSegmentInfo {
                table_index: extraction.table_index,
                element_type: extraction.element_type,
                offset_expr_bytes: extraction.offset_expr_bytes,
                init_data_type: match extraction.init_data_type {
                    ElementInitType::FunctionIndices => RuntimeElementInitType::FunctionIndices,
                    ElementInitType::ExpressionBytes => RuntimeElementInitType::ExpressionBytes,
                },
                requires_offset_evaluation: !extraction.offset_expr_bytes.is_empty(),
                initialization_priority: if extraction.is_active { 
                    InitializationPriority::High 
                } else { 
                    InitializationPriority::Low 
                },
            });
        }
        
        Ok(RuntimeCompatibleData {
            start_function: data.start_function,
            data_segments: runtime_data_segments,
            element_segments: runtime_element_segments,
            total_segments: data.data_extractions.len() + data.element_extractions.len(),
        })
    }
    
    /// Create runtime initialization guide
    fn create_runtime_guide(plan: ModuleInitializationPlan) -> Result<RuntimeInitializationGuide> {
        let mut steps = Vec::new();
        
        // Add data initialization steps
        for (index, hint) in plan.data_initialization_order {
            if hint.offset_evaluation_needed {
                steps.push(RuntimeInitializationStep {
                    step_type: RuntimeStepType::EvaluateDataOffset,
                    target_index: index,
                    estimated_time_us: 10, // 10 microseconds for offset evaluation
                    memory_requirement: 0,
                    asil_safe: true,
                });
            }
            
            steps.push(RuntimeInitializationStep {
                step_type: RuntimeStepType::InitializeDataSegment,
                target_index: index,
                estimated_time_us: 50, // 50 microseconds for data initialization
                memory_requirement: hint.data_bytes_ref.length,
                asil_safe: true,
            });
        }
        
        // Add element initialization steps
        for (index, hint) in plan.element_initialization_order {
            if hint.offset_evaluation_needed {
                steps.push(RuntimeInitializationStep {
                    step_type: RuntimeStepType::EvaluateElementOffset,
                    target_index: index,
                    estimated_time_us: 10,
                    memory_requirement: 0,
                    asil_safe: true,
                });
            }
            
            steps.push(RuntimeInitializationStep {
                step_type: RuntimeStepType::InitializeElementSegment,
                target_index: index,
                estimated_time_us: 30, // 30 microseconds per element
                memory_requirement: hint.element_count * 4, // 4 bytes per element reference
                asil_safe: true,
            });
        }
        
        // Add start function call
        if plan.start_function.is_some() {
            steps.push(RuntimeInitializationStep {
                step_type: RuntimeStepType::CallStartFunction,
                target_index: 0, // Start function index is stored separately
                estimated_time_us: 100, // 100 microseconds for function call
                memory_requirement: 0,
                asil_safe: true, // Depends on start function implementation
            });
        }
        
        Ok(RuntimeInitializationGuide {
            start_function: plan.start_function,
            initialization_steps: steps,
            total_estimated_time_us: plan.estimated_initialization_steps as u64 * 50,
            total_memory_requirement: Self::calculate_total_memory(&steps),
            asil_compatible: true, // Will be validated based on actual steps
        })
    }
    
    /// Estimate memory usage for a module
    fn estimate_memory_usage(module: &Module) -> MemoryUsageEstimate {
        let data_memory: usize = module.data.iter()
            .map(|d| d.init.len())
            .sum();
            
        let element_memory: usize = module.elements.iter()
            .map(|e| match &e.init {
                crate::module::ElementInit::FuncIndices(indices) => indices.len() * 4,
                crate::module::ElementInit::Expressions(exprs) => {
                    exprs.iter().map(|expr| expr.len()).sum::<usize>()
                },
            })
            .sum();
            
        let overhead = (data_memory + element_memory) / 10; // 10% overhead estimate
        
        MemoryUsageEstimate {
            data_memory,
            element_memory,
            initialization_overhead: overhead,
            total_estimated: data_memory + element_memory + overhead,
            peak_during_initialization: (data_memory + element_memory) * 2, // Double during init
        }
    }
    
    /// Calculate total memory requirement for initialization steps
    fn calculate_total_memory(steps: &[RuntimeInitializationStep]) -> usize {
        steps.iter().map(|step| step.memory_requirement).sum()
    }
}

/// Result of preparing module for runtime
#[derive(Debug)]
pub struct RuntimePreparationResult {
    /// Runtime-compatible data
    pub runtime_data: RuntimeCompatibleData,
    /// Initialization guide for runtime
    pub initialization_guide: RuntimeInitializationGuide,
    /// Whether runtime initialization is required
    pub requires_runtime_initialization: bool,
    /// Estimated memory usage
    pub estimated_memory_usage: MemoryUsageEstimate,
}

/// Runtime-compatible data format
#[derive(Debug)]
pub struct RuntimeCompatibleData {
    /// Start function index
    pub start_function: Option<u32>,
    /// Data segment information
    pub data_segments: Vec<RuntimeDataSegmentInfo>,
    /// Element segment information
    pub element_segments: Vec<RuntimeElementSegmentInfo>,
    /// Total number of segments
    pub total_segments: usize,
}

/// Runtime data segment information
#[derive(Debug)]
pub struct RuntimeDataSegmentInfo {
    /// Memory index for active segments
    pub memory_index: Option<u32>,
    /// Raw offset expression bytes
    pub offset_expr_bytes: Vec<u8>,
    /// Data size in bytes
    pub data_size: usize,
    /// Whether offset evaluation is required
    pub requires_offset_evaluation: bool,
    /// Initialization priority
    pub initialization_priority: InitializationPriority,
}

/// Runtime element segment information
#[derive(Debug)]
pub struct RuntimeElementSegmentInfo {
    /// Table index for active segments
    pub table_index: Option<u32>,
    /// Element type
    pub element_type: crate::types::RefType,
    /// Raw offset expression bytes
    pub offset_expr_bytes: Vec<u8>,
    /// Initialization data type
    pub init_data_type: RuntimeElementInitType,
    /// Whether offset evaluation is required
    pub requires_offset_evaluation: bool,
    /// Initialization priority
    pub initialization_priority: InitializationPriority,
}

/// Runtime element initialization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeElementInitType {
    /// Function indices
    FunctionIndices,
    /// Expression bytes
    ExpressionBytes,
}

/// Initialization priority for runtime scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationPriority {
    /// High priority (active segments)
    High,
    /// Medium priority (declared segments)
    Medium,
    /// Low priority (passive segments)
    Low,
}

/// Runtime initialization guide
#[derive(Debug)]
pub struct RuntimeInitializationGuide {
    /// Start function to call
    pub start_function: Option<u32>,
    /// Ordered initialization steps
    pub initialization_steps: Vec<RuntimeInitializationStep>,
    /// Total estimated time in microseconds
    pub total_estimated_time_us: u64,
    /// Total memory requirement
    pub total_memory_requirement: usize,
    /// Whether compatible with ASIL requirements
    pub asil_compatible: bool,
}

/// Individual runtime initialization step
#[derive(Debug)]
pub struct RuntimeInitializationStep {
    /// Type of step to perform
    pub step_type: RuntimeStepType,
    /// Target index (segment index, function index, etc.)
    pub target_index: usize,
    /// Estimated time in microseconds
    pub estimated_time_us: u64,
    /// Memory requirement in bytes
    pub memory_requirement: usize,
    /// Whether this step is ASIL-safe
    pub asil_safe: bool,
}

/// Type of runtime initialization step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStepType {
    /// Evaluate data segment offset expression
    EvaluateDataOffset,
    /// Initialize data segment
    InitializeDataSegment,
    /// Evaluate element segment offset expression
    EvaluateElementOffset,
    /// Initialize element segment
    InitializeElementSegment,
    /// Call start function
    CallStartFunction,
}

/// Memory usage estimate
#[derive(Debug)]
pub struct MemoryUsageEstimate {
    /// Memory used by data segments
    pub data_memory: usize,
    /// Memory used by element segments
    pub element_memory: usize,
    /// Overhead during initialization
    pub initialization_overhead: usize,
    /// Total estimated memory usage
    pub total_estimated: usize,
    /// Peak memory during initialization
    pub peak_during_initialization: usize,
}

/// Example usage function
pub fn example_usage() -> Result<RuntimePreparationSummary> {
    // This function demonstrates how to use the connector
    // In real usage, you would have a parsed module
    
    // Create a simple example module
    let module = Module::new();
    
    // Prepare for runtime
    let preparation_result = FormatRuntimeConnector::prepare_module_for_runtime(&module)?;
    
    // The preparation_result can now be passed to wrt-runtime
    // for actual initialization using the format_bridge module
    
    Ok(RuntimePreparationSummary {
        requires_initialization: preparation_result.requires_runtime_initialization,
        total_segments: preparation_result.runtime_data.total_segments,
        estimated_time_us: preparation_result.initialization_guide.total_estimated_time_us,
        memory_requirement_bytes: preparation_result.estimated_memory_usage.total_estimated,
    })
}

/// Summary of runtime preparation (for no_std compatibility)
#[derive(Debug)]
pub struct RuntimePreparationSummary {
    pub requires_initialization: bool,
    pub total_segments: usize,
    pub estimated_time_us: u64,
    pub memory_requirement_bytes: usize,
}