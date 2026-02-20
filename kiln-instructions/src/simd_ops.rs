// WRT - wrt-instructions
// Module: SIMD Operations
// SW-REQ-ID: REQ_SIMD_INST_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! SIMD (Single Instruction, Multiple Data) instruction implementations for
//! WebAssembly.
//!
//! This module provides implementations for WebAssembly SIMD instructions (v128
//! operations). These instructions operate on 128-bit vectors and are essential
//! for high-performance computing in WebAssembly.

use wrt_error::Result;
use wrt_foundation::values::Value;

use crate::prelude::{
    Debug,
    PartialEq,
    PureInstruction,
};

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
use std::vec::Vec;

/// SIMD operation context trait for accessing SIMD functionality
pub trait SimdContext {
    /// Execute a SIMD operation on v128 values
    ///
    /// # Errors
    ///
    /// Returns an error if SIMD operation execution fails or operand types are invalid
    fn execute_simd_op(&mut self, op: SimdOp, inputs: &[Value]) -> Result<Value>;
}

/// SIMD instruction operations
#[derive(Debug, Clone, PartialEq)]
pub enum SimdOp {
    // --- Load and Store Operations ---
    V128Load { offset: u32, align: u32 },
    V128Load8x8S { offset: u32, align: u32 },
    V128Load8x8U { offset: u32, align: u32 },
    V128Load16x4S { offset: u32, align: u32 },
    V128Load16x4U { offset: u32, align: u32 },
    V128Load32x2S { offset: u32, align: u32 },
    V128Load32x2U { offset: u32, align: u32 },
    V128Load8Splat { offset: u32, align: u32 },
    V128Load16Splat { offset: u32, align: u32 },
    V128Load32Splat { offset: u32, align: u32 },
    V128Load64Splat { offset: u32, align: u32 },
    V128Store { offset: u32, align: u32 },

    // --- Lane Access Operations ---
    I8x16ExtractLaneS { lane: u8 },
    I8x16ExtractLaneU { lane: u8 },
    I8x16ReplaceLane { lane: u8 },
    I16x8ExtractLaneS { lane: u8 },
    I16x8ExtractLaneU { lane: u8 },
    I16x8ReplaceLane { lane: u8 },
    I32x4ExtractLane { lane: u8 },
    I32x4ReplaceLane { lane: u8 },
    I64x2ExtractLane { lane: u8 },
    I64x2ReplaceLane { lane: u8 },
    F32x4ExtractLane { lane: u8 },
    F32x4ReplaceLane { lane: u8 },
    F64x2ExtractLane { lane: u8 },
    F64x2ReplaceLane { lane: u8 },

    // --- Splat Operations ---
    I8x16Splat,
    I16x8Splat,
    I32x4Splat,
    I64x2Splat,
    F32x4Splat,
    F64x2Splat,

    // --- Arithmetic Operations ---
    // i8x16
    I8x16Add,
    I8x16Sub,
    I8x16Neg,
    I8x16Abs,
    I8x16MinS,
    I8x16MinU,
    I8x16MaxS,
    I8x16MaxU,
    I8x16AvgrU,

    // i16x8
    I16x8Add,
    I16x8Sub,
    I16x8Mul,
    I16x8Neg,
    I16x8Abs,
    I16x8MinS,
    I16x8MinU,
    I16x8MaxS,
    I16x8MaxU,
    I16x8AvgrU,

    // i32x4
    I32x4Add,
    I32x4Sub,
    I32x4Mul,
    I32x4Neg,
    I32x4Abs,
    I32x4MinS,
    I32x4MinU,
    I32x4MaxS,
    I32x4MaxU,

    // i64x2
    I64x2Add,
    I64x2Sub,
    I64x2Mul,
    I64x2Neg,
    I64x2Abs,

    // f32x4
    F32x4Add,
    F32x4Sub,
    F32x4Mul,
    F32x4Div,
    F32x4Neg,
    F32x4Sqrt,
    F32x4Abs,
    F32x4Min,
    F32x4Max,
    F32x4Pmin,
    F32x4Pmax,

    // f64x2
    F64x2Add,
    F64x2Sub,
    F64x2Mul,
    F64x2Div,
    F64x2Neg,
    F64x2Sqrt,
    F64x2Abs,
    F64x2Min,
    F64x2Max,
    F64x2Pmin,
    F64x2Pmax,

    // --- Comparison Operations ---
    // i8x16
    I8x16Eq,
    I8x16Ne,
    I8x16LtS,
    I8x16LtU,
    I8x16GtS,
    I8x16GtU,
    I8x16LeS,
    I8x16LeU,
    I8x16GeS,
    I8x16GeU,

    // i16x8
    I16x8Eq,
    I16x8Ne,
    I16x8LtS,
    I16x8LtU,
    I16x8GtS,
    I16x8GtU,
    I16x8LeS,
    I16x8LeU,
    I16x8GeS,
    I16x8GeU,

    // i32x4
    I32x4Eq,
    I32x4Ne,
    I32x4LtS,
    I32x4LtU,
    I32x4GtS,
    I32x4GtU,
    I32x4LeS,
    I32x4LeU,
    I32x4GeS,
    I32x4GeU,

    // i64x2
    I64x2Eq,
    I64x2Ne,
    I64x2LtS,
    I64x2GtS,
    I64x2LeS,
    I64x2GeS,

    // f32x4
    F32x4Eq,
    F32x4Ne,
    F32x4Lt,
    F32x4Gt,
    F32x4Le,
    F32x4Ge,

    // f64x2
    F64x2Eq,
    F64x2Ne,
    F64x2Lt,
    F64x2Gt,
    F64x2Le,
    F64x2Ge,

    // --- Shift Operations ---
    I8x16Shl,
    I8x16ShrS,
    I8x16ShrU,
    I16x8Shl,
    I16x8ShrS,
    I16x8ShrU,
    I32x4Shl,
    I32x4ShrS,
    I32x4ShrU,
    I64x2Shl,
    I64x2ShrS,
    I64x2ShrU,

    // --- Bitwise Operations ---
    V128Not,
    V128And,
    V128Or,
    V128Xor,
    V128AndNot,
    V128Bitselect,

    // --- Test Operations ---
    V128AnyTrue,
    I8x16AllTrue,
    I16x8AllTrue,
    I32x4AllTrue,
    I64x2AllTrue,

    // --- Conversion Operations ---
    I32x4TruncSatF32x4S,
    I32x4TruncSatF32x4U,
    F32x4ConvertI32x4S,
    F32x4ConvertI32x4U,
    I32x4TruncSatF64x2SZero,
    I32x4TruncSatF64x2UZero,
    F64x2ConvertLowI32x4S,
    F64x2ConvertLowI32x4U,
    F32x4DemoteF64x2Zero,
    F64x2PromoteLowF32x4,

    // --- Narrow Operations ---
    I8x16NarrowI16x8S,
    I8x16NarrowI16x8U,
    I16x8NarrowI32x4S,
    I16x8NarrowI32x4U,

    // --- Extend Operations ---
    I16x8ExtendLowI8x16S,
    I16x8ExtendHighI8x16S,
    I16x8ExtendLowI8x16U,
    I16x8ExtendHighI8x16U,
    I32x4ExtendLowI16x8S,
    I32x4ExtendHighI16x8S,
    I32x4ExtendLowI16x8U,
    I32x4ExtendHighI16x8U,
    I64x2ExtendLowI32x4S,
    I64x2ExtendHighI32x4S,
    I64x2ExtendLowI32x4U,
    I64x2ExtendHighI32x4U,

    // --- Advanced Operations ---
    I8x16Swizzle,
    I8x16Shuffle { lanes: [u8; 16] },

    // --- Saturating Arithmetic ---
    I8x16AddSatS,
    I8x16AddSatU,
    I8x16SubSatS,
    I8x16SubSatU,
    I16x8AddSatS,
    I16x8AddSatU,
    I16x8SubSatS,
    I16x8SubSatU,

    // --- Dot Product ---
    I32x4DotI16x8S,

    // --- Extended Multiplication ---
    I16x8ExtMulLowI8x16S,
    I16x8ExtMulHighI8x16S,
    I16x8ExtMulLowI8x16U,
    I16x8ExtMulHighI8x16U,
    I32x4ExtMulLowI16x8S,
    I32x4ExtMulHighI16x8S,
    I32x4ExtMulLowI16x8U,
    I32x4ExtMulHighI16x8U,
    I64x2ExtMulLowI32x4S,
    I64x2ExtMulHighI32x4S,
    I64x2ExtMulLowI32x4U,
    I64x2ExtMulHighI32x4U,

    // --- Pairwise Addition ---
    I16x8ExtAddPairwiseI8x16S,
    I16x8ExtAddPairwiseI8x16U,
    I32x4ExtAddPairwiseI16x8S,
    I32x4ExtAddPairwiseI16x8U,

    // --- Q15 Multiplication ---
    I16x8Q15MulrSatS,

    // --- Relaxed SIMD Operations (optional) ---
    F32x4RelaxedMin,
    F32x4RelaxedMax,
    F64x2RelaxedMin,
    F64x2RelaxedMax,
    I8x16RelaxedSwizzle,
    I32x4RelaxedTruncF32x4S,
    I32x4RelaxedTruncF32x4U,
    I32x4RelaxedTruncF64x2SZero,
    I32x4RelaxedTruncF64x2UZero,
    F32x4RelaxedMadd,
    F32x4RelaxedNmadd,
    F64x2RelaxedMadd,
    F64x2RelaxedNmadd,
    I8x16RelaxedLaneselect,
    I16x8RelaxedLaneselect,
    I32x4RelaxedLaneselect,
    I64x2RelaxedLaneselect,
    I16x8RelaxedQ15MulrS,
    I16x8RelaxedDotI8x16I7x16S,
    I32x4RelaxedDotI8x16I7x16AddS,
}

impl SimdOp {
    /// Get the number of input values this operation expects
    // Large match is intentional - SIMD has 200+ operations to dispatch.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn input_count(&self) -> usize {
        use SimdOp::{
            F32x4Abs,
            F32x4Add,
            F32x4ConvertI32x4S,
            F32x4ConvertI32x4U,
            F32x4DemoteF64x2Zero,
            F32x4Div,
            F32x4Eq,
            F32x4ExtractLane,
            F32x4Ge,
            F32x4Gt,
            F32x4Le,
            F32x4Lt,
            F32x4Max,
            F32x4Min,
            F32x4Mul,
            F32x4Ne,
            F32x4Neg,
            F32x4Pmax,
            F32x4Pmin,
            F32x4RelaxedMadd,
            F32x4RelaxedMax,
            F32x4RelaxedMin,
            F32x4RelaxedNmadd,
            F32x4ReplaceLane,
            F32x4Splat,
            F32x4Sqrt,
            F32x4Sub,
            F64x2Abs,
            F64x2Add,
            F64x2ConvertLowI32x4S,
            F64x2ConvertLowI32x4U,
            F64x2Div,
            F64x2Eq,
            F64x2ExtractLane,
            F64x2Ge,
            F64x2Gt,
            F64x2Le,
            F64x2Lt,
            F64x2Max,
            F64x2Min,
            F64x2Mul,
            F64x2Ne,
            F64x2Neg,
            F64x2Pmax,
            F64x2Pmin,
            F64x2PromoteLowF32x4,
            F64x2RelaxedMadd,
            F64x2RelaxedMax,
            F64x2RelaxedMin,
            F64x2RelaxedNmadd,
            F64x2ReplaceLane,
            F64x2Splat,
            F64x2Sqrt,
            F64x2Sub,
            I16x8Abs,
            I16x8Add,
            I16x8AddSatS,
            I16x8AddSatU,
            I16x8AllTrue,
            I16x8AvgrU,
            I16x8Eq,
            I16x8ExtAddPairwiseI8x16S,
            I16x8ExtAddPairwiseI8x16U,
            I16x8ExtMulHighI8x16S,
            I16x8ExtMulHighI8x16U,
            I16x8ExtMulLowI8x16S,
            I16x8ExtMulLowI8x16U,
            I16x8ExtendHighI8x16S,
            I16x8ExtendHighI8x16U,
            I16x8ExtendLowI8x16S,
            I16x8ExtendLowI8x16U,
            I16x8ExtractLaneS,
            I16x8ExtractLaneU,
            I16x8GeS,
            I16x8GeU,
            I16x8GtS,
            I16x8GtU,
            I16x8LeS,
            I16x8LeU,
            I16x8LtS,
            I16x8LtU,
            I16x8MaxS,
            I16x8MaxU,
            I16x8MinS,
            I16x8MinU,
            I16x8Mul,
            I16x8NarrowI32x4S,
            I16x8NarrowI32x4U,
            I16x8Ne,
            I16x8Neg,
            I16x8Q15MulrSatS,
            I16x8RelaxedDotI8x16I7x16S,
            I16x8RelaxedLaneselect,
            I16x8RelaxedQ15MulrS,
            I16x8ReplaceLane,
            I16x8Shl,
            I16x8ShrS,
            I16x8ShrU,
            I16x8Splat,
            I16x8Sub,
            I16x8SubSatS,
            I16x8SubSatU,
            I32x4Abs,
            I32x4Add,
            I32x4AllTrue,
            I32x4DotI16x8S,
            I32x4Eq,
            I32x4ExtAddPairwiseI16x8S,
            I32x4ExtAddPairwiseI16x8U,
            I32x4ExtMulHighI16x8S,
            I32x4ExtMulHighI16x8U,
            I32x4ExtMulLowI16x8S,
            I32x4ExtMulLowI16x8U,
            I32x4ExtendHighI16x8S,
            I32x4ExtendHighI16x8U,
            I32x4ExtendLowI16x8S,
            I32x4ExtendLowI16x8U,
            I32x4ExtractLane,
            I32x4GeS,
            I32x4GeU,
            I32x4GtS,
            I32x4GtU,
            I32x4LeS,
            I32x4LeU,
            I32x4LtS,
            I32x4LtU,
            I32x4MaxS,
            I32x4MaxU,
            I32x4MinS,
            I32x4MinU,
            I32x4Mul,
            I32x4Ne,
            I32x4Neg,
            I32x4RelaxedDotI8x16I7x16AddS,
            I32x4RelaxedLaneselect,
            I32x4RelaxedTruncF32x4S,
            I32x4RelaxedTruncF32x4U,
            I32x4RelaxedTruncF64x2SZero,
            I32x4RelaxedTruncF64x2UZero,
            I32x4ReplaceLane,
            I32x4Shl,
            I32x4ShrS,
            I32x4ShrU,
            I32x4Splat,
            I32x4Sub,
            I32x4TruncSatF32x4S,
            I32x4TruncSatF32x4U,
            I32x4TruncSatF64x2SZero,
            I32x4TruncSatF64x2UZero,
            I64x2Abs,
            I64x2Add,
            I64x2AllTrue,
            I64x2Eq,
            I64x2ExtMulHighI32x4S,
            I64x2ExtMulHighI32x4U,
            I64x2ExtMulLowI32x4S,
            I64x2ExtMulLowI32x4U,
            I64x2ExtendHighI32x4S,
            I64x2ExtendHighI32x4U,
            I64x2ExtendLowI32x4S,
            I64x2ExtendLowI32x4U,
            I64x2ExtractLane,
            I64x2GeS,
            I64x2GtS,
            I64x2LeS,
            I64x2LtS,
            I64x2Mul,
            I64x2Ne,
            I64x2Neg,
            I64x2RelaxedLaneselect,
            I64x2ReplaceLane,
            I64x2Shl,
            I64x2ShrS,
            I64x2ShrU,
            I64x2Splat,
            I64x2Sub,
            I8x16Abs,
            I8x16Add,
            I8x16AddSatS,
            I8x16AddSatU,
            I8x16AllTrue,
            I8x16AvgrU,
            I8x16Eq,
            I8x16ExtractLaneS,
            I8x16ExtractLaneU,
            I8x16GeS,
            I8x16GeU,
            I8x16GtS,
            I8x16GtU,
            I8x16LeS,
            I8x16LeU,
            I8x16LtS,
            I8x16LtU,
            I8x16MaxS,
            I8x16MaxU,
            I8x16MinS,
            I8x16MinU,
            I8x16NarrowI16x8S,
            I8x16NarrowI16x8U,
            I8x16Ne,
            I8x16Neg,
            I8x16RelaxedLaneselect,
            I8x16RelaxedSwizzle,
            I8x16ReplaceLane,
            I8x16Shl,
            I8x16ShrS,
            I8x16ShrU,
            I8x16Shuffle,
            I8x16Splat,
            I8x16Sub,
            I8x16SubSatS,
            I8x16SubSatU,
            I8x16Swizzle,
            V128And,
            V128AndNot,
            V128AnyTrue,
            V128Bitselect,
            V128Load,
            V128Load16Splat,
            V128Load16x4S,
            V128Load16x4U,
            V128Load32Splat,
            V128Load32x2S,
            V128Load32x2U,
            V128Load64Splat,
            V128Load8Splat,
            V128Load8x8S,
            V128Load8x8U,
            V128Not,
            V128Or,
            V128Store,
            V128Xor,
        };
        match self {
            // Operations that take 1 input:
            // - Load operations (memory index)
            // - Extract lane operations (vector)
            // - Splat operations (scalar value)
            // - Unary operations
            V128Load { .. }
            | V128Load8x8S { .. }
            | V128Load8x8U { .. }
            | V128Load16x4S { .. }
            | V128Load16x4U { .. }
            | V128Load32x2S { .. }
            | V128Load32x2U { .. }
            | V128Load8Splat { .. }
            | V128Load16Splat { .. }
            | V128Load32Splat { .. }
            | V128Load64Splat { .. }
            | I8x16ExtractLaneS { .. }
            | I8x16ExtractLaneU { .. }
            | I16x8ExtractLaneS { .. }
            | I16x8ExtractLaneU { .. }
            | I32x4ExtractLane { .. }
            | I64x2ExtractLane { .. }
            | F32x4ExtractLane { .. }
            | F64x2ExtractLane { .. }
            | I8x16Splat
            | I16x8Splat
            | I32x4Splat
            | I64x2Splat
            | F32x4Splat
            | F64x2Splat
            | I8x16Neg
            | I8x16Abs
            | I16x8Neg
            | I16x8Abs
            | I32x4Neg
            | I32x4Abs
            | I64x2Neg
            | I64x2Abs
            | F32x4Neg
            | F32x4Sqrt
            | F32x4Abs
            | F64x2Neg
            | F64x2Sqrt
            | F64x2Abs
            | V128Not
            | V128AnyTrue
            | I8x16AllTrue
            | I16x8AllTrue
            | I32x4AllTrue
            | I64x2AllTrue
            | I32x4TruncSatF32x4S
            | I32x4TruncSatF32x4U
            | F32x4ConvertI32x4S
            | F32x4ConvertI32x4U
            | I32x4TruncSatF64x2SZero
            | I32x4TruncSatF64x2UZero
            | F64x2ConvertLowI32x4S
            | F64x2ConvertLowI32x4U
            | F32x4DemoteF64x2Zero
            | F64x2PromoteLowF32x4
            | I16x8ExtendLowI8x16S
            | I16x8ExtendHighI8x16S
            | I16x8ExtendLowI8x16U
            | I16x8ExtendHighI8x16U
            | I32x4ExtendLowI16x8S
            | I32x4ExtendHighI16x8S
            | I32x4ExtendLowI16x8U
            | I32x4ExtendHighI16x8U
            | I64x2ExtendLowI32x4S
            | I64x2ExtendHighI32x4S
            | I64x2ExtendLowI32x4U
            | I64x2ExtendHighI32x4U
            | I16x8ExtAddPairwiseI8x16S
            | I16x8ExtAddPairwiseI8x16U
            | I32x4ExtAddPairwiseI16x8S
            | I32x4ExtAddPairwiseI16x8U => 1,

            // Operations that take 2 inputs:
            // - Store operations (memory index and value)
            // - Replace lane operations (vector and value)
            // - Binary operations
            V128Store { .. }
            | I8x16ReplaceLane { .. }
            | I16x8ReplaceLane { .. }
            | I32x4ReplaceLane { .. }
            | I64x2ReplaceLane { .. }
            | F32x4ReplaceLane { .. }
            | F64x2ReplaceLane { .. }
            | I8x16Add
            | I8x16Sub
            | I8x16MinS
            | I8x16MinU
            | I8x16MaxS
            | I8x16MaxU
            | I8x16AvgrU
            | I16x8Add
            | I16x8Sub
            | I16x8Mul
            | I16x8MinS
            | I16x8MinU
            | I16x8MaxS
            | I16x8MaxU
            | I16x8AvgrU
            | I32x4Add
            | I32x4Sub
            | I32x4Mul
            | I32x4MinS
            | I32x4MinU
            | I32x4MaxS
            | I32x4MaxU
            | I64x2Add
            | I64x2Sub
            | I64x2Mul
            | F32x4Add
            | F32x4Sub
            | F32x4Mul
            | F32x4Div
            | F32x4Min
            | F32x4Max
            | F32x4Pmin
            | F32x4Pmax
            | F64x2Add
            | F64x2Sub
            | F64x2Mul
            | F64x2Div
            | F64x2Min
            | F64x2Max
            | F64x2Pmin
            | F64x2Pmax
            | I8x16Eq
            | I8x16Ne
            | I8x16LtS
            | I8x16LtU
            | I8x16GtS
            | I8x16GtU
            | I8x16LeS
            | I8x16LeU
            | I8x16GeS
            | I8x16GeU
            | I16x8Eq
            | I16x8Ne
            | I16x8LtS
            | I16x8LtU
            | I16x8GtS
            | I16x8GtU
            | I16x8LeS
            | I16x8LeU
            | I16x8GeS
            | I16x8GeU
            | I32x4Eq
            | I32x4Ne
            | I32x4LtS
            | I32x4LtU
            | I32x4GtS
            | I32x4GtU
            | I32x4LeS
            | I32x4LeU
            | I32x4GeS
            | I32x4GeU
            | I64x2Eq
            | I64x2Ne
            | I64x2LtS
            | I64x2GtS
            | I64x2LeS
            | I64x2GeS
            | F32x4Eq
            | F32x4Ne
            | F32x4Lt
            | F32x4Gt
            | F32x4Le
            | F32x4Ge
            | F64x2Eq
            | F64x2Ne
            | F64x2Lt
            | F64x2Gt
            | F64x2Le
            | F64x2Ge
            | V128And
            | V128Or
            | V128Xor
            | V128AndNot
            | I8x16Shl
            | I8x16ShrS
            | I8x16ShrU
            | I16x8Shl
            | I16x8ShrS
            | I16x8ShrU
            | I32x4Shl
            | I32x4ShrS
            | I32x4ShrU
            | I64x2Shl
            | I64x2ShrS
            | I64x2ShrU
            | I8x16NarrowI16x8S
            | I8x16NarrowI16x8U
            | I16x8NarrowI32x4S
            | I16x8NarrowI32x4U
            | I8x16Swizzle
            | I8x16Shuffle { .. }
            | I8x16AddSatS
            | I8x16AddSatU
            | I8x16SubSatS
            | I8x16SubSatU
            | I16x8AddSatS
            | I16x8AddSatU
            | I16x8SubSatS
            | I16x8SubSatU
            | I16x8Q15MulrSatS
            | I32x4DotI16x8S
            | I16x8ExtMulLowI8x16S
            | I16x8ExtMulHighI8x16S
            | I16x8ExtMulLowI8x16U
            | I16x8ExtMulHighI8x16U
            | I32x4ExtMulLowI16x8S
            | I32x4ExtMulHighI16x8S
            | I32x4ExtMulLowI16x8U
            | I32x4ExtMulHighI16x8U
            | I64x2ExtMulLowI32x4S
            | I64x2ExtMulHighI32x4S
            | I64x2ExtMulLowI32x4U
            | I64x2ExtMulHighI32x4U
            | F32x4RelaxedMin
            | F32x4RelaxedMax
            | F64x2RelaxedMin
            | F64x2RelaxedMax
            | I8x16RelaxedSwizzle
            | I32x4RelaxedTruncF32x4S
            | I32x4RelaxedTruncF32x4U
            | I32x4RelaxedTruncF64x2SZero
            | I32x4RelaxedTruncF64x2UZero
            | I16x8RelaxedQ15MulrS
            | I16x8RelaxedDotI8x16I7x16S => 2,

            // Ternary operations take 3 inputs
            V128Bitselect
            | F32x4RelaxedMadd
            | F32x4RelaxedNmadd
            | F64x2RelaxedMadd
            | F64x2RelaxedNmadd
            | I8x16RelaxedLaneselect
            | I16x8RelaxedLaneselect
            | I32x4RelaxedLaneselect
            | I64x2RelaxedLaneselect
            | I32x4RelaxedDotI8x16I7x16AddS => 3,
        }
    }

    /// Get the number of output values this operation produces
    #[must_use]
    pub fn output_count(&self) -> usize {
        use SimdOp::V128Store;
        match self {
            // Store operations produce no outputs
            V128Store { .. } => 0,

            // All other operations produce 1 output
            _ => 1,
        }
    }
}

/// SIMD instruction implementation using the `PureInstruction` trait
#[derive(Debug, Clone, PartialEq)]
pub struct SimdInstruction {
    op: SimdOp,
}

impl SimdInstruction {
    /// Create a new SIMD instruction
    #[must_use]
    pub fn new(op: SimdOp) -> Self {
        Self { op }
    }

    /// Get the SIMD operation
    #[must_use]
    pub fn op(&self) -> &SimdOp {
        &self.op
    }
}

/// SIMD execution context trait for accessing execution state
pub trait SimdExecutionContext {
    /// Pop a value from the execution stack
    ///
    /// # Errors
    ///
    /// Returns an error if stack is empty
    fn pop_value(&mut self) -> Result<Value>;

    /// Push a value onto the execution stack
    ///
    /// # Errors
    ///
    /// Returns an error if stack is full
    fn push_value(&mut self, value: Value) -> Result<()>;

    /// Get access to SIMD context for executing SIMD operations
    fn simd_context(&mut self) -> &mut dyn SimdContext;
}

#[cfg(feature = "std")]
impl<T: SimdExecutionContext> PureInstruction<T, wrt_error::Error> for SimdInstruction {
    fn execute(&self, context: &mut T) -> Result<()> {
        // Get the required inputs from the execution stack
        let input_count = self.op.input_count();
        let mut inputs = Vec::with_capacity(input_count);

        // Pop inputs from the stack in reverse order (stack is LIFO)
        for _ in 0..input_count {
            inputs.push(context.pop_value()?);
        }
        inputs.reverse(); // Reverse to get correct order

        // Execute the SIMD operation
        let result = context.simd_context().execute_simd_op(self.op.clone(), &inputs)?;

        // Push result(s) back onto the stack if the operation produces output
        if self.op.output_count() > 0 {
            context.push_value(result)?;
        }

        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl<T: SimdExecutionContext> PureInstruction<T, wrt_error::Error> for SimdInstruction {
    fn execute(&self, _context: &mut T) -> Result<()> {
        // Binary std/no_std choice
        Err(wrt_error::Error::runtime_execution_error(
            "SIMD operations require alloc feature",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_op_input_count() {
        assert_eq!(SimdOp::I8x16Add.input_count(), 2);
        assert_eq!(SimdOp::I8x16Neg.input_count(), 1);
        assert_eq!(SimdOp::V128Bitselect.input_count(), 3);
        assert_eq!(SimdOp::I8x16Splat.input_count(), 1);
        assert_eq!(
            SimdOp::V128Store {
                offset: 0,
                align:  0,
            }
            .input_count(),
            2
        );
    }

    #[test]
    fn test_simd_op_output_count() {
        assert_eq!(SimdOp::I8x16Add.output_count(), 1);
        assert_eq!(
            SimdOp::V128Store {
                offset: 0,
                align:  0,
            }
            .output_count(),
            0
        );
        assert_eq!(SimdOp::V128AnyTrue.output_count(), 1);
    }

    #[test]
    fn test_simd_instruction_creation() {
        let inst = SimdInstruction::new(SimdOp::I8x16Add);
        assert_eq!(inst.op(), &SimdOp::I8x16Add);
    }
}
