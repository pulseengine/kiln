// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Parametric operations for WebAssembly instructions.
//!
//! This module provides implementations for WebAssembly parametric instructions
//! including drop, select, and typed select operations.

use crate::prelude::{
    Debug,
    Error,
    PartialEq,
    PureInstruction,
    Result,
    Value,
    ValueType,
};

/// Represents a parametric operation for WebAssembly.
#[derive(Debug, Clone, PartialEq)]
pub enum ParametricOp {
    /// Drop a value from the stack
    Drop,
    /// Select between two values based on a condition
    /// If condition is non-zero, selects first value, otherwise second
    Select,
    /// Typed select with explicit value types
    /// Similar to Select but with type annotations for validation
    SelectTyped(ValueType),
}

/// Execution context for parametric operations
pub trait ParametricContext {
    /// Push a value to the stack
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is full
    fn push_value(&mut self, value: Value) -> Result<()>;

    /// Pop a value from the stack
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is empty
    fn pop_value(&mut self) -> Result<Value>;

    /// Peek at the top value without popping
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is empty
    fn peek_value(&self) -> Result<&Value>;
}

impl<T: ParametricContext> PureInstruction<T, Error> for ParametricOp {
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            Self::Drop => {
                // Pop and discard the top value
                context.pop_value()?;
                Ok(())
            },
            Self::Select => {
                // Pop condition
                let condition = context
                    .pop_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Select condition must be i32"))?;

                // Pop val2 (second option)
                let val2 = context.pop_value()?;

                // Pop val1 (first option)
                let val1 = context.pop_value()?;

                // Type check - both values must have the same type
                if core::mem::discriminant(&val1) != core::mem::discriminant(&val2) {
                    return Err(Error::type_error("Select operands must have the same type"));
                }

                // Push selected value
                context.push_value(if condition != 0 { val1 } else { val2 })
            },
            Self::SelectTyped(expected_type) => {
                // Pop condition
                let condition = context
                    .pop_value()?
                    .into_i32()
                    .map_err(|_| Error::type_error("Select condition must be i32"))?;

                // Pop val2 (second option)
                let val2 = context.pop_value()?;

                // Pop val1 (first option)
                let val1 = context.pop_value()?;

                // Type check against expected type
                let val1_type = val1.value_type();
                let val2_type = val2.value_type();

                if val1_type != *expected_type || val2_type != *expected_type {
                    return Err(Error::type_error(
                        "Select operands must match expected type",
                    ));
                }

                // Push selected value
                context.push_value(if condition != 0 { val1 } else { val2 })
            },
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    // Import Vec based on feature flags
    use std::vec::Vec;

    use super::*;
    use crate::prelude::{FloatBits32, FloatBits64};

    // Mock context for testing
    struct MockParametricContext {
        stack: Vec<Value>,
    }

    impl MockParametricContext {
        fn new() -> Self {
            Self { stack: Vec::new() }
        }
    }

    impl ParametricContext for MockParametricContext {
        fn push_value(&mut self, value: Value) -> Result<()> {
            self.stack.push(value);
            Ok(())
        }

        fn pop_value(&mut self) -> Result<Value> {
            self.stack
                .pop()
                .ok_or_else(|| Error::runtime_stack_underflow("Stack underflow"))
        }

        fn peek_value(&self) -> Result<&Value> {
            self.stack
                .last()
                .ok_or_else(|| Error::runtime_stack_underflow("Stack underflow"))
        }
    }

    #[test]
    fn test_drop() {
        let mut context = MockParametricContext::new();

        // Push a value
        context.push_value(Value::I32(42)).unwrap();
        assert_eq!(context.stack.len(), 1);

        // Execute drop
        ParametricOp::Drop.execute(&mut context).unwrap();
        assert_eq!(context.stack.len(), 0);

        // Test drop on empty stack
        let result = ParametricOp::Drop.execute(&mut context);
        assert!(result.is_err());
    }

    #[test]
    fn test_select() {
        let mut context = MockParametricContext::new();

        // Test selecting first value (condition true)
        context.push_value(Value::I32(10)).unwrap(); // val1
        context.push_value(Value::I32(20)).unwrap(); // val2
        context.push_value(Value::I32(1)).unwrap(); // condition (true)

        ParametricOp::Select.execute(&mut context).unwrap();
        assert_eq!(context.pop_value().unwrap(), Value::I32(10));

        // Test selecting second value (condition false)
        context.push_value(Value::I32(10)).unwrap(); // val1
        context.push_value(Value::I32(20)).unwrap(); // val2
        context.push_value(Value::I32(0)).unwrap(); // condition (false)

        ParametricOp::Select.execute(&mut context).unwrap();
        assert_eq!(context.pop_value().unwrap(), Value::I32(20));

        // Test with different types
        context.push_value(Value::F32(FloatBits32::from_float(1.0))).unwrap(); // val1
        context.push_value(Value::F32(FloatBits32::from_float(2.0))).unwrap(); // val2
        context.push_value(Value::I32(1)).unwrap(); // condition

        ParametricOp::Select.execute(&mut context).unwrap();
        assert_eq!(
            context.pop_value().unwrap(),
            Value::F32(FloatBits32::from_float(1.0))
        );
    }

    #[test]
    fn test_select_type_mismatch() {
        let mut context = MockParametricContext::new();

        // Push values of different types
        context.push_value(Value::I32(10)).unwrap(); // val1
        context.push_value(Value::F32(FloatBits32::from_float(2.0))).unwrap(); // val2 (different type)
        context.push_value(Value::I32(1)).unwrap(); // condition

        let result = ParametricOp::Select.execute(&mut context);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_typed() {
        let mut context = MockParametricContext::new();

        // Test with correct types
        context.push_value(Value::I64(100)).unwrap(); // val1
        context.push_value(Value::I64(200)).unwrap(); // val2
        context.push_value(Value::I32(0)).unwrap(); // condition

        ParametricOp::SelectTyped(ValueType::I64).execute(&mut context).unwrap();
        assert_eq!(context.pop_value().unwrap(), Value::I64(200));

        // Test with incorrect types
        context.push_value(Value::I32(10)).unwrap(); // val1 (wrong type)
        context.push_value(Value::I32(20)).unwrap(); // val2 (wrong type)
        context.push_value(Value::I32(1)).unwrap(); // condition

        let result = ParametricOp::SelectTyped(ValueType::I64).execute(&mut context);
        assert!(result.is_err());
    }
}
