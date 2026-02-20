//! WebAssembly Atomic Operation Execution Engine
//!
//! This module implements the runtime execution of WebAssembly 3.0 atomic
//! operations, providing thread-safe memory access with proper memory ordering
//! semantics.
//!
//! # Safety
//!
//! This module requires unsafe code for direct memory access to implement
//! atomic operations. All unsafe blocks are carefully reviewed and justified
//! for correctness.

#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::unsafe_derive_deserialize)]

// alloc is imported in lib.rs with proper feature gates

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    sync::Arc,
    vec::Vec,
};
// Import platform abstractions from wrt-foundation PAI layer
use core::sync::atomic::{
    AtomicU32,
    AtomicU64,
    AtomicUsize,
    Ordering as AtomicOrdering,
};
use core::time::Duration;
#[cfg(feature = "std")]
use alloc::{
    collections::BTreeMap,
    sync::Arc,
    vec::Vec,
};

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
#[cfg(all(not(feature = "std"), not(feature = "std")))]
use wrt_foundation::bounded::BoundedVec;
use wrt_foundation::{
    traits::BoundedCapacity,
    MemArg,
};
use wrt_instructions::atomic_ops::{
    AtomicCmpxchgInstr,
    AtomicFence,
    AtomicLoadOp,
    AtomicOp,
    AtomicRMWInstr,
    AtomicRMWOp,
    AtomicStoreOp,
    AtomicWaitNotifyOp,
    MemoryOrdering,
};

use crate::{
    bounded_runtime_infra::new_atomic_op_map,
    prelude::Debug,
    thread_manager::{
        ThreadExecutionStats,
        ThreadId,
        ThreadManager,
    },
};

// Type alias for return results
/// Result vector type for std environments
#[cfg(feature = "std")]
pub type ResultVec = Vec<u32>;
/// Result vector type for `no_std` environments with bounded capacity
#[cfg(all(not(feature = "std"), not(feature = "std")))]
pub type ResultVec =
    wrt_foundation::bounded::BoundedVec<u32, 256, wrt_foundation::safe_memory::NoStdProvider<8192>>;

// Type alias for thread ID vectors - use bounded collections consistently
type ThreadIdVec = wrt_foundation::bounded::BoundedVec<
    ThreadId,
    64,
    wrt_foundation::safe_memory::NoStdProvider<8192>,
>;

// Helper macro for creating Vec compatible with no_std
macro_rules! result_vec {
    () => {
        {
            #[cfg(feature = "std")]
            {
                Ok(Vec::new())
            }
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            {
                let provider = wrt_foundation::safe_managed_alloc!(8192, wrt_foundation::budget_aware_provider::CrateId::Runtime)?;
                wrt_foundation::bounded::BoundedVec::new(provider)?
            }
        }
    };
    ($item:expr; $count:expr) => {
        {
            #[cfg(feature = "std")]
            {
                Ok(vec![$item; $count])
            }
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            {
                let provider = wrt_foundation::safe_managed_alloc!(8192, wrt_foundation::budget_aware_provider::CrateId::Runtime)?;
                let mut v = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for _ in 0..$count {
                    v.push($item)?;
                }
                Ok(v)
            }
        }
    };
    ($($item:expr),+) => {
        {
            #[cfg(feature = "std")]
            {
                Ok(vec![$($item),+])
            }
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            {
                let provider = wrt_foundation::safe_managed_alloc!(8192, wrt_foundation::budget_aware_provider::CrateId::Runtime)?;
                let mut v = wrt_foundation::bounded::BoundedVec::new(provider)?;
                $(v.push($item)?;)+
                v
            }
        }
    };
}

/// Conversion from WebAssembly memory ordering to platform ordering
fn convert_memory_ordering(ordering: MemoryOrdering) -> AtomicOrdering {
    match ordering {
        MemoryOrdering::Unordered => AtomicOrdering::Relaxed,
        MemoryOrdering::SeqCst => AtomicOrdering::SeqCst,
        MemoryOrdering::Release => AtomicOrdering::Release,
        MemoryOrdering::Acquire => AtomicOrdering::Acquire,
        MemoryOrdering::AcqRel => AtomicOrdering::AcqRel,
        MemoryOrdering::Relaxed => AtomicOrdering::Relaxed,
    }
}

/// Atomic memory access context
#[derive(Debug)]
pub struct AtomicMemoryContext {
    /// Base memory for atomic operations
    memory_base:        *mut u8,
    /// Memory size in bytes
    memory_size:        AtomicUsize,
    /// Thread manager for coordination
    pub thread_manager: ThreadManager,
    /// Wait/notify coordination data structures
    #[cfg(feature = "std")]
    wait_queues:        crate::bounded_runtime_infra::BoundedAtomicOpMap<ThreadIdVec>,
    #[cfg(not(feature = "std"))]
    wait_queues:        [(u32, [Option<ThreadId>; 8]); 16], // Fixed arrays for no_std
    /// Atomic operation statistics
    pub stats:          AtomicExecutionStats,
}

impl AtomicMemoryContext {
    /// Create new atomic memory context
    pub fn new(
        memory_base: *mut u8,
        memory_size: usize,
        thread_manager: ThreadManager,
    ) -> Result<Self> {
        Ok(Self {
            memory_base,
            memory_size: AtomicUsize::new(memory_size),
            thread_manager,
            wait_queues: new_atomic_op_map()?,
            stats: AtomicExecutionStats::new(),
        })
    }

    /// Execute atomic operation
    pub fn execute_atomic(&mut self, thread_id: ThreadId, op: AtomicOp) -> Result<ResultVec> {
        self.stats.total_operations += 1;

        // Update thread statistics
        if let Ok(context) = self.thread_manager.get_thread_context_mut(thread_id) {
            context.stats.record_atomic_operation();
        }

        match op {
            AtomicOp::Load(load_op) => self.execute_atomic_load(load_op),
            AtomicOp::Store(store_op) => {
                // Pop value from stack for store operation
                let value = 0u64; // TODO: Should be popped from execution stack
                self.execute_atomic_store(store_op, value)?;
                result_vec![]
            },
            AtomicOp::RMW(rmw_op) => {
                // Pop value from stack for RMW operation
                let value = 0u64; // TODO: Should be popped from execution stack
                self.execute_atomic_rmw(rmw_op, value)
            },
            AtomicOp::Cmpxchg(cmpxchg_op) => {
                // Pop expected and replacement values from stack for compare-exchange operation
                let expected = 0u64; // TODO: Should be popped from execution stack
                let replacement = 0u64; // TODO: Should be popped from execution stack
                self.execute_atomic_cmpxchg(cmpxchg_op, expected, replacement)
            },
            AtomicOp::WaitNotify(wait_notify_op) => {
                self.execute_wait_notify(thread_id, wait_notify_op)
            },
            AtomicOp::Fence(fence) => {
                self.execute_atomic_fence(fence)?;
                result_vec![]
            },
        }
    }

    /// Execute atomic operation with provided operands
    pub fn execute_atomic_with_operands(
        &mut self,
        thread_id: ThreadId,
        op: AtomicOp,
        operands: &[u64],
    ) -> Result<ResultVec> {
        self.stats.total_operations += 1;

        // Update thread statistics
        if let Ok(context) = self.thread_manager.get_thread_context_mut(thread_id) {
            context.stats.record_atomic_operation();
        }

        match op {
            AtomicOp::Load(load_op) => self.execute_atomic_load(load_op),
            AtomicOp::Store(store_op) => {
                // Get value from operands
                let value = operands.first().copied().unwrap_or(0u64);
                self.execute_atomic_store(store_op, value)?;
                result_vec![]
            },
            AtomicOp::RMW(rmw_op) => {
                // Get value from operands
                let value = operands.first().copied().unwrap_or(0u64);
                self.execute_atomic_rmw(rmw_op, value)
            },
            AtomicOp::Cmpxchg(cmpxchg_op) => {
                // Get expected and replacement values from operands
                let expected = operands.first().copied().unwrap_or(0u64);
                let replacement = operands.get(1).copied().unwrap_or(0u64);
                self.execute_atomic_cmpxchg(cmpxchg_op, expected, replacement)
            },
            AtomicOp::WaitNotify(wait_notify_op) => {
                self.execute_wait_notify(thread_id, wait_notify_op)
            },
            AtomicOp::Fence(fence) => {
                self.execute_atomic_fence(fence)?;
                result_vec![]
            },
        }
    }

    /// Execute atomic load operation
    fn execute_atomic_load(&mut self, load_op: AtomicLoadOp) -> Result<ResultVec> {
        self.stats.load_operations += 1;

        match load_op {
            AtomicLoadOp::I32AtomicLoad { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = self.atomic_load_u32(addr, MemoryOrdering::SeqCst)?;
                result_vec![value]
            },
            AtomicLoadOp::I64AtomicLoad { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = self.atomic_load_u64(addr, MemoryOrdering::SeqCst)?;
                result_vec![value as u32, (value >> 32) as u32]
            },
            AtomicLoadOp::I32AtomicLoad8U { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = u32::from(self.atomic_load_u8(addr, MemoryOrdering::SeqCst)?);
                result_vec![value]
            },
            AtomicLoadOp::I32AtomicLoad16U { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = u32::from(self.atomic_load_u16(addr, MemoryOrdering::SeqCst)?);
                result_vec![value]
            },
            AtomicLoadOp::I64AtomicLoad8U { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = u64::from(self.atomic_load_u8(addr, MemoryOrdering::SeqCst)?);
                result_vec![value as u32, (value >> 32) as u32]
            },
            AtomicLoadOp::I64AtomicLoad16U { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = u64::from(self.atomic_load_u16(addr, MemoryOrdering::SeqCst)?);
                result_vec![value as u32, (value >> 32) as u32]
            },
            AtomicLoadOp::I64AtomicLoad32U { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let value = u64::from(self.atomic_load_u32(addr, MemoryOrdering::SeqCst)?);
                result_vec![value as u32, (value >> 32) as u32]
            },
        }
    }

    /// Execute atomic store operation
    fn execute_atomic_store(&mut self, store_op: AtomicStoreOp, value: u64) -> Result<()> {
        self.stats.store_operations += 1;

        match store_op {
            AtomicStoreOp::I32AtomicStore { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u32(addr, value as u32, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I64AtomicStore { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u64(addr, value, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I32AtomicStore8 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u8(addr, value as u8, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I32AtomicStore16 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u16(addr, value as u16, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I64AtomicStore8 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u8(addr, value as u8, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I64AtomicStore16 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u16(addr, value as u16, MemoryOrdering::SeqCst)
            },
            AtomicStoreOp::I64AtomicStore32 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_store_u32(addr, value as u32, MemoryOrdering::SeqCst)
            },
        }
    }

    /// Execute atomic read-modify-write operation
    fn execute_atomic_rmw(&mut self, rmw_op: AtomicRMWInstr, value: u64) -> Result<ResultVec> {
        self.stats.rmw_operations += 1;

        match rmw_op {
            AtomicRMWInstr::I32AtomicRmwAdd { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::Add,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwAdd { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            AtomicRMWInstr::I32AtomicRmwSub { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::Sub,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwSub { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            AtomicRMWInstr::I32AtomicRmwAnd { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::And,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwAnd { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::And, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            AtomicRMWInstr::I32AtomicRmwOr { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::Or,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwOr { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            AtomicRMWInstr::I32AtomicRmwXor { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::Xor,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwXor { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            AtomicRMWInstr::I32AtomicRmwXchg { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_rmw_u32(
                    addr,
                    value as u32,
                    AtomicRMWOp::Xchg,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicRMWInstr::I64AtomicRmwXchg { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_rmw_u64(addr, value, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            // 8-bit i32 RMW variants (zero-extend to i32)
            AtomicRMWInstr::I32AtomicRmw8AddU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw8SubU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw8AndU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::And, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw8OrU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw8XorU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw8XchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            // 16-bit i32 RMW variants (zero-extend to i32)
            AtomicRMWInstr::I32AtomicRmw16AddU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw16SubU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw16AndU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::And, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw16OrU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw16XorU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            AtomicRMWInstr::I32AtomicRmw16XchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            // 8-bit i64 RMW variants (zero-extend to i64)
            AtomicRMWInstr::I64AtomicRmw8AddU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw8SubU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw8AndU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::And, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw8OrU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw8XorU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw8XchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u8(addr, value as u8, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            // 16-bit i64 RMW variants (zero-extend to i64)
            AtomicRMWInstr::I64AtomicRmw16AddU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw16SubU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw16AndU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::And, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw16OrU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw16XorU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw16XchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u16(addr, value as u16, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            // 32-bit i64 RMW variants (zero-extend to i64)
            AtomicRMWInstr::I64AtomicRmw32AddU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::Add, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw32SubU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::Sub, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw32AndU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::And, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw32OrU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::Or, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw32XorU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::Xor, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            AtomicRMWInstr::I64AtomicRmw32XchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_rmw_u32(addr, value as u32, AtomicRMWOp::Xchg, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
        }
    }

    /// Execute atomic compare-and-exchange operation
    fn execute_atomic_cmpxchg(
        &mut self,
        cmpxchg_op: AtomicCmpxchgInstr,
        expected: u64,
        replacement: u64,
    ) -> Result<ResultVec> {
        self.stats.cmpxchg_operations += 1;

        match cmpxchg_op {
            AtomicCmpxchgInstr::I32AtomicRmwCmpxchg { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value = self.atomic_cmpxchg_u32(
                    addr,
                    expected as u32,
                    replacement as u32,
                    MemoryOrdering::SeqCst,
                )?;
                result_vec![old_value]
            },
            AtomicCmpxchgInstr::I64AtomicRmwCmpxchg { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old_value =
                    self.atomic_cmpxchg_u64(addr, expected, replacement, MemoryOrdering::SeqCst)?;
                result_vec![old_value as u32, (old_value >> 32) as u32]
            },
            // 8-bit i32 cmpxchg (zero-extend to i32)
            AtomicCmpxchgInstr::I32AtomicRmw8CmpxchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_cmpxchg_u8(addr, expected as u8, replacement as u8, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            // 16-bit i32 cmpxchg (zero-extend to i32)
            AtomicCmpxchgInstr::I32AtomicRmw16CmpxchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = self.atomic_cmpxchg_u16(addr, expected as u16, replacement as u16, MemoryOrdering::SeqCst)?;
                result_vec![u32::from(old)]
            },
            // 8-bit i64 cmpxchg (zero-extend to i64)
            AtomicCmpxchgInstr::I64AtomicRmw8CmpxchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_cmpxchg_u8(addr, expected as u8, replacement as u8, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            // 16-bit i64 cmpxchg (zero-extend to i64)
            AtomicCmpxchgInstr::I64AtomicRmw16CmpxchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_cmpxchg_u16(addr, expected as u16, replacement as u16, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
            // 32-bit i64 cmpxchg (zero-extend to i64)
            AtomicCmpxchgInstr::I64AtomicRmw32CmpxchgU { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let old = u64::from(self.atomic_cmpxchg_u32(addr, expected as u32, replacement as u32, MemoryOrdering::SeqCst)?);
                result_vec![old as u32, (old >> 32) as u32]
            },
        }
    }

    /// Execute wait/notify operations
    fn execute_wait_notify(
        &mut self,
        thread_id: ThreadId,
        wait_notify_op: AtomicWaitNotifyOp,
    ) -> Result<ResultVec> {
        match wait_notify_op {
            AtomicWaitNotifyOp::MemoryAtomicWait32 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_wait_u32(thread_id, addr, Duration::from_secs(u64::MAX))
            },
            AtomicWaitNotifyOp::MemoryAtomicWait64 { memarg } => {
                let addr = self.calculate_address(memarg)?;
                self.atomic_wait_u64(thread_id, addr, Duration::from_secs(u64::MAX))
            },
            AtomicWaitNotifyOp::MemoryAtomicNotify { memarg } => {
                let addr = self.calculate_address(memarg)?;
                let count = self.atomic_notify(addr, u32::MAX)?;
                result_vec![count]
            },
        }
    }

    /// Execute atomic fence operation
    fn execute_atomic_fence(&mut self, fence: AtomicFence) -> Result<()> {
        self.stats.fence_operations += 1;

        // Execute memory fence with specified ordering
        let ordering: AtomicOrdering = convert_memory_ordering(fence.ordering);

        // Platform-specific fence implementation
        match ordering {
            AtomicOrdering::SeqCst => {
                // Full memory barrier
                core::sync::atomic::fence(AtomicOrdering::SeqCst);
            },
            AtomicOrdering::Relaxed => {
                // No fence needed for relaxed ordering
            },
            _ => {
                core::sync::atomic::fence(ordering);
            },
        }

        Ok(())
    }

    // Low-level atomic memory operations

    fn calculate_address(&self, memarg: MemArg) -> Result<usize> {
        let addr = memarg.offset as usize;
        if addr >= self.memory_size.load(AtomicOrdering::Relaxed) {
            return Err(Error::runtime_execution_error(
                "Atomic operation address out of bounds",
            ));
        }
        Ok(addr)
    }

    /// Helper to safely get atomic reference from memory address
    ///
    /// # Safety
    ///
    /// This function creates atomic references to memory. It's safe because:
    /// - Address bounds are checked by `calculate_address()` before calling
    /// - Memory is valid WebAssembly linear memory owned by this context
    /// - Alignment requirements are checked by caller for multi-byte types
    /// - The atomic types ensure thread-safe access
    #[inline]
    unsafe fn get_atomic_ref<T>(&self, addr: usize) -> &T { unsafe {
        let ptr = self.memory_base.add(addr) as *const T;
        &*ptr
    }}

    fn atomic_load_u8(&self, addr: usize, ordering: MemoryOrdering) -> Result<u8> {
        // SAFETY: Bounds checked, using helper function
        let atomic_ref: &AtomicU8 = unsafe { self.get_atomic_ref(addr) };
        Ok(atomic_ref.load(convert_memory_ordering(ordering)))
    }

    fn atomic_load_u16(&self, addr: usize, ordering: MemoryOrdering) -> Result<u16> {
        if addr % 2 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u16 access",
            ));
        }
        // SAFETY: Bounds and alignment checked, using helper function
        let atomic_ref: &AtomicU16 = unsafe { self.get_atomic_ref(addr) };
        Ok(atomic_ref.load(convert_memory_ordering(ordering)))
    }

    fn atomic_load_u32(&self, addr: usize, ordering: MemoryOrdering) -> Result<u32> {
        if addr % 4 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u32 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU32 };
        let atomic_ref = unsafe { &*ptr };
        Ok(atomic_ref.load(convert_memory_ordering(ordering)))
    }

    fn atomic_load_u64(&self, addr: usize, ordering: MemoryOrdering) -> Result<u64> {
        if addr % 8 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u64 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU64 };
        let atomic_ref = unsafe { &*ptr };
        Ok(atomic_ref.load(convert_memory_ordering(ordering)))
    }

    fn atomic_store_u8(&self, addr: usize, value: u8, ordering: MemoryOrdering) -> Result<()> {
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU8 };
        let atomic_ref = unsafe { &*ptr };
        atomic_ref.store(value, convert_memory_ordering(ordering));
        Ok(())
    }

    fn atomic_store_u16(&self, addr: usize, value: u16, ordering: MemoryOrdering) -> Result<()> {
        if addr % 2 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u16 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU16 };
        let atomic_ref = unsafe { &*ptr };
        atomic_ref.store(value, convert_memory_ordering(ordering));
        Ok(())
    }

    fn atomic_store_u32(&self, addr: usize, value: u32, ordering: MemoryOrdering) -> Result<()> {
        if addr % 4 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u32 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU32 };
        let atomic_ref = unsafe { &*ptr };
        atomic_ref.store(value, convert_memory_ordering(ordering));
        Ok(())
    }

    fn atomic_store_u64(&self, addr: usize, value: u64, ordering: MemoryOrdering) -> Result<()> {
        if addr % 8 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u64 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU64 };
        let atomic_ref = unsafe { &*ptr };
        atomic_ref.store(value, convert_memory_ordering(ordering));
        Ok(())
    }

    fn atomic_rmw_u8(
        &self,
        addr: usize,
        value: u8,
        op: AtomicRMWOp,
        ordering: MemoryOrdering,
    ) -> Result<u8> {
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU8 };
        let atomic_ref = unsafe { &*ptr };
        let ordering = convert_memory_ordering(ordering);

        Ok(match op {
            AtomicRMWOp::Add => atomic_ref.fetch_add(value, ordering),
            AtomicRMWOp::Sub => atomic_ref.fetch_sub(value, ordering),
            AtomicRMWOp::And => atomic_ref.fetch_and(value, ordering),
            AtomicRMWOp::Or => atomic_ref.fetch_or(value, ordering),
            AtomicRMWOp::Xor => atomic_ref.fetch_xor(value, ordering),
            AtomicRMWOp::Xchg => atomic_ref.swap(value, ordering),
        })
    }

    fn atomic_rmw_u16(
        &self,
        addr: usize,
        value: u16,
        op: AtomicRMWOp,
        ordering: MemoryOrdering,
    ) -> Result<u16> {
        if addr % 2 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u16 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU16 };
        let atomic_ref = unsafe { &*ptr };
        let ordering = convert_memory_ordering(ordering);

        Ok(match op {
            AtomicRMWOp::Add => atomic_ref.fetch_add(value, ordering),
            AtomicRMWOp::Sub => atomic_ref.fetch_sub(value, ordering),
            AtomicRMWOp::And => atomic_ref.fetch_and(value, ordering),
            AtomicRMWOp::Or => atomic_ref.fetch_or(value, ordering),
            AtomicRMWOp::Xor => atomic_ref.fetch_xor(value, ordering),
            AtomicRMWOp::Xchg => atomic_ref.swap(value, ordering),
        })
    }

    fn atomic_rmw_u32(
        &self,
        addr: usize,
        value: u32,
        op: AtomicRMWOp,
        ordering: MemoryOrdering,
    ) -> Result<u32> {
        if addr % 4 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u32 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU32 };
        let atomic_ref = unsafe { &*ptr };
        let ordering = convert_memory_ordering(ordering);

        Ok(match op {
            AtomicRMWOp::Add => atomic_ref.fetch_add(value, ordering),
            AtomicRMWOp::Sub => atomic_ref.fetch_sub(value, ordering),
            AtomicRMWOp::And => atomic_ref.fetch_and(value, ordering),
            AtomicRMWOp::Or => atomic_ref.fetch_or(value, ordering),
            AtomicRMWOp::Xor => atomic_ref.fetch_xor(value, ordering),
            AtomicRMWOp::Xchg => atomic_ref.swap(value, ordering),
        })
    }

    fn atomic_rmw_u64(
        &self,
        addr: usize,
        value: u64,
        op: AtomicRMWOp,
        ordering: MemoryOrdering,
    ) -> Result<u64> {
        if addr % 8 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u64 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU64 };
        let atomic_ref = unsafe { &*ptr };
        let ordering = convert_memory_ordering(ordering);

        Ok(match op {
            AtomicRMWOp::Add => atomic_ref.fetch_add(value, ordering),
            AtomicRMWOp::Sub => atomic_ref.fetch_sub(value, ordering),
            AtomicRMWOp::And => atomic_ref.fetch_and(value, ordering),
            AtomicRMWOp::Or => atomic_ref.fetch_or(value, ordering),
            AtomicRMWOp::Xor => atomic_ref.fetch_xor(value, ordering),
            AtomicRMWOp::Xchg => atomic_ref.swap(value, ordering),
        })
    }

    fn atomic_cmpxchg_u8(
        &self,
        addr: usize,
        expected: u8,
        replacement: u8,
        ordering: MemoryOrdering,
    ) -> Result<u8> {
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU8 };
        let atomic_ref = unsafe { &*ptr };

        match atomic_ref.compare_exchange(
            expected,
            replacement,
            convert_memory_ordering(ordering),
            convert_memory_ordering(ordering),
        ) {
            Ok(old_value) => Ok(old_value),
            Err(old_value) => Ok(old_value),
        }
    }

    fn atomic_cmpxchg_u16(
        &self,
        addr: usize,
        expected: u16,
        replacement: u16,
        ordering: MemoryOrdering,
    ) -> Result<u16> {
        if addr % 2 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u16 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU16 };
        let atomic_ref = unsafe { &*ptr };

        match atomic_ref.compare_exchange(
            expected,
            replacement,
            convert_memory_ordering(ordering),
            convert_memory_ordering(ordering),
        ) {
            Ok(old_value) => Ok(old_value),
            Err(old_value) => Ok(old_value),
        }
    }

    fn atomic_cmpxchg_u32(
        &self,
        addr: usize,
        expected: u32,
        replacement: u32,
        ordering: MemoryOrdering,
    ) -> Result<u32> {
        if addr % 4 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u32 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU32 };
        let atomic_ref = unsafe { &*ptr };

        match atomic_ref.compare_exchange(
            expected,
            replacement,
            convert_memory_ordering(ordering),
            convert_memory_ordering(ordering),
        ) {
            Ok(old_value) => Ok(old_value),
            Err(old_value) => Ok(old_value),
        }
    }

    fn atomic_cmpxchg_u64(
        &self,
        addr: usize,
        expected: u64,
        replacement: u64,
        ordering: MemoryOrdering,
    ) -> Result<u64> {
        if addr % 8 != 0 {
            return Err(Error::runtime_execution_error(
                "Unaligned atomic u64 access",
            ));
        }
        let ptr = unsafe { self.memory_base.add(addr) as *const AtomicU64 };
        let atomic_ref = unsafe { &*ptr };

        match atomic_ref.compare_exchange(
            expected,
            replacement,
            convert_memory_ordering(ordering),
            convert_memory_ordering(ordering),
        ) {
            Ok(old_value) => Ok(old_value),
            Err(old_value) => Ok(old_value),
        }
    }

    fn atomic_wait_u32(
        &mut self,
        thread_id: ThreadId,
        addr: usize,
        timeout: Duration,
    ) -> Result<ResultVec> {
        self.stats.wait_operations += 1;

        // Add thread to wait queue for this address
        #[cfg(feature = "std")]
        {
            // BoundedMap API is different from HashMap - handle explicitly
            let provider = wrt_foundation::safe_managed_alloc!(
                8192,
                wrt_foundation::budget_aware_provider::CrateId::Runtime
            )?;
            let default_vec = wrt_foundation::bounded::BoundedVec::new(provider)
                .map_err(|_| Error::runtime_error("Failed to create thread wait queue"))?;

            match self.wait_queues.get(&(addr as u64))? {
                Some(mut existing_vec) => {
                    existing_vec
                        .push(thread_id)
                        .map_err(|_| Error::runtime_error("Thread wait queue capacity exceeded"))?;
                    self.wait_queues.insert(addr as u64, existing_vec)?;
                },
                None => {
                    let mut new_vec = default_vec;
                    new_vec
                        .push(thread_id)
                        .map_err(|_| Error::runtime_error("Failed to add thread to wait queue"))?;
                    self.wait_queues.insert(addr as u64, new_vec)?;
                },
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // Binary std/no_std choice
            let mut found = false;
            for (wait_addr, queue) in &mut self.wait_queues {
                if *wait_addr == addr as u32 {
                    // Find empty slot in queue
                    for slot in queue.iter_mut() {
                        if slot.is_none() {
                            *slot = Some(thread_id);
                            found = true;
                            break;
                        }
                    }
                    break;
                }
            }
            if !found {
                // Find empty queue slot
                for (wait_addr, queue) in &mut self.wait_queues {
                    if *wait_addr == 0 {
                        // 0 means unused
                        *wait_addr = addr as u32;
                        queue[0] = Some(thread_id);
                        break;
                    }
                }
            }
        }

        // Return 0 for successful wait (simplified - real implementation would suspend
        // thread)
        #[cfg(feature = "std")]
        return result_vec![0];
        #[cfg(not(feature = "std"))]
        {
            result_vec![0]
        }
    }

    fn atomic_wait_u64(
        &mut self,
        thread_id: ThreadId,
        addr: usize,
        timeout: Duration,
    ) -> Result<ResultVec> {
        // Same implementation as u32 wait but for 64-bit values
        self.atomic_wait_u32(thread_id, addr, timeout)
    }

    fn atomic_notify(&mut self, addr: usize, count: u32) -> Result<u32> {
        self.stats.notify_operations += 1;

        let mut notified = 0u32;

        #[cfg(feature = "std")]
        {
            if let Ok(Some(queue)) = self.wait_queues.get_mut(&(addr as u64)) {
                let to_notify = core::cmp::min(count as usize, queue.len());
                for _ in 0..to_notify {
                    if let Ok(Some(_thread_id)) = queue.pop() {
                        // In real implementation, would wake up the thread
                        notified += 1;
                    }
                }
                if queue.is_empty() {
                    self.wait_queues.remove(&(addr as u64))?;
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // Binary std/no_std choice
            for (wait_addr, queue) in &mut self.wait_queues {
                if *wait_addr == addr as u32 {
                    let mut removed = 0;
                    // For arrays, we remove by setting elements to None from the end
                    for slot in queue.iter_mut().rev() {
                        if removed >= count as usize {
                            break;
                        }
                        if slot.is_some() {
                            *slot = None;
                            removed += 1;
                            notified += 1;
                        }
                    }
                    break;
                }
            }
        }

        Ok(notified)
    }
}

/// Statistics for atomic operation execution
#[derive(Debug, Clone)]
pub struct AtomicExecutionStats {
    /// Total atomic operations executed
    pub total_operations:   u64,
    /// Atomic load operations
    pub load_operations:    u64,
    /// Atomic store operations
    pub store_operations:   u64,
    /// Atomic read-modify-write operations
    pub rmw_operations:     u64,
    /// Atomic compare-exchange operations
    pub cmpxchg_operations: u64,
    /// Atomic fence operations
    pub fence_operations:   u64,
    /// Atomic wait operations
    pub wait_operations:    u64,
    /// Atomic notify operations
    pub notify_operations:  u64,
    /// Memory ordering conflicts detected
    pub ordering_conflicts: u64,
}

impl AtomicExecutionStats {
    fn new() -> Self {
        Self {
            total_operations:   0,
            load_operations:    0,
            store_operations:   0,
            rmw_operations:     0,
            cmpxchg_operations: 0,
            fence_operations:   0,
            wait_operations:    0,
            notify_operations:  0,
            ordering_conflicts: 0,
        }
    }

    /// Get atomic operation throughput (operations per call)
    #[must_use]
    pub fn throughput(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.load_operations + self.store_operations + self.rmw_operations) as f64
                / self.total_operations as f64
        }
    }

    /// Check if atomic execution is performing well
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.total_operations > 0 && self.ordering_conflicts < self.total_operations / 10
    }
}

// Type aliases for atomic types missing from platform layer
type AtomicU8 = core::sync::atomic::AtomicU8;
type AtomicU16 = core::sync::atomic::AtomicU16;

