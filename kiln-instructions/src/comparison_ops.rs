// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Comparison operations for WebAssembly instructions.
//!
//! This module provides pure implementations for WebAssembly comparison
//! instructions, including equality, inequality, and relational operations for
//! various numeric types.

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
        PureInstruction,
        Result,
        Value,
        ValueType,
    },
    validation::{
        Validate,
        ValidationContext,
    },
};

/// Represents a pure comparison operation for WebAssembly.
#[derive(Debug, Clone)]
pub enum ComparisonOp {
    // Integer equality operations (i32)
    /// Equals comparison for i32 values
    I32Eq,
    /// Not equals comparison for i32 values
    I32Ne,
    /// Less than comparison for i32 values (signed)
    I32LtS,
    /// Less than comparison for i32 values (unsigned)
    I32LtU,
    /// Greater than comparison for i32 values (signed)
    I32GtS,
    /// Greater than comparison for i32 values (unsigned)
    I32GtU,
    /// Less than or equal comparison for i32 values (signed)
    I32LeS,
    /// Less than or equal comparison for i32 values (unsigned)
    I32LeU,
    /// Greater than or equal comparison for i32 values (signed)
    I32GeS,
    /// Greater than or equal comparison for i32 values (unsigned)
    I32GeU,

    // Integer equality operations (i64)
    /// Equals comparison for i64 values
    I64Eq,
    /// Not equals comparison for i64 values
    I64Ne,
    /// Less than comparison for i64 values (signed)
    I64LtS,
    /// Less than comparison for i64 values (unsigned)
    I64LtU,
    /// Greater than comparison for i64 values (signed)
    I64GtS,
    /// Greater than comparison for i64 values (unsigned)
    I64GtU,
    /// Less than or equal comparison for i64 values (signed)
    I64LeS,
    /// Less than or equal comparison for i64 values (unsigned)
    I64LeU,
    /// Greater than or equal comparison for i64 values (signed)
    I64GeS,
    /// Greater than or equal comparison for i64 values (unsigned)
    I64GeU,

    // Float comparison operations (f32)
    /// Equals comparison for f32 values
    F32Eq,
    /// Not equals comparison for f32 values
    F32Ne,
    /// Less than comparison for f32 values
    F32Lt,
    /// Greater than comparison for f32 values
    F32Gt,
    /// Less than or equal comparison for f32 values
    F32Le,
    /// Greater than or equal comparison for f32 values
    F32Ge,

    // Float comparison operations (f64)
    /// Equals comparison for f64 values
    F64Eq,
    /// Not equals comparison for f64 values
    F64Ne,
    /// Less than comparison for f64 values
    F64Lt,
    /// Greater than comparison for f64 values
    F64Gt,
    /// Less than or equal comparison for f64 values
    F64Le,
    /// Greater than or equal comparison for f64 values
    F64Ge,

    // Test operations
    /// Test if i32 value equals zero
    I32Eqz,
    /// Test if i64 value equals zero
    I64Eqz,
}

/// Execution context for comparison operations
pub trait ComparisonContext {
    /// Pop a value from the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is empty or value type is invalid
    fn pop_comparison_value(&mut self) -> Result<Value>;

    /// Push a value to the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is full or value cannot be pushed
    fn push_comparison_value(&mut self, value: Value) -> Result<()>;
}

// Helper function to execute f32 comparison operations
fn execute_f32_comparison<F>(context: &mut impl ComparisonContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits32, math::FloatBits32) -> Result<i32>,
{
    let val_b = context.pop_comparison_value()?;
    let Value::F32(float_bits_b) = val_b else {
        return Err(Error::type_error("Expected F32 operand"));
    };
    let val_a = context.pop_comparison_value()?;
    let Value::F32(float_bits_a) = val_a else {
        return Err(Error::type_error("Expected F32 operand"));
    };
    let math_bits_a = math::FloatBits32(float_bits_a.0);
    let math_bits_b = math::FloatBits32(float_bits_b.0);
    let result = f(math_bits_a, math_bits_b)?;
    context.push_comparison_value(Value::I32(result))
}

// Helper function to execute f64 comparison operations
fn execute_f64_comparison<F>(context: &mut impl ComparisonContext, f: F) -> Result<()>
where
    F: FnOnce(math::FloatBits64, math::FloatBits64) -> Result<i32>,
{
    let val_b = context.pop_comparison_value()?;
    let Value::F64(float_bits_b) = val_b else {
        return Err(Error::type_error("Expected F64 operand"));
    };
    let val_a = context.pop_comparison_value()?;
    let Value::F64(float_bits_a) = val_a else {
        return Err(Error::type_error("Expected F64 operand"));
    };
    let math_bits_a = math::FloatBits64(float_bits_a.0);
    let math_bits_b = math::FloatBits64(float_bits_b.0);
    let result = f(math_bits_a, math_bits_b)?;
    context.push_comparison_value(Value::I32(result))
}

impl<T: ComparisonContext> PureInstruction<T, Error> for ComparisonOp {
    // Large match is intentional - interpreter dispatch for all comparison opcodes.
    #[allow(clippy::too_many_lines)]
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            // i32 comparison operations
            Self::I32Eq => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.eq operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.eq operand"))?;
                let result = math::i32_eq(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32Ne => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.ne operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.ne operand"))?;
                let result = math::i32_ne(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32LtS => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.lt_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.lt_s operand"))?;
                let result = math::i32_lt_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32LtU => {
                let b = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.lt_u operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.lt_u operand"))?;
                let result = math::i32_lt_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32GtS => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.gt_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.gt_s operand"))?;
                let result = math::i32_gt_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32GtU => {
                let b = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.gt_u operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.gt_u operand"))?;
                let result = math::i32_gt_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32LeS => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.le_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.le_s operand"))?;
                let result = math::i32_le_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32LeU => {
                let b = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.le_u operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.le_u operand"))?;
                let result = math::i32_le_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32GeS => {
                let b = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.ge_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.ge_s operand"))?;
                let result = math::i32_ge_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I32GeU => {
                let b = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.ge_u operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_u32()
                    .ok_or_else(|| Error::type_error("Expected I32 for i32.ge_u operand"))?;
                let result = math::i32_ge_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },

            // i64 comparison operations
            Self::I64Eq => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.eq operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.eq operand"))?;
                let result = math::i64_eq(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64Ne => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ne operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ne operand"))?;
                let result = math::i64_ne(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64LtS => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.lt_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.lt_s operand"))?;
                let result = math::i64_lt_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64LtU => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.lt_u operand"))?
                    as u64;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.lt_u operand"))?
                    as u64;
                let result = math::i64_lt_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64GtS => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.gt_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.gt_s operand"))?;
                let result = math::i64_gt_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64GtU => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.gt_u operand"))?
                    as u64;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.gt_u operand"))?
                    as u64;
                let result = math::i64_gt_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64LeS => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.le_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.le_s operand"))?;
                let result = math::i64_le_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64LeU => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.le_u operand"))?
                    as u64;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.le_u operand"))?
                    as u64;
                let result = math::i64_le_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64GeS => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ge_s operand"))?;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ge_s operand"))?;
                let result = math::i64_ge_s(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64GeU => {
                let b = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ge_u operand"))?
                    as u64;
                let a = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.ge_u operand"))?
                    as u64;
                let result = math::i64_ge_u(a, b)?;
                context.push_comparison_value(Value::I32(result))
            },

            // f32 comparison operations
            Self::F32Eq => execute_f32_comparison(context, math::f32_eq),
            Self::F32Ne => execute_f32_comparison(context, math::f32_ne),
            Self::F32Lt => execute_f32_comparison(context, math::f32_lt),
            Self::F32Gt => execute_f32_comparison(context, math::f32_gt),
            Self::F32Le => execute_f32_comparison(context, math::f32_le),
            Self::F32Ge => execute_f32_comparison(context, math::f32_ge),

            // f64 comparison operations
            Self::F64Eq => execute_f64_comparison(context, math::f64_eq),
            Self::F64Ne => execute_f64_comparison(context, math::f64_ne),
            Self::F64Lt => execute_f64_comparison(context, math::f64_lt),
            Self::F64Gt => execute_f64_comparison(context, math::f64_gt),
            Self::F64Le => execute_f64_comparison(context, math::f64_le),
            Self::F64Ge => execute_f64_comparison(context, math::f64_ge),

            // Test operations
            Self::I32Eqz => {
                let val = context
                    .pop_comparison_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Expected I32 for i32.eqz"))?;
                let result = math::i32_eqz(val)?;
                context.push_comparison_value(Value::I32(result))
            },
            Self::I64Eqz => {
                let val = context
                    .pop_comparison_value()?
                    .as_i64()
                    .ok_or_else(|| Error::type_error("Expected I64 for i64.eqz"))?;
                let result = math::i64_eqz(val)?;
                context.push_comparison_value(Value::I32(result))
            },
        }
    }
}

impl Validate for ComparisonOp {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            // I32 binary comparison operations
            Self::I32Eq
            | Self::I32Ne
            | Self::I32LtS
            | Self::I32LtU
            | Self::I32GtS
            | Self::I32GtU
            | Self::I32LeS
            | Self::I32LeU
            | Self::I32GeS
            | Self::I32GeU => {
                // Pop two i32 operands and push one i32 result
                ctx.pop_types(&[ValueType::I32, ValueType::I32])?;
                ctx.push_type(ValueType::I32)
            },

            // I64 binary comparison operations
            Self::I64Eq
            | Self::I64Ne
            | Self::I64LtS
            | Self::I64LtU
            | Self::I64GtS
            | Self::I64GtU
            | Self::I64LeS
            | Self::I64LeU
            | Self::I64GeS
            | Self::I64GeU => {
                // Pop two i64 operands and push one i32 result
                ctx.pop_types(&[ValueType::I64, ValueType::I64])?;
                ctx.push_type(ValueType::I32)
            },

            // F32 binary comparison operations
            Self::F32Eq | Self::F32Ne | Self::F32Lt | Self::F32Gt | Self::F32Le | Self::F32Ge => {
                // Pop two f32 operands and push one i32 result
                ctx.pop_types(&[ValueType::F32, ValueType::F32])?;
                ctx.push_type(ValueType::I32)
            },

            // F64 binary comparison operations
            Self::F64Eq | Self::F64Ne | Self::F64Lt | Self::F64Gt | Self::F64Le | Self::F64Ge => {
                // Pop two f64 operands and push one i32 result
                ctx.pop_types(&[ValueType::F64, ValueType::F64])?;
                ctx.push_type(ValueType::I32)
            },

            // Unary test operations
            Self::I32Eqz => {
                // Pop one i32 operand and push one i32 result
                ctx.pop_expect(ValueType::I32)?;
                ctx.push_type(ValueType::I32)
            },
            Self::I64Eqz => {
                // Pop one i64 operand and push one i32 result
                ctx.pop_expect(ValueType::I64)?;
                ctx.push_type(ValueType::I32)
            },
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::prelude::{FloatBits32, FloatBits64};

    // Mock context for testing comparison operations
    struct MockComparisonContext {
        stack: Vec<Value>,
    }

    impl MockComparisonContext {
        fn new() -> Self {
            Self { stack: Vec::new() }
        }
    }

    impl ComparisonContext for MockComparisonContext {
        fn push_comparison_value(&mut self, value: Value) -> Result<()> {
            self.stack.push(value);
            Ok(())
        }

        fn pop_comparison_value(&mut self) -> Result<Value> {
            self.stack
                .pop()
                .ok_or_else(|| Error::runtime_stack_underflow("Stack underflow"))
        }
    }

    #[test]
    fn test_i32_equality() {
        let mut context = MockComparisonContext::new();

        // Test i32.eq (equal)
        context.push_comparison_value(Value::I32(5)).unwrap();
        context.push_comparison_value(Value::I32(5)).unwrap();
        ComparisonOp::I32Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.eq (not equal)
        context.push_comparison_value(Value::I32(5)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test i32.ne (not equal)
        context.push_comparison_value(Value::I32(5)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.ne (equal)
        context.push_comparison_value(Value::I32(5)).unwrap();
        context.push_comparison_value(Value::I32(5)).unwrap();
        ComparisonOp::I32Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));
    }

    #[test]
    fn test_i32_relational_signed() {
        let mut context = MockComparisonContext::new();

        // Test i32.lt_s (less than, signed)
        context.push_comparison_value(Value::I32(-5)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32LtS.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.gt_s (greater than, signed)
        context.push_comparison_value(Value::I32(10)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32GtS.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.le_s (less than or equal, signed)
        context.push_comparison_value(Value::I32(7)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32LeS.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.ge_s (greater than or equal, signed)
        context.push_comparison_value(Value::I32(7)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32GeS.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_i32_relational_unsigned() {
        let mut context = MockComparisonContext::new();

        // Test i32.lt_u (less than, unsigned)
        // Note: -1 as unsigned is 0xFFFFFFFF, which is larger than 7
        context.push_comparison_value(Value::I32(-1)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32LtU.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test i32.gt_u (greater than, unsigned)
        context.push_comparison_value(Value::I32(-1)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32GtU.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test with positive numbers
        context.push_comparison_value(Value::I32(5)).unwrap();
        context.push_comparison_value(Value::I32(7)).unwrap();
        ComparisonOp::I32LtU.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_i64_comparisons() {
        let mut context = MockComparisonContext::new();

        // Test i64.eq (equal)
        context.push_comparison_value(Value::I64(0x123456789ABCDEF0)).unwrap();
        context.push_comparison_value(Value::I64(0x123456789ABCDEF0)).unwrap();
        ComparisonOp::I64Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i64.ne (not equal)
        context.push_comparison_value(Value::I64(5)).unwrap();
        context.push_comparison_value(Value::I64(7)).unwrap();
        ComparisonOp::I64Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i64.lt_s (less than, signed)
        context.push_comparison_value(Value::I64(-1000)).unwrap();
        context.push_comparison_value(Value::I64(1000)).unwrap();
        ComparisonOp::I64LtS.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i64.gt_u (greater than, unsigned)
        context.push_comparison_value(Value::I64(-1)).unwrap(); // Large unsigned value
        context.push_comparison_value(Value::I64(1000)).unwrap();
        ComparisonOp::I64GtU.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_f32_comparisons() {
        let mut context = MockComparisonContext::new();

        // Test f32.eq (equal)
        context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0))).unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0))).unwrap();
        ComparisonOp::F32Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32.ne (not equal)
        context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0))).unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        ComparisonOp::F32Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32.lt (less than)
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(3.14)))
            .unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        ComparisonOp::F32Lt.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32.gt (greater than)
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(10.0)))
            .unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        ComparisonOp::F32Gt.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32.le (less than or equal)
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        ComparisonOp::F32Le.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32.ge (greater than or equal)
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(7.0))).unwrap();
        ComparisonOp::F32Ge.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_f64_comparisons() {
        let mut context = MockComparisonContext::new();

        // Test f64.eq (equal)
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(3.141592653589793)))
            .unwrap();
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(3.141592653589793)))
            .unwrap();
        ComparisonOp::F64Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f64.lt (less than)
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(2.718281828459045)))
            .unwrap();
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(3.141592653589793)))
            .unwrap();
        ComparisonOp::F64Lt.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_eqz_operations() {
        let mut context = MockComparisonContext::new();

        // Test i32.eqz with zero
        context.push_comparison_value(Value::I32(0)).unwrap();
        ComparisonOp::I32Eqz.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i32.eqz with non-zero
        context.push_comparison_value(Value::I32(42)).unwrap();
        ComparisonOp::I32Eqz.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test i64.eqz with zero
        context.push_comparison_value(Value::I64(0)).unwrap();
        ComparisonOp::I64Eqz.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test i64.eqz with non-zero
        context.push_comparison_value(Value::I64(-100)).unwrap();
        ComparisonOp::I64Eqz.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));
    }

    #[test]
    fn test_nan_handling() {
        let mut context = MockComparisonContext::new();

        // Test f32 NaN equality (should be false)
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))
            .unwrap();
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))
            .unwrap();
        ComparisonOp::F32Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test f32 NaN inequality (should be true)
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))
            .unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0))).unwrap();
        ComparisonOp::F32Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));

        // Test f32 NaN less than (should be false)
        context
            .push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))
            .unwrap();
        context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0))).unwrap();
        ComparisonOp::F32Lt.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test f64 NaN equality (should be false)
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(f64::NAN)))
            .unwrap();
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(f64::NAN)))
            .unwrap();
        ComparisonOp::F64Eq.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(0));

        // Test f64 NaN inequality (should be true)
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(f64::NAN)))
            .unwrap();
        context
            .push_comparison_value(Value::F64(FloatBits64::from_float(42.0)))
            .unwrap();
        ComparisonOp::F64Ne.execute(&mut context).unwrap();
        assert_eq!(context.pop_comparison_value().unwrap(), Value::I32(1));
    }
}
