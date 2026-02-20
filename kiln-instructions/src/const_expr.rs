//! WebAssembly constant expressions implementation.
//!
//! This module implements support for WebAssembly constant expressions,
//! which are limited sequences of instructions that can be evaluated at
//! compile time. These are used for:
//! - Global variable initialization
//! - Element segment offsets
//! - Data segment offsets
//!
//! The extended constant expressions proposal adds support for more
//! instructions in constant contexts.

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    types::{
        RefType,
        ValueType,
    },
    values::Value,
};
use wrt_math;

#[cfg(not(feature = "std"))]
use crate::prelude::BoundedVec;
use crate::{
    prelude::{
        Debug,
        PartialEq,
    },
    validation::{
        Validate,
        ValidationContext,
    },
};

/// Instructions allowed in constant expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ConstExpr {
    /// Push an i32 constant
    I32Const(i32),
    /// Push an i64 constant
    I64Const(i64),
    /// Push an f32 constant
    F32Const(f32),
    /// Push an f64 constant
    F64Const(f64),
    /// Push a null reference (extended const expressions)
    RefNull(RefType),
    /// Get a function reference (extended const expressions)
    RefFunc(u32),
    /// Get a global value (extended const expressions)
    GlobalGet(u32),

    // Extended constant expressions operations
    /// Add two i32 values (extended const expressions)
    I32Add,
    /// Subtract two i32 values (extended const expressions)
    I32Sub,
    /// Multiply two i32 values (extended const expressions)
    I32Mul,
    /// Add two i64 values (extended const expressions)
    I64Add,
    /// Subtract two i64 values (extended const expressions)
    I64Sub,
    /// Multiply two i64 values (extended const expressions)
    I64Mul,

    /// End marker for constant expression
    End,
}

impl Default for ConstExpr {
    fn default() -> Self {
        ConstExpr::I32Const(0)
    }
}

/// Context for evaluating constant expressions
pub trait ConstExprContext {
    /// Get the value of a global variable
    ///
    /// # Errors
    ///
    /// Returns an error if the global index is invalid
    fn get_global(&self, index: u32) -> Result<Value>;

    /// Check if a function index is valid
    fn is_valid_func(&self, index: u32) -> bool;

    /// Get the number of globals
    fn global_count(&self) -> u32;
}

/// A sequence of constant expression instructions
#[derive(Debug, Clone)]
pub struct ConstExprSequence {
    // Use a fixed-size array for no_std compatibility
    instructions: [Option<ConstExpr>; 16],
    len:          usize,
}

impl ConstExprSequence {
    /// Create a new constant expression sequence
    #[must_use]
    pub fn new() -> Self {
        Self {
            instructions: Default::default(),
            len:          0,
        }
    }

    /// Add an instruction to the sequence
    ///
    /// # Errors
    ///
    /// Returns an error if the sequence is full
    pub fn push(&mut self, instr: ConstExpr) -> Result<()> {
        if self.len >= 16 {
            return Err(Error::memory_error(
                "Constant expression sequence exceeds maximum size",
            ));
        }
        self.instructions[self.len] = Some(instr);
        self.len += 1;
        Ok(())
    }

    /// Helper to pop from stack in both std and `no_std` environments
    #[cfg(feature = "std")]
    fn stack_pop(stack: &mut Vec<Value>) -> Result<Value> {
        stack
            .pop()
            .ok_or_else(|| Error::runtime_error("Constant expression stack underflow"))
    }

    /// Helper to pop from stack in both std and `no_std` environments
    #[cfg(not(feature = "std"))]
    fn stack_pop(
        stack: &mut BoundedVec<Value, 8, wrt_foundation::NoStdProvider<128>>,
    ) -> Result<Value> {
        match stack.pop() {
            Ok(Some(val)) => Ok(val),
            Ok(None) => Err(Error::runtime_error("Constant expression stack underflow")),
            Err(_) => Err(Error::runtime_error("Constant expression stack error")),
        }
    }

    /// Evaluate the constant expression sequence
    ///
    /// # Errors
    ///
    /// Returns an error if evaluation fails or invalid operations occur
    // Large match is intentional - interpreter dispatch for const expression opcodes.
    #[allow(clippy::too_many_lines)]
    pub fn evaluate(&self, context: &dyn ConstExprContext) -> Result<Value> {
        #[cfg(feature = "std")]
        let mut stack = Vec::new();

        #[cfg(not(feature = "std"))]
        let mut stack = {
            let provider = wrt_foundation::safe_managed_alloc!(
                128,
                wrt_foundation::budget_aware_provider::CrateId::Instructions
            )?;
            BoundedVec::<Value, 8, wrt_foundation::NoStdProvider<128>>::new(provider)
                .map_err(|_| Error::memory_error("Failed to create evaluation stack"))?
        };

        for i in 0..self.len {
            let instr = self.instructions[i]
                .as_ref()
                .ok_or_else(|| Error::runtime_error("Invalid constant expression"))?;
            match instr {
                ConstExpr::I32Const(v) => {
                    #[cfg(feature = "std")]
                    stack.push(Value::I32(*v));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I32(*v))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I64Const(v) => {
                    #[cfg(feature = "std")]
                    stack.push(Value::I64(*v));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I64(*v))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::F32Const(v) => {
                    let float_bits = wrt_foundation::values::FloatBits32::from_float(*v);
                    #[cfg(feature = "std")]
                    stack.push(Value::F32(float_bits));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::F32(float_bits))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::F64Const(v) => {
                    let float_bits = wrt_foundation::values::FloatBits64::from_float(*v);
                    #[cfg(feature = "std")]
                    stack.push(Value::F64(float_bits));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::F64(float_bits))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::RefNull(ref_type) => {
                    let value = match ref_type {
                        RefType::Funcref => Value::FuncRef(None),
                        RefType::Externref => Value::ExternRef(None),
                    };

                    #[cfg(feature = "std")]
                    stack.push(value);

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(value)
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::RefFunc(idx) => {
                    if !context.is_valid_func(*idx) {
                        return Err(Error::validation_error(
                            "Invalid function index in const expression",
                        ));
                    }

                    let func_ref = wrt_foundation::values::FuncRef { index: *idx };

                    #[cfg(feature = "std")]
                    stack.push(Value::FuncRef(Some(func_ref)));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::FuncRef(Some(func_ref)))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::GlobalGet(idx) => {
                    let value = context.get_global(*idx)?;

                    #[cfg(feature = "std")]
                    stack.push(value);

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(value)
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I32Add => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I32(a_val), Value::I32(b_val)) = (a, b) else {
                        return Err(Error::type_error("I32Add requires two i32 values"));
                    };

                    let result = wrt_math::i32_add(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I32(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I32(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I32Sub => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I32(a_val), Value::I32(b_val)) = (a, b) else {
                        return Err(Error::type_error("I32Sub requires two i32 values"));
                    };

                    let result = wrt_math::i32_sub(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I32(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I32(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I32Mul => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I32(a_val), Value::I32(b_val)) = (a, b) else {
                        return Err(Error::type_error("I32Mul requires two i32 values"));
                    };

                    let result = wrt_math::i32_mul(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I32(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I32(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I64Add => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I64(a_val), Value::I64(b_val)) = (a, b) else {
                        return Err(Error::type_error("I64Add requires two i64 values"));
                    };

                    let result = wrt_math::i64_add(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I64(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I64(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I64Sub => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I64(a_val), Value::I64(b_val)) = (a, b) else {
                        return Err(Error::type_error("I64Sub requires two i64 values"));
                    };

                    let result = wrt_math::i64_sub(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I64(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I64(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::I64Mul => {
                    let b = Self::stack_pop(&mut stack)?;
                    let a = Self::stack_pop(&mut stack)?;

                    let (Value::I64(a_val), Value::I64(b_val)) = (a, b) else {
                        return Err(Error::type_error("I64Mul requires two i64 values"));
                    };

                    let result = wrt_math::i64_mul(a_val, b_val)?;

                    #[cfg(feature = "std")]
                    stack.push(Value::I64(result));

                    #[cfg(not(feature = "std"))]
                    stack
                        .push(Value::I64(result))
                        .map_err(|_| Error::runtime_error("Constant expression stack overflow"))?;
                },
                ConstExpr::End => {
                    // End of expression - return top of stack
                    return Self::stack_pop(&mut stack);
                },
            }
        }

        // If we get here without an End, the expression is invalid
        Err(Error::validation_error(
            "Constant expression missing End marker",
        ))
    }
}

impl Validate for ConstExpr {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            ConstExpr::I32Const(_) => {
                ctx.push_type(ValueType::I32)?;
                Ok(())
            },
            ConstExpr::I64Const(_) => {
                ctx.push_type(ValueType::I64)?;
                Ok(())
            },
            ConstExpr::F32Const(_) => {
                ctx.push_type(ValueType::F32)?;
                Ok(())
            },
            ConstExpr::F64Const(_) => {
                ctx.push_type(ValueType::F64)?;
                Ok(())
            },
            ConstExpr::RefNull(ref_type) => {
                let val_type = match ref_type {
                    RefType::Funcref => ValueType::FuncRef,
                    RefType::Externref => ValueType::ExternRef,
                };
                ctx.push_type(val_type)?;
                Ok(())
            },
            ConstExpr::RefFunc(_idx) => {
                // TODO: Validate function index
                ctx.push_type(ValueType::FuncRef)?;
                Ok(())
            },
            ConstExpr::GlobalGet(idx) => {
                // TODO: Add globals field to ValidationContext
                // For now, just validate index is reasonable
                if *idx >= 1000 {
                    return Err(Error::validation_error("Invalid global index"));
                }
                // TODO: Get actual global type
                ctx.push_type(ValueType::I32)?;
                Ok(())
            },
            ConstExpr::I32Add | ConstExpr::I32Sub | ConstExpr::I32Mul => {
                ctx.pop_expect(ValueType::I32)?;
                ctx.pop_expect(ValueType::I32)?;
                ctx.push_type(ValueType::I32)?;
                Ok(())
            },
            ConstExpr::I64Add | ConstExpr::I64Sub | ConstExpr::I64Mul => {
                ctx.pop_expect(ValueType::I64)?;
                ctx.pop_expect(ValueType::I64)?;
                ctx.push_type(ValueType::I64)?;
                Ok(())
            },
            ConstExpr::End => Ok(()),
        }
    }
}

impl Default for ConstExprSequence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    // Import Vec and vec! based on feature flags
    use std::{
        vec,
        vec::Vec,
    };

    use super::*;

    struct TestConstExprContext {
        globals:    Vec<Value>,
        func_count: u32,
    }

    impl ConstExprContext for TestConstExprContext {
        fn get_global(&self, index: u32) -> Result<Value> {
            self.globals
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Error::validation_error("Invalid global index"))
        }

        fn is_valid_func(&self, index: u32) -> bool {
            index < self.func_count
        }

        fn global_count(&self) -> u32 {
            self.globals.len() as u32
        }
    }

    #[test]
    fn test_simple_const_expr() {
        let mut expr = ConstExprSequence::new();
        expr.push(ConstExpr::I32Const(42)).unwrap();
        expr.push(ConstExpr::End).unwrap();

        let context = TestConstExprContext {
            globals:    Vec::new(),
            func_count: 0,
        };

        let result = expr.evaluate(&context).unwrap();
        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn test_arithmetic_const_expr() {
        let mut expr = ConstExprSequence::new();
        expr.push(ConstExpr::I32Const(10)).unwrap();
        expr.push(ConstExpr::I32Const(32)).unwrap();
        expr.push(ConstExpr::I32Add).unwrap();
        expr.push(ConstExpr::End).unwrap();

        let context = TestConstExprContext {
            globals:    Vec::new(),
            func_count: 0,
        };

        let result = expr.evaluate(&context).unwrap();
        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn test_global_get_const_expr() {
        let mut expr = ConstExprSequence::new();
        expr.push(ConstExpr::GlobalGet(0)).unwrap();
        expr.push(ConstExpr::End).unwrap();

        let context = TestConstExprContext {
            globals:    {
                let mut v = Vec::new();
                v.push(Value::I32(100));
                v
            },
            func_count: 0,
        };

        let result = expr.evaluate(&context).unwrap();
        assert_eq!(result, Value::I32(100));
    }
}
