//! Bulk Memory Operations Runtime Implementation with ASIL Compliance
//!
//! This module provides the complete execution logic for WebAssembly bulk
//! memory operations with support for all ASIL levels (QM, ASIL-A, ASIL-B,
//! ASIL-C, ASIL-D).
//!
//! # Operations Supported
//! - memory.fill - Fill memory region with a byte value
//! - memory.copy - Copy memory region within the same memory
//! - memory.init - Initialize memory from a data segment
//! - data.drop - Drop (mark as unavailable) a data segment
//! - memory.size - Get memory size in pages
//! - memory.grow - Grow memory by specified pages
//!
//! # Safety and Compliance
//! - No unsafe code in safety-critical configurations
//! - Deterministic execution across all ASIL levels
//! - Bounded memory usage with compile-time guarantees
//! - Comprehensive validation and error handling
//! - Proper bounds checking and overflow detection

// Binary std/no_std choice
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::format;

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::values::Value;
use wrt_instructions::memory_ops::{
    DataDrop,
    DataSegmentOperations,
    MemoryCopy,
    MemoryFill,
    MemoryGrow,
    MemoryInit,
    MemoryOperations,
    MemorySize,
};

/// Provider trait for bulk memory management across ASIL levels
pub trait BulkMemoryProvider {
    /// Execute bulk memory operation with provider-specific optimizations
    fn execute_with_provider(
        &self,
        op: &BulkMemoryOp,
        inputs: &[Value],
        memory: &mut dyn MemoryOperations,
        data_segments: Option<&mut dyn DataSegmentOperations>,
    ) -> Result<Option<Value>>;
}

/// Bulk memory operation types
#[derive(Debug, Clone)]
pub enum BulkMemoryOp {
    /// Fill memory region with byte value
    Fill(MemoryFill),
    /// Copy memory region within same memory
    Copy(MemoryCopy),
    /// Initialize memory from data segment
    Init(MemoryInit),
    /// Drop data segment
    DataDrop(DataDrop),
    /// Get memory size in pages
    Size(MemorySize),
    /// Grow memory by pages
    Grow(MemoryGrow),
}

/// Execute a bulk memory operation with ASIL-compliant implementation
///
/// This function provides the main entry point for all bulk memory operations,
/// ensuring consistent behavior across all ASIL levels.
///
/// # Arguments
/// * `op` - The bulk memory operation to execute
/// * `inputs` - Input values for the operation
/// * `memory` - Memory instance for the operation
/// * `data_segments` - Data segments (optional, only needed for init/drop
///   operations)
/// * `provider` - Memory provider for ASIL compliance
///
/// # Returns
/// * `Ok(Some(Value))` - The result value (for size/grow operations)
/// * `Ok(None)` - No result value (for fill/copy/init/drop operations)
/// * `Err(Error)` - If the operation fails validation or execution
///
/// # Safety
/// This function contains no unsafe code and is suitable for all ASIL levels.
pub fn execute_bulk_memory_operation(
    op: BulkMemoryOp,
    inputs: &[Value],
    memory: &mut dyn MemoryOperations,
    data_segments: Option<&mut dyn DataSegmentOperations>,
    provider: &dyn BulkMemoryProvider,
) -> Result<Option<Value>> {
    // Validate input count
    validate_input_count(&op, inputs)?;

    // Execute operation using provider-specific implementation
    let result = provider.execute_with_provider(&op, inputs, memory, data_segments)?;

    // Validate result
    validate_bulk_memory_result(&op, &result)?;

    Ok(result)
}

/// Validate input count for bulk memory operation
#[inline]
fn validate_input_count(op: &BulkMemoryOp, inputs: &[Value]) -> Result<()> {
    let expected = op.input_count();
    let actual = inputs.len();

    if actual != expected {
        return Err(Error::runtime_execution_error(
            "Bulk memory operation {:?} expects {} inputs, got {}",
        ));
    }

    Ok(())
}

/// Validate bulk memory operation result
#[inline]
fn validate_bulk_memory_result(op: &BulkMemoryOp, result: &Option<Value>) -> Result<()> {
    let expects_result = op.produces_result();
    let has_result = result.is_some();

    if expects_result && !has_result {
        return Err(Error::runtime_execution_error(
            "Bulk memory operation {:?} should produce a result but didn't",
        ));
    }

    if !expects_result && has_result {
        return Err(Error::runtime_execution_error(
            "Bulk memory operation {:?} should not produce a result but did",
        ));
    }

    // Validate result type for operations that produce values
    if let Some(value) = result {
        match value {
            Value::I32(_) => Ok(()),
            _ => Err(Error::runtime_execution_error(
                "Invalid result type for bulk memory operation {:?}",
            )),
        }
    } else {
        Ok(())
    }
}

impl BulkMemoryOp {
    /// Get the number of input values this operation expects
    pub fn input_count(&self) -> usize {
        match self {
            BulkMemoryOp::Fill(_) => 3,     // dest, value, size
            BulkMemoryOp::Copy(_) => 3,     // dest, src, size
            BulkMemoryOp::Init(_) => 3,     // dest, src, size
            BulkMemoryOp::DataDrop(_) => 0, // no inputs
            BulkMemoryOp::Size(_) => 0,     // no inputs
            BulkMemoryOp::Grow(_) => 1,     // delta pages
        }
    }

    /// Check if this operation produces a result value
    pub fn produces_result(&self) -> bool {
        match self {
            BulkMemoryOp::Fill(_) => false,
            BulkMemoryOp::Copy(_) => false,
            BulkMemoryOp::Init(_) => false,
            BulkMemoryOp::DataDrop(_) => false,
            BulkMemoryOp::Size(_) => true,
            BulkMemoryOp::Grow(_) => true,
        }
    }
}

/// Default bulk memory provider implementation for all ASIL levels
pub struct AssilCompliantBulkMemoryProvider;

impl BulkMemoryProvider for AssilCompliantBulkMemoryProvider {
    fn execute_with_provider(
        &self,
        op: &BulkMemoryOp,
        inputs: &[Value],
        memory: &mut dyn MemoryOperations,
        data_segments: Option<&mut dyn DataSegmentOperations>,
    ) -> Result<Option<Value>> {
        match op {
            BulkMemoryOp::Fill(fill_op) => {
                execute_memory_fill(fill_op, inputs, memory)?;
                Ok(None)
            },
            BulkMemoryOp::Copy(copy_op) => {
                execute_memory_copy(copy_op, inputs, memory)?;
                Ok(None)
            },
            BulkMemoryOp::Init(init_op) => {
                let data_segments = data_segments.ok_or_else(|| {
                    Error::validation_error("Data segments required for memory.init operation")
                })?;
                execute_memory_init(init_op, inputs, memory, data_segments)?;
                Ok(None)
            },
            BulkMemoryOp::DataDrop(drop_op) => {
                let data_segments = data_segments.ok_or_else(|| {
                    Error::validation_error("Data segments required for data.drop operation")
                })?;
                execute_data_drop(drop_op, data_segments)?;
                Ok(None)
            },
            BulkMemoryOp::Size(size_op) => {
                let result = execute_memory_size(size_op, memory)?;
                Ok(Some(result))
            },
            BulkMemoryOp::Grow(grow_op) => {
                let result = execute_memory_grow(grow_op, inputs, memory)?;
                Ok(Some(result))
            },
        }
    }
}

// ================================================================================================
// Bulk Memory Operation Implementations
// ================================================================================================

/// Execute memory.fill operation
fn execute_memory_fill(
    fill_op: &MemoryFill,
    inputs: &[Value],
    memory: &mut dyn MemoryOperations,
) -> Result<()> {
    // Validate inputs
    if inputs.len() != 3 {
        return Err(Error::validation_error(
            "memory.fill requires exactly 3 inputs: dest, value, size",
        ));
    }

    fill_op.execute(memory, &inputs[0], &inputs[1], &inputs[2])
}

/// Execute memory.copy operation
fn execute_memory_copy(
    copy_op: &MemoryCopy,
    inputs: &[Value],
    memory: &mut dyn MemoryOperations,
) -> Result<()> {
    // Validate inputs
    if inputs.len() != 3 {
        return Err(Error::validation_error(
            "memory.copy requires exactly 3 inputs: dest, src, size",
        ));
    }

    copy_op.execute(memory, &inputs[0], &inputs[1], &inputs[2])
}

/// Execute memory.init operation
fn execute_memory_init(
    init_op: &MemoryInit,
    inputs: &[Value],
    memory: &mut dyn MemoryOperations,
    data_segments: &mut dyn DataSegmentOperations,
) -> Result<()> {
    // Validate inputs
    if inputs.len() != 3 {
        return Err(Error::validation_error(
            "memory.init requires exactly 3 inputs: dest, src, size",
        ));
    }

    init_op.execute(memory, data_segments, &inputs[0], &inputs[1], &inputs[2])
}

/// Execute data.drop operation
fn execute_data_drop(
    drop_op: &DataDrop,
    data_segments: &mut dyn DataSegmentOperations,
) -> Result<()> {
    drop_op.execute(data_segments)
}

/// Execute memory.size operation
fn execute_memory_size(size_op: &MemorySize, memory: &dyn MemoryOperations) -> Result<Value> {
    size_op.execute(memory)
}

/// Execute memory.grow operation
fn execute_memory_grow(
    grow_op: &MemoryGrow,
    inputs: &[Value],
    memory: &mut dyn MemoryOperations,
) -> Result<Value> {
    // Validate inputs
    if inputs.len() != 1 {
        return Err(Error::validation_error(
            "memory.grow requires exactly 1 input: delta_pages",
        ));
    }

    grow_op.execute(memory, &inputs[0])
}

/// Extract i32 from a Value with validation
#[inline]
fn extract_i32(value: &Value) -> Result<i32> {
    match value {
        Value::I32(val) => Ok(*val),
        _ => Err(Error::runtime_execution_error(
            "Expected i32 value, got {:?}",
        )),
    }
}

// ================================================================================================
// Convenience Functions for Common Operations
// ================================================================================================

/// High-level memory fill operation
pub fn memory_fill(
    memory: &mut dyn MemoryOperations,
    dest: u32,
    value: u8,
    size: u32,
) -> Result<()> {
    let fill_op = MemoryFill::new(0); // Memory index 0 for MVP
    let inputs = [
        Value::I32(dest as i32),
        Value::I32(value as i32),
        Value::I32(size as i32),
    ];

    let provider = AssilCompliantBulkMemoryProvider;
    execute_bulk_memory_operation(
        BulkMemoryOp::Fill(fill_op),
        &inputs,
        memory,
        None,
        &provider,
    )?;

    Ok(())
}

/// High-level memory copy operation
pub fn memory_copy(
    memory: &mut dyn MemoryOperations,
    dest: u32,
    src: u32,
    size: u32,
) -> Result<()> {
    let copy_op = MemoryCopy::new(0, 0); // Same memory for MVP
    let inputs = [
        Value::I32(dest as i32),
        Value::I32(src as i32),
        Value::I32(size as i32),
    ];

    let provider = AssilCompliantBulkMemoryProvider;
    execute_bulk_memory_operation(
        BulkMemoryOp::Copy(copy_op),
        &inputs,
        memory,
        None,
        &provider,
    )?;

    Ok(())
}

/// High-level memory init operation
pub fn memory_init(
    memory: &mut dyn MemoryOperations,
    data_segments: &mut dyn DataSegmentOperations,
    data_index: u32,
    dest: u32,
    src: u32,
    size: u32,
) -> Result<()> {
    let init_op = MemoryInit::new(0, data_index); // Memory index 0 for MVP
    let inputs = [
        Value::I32(dest as i32),
        Value::I32(src as i32),
        Value::I32(size as i32),
    ];

    let provider = AssilCompliantBulkMemoryProvider;
    execute_bulk_memory_operation(
        BulkMemoryOp::Init(init_op),
        &inputs,
        memory,
        Some(data_segments),
        &provider,
    )?;

    Ok(())
}

/// High-level data drop operation
pub fn data_drop(data_segments: &mut dyn DataSegmentOperations, data_index: u32) -> Result<()> {
    let drop_op = DataDrop::new(data_index);

    let provider = AssilCompliantBulkMemoryProvider;
    execute_bulk_memory_operation(
        BulkMemoryOp::DataDrop(drop_op),
        &[],
        // Dummy memory reference - not used for data.drop
        &mut EmptyMemory,
        Some(data_segments),
        &provider,
    )?;

    Ok(())
}

/// High-level memory size operation
pub fn memory_size(memory: &dyn MemoryOperations) -> Result<u32> {
    let size_op = MemorySize::new(0); // Memory index 0 for MVP

    // Call the size operation directly
    let result = size_op.execute(memory)?;
    match result {
        Value::I32(pages) => Ok(pages as u32),
        _ => Err(Error::type_error("memory.size should return i32")),
    }
}

/// High-level memory grow operation
pub fn memory_grow(memory: &mut dyn MemoryOperations, delta_pages: u32) -> Result<u32> {
    let grow_op = MemoryGrow::new(0); // Memory index 0 for MVP
    let inputs = [Value::I32(delta_pages as i32)];

    let provider = AssilCompliantBulkMemoryProvider;
    let result = execute_bulk_memory_operation(
        BulkMemoryOp::Grow(grow_op),
        &inputs,
        memory,
        None,
        &provider,
    )?;

    match result {
        Some(Value::I32(old_pages)) => {
            if old_pages < 0 {
                Ok(u32::MAX) // WebAssembly convention: -1 means grow failed
            } else {
                Ok(old_pages as u32)
            }
        },
        _ => Err(Error::type_error("memory.grow should return i32")),
    }
}

// Dummy memory implementation for operations that don't actually use memory
struct EmptyMemory;

impl MemoryOperations for EmptyMemory {
    #[cfg(feature = "std")]
    fn read_bytes(&self, _offset: u64, _len: u64) -> Result<Vec<u8>> {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory read not supported",
        ))
    }

    #[cfg(not(feature = "std"))]
    fn read_bytes(
        &self,
        _offset: u64,
        _len: u64,
    ) -> Result<
        wrt_foundation::BoundedVec<u8, 65_536, wrt_foundation::safe_memory::NoStdProvider<65_536>>,
    > {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory read not supported",
        ))
    }

    fn write_bytes(&mut self, _offset: u64, _bytes: &[u8]) -> Result<()> {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory write not supported",
        ))
    }

    fn size_in_bytes(&self) -> Result<u64> {
        Ok(0)
    }

    fn grow(&mut self, _bytes: u64) -> Result<()> {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory grow not supported",
        ))
    }

    fn fill(&mut self, _offset: u64, _value: u8, _size: u64) -> Result<()> {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory fill not supported",
        ))
    }

    fn copy(&mut self, _dest: u64, _src: u64, _size: u64) -> Result<()> {
        Err(Error::runtime_unsupported_operation(
            "EmptyMemory copy not supported",
        ))
    }
}
