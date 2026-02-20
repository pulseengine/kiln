// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Arithmetic operations for WebAssembly instructions.
//!
//! This module provides pure implementations for WebAssembly arithmetic
//! instructions, including add, subtract, multiply, divide, and remainder
//! operations for various numeric types.

// WebAssembly value semantics require bit reinterpretation casts.
// i32/u32 and i64/u64 are the same bits - just different interpretations.
// These casts implement correct WASM behavior, not bugs.
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]

use wrt_math as math;

use crate::{
    prelude::{
        Debug,
        Error,
        FloatBits32,
        FloatBits64,
        PureInstruction,
        Result,
        Value,
        ValueType,
    },
    validation::{
        validate_arithmetic_op,
        Validate,
        ValidationContext,
    },
};

/// Represents a pure arithmetic operation for WebAssembly.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum ArithmeticOp {
    // Integer operations (i32)
    /// Add two 32-bit integers
    #[default]
    I32Add,
    /// Subtract one 32-bit integer from another
    I32Sub,
    /// Multiply two 32-bit integers
    I32Mul,
    /// Divide two 32-bit integers (signed)
    I32DivS,
    /// Divide two 32-bit integers (unsigned)
    I32DivU,
    /// Get remainder after dividing two 32-bit integers (signed)
    I32RemS,
    /// Get remainder after dividing two 32-bit integers (unsigned)
    I32RemU,
    /// Perform bitwise AND on two 32-bit integers
    I32And,
    /// Perform bitwise OR on two 32-bit integers
    I32Or,
    /// Perform bitwise XOR on two 32-bit integers
    I32Xor,
    /// Shift 32-bit integer left
    I32Shl,
    /// Shift 32-bit integer right (signed)
    I32ShrS,
    /// Shift 32-bit integer right (unsigned)
    I32ShrU,
    /// Rotate 32-bit integer left
    I32Rotl,
    /// Rotate 32-bit integer right
    I32Rotr,
    /// Count leading zeros in a 32-bit integer
    I32Clz,
    /// Count trailing zeros in a 32-bit integer
    I32Ctz,
    /// Count number of set bits in a 32-bit integer
    I32Popcnt,

    // Integer operations (i64)
    /// Add two 64-bit integers
    I64Add,
    /// Subtract one 64-bit integer from another
    I64Sub,
    /// Multiply two 64-bit integers
    I64Mul,
    /// Divide two 64-bit integers (signed)
    I64DivS,
    /// Divide two 64-bit integers (unsigned)
    I64DivU,
    /// Get remainder after dividing two 64-bit integers (signed)
    I64RemS,
    /// Get remainder after dividing two 64-bit integers (unsigned)
    I64RemU,
    /// Perform bitwise AND on two 64-bit integers
    I64And,
    /// Perform bitwise OR on two 64-bit integers
    I64Or,
    /// Perform bitwise XOR on two 64-bit integers
    I64Xor,
    /// Shift 64-bit integer left
    I64Shl,
    /// Shift 64-bit integer right (signed)
    I64ShrS,
    /// Shift 64-bit integer right (unsigned)
    I64ShrU,
    /// Rotate 64-bit integer left
    I64Rotl,
    /// Rotate 64-bit integer right
    I64Rotr,
    /// Count leading zeros in a 64-bit integer
    I64Clz,
    /// Count trailing zeros in a 64-bit integer
    I64Ctz,
    /// Count number of set bits in a 64-bit integer
    I64Popcnt,

    // Float operations (f32)
    /// Add two 32-bit float values
    F32Add,
    /// Subtract 32-bit float values
    F32Sub,
    /// Multiply 32-bit float values
    F32Mul,
    /// Divide 32-bit float values
    F32Div,
    /// Get the minimum of two 32-bit float values
    F32Min,
    /// Get the maximum of two 32-bit float values
    F32Max,
    /// Get the absolute value of a 32-bit float
    F32Abs,
    /// Negate a 32-bit float
    F32Neg,
    /// Round a 32-bit float up to the nearest integer
    F32Ceil,
    /// Round a 32-bit float down to the nearest integer
    F32Floor,
    /// Truncate a 32-bit float to an integer
    F32Trunc,
    /// Round a 32-bit float to the nearest integer
    F32Nearest,
    /// Calculate the square root of a 32-bit float
    F32Sqrt,
    /// Copy sign from one 32-bit float to another
    F32Copysign,

    // Float operations (f64)
    /// Add two 64-bit float values
    F64Add,
    /// Subtract 64-bit float values
    F64Sub,
    /// Multiply 64-bit float values
    F64Mul,
    /// Divide 64-bit float values
    F64Div,
    /// Get the minimum of two 64-bit float values
    F64Min,
    /// Get the maximum of two 64-bit float values
    F64Max,
    /// Get the absolute value of a 64-bit float
    F64Abs,
    /// Negate a 64-bit float
    F64Neg,
    /// Round a 64-bit float up to the nearest integer
    F64Ceil,
    /// Round a 64-bit float down to the nearest integer
    F64Floor,
    /// Truncate a 64-bit float to an integer
    F64Trunc,
    /// Round a 64-bit float to the nearest integer
    F64Nearest,
    /// Calculate the square root of a 64-bit float
    F64Sqrt,
    /// Copy sign from one 64-bit float to another
    F64Copysign,
}


/// Execution context for arithmetic operations
pub trait ArithmeticContext {
    /// Pop a value from the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is empty or value access fails
    fn pop_arithmetic_value(&mut self) -> Result<Value>;

    /// Push a value to the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is full or push operation fails
    fn push_arithmetic_value(&mut self, value: Value) -> Result<()>;
}

// Helper function to convert foundation FloatBits to math FloatBits and execute
fn execute_f32_unary<F>(context: &mut impl ArithmeticContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits32) -> Result<math::FloatBits32>,
{
    let val = context.pop_arithmetic_value()?;
    let Value::F32(float_bits) = val else {
        return Err(Error::invalid_type_error("Expected F32 operand"));
    };
    let math_bits = math::FloatBits32(float_bits.0);
    let result = f(math_bits)?;
    context.push_arithmetic_value(Value::F32(FloatBits32(result.0)))
}

fn execute_f32_binary<F>(context: &mut impl ArithmeticContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits32, math::FloatBits32) -> Result<math::FloatBits32>,
{
    let val_b = context.pop_arithmetic_value()?;
    let Value::F32(float_bits_b) = val_b else {
        return Err(Error::invalid_type_error("Expected F32 operand"));
    };
    let val_a = context.pop_arithmetic_value()?;
    let Value::F32(float_bits_a) = val_a else {
        return Err(Error::invalid_type_error("Expected F32 operand"));
    };
    let math_bits_a = math::FloatBits32(float_bits_a.0);
    let math_bits_b = math::FloatBits32(float_bits_b.0);
    let result = f(math_bits_a, math_bits_b)?;
    context.push_arithmetic_value(Value::F32(FloatBits32(result.0)))
}

fn execute_f64_unary<F>(context: &mut impl ArithmeticContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits64) -> Result<math::FloatBits64>,
{
    let val = context.pop_arithmetic_value()?;
    let Value::F64(float_bits) = val else {
        return Err(Error::invalid_type_error("Expected F64 operand"));
    };
    let math_bits = math::FloatBits64(float_bits.0);
    let result = f(math_bits)?;
    context.push_arithmetic_value(Value::F64(FloatBits64(result.0)))
}

fn execute_f64_binary<F>(context: &mut impl ArithmeticContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits64, math::FloatBits64) -> Result<math::FloatBits64>,
{
    let val_b = context.pop_arithmetic_value()?;
    let Value::F64(float_bits_b) = val_b else {
        return Err(Error::invalid_type_error("Expected F64 operand"));
    };
    let val_a = context.pop_arithmetic_value()?;
    let Value::F64(float_bits_a) = val_a else {
        return Err(Error::invalid_type_error("Expected F64 operand"));
    };
    let math_bits_a = math::FloatBits64(float_bits_a.0);
    let math_bits_b = math::FloatBits64(float_bits_b.0);
    let result = f(math_bits_a, math_bits_b)?;
    context.push_arithmetic_value(Value::F64(FloatBits64(result.0)))
}

impl<T: ArithmeticContext> PureInstruction<T, Error> for ArithmeticOp {
    // Large match is intentional - interpreter dispatch for all arithmetic opcodes.
    #[allow(clippy::too_many_lines)]
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            // Integer operations (i32)
            Self::I32Add => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.add operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.add operand"))?;
                let result = math::i32_add(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Sub => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.sub operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.sub operand"))?;
                let result = math::i32_sub(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Mul => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.mul operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.mul operand"))?;
                let result = math::i32_mul(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32DivS => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.div_s operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.div_s operand"))?;
                let result = math::i32_div_s(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32DivU => {
                let b = context.pop_arithmetic_value()?.as_u32().ok_or_else(|| {
                    Error::invalid_type_error("Expected I32 for i32.div_u operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_u32().ok_or_else(|| {
                    Error::invalid_type_error("Expected I32 for i32.div_u operand")
                })?;
                let result = math::i32_div_u(a, b)?;
                context.push_arithmetic_value(Value::I32(result as i32))
            },
            Self::I32RemS => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rem_s operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rem_s operand"))?;
                let result = math::i32_rem_s(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32RemU => {
                let b = context.pop_arithmetic_value()?.as_u32().ok_or_else(|| {
                    Error::invalid_type_error("Expected I32 for i32.rem_u operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_u32().ok_or_else(|| {
                    Error::invalid_type_error("Expected I32 for i32.rem_u operand")
                })?;
                let result = math::i32_rem_u(a, b)?;
                context.push_arithmetic_value(Value::I32(result as i32))
            },
            Self::I32And => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.and operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.and operand"))?;
                let result = math::i32_and(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Or => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.or operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.or operand"))?;
                let result = math::i32_or(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Xor => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.xor operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.xor operand"))?;
                let result = math::i32_xor(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Shl => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shl operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shl operand"))?;
                let result = math::i32_shl(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32ShrS => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shr_s operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shr_s operand"))?;
                let result = math::i32_shr_s(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32ShrU => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shr_u operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.shr_u operand"))?;
                let result = math::i32_shr_u(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Rotl => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rotl operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rotl operand"))?;
                let result = math::i32_rotl(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Rotr => {
                let b = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rotr operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.rotr operand"))?;
                let result = math::i32_rotr(a, b)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Clz => {
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.clz operand"))?;
                let result = math::i32_clz(a)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Ctz => {
                let a = context
                    .pop_arithmetic_value()?
                    .into_i32()
                    .map_err(|_| Error::invalid_type_error("Expected I32 for i32.ctz operand"))?;
                let result = math::i32_ctz(a)?;
                context.push_arithmetic_value(Value::I32(result))
            },
            Self::I32Popcnt => {
                let a = context.pop_arithmetic_value()?.into_i32().map_err(|_| {
                    Error::invalid_type_error("Expected I32 for i32.popcnt operand")
                })?;
                let result = math::i32_popcnt(a)?;
                context.push_arithmetic_value(Value::I32(result))
            },

            // Integer operations (i64)
            Self::I64Add => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.add operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.add operand"))?;
                let result = math::i64_add(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Sub => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.sub operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.sub operand"))?;
                let result = math::i64_sub(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Mul => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.mul operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.mul operand"))?;
                let result = math::i64_mul(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64DivS => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.div_s operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.div_s operand")
                })?;
                let result = math::i64_div_s(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64DivU => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.div_u operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.div_u operand")
                })?;
                let result = math::i64_div_u(a as u64, b as u64)?;
                context.push_arithmetic_value(Value::I64(result as i64))
            },
            Self::I64RemS => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rem_s operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rem_s operand")
                })?;
                let result = math::i64_rem_s(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64RemU => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rem_u operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rem_u operand")
                })?;
                let result = math::i64_rem_u(a as u64, b as u64)?;
                context.push_arithmetic_value(Value::I64(result as i64))
            },
            Self::I64And => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.and operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.and operand"))?;
                let result = math::i64_and(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Or => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.or operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.or operand"))?;
                let result = math::i64_or(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Xor => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.xor operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.xor operand"))?;
                let result = math::i64_xor(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Shl => {
                let b = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.shl operand"))?;
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.shl operand"))?;
                let result = math::i64_shl(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64ShrS => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.shr_s operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.shr_s operand")
                })?;
                let result = math::i64_shr_s(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64ShrU => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.shr_u operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.shr_u operand")
                })?;
                let result = math::i64_shr_u(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Rotl => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rotl operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rotl operand")
                })?;
                let result = math::i64_rotl(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Rotr => {
                let b = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rotr operand")
                })?;
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.rotr operand")
                })?;
                let result = math::i64_rotr(a, b)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Clz => {
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.clz operand"))?;
                let result = math::i64_clz(a)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Ctz => {
                let a = context
                    .pop_arithmetic_value()?
                    .as_i64()
                    .ok_or_else(|| Error::invalid_type_error("Expected I64 for i64.ctz operand"))?;
                let result = math::i64_ctz(a)?;
                context.push_arithmetic_value(Value::I64(result))
            },
            Self::I64Popcnt => {
                let a = context.pop_arithmetic_value()?.as_i64().ok_or_else(|| {
                    Error::invalid_type_error("Expected I64 for i64.popcnt operand")
                })?;
                let result = math::i64_popcnt(a)?;
                context.push_arithmetic_value(Value::I64(result))
            },

            // Float operations (f32)
            Self::F32Add => execute_f32_binary(context, math::f32_add),
            Self::F32Sub => execute_f32_binary(context, math::f32_sub),
            Self::F32Mul => execute_f32_binary(context, math::f32_mul),
            Self::F32Div => execute_f32_binary(context, math::f32_div),
            Self::F32Min => execute_f32_binary(context, math::wasm_f32_min),
            Self::F32Max => execute_f32_binary(context, math::wasm_f32_max),
            Self::F32Copysign => execute_f32_binary(context, math::wasm_f32_copysign),
            Self::F32Abs => execute_f32_unary(context, math::wasm_f32_abs),
            Self::F32Neg => execute_f32_unary(context, math::wasm_f32_neg),
            Self::F32Ceil => execute_f32_unary(context, math::wasm_f32_ceil),
            Self::F32Floor => execute_f32_unary(context, math::wasm_f32_floor),
            Self::F32Trunc => execute_f32_unary(context, math::wasm_f32_trunc),
            Self::F32Nearest => execute_f32_unary(context, math::wasm_f32_nearest),
            Self::F32Sqrt => execute_f32_unary(context, math::wasm_f32_sqrt),

            // Float operations (f64)
            Self::F64Add => execute_f64_binary(context, math::f64_add),
            Self::F64Sub => execute_f64_binary(context, math::f64_sub),
            Self::F64Mul => execute_f64_binary(context, math::f64_mul),
            Self::F64Div => execute_f64_binary(context, math::f64_div),
            Self::F64Min => execute_f64_binary(context, math::wasm_f64_min),
            Self::F64Max => execute_f64_binary(context, math::wasm_f64_max),
            Self::F64Copysign => execute_f64_binary(context, math::wasm_f64_copysign),
            Self::F64Abs => execute_f64_unary(context, math::wasm_f64_abs),
            Self::F64Neg => execute_f64_unary(context, math::wasm_f64_neg),
            Self::F64Ceil => execute_f64_unary(context, math::wasm_f64_ceil),
            Self::F64Floor => execute_f64_unary(context, math::wasm_f64_floor),
            Self::F64Trunc => execute_f64_unary(context, math::wasm_f64_trunc),
            Self::F64Nearest => execute_f64_unary(context, math::wasm_f64_nearest),
            Self::F64Sqrt => execute_f64_unary(context, math::wasm_f64_sqrt),
        }
    }
}

impl Validate for ArithmeticOp {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            // I32 operations
            Self::I32Add
            | Self::I32Sub
            | Self::I32Mul
            | Self::I32DivS
            | Self::I32DivU
            | Self::I32RemS
            | Self::I32RemU
            | Self::I32And
            | Self::I32Or
            | Self::I32Xor
            | Self::I32Shl
            | Self::I32ShrS
            | Self::I32ShrU
            | Self::I32Rotl
            | Self::I32Rotr => validate_arithmetic_op(
                "i32 binary op",
                &[ValueType::I32, ValueType::I32],
                ValueType::I32,
                ctx,
            ),

            Self::I32Clz | Self::I32Ctz | Self::I32Popcnt => {
                validate_arithmetic_op("i32 unary op", &[ValueType::I32], ValueType::I32, ctx)
            },

            // I64 operations
            Self::I64Add
            | Self::I64Sub
            | Self::I64Mul
            | Self::I64DivS
            | Self::I64DivU
            | Self::I64RemS
            | Self::I64RemU
            | Self::I64And
            | Self::I64Or
            | Self::I64Xor
            | Self::I64Shl
            | Self::I64ShrS
            | Self::I64ShrU
            | Self::I64Rotl
            | Self::I64Rotr => validate_arithmetic_op(
                "i64 binary op",
                &[ValueType::I64, ValueType::I64],
                ValueType::I64,
                ctx,
            ),

            Self::I64Clz | Self::I64Ctz | Self::I64Popcnt => {
                validate_arithmetic_op("i64 unary op", &[ValueType::I64], ValueType::I64, ctx)
            },

            // F32 operations
            Self::F32Add
            | Self::F32Sub
            | Self::F32Mul
            | Self::F32Div
            | Self::F32Min
            | Self::F32Max
            | Self::F32Copysign => validate_arithmetic_op(
                "f32 binary op",
                &[ValueType::F32, ValueType::F32],
                ValueType::F32,
                ctx,
            ),

            Self::F32Abs
            | Self::F32Neg
            | Self::F32Ceil
            | Self::F32Floor
            | Self::F32Trunc
            | Self::F32Nearest
            | Self::F32Sqrt => {
                validate_arithmetic_op("f32 unary op", &[ValueType::F32], ValueType::F32, ctx)
            },

            // F64 operations
            Self::F64Add
            | Self::F64Sub
            | Self::F64Mul
            | Self::F64Div
            | Self::F64Min
            | Self::F64Max
            | Self::F64Copysign => validate_arithmetic_op(
                "f64 binary op",
                &[ValueType::F64, ValueType::F64],
                ValueType::F64,
                ctx,
            ),

            Self::F64Abs
            | Self::F64Neg
            | Self::F64Ceil
            | Self::F64Floor
            | Self::F64Trunc
            | Self::F64Nearest
            | Self::F64Sqrt => {
                validate_arithmetic_op("f64 unary op", &[ValueType::F64], ValueType::F64, ctx)
            },
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    // Mock context for testing arithmetic operations
    struct MockArithmeticContext {
        stack: Vec<Value>,
    }

    impl MockArithmeticContext {
        fn new() -> Self {
            Self { stack: Vec::new() }
        }
    }

    impl ArithmeticContext for MockArithmeticContext {
        fn push_arithmetic_value(&mut self, value: Value) -> Result<()> {
            self.stack.push(value);
            Ok(())
        }

        fn pop_arithmetic_value(&mut self) -> Result<Value> {
            self.stack
                .pop()
                .ok_or_else(|| Error::runtime_stack_underflow("Stack underflow"))
        }
    }

    #[test]
    fn test_i32_arithmetic() {
        // Create a simple test context
        let mut context = MockArithmeticContext::new();

        // Test i32.add
        context.push_arithmetic_value(Value::I32(2)).unwrap();
        context.push_arithmetic_value(Value::I32(3)).unwrap();
        ArithmeticOp::I32Add.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(5));

        // Test i32.sub
        context.push_arithmetic_value(Value::I32(10)).unwrap();
        context.push_arithmetic_value(Value::I32(4)).unwrap();
        ArithmeticOp::I32Sub.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(6));

        // Test i32.mul
        context.push_arithmetic_value(Value::I32(3)).unwrap();
        context.push_arithmetic_value(Value::I32(4)).unwrap();
        ArithmeticOp::I32Mul.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(12));

        // Test i32.div_s
        context.push_arithmetic_value(Value::I32(10)).unwrap();
        context.push_arithmetic_value(Value::I32(3)).unwrap();
        ArithmeticOp::I32DivS.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(3));

        // Test i32.div_u
        context.push_arithmetic_value(Value::I32(10)).unwrap();
        context.push_arithmetic_value(Value::I32(3)).unwrap();
        ArithmeticOp::I32DivU.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(3));
    }

    #[test]
    fn test_i32_bitwise() {
        // Create a simple test context
        let mut context = MockArithmeticContext::new();

        // Test i32.and
        context.push_arithmetic_value(Value::I32(0b1010)).unwrap();
        context.push_arithmetic_value(Value::I32(0b1100)).unwrap();
        ArithmeticOp::I32And.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(0b1000));

        // Test i32.or
        context.push_arithmetic_value(Value::I32(0b1010)).unwrap();
        context.push_arithmetic_value(Value::I32(0b1100)).unwrap();
        ArithmeticOp::I32Or.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(0b1110));

        // Test i32.xor
        context.push_arithmetic_value(Value::I32(0b1010)).unwrap();
        context.push_arithmetic_value(Value::I32(0b1100)).unwrap();
        ArithmeticOp::I32Xor.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(0b0110));
    }

    #[test]
    fn test_i32_shift_rotate() {
        // Create a simple test context
        let mut context = MockArithmeticContext::new();

        // Test i32.shl
        context.push_arithmetic_value(Value::I32(1)).unwrap();
        context.push_arithmetic_value(Value::I32(3)).unwrap();
        ArithmeticOp::I32Shl.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(8));

        // Test i32.shr_s (signed)
        context.push_arithmetic_value(Value::I32(-8)).unwrap();
        context.push_arithmetic_value(Value::I32(2)).unwrap();
        ArithmeticOp::I32ShrS.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(-2));

        // Test i32.shr_u (unsigned)
        context.push_arithmetic_value(Value::I32(8)).unwrap();
        context.push_arithmetic_value(Value::I32(2)).unwrap();
        ArithmeticOp::I32ShrU.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(2));

        // Test i32.rotl
        context
            .push_arithmetic_value(Value::I32(0b10110000_00000000_00000000_00000001u32 as i32))
            .unwrap();
        context.push_arithmetic_value(Value::I32(1)).unwrap();
        ArithmeticOp::I32Rotl.execute(&mut context).unwrap();
        assert_eq!(
            context.pop_arithmetic_value().unwrap(),
            Value::I32(0b01100000_00000000_00000000_00000011)
        );

        // Test i32.rotr
        context
            .push_arithmetic_value(Value::I32(0b10110000_00000000_00000000_00000001u32 as i32))
            .unwrap();
        context.push_arithmetic_value(Value::I32(1)).unwrap();
        ArithmeticOp::I32Rotr.execute(&mut context).unwrap();
        assert_eq!(
            context.pop_arithmetic_value().unwrap(),
            Value::I32(0b11011000_00000000_00000000_00000000u32 as i32)
        );
    }

    #[test]
    fn test_i32_count_operations() {
        let mut context = MockArithmeticContext::new();

        // Test i32.clz (count leading zeros)
        context
            .push_arithmetic_value(Value::I32(0b00000000_00000000_00000000_00001000))
            .unwrap();
        ArithmeticOp::I32Clz.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(28));

        // Test i32.ctz (count trailing zeros)
        context
            .push_arithmetic_value(Value::I32(0b00001000_00000000_00000000_00000000))
            .unwrap();
        ArithmeticOp::I32Ctz.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(27));

        // Test i32.popcnt (population count)
        context
            .push_arithmetic_value(Value::I32(0b01010101_01010101_01010101_01010101))
            .unwrap();
        ArithmeticOp::I32Popcnt.execute(&mut context).unwrap();
        assert_eq!(context.pop_arithmetic_value().unwrap(), Value::I32(16));
    }

    #[test]
    fn test_f32_arithmetic() {
        let mut context = MockArithmeticContext::new();

        // Test f32.add
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.5))).unwrap();
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(3.75)))
            .unwrap();
        ArithmeticOp::F32Add.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 6.25);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.sub
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(10.0)))
            .unwrap();
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.5))).unwrap();
        ArithmeticOp::F32Sub.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 6.5);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.mul
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.5))).unwrap();
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(4.0))).unwrap();
        ArithmeticOp::F32Mul.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 10.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.div
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(10.0)))
            .unwrap();
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.5))).unwrap();
        ArithmeticOp::F32Div.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 4.0);
        } else {
            panic!("Expected F32 value");
        }
    }

    #[test]
    fn test_f32_math_operations() {
        let mut context = MockArithmeticContext::new();

        // Test f32.abs
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(-5.5)))
            .unwrap();
        ArithmeticOp::F32Abs.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 5.5);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.neg
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(3.14)))
            .unwrap();
        ArithmeticOp::F32Neg.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), -3.14);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.sqrt
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(16.0)))
            .unwrap();
        ArithmeticOp::F32Sqrt.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 4.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.ceil
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.3))).unwrap();
        ArithmeticOp::F32Ceil.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 3.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.floor
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.7))).unwrap();
        ArithmeticOp::F32Floor.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 2.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.trunc
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(-2.7)))
            .unwrap();
        ArithmeticOp::F32Trunc.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), -2.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.nearest
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.5))).unwrap();
        ArithmeticOp::F32Nearest.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 2.0); // Even rounding
        } else {
            panic!("Expected F32 value");
        }
    }

    #[test]
    fn test_f32_minmax() {
        let mut context = MockArithmeticContext::new();

        // Test f32.min
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.0))).unwrap();
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.0))).unwrap();
        ArithmeticOp::F32Min.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 2.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.max
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.0))).unwrap();
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.0))).unwrap();
        ArithmeticOp::F32Max.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), 3.0);
        } else {
            panic!("Expected F32 value");
        }

        // Test f32.copysign
        context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.0))).unwrap();
        context
            .push_arithmetic_value(Value::F32(FloatBits32::from_float(-1.0)))
            .unwrap();
        ArithmeticOp::F32Copysign.execute(&mut context).unwrap();
        let result = context.pop_arithmetic_value().unwrap();
        if let Value::F32(bits) = result {
            assert_eq!(bits.value(), -3.0);
        } else {
            panic!("Expected F32 value");
        }
    }
}
