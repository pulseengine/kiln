//! Stackless function activation frame
//!
//! This module implements a basic activation frame structure for the stackless
//! WebAssembly execution engine.

use wrt_error::Result;
use wrt_foundation::values::Value;

/// Simple stackless function frame
#[derive(Debug, Clone, PartialEq)]
pub struct StacklessFrame {
    /// Function index being executed
    pub function_index: usize,
    /// Local variables
    pub locals:         [Value; 8], // Fixed size for simplicity
    /// Number of locals in use
    pub locals_count:   usize,
    /// Frame ID for debugging
    pub frame_id:       u32,
}

impl StacklessFrame {
    /// Create a new frame for a function
    pub fn new(function_index: usize, locals: &[Value]) -> Result<Self> {
        let mut frame_locals = [
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
            Value::I32(0),
        ];
        let locals_count = locals.len().min(8);

        for (i, local) in locals.iter().take(locals_count).enumerate() {
            frame_locals[i] = local.clone();
        }

        Ok(Self {
            function_index,
            locals: frame_locals,
            locals_count,
            frame_id: 0,
        })
    }

    /// Get a local variable
    pub fn get_local(&self, index: usize) -> Result<Value> {
        if index < self.locals_count {
            Ok(self.locals[index].clone())
        } else {
            Err(wrt_error::Error::runtime_execution_error(
                "Local index out of bounds",
            ))
        }
    }

    /// Set a local variable
    pub fn set_local(&mut self, index: usize, value: Value) -> Result<()> {
        if index < self.locals_count {
            self.locals[index] = value;
            Ok(())
        } else {
            Err(wrt_error::Error::runtime_execution_error(
                "Local index out of bounds",
            ))
        }
    }
}

impl Default for StacklessFrame {
    fn default() -> Self {
        Self {
            function_index: 0,
            locals:         [
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
                Value::I32(0),
            ],
            locals_count:   0,
            frame_id:       0,
        }
    }
}
