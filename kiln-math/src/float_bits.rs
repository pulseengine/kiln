// WRT - wrt-math
// Module: Float Bit Patterns
// SW-REQ-ID: REQ_018 (Partially, as type representation)
//
// Copyright (c) 2024 Your Name/Organization
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Wrapper types for f32 and f64 ensuring bit-pattern based equality and
//! hashing.

// Binary std/no_std choice
#[cfg(feature = "std")]
use alloc::vec::Vec;
use core::{
    // cmp::Ordering, // Unused import
    hash::{
        Hash,
        Hasher,
    },
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
}; /* Changed ErrorKind to
    * ErrorCategory, Added
    * Error */

use crate::traits::LittleEndian; // Import the trait

/// Wrapper for f32 that implements Hash, `PartialEq`, and Eq based on bit
/// patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FloatBits32(pub u32);

impl FloatBits32 {
    /// Represents a canonical Not-a-Number (`NaN`) value for f32.
    /// The specific bit pattern for canonical `NaN` can vary, but this is a
    /// common one. (Sign bit 0, exponent all 1s, significand MSB 1, rest 0)
    pub const NAN: Self = FloatBits32(0x7fc0_0000);

    /// Creates a new `FloatBits32` from an `f32` value.
    #[must_use]
    pub fn from_float(val: f32) -> Self {
        Self(val.to_bits())
    }

    /// Returns the `f32` value represented by this `FloatBits32`.
    #[must_use]
    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }

    /// Returns the underlying `u32` bits of this `FloatBits32`.
    #[must_use]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Creates a `FloatBits32` from raw `u32` bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

impl Hash for FloatBits32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Wrapper for f64 that implements Hash, `PartialEq`, and Eq based on bit
/// patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FloatBits64(pub u64);

impl FloatBits64 {
    /// Represents a canonical Not-a-Number (`NaN`) value for f64.
    /// The specific bit pattern for canonical `NaN` can vary, but this is a
    /// common one. (Sign bit 0, exponent all 1s, significand MSB 1, rest 0)
    pub const NAN: Self = FloatBits64(0x7ff8_0000_0000_0000);

    /// Creates a new `FloatBits64` from an `f64` value.
    #[must_use]
    pub fn from_float(val: f64) -> Self {
        Self(val.to_bits())
    }

    /// Returns the `f64` value represented by this `FloatBits64`.
    #[must_use]
    pub const fn value(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Returns the underlying `u64` bits of this `FloatBits64`.
    #[must_use]
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    /// Creates a `FloatBits64` from raw `u64` bits.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

impl Hash for FloatBits64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl LittleEndian for FloatBits32 {
    fn from_le_bytes(bytes: &[u8]) -> wrt_error::Result<Self> {
        if bytes.len() != 4 {
            return Err(Error::runtime_execution_error(
                "Invalid byte length for f32",
            ));
        }
        let arr: [u8; 4] = bytes.try_into().map_err(|_| {
            Error::new(
                ErrorCategory::System,
                codes::CONVERSION_ERROR,
                "Failed to convert bytes to float bits",
            )
        })?;
        Ok(FloatBits32(u32::from_le_bytes(arr)))
    }

    #[cfg(feature = "std")]
    fn to_le_bytes(&self) -> wrt_error::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }
}

impl LittleEndian for FloatBits64 {
    fn from_le_bytes(bytes: &[u8]) -> wrt_error::Result<Self> {
        if bytes.len() != 8 {
            return Err(Error::runtime_execution_error(
                "Invalid byte length for f64",
            ));
        }
        let arr: [u8; 8] = bytes.try_into().map_err(|_| {
            Error::new(
                ErrorCategory::System,
                codes::CONVERSION_ERROR,
                "Failed to convert bytes to float bits",
            )
        })?;
        Ok(FloatBits64(u64::from_le_bytes(arr)))
    }

    #[cfg(feature = "std")]
    fn to_le_bytes(&self) -> wrt_error::Result<Vec<u8>> {
        Ok(self.0.to_le_bytes().to_vec())
    }
}
