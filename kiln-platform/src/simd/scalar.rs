// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Scalar (non-SIMD) fallback implementations
//!
//! This module provides portable scalar implementations of all SIMD operations.
//! These implementations are used when no hardware SIMD support is available
//! or in no_std environments where we can't detect CPU features.

#![allow(clippy::cast_sign_loss)] // Mathematical operations require specific casting
#![allow(clippy::cast_possible_wrap)] // Mathematical operations require specific casting
#![allow(clippy::cast_lossless)] // Mathematical operations require specific casting
#![allow(clippy::if_not_else)] // SIMD comparisons use != for mask generation
#![allow(clippy::float_cmp)] // SIMD operations require exact floating point comparisons
#![allow(clippy::cast_abs_to_unsigned)] // SIMD absolute value operations
#![allow(clippy::too_many_lines)] // Test functions need comprehensive test cases

use super::{
    SimdLevel,
    SimdProvider,
};

// Simple sqrt implementation for no_std environments
#[cfg(not(feature = "std"))]
fn sqrt_f32(x: f32) -> f32 {
    if x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 || x == f32::INFINITY {
        return x;
    }

    // Newton-Raphson iteration for square root
    let mut guess = x * 0.5;
    for _ in 0..8 {
        guess = (guess + x / guess) * 0.5;
    }
    guess
}

#[cfg(not(feature = "std"))]
fn sqrt_f64(x: f64) -> f64 {
    if x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 || x == f64::INFINITY {
        return x;
    }

    // Newton-Raphson iteration for square root
    let mut guess = x * 0.5;
    for _ in 0..16 {
        guess = (guess + x / guess) * 0.5;
    }
    guess
}

/// Scalar SIMD provider that implements all operations without SIMD
/// instructions
#[derive(Debug, Clone)]
pub struct ScalarSimdProvider;

impl ScalarSimdProvider {
    /// Create a new scalar SIMD provider
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ScalarSimdProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdProvider for ScalarSimdProvider {
    fn simd_level(&self) -> SimdLevel {
        SimdLevel::None
    }

    fn is_available(&self) -> bool {
        true // Scalar is always available
    }

    // i8x16 operations
    fn v128_i8x16_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = (a[i] as i8).wrapping_add(b[i] as i8) as u8;
        }
        result
    }

    fn v128_i8x16_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = (a[i] as i8).wrapping_sub(b[i] as i8) as u8;
        }
        result
    }

    fn v128_i8x16_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = (-(a[i] as i8)) as u8;
        }
        result
    }

    // i16x8 operations
    fn v128_i16x8_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let sum = a_val.wrapping_add(b_val);
            result[offset..offset + 2].copy_from_slice(&sum.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let diff = a_val.wrapping_sub(b_val);
            result[offset..offset + 2].copy_from_slice(&diff.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let product = a_val.wrapping_mul(b_val);
            result[offset..offset + 2].copy_from_slice(&product.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let neg = a_val.wrapping_neg();
            result[offset..offset + 2].copy_from_slice(&neg.to_le_bytes());
        }
        result
    }

    // i32x4 operations
    fn v128_i32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let sum = a_val.wrapping_add(b_val);
            result[offset..offset + 4].copy_from_slice(&sum.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let diff = a_val.wrapping_sub(b_val);
            result[offset..offset + 4].copy_from_slice(&diff.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let product = a_val.wrapping_mul(b_val);
            result[offset..offset + 4].copy_from_slice(&product.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let neg = a_val.wrapping_neg();
            result[offset..offset + 4].copy_from_slice(&neg.to_le_bytes());
        }
        result
    }

    // i64x2 operations
    fn v128_i64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let sum = a_val.wrapping_add(b_val);
            result[offset..offset + 8].copy_from_slice(&sum.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let diff = a_val.wrapping_sub(b_val);
            result[offset..offset + 8].copy_from_slice(&diff.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let product = a_val.wrapping_mul(b_val);
            result[offset..offset + 8].copy_from_slice(&product.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let neg = a_val.wrapping_neg();
            result[offset..offset + 8].copy_from_slice(&neg.to_le_bytes());
        }
        result
    }

    // f32x4 operations
    fn v128_f32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let sum = a_val + b_val;
            result[offset..offset + 4].copy_from_slice(&sum.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let diff = a_val - b_val;
            result[offset..offset + 4].copy_from_slice(&diff.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let product = a_val * b_val;
            result[offset..offset + 4].copy_from_slice(&product.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let quotient = a_val / b_val;
            result[offset..offset + 4].copy_from_slice(&quotient.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let neg = -a_val;
            result[offset..offset + 4].copy_from_slice(&neg.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            #[cfg(feature = "std")]
            let sqrt_val = a_val.sqrt();
            #[cfg(not(feature = "std"))]
            let sqrt_val = sqrt_f32(a_val);
            result[offset..offset + 4].copy_from_slice(&sqrt_val.to_bits().to_le_bytes());
        }
        result
    }

    // f64x2 operations
    fn v128_f64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let sum = a_val + b_val;
            result[offset..offset + 8].copy_from_slice(&sum.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let diff = a_val - b_val;
            result[offset..offset + 8].copy_from_slice(&diff.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let product = a_val * b_val;
            result[offset..offset + 8].copy_from_slice(&product.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let quotient = a_val / b_val;
            result[offset..offset + 8].copy_from_slice(&quotient.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let neg = -a_val;
            result[offset..offset + 8].copy_from_slice(&neg.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            #[cfg(feature = "std")]
            let sqrt_val = a_val.sqrt();
            #[cfg(not(feature = "std"))]
            let sqrt_val = sqrt_f64(a_val);
            result[offset..offset + 8].copy_from_slice(&sqrt_val.to_bits().to_le_bytes());
        }
        result
    }

    // Bitwise operations
    fn v128_not(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = !a[i];
        }
        result
    }

    fn v128_and(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i] & b[i];
        }
        result
    }

    fn v128_or(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i] | b[i];
        }
        result
    }

    fn v128_xor(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i] ^ b[i];
        }
        result
    }

    fn v128_andnot(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i] & !b[i];
        }
        result
    }

    fn v128_bitselect(&self, a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            // v128.bitselect: (a & c) | (b & ~c)
            result[i] = (a[i] & c[i]) | (b[i] & !c[i]);
        }
        result
    }

    // Test operations
    fn v128_any_true(&self, a: &[u8; 16]) -> bool {
        for &byte in a {
            if byte != 0 {
                return true;
            }
        }
        false
    }

    fn v128_i8x16_all_true(&self, a: &[u8; 16]) -> bool {
        for &byte in a {
            if byte == 0 {
                return false;
            }
        }
        true
    }

    fn v128_i16x8_all_true(&self, a: &[u8; 16]) -> bool {
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            if val == 0 {
                return false;
            }
        }
        true
    }

    fn v128_i32x4_all_true(&self, a: &[u8; 16]) -> bool {
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            if val == 0 {
                return false;
            }
        }
        true
    }

    fn v128_i64x2_all_true(&self, a: &[u8; 16]) -> bool {
        for i in 0..2 {
            let offset = i * 8;
            let val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            if val == 0 {
                return false;
            }
        }
        true
    }

    // Lane access operations - extract_lane
    fn v128_i8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> i8 {
        if idx >= 16 {
            return 0;
        }
        a[idx as usize] as i8
    }

    fn v128_u8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> u8 {
        if idx >= 16 {
            return 0;
        }
        a[idx as usize]
    }

    fn v128_i16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> i16 {
        if idx >= 8 {
            return 0;
        }
        let offset = (idx as usize) * 2;
        i16::from_le_bytes([a[offset], a[offset + 1]])
    }

    fn v128_u16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> u16 {
        if idx >= 8 {
            return 0;
        }
        let offset = (idx as usize) * 2;
        u16::from_le_bytes([a[offset], a[offset + 1]])
    }

    fn v128_i32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> i32 {
        if idx >= 4 {
            return 0;
        }
        let offset = (idx as usize) * 4;
        i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]])
    }

    fn v128_i64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> i64 {
        if idx >= 2 {
            return 0;
        }
        let offset = (idx as usize) * 8;
        i64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ])
    }

    fn v128_f32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> f32 {
        if idx >= 4 {
            return 0.0;
        }
        let offset = (idx as usize) * 4;
        let bits = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        f32::from_bits(bits)
    }

    fn v128_f64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> f64 {
        if idx >= 2 {
            return 0.0;
        }
        let offset = (idx as usize) * 8;
        let bits = u64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        f64::from_bits(bits)
    }

    // Lane access operations - replace_lane
    fn v128_i8x16_replace_lane(&self, a: &[u8; 16], idx: u8, val: i8) -> [u8; 16] {
        let mut result = *a;
        if idx < 16 {
            result[idx as usize] = val as u8;
        }
        result
    }

    fn v128_i16x8_replace_lane(&self, a: &[u8; 16], idx: u8, val: i16) -> [u8; 16] {
        let mut result = *a;
        if idx < 8 {
            let offset = (idx as usize) * 2;
            let bytes = val.to_le_bytes();
            result[offset] = bytes[0];
            result[offset + 1] = bytes[1];
        }
        result
    }

    fn v128_i32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: i32) -> [u8; 16] {
        let mut result = *a;
        if idx < 4 {
            let offset = (idx as usize) * 4;
            let bytes = val.to_le_bytes();
            result[offset..offset + 4].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_i64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: i64) -> [u8; 16] {
        let mut result = *a;
        if idx < 2 {
            let offset = (idx as usize) * 8;
            let bytes = val.to_le_bytes();
            result[offset..offset + 8].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_f32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: f32) -> [u8; 16] {
        let mut result = *a;
        if idx < 4 {
            let offset = (idx as usize) * 4;
            let bytes = val.to_bits().to_le_bytes();
            result[offset..offset + 4].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_f64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: f64) -> [u8; 16] {
        let mut result = *a;
        if idx < 2 {
            let offset = (idx as usize) * 8;
            let bytes = val.to_bits().to_le_bytes();
            result[offset..offset + 8].copy_from_slice(&bytes);
        }
        result
    }

    // Splat operations (create vector from scalar)
    fn v128_i8x16_splat(&self, val: i8) -> [u8; 16] {
        [val as u8; 16]
    }

    fn v128_i16x8_splat(&self, val: i16) -> [u8; 16] {
        let mut result = [0u8; 16];
        let bytes = val.to_le_bytes();
        for i in 0..8 {
            let offset = i * 2;
            result[offset] = bytes[0];
            result[offset + 1] = bytes[1];
        }
        result
    }

    fn v128_i32x4_splat(&self, val: i32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let bytes = val.to_le_bytes();
        for i in 0..4 {
            let offset = i * 4;
            result[offset..offset + 4].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_i64x2_splat(&self, val: i64) -> [u8; 16] {
        let mut result = [0u8; 16];
        let bytes = val.to_le_bytes();
        for i in 0..2 {
            let offset = i * 8;
            result[offset..offset + 8].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_f32x4_splat(&self, val: f32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let bytes = val.to_bits().to_le_bytes();
        for i in 0..4 {
            let offset = i * 4;
            result[offset..offset + 4].copy_from_slice(&bytes);
        }
        result
    }

    fn v128_f64x2_splat(&self, val: f64) -> [u8; 16] {
        let mut result = [0u8; 16];
        let bytes = val.to_bits().to_le_bytes();
        for i in 0..2 {
            let offset = i * 8;
            result[offset..offset + 8].copy_from_slice(&bytes);
        }
        result
    }

    // Comparison operations - i8x16
    fn v128_i8x16_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] as i8 == b[i] as i8 { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] as i8 != b[i] as i8 { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if (a[i] as i8) < (b[i] as i8) { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] < b[i] { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if (a[i] as i8) > (b[i] as i8) { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] > b[i] { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if (a[i] as i8) <= (b[i] as i8) { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] <= b[i] { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if (a[i] as i8) >= (b[i] as i8) { 0xFF } else { 0x00 };
        }
        result
    }

    fn v128_i8x16_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = if a[i] >= b[i] { 0xFF } else { 0x00 };
        }
        result
    }

    // Comparison operations - i16x8 (similar pattern to i8x16)
    fn v128_i16x8_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val == b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val != b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val < b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val < b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val > b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val > b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val <= b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val <= b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val >= b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let mask: u16 = if a_val >= b_val { 0xFFFF } else { 0x0000 };
            result[offset..offset + 2].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // I'll continue with the other operations in the next edit to keep the
    // implementation manageable For now, let me add some basic stubs to get
    // compilation working

    // Comparison operations - i32x4 (basic implementation)
    fn v128_i32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val == b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // Comparison operations - i32x4 (implementing all operations)
    fn v128_i32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val != b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val < b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val < b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val > b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val > b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val <= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val <= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val >= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let mask: u32 = if a_val >= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // Comparison operations - i64x2
    fn v128_i64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val == b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val != b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val < b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val > b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val <= b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_val = i64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let mask: u64 = if a_val >= b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // f32x4 comparison operations
    fn v128_f32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val == b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val != b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val < b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val > b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val <= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);
            let mask: u32 = if a_val >= b_val { 0xFFFF_FFFF } else { 0x0000_0000 };
            result[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // f64x2 comparison operations
    fn v128_f64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val == b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val != b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f64x2_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val < b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f64x2_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val > b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f64x2_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val <= b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    fn v128_f64x2_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);
            let mask: u64 = if a_val >= b_val { 0xFFFF_FFFFFFFFFFFF } else { 0x0000_000000000000 };
            result[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
        }
        result
    }

    // Integer absolute value operations
    fn v128_i8x16_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            let val = a[i] as i8;
            // Handle overflow case for i8::MIN
            result[i] = if val == i8::MIN {
                128u8 // abs(i8::MIN) = 128 (as u8)
            } else {
                val.abs() as u8
            };
        }
        result
    }

    fn v128_i16x8_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let abs_val = if val == i16::MIN {
                32768u16 // abs(i16::MIN) = 32768 (as u16)
            } else {
                val.abs() as u16
            };
            result[offset..offset + 2].copy_from_slice(&abs_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let abs_val = if val == i32::MIN {
                2_147_483_648u32 // abs(i32::MIN) = 2147483648 (as u32)
            } else {
                val.abs() as u32
            };
            result[offset..offset + 4].copy_from_slice(&abs_val.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let abs_val = if val == i64::MIN {
                9_223_372_036_854_775_808u64 // abs(i64::MIN) =
                                             // 9223372036854775808 (as u64)
            } else {
                val.abs() as u64
            };
            result[offset..offset + 8].copy_from_slice(&abs_val.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let f32_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = f32::from_bits(f32_bits);
            let abs_val = f32_val.abs();
            result[offset..offset + 4].copy_from_slice(&abs_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);

            // IEEE 754 min: NaN propagation, -0.0 < +0.0
            let min_val = if a_val.is_nan() || b_val.is_nan() {
                f32::NAN
            } else if a_val == 0.0 && b_val == 0.0 {
                if a_val.is_sign_negative() {
                    a_val
                } else {
                    b_val
                }
            } else {
                a_val.min(b_val)
            };

            result[offset..offset + 4].copy_from_slice(&min_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);

            // IEEE 754 max: NaN propagation, +0.0 > -0.0
            let max_val = if a_val.is_nan() || b_val.is_nan() {
                f32::NAN
            } else if a_val == 0.0 && b_val == 0.0 {
                if a_val.is_sign_positive() {
                    a_val
                } else {
                    b_val
                }
            } else {
                a_val.max(b_val)
            };

            result[offset..offset + 4].copy_from_slice(&max_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);

            // Pseudo-min: returns b if either operand is NaN
            let pmin_val = if a_val.is_nan() || b_val.is_nan() {
                b_val
            } else if a_val < b_val {
                a_val
            } else {
                b_val
            };

            result[offset..offset + 4].copy_from_slice(&pmin_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_bits =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let a_val = f32::from_bits(a_bits);
            let b_val = f32::from_bits(b_bits);

            // Pseudo-max: returns b if either operand is NaN
            let pmax_val = if a_val.is_nan() || b_val.is_nan() {
                b_val
            } else if a_val > b_val {
                a_val
            } else {
                b_val
            };

            result[offset..offset + 4].copy_from_slice(&pmax_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let f64_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let f64_val = f64::from_bits(f64_bits);
            let abs_val = f64_val.abs();
            result[offset..offset + 8].copy_from_slice(&abs_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);

            // IEEE 754 min: NaN propagation, -0.0 < +0.0
            let min_val = if a_val.is_nan() || b_val.is_nan() {
                f64::NAN
            } else if a_val == 0.0 && b_val == 0.0 {
                if a_val.is_sign_negative() {
                    a_val
                } else {
                    b_val
                }
            } else {
                a_val.min(b_val)
            };

            result[offset..offset + 8].copy_from_slice(&min_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);

            // IEEE 754 max: NaN propagation, +0.0 > -0.0
            let max_val = if a_val.is_nan() || b_val.is_nan() {
                f64::NAN
            } else if a_val == 0.0 && b_val == 0.0 {
                if a_val.is_sign_positive() {
                    a_val
                } else {
                    b_val
                }
            } else {
                a_val.max(b_val)
            };

            result[offset..offset + 8].copy_from_slice(&max_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);

            // Pseudo-min: returns b if either operand is NaN
            let pmin_val = if a_val.is_nan() || b_val.is_nan() {
                b_val
            } else if a_val < b_val {
                a_val
            } else {
                b_val
            };

            result[offset..offset + 8].copy_from_slice(&pmin_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..2 {
            let offset = i * 8;
            let a_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let b_bits = u64::from_le_bytes([
                b[offset],
                b[offset + 1],
                b[offset + 2],
                b[offset + 3],
                b[offset + 4],
                b[offset + 5],
                b[offset + 6],
                b[offset + 7],
            ]);
            let a_val = f64::from_bits(a_bits);
            let b_val = f64::from_bits(b_bits);

            // Pseudo-max: returns b if either operand is NaN
            let pmax_val = if a_val.is_nan() || b_val.is_nan() {
                b_val
            } else if a_val > b_val {
                a_val
            } else {
                b_val
            };

            result[offset..offset + 8].copy_from_slice(&pmax_val.to_bits().to_le_bytes());
        }
        result
    }

    // Integer min/max operations - i8x16
    fn v128_i8x16_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            let a_val = a[i] as i8;
            let b_val = b[i] as i8;
            result[i] = a_val.min(b_val) as u8;
        }
        result
    }

    fn v128_i8x16_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i].min(b[i]);
        }
        result
    }

    fn v128_i8x16_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            let a_val = a[i] as i8;
            let b_val = b[i] as i8;
            result[i] = a_val.max(b_val) as u8;
        }
        result
    }

    fn v128_i8x16_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = a[i].max(b[i]);
        }
        result
    }

    // Integer min/max operations - i16x8
    fn v128_i16x8_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let min_val = a_val.min(b_val);
            result[offset..offset + 2].copy_from_slice(&min_val.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let min_val = a_val.min(b_val);
            result[offset..offset + 2].copy_from_slice(&min_val.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let max_val = a_val.max(b_val);
            result[offset..offset + 2].copy_from_slice(&max_val.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..8 {
            let offset = i * 2;
            let a_val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let b_val = u16::from_le_bytes([b[offset], b[offset + 1]]);
            let max_val = a_val.max(b_val);
            result[offset..offset + 2].copy_from_slice(&max_val.to_le_bytes());
        }
        result
    }

    // Integer min/max operations - i32x4
    fn v128_i32x4_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let min_val = a_val.min(b_val);
            result[offset..offset + 4].copy_from_slice(&min_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let min_val = a_val.min(b_val);
            result[offset..offset + 4].copy_from_slice(&min_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let max_val = a_val.max(b_val);
            result[offset..offset + 4].copy_from_slice(&max_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let a_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let b_val =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let max_val = a_val.max(b_val);
            result[offset..offset + 4].copy_from_slice(&max_val.to_le_bytes());
        }
        result
    }

    // Float-to-integer conversion with saturation
    fn v128_i32x4_trunc_sat_f32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let f32_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = f32::from_bits(f32_bits);

            // Saturating conversion: clamp to i32 range and handle NaN
            let i32_val = if f32_val.is_nan() {
                0i32
            } else if f32_val >= i32::MAX as f32 {
                i32::MAX
            } else if f32_val <= i32::MIN as f32 {
                i32::MIN
            } else {
                f32_val as i32
            };

            result[offset..offset + 4].copy_from_slice(&i32_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_trunc_sat_f32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let f32_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = f32::from_bits(f32_bits);

            // Saturating conversion: clamp to u32 range and handle NaN
            let u32_val = if f32_val.is_nan() || f32_val < 0.0 {
                0u32
            } else if f32_val >= u32::MAX as f32 {
                u32::MAX
            } else {
                f32_val as u32
            };

            result[offset..offset + 4].copy_from_slice(&u32_val.to_le_bytes());
        }
        result
    }

    // Integer-to-float conversion
    fn v128_f32x4_convert_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let i32_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = i32_val as f32;
            result[offset..offset + 4].copy_from_slice(&f32_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f32x4_convert_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let u32_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = u32_val as f32;
            result[offset..offset + 4].copy_from_slice(&f32_val.to_bits().to_le_bytes());
        }
        result
    }

    // f64-to-i32 conversion with saturation and zero-fill
    fn v128_i32x4_trunc_sat_f64x2_s_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert first two f64 lanes to i32, fill last two lanes with zero
        for i in 0..2 {
            let offset = i * 8;
            let f64_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let f64_val = f64::from_bits(f64_bits);

            // Saturating conversion: clamp to i32 range and handle NaN
            let i32_val = if f64_val.is_nan() {
                0i32
            } else if f64_val >= i32::MAX as f64 {
                i32::MAX
            } else if f64_val <= i32::MIN as f64 {
                i32::MIN
            } else {
                f64_val as i32
            };

            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&i32_val.to_le_bytes());
        }
        // Last two i32 lanes are already zeroed from initialization
        result
    }

    fn v128_i32x4_trunc_sat_f64x2_u_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert first two f64 lanes to u32, fill last two lanes with zero
        for i in 0..2 {
            let offset = i * 8;
            let f64_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let f64_val = f64::from_bits(f64_bits);

            // Saturating conversion: clamp to u32 range and handle NaN
            let u32_val = if f64_val.is_nan() || f64_val < 0.0 {
                0u32
            } else if f64_val >= u32::MAX as f64 {
                u32::MAX
            } else {
                f64_val as u32
            };

            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&u32_val.to_le_bytes());
        }
        // Last two i32 lanes are already zeroed from initialization
        result
    }

    // i32-to-f64 conversion of low lanes
    fn v128_f64x2_convert_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert first two i32 lanes to f64
        for i in 0..2 {
            let offset = i * 4;
            let i32_val =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f64_val = i32_val as f64;

            let result_offset = i * 8;
            result[result_offset..result_offset + 8]
                .copy_from_slice(&f64_val.to_bits().to_le_bytes());
        }
        result
    }

    fn v128_f64x2_convert_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert first two u32 lanes to f64
        for i in 0..2 {
            let offset = i * 4;
            let u32_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f64_val = u32_val as f64;

            let result_offset = i * 8;
            result[result_offset..result_offset + 8]
                .copy_from_slice(&f64_val.to_bits().to_le_bytes());
        }
        result
    }

    // f64-to-f32 demotion and f32-to-f64 promotion
    fn v128_f32x4_demote_f64x2_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert two f64 lanes to f32, fill last two lanes with zero
        for i in 0..2 {
            let offset = i * 8;
            let f64_bits = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let f64_val = f64::from_bits(f64_bits);
            let f32_val = f64_val as f32;

            let result_offset = i * 4;
            result[result_offset..result_offset + 4]
                .copy_from_slice(&f32_val.to_bits().to_le_bytes());
        }
        // Last two f32 lanes are already zeroed from initialization
        result
    }

    fn v128_f64x2_promote_low_f32x4(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Convert first two f32 lanes to f64
        for i in 0..2 {
            let offset = i * 4;
            let f32_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f32_val = f32::from_bits(f32_bits);
            let f64_val = f32_val as f64;

            let result_offset = i * 8;
            result[result_offset..result_offset + 8]
                .copy_from_slice(&f64_val.to_bits().to_le_bytes());
        }
        result
    }

    // Narrow operations - convert wider lanes to narrower with saturation
    fn v128_i8x16_narrow_i16x8_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Pack 8 i16 lanes from a into first 8 i8 lanes of result
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let narrow_val = val.clamp(i8::MIN as i16, i8::MAX as i16) as i8;
            result[i] = narrow_val as u8;
        }

        // Pack 8 i16 lanes from b into last 8 i8 lanes of result
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let narrow_val = val.clamp(i8::MIN as i16, i8::MAX as i16) as i8;
            result[i + 8] = narrow_val as u8;
        }

        result
    }

    fn v128_i8x16_narrow_i16x8_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Pack 8 i16 lanes from a into first 8 u8 lanes of result
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let narrow_val = val.clamp(0, u8::MAX as i16) as u8;
            result[i] = narrow_val;
        }

        // Pack 8 i16 lanes from b into last 8 u8 lanes of result
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([b[offset], b[offset + 1]]);
            let narrow_val = val.clamp(0, u8::MAX as i16) as u8;
            result[i + 8] = narrow_val;
        }

        result
    }

    fn v128_i16x8_narrow_i32x4_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Pack 4 i32 lanes from a into first 4 i16 lanes of result
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let narrow_val = val.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&narrow_val.to_le_bytes());
        }

        // Pack 4 i32 lanes from b into last 4 i16 lanes of result
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let narrow_val = val.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let result_offset = (i + 4) * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&narrow_val.to_le_bytes());
        }

        result
    }

    fn v128_i16x8_narrow_i32x4_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Pack 4 i32 lanes from a into first 4 u16 lanes of result
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let narrow_val = val.clamp(0, u16::MAX as i32) as u16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&narrow_val.to_le_bytes());
        }

        // Pack 4 i32 lanes from b into last 4 u16 lanes of result
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
            let narrow_val = val.clamp(0, u16::MAX as i32) as u16;
            let result_offset = (i + 4) * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&narrow_val.to_le_bytes());
        }

        result
    }

    // Extend operations - convert narrower lanes to wider with sign/zero extension
    fn v128_i16x8_extend_low_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 8 i8 lanes to i16
        for i in 0..8 {
            let val = a[i] as i8; // Sign-extend
            let extended_val = val as i16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i16x8_extend_high_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 8 i8 lanes to i16
        for i in 0..8 {
            let val = a[i + 8] as i8; // Sign-extend
            let extended_val = val as i16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i16x8_extend_low_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 8 u8 lanes to u16
        for i in 0..8 {
            let val = a[i]; // Zero-extend
            let extended_val = val as u16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i16x8_extend_high_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 8 u8 lanes to u16
        for i in 0..8 {
            let val = a[i + 8]; // Zero-extend
            let extended_val = val as u16;
            let result_offset = i * 2;
            result[result_offset..result_offset + 2].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i32x4_extend_low_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 4 i16 lanes to i32
        for i in 0..4 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]); // Sign-extend
            let extended_val = val as i32;
            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i32x4_extend_high_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 4 i16 lanes to i32
        for i in 0..4 {
            let offset = (i + 4) * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]); // Sign-extend
            let extended_val = val as i32;
            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i32x4_extend_low_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 4 u16 lanes to u32
        for i in 0..4 {
            let offset = i * 2;
            let val = u16::from_le_bytes([a[offset], a[offset + 1]]); // Zero-extend
            let extended_val = val as u32;
            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i32x4_extend_high_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 4 u16 lanes to u32
        for i in 0..4 {
            let offset = (i + 4) * 2;
            let val = u16::from_le_bytes([a[offset], a[offset + 1]]); // Zero-extend
            let extended_val = val as u32;
            let result_offset = i * 4;
            result[result_offset..result_offset + 4].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i64x2_extend_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 2 i32 lanes to i64
        for i in 0..2 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]); // Sign-extend
            let extended_val = val as i64;
            let result_offset = i * 8;
            result[result_offset..result_offset + 8].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i64x2_extend_high_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 2 i32 lanes to i64
        for i in 0..2 {
            let offset = (i + 2) * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]); // Sign-extend
            let extended_val = val as i64;
            let result_offset = i * 8;
            result[result_offset..result_offset + 8].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i64x2_extend_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend first 2 u32 lanes to u64
        for i in 0..2 {
            let offset = i * 4;
            let val = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]); // Zero-extend
            let extended_val = val as u64;
            let result_offset = i * 8;
            result[result_offset..result_offset + 8].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    fn v128_i64x2_extend_high_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // Extend last 2 u32 lanes to u64
        for i in 0..2 {
            let offset = (i + 2) * 4;
            let val = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]); // Zero-extend
            let extended_val = val as u64;
            let result_offset = i * 8;
            result[result_offset..result_offset + 8].copy_from_slice(&extended_val.to_le_bytes());
        }

        result
    }

    // Shift operations - i8x16
    fn v128_i8x16_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 7; // i8 shift is modulo 8
        for i in 0..16 {
            result[i] = a[i] << shift;
        }
        result
    }

    fn v128_i8x16_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 7; // i8 shift is modulo 8
        for i in 0..16 {
            let val = a[i] as i8;
            result[i] = (val >> shift) as u8;
        }
        result
    }

    fn v128_i8x16_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 7; // i8 shift is modulo 8
        for i in 0..16 {
            result[i] = a[i] >> shift;
        }
        result
    }

    // Shift operations - i16x8
    fn v128_i16x8_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // i16 shift is modulo 16
        for i in 0..8 {
            let offset = i * 2;
            let val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val << shift;
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // i16 shift is modulo 16
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val >> shift;
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // i16 shift is modulo 16
        for i in 0..8 {
            let offset = i * 2;
            let val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val >> shift;
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    // Shift operations - i32x4
    fn v128_i32x4_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // i32 shift is modulo 32
        for i in 0..4 {
            let offset = i * 4;
            let val = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val << shift;
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // i32 shift is modulo 32
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val >> shift;
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // i32 shift is modulo 32
        for i in 0..4 {
            let offset = i * 4;
            let val = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val >> shift;
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    // Shift operations - i64x2
    fn v128_i64x2_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 63; // i64 shift is modulo 64
        for i in 0..2 {
            let offset = i * 8;
            let val = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let shifted = val << shift;
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 63; // i64 shift is modulo 64
        for i in 0..2 {
            let offset = i * 8;
            let val = i64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let shifted = val >> shift;
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 63; // i64 shift is modulo 64
        for i in 0..2 {
            let offset = i * 8;
            let val = u64::from_le_bytes([
                a[offset],
                a[offset + 1],
                a[offset + 2],
                a[offset + 3],
                a[offset + 4],
                a[offset + 5],
                a[offset + 6],
                a[offset + 7],
            ]);
            let shifted = val >> shift;
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    // Advanced shuffle operation stubs
    fn v128_i8x16_swizzle(&self, a: &[u8; 16], s: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // For each output lane, read index from s and use it to select from a
        for i in 0..16 {
            let index = s[i];
            if index < 16 {
                result[i] = a[index as usize];
            } else {
                result[i] = 0; // Out of bounds indices return 0
            }
        }

        result
    }

    fn v128_i8x16_shuffle(&self, a: &[u8; 16], b: &[u8; 16], lanes: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];

        // For each output lane, read index from lanes and select from a or b
        for i in 0..16 {
            let index = lanes[i];
            if index < 16 {
                result[i] = a[index as usize];
            } else if index < 32 {
                result[i] = b[(index - 16) as usize];
            } else {
                result[i] = 0; // Out of bounds indices return 0
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_i32x4_add() {
        let provider = ScalarSimdProvider::new();
        let a = [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]; // [1, 2, 3, 4]
        let b = [5, 0, 0, 0, 6, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0]; // [5, 6, 7, 8]
        let result = provider.v128_i32x4_add(&a, &b);

        // Expected: [6, 8, 10, 12]
        assert_eq!(&result[0..4], &[6, 0, 0, 0]);
        assert_eq!(&result[4..8], &[8, 0, 0, 0]);
        assert_eq!(&result[8..12], &[10, 0, 0, 0]);
        assert_eq!(&result[12..16], &[12, 0, 0, 0]);
    }

    #[test]
    fn test_scalar_f32x4_mul() {
        let provider = ScalarSimdProvider::new();

        // Create f32x4 vectors [2.0, 3.0, 4.0, 5.0] and [0.5, 2.0, 0.25, 0.2]
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];

        a[0..4].copy_from_slice(&2.0f32.to_bits().to_le_bytes());
        a[4..8].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
        a[8..12].copy_from_slice(&4.0f32.to_bits().to_le_bytes());
        a[12..16].copy_from_slice(&5.0f32.to_bits().to_le_bytes());

        b[0..4].copy_from_slice(&0.5f32.to_bits().to_le_bytes());
        b[4..8].copy_from_slice(&2.0f32.to_bits().to_le_bytes());
        b[8..12].copy_from_slice(&0.25f32.to_bits().to_le_bytes());
        b[12..16].copy_from_slice(&0.2f32.to_bits().to_le_bytes());

        let result = provider.v128_f32x4_mul(&a, &b);

        // Extract results
        let r0 = f32::from_bits(u32::from_le_bytes([
            result[0], result[1], result[2], result[3],
        ]));
        let r1 = f32::from_bits(u32::from_le_bytes([
            result[4], result[5], result[6], result[7],
        ]));
        let r2 = f32::from_bits(u32::from_le_bytes([
            result[8], result[9], result[10], result[11],
        ]));
        let r3 = f32::from_bits(u32::from_le_bytes([
            result[12], result[13], result[14], result[15],
        ]));

        // Expected: [1.0, 6.0, 1.0, 1.0]
        assert_eq!(r0, 1.0);
        assert_eq!(r1, 6.0);
        assert_eq!(r2, 1.0);
        assert_eq!(r3, 1.0);
    }

    #[test]
    fn test_scalar_bitwise_ops() {
        let provider = ScalarSimdProvider::new();

        let a = [0xFF; 16];
        let b = [0xAA; 16];

        // Test NOT
        let not_result = provider.v128_not(&a);
        assert_eq!(not_result, [0x00; 16]);

        // Test AND
        let and_result = provider.v128_and(&a, &b);
        assert_eq!(and_result, [0xAA; 16]);

        // Test OR
        let or_result = provider.v128_or(&[0x00; 16], &b);
        assert_eq!(or_result, [0xAA; 16]);

        // Test XOR
        let xor_result = provider.v128_xor(&a, &b);
        assert_eq!(xor_result, [0x55; 16]);
    }

    #[test]
    fn test_scalar_any_true() {
        let provider = ScalarSimdProvider::new();

        assert!(!provider.v128_any_true(&[0x00; 16]));
        assert!(provider.v128_any_true(&[
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]));
    }

    #[test]
    fn test_scalar_all_true() {
        let provider = ScalarSimdProvider::new();

        // i8x16 all true
        assert!(provider.v128_i8x16_all_true(&[0xFF; 16]));
        assert!(!provider.v128_i8x16_all_true(&[
            0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF
        ]));

        // i32x4 all true
        let mut all_ones = [0u8; 16];
        for i in 0..4 {
            all_ones[i * 4..(i + 1) * 4].copy_from_slice(&(-1i32).to_le_bytes());
        }
        assert!(provider.v128_i32x4_all_true(&all_ones));

        let mut one_zero = all_ones;
        one_zero[8..12].copy_from_slice(&0i32.to_le_bytes());
        assert!(!provider.v128_i32x4_all_true(&one_zero));
    }

    #[test]
    fn test_scalar_float_comparisons() {
        let provider = ScalarSimdProvider::new();

        // Create f32x4 vectors [1.0, 2.0, 3.0, 4.0] and [1.0, 1.5, 4.0, 3.5]
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];

        a[0..4].copy_from_slice(&1.0f32.to_bits().to_le_bytes());
        a[4..8].copy_from_slice(&2.0f32.to_bits().to_le_bytes());
        a[8..12].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
        a[12..16].copy_from_slice(&4.0f32.to_bits().to_le_bytes());

        b[0..4].copy_from_slice(&1.0f32.to_bits().to_le_bytes());
        b[4..8].copy_from_slice(&1.5f32.to_bits().to_le_bytes());
        b[8..12].copy_from_slice(&4.0f32.to_bits().to_le_bytes());
        b[12..16].copy_from_slice(&3.5f32.to_bits().to_le_bytes());

        // Test eq: [true, false, false, false] -> [0xFFFF_FFFF, 0x0000_0000,
        // 0x0000_0000, 0x0000_0000]
        let eq_result = provider.v128_f32x4_eq(&a, &b);
        assert_eq!(&eq_result[0..4], &0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(&eq_result[4..8], &0x0000_0000u32.to_le_bytes());
        assert_eq!(&eq_result[8..12], &0x0000_0000u32.to_le_bytes());
        assert_eq!(&eq_result[12..16], &0x0000_0000u32.to_le_bytes());

        // Test lt: [false, false, true, false] -> [0x0000_0000, 0x0000_0000,
        // 0xFFFF_FFFF, 0x0000_0000]
        let lt_result = provider.v128_f32x4_lt(&a, &b);
        assert_eq!(&lt_result[0..4], &0x0000_0000u32.to_le_bytes());
        assert_eq!(&lt_result[4..8], &0x0000_0000u32.to_le_bytes());
        assert_eq!(&lt_result[8..12], &0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(&lt_result[12..16], &0x0000_0000u32.to_le_bytes());
    }

    #[test]
    fn test_scalar_integer_abs() {
        let provider = ScalarSimdProvider::new();

        // Test i8x16 abs with negative values
        let a = [
            0xFF, 0xFE, 0x01, 0x7F, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]; // [-1, -2, 1, 127, -128, ...]
        let result = provider.v128_i8x16_abs(&a);
        assert_eq!(result[0], 1); // abs(-1) = 1
        assert_eq!(result[1], 2); // abs(-2) = 2
        assert_eq!(result[2], 1); // abs(1) = 1
        assert_eq!(result[3], 127); // abs(127) = 127
        assert_eq!(result[4], 128); // abs(-128) = 128 (as u8)

        // Test i32x4 abs
        let mut a = [0u8; 16];
        a[0..4].copy_from_slice(&(-42i32).to_le_bytes());
        a[4..8].copy_from_slice(&42i32.to_le_bytes());
        a[8..12].copy_from_slice(&(-1000i32).to_le_bytes());
        a[12..16].copy_from_slice(&0i32.to_le_bytes());

        let result = provider.v128_i32x4_abs(&a);

        assert_eq!(
            i32::from_le_bytes([result[0], result[1], result[2], result[3]]),
            42
        );
        assert_eq!(
            i32::from_le_bytes([result[4], result[5], result[6], result[7]]),
            42
        );
        assert_eq!(
            i32::from_le_bytes([result[8], result[9], result[10], result[11]]),
            1000
        );
        assert_eq!(
            i32::from_le_bytes([result[12], result[13], result[14], result[15]]),
            0
        );
    }

    #[test]
    fn test_scalar_integer_min_max() {
        let provider = ScalarSimdProvider::new();

        // Test i32x4 min/max
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];

        a[0..4].copy_from_slice(&10i32.to_le_bytes());
        a[4..8].copy_from_slice(&(-5i32).to_le_bytes());
        a[8..12].copy_from_slice(&100i32.to_le_bytes());
        a[12..16].copy_from_slice(&0i32.to_le_bytes());

        b[0..4].copy_from_slice(&5i32.to_le_bytes());
        b[4..8].copy_from_slice(&(-10i32).to_le_bytes());
        b[8..12].copy_from_slice(&200i32.to_le_bytes());
        b[12..16].copy_from_slice(&(-1i32).to_le_bytes());

        // Test signed min: min(10,5)=5, min(-5,-10)=-10, min(100,200)=100, min(0,-1)=-1
        let min_result = provider.v128_i32x4_min_s(&a, &b);
        assert_eq!(
            i32::from_le_bytes([min_result[0], min_result[1], min_result[2], min_result[3]]),
            5
        );
        assert_eq!(
            i32::from_le_bytes([min_result[4], min_result[5], min_result[6], min_result[7]]),
            -10
        );
        assert_eq!(
            i32::from_le_bytes([min_result[8], min_result[9], min_result[10], min_result[11]]),
            100
        );
        assert_eq!(
            i32::from_le_bytes([
                min_result[12],
                min_result[13],
                min_result[14],
                min_result[15]
            ]),
            -1
        );

        // Test signed max: max(10,5)=10, max(-5,-10)=-5, max(100,200)=200, max(0,-1)=0
        let max_result = provider.v128_i32x4_max_s(&a, &b);
        assert_eq!(
            i32::from_le_bytes([max_result[0], max_result[1], max_result[2], max_result[3]]),
            10
        );
        assert_eq!(
            i32::from_le_bytes([max_result[4], max_result[5], max_result[6], max_result[7]]),
            -5
        );
        assert_eq!(
            i32::from_le_bytes([max_result[8], max_result[9], max_result[10], max_result[11]]),
            200
        );
        assert_eq!(
            i32::from_le_bytes([
                max_result[12],
                max_result[13],
                max_result[14],
                max_result[15]
            ]),
            0
        );
    }

    #[test]
    fn test_scalar_shift_operations() {
        let provider = ScalarSimdProvider::new();

        // Test i32x4 shift operations
        let mut a = [0u8; 16];
        a[0..4].copy_from_slice(&8i32.to_le_bytes()); // 8
        a[4..8].copy_from_slice(&(-16i32).to_le_bytes()); // -16
        a[8..12].copy_from_slice(&1i32.to_le_bytes()); // 1
        a[12..16].copy_from_slice(&(-1i32).to_le_bytes()); // -1

        // Test left shift by 2
        let shl_result = provider.v128_i32x4_shl(&a, 2);
        assert_eq!(
            i32::from_le_bytes([shl_result[0], shl_result[1], shl_result[2], shl_result[3]]),
            32
        ); // 8 << 2 = 32
        assert_eq!(
            i32::from_le_bytes([shl_result[4], shl_result[5], shl_result[6], shl_result[7]]),
            -64
        ); // -16 << 2 = -64
        assert_eq!(
            i32::from_le_bytes([shl_result[8], shl_result[9], shl_result[10], shl_result[11]]),
            4
        ); // 1 << 2 = 4
        assert_eq!(
            i32::from_le_bytes([
                shl_result[12],
                shl_result[13],
                shl_result[14],
                shl_result[15]
            ]),
            -4
        ); // -1 << 2 = -4

        // Test signed right shift by 2
        let shr_s_result = provider.v128_i32x4_shr_s(&a, 2);
        assert_eq!(
            i32::from_le_bytes([
                shr_s_result[0],
                shr_s_result[1],
                shr_s_result[2],
                shr_s_result[3]
            ]),
            2
        ); // 8 >> 2 = 2
        assert_eq!(
            i32::from_le_bytes([
                shr_s_result[4],
                shr_s_result[5],
                shr_s_result[6],
                shr_s_result[7]
            ]),
            -4
        ); // -16 >> 2 = -4
        assert_eq!(
            i32::from_le_bytes([
                shr_s_result[8],
                shr_s_result[9],
                shr_s_result[10],
                shr_s_result[11]
            ]),
            0
        ); // 1 >> 2 = 0
        assert_eq!(
            i32::from_le_bytes([
                shr_s_result[12],
                shr_s_result[13],
                shr_s_result[14],
                shr_s_result[15]
            ]),
            -1
        ); // -1 >> 2 = -1

        // Test unsigned right shift by 2
        let shr_unsigned_result = provider.v128_i32x4_shr_u(&a, 2);
        assert_eq!(
            u32::from_le_bytes([
                shr_unsigned_result[0],
                shr_unsigned_result[1],
                shr_unsigned_result[2],
                shr_unsigned_result[3]
            ]),
            2
        ); // 8 >> 2 = 2
        assert_eq!(
            u32::from_le_bytes([
                shr_unsigned_result[4],
                shr_unsigned_result[5],
                shr_unsigned_result[6],
                shr_unsigned_result[7]
            ]),
            1_073_741_820
        ); // (u32)(-16) >> 2
        assert_eq!(
            u32::from_le_bytes([
                shr_unsigned_result[8],
                shr_unsigned_result[9],
                shr_unsigned_result[10],
                shr_unsigned_result[11]
            ]),
            0
        ); // 1 >> 2 = 0
        assert_eq!(
            u32::from_le_bytes([
                shr_unsigned_result[12],
                shr_unsigned_result[13],
                shr_unsigned_result[14],
                shr_unsigned_result[15]
            ]),
            1_073_741_823
        ); // (u32)(-1) >> 2

        // Test shift count modulo (shift by 34 should be same as shift by 2 for i32)
        let shl_mod_result = provider.v128_i32x4_shl(&a, 34);
        assert_eq!(shl_result, shl_mod_result);
    }
}
