//! WebAssembly instruction implementations for the WRT runtime.
//!
//! This crate provides implementations of WebAssembly instructions
//! that can be used by various execution engines. It offers pure, stateless
//! implementations of WebAssembly instructions that can be integrated with
//! different execution engines.
//!
//! # Architecture
//!
//! The crate is organized into modules for different instruction categories:
//!
//! - `memory_ops`: Memory operations (load, store)
//! - `arithmetic_ops`: Arithmetic operations (add, subtract, multiply, divide)
//! - `control_ops`: Control flow operations (block, loop, if, branch)
//! - `comparison_ops`: Comparison operations (equality, relational)
//! - `conversion_ops`: Conversion operations (type conversions)
//! - `variable_ops`: Variable access operations (local, global)
//! - `table_ops`: Table operations (get, set, grow)
//!
//! Each module provides pure implementations that can be used with the
//! traits defined in `instruction_traits`.

// WRT - wrt-instructions
// Module: WebAssembly Instruction Implementations
// SW-REQ-ID: REQ_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)] // Rule 2
#![cfg_attr(not(feature = "std"), no_std)]
//#![warn(missing_docs)] // Temporarily disabled - docs will be added systematically
#![warn(clippy::missing_panics_doc)]

// Binary std/no_std choice
extern crate alloc;

// Binary std/no_std choice
// from wrt-foundation

// Import prelude for common type access
pub mod prelude;

// Bounded infrastructure for static memory allocation
#[cfg(not(feature = "std"))]
pub mod bounded_instruction_infra;

pub mod arithmetic_ops;
pub mod comparison_ops;
pub mod const_expr;
pub mod control_ops;
pub mod conversion_ops;
pub mod error_utils;
// pub mod execution; // Temporarily disabled due to compilation issues
pub mod instruction_traits;
pub mod memory_ops;
pub mod multi_memory;
pub mod parametric_ops;
pub mod reference_ops;
pub mod table_ops;
pub mod types;
pub mod validation;
pub mod variable_ops;

// CFI-enhanced control flow operations
pub mod cfi_control_ops;

// SIMD operations
pub mod simd_ops;

// WebAssembly 3.0 Aggregate operations
pub mod aggregate_ops;

// WebAssembly 3.0 Atomic operations (Threads and Atomics proposal)
pub mod atomic_ops;

// WebAssembly 3.0 Branch Hinting operations
pub mod branch_hinting;

// Re-export commonly used types
pub use control_ops::BranchTarget;
// Test module for arithmetic operations
#[cfg(test)]
// arithmetic tests moved to arithmetic_ops.rs

// Re-exports for convenience
// Re-export prelude for convenience
pub use prelude::*;
pub use wrt_error::{
    Error,
    Result,
};
pub use wrt_foundation::{
    types::ValueType,
    values::Value,
    BlockType,
    RefType,
};

// Re-export Aggregate operations
pub use crate::aggregate_ops::{
    AggregateOp,
    AggregateOperations,
    ArrayGet,
    ArrayLen,
    ArrayNew,
    ArraySet,
    StructGet,
    StructNew,
    StructSet,
};
// Re-export Atomic operations
pub use crate::atomic_ops::{
    AtomicCmpxchgInstr,
    AtomicFence,
    AtomicLoadOp,
    AtomicOp,
    AtomicRMWInstr,
    AtomicRMWOp,
    AtomicStoreOp,
    AtomicWaitNotifyOp,
};
// Re-export Branch Hinting operations
pub use crate::branch_hinting::{
    BrOnNonNull,
    BrOnNull,
    BranchHintOp,
    BranchHintingContext,
};
// Re-export CFI control flow operations
pub use crate::cfi_control_ops::{
    CfiControlFlowOps,
    CfiControlFlowProtection,
    CfiExecutionContext,
    CfiLandingPad,
    CfiProtectedBranchTarget,
    CfiProtectionLevel,
    CfiTargetProtection,
    CfiTargetType,
    DefaultCfiControlFlowOps,
};
pub use crate::control_ops::{
    Block,
    BrTable,
    CallIndirect,
    ControlBlockType,
    ControlContext,
    // BranchTarget is already exported from control_ops above
    ControlOp,
    FunctionOperations,
    Return,
};
pub use crate::memory_ops::{
    MemoryContext,
    MemoryGrow,
    MemoryLoad,
    MemoryOp,
    MemorySize,
    MemoryStore,
}; // Removed MemoryArg
// Re-export Reference operations
pub use crate::reference_ops::{
    RefAsNonNull,
    RefEq,
    RefFunc,
    RefIsNull,
    RefNull,
    ReferenceOp,
    ReferenceOperations,
};
// Re-export SIMD operations
pub use crate::simd_ops::{
    SimdContext,
    SimdExecutionContext,
    SimdInstruction,
    SimdOp,
};
pub use crate::{
    arithmetic_ops::{
        ArithmeticContext,
        ArithmeticOp,
    },
    comparison_ops::{
        ComparisonContext,
        ComparisonOp,
    },
    conversion_ops::{
        ConversionContext,
        ConversionOp,
    },
    instruction_traits::PureInstruction,
    parametric_ops::ParametricOp,
    table_ops::{
        ElemDrop,
        ElementSegmentOperations,
        TableContext,
        TableCopy,
        TableFill,
        TableGet,
        TableGrow,
        TableInit,
        TableOp,
        TableOperations,
        TableSet,
        TableSize,
    },
    variable_ops::VariableOp,
};

// If there's a combined Instruction enum, export it here. Otherwise, runtime
// will use the Ops. pub enum Instruction { Arithmetic(ArithmeticOp),
// Control(ControlOp), ... }

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-instructions is being tested in
// isolation #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
