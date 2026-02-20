// WRT - wrt-foundation
// Module: Verification Utilities
// SW-REQ-ID: REQ_VERIFY_001
// SW-REQ-ID: REQ_VERIFY_003
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Verification utilities for ensuring data integrity
//!
//! This module provides tools for verifying data integrity
//! through checksums and validation functions, with a focus on
//! functional safety.

// Conditionally import from std or core
#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(not(feature = "std"))]
use core::sync::atomic::{
    AtomicU32,
    Ordering,
};
#[cfg(feature = "std")]
use std::fmt;
#[cfg(feature = "std")]
use std::sync::atomic::{
    AtomicU32,
    Ordering,
};

use crate::traits::{
    FromBytes,
    ReadStream,
    SerializationError,
    ToBytes,
    WriteStream,
};

/// Defines the level of verification to apply for checksums and other checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Default, Hash)]
#[repr(u8)]
pub enum VerificationLevel {
    /// No verification checks are performed.
    Off       = 0,
    /// Basic verification checks (e.g., length checks).
    Basic     = 1,
    /// Standard verification checks (reasonable default).
    #[default]
    Standard  = 2,
    /// Full verification including checksums on every relevant operation.
    Full      = 3,
    /// Perform verification checks based on sampling.
    Sampling  = 4,
    /// Perform redundant checks in addition to sampling or full checks.
    Redundant = 5,
}

impl VerificationLevel {
    /// Alias for Off - no verification checks are performed.
    pub const NONE: Self = Self::Off;
    /// Alias for Full - critical safety-level verification.
    pub const CRITICAL: Self = Self::Full;
}

impl VerificationLevel {
    /// Returns the byte representation of the verification level.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Check if verification should be performed for a given operation
    ///
    /// For sampling verification, this will return true with a probability
    /// based on the importance of the operation.
    pub fn should_verify(&self, operation_importance: u8) -> bool {
        match self {
            Self::Off => false,
            Self::Basic => operation_importance > 0, // Basic verifies if there's any importance
            Self::Standard => operation_importance >= 50, // Standard verifies important operations
            Self::Sampling => {
                // Simple sampling strategy: verify based on importance
                // Higher importance = higher chance of being verified
                // This is deterministic based on a counter to ensure
                // predictable behavior for WCET analysis
                static COUNTER: AtomicU32 = AtomicU32::new(0);

                // Get the current counter value and increment it atomically
                let current = COUNTER.fetch_add(1, Ordering::Relaxed);
                (current % 256) < u32::from(operation_importance)
            },
            Self::Full => true,
            Self::Redundant => true, // Redundant implies Full for standard verification checks
        }
    }

    /// Check if full redundant verification should be performed
    #[must_use]
    pub fn should_verify_redundant(&self) -> bool {
        // This check is specifically for *additional* redundant checks,
        // not the standard checks covered by should_verify.
        // Test `test_should_validate_redundant` asserts that Full should pass this.
        matches!(self, Self::Full | Self::Redundant)
    }

    /// Check if statistics tracking should be enabled
    #[must_use]
    pub fn should_track_stats(&self) -> bool {
        // Only track stats for Full and Redundant levels to minimize overhead
        matches!(self, Self::Full | Self::Redundant)
    }

    /// Check if initialization checks should be performed
    #[must_use]
    pub fn should_check_init(&self) -> bool {
        // Check initialization for Basic and higher verification levels
        !matches!(self, Self::Off)
    }
}

impl ToBytes for VerificationLevel {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(self.to_byte())
    }
}

impl FromBytes for VerificationLevel {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(VerificationLevel::Off),
            1 => Ok(VerificationLevel::Basic),
            2 => Ok(VerificationLevel::Standard),
            3 => Ok(VerificationLevel::Full),
            4 => Ok(VerificationLevel::Sampling),
            5 => Ok(VerificationLevel::Redundant),
            _ => Err(SerializationError::InvalidEnumValue.into()),
        }
    }
}

/// A simple checksum implementation for data integrity verification
///
/// This is a simple Adler32-like checksum that doesn't rely on external
/// dependencies. For functional safety purposes, checksums help detect
/// memory corruption and verify data integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checksum {
    /// First component of checksum (sum of bytes)
    a: u32,
    /// Second component of checksum (sum of first component)
    b: u32,
}

impl Default for Checksum {
    fn default() -> Self {
        Self::new()
    }
}

impl Checksum {
    /// Create a new empty checksum
    #[must_use]
    pub const fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    /// Reset the checksum to its initial state.
    pub fn reset(&mut self) {
        self.a = 1;
        self.b = 0;
    }

    /// Compute a checksum for a byte slice
    #[must_use]
    pub fn compute(data: &[u8]) -> Self {
        let mut checksum = Self::new();
        for &byte in data {
            checksum.update(byte);
        }
        checksum
    }

    /// Update the checksum with a single byte
    pub fn update(&mut self, byte: u8) {
        self.a = (self.a + u32::from(byte)) % 65_521;
        self.b = (self.b + self.a) % 65_521;
    }

    /// Update the checksum with multiple bytes
    pub fn update_slice(&mut self, data: &[u8]) {
        for &byte in data {
            self.update(byte);
        }
    }

    /// Get the checksum value as a u32
    #[must_use]
    pub fn value(&self) -> u32 {
        (self.b << 16) | self.a
    }

    /// Verify that a checksum matches the expected value
    #[must_use]
    pub fn verify(&self, expected: &Self) -> bool {
        self == expected
    }

    /// Create a checksum from a u32 value
    #[must_use]
    pub fn from_value(value: u32) -> Self {
        Self {
            a: value & 0xFFFF,
            b: value >> 16,
        }
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.value())
    }
}

impl ToBytes for Checksum {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream, // Provider not directly used for u32
    ) -> wrt_error::Result<()> {
        writer.write_u32_le(self.value())
    }
}

impl FromBytes for Checksum {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream, // Provider not directly used for u32
    ) -> wrt_error::Result<Self> {
        let value = reader.read_u32_le()?;
        Ok(Checksum::from_value(value))
    }
}

impl core::hash::Hash for Checksum {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.a.hash(state);
        self.b.hash(state);
    }
}

/// A simple hash function for creating type identifiers
///
/// This hash function is designed for functional safety,
/// providing deterministic results for type validation.
pub struct Hasher {
    /// Internal state of the hasher
    state: u32,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher {
    /// Create a new hasher with initial state
    #[must_use]
    pub fn new() -> Self {
        Self { state: 0x811c_9dc5 } // FNV-1a initial value
    }

    /// Update the hash state with bytes
    pub fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u32::from(byte);
            self.state = self.state.wrapping_mul(16_777_619); // FNV-1a prime
        }
    }

    /// Finalize and return the hash value
    #[must_use]
    pub fn finalize(self) -> u32 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_empty() {
        let checksum = Checksum::compute(&[]);
        assert_eq!(checksum.a, 1);
        assert_eq!(checksum.b, 0);
    }

    #[test]
    fn test_checksum_values() {
        let data = b"WebAssembly";
        let checksum = Checksum::compute(data);
        // Verify against known good value
        assert_eq!(checksum.value(), 422_511_711);
    }

    #[test]
    fn test_hasher() {
        let mut hasher = Hasher::new();
        hasher.update(b"test");
        let hash = hasher.finalize();
        // Verify against known good value
        assert_eq!(hash, 0xafd0_71e5);
    }
}
