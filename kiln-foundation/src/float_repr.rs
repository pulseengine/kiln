// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Wrapper types for f32 and f64 ensuring bit-pattern based equality and
//! hashing, suitable for use in WebAssembly type definitions.

use core::hash::{
    Hash,
    Hasher,
};

// Use Result and Error types from this crate's prelude or error module
use crate::prelude::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use crate::{
    traits::{
        BytesWriter,
        Checksummable,
        FromBytes,
        LittleEndian,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
};

/// Wrapper for f32 that implements Hash, `PartialEq`, and Eq based on bit
/// patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FloatBits32(pub u32);

impl FloatBits32 {
    /// Represents a canonical Not-a-Number (`NaN`) value for f32.
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

    /// Creates a `FloatBits32` from an `f32` value (alias for `from_float`).
    #[must_use]
    pub fn from_f32(val: f32) -> Self {
        Self::from_float(val)
    }

    /// Converts this `FloatBits32` to an `f32` value (alias for `value`).
    #[must_use]
    pub const fn to_f32(self) -> f32 {
        self.value()
    }

    /// Returns the little-endian byte representation of this `FloatBits32`.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl Hash for FloatBits32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Checksummable for FloatBits32 {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for FloatBits32 {
    #[inline]
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream, // PStream is not used for primitive types
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, _provider)
    }
}

impl FromBytes for FloatBits32 {
    #[inline]
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream, // PStream is not used for primitive types
    ) -> wrt_error::Result<Self> {
        let val = u32::from_bytes_with_provider(reader, _provider)?;
        Ok(FloatBits32(val))
    }
}

/// Wrapper for f64 that implements Hash, `PartialEq`, and Eq based on bit
/// patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FloatBits64(pub u64);

impl FloatBits64 {
    /// Represents a canonical Not-a-Number (`NaN`) value for f64.
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

    /// Creates a `FloatBits64` from an `f64` value (alias for `from_float`).
    #[must_use]
    pub fn from_f64(val: f64) -> Self {
        Self::from_float(val)
    }

    /// Converts this `FloatBits64` to an `f64` value (alias for `value`).
    #[must_use]
    pub const fn to_f64(self) -> f64 {
        self.value()
    }

    /// Returns the little-endian byte representation of this `FloatBits64`.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
}

impl Hash for FloatBits64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Checksummable for FloatBits64 {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for FloatBits64 {
    #[inline]
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream, // PStream is not used for primitive types
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, _provider)
    }
}

impl FromBytes for FloatBits64 {
    #[inline]
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream, // PStream is not used for primitive types
    ) -> wrt_error::Result<Self> {
        let val = u64::from_bytes_with_provider(reader, _provider)?;
        Ok(FloatBits64(val))
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
                "Failed to convert bytes to array",
            )
        })?;
        Ok(FloatBits32(u32::from_le_bytes(arr)))
    }

    fn write_le_bytes<W: BytesWriter>(&self, writer: &mut W) -> wrt_error::Result<()> {
        self.0.write_le_bytes(writer)
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
                "Failed to convert bytes to array",
            )
        })?;
        Ok(FloatBits64(u64::from_le_bytes(arr)))
    }

    fn write_le_bytes<W: BytesWriter>(&self, writer: &mut W) -> wrt_error::Result<()> {
        self.0.write_le_bytes(writer)
    }
}
