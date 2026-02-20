// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Traits for WebAssembly instruction implementation.
//!
//! This module provides traits that define the interfaces for pure instruction
//! implementations. These traits establish a clear boundary between instruction
//! implementations and runtime details.

/// Trait for pure instruction execution.
///
/// This trait defines the interface for executing a pure instruction with a
/// given context. The context type is generic, allowing different execution
/// engines to provide their own context.
pub trait PureInstruction<T, E> {
    /// Executes the instruction with the given context.
    ///
    /// # Arguments
    ///
    /// * `context` - The execution context
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the instruction executed successfully
    /// * `Err(E)` - If an error occurred during execution
    ///
    /// # Errors
    ///
    /// Returns an error if instruction execution fails
    fn execute(&self, context: &mut T) -> core::result::Result<(), E>;
}

/// Trait for pure memory instructions.
///
/// This trait defines the interface for executing a pure memory instruction
/// with a given memory. The memory type is generic, allowing different
/// execution engines to provide their own memory.
pub trait PureMemoryInstruction<T, E> {
    /// Executes the memory instruction with the given memory.
    ///
    /// # Arguments
    ///
    /// * `memory` - The memory to operate on
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the instruction executed successfully
    /// * `Err(E)` - If an error occurred during execution
    ///
    /// # Errors
    ///
    /// Returns an error if memory operation fails
    fn execute_memory(&self, memory: &mut T) -> core::result::Result<(), E>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn test_pure_instruction_trait() {
        struct TestContext {
            value: i32,
        }

        struct TestInstruction;

        impl PureInstruction<TestContext, Error> for TestInstruction {
            fn execute(&self, context: &mut TestContext) -> core::result::Result<(), Error> {
                context.value += 1;
                Ok(())
            }
        }

        let mut context = TestContext { value: 0 };
        let instruction = TestInstruction;
        instruction.execute(&mut context).unwrap();
        assert_eq!(context.value, 1);
    }
}
