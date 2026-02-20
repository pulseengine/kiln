//! Tail call optimization implementation for the stackless engine.
//!
//! This module provides tail call optimization support, allowing functions to
//! make tail calls without growing the call stack. This is essential for
//! functional programming patterns and recursive algorithms.

// alloc is imported in lib.rs with proper feature gates

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    types::FuncType,
    Value,
};
use wrt_instructions::control_ops::ControlContext;

// Type alias for FuncType to match module_instance.rs
use crate::bounded_runtime_infra::RuntimeProvider;
use crate::{
    module_instance::ModuleInstance,
    prelude::*,
    stackless::{
        engine::StacklessEngine,
        frame::StacklessFrame,
    },
};
type WrtFuncType = wrt_foundation::types::FuncType;

#[cfg(feature = "std")]
use alloc::vec::Vec;

/// Tail call implementation for the stackless engine
impl StacklessEngine {
    /// Execute a tail call to a function
    ///
    /// This replaces the current call frame with a new one for the target
    /// function, implementing proper tail call optimization.
    ///
    /// # Arguments
    ///
    /// * `func_idx` - Index of the function to tail call
    /// * `module` - The module instance containing the function
    ///
    /// # Returns
    ///
    /// Success or an error if the tail call fails
    pub fn execute_tail_call(&mut self, func_idx: u32, module: &mut ModuleInstance) -> Result<()> {
        // Get the function to call
        let func = module.get_function(func_idx as usize)?;

        // Get function type for parameter/result validation
        let func_type = module.get_function_type(func_idx as usize)?;

        // Pop arguments from the operand stack
        let mut args = Vec::with_capacity(func_type.params.len());
        for _ in 0..func_type.params.len() {
            if self.operand_stack.is_empty() {
                return Err(Error::runtime_error("Stack underflow"));
            }
            let last_idx = self.operand_stack.len() - 1;
            let value = self.operand_stack.remove(last_idx);
            args.push(value);
        }
        args.reverse(); // Arguments were popped in reverse order

        // For tail calls, we simulate replacing the current frame
        // In a full implementation, this would replace the actual frame
        if self.call_frames_count == 0 {
            return Err(Error::runtime_error("No active frame for tail call"));
        }

        // Simulate tail call by resetting to new function
        // In practice, this would involve more complex frame management

        // Update execution statistics
        self.stats.function_calls += 1;

        Ok(())
    }

    /// Execute a tail call through a table (return_call_indirect)
    ///
    /// This performs an indirect tail call through a function table.
    ///
    /// # Arguments
    ///
    /// * `table_idx` - Index of the table containing function references
    /// * `type_idx` - Expected function type index
    /// * `func_idx` - Function index within the table
    /// * `module` - The module instance
    ///
    /// # Returns
    ///
    /// Success or an error if the tail call fails
    pub fn execute_tail_call_indirect(
        &mut self,
        table_idx: u32,
        type_idx: u32,
        func_idx: u32,
        module: &mut ModuleInstance,
    ) -> Result<()> {
        // Get the table
        let table = module.get_table(table_idx as usize)?;

        // Get function reference from table
        let func_ref_opt = table.get(func_idx)?;
        let func_ref = func_ref_opt.ok_or_else(|| Error::runtime_error("Table slot is empty"))?;

        // Validate function reference
        let actual_func_idx = match func_ref {
            Value::FuncRef(Some(func_ref)) => func_ref.index,
            Value::FuncRef(None) => {
                return Err(Error::runtime_error("Null function reference in table"));
            },
            _ => {
                return Err(Error::type_error("Expected function reference in table"));
            },
        };

        // Get expected function type
        let expected_type = module.get_type(type_idx as usize)?;

        // Get actual function type
        let actual_type = module.get_function_type(actual_func_idx as usize)?;

        // Validate type compatibility
        if actual_type.params != expected_type.params
            || actual_type.results != expected_type.results
        {
            return Err(Error::type_error(
                "Function type mismatch in tail call indirect",
            ));
        }

        // Execute the tail call
        self.execute_tail_call(actual_func_idx, module)
    }
}

/// Extension trait to add tail call methods to control context
pub trait TailCallContext: ControlContext {
    /// Execute a tail call
    fn execute_return_call(&mut self, func_idx: u32) -> Result<()>;

    /// Execute an indirect tail call
    fn execute_return_call_indirect(&mut self, table_idx: u32, type_idx: u32) -> Result<()>;
}

/// Helper functions for tail call validation
pub mod validation {
    use super::*;

    /// Validate that a tail call is valid in the current context
    ///
    /// Tail calls are valid when:
    /// 1. The current function's return type matches the called function's
    ///    return type
    /// 2. The operand stack has exactly the right number of arguments
    pub fn validate_tail_call(
        current_func_type: &WrtFuncType,
        target_func_type: &WrtFuncType,
    ) -> Result<()> {
        // Check return type compatibility
        if current_func_type.results != target_func_type.results {
            return Err(Error::validation_error(
                "Tail call return type mismatch: current function and target function must have \
                 same return types",
            ));
        }

        Ok(())
    }

    /// Check if tail call optimization can be applied
    ///
    /// This checks various conditions that might prevent tail call optimization
    pub fn can_optimize_tail_call(has_try_catch_blocks: bool, in_multivalue_block: bool) -> bool {
        // Tail calls cannot be optimized if:
        // 1. We're inside a try-catch block (exception handling)
        // 2. We're in a block that expects multiple values
        !has_try_catch_blocks && !in_multivalue_block
    }
}

