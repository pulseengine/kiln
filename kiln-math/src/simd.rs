// WRT - wrt-math
// Module: SIMD Operations
// SW-REQ-ID: REQ_SIMD_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! SIMD (Single Instruction, Multiple Data) operations for WebAssembly v128
//! types.
//!
//! This module provides a high-level mathematical interface for WebAssembly
//! SIMD operations, delegating the actual implementation to the
//! platform-specific SIMD providers in wrt-platform.

use wrt_error::Result;
use wrt_platform::simd::SimdRuntime;

/// SIMD operation executor that uses the best available SIMD provider
pub struct SimdOperations {
    runtime: SimdRuntime,
}

impl SimdOperations {
    /// Create a new SIMD operations executor with runtime detection
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime: SimdRuntime::new(),
        }
    }

    /// Get the underlying SIMD runtime for direct access if needed
    #[must_use]
    pub fn runtime(&self) -> &SimdRuntime {
        &self.runtime
    }

    // --- Integer Arithmetic Operations ---

    /// Add two i8x16 vectors
    pub fn i8x16_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_add(a, b))
    }

    /// Subtract two i8x16 vectors
    pub fn i8x16_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_sub(a, b))
    }

    /// Negate an i8x16 vector
    pub fn i8x16_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_neg(a))
    }

    /// Add two i16x8 vectors
    pub fn i16x8_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i16x8_add(a, b))
    }

    /// Subtract two i16x8 vectors
    pub fn i16x8_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i16x8_sub(a, b))
    }

    /// Multiply two i16x8 vectors
    pub fn i16x8_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i16x8_mul(a, b))
    }

    /// Negate an i16x8 vector
    pub fn i16x8_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i16x8_neg(a))
    }

    /// Add two i32x4 vectors
    pub fn i32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_add(a, b))
    }

    /// Subtract two i32x4 vectors
    pub fn i32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_sub(a, b))
    }

    /// Multiply two i32x4 vectors
    pub fn i32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_mul(a, b))
    }

    /// Negate an i32x4 vector
    pub fn i32x4_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_neg(a))
    }

    /// Add two i64x2 vectors
    pub fn i64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i64x2_add(a, b))
    }

    /// Subtract two i64x2 vectors
    pub fn i64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i64x2_sub(a, b))
    }

    /// Multiply two i64x2 vectors
    pub fn i64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i64x2_mul(a, b))
    }

    /// Negate an i64x2 vector
    pub fn i64x2_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i64x2_neg(a))
    }

    // --- Floating-Point Arithmetic Operations ---

    /// Add two f32x4 vectors
    pub fn f32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_add(a, b))
    }

    /// Subtract two f32x4 vectors
    pub fn f32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_sub(a, b))
    }

    /// Multiply two f32x4 vectors
    pub fn f32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_mul(a, b))
    }

    /// Divide two f32x4 vectors
    pub fn f32x4_div(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_div(a, b))
    }

    /// Negate an f32x4 vector
    pub fn f32x4_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_neg(a))
    }

    /// Square root of f32x4 vector
    pub fn f32x4_sqrt(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_sqrt(a))
    }

    /// Add two f64x2 vectors
    pub fn f64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_add(a, b))
    }

    /// Subtract two f64x2 vectors
    pub fn f64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_sub(a, b))
    }

    /// Multiply two f64x2 vectors
    pub fn f64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_mul(a, b))
    }

    /// Divide two f64x2 vectors
    pub fn f64x2_div(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_div(a, b))
    }

    /// Negate an f64x2 vector
    pub fn f64x2_neg(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_neg(a))
    }

    /// Square root of f64x2 vector
    pub fn f64x2_sqrt(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_sqrt(a))
    }

    // --- Bitwise Operations ---

    /// Bitwise NOT of v128
    pub fn v128_not(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_not(a))
    }

    /// Bitwise AND of two v128 vectors
    pub fn v128_and(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_and(a, b))
    }

    /// Bitwise OR of two v128 vectors
    pub fn v128_or(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_or(a, b))
    }

    /// Bitwise XOR of two v128 vectors
    pub fn v128_xor(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_xor(a, b))
    }

    /// Bitwise AND-NOT of two v128 vectors (a AND NOT b)
    pub fn v128_andnot(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_andnot(a, b))
    }

    /// Bitwise select: use c as mask to select bits from a and b
    pub fn v128_bitselect(&self, a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_bitselect(a, b, c))
    }

    // --- Test Operations ---

    /// Test if any lane is true (non-zero)
    pub fn v128_any_true(&self, a: &[u8; 16]) -> Result<bool> {
        Ok(self.runtime.provider().v128_any_true(a))
    }

    /// Test if all i8x16 lanes are true (non-zero)
    pub fn i8x16_all_true(&self, a: &[u8; 16]) -> Result<bool> {
        Ok(self.runtime.provider().v128_i8x16_all_true(a))
    }

    /// Test if all i16x8 lanes are true (non-zero)
    pub fn i16x8_all_true(&self, a: &[u8; 16]) -> Result<bool> {
        Ok(self.runtime.provider().v128_i16x8_all_true(a))
    }

    /// Test if all i32x4 lanes are true (non-zero)
    pub fn i32x4_all_true(&self, a: &[u8; 16]) -> Result<bool> {
        Ok(self.runtime.provider().v128_i32x4_all_true(a))
    }

    /// Test if all i64x2 lanes are true (non-zero)
    pub fn i64x2_all_true(&self, a: &[u8; 16]) -> Result<bool> {
        Ok(self.runtime.provider().v128_i64x2_all_true(a))
    }

    // --- Comparison Operations ---

    /// Compare i8x16 vectors for equality
    pub fn i8x16_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_eq(a, b))
    }

    /// Compare i8x16 vectors for inequality
    pub fn i8x16_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_ne(a, b))
    }

    /// Compare i8x16 vectors for less than (signed)
    pub fn i8x16_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_lt_s(a, b))
    }

    /// Compare i8x16 vectors for less than (unsigned)
    pub fn i8x16_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_lt_u(a, b))
    }

    /// Compare i8x16 vectors for greater than (signed)
    pub fn i8x16_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_gt_s(a, b))
    }

    /// Compare i8x16 vectors for greater than (unsigned)
    pub fn i8x16_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_gt_u(a, b))
    }

    /// Compare i8x16 vectors for less than or equal (signed)
    pub fn i8x16_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_le_s(a, b))
    }

    /// Compare i8x16 vectors for less than or equal (unsigned)
    pub fn i8x16_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_le_u(a, b))
    }

    /// Compare i8x16 vectors for greater than or equal (signed)
    pub fn i8x16_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_ge_s(a, b))
    }

    /// Compare i8x16 vectors for greater than or equal (unsigned)
    pub fn i8x16_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_ge_u(a, b))
    }

    // Similar comparison operations for other types can be added as needed

    // --- Min/Max Operations ---

    /// Minimum of two i8x16 vectors (signed)
    pub fn i8x16_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_min_s(a, b))
    }

    /// Minimum of two i8x16 vectors (unsigned)
    pub fn i8x16_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_min_u(a, b))
    }

    /// Maximum of two i8x16 vectors (signed)
    pub fn i8x16_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_max_s(a, b))
    }

    /// Maximum of two i8x16 vectors (unsigned)
    pub fn i8x16_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_max_u(a, b))
    }

    /// Minimum of two f32x4 vectors
    pub fn f32x4_min(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_min(a, b))
    }

    /// Maximum of two f32x4 vectors
    pub fn f32x4_max(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_max(a, b))
    }

    /// Pseudo-minimum of two f32x4 vectors (returns b if either is NaN)
    pub fn f32x4_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_pmin(a, b))
    }

    /// Pseudo-maximum of two f32x4 vectors (returns b if either is NaN)
    pub fn f32x4_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_pmax(a, b))
    }

    // --- Absolute Value Operations ---

    /// Absolute value of i8x16 vector
    pub fn i8x16_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_abs(a))
    }

    /// Absolute value of i16x8 vector
    pub fn i16x8_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i16x8_abs(a))
    }

    /// Absolute value of i32x4 vector
    pub fn i32x4_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_abs(a))
    }

    /// Absolute value of i64x2 vector
    pub fn i64x2_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i64x2_abs(a))
    }

    /// Absolute value of f32x4 vector
    pub fn f32x4_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_abs(a))
    }

    /// Absolute value of f64x2 vector
    pub fn f64x2_abs(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f64x2_abs(a))
    }

    // --- Shift Operations ---

    /// Shift left i8x16 vector by count
    pub fn i8x16_shl(&self, a: &[u8; 16], count: u32) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_shl(a, count))
    }

    /// Shift right i8x16 vector by count (arithmetic/signed)
    pub fn i8x16_shr_s(&self, a: &[u8; 16], count: u32) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_shr_s(a, count))
    }

    /// Shift right i8x16 vector by count (logical/unsigned)
    pub fn i8x16_shr_u(&self, a: &[u8; 16], count: u32) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_shr_u(a, count))
    }

    // Similar shift operations for other integer types...

    // --- Conversion Operations ---

    /// Convert f32x4 to i32x4 with saturation (signed)
    pub fn i32x4_trunc_sat_f32x4_s(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_trunc_sat_f32x4_s(a))
    }

    /// Convert f32x4 to i32x4 with saturation (unsigned)
    pub fn i32x4_trunc_sat_f32x4_u(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i32x4_trunc_sat_f32x4_u(a))
    }

    /// Convert i32x4 to f32x4 (signed)
    pub fn f32x4_convert_i32x4_s(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_convert_i32x4_s(a))
    }

    /// Convert i32x4 to f32x4 (unsigned)
    pub fn f32x4_convert_i32x4_u(&self, a: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_f32x4_convert_i32x4_u(a))
    }

    // --- Advanced Operations ---

    /// Swizzle (rearrange) bytes in a vector
    pub fn i8x16_swizzle(&self, a: &[u8; 16], s: &[u8; 16]) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_swizzle(a, s))
    }

    /// Shuffle bytes from two vectors according to indices
    pub fn i8x16_shuffle(
        &self,
        a: &[u8; 16],
        b: &[u8; 16],
        indices: &[u8; 16],
    ) -> Result<[u8; 16]> {
        Ok(self.runtime.provider().v128_i8x16_shuffle(a, b, indices))
    }
}

impl Default for SimdOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_operations_creation() {
        let simd_ops = SimdOperations::new();
        // Just verify we can create the operations
        let _ = simd_ops.runtime();
    }

    #[test]
    fn test_i8x16_add() {
        let simd_ops = SimdOperations::new();
        let a = [1u8; 16];
        let b = [2u8; 16];
        let result = simd_ops.i8x16_add(&a, &b).unwrap();

        // All elements should be 3
        for &byte in &result {
            assert_eq!(byte, 3);
        }
    }

    #[test]
    fn test_v128_and() {
        let simd_ops = SimdOperations::new();
        let a = [0xFFu8; 16];
        let b = [0xF0u8; 16];
        let result = simd_ops.v128_and(&a, &b).unwrap();

        // All elements should be 0xF0
        for &byte in &result {
            assert_eq!(byte, 0xF0);
        }
    }

    #[test]
    fn test_v128_any_true() {
        let simd_ops = SimdOperations::new();

        // Test with all zeros
        let zeros = [0u8; 16];
        assert!(!simd_ops.v128_any_true(&zeros).unwrap());

        // Test with one non-zero
        let mut one_set = [0u8; 16];
        one_set[7] = 1;
        assert!(simd_ops.v128_any_true(&one_set).unwrap());
    }
}
