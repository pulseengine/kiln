// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Variable operations for WebAssembly instructions.
//!
//! This module provides pure implementations for WebAssembly variable access
//! instructions, including local and global variable operations.

use crate::prelude::{
    Debug,
    Error,
    PureInstruction,
    Result,
    Value,
};

// ToString is brought in through the prelude for both std and no_std
// configurations so we don't need explicit imports

/// Represents a pure variable operation for WebAssembly.
#[derive(Debug, Clone)]
pub enum VariableOp {
    /// Get the value of a local variable
    LocalGet(u32),
    /// Set the value of a local variable
    LocalSet(u32),
    /// Set the value of a local variable and return the value
    LocalTee(u32),
    /// Get the value of a global variable
    GlobalGet(u32),
    /// Set the value of a global variable
    GlobalSet(u32),
}

/// Execution context for variable operations
pub trait VariableContext {
    /// Get a local variable value by index
    ///
    /// # Errors
    ///
    /// Returns an error if the local index is invalid
    fn get_local(&self, index: u32) -> Result<Value>;

    /// Set a local variable value by index
    ///
    /// # Errors
    ///
    /// Returns an error if the local index is invalid or type mismatch occurs
    fn set_local(&mut self, index: u32, value: Value) -> Result<()>;

    /// Get a global variable value by index
    ///
    /// # Errors
    ///
    /// Returns an error if the global index is invalid
    fn get_global(&self, index: u32) -> Result<Value>;

    /// Set a global variable value by index
    ///
    /// # Errors
    ///
    /// Returns an error if the global index is invalid or type mismatch occurs
    fn set_global(&mut self, index: u32, value: Value) -> Result<()>;

    /// Push a value to the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is full
    fn push_value(&mut self, value: Value) -> Result<()>;

    /// Pop a value from the context
    ///
    /// # Errors
    ///
    /// Returns an error if the stack is empty
    fn pop_value(&mut self) -> Result<Value>;
}

impl<T: VariableContext> PureInstruction<T, Error> for VariableOp {
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            Self::LocalGet(index) => {
                let value = context.get_local(*index)?;
                context.push_value(value)
            },
            Self::LocalSet(index) => {
                let value = context.pop_value()?;
                context.set_local(*index, value)
            },
            Self::LocalTee(index) => {
                let value = context.pop_value()?;
                context.set_local(*index, value.clone())?;
                context.push_value(value)
            },
            Self::GlobalGet(index) => {
                let value = context.get_global(*index)?;
                context.push_value(value)
            },
            Self::GlobalSet(index) => {
                let value = context.pop_value()?;
                context.set_global(*index, value)
            },
        }
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

    // Mock variable context for testing
    struct MockVariableContext {
        locals:  Vec<Value>,
        globals: Vec<Value>,
        stack:   Vec<Value>,
    }

    impl MockVariableContext {
        fn new() -> Self {
            Self {
                locals:  {
                    let mut v = Vec::with_capacity(10);
                    for _ in 0..10 {
                        v.push(Value::I32(0));
                    }
                    v
                },
                globals: {
                    let mut v = Vec::with_capacity(5);
                    for _ in 0..5 {
                        v.push(Value::I32(0));
                    }
                    v
                },
                stack:   Vec::new(),
            }
        }
    }

    impl VariableContext for MockVariableContext {
        fn get_local(&self, index: u32) -> Result<Value> {
            self.locals
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Error::invalid_function_index("Invalid local index"))
        }

        fn set_local(&mut self, index: u32, value: Value) -> Result<()> {
            if let Some(local) = self.locals.get_mut(index as usize) {
                *local = value;
                Ok(())
            } else {
                Err(Error::invalid_function_index("Invalid local index"))
            }
        }

        fn get_global(&self, index: u32) -> Result<Value> {
            self.globals
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Error::invalid_function_index("Invalid global index"))
        }

        fn set_global(&mut self, index: u32, value: Value) -> Result<()> {
            if let Some(global) = self.globals.get_mut(index as usize) {
                *global = value;
                Ok(())
            } else {
                Err(Error::invalid_function_index("Invalid global index"))
            }
        }

        fn push_value(&mut self, value: Value) -> Result<()> {
            self.stack.push(value);
            Ok(())
        }

        fn pop_value(&mut self) -> Result<Value> {
            self.stack
                .pop()
                .ok_or_else(|| Error::runtime_stack_underflow("Stack underflow"))
        }
    }

    #[test]
    fn test_local_operations() {
        let mut context = MockVariableContext::new();

        // Set up initial values
        context.set_local(0, Value::I32(42)).unwrap();
        context.set_local(1, Value::I64(100)).unwrap();

        // Test local.get
        VariableOp::LocalGet(0).execute(&mut context).unwrap();
        assert_eq!(context.pop_value().unwrap(), Value::I32(42));

        // Test local.set
        context.push_value(Value::I32(123)).unwrap();
        VariableOp::LocalSet(0).execute(&mut context).unwrap();
        assert_eq!(context.get_local(0).unwrap(), Value::I32(123));

        // Test local.tee
        context.push_value(Value::I32(999)).unwrap();
        VariableOp::LocalTee(0).execute(&mut context).unwrap();
        assert_eq!(context.get_local(0).unwrap(), Value::I32(999));
        assert_eq!(context.pop_value().unwrap(), Value::I32(999));
    }

    #[test]
    fn test_global_operations() {
        let mut context = MockVariableContext::new();

        // Set up initial values
        context.set_global(0, Value::I32(42)).unwrap();
        context.set_global(1, Value::I64(100)).unwrap();

        // Test global.get
        VariableOp::GlobalGet(0).execute(&mut context).unwrap();
        assert_eq!(context.pop_value().unwrap(), Value::I32(42));

        // Test global.set
        context.push_value(Value::I32(123)).unwrap();
        VariableOp::GlobalSet(0).execute(&mut context).unwrap();
        assert_eq!(context.get_global(0).unwrap(), Value::I32(123));
    }

    #[test]
    fn test_variable_errors() {
        let mut context = MockVariableContext::new();

        // Test invalid local index
        let result = VariableOp::LocalGet(100).execute(&mut context);
        assert!(result.is_err());

        // Test invalid global index
        let result = VariableOp::GlobalGet(100).execute(&mut context);
        assert!(result.is_err());

        // Test stack underflow
        let result = VariableOp::LocalSet(0).execute(&mut context);
        assert!(result.is_err());
    }
}
