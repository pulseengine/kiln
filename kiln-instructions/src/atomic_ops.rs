//! WebAssembly 3.0 atomic operations implementation
//!
//! This module provides support for atomic memory operations including:
//! - Atomic loads and stores
//! - Read-modify-write operations
//! - Compare and exchange
//! - Wait and notify operations
//! - Memory fences

use wrt_foundation::MemArg;

use crate::prelude::{
    Debug,
    Eq,
    PartialEq,
    Result,
};

/// Memory ordering for atomic operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOrdering {
    /// Unordered atomic operation (WebAssembly default)
    Unordered,
    /// Sequentially consistent ordering
    SeqCst,
    /// Release ordering (store operations)
    Release,
    /// Acquire ordering (load operations)
    Acquire,
    /// Acquire-Release ordering (RMW operations)
    AcqRel,
    /// Relaxed ordering (no synchronization)
    Relaxed,
}

impl Default for MemoryOrdering {
    fn default() -> Self {
        Self::SeqCst
    }
}

/// Atomic read-modify-write operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicRMWOp {
    /// Atomic addition
    Add,
    /// Atomic subtraction
    Sub,
    /// Atomic bitwise AND
    And,
    /// Atomic bitwise OR
    Or,
    /// Atomic bitwise XOR
    Xor,
    /// Atomic exchange (swap)
    Xchg,
}

/// Atomic load instructions
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicLoadOp {
    /// i32.atomic.load
    I32AtomicLoad { memarg: MemArg },
    /// i64.atomic.load
    I64AtomicLoad { memarg: MemArg },
    /// `i32.atomic.load8_u`
    I32AtomicLoad8U { memarg: MemArg },
    /// `i32.atomic.load16_u`
    I32AtomicLoad16U { memarg: MemArg },
    /// `i64.atomic.load8_u`
    I64AtomicLoad8U { memarg: MemArg },
    /// `i64.atomic.load16_u`
    I64AtomicLoad16U { memarg: MemArg },
    /// `i64.atomic.load32_u`
    I64AtomicLoad32U { memarg: MemArg },
}

/// Atomic store instructions
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicStoreOp {
    /// i32.atomic.store
    I32AtomicStore { memarg: MemArg },
    /// i64.atomic.store
    I64AtomicStore { memarg: MemArg },
    /// i32.atomic.store8
    I32AtomicStore8 { memarg: MemArg },
    /// i32.atomic.store16
    I32AtomicStore16 { memarg: MemArg },
    /// i64.atomic.store8
    I64AtomicStore8 { memarg: MemArg },
    /// i64.atomic.store16
    I64AtomicStore16 { memarg: MemArg },
    /// i64.atomic.store32
    I64AtomicStore32 { memarg: MemArg },
}

/// Atomic read-modify-write instructions
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicRMWInstr {
    /// i32.atomic.rmw.add
    I32AtomicRmwAdd { memarg: MemArg },
    /// i64.atomic.rmw.add
    I64AtomicRmwAdd { memarg: MemArg },
    /// `i32.atomic.rmw8.add_u`
    I32AtomicRmw8AddU { memarg: MemArg },
    /// `i32.atomic.rmw16.add_u`
    I32AtomicRmw16AddU { memarg: MemArg },
    /// `i64.atomic.rmw8.add_u`
    I64AtomicRmw8AddU { memarg: MemArg },
    /// `i64.atomic.rmw16.add_u`
    I64AtomicRmw16AddU { memarg: MemArg },
    /// `i64.atomic.rmw32.add_u`
    I64AtomicRmw32AddU { memarg: MemArg },

    /// i32.atomic.rmw.sub
    I32AtomicRmwSub { memarg: MemArg },
    /// i64.atomic.rmw.sub
    I64AtomicRmwSub { memarg: MemArg },
    /// `i32.atomic.rmw8.sub_u`
    I32AtomicRmw8SubU { memarg: MemArg },
    /// `i32.atomic.rmw16.sub_u`
    I32AtomicRmw16SubU { memarg: MemArg },
    /// `i64.atomic.rmw8.sub_u`
    I64AtomicRmw8SubU { memarg: MemArg },
    /// `i64.atomic.rmw16.sub_u`
    I64AtomicRmw16SubU { memarg: MemArg },
    /// `i64.atomic.rmw32.sub_u`
    I64AtomicRmw32SubU { memarg: MemArg },

    /// i32.atomic.rmw.and
    I32AtomicRmwAnd { memarg: MemArg },
    /// i64.atomic.rmw.and
    I64AtomicRmwAnd { memarg: MemArg },
    /// `i32.atomic.rmw8.and_u`
    I32AtomicRmw8AndU { memarg: MemArg },
    /// `i32.atomic.rmw16.and_u`
    I32AtomicRmw16AndU { memarg: MemArg },
    /// `i64.atomic.rmw8.and_u`
    I64AtomicRmw8AndU { memarg: MemArg },
    /// `i64.atomic.rmw16.and_u`
    I64AtomicRmw16AndU { memarg: MemArg },
    /// `i64.atomic.rmw32.and_u`
    I64AtomicRmw32AndU { memarg: MemArg },

    /// i32.atomic.rmw.or
    I32AtomicRmwOr { memarg: MemArg },
    /// i64.atomic.rmw.or
    I64AtomicRmwOr { memarg: MemArg },
    /// `i32.atomic.rmw8.or_u`
    I32AtomicRmw8OrU { memarg: MemArg },
    /// `i32.atomic.rmw16.or_u`
    I32AtomicRmw16OrU { memarg: MemArg },
    /// `i64.atomic.rmw8.or_u`
    I64AtomicRmw8OrU { memarg: MemArg },
    /// `i64.atomic.rmw16.or_u`
    I64AtomicRmw16OrU { memarg: MemArg },
    /// `i64.atomic.rmw32.or_u`
    I64AtomicRmw32OrU { memarg: MemArg },

    /// i32.atomic.rmw.xor
    I32AtomicRmwXor { memarg: MemArg },
    /// i64.atomic.rmw.xor
    I64AtomicRmwXor { memarg: MemArg },
    /// `i32.atomic.rmw8.xor_u`
    I32AtomicRmw8XorU { memarg: MemArg },
    /// `i32.atomic.rmw16.xor_u`
    I32AtomicRmw16XorU { memarg: MemArg },
    /// `i64.atomic.rmw8.xor_u`
    I64AtomicRmw8XorU { memarg: MemArg },
    /// `i64.atomic.rmw16.xor_u`
    I64AtomicRmw16XorU { memarg: MemArg },
    /// `i64.atomic.rmw32.xor_u`
    I64AtomicRmw32XorU { memarg: MemArg },

    /// i32.atomic.rmw.xchg
    I32AtomicRmwXchg { memarg: MemArg },
    /// i64.atomic.rmw.xchg
    I64AtomicRmwXchg { memarg: MemArg },
    /// `i32.atomic.rmw8.xchg_u`
    I32AtomicRmw8XchgU { memarg: MemArg },
    /// `i32.atomic.rmw16.xchg_u`
    I32AtomicRmw16XchgU { memarg: MemArg },
    /// `i64.atomic.rmw8.xchg_u`
    I64AtomicRmw8XchgU { memarg: MemArg },
    /// `i64.atomic.rmw16.xchg_u`
    I64AtomicRmw16XchgU { memarg: MemArg },
    /// `i64.atomic.rmw32.xchg_u`
    I64AtomicRmw32XchgU { memarg: MemArg },
}

/// Atomic compare and exchange instructions
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicCmpxchgInstr {
    /// i32.atomic.rmw.cmpxchg
    I32AtomicRmwCmpxchg { memarg: MemArg },
    /// i64.atomic.rmw.cmpxchg
    I64AtomicRmwCmpxchg { memarg: MemArg },
    /// `i32.atomic.rmw8.cmpxchg_u`
    I32AtomicRmw8CmpxchgU { memarg: MemArg },
    /// `i32.atomic.rmw16.cmpxchg_u`
    I32AtomicRmw16CmpxchgU { memarg: MemArg },
    /// `i64.atomic.rmw8.cmpxchg_u`
    I64AtomicRmw8CmpxchgU { memarg: MemArg },
    /// `i64.atomic.rmw16.cmpxchg_u`
    I64AtomicRmw16CmpxchgU { memarg: MemArg },
    /// `i64.atomic.rmw32.cmpxchg_u`
    I64AtomicRmw32CmpxchgU { memarg: MemArg },
}

/// Wait and notify instructions
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicWaitNotifyOp {
    /// memory.atomic.wait32
    MemoryAtomicWait32 { memarg: MemArg },
    /// memory.atomic.wait64
    MemoryAtomicWait64 { memarg: MemArg },
    /// memory.atomic.notify
    MemoryAtomicNotify { memarg: MemArg },
}

/// Atomic fence instruction
#[derive(Debug, Clone, PartialEq)]
pub struct AtomicFence {
    /// Memory ordering for the fence
    pub ordering: MemoryOrdering,
}

/// All atomic operations
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicOp {
    /// Atomic load
    Load(AtomicLoadOp),
    /// Atomic store
    Store(AtomicStoreOp),
    /// Atomic read-modify-write
    RMW(AtomicRMWInstr),
    /// Atomic compare and exchange
    Cmpxchg(AtomicCmpxchgInstr),
    /// Wait/notify operations
    WaitNotify(AtomicWaitNotifyOp),
    /// Atomic fence
    Fence(AtomicFence),
}

/// Trait for atomic memory operations implementation
pub trait AtomicOperations {
    /// Atomic wait on 32-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_wait32(&mut self, addr: u32, expected: i32, timeout_ns: Option<u64>) -> Result<i32>;

    /// Atomic wait on 64-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_wait64(&mut self, addr: u32, expected: i64, timeout_ns: Option<u64>) -> Result<i32>;

    /// Notify waiters on memory address
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_notify(&mut self, addr: u32, count: u32) -> Result<u32>;

    /// Atomic load operations
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_load_i32(&self, addr: u32) -> Result<i32>;

    /// Atomic load 64-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_load_i64(&self, addr: u32) -> Result<i64>;

    /// Atomic store operations
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_store_i32(&mut self, addr: u32, value: i32) -> Result<()>;

    /// Atomic store 64-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_store_i64(&mut self, addr: u32, value: i64) -> Result<()>;

    /// Atomic read-modify-write operations
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_add_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_add_i64(&mut self, addr: u32, value: i64) -> Result<i64>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_sub_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_sub_i64(&mut self, addr: u32, value: i64) -> Result<i64>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_and_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_and_i64(&mut self, addr: u32, value: i64) -> Result<i64>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_or_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_or_i64(&mut self, addr: u32, value: i64) -> Result<i64>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_xor_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_xor_i64(&mut self, addr: u32, value: i64) -> Result<i64>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_xchg_i32(&mut self, addr: u32, value: i32) -> Result<i32>;
    /// # Errors
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_xchg_i64(&mut self, addr: u32, value: i64) -> Result<i64>;

    /// Atomic compare and exchange operations
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_cmpxchg_i32(&mut self, addr: u32, expected: i32, replacement: i32) -> Result<i32>;

    /// Atomic compare and exchange 64-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_cmpxchg_i64(&mut self, addr: u32, expected: i64, replacement: i64) -> Result<i64>;

    /// Atomic read-modify-write compare and exchange operations (additional
    /// variants)
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_cmpxchg_i32(&mut self, addr: u32, expected: i32, replacement: i32)
        -> Result<i32>;

    /// Atomic RMW compare and exchange 64-bit value
    ///
    /// # Errors
    ///
    /// Returns an error if memory access fails or address is misaligned
    fn atomic_rmw_cmpxchg_i64(&mut self, addr: u32, expected: i64, replacement: i64)
        -> Result<i64>;
}

/// WebAssembly opcodes for atomic operations
pub mod opcodes {
    // Atomic wait/notify
    pub const MEMORY_ATOMIC_NOTIFY: u8 = 0x00;
    pub const MEMORY_ATOMIC_WAIT32: u8 = 0x01;
    pub const MEMORY_ATOMIC_WAIT64: u8 = 0x02;
    pub const ATOMIC_FENCE: u8 = 0x03;

    // i32 atomic loads
    pub const I32_ATOMIC_LOAD: u8 = 0x10;
    pub const I64_ATOMIC_LOAD: u8 = 0x11;
    pub const I32_ATOMIC_LOAD8_U: u8 = 0x12;
    pub const I32_ATOMIC_LOAD16_U: u8 = 0x13;
    pub const I64_ATOMIC_LOAD8_U: u8 = 0x14;
    pub const I64_ATOMIC_LOAD16_U: u8 = 0x15;
    pub const I64_ATOMIC_LOAD32_U: u8 = 0x16;

    // Atomic stores
    pub const I32_ATOMIC_STORE: u8 = 0x17;
    pub const I64_ATOMIC_STORE: u8 = 0x18;
    pub const I32_ATOMIC_STORE8: u8 = 0x19;
    pub const I32_ATOMIC_STORE16: u8 = 0x1a;
    pub const I64_ATOMIC_STORE8: u8 = 0x1b;
    pub const I64_ATOMIC_STORE16: u8 = 0x1c;
    pub const I64_ATOMIC_STORE32: u8 = 0x1d;

    // i32 atomic RMW add
    pub const I32_ATOMIC_RMW_ADD: u8 = 0x1e;
    pub const I64_ATOMIC_RMW_ADD: u8 = 0x1f;
    pub const I32_ATOMIC_RMW8_ADD_U: u8 = 0x20;
    pub const I32_ATOMIC_RMW16_ADD_U: u8 = 0x21;
    pub const I64_ATOMIC_RMW8_ADD_U: u8 = 0x22;
    pub const I64_ATOMIC_RMW16_ADD_U: u8 = 0x23;
    pub const I64_ATOMIC_RMW32_ADD_U: u8 = 0x24;

    // i32 atomic RMW sub
    pub const I32_ATOMIC_RMW_SUB: u8 = 0x25;
    pub const I64_ATOMIC_RMW_SUB: u8 = 0x26;
    pub const I32_ATOMIC_RMW8_SUB_U: u8 = 0x27;
    pub const I32_ATOMIC_RMW16_SUB_U: u8 = 0x28;
    pub const I64_ATOMIC_RMW8_SUB_U: u8 = 0x29;
    pub const I64_ATOMIC_RMW16_SUB_U: u8 = 0x2a;
    pub const I64_ATOMIC_RMW32_SUB_U: u8 = 0x2b;

    // i32 atomic RMW and
    pub const I32_ATOMIC_RMW_AND: u8 = 0x2c;
    pub const I64_ATOMIC_RMW_AND: u8 = 0x2d;
    pub const I32_ATOMIC_RMW8_AND_U: u8 = 0x2e;
    pub const I32_ATOMIC_RMW16_AND_U: u8 = 0x2f;
    pub const I64_ATOMIC_RMW8_AND_U: u8 = 0x30;
    pub const I64_ATOMIC_RMW16_AND_U: u8 = 0x31;
    pub const I64_ATOMIC_RMW32_AND_U: u8 = 0x32;

    // i32 atomic RMW or
    pub const I32_ATOMIC_RMW_OR: u8 = 0x33;
    pub const I64_ATOMIC_RMW_OR: u8 = 0x34;
    pub const I32_ATOMIC_RMW8_OR_U: u8 = 0x35;
    pub const I32_ATOMIC_RMW16_OR_U: u8 = 0x36;
    pub const I64_ATOMIC_RMW8_OR_U: u8 = 0x37;
    pub const I64_ATOMIC_RMW16_OR_U: u8 = 0x38;
    pub const I64_ATOMIC_RMW32_OR_U: u8 = 0x39;

    // i32 atomic RMW xor
    pub const I32_ATOMIC_RMW_XOR: u8 = 0x3a;
    pub const I64_ATOMIC_RMW_XOR: u8 = 0x3b;
    pub const I32_ATOMIC_RMW8_XOR_U: u8 = 0x3c;
    pub const I32_ATOMIC_RMW16_XOR_U: u8 = 0x3d;
    pub const I64_ATOMIC_RMW8_XOR_U: u8 = 0x3e;
    pub const I64_ATOMIC_RMW16_XOR_U: u8 = 0x3f;
    pub const I64_ATOMIC_RMW32_XOR_U: u8 = 0x40;

    // i32 atomic RMW xchg
    pub const I32_ATOMIC_RMW_XCHG: u8 = 0x41;
    pub const I64_ATOMIC_RMW_XCHG: u8 = 0x42;
    pub const I32_ATOMIC_RMW8_XCHG_U: u8 = 0x43;
    pub const I32_ATOMIC_RMW16_XCHG_U: u8 = 0x44;
    pub const I64_ATOMIC_RMW8_XCHG_U: u8 = 0x45;
    pub const I64_ATOMIC_RMW16_XCHG_U: u8 = 0x46;
    pub const I64_ATOMIC_RMW32_XCHG_U: u8 = 0x47;

    // i32 atomic RMW cmpxchg
    pub const I32_ATOMIC_RMW_CMPXCHG: u8 = 0x48;
    pub const I64_ATOMIC_RMW_CMPXCHG: u8 = 0x49;
    pub const I32_ATOMIC_RMW8_CMPXCHG_U: u8 = 0x4a;
    pub const I32_ATOMIC_RMW16_CMPXCHG_U: u8 = 0x4b;
    pub const I64_ATOMIC_RMW8_CMPXCHG_U: u8 = 0x4c;
    pub const I64_ATOMIC_RMW16_CMPXCHG_U: u8 = 0x4d;
    pub const I64_ATOMIC_RMW32_CMPXCHG_U: u8 = 0x4e;
}

impl AtomicOp {
    /// Get the opcode for this atomic operation
    // Large match is intentional - maps all atomic operations to their opcodes.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn opcode(&self) -> u8 {
        use opcodes::{
            ATOMIC_FENCE,
            I32_ATOMIC_LOAD,
            I32_ATOMIC_LOAD16_U,
            I32_ATOMIC_LOAD8_U,
            I32_ATOMIC_RMW16_ADD_U,
            I32_ATOMIC_RMW16_AND_U,
            I32_ATOMIC_RMW16_CMPXCHG_U,
            I32_ATOMIC_RMW16_OR_U,
            I32_ATOMIC_RMW16_SUB_U,
            I32_ATOMIC_RMW16_XCHG_U,
            I32_ATOMIC_RMW16_XOR_U,
            I32_ATOMIC_RMW8_ADD_U,
            I32_ATOMIC_RMW8_AND_U,
            I32_ATOMIC_RMW8_CMPXCHG_U,
            I32_ATOMIC_RMW8_OR_U,
            I32_ATOMIC_RMW8_SUB_U,
            I32_ATOMIC_RMW8_XCHG_U,
            I32_ATOMIC_RMW8_XOR_U,
            I32_ATOMIC_RMW_ADD,
            I32_ATOMIC_RMW_AND,
            I32_ATOMIC_RMW_CMPXCHG,
            I32_ATOMIC_RMW_OR,
            I32_ATOMIC_RMW_SUB,
            I32_ATOMIC_RMW_XCHG,
            I32_ATOMIC_RMW_XOR,
            I32_ATOMIC_STORE,
            I32_ATOMIC_STORE16,
            I32_ATOMIC_STORE8,
            I64_ATOMIC_LOAD,
            I64_ATOMIC_LOAD16_U,
            I64_ATOMIC_LOAD32_U,
            I64_ATOMIC_LOAD8_U,
            I64_ATOMIC_RMW16_ADD_U,
            I64_ATOMIC_RMW16_AND_U,
            I64_ATOMIC_RMW16_CMPXCHG_U,
            I64_ATOMIC_RMW16_OR_U,
            I64_ATOMIC_RMW16_SUB_U,
            I64_ATOMIC_RMW16_XCHG_U,
            I64_ATOMIC_RMW16_XOR_U,
            I64_ATOMIC_RMW32_ADD_U,
            I64_ATOMIC_RMW32_AND_U,
            I64_ATOMIC_RMW32_CMPXCHG_U,
            I64_ATOMIC_RMW32_OR_U,
            I64_ATOMIC_RMW32_SUB_U,
            I64_ATOMIC_RMW32_XCHG_U,
            I64_ATOMIC_RMW32_XOR_U,
            I64_ATOMIC_RMW8_ADD_U,
            I64_ATOMIC_RMW8_AND_U,
            I64_ATOMIC_RMW8_CMPXCHG_U,
            I64_ATOMIC_RMW8_OR_U,
            I64_ATOMIC_RMW8_SUB_U,
            I64_ATOMIC_RMW8_XCHG_U,
            I64_ATOMIC_RMW8_XOR_U,
            I64_ATOMIC_RMW_ADD,
            I64_ATOMIC_RMW_AND,
            I64_ATOMIC_RMW_CMPXCHG,
            I64_ATOMIC_RMW_OR,
            I64_ATOMIC_RMW_SUB,
            I64_ATOMIC_RMW_XCHG,
            I64_ATOMIC_RMW_XOR,
            I64_ATOMIC_STORE,
            I64_ATOMIC_STORE16,
            I64_ATOMIC_STORE32,
            I64_ATOMIC_STORE8,
            MEMORY_ATOMIC_NOTIFY,
            MEMORY_ATOMIC_WAIT32,
            MEMORY_ATOMIC_WAIT64,
        };

        match self {
            AtomicOp::Load(load) => match load {
                AtomicLoadOp::I32AtomicLoad { .. } => I32_ATOMIC_LOAD,
                AtomicLoadOp::I64AtomicLoad { .. } => I64_ATOMIC_LOAD,
                AtomicLoadOp::I32AtomicLoad8U { .. } => I32_ATOMIC_LOAD8_U,
                AtomicLoadOp::I32AtomicLoad16U { .. } => I32_ATOMIC_LOAD16_U,
                AtomicLoadOp::I64AtomicLoad8U { .. } => I64_ATOMIC_LOAD8_U,
                AtomicLoadOp::I64AtomicLoad16U { .. } => I64_ATOMIC_LOAD16_U,
                AtomicLoadOp::I64AtomicLoad32U { .. } => I64_ATOMIC_LOAD32_U,
            },
            AtomicOp::Store(store) => match store {
                AtomicStoreOp::I32AtomicStore { .. } => I32_ATOMIC_STORE,
                AtomicStoreOp::I64AtomicStore { .. } => I64_ATOMIC_STORE,
                AtomicStoreOp::I32AtomicStore8 { .. } => I32_ATOMIC_STORE8,
                AtomicStoreOp::I32AtomicStore16 { .. } => I32_ATOMIC_STORE16,
                AtomicStoreOp::I64AtomicStore8 { .. } => I64_ATOMIC_STORE8,
                AtomicStoreOp::I64AtomicStore16 { .. } => I64_ATOMIC_STORE16,
                AtomicStoreOp::I64AtomicStore32 { .. } => I64_ATOMIC_STORE32,
            },
            AtomicOp::RMW(rmw) => match rmw {
                AtomicRMWInstr::I32AtomicRmwAdd { .. } => I32_ATOMIC_RMW_ADD,
                AtomicRMWInstr::I64AtomicRmwAdd { .. } => I64_ATOMIC_RMW_ADD,
                AtomicRMWInstr::I32AtomicRmw8AddU { .. } => I32_ATOMIC_RMW8_ADD_U,
                AtomicRMWInstr::I32AtomicRmw16AddU { .. } => I32_ATOMIC_RMW16_ADD_U,
                AtomicRMWInstr::I64AtomicRmw8AddU { .. } => I64_ATOMIC_RMW8_ADD_U,
                AtomicRMWInstr::I64AtomicRmw16AddU { .. } => I64_ATOMIC_RMW16_ADD_U,
                AtomicRMWInstr::I64AtomicRmw32AddU { .. } => I64_ATOMIC_RMW32_ADD_U,

                AtomicRMWInstr::I32AtomicRmwSub { .. } => I32_ATOMIC_RMW_SUB,
                AtomicRMWInstr::I64AtomicRmwSub { .. } => I64_ATOMIC_RMW_SUB,
                AtomicRMWInstr::I32AtomicRmw8SubU { .. } => I32_ATOMIC_RMW8_SUB_U,
                AtomicRMWInstr::I32AtomicRmw16SubU { .. } => I32_ATOMIC_RMW16_SUB_U,
                AtomicRMWInstr::I64AtomicRmw8SubU { .. } => I64_ATOMIC_RMW8_SUB_U,
                AtomicRMWInstr::I64AtomicRmw16SubU { .. } => I64_ATOMIC_RMW16_SUB_U,
                AtomicRMWInstr::I64AtomicRmw32SubU { .. } => I64_ATOMIC_RMW32_SUB_U,

                AtomicRMWInstr::I32AtomicRmwAnd { .. } => I32_ATOMIC_RMW_AND,
                AtomicRMWInstr::I64AtomicRmwAnd { .. } => I64_ATOMIC_RMW_AND,
                AtomicRMWInstr::I32AtomicRmw8AndU { .. } => I32_ATOMIC_RMW8_AND_U,
                AtomicRMWInstr::I32AtomicRmw16AndU { .. } => I32_ATOMIC_RMW16_AND_U,
                AtomicRMWInstr::I64AtomicRmw8AndU { .. } => I64_ATOMIC_RMW8_AND_U,
                AtomicRMWInstr::I64AtomicRmw16AndU { .. } => I64_ATOMIC_RMW16_AND_U,
                AtomicRMWInstr::I64AtomicRmw32AndU { .. } => I64_ATOMIC_RMW32_AND_U,

                AtomicRMWInstr::I32AtomicRmwOr { .. } => I32_ATOMIC_RMW_OR,
                AtomicRMWInstr::I64AtomicRmwOr { .. } => I64_ATOMIC_RMW_OR,
                AtomicRMWInstr::I32AtomicRmw8OrU { .. } => I32_ATOMIC_RMW8_OR_U,
                AtomicRMWInstr::I32AtomicRmw16OrU { .. } => I32_ATOMIC_RMW16_OR_U,
                AtomicRMWInstr::I64AtomicRmw8OrU { .. } => I64_ATOMIC_RMW8_OR_U,
                AtomicRMWInstr::I64AtomicRmw16OrU { .. } => I64_ATOMIC_RMW16_OR_U,
                AtomicRMWInstr::I64AtomicRmw32OrU { .. } => I64_ATOMIC_RMW32_OR_U,

                AtomicRMWInstr::I32AtomicRmwXor { .. } => I32_ATOMIC_RMW_XOR,
                AtomicRMWInstr::I64AtomicRmwXor { .. } => I64_ATOMIC_RMW_XOR,
                AtomicRMWInstr::I32AtomicRmw8XorU { .. } => I32_ATOMIC_RMW8_XOR_U,
                AtomicRMWInstr::I32AtomicRmw16XorU { .. } => I32_ATOMIC_RMW16_XOR_U,
                AtomicRMWInstr::I64AtomicRmw8XorU { .. } => I64_ATOMIC_RMW8_XOR_U,
                AtomicRMWInstr::I64AtomicRmw16XorU { .. } => I64_ATOMIC_RMW16_XOR_U,
                AtomicRMWInstr::I64AtomicRmw32XorU { .. } => I64_ATOMIC_RMW32_XOR_U,

                AtomicRMWInstr::I32AtomicRmwXchg { .. } => I32_ATOMIC_RMW_XCHG,
                AtomicRMWInstr::I64AtomicRmwXchg { .. } => I64_ATOMIC_RMW_XCHG,
                AtomicRMWInstr::I32AtomicRmw8XchgU { .. } => I32_ATOMIC_RMW8_XCHG_U,
                AtomicRMWInstr::I32AtomicRmw16XchgU { .. } => I32_ATOMIC_RMW16_XCHG_U,
                AtomicRMWInstr::I64AtomicRmw8XchgU { .. } => I64_ATOMIC_RMW8_XCHG_U,
                AtomicRMWInstr::I64AtomicRmw16XchgU { .. } => I64_ATOMIC_RMW16_XCHG_U,
                AtomicRMWInstr::I64AtomicRmw32XchgU { .. } => I64_ATOMIC_RMW32_XCHG_U,
            },
            AtomicOp::Cmpxchg(cmpxchg) => match cmpxchg {
                AtomicCmpxchgInstr::I32AtomicRmwCmpxchg { .. } => I32_ATOMIC_RMW_CMPXCHG,
                AtomicCmpxchgInstr::I64AtomicRmwCmpxchg { .. } => I64_ATOMIC_RMW_CMPXCHG,
                AtomicCmpxchgInstr::I32AtomicRmw8CmpxchgU { .. } => I32_ATOMIC_RMW8_CMPXCHG_U,
                AtomicCmpxchgInstr::I32AtomicRmw16CmpxchgU { .. } => I32_ATOMIC_RMW16_CMPXCHG_U,
                AtomicCmpxchgInstr::I64AtomicRmw8CmpxchgU { .. } => I64_ATOMIC_RMW8_CMPXCHG_U,
                AtomicCmpxchgInstr::I64AtomicRmw16CmpxchgU { .. } => I64_ATOMIC_RMW16_CMPXCHG_U,
                AtomicCmpxchgInstr::I64AtomicRmw32CmpxchgU { .. } => I64_ATOMIC_RMW32_CMPXCHG_U,
            },
            AtomicOp::WaitNotify(wait_notify) => match wait_notify {
                AtomicWaitNotifyOp::MemoryAtomicWait32 { .. } => MEMORY_ATOMIC_WAIT32,
                AtomicWaitNotifyOp::MemoryAtomicWait64 { .. } => MEMORY_ATOMIC_WAIT64,
                AtomicWaitNotifyOp::MemoryAtomicNotify { .. } => MEMORY_ATOMIC_NOTIFY,
            },
            AtomicOp::Fence(_) => ATOMIC_FENCE,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::vec;
    use super::*;

    #[test]
    fn test_memory_ordering_default() {
        assert_eq!(MemoryOrdering::default(), MemoryOrdering::SeqCst);
    }

    #[test]
    fn test_atomic_load_opcodes() {
        let memarg = MemArg {
            offset: 0,
            align_exponent:  2,
            memory_index: 0,
        };

        let tests = vec![
            (
                AtomicOp::Load(AtomicLoadOp::I32AtomicLoad { memarg }),
                opcodes::I32_ATOMIC_LOAD,
            ),
            (
                AtomicOp::Load(AtomicLoadOp::I64AtomicLoad { memarg }),
                opcodes::I64_ATOMIC_LOAD,
            ),
            (
                AtomicOp::Load(AtomicLoadOp::I32AtomicLoad8U { memarg }),
                opcodes::I32_ATOMIC_LOAD8_U,
            ),
            (
                AtomicOp::Load(AtomicLoadOp::I32AtomicLoad16U { memarg }),
                opcodes::I32_ATOMIC_LOAD16_U,
            ),
        ];

        for (op, expected_opcode) in tests {
            assert_eq!(op.opcode(), expected_opcode);
        }
    }

    #[test]
    fn test_atomic_store_opcodes() {
        let memarg = MemArg {
            offset: 0,
            align_exponent:  2,
            memory_index: 0,
        };

        let tests = vec![
            (
                AtomicOp::Store(AtomicStoreOp::I32AtomicStore { memarg }),
                opcodes::I32_ATOMIC_STORE,
            ),
            (
                AtomicOp::Store(AtomicStoreOp::I64AtomicStore { memarg }),
                opcodes::I64_ATOMIC_STORE,
            ),
            (
                AtomicOp::Store(AtomicStoreOp::I32AtomicStore8 { memarg }),
                opcodes::I32_ATOMIC_STORE8,
            ),
            (
                AtomicOp::Store(AtomicStoreOp::I32AtomicStore16 { memarg }),
                opcodes::I32_ATOMIC_STORE16,
            ),
        ];

        for (op, expected_opcode) in tests {
            assert_eq!(op.opcode(), expected_opcode);
        }
    }

    #[test]
    fn test_atomic_rmw_opcodes() {
        let memarg = MemArg {
            offset: 0,
            align_exponent:  2,
            memory_index: 0,
        };

        let tests = vec![
            (
                AtomicOp::RMW(AtomicRMWInstr::I32AtomicRmwAdd { memarg }),
                opcodes::I32_ATOMIC_RMW_ADD,
            ),
            (
                AtomicOp::RMW(AtomicRMWInstr::I64AtomicRmwSub { memarg }),
                opcodes::I64_ATOMIC_RMW_SUB,
            ),
            (
                AtomicOp::RMW(AtomicRMWInstr::I32AtomicRmwAnd { memarg }),
                opcodes::I32_ATOMIC_RMW_AND,
            ),
            (
                AtomicOp::RMW(AtomicRMWInstr::I64AtomicRmwOr { memarg }),
                opcodes::I64_ATOMIC_RMW_OR,
            ),
            (
                AtomicOp::RMW(AtomicRMWInstr::I32AtomicRmwXor { memarg }),
                opcodes::I32_ATOMIC_RMW_XOR,
            ),
            (
                AtomicOp::RMW(AtomicRMWInstr::I64AtomicRmwXchg { memarg }),
                opcodes::I64_ATOMIC_RMW_XCHG,
            ),
        ];

        for (op, expected_opcode) in tests {
            assert_eq!(op.opcode(), expected_opcode);
        }
    }

    #[test]
    fn test_atomic_cmpxchg_opcodes() {
        let memarg = MemArg {
            offset: 0,
            align_exponent:  2,
            memory_index: 0,
        };

        let tests = vec![
            (
                AtomicOp::Cmpxchg(AtomicCmpxchgInstr::I32AtomicRmwCmpxchg { memarg }),
                opcodes::I32_ATOMIC_RMW_CMPXCHG,
            ),
            (
                AtomicOp::Cmpxchg(AtomicCmpxchgInstr::I64AtomicRmwCmpxchg { memarg }),
                opcodes::I64_ATOMIC_RMW_CMPXCHG,
            ),
            (
                AtomicOp::Cmpxchg(AtomicCmpxchgInstr::I32AtomicRmw8CmpxchgU { memarg }),
                opcodes::I32_ATOMIC_RMW8_CMPXCHG_U,
            ),
        ];

        for (op, expected_opcode) in tests {
            assert_eq!(op.opcode(), expected_opcode);
        }
    }

    #[test]
    fn test_wait_notify_opcodes() {
        let memarg = MemArg {
            offset: 0,
            align_exponent:  2,
            memory_index: 0,
        };

        let tests = vec![
            (
                AtomicOp::WaitNotify(AtomicWaitNotifyOp::MemoryAtomicWait32 { memarg }),
                opcodes::MEMORY_ATOMIC_WAIT32,
            ),
            (
                AtomicOp::WaitNotify(AtomicWaitNotifyOp::MemoryAtomicWait64 { memarg }),
                opcodes::MEMORY_ATOMIC_WAIT64,
            ),
            (
                AtomicOp::WaitNotify(AtomicWaitNotifyOp::MemoryAtomicNotify { memarg }),
                opcodes::MEMORY_ATOMIC_NOTIFY,
            ),
        ];

        for (op, expected_opcode) in tests {
            assert_eq!(op.opcode(), expected_opcode);
        }
    }

    #[test]
    fn test_fence_opcode() {
        let fence = AtomicOp::Fence(AtomicFence {
            ordering: MemoryOrdering::SeqCst,
        });
        assert_eq!(fence.opcode(), opcodes::ATOMIC_FENCE);
    }

    #[test]
    fn test_rmw_op_variants() {
        // Ensure all RMW operation types are covered
        let ops = vec![
            AtomicRMWOp::Add,
            AtomicRMWOp::Sub,
            AtomicRMWOp::And,
            AtomicRMWOp::Or,
            AtomicRMWOp::Xor,
            AtomicRMWOp::Xchg,
        ];

        assert_eq!(ops.len(), 6);

        // Test that each variant is distinct
        for (i, op1) in ops.iter().enumerate() {
            for (j, op2) in ops.iter().enumerate() {
                if i == j {
                    assert_eq!(op1, op2);
                } else {
                    assert_ne!(op1, op2);
                }
            }
        }
    }
}
