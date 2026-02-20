//! Host ABI Bridge for Canonical Function Calls
//!
//! This module provides the bridge between host function calls and the
//! Canonical ABI. It handles:
//! - Converting between foundation Value types and ComponentValue types
//! - Lifting arguments from core WASM representation
//! - Lowering results to core WASM representation or memory (retptr)
//!
//! # Usage
//!
//! When the engine calls a host function:
//! 1. Create a HostAbiContext with the function signature
//! 2. Call process_host_call with stack values and memory
//! 3. Get back either stack values to push or nothing (if retptr was used)

#[cfg(feature = "std")]
use std::vec;
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(feature = "std")]
use std::string::String;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use wrt_error::{Error, Result};
use wrt_foundation::values::Value as FoundationValue;

use super::{
    CanonicalABI, CanonicalMemory, ComponentType, ComponentValue,
};

/// Result of processing a host call through the canonical ABI.
pub enum HostCallResult {
    /// Values to push onto the stack
    StackValues(Vec<FoundationValue>),
    /// Result was written to memory via retptr, no stack values
    WrittenToMemory,
}

/// Context for handling host function calls with proper canonical ABI.
pub struct HostAbiContext {
    /// The canonical ABI instance
    abi: CanonicalABI,
    /// Parameter types for the function
    param_types: Vec<ComponentType>,
    /// Result types for the function
    result_types: Vec<ComponentType>,
}

impl HostAbiContext {
    /// Create a new host ABI context for a function with the given signature.
    pub fn new(param_types: Vec<ComponentType>, result_types: Vec<ComponentType>) -> Self {
        Self {
            abi: CanonicalABI::new(),
            param_types,
            result_types,
        }
    }

    /// Create context for wall-clock::now() -> datetime
    /// datetime = record { seconds: u64, nanoseconds: u32 }
    pub fn for_wall_clock_now() -> Self {
        Self::new(
            vec![], // no params (retptr is implicit)
            vec![ComponentType::Record(vec![
                ("seconds".into(), ComponentType::U64),
                ("nanoseconds".into(), ComponentType::U32),
            ])],
        )
    }

    /// Process a host function result and lower it appropriately.
    ///
    /// If the result fits on the stack, returns StackValues.
    /// If the result requires retptr (records, etc.), writes to memory and returns WrittenToMemory.
    ///
    /// # Arguments
    /// * `result` - The result from the host function (in FoundationValue format)
    /// * `stack_args` - The arguments that were on the stack (may include retptr)
    /// * `memory` - The linear memory to write to if needed
    pub fn lower_result<M: CanonicalMemory>(
        &self,
        result: Vec<FoundationValue>,
        stack_args: &[FoundationValue],
        memory: &mut M,
    ) -> Result<HostCallResult> {
        // If no result types, nothing to return
        if self.result_types.is_empty() {
            return Ok(HostCallResult::StackValues(vec![]));
        }

        // Check if result needs retptr (records, tuples with >1 element, etc.)
        let needs_retptr = self.result_needs_retptr();

        if needs_retptr {
            // First stack arg should be retptr
            let retptr = match stack_args.first() {
                Some(FoundationValue::I32(ptr)) => *ptr as u32,
                _ => return Err(Error::validation_error("Expected retptr as first argument")),
            };

            // Convert result to ComponentValue and lower to memory
            if let Some(foundation_val) = result.first() {
                let component_val = self.foundation_to_component(foundation_val)?;

                // Lower the component value to memory at retptr
                self.abi.lower(memory, &component_val, retptr)?;
            }

            Ok(HostCallResult::WrittenToMemory)
        } else {
            // Result fits on stack - convert and return
            let mut stack_results = Vec::new();
            for (val, ty) in result.iter().zip(self.result_types.iter()) {
                let core_vals = self.flatten_to_core(val, ty)?;
                stack_results.extend(core_vals);
            }
            Ok(HostCallResult::StackValues(stack_results))
        }
    }

    /// Check if the result type requires a retptr.
    /// Per Canonical ABI: records, tuples (depending on size), and other aggregate types
    /// need to be written to memory rather than returned on stack.
    fn result_needs_retptr(&self) -> bool {
        if self.result_types.len() != 1 {
            return self.result_types.len() > 1; // Multiple results need retptr
        }

        match &self.result_types[0] {
            // Records always need retptr
            ComponentType::Record(_) => true,
            // Tuples with more than one element need retptr
            ComponentType::Tuple(types) => types.len() > 1,
            // Lists need retptr
            ComponentType::List(_) => true,
            // Strings need retptr
            ComponentType::String => true,
            // Primitives don't need retptr
            _ => false,
        }
    }

    /// Convert a FoundationValue to a ComponentValue.
    fn foundation_to_component(&self, val: &FoundationValue) -> Result<ComponentValue> {
        match val {
            FoundationValue::Bool(b) => Ok(ComponentValue::Bool(*b)),
            FoundationValue::S8(v) => Ok(ComponentValue::S8(*v)),
            FoundationValue::U8(v) => Ok(ComponentValue::U8(*v)),
            FoundationValue::S16(v) => Ok(ComponentValue::S16(*v)),
            FoundationValue::U16(v) => Ok(ComponentValue::U16(*v)),
            FoundationValue::S32(v) => Ok(ComponentValue::S32(*v)),
            FoundationValue::U32(v) => Ok(ComponentValue::U32(*v)),
            FoundationValue::I32(v) => Ok(ComponentValue::S32(*v)),
            FoundationValue::S64(v) => Ok(ComponentValue::S64(*v)),
            FoundationValue::U64(v) => Ok(ComponentValue::U64(*v)),
            FoundationValue::I64(v) => Ok(ComponentValue::S64(*v)),
            FoundationValue::Char(c) => Ok(ComponentValue::Char(*c)),
            FoundationValue::String(s) => Ok(ComponentValue::String(s.clone())),
            FoundationValue::Tuple(items) => {
                let mut converted = Vec::new();
                for item in items {
                    converted.push(self.foundation_to_component(item)?);
                }
                Ok(ComponentValue::Tuple(converted))
            }
            FoundationValue::Record(fields) => {
                let mut converted = Vec::new();
                for (name, val) in fields {
                    converted.push((name.clone(), self.foundation_to_component(val)?));
                }
                Ok(ComponentValue::Record(converted))
            }
            FoundationValue::List(items) => {
                let mut converted = Vec::new();
                for item in items {
                    converted.push(self.foundation_to_component(item)?);
                }
                Ok(ComponentValue::List(converted))
            }
            _ => Err(Error::validation_error("Unsupported value type for component conversion")),
        }
    }

    /// Flatten a FoundationValue to core WASM values for stack return.
    fn flatten_to_core(&self, val: &FoundationValue, _ty: &ComponentType) -> Result<Vec<FoundationValue>> {
        match val {
            // Primitives map directly
            FoundationValue::I32(_) | FoundationValue::I64(_) |
            FoundationValue::U32(_) | FoundationValue::U64(_) |
            FoundationValue::S32(_) | FoundationValue::S64(_) |
            FoundationValue::Bool(_) => Ok(vec![val.clone()]),

            // Other types should use retptr, not stack
            _ => Err(Error::validation_error("Complex types should use retptr, not stack")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wall_clock_now_needs_retptr() {
        let ctx = HostAbiContext::for_wall_clock_now();
        assert!(ctx.result_needs_retptr());
    }

    #[test]
    fn test_primitive_doesnt_need_retptr() {
        let ctx = HostAbiContext::new(vec![], vec![ComponentType::U32]);
        assert!(!ctx.result_needs_retptr());
    }
}
