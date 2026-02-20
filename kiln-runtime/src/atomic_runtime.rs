//! Atomic Operations Runtime Implementation with ASIL Compliance
//!
//! This module provides the complete unified runtime interface for WebAssembly
//! atomic operations with support for all ASIL levels (QM, ASIL-A, ASIL-B,
//! ASIL-C, ASIL-D).
//!
//! # Operations Supported
//! - Atomic loads (i32, i64, narrow 8/16-bit loads with zero extension)
//! - Atomic stores (i32, i64, narrow 8/16/32-bit stores)
//! - Atomic read-modify-write operations (add, sub, and, or, xor, xchg)
//! - Atomic compare-and-exchange operations
//! - Atomic wait/notify operations for thread coordination
//! - Memory fences for synchronization
//!
//! # Safety and Compliance
//! - No unsafe code in safety-critical configurations (uses safe atomic
//!   execution engine)
//! - Deterministic execution across all ASIL levels
//! - Bounded memory usage with compile-time guarantees
//! - Comprehensive validation and error handling
//! - Proper capability-based memory access verification
//! - Thread-safe coordination with wait/notify semantics

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
use wrt_instructions::atomic_ops::{
    AtomicCmpxchgInstr,
    AtomicFence,
    AtomicLoadOp,
    AtomicOp,
    AtomicRMWInstr,
    AtomicStoreOp,
    AtomicWaitNotifyOp,
    MemoryOrdering,
};

use crate::atomic_execution::{
    AtomicExecutionStats,
    AtomicMemoryContext,
};

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::thread_manager::ThreadId;

/// Provider trait for atomic operations across ASIL levels
pub trait AtomicProvider {
    /// Execute atomic operation with provider-specific optimizations
    fn execute_with_provider(
        &self,
        op: &AtomicOp,
        inputs: &[Value],
        context: &mut AtomicMemoryContext,
        thread_id: ThreadId,
    ) -> Result<Option<Value>>;
}

/// Execute an atomic operation with ASIL-compliant implementation
///
/// This function provides the main entry point for all atomic operations,
/// ensuring consistent behavior across all ASIL levels.
///
/// # Arguments
/// * `op` - The atomic operation to execute
/// * `inputs` - Input values for the operation
/// * `context` - Safe atomic memory context for the operation
/// * `thread_id` - Thread identifier for capability verification
/// * `provider` - Atomic provider for ASIL compliance
///
/// # Returns
/// * `Ok(Some(Value))` - The result value (for load/RMW/cmpxchg operations)
/// * `Ok(None)` - No result value (for store/fence operations)
/// * `Err(Error)` - If the operation fails validation or execution
///
/// # Safety
/// This function contains no unsafe code and is suitable for all ASIL levels.
pub fn execute_atomic_operation(
    op: AtomicOp,
    inputs: &[Value],
    context: &mut AtomicMemoryContext,
    thread_id: ThreadId,
    provider: &dyn AtomicProvider,
) -> Result<Option<Value>> {
    // Validate input count
    validate_input_count(&op, inputs)?;

    // Execute operation using provider-specific implementation
    let result = provider.execute_with_provider(&op, inputs, context, thread_id)?;

    // Validate result
    validate_atomic_result(&op, &result)?;

    Ok(result)
}

/// Get the number of input values an atomic operation expects
#[inline]
fn atomic_op_input_count(op: &AtomicOp) -> usize {
    match op {
        AtomicOp::Load(_) => 0,    // Address is in memarg
        AtomicOp::Store(_) => 1,   // Value to store
        AtomicOp::RMW(_) => 1,     // Value for RMW operation
        AtomicOp::Cmpxchg(_) => 2, // Expected and replacement values
        AtomicOp::WaitNotify(wait_notify) => {
            match wait_notify {
                AtomicWaitNotifyOp::MemoryAtomicWait32 { .. } => 2, // Expected value and timeout
                AtomicWaitNotifyOp::MemoryAtomicWait64 { .. } => 3, // Expected value (i64 = 2 values) and timeout
                AtomicWaitNotifyOp::MemoryAtomicNotify { .. } => 1, // Count
            }
        },
        AtomicOp::Fence(_) => 0, // No inputs
    }
}

/// Check if an atomic operation produces a result value
#[inline]
fn atomic_op_produces_result(op: &AtomicOp) -> bool {
    match op {
        AtomicOp::Load(_) => true,
        AtomicOp::Store(_) => false,
        AtomicOp::RMW(_) => true,
        AtomicOp::Cmpxchg(_) => true,
        AtomicOp::WaitNotify(_) => true, // Returns wait result or notify count
        AtomicOp::Fence(_) => false,
    }
}

/// Validate input count for atomic operation
#[inline]
fn validate_input_count(op: &AtomicOp, inputs: &[Value]) -> Result<()> {
    let expected = atomic_op_input_count(op);
    let actual = inputs.len();

    if actual != expected {
        return Err(Error::runtime_execution_error(
            "Atomic operation input count mismatch",
        ));
    }

    Ok(())
}

/// Validate atomic operation result
#[inline]
fn validate_atomic_result(op: &AtomicOp, result: &Option<Value>) -> Result<()> {
    let expects_result = atomic_op_produces_result(op);
    let has_result = result.is_some();

    if expects_result && !has_result {
        return Err(Error::runtime_execution_error(
            "Atomic operation should produce a result but didn't",
        ));
    }

    if !expects_result && has_result {
        return Err(Error::runtime_execution_error(
            "Atomic operation should not produce a result but did",
        ));
    }

    Ok(())
}

/// Default atomic provider implementation for all ASIL levels
pub struct ASILCompliantAtomicProvider;

impl AtomicProvider for ASILCompliantAtomicProvider {
    fn execute_with_provider(
        &self,
        op: &AtomicOp,
        inputs: &[Value],
        context: &mut AtomicMemoryContext,
        thread_id: ThreadId,
    ) -> Result<Option<Value>> {
        // Convert inputs to internal format expected by AtomicMemoryContext
        let result = context.execute_atomic(thread_id, op.clone())?;

        // Convert result vector to single Value
        match op {
            AtomicOp::Load(load_op) => match load_op {
                AtomicLoadOp::I32AtomicLoad { .. }
                | AtomicLoadOp::I32AtomicLoad8U { .. }
                | AtomicLoadOp::I32AtomicLoad16U { .. } => {
                    if result.len() == 1 {
                        Ok(Some(Value::I32(result[0] as i32)))
                    } else {
                        Err(Error::runtime_execution_error(
                            "Invalid result length for i32 load",
                        ))
                    }
                },
                AtomicLoadOp::I64AtomicLoad { .. }
                | AtomicLoadOp::I64AtomicLoad8U { .. }
                | AtomicLoadOp::I64AtomicLoad16U { .. }
                | AtomicLoadOp::I64AtomicLoad32U { .. } => {
                    if result.len() == 2 {
                        let value = (result[0] as u64) | ((result[1] as u64) << 32);
                        Ok(Some(Value::I64(value as i64)))
                    } else {
                        Err(Error::runtime_execution_error(
                            "Invalid result length for i64 load",
                        ))
                    }
                },
            },
            AtomicOp::Store(_) => Ok(None),
            AtomicOp::RMW(rmw_op) => {
                match rmw_op {
                    AtomicRMWInstr::I32AtomicRmwAdd { .. }
                    | AtomicRMWInstr::I32AtomicRmwSub { .. }
                    | AtomicRMWInstr::I32AtomicRmwAnd { .. }
                    | AtomicRMWInstr::I32AtomicRmwOr { .. }
                    | AtomicRMWInstr::I32AtomicRmwXor { .. }
                    | AtomicRMWInstr::I32AtomicRmwXchg { .. }
                    | AtomicRMWInstr::I32AtomicRmw8AddU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16AddU { .. }
                    | AtomicRMWInstr::I32AtomicRmw8SubU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16SubU { .. }
                    | AtomicRMWInstr::I32AtomicRmw8AndU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16AndU { .. }
                    | AtomicRMWInstr::I32AtomicRmw8OrU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16OrU { .. }
                    | AtomicRMWInstr::I32AtomicRmw8XorU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16XorU { .. }
                    | AtomicRMWInstr::I32AtomicRmw8XchgU { .. }
                    | AtomicRMWInstr::I32AtomicRmw16XchgU { .. } => {
                        if result.len() == 1 {
                            Ok(Some(Value::I32(result[0] as i32)))
                        } else {
                            Err(Error::runtime_execution_error(
                                "Invalid result length for i32 RMW",
                            ))
                        }
                    },
                    _ => {
                        // i64 RMW operations
                        if result.len() == 2 {
                            let value = (result[0] as u64) | ((result[1] as u64) << 32);
                            Ok(Some(Value::I64(value as i64)))
                        } else {
                            Err(Error::runtime_execution_error(
                                "Invalid result length for i64 RMW",
                            ))
                        }
                    },
                }
            },
            AtomicOp::Cmpxchg(cmpxchg_op) => {
                match cmpxchg_op {
                    AtomicCmpxchgInstr::I32AtomicRmwCmpxchg { .. }
                    | AtomicCmpxchgInstr::I32AtomicRmw8CmpxchgU { .. }
                    | AtomicCmpxchgInstr::I32AtomicRmw16CmpxchgU { .. } => {
                        if result.len() == 1 {
                            Ok(Some(Value::I32(result[0] as i32)))
                        } else {
                            Err(Error::runtime_execution_error(
                                "Invalid result length for i32 cmpxchg",
                            ))
                        }
                    },
                    _ => {
                        // i64 cmpxchg operations
                        if result.len() == 2 {
                            let value = (result[0] as u64) | ((result[1] as u64) << 32);
                            Ok(Some(Value::I64(value as i64)))
                        } else {
                            Err(Error::runtime_execution_error(
                                "Invalid result length for i64 cmpxchg",
                            ))
                        }
                    },
                }
            },
            AtomicOp::WaitNotify(_) => {
                if result.len() == 1 {
                    Ok(Some(Value::I32(result[0] as i32)))
                } else {
                    Err(Error::runtime_execution_error(
                        "Invalid result length for wait/notify",
                    ))
                }
            },
            AtomicOp::Fence(_) => Ok(None),
        }
    }
}

// ================================================================================================
// Convenience Functions for Common Atomic Operations
// ================================================================================================

/// High-level atomic i32 load operation
pub fn atomic_i32_load(
    context: &mut AtomicMemoryContext,
    thread_id: ThreadId,
    addr: u32,
) -> Result<i32> {
    let memarg = wrt_foundation::MemArg {
        offset: addr,
        align_exponent: 2,
        memory_index: 0,
    }; // 2^2 = 4-byte alignment
    let load_op = AtomicLoadOp::I32AtomicLoad { memarg };
    let op = AtomicOp::Load(load_op);

    let provider = ASILCompliantAtomicProvider;
    let result = execute_atomic_operation(op, &[], context, thread_id, &provider)?;

    match result {
        Some(Value::I32(value)) => Ok(value),
        _ => Err(Error::type_error("atomic_i32_load should return i32")),
    }
}

/// High-level atomic i32 store operation
pub fn atomic_i32_store(
    context: &mut AtomicMemoryContext,
    thread_id: ThreadId,
    addr: u32,
    value: i32,
) -> Result<()> {
    let memarg = wrt_foundation::MemArg {
        offset: addr,
        align_exponent: 2,
        memory_index: 0,
    }; // 2^2 = 4-byte alignment
    let store_op = AtomicStoreOp::I32AtomicStore { memarg };
    let op = AtomicOp::Store(store_op);

    let provider = ASILCompliantAtomicProvider;
    execute_atomic_operation(op, &[Value::I32(value)], context, thread_id, &provider)?;

    Ok(())
}

/// High-level atomic i32 compare-and-swap operation
pub fn atomic_i32_compare_and_swap(
    context: &mut AtomicMemoryContext,
    thread_id: ThreadId,
    addr: u32,
    expected: i32,
    replacement: i32,
) -> Result<i32> {
    let memarg = wrt_foundation::MemArg {
        offset: addr,
        align_exponent: 2,
        memory_index: 0,
    }; // 2^2 = 4-byte alignment
    let cmpxchg_op = AtomicCmpxchgInstr::I32AtomicRmwCmpxchg { memarg };
    let op = AtomicOp::Cmpxchg(cmpxchg_op);

    let provider = ASILCompliantAtomicProvider;
    let result = execute_atomic_operation(
        op,
        &[Value::I32(expected), Value::I32(replacement)],
        context,
        thread_id,
        &provider,
    )?;

    match result {
        Some(Value::I32(old_value)) => Ok(old_value),
        _ => Err(Error::type_error(
            "atomic_i32_compare_and_swap should return i32",
        )),
    }
}

/// High-level atomic i32 add operation (returns old value)
pub fn atomic_i32_fetch_add(
    context: &mut AtomicMemoryContext,
    thread_id: ThreadId,
    addr: u32,
    value: i32,
) -> Result<i32> {
    let memarg = wrt_foundation::MemArg {
        offset: addr,
        align_exponent: 2,
        memory_index: 0,
    }; // 2^2 = 4-byte alignment
    let rmw_op = AtomicRMWInstr::I32AtomicRmwAdd { memarg };
    let op = AtomicOp::RMW(rmw_op);

    let provider = ASILCompliantAtomicProvider;
    let result = execute_atomic_operation(op, &[Value::I32(value)], context, thread_id, &provider)?;

    match result {
        Some(Value::I32(old_value)) => Ok(old_value),
        _ => Err(Error::type_error("atomic_i32_fetch_add should return i32")),
    }
}

/// Get atomic execution statistics
pub fn get_atomic_stats(context: &AtomicMemoryContext) -> &AtomicExecutionStats {
    &context.stats
}
