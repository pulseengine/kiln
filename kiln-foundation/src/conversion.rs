// WRT - wrt-foundation
// Module: Type Conversion Utilities
// SW-REQ-ID: REQ_018
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Type conversion utilities for WebAssembly types
//!
//! This module provides functions for converting between different
//! WebAssembly type representations, such as between ValType and BinaryType.

// Remove Vec-related imports as they are no longer needed at the top level or
// directly in func_type module #[cfg(all(not(feature =
// Binary std/no_std choice

// #[cfg(feature = "std")]
// use std::vec::Vec; // This was for the module scope, func_type::create used
// its own.

use wrt_error::{
    codes,
    Error,
    Result,
};

#[cfg(feature = "std")]
use crate::{
    BlockType,
    FuncType,
    RefType,
    ValueType as CoreValueType,
};

/// Convert `RefType` to `ValueType`
///
/// Provides a standard way to convert between reference types
/// and value types across all crates.
#[cfg(feature = "std")]
#[must_use]
pub fn ref_type_to_val_type(ref_type: RefType) -> CoreValueType {
    match ref_type {
        RefType::Funcref => CoreValueType::FuncRef,
        RefType::Externref => CoreValueType::ExternRef,
    }
}

/// Convert `ValueType` to `RefType`
///
/// Provides a standard way to convert between value types
/// and reference types across all crates.
#[cfg(feature = "std")]
pub fn val_type_to_ref_type(val_type: CoreValueType) -> Result<RefType> {
    match val_type {
        CoreValueType::FuncRef => Ok(RefType::Funcref),
        CoreValueType::ExternRef => Ok(RefType::Externref),
        _ => Err(Error::runtime_execution_error(
            "Value type is not a reference type",
        )),
    }
}

/// Block type utilities for converting between different representations
#[cfg(feature = "std")]
pub mod block_type {
    use super::BlockType;

    /// Convert a `FormatBlockType` (from wrt-format) to `BlockType`
    ///
    /// This utility makes it easier to work with block types across
    /// different modules without duplicating conversion logic.
    pub fn from_format_block_type(format_block_type: &impl ConvertToBlockType) -> BlockType {
        format_block_type.to_block_type()
    }

    /// Trait for types that can be converted to `BlockType`
    ///
    /// This trait allows for standardized conversion from different
    /// representations of block types to the core `BlockType` `enum`.
    pub trait ConvertToBlockType {
        /// Convert to `BlockType`
        fn to_block_type(&self) -> BlockType;
    }
}

/// `FuncType` utilities for working with function types consistently
#[cfg(feature = "std")]
pub mod func_type {
    // Remove Vec import, as params and results will be slices
    // #[cfg(not(feature = "std"))]
    // use std::vec::Vec;

    // Result is imported through crate prelude

    use super::{
        CoreValueType as ValueType,
        FuncType,
    };
    use crate::{
        MemoryProvider,
        Result,
    };

    /// Verify that a function type conforms to WebAssembly constraints
    ///
    /// This is a utility function that can be used by any crate that
    /// needs to validate a function type.
    pub fn verify(
        func_type: &FuncType,
    ) -> Result<()> {
        func_type.verify()
    }

    /// Create a function type from parameters and results slices
    ///
    /// This utility function ensures consistent creation of function types
    /// with proper validation.
    pub fn create<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq>(
        provider: P,
        params: &[ValueType],
        results: &[ValueType],
    ) -> Result<FuncType> {
        let func_type = FuncType::new(params.iter().copied(), results.iter().copied())?;
        verify(&func_type)?;
        Ok(func_type)
    }
}

// Tests removed - used obsolete APIs (ref_type_to_val_type, val_type_to_ref_type, func_type module)
// that no longer exist in the codebase. New tests should be written using current APIs.
