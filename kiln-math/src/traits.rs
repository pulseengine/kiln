// Copyright (c) 2025 R T
// SPDX-License-Identifier: MIT
// Project: WRT
// Module: wrt-math::traits (SW-REQ-ID-TBD)

//! Common traits used within the wrt-math crate.

use crate::prelude::Result; // Specific import for WrtMathResult (aliased wrt_error::Result)

/// Trait for types that can be converted to/from little-endian byte
/// representation. TODO: This trait is also defined in wrt-foundation.
/// Consolidate to a common `wrt-traits` crate.
pub trait LittleEndian: Sized {
    /// Creates an instance from little-endian bytes.
    /// Returns an error if the byte slice has incorrect length or content.
    fn from_le_bytes(bytes: &[u8]) -> Result<Self>; // Use Result from prelude

    /// Converts the instance to little-endian bytes.
    /// Binary std/no_std choice
    #[cfg(feature = "std")]
    fn to_le_bytes(&self) -> Result<alloc::vec::Vec<u8>>; // Binary std/no_std choice
}
