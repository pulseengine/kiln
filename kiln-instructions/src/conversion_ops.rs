// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Conversion operations for WebAssembly instructions.
//!
//! This module provides pure implementations for WebAssembly conversion
//! instructions, including type conversions between numeric types.

// WebAssembly value semantics require bit reinterpretation casts.
// i32/u32 and i64/u64 are the same bits - just different interpretations.
// These casts implement correct WASM behavior, not bugs.
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]

use wrt_math as math;

use crate::prelude::{
    Debug,
    Error,
    FloatBits32,
    FloatBits64,
    PureInstruction,
    Result,
    Value,
};

/// Represents a pure conversion operation for WebAssembly.
#[derive(Debug, Clone)]
pub enum ConversionOp {
    // i32 conversions
    /// Convert i64 to i32 (truncate)
    I32WrapI64,
    /// Convert f32 to i32 (signed, truncate)
    I32TruncF32S,
    /// Convert f32 to i32 (unsigned, truncate)
    I32TruncF32U,
    /// Convert f64 to i32 (signed, truncate)
    I32TruncF64S,
    /// Convert f64 to i32 (unsigned, truncate)
    I32TruncF64U,
    /// Convert i32 to f32 (reinterpret bits)
    I32ReinterpretF32,
    // Wasm 2.0: Sign-extension operators for i32
    /// Sign-extend 8-bit integer to 32-bit integer
    I32Extend8S,
    /// Sign-extend 16-bit integer to 32-bit integer
    I32Extend16S,

    // i64 conversions
    /// Sign-extend i32 to i64
    I64ExtendI32S,
    /// Zero-extend i32 to i64
    I64ExtendI32U,
    /// Convert f32 to i64 (signed, truncate)
    I64TruncF32S,
    /// Convert f32 to i64 (unsigned, truncate)
    I64TruncF32U,
    /// Convert f64 to i64 (signed, truncate)
    I64TruncF64S,
    /// Convert f64 to i64 (unsigned, truncate)
    I64TruncF64U,
    /// Convert i64 to f64 (reinterpret bits)
    I64ReinterpretF64,
    // Wasm 2.0: Sign-extension operators for i64
    /// Sign-extend 8-bit integer to 64-bit integer
    I64Extend8S,
    /// Sign-extend 16-bit integer to 64-bit integer
    I64Extend16S,
    /// Sign-extend 32-bit integer to 64-bit integer
    I64Extend32S,

    // f32 conversions
    /// Convert i32 to f32 (signed)
    F32ConvertI32S,
    /// Convert i32 to f32 (unsigned)
    F32ConvertI32U,
    /// Convert i64 to f32 (signed)
    F32ConvertI64S,
    /// Convert i64 to f32 (unsigned)
    F32ConvertI64U,
    /// Demote f64 to f32
    F32DemoteF64,
    /// Reinterpret i32 bits as f32
    F32ReinterpretI32,

    // f64 conversions
    /// Convert i32 to f64 (signed)
    F64ConvertI32S,
    /// Convert i32 to f64 (unsigned)
    F64ConvertI32U,
    /// Convert i64 to f64 (signed)
    F64ConvertI64S,
    /// Convert i64 to f64 (unsigned)
    F64ConvertI64U,
    /// Promote f32 to f64
    F64PromoteF32,
    /// Reinterpret i64 bits as f64
    F64ReinterpretI64,

    // Wasm 2.0: Non-trapping float-to-int conversions
    /// Convert f32 to i32 (signed, saturate)
    I32TruncSatF32S,
    /// Convert f32 to i32 (unsigned, saturate)
    I32TruncSatF32U,
    /// Convert f64 to i32 (signed, saturate)
    I32TruncSatF64S,
    /// Convert f64 to i32 (unsigned, saturate)
    I32TruncSatF64U,
    /// Convert f32 to i64 (signed, saturate)
    I64TruncSatF32S,
    /// Convert f32 to i64 (unsigned, saturate)
    I64TruncSatF32U,
    /// Convert f64 to i64 (signed, saturate)
    I64TruncSatF64S,
    /// Convert f64 to i64 (unsigned, saturate)
    I64TruncSatF64U,
}

/// Execution context for conversion operations
pub trait ConversionContext {
    /// Pop a value from the context
    ///
    /// # Errors
    ///
    /// Returns an error if stack is empty
    fn pop_conversion_value(&mut self) -> Result<Value>;

    /// Push a value to the context
    ///
    /// # Errors
    ///
    /// Returns an error if stack is full
    fn push_conversion_value(&mut self, value: Value) -> Result<()>;
}

impl<T: ConversionContext> PureInstruction<T, Error> for ConversionOp {
    // Large match is intentional - interpreter dispatch for all conversion opcodes.
    #[allow(clippy::too_many_lines)]
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            // i32 conversions
            Self::I32WrapI64 => {
                let a = context
                    .pop_conversion_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i32.wrap_i64 operand"))?;
                let result = math::i32_wrap_i64(a)?;
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncF32S => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i32.trunc_f32_s operand",
                    ));
                };
                // Convert wrt_foundation::FloatBits32 to wrt_math::FloatBits32
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i32_trunc_f32_s(math_bits)?;
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncF32U => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i32.trunc_f32_u operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i32_trunc_f32_u(math_bits)?;
                context.push_conversion_value(Value::I32(result as i32))
            },
            Self::I32TruncF64S => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i32.trunc_f64_s operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i32_trunc_f64_s(math_bits)?;
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncF64U => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i32.trunc_f64_u operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i32_trunc_f64_u(math_bits)?;
                context.push_conversion_value(Value::I32(result as i32))
            },
            Self::I32ReinterpretF32 => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i32.reinterpret_f32 operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i32_reinterpret_f32(math_bits)?;
                context.push_conversion_value(Value::I32(result))
            },

            // i32 sign extensions
            Self::I32Extend8S => {
                let a = context
                    .pop_conversion_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.extend8_s operand"))?;
                let result = math::i32_extend8_s(a)?;
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32Extend16S => {
                let a = context
                    .pop_conversion_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.extend16_s operand"))?;
                let result = math::i32_extend16_s(a)?;
                context.push_conversion_value(Value::I32(result))
            },

            // i64 conversions
            Self::I64ExtendI32S => {
                let a = context
                    .pop_conversion_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i64.extend_i32_s operand"))?;
                let result = math::i64_extend_i32_s(a)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64ExtendI32U => {
                let a = context.pop_conversion_value()?.as_u32().ok_or_else(|| {
                    Error::type_error("Expected I32 for i64.extend_i32_u operand")
                })?;
                let result = math::i64_extend_i32_u(a)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncF32S => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i64.trunc_f32_s operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i64_trunc_f32_s(math_bits)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncF32U => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i64.trunc_f32_u operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i64_trunc_f32_u(math_bits)?;
                context.push_conversion_value(Value::I64(result as i64))
            },
            Self::I64TruncF64S => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i64.trunc_f64_s operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i64_trunc_f64_s(math_bits)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncF64U => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i64.trunc_f64_u operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i64_trunc_f64_u(math_bits)?;
                context.push_conversion_value(Value::I64(result as i64))
            },
            Self::I64ReinterpretF64 => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i64.reinterpret_f64 operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i64_reinterpret_f64(math_bits)?;
                context.push_conversion_value(Value::I64(result))
            },

            // i64 sign extensions
            Self::I64Extend8S => {
                let a = context
                    .pop_conversion_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.extend8_s operand"))?;
                let result = math::i64_extend8_s(a)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64Extend16S => {
                let a = context
                    .pop_conversion_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.extend16_s operand"))?;
                let result = math::i64_extend16_s(a)?;
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64Extend32S => {
                let a = context
                    .pop_conversion_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.extend32_s operand"))?;
                let result = math::i64_extend32_s(a)?;
                context.push_conversion_value(Value::I64(result))
            },

            // f32 conversions
            Self::F32ConvertI32S => {
                let a = context
                    .pop_conversion_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for f32.convert_i32_s operand"))?;
                let result = math::f32_convert_i32_s(a)?;
                // Convert wrt_math::FloatBits32 to wrt_foundation::FloatBits32
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },
            Self::F32ConvertI32U => {
                let a = context.pop_conversion_value()?.as_u32().ok_or_else(|| {
                    Error::type_error("Expected I32 for f32.convert_i32_u operand")
                })?;
                let result = math::f32_convert_i32_u(a)?;
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },
            Self::F32ConvertI64S => {
                let a = context.pop_conversion_value()?.as_i64().ok_or_else(|| {
                    Error::type_error("Expected I64 for f32.convert_i64_s operand")
                })?;
                let result = math::f32_convert_i64_s(a)?;
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },
            Self::F32ConvertI64U => {
                let a = context.pop_conversion_value()?.as_i64().ok_or_else(|| {
                    Error::type_error("Expected I64 for f32.convert_i64_u operand")
                })?;
                let result = math::f32_convert_i64_u(a as u64)?;
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },
            Self::F32DemoteF64 => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error("Expected F64 for f32.demote_f64 operand"));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::f32_demote_f64(math_bits)?;
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },
            Self::F32ReinterpretI32 => {
                let a = context.pop_conversion_value()?.into_i32().map_err(|_| {
                    Error::type_error("Expected I32 for f32.reinterpret_i32 operand")
                })?;
                let result = math::f32_reinterpret_i32(a)?;
                context.push_conversion_value(Value::F32(FloatBits32(result.0)))
            },

            // f64 conversions
            Self::F64ConvertI32S => {
                let a = context
                    .pop_conversion_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for f64.convert_i32_s operand"))?;
                let result = math::f64_convert_i32_s(a)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },
            Self::F64ConvertI32U => {
                let a = context.pop_conversion_value()?.as_u32().ok_or_else(|| {
                    Error::type_error("Expected I32 for f64.convert_i32_u operand")
                })?;
                let result = math::f64_convert_i32_u(a)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },
            Self::F64ConvertI64S => {
                let a = context.pop_conversion_value()?.as_i64().ok_or_else(|| {
                    Error::type_error("Expected I64 for f64.convert_i64_s operand")
                })?;
                let result = math::f64_convert_i64_s(a)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },
            Self::F64ConvertI64U => {
                let a = context.pop_conversion_value()?.as_i64().ok_or_else(|| {
                    Error::type_error("Expected I64 for f64.convert_i64_u operand")
                })?;
                let result = math::f64_convert_i64_u(a as u64)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },
            Self::F64PromoteF32 => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for f64.promote_f32 operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::f64_promote_f32(math_bits)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },
            Self::F64ReinterpretI64 => {
                let a = context.pop_conversion_value()?.as_i64().ok_or_else(|| {
                    Error::type_error("Expected I64 for f64.reinterpret_i64 operand")
                })?;
                let result = math::f64_reinterpret_i64(a)?;
                context.push_conversion_value(Value::F64(FloatBits64(result.0)))
            },

            // Saturating truncations
            Self::I32TruncSatF32S => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i32.trunc_sat_f32_s operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i32_trunc_sat_f32_s(math_bits);
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncSatF32U => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i32.trunc_sat_f32_u operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i32_trunc_sat_f32_u(math_bits);
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncSatF64S => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i32.trunc_sat_f64_s operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i32_trunc_sat_f64_s(math_bits);
                context.push_conversion_value(Value::I32(result))
            },
            Self::I32TruncSatF64U => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i32.trunc_sat_f64_u operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i32_trunc_sat_f64_u(math_bits);
                context.push_conversion_value(Value::I32(result))
            },
            Self::I64TruncSatF32S => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i64.trunc_sat_f32_s operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i64_trunc_sat_f32_s(math_bits);
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncSatF32U => {
                let val = context.pop_conversion_value()?;
                let Value::F32(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F32 for i64.trunc_sat_f32_u operand",
                    ));
                };
                let math_bits = math::FloatBits32(float_bits.0);
                let result = math::i64_trunc_sat_f32_u(math_bits);
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncSatF64S => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i64.trunc_sat_f64_s operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i64_trunc_sat_f64_s(math_bits);
                context.push_conversion_value(Value::I64(result))
            },
            Self::I64TruncSatF64U => {
                let val = context.pop_conversion_value()?;
                let Value::F64(float_bits) = val else {
                    return Err(Error::type_error(
                        "Expected F64 for i64.trunc_sat_f64_u operand",
                    ));
                };
                let math_bits = math::FloatBits64(float_bits.0);
                let result = math::i64_trunc_sat_f64_u(math_bits);
                context.push_conversion_value(Value::I64(result))
            },
        }
    }
}

/// `I32WrapI64` conversion operation
pub struct I32WrapI64(pub Value);

/// `I64ExtendI32S` conversion operation
pub struct I64ExtendI32S(pub Value);

/// `I64ExtendI32U` conversion operation
pub struct I64ExtendI32U(pub Value);

/// `I64TruncF32S` conversion operation
pub struct I64TruncF32S(pub Value);

/// `I64TruncF32U` conversion operation
pub struct I64TruncF32U(pub Value);

/// `I64TruncF64S` conversion operation
pub struct I64TruncF64S(pub Value);

/// `I64TruncF64U` conversion operation
pub struct I64TruncF64U(pub Value);

/// `F32ConvertI32S` conversion operation
pub struct F32ConvertI32S(pub Value);

/// `F32ConvertI32U` conversion operation
pub struct F32ConvertI32U(pub Value);

/// `F32ConvertI64S` conversion operation
pub struct F32ConvertI64S(pub Value);

/// `F32ConvertI64U` conversion operation
pub struct F32ConvertI64U(pub Value);

/// `F32DemoteF64` conversion operation
pub struct F32DemoteF64(pub Value);

/// `F64ConvertI32S` conversion operation
pub struct F64ConvertI32S(pub Value);

/// `F64ConvertI32U` conversion operation
pub struct F64ConvertI32U(pub Value);

/// `F64ConvertI64S` conversion operation
pub struct F64ConvertI64S(pub Value);

/// `F64ConvertI64U` conversion operation
pub struct F64ConvertI64U(pub Value);

/// `F64PromoteF32` conversion operation
pub struct F64PromoteF32(pub Value);
