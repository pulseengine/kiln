// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! x86_64 SIMD implementation using SSE2 and AVX2
//!
//! This module provides optimized SIMD implementations for x86_64 processors.
//! It supports SSE2 (baseline for x86_64) and AVX2 for enhanced performance.

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use super::{
    SimdLevel,
    SimdProvider,
};

/// x86_64 SIMD provider with SSE2 and AVX2 support
#[derive(Debug, Clone)]
pub struct X86SimdProvider {
    level:    SimdLevel,
    has_sse2: bool,
    has_avx2: bool,
}

impl X86SimdProvider {
    /// Create a new SSE2-only provider
    pub const fn new_sse2() -> Self {
        Self {
            level:    SimdLevel::Basic,
            has_sse2: true,
            has_avx2: false,
        }
    }

    /// Create a new AVX2 provider (includes SSE2)
    pub const fn new_avx2() -> Self {
        Self {
            level:    SimdLevel::Advanced,
            has_sse2: true,
            has_avx2: true,
        }
    }
}

impl SimdProvider for X86SimdProvider {
    fn simd_level(&self) -> SimdLevel {
        self.level
    }

    fn is_available(&self) -> bool {
        // Runtime check could be added here, but we trust the constructor
        self.has_sse2
    }

    // i8x16 operations
    fn v128_i8x16_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_add_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_sub_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let result = _mm_sub_epi8(zero, a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    // i16x8 operations
    fn v128_i16x8_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_add_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_sub_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_mullo_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let result = _mm_sub_epi16(zero, a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    // i32x4 operations
    fn v128_i32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_add_epi32(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_sub_epi32(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            // SSE2 doesn't have _mm_mullo_epi32, use SSE4.1 if available or emulate
            #[cfg(target_feature = "sse4.1")]
            let result = _mm_mullo_epi32(a_vec, b_vec);

            #[cfg(not(target_feature = "sse4.1"))]
            let result = {
                // Emulate with mul_epu32 and shuffles
                let a_even = _mm_shuffle_epi32(a_vec, 0xA0); // 0b10100000
                let b_even = _mm_shuffle_epi32(b_vec, 0xA0);
                let even_prod = _mm_mul_epu32(a_even, b_even);

                let a_odd = _mm_shuffle_epi32(a_vec, 0xF5); // 0b11110101
                let b_odd = _mm_shuffle_epi32(b_vec, 0xF5);
                let odd_prod = _mm_mul_epu32(a_odd, b_odd);

                let even_32 = _mm_shuffle_epi32(even_prod, 0x08); // 0b00001000
                let odd_32 = _mm_shuffle_epi32(odd_prod, 0x08);
                _mm_unpacklo_epi32(even_32, odd_32)
            };

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let result = _mm_sub_epi32(zero, a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    // i64x2 operations
    fn v128_i64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_add_epi64(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_sub_epi64(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have direct i64 multiply, need to emulate
        let mut result = [0u8; 16];

        // Extract i64 values
        let a0 = i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let a1 = i64::from_le_bytes([a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]]);
        let b0 = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let b1 = i64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);

        // Multiply
        let r0 = a0.wrapping_mul(b0);
        let r1 = a1.wrapping_mul(b1);

        // Store back
        result[0..8].copy_from_slice(&r0.to_le_bytes());
        result[8..16].copy_from_slice(&r1.to_le_bytes());

        result
    }

    fn v128_i64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let result = _mm_sub_epi64(zero, a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    // f32x4 operations
    fn v128_f32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_add_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_sub_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_mul_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_div_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            // Negate by XORing with sign bit mask
            let sign_mask = _mm_set1_ps(-0.0);
            let result = _mm_xor_ps(a_vec, sign_mask);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let result = _mm_sqrt_ps(a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    // f64x2 operations
    fn v128_f64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_add_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_sub_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_mul_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_div_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            // Negate by XORing with sign bit mask
            let sign_mask = _mm_set1_pd(-0.0);
            let result = _mm_xor_pd(a_vec, sign_mask);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let result = _mm_sqrt_pd(a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    // Bitwise operations
    fn v128_not(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let all_ones = _mm_set1_epi32(-1);
            let result = _mm_xor_si128(a_vec, all_ones);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_and(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_and_si128(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_or(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_or_si128(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_xor(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_xor_si128(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_andnot(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            // _mm_andnot_si128 computes NOT(a) AND b
            // We want a AND NOT(b), so swap arguments
            let result = _mm_andnot_si128(b_vec, a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_bitselect(&self, a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let c_vec = _mm_loadu_si128(c.as_ptr() as *const __m128i);

            // v128.bitselect: (a & c) | (b & ~c)
            let a_and_c = _mm_and_si128(a_vec, c_vec);
            let b_and_not_c = _mm_andnot_si128(c_vec, b_vec);
            let result = _mm_or_si128(a_and_c, b_and_not_c);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    // Test operations
    fn v128_any_true(&self, a: &[u8; 16]) -> bool {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let cmp = _mm_cmpeq_epi8(a_vec, zero);
            // If all bytes are zero, movemask will be 0xFFFF
            let mask = _mm_movemask_epi8(cmp);
            mask != 0xFFFF
        }
    }

    fn v128_i8x16_all_true(&self, a: &[u8; 16]) -> bool {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let cmp = _mm_cmpeq_epi8(a_vec, zero);
            // If any byte is zero, movemask will have at least one bit set
            let mask = _mm_movemask_epi8(cmp);
            mask == 0
        }
    }

    fn v128_i16x8_all_true(&self, a: &[u8; 16]) -> bool {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let cmp = _mm_cmpeq_epi16(a_vec, zero);
            // Convert 16-bit comparison to byte mask
            let mask = _mm_movemask_epi8(cmp);
            // Each i16 produces 2 bits in the mask
            mask == 0
        }
    }

    fn v128_i32x4_all_true(&self, a: &[u8; 16]) -> bool {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let cmp = _mm_cmpeq_epi32(a_vec, zero);
            // Use floating-point movemask for 32-bit values
            let mask = _mm_movemask_ps(_mm_castsi128_ps(cmp));
            mask == 0
        }
    }

    fn v128_i64x2_all_true(&self, a: &[u8; 16]) -> bool {
        // SSE2 doesn't have _mm_cmpeq_epi64, check manually
        let a0 = i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let a1 = i64::from_le_bytes([a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]]);
        a0 != 0 && a1 != 0
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
            result[offset..offset + 2].copy_from_slice(&bytes);
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
        result[0..8].copy_from_slice(&bytes);
        result[8..16].copy_from_slice(&bytes);
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
        result[0..8].copy_from_slice(&bytes);
        result[8..16].copy_from_slice(&bytes);
        result
    }

    // Comparison operations - i8x16
    fn v128_i8x16_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpeq_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let eq_result = self.v128_i8x16_eq(a, b);
        self.v128_not(&eq_result)
    }

    fn v128_i8x16_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmplt_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have unsigned compare, simulate by adjusting sign
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let sign_flip = _mm_set1_epi8(-128_i8);
            let a_adj = _mm_xor_si128(a_vec, sign_flip);
            let b_adj = _mm_xor_si128(b_vec, sign_flip);
            let result = _mm_cmplt_epi8(a_adj, b_adj);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpgt_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_i8x16_lt_u(b, a)
    }

    fn v128_i8x16_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i8x16_gt_s(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i8x16_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i8x16_gt_u(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i8x16_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i8x16_lt_s(a, b);
        self.v128_not(&lt_result)
    }

    fn v128_i8x16_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i8x16_lt_u(a, b);
        self.v128_not(&lt_result)
    }

    // Comparison operations - i16x8
    fn v128_i16x8_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpeq_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let eq_result = self.v128_i16x8_eq(a, b);
        self.v128_not(&eq_result)
    }

    fn v128_i16x8_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmplt_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let sign_flip = _mm_set1_epi16(-32768_i16);
            let a_adj = _mm_xor_si128(a_vec, sign_flip);
            let b_adj = _mm_xor_si128(b_vec, sign_flip);
            let result = _mm_cmplt_epi16(a_adj, b_adj);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpgt_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_i16x8_lt_u(b, a)
    }

    fn v128_i16x8_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i16x8_gt_s(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i16x8_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i16x8_gt_u(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i16x8_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i16x8_lt_s(a, b);
        self.v128_not(&lt_result)
    }

    fn v128_i16x8_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i16x8_lt_u(a, b);
        self.v128_not(&lt_result)
    }

    // Comparison operations - i32x4
    fn v128_i32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpeq_epi32(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let eq_result = self.v128_i32x4_eq(a, b);
        self.v128_not(&eq_result)
    }

    fn v128_i32x4_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmplt_epi32(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let sign_flip = _mm_set1_epi32(-2147483648_i32);
            let a_adj = _mm_xor_si128(a_vec, sign_flip);
            let b_adj = _mm_xor_si128(b_vec, sign_flip);
            let result = _mm_cmplt_epi32(a_adj, b_adj);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_cmpgt_epi32(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_i32x4_lt_u(b, a)
    }

    fn v128_i32x4_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i32x4_gt_s(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i32x4_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i32x4_gt_u(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i32x4_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i32x4_lt_s(a, b);
        self.v128_not(&lt_result)
    }

    fn v128_i32x4_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i32x4_lt_u(a, b);
        self.v128_not(&lt_result)
    }

    // Comparison operations - i64x2 (limited in SSE2)
    fn v128_i64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // Manual comparison for i64 since SSE2 doesn't have _mm_cmpeq_epi64
        let a0 = i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let a1 = i64::from_le_bytes([a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]]);
        let b0 = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let b1 = i64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);

        let r0 = if a0 == b0 { -1i64 } else { 0i64 };
        let r1 = if a1 == b1 { -1i64 } else { 0i64 };

        let mut result = [0u8; 16];
        result[0..8].copy_from_slice(&r0.to_le_bytes());
        result[8..16].copy_from_slice(&r1.to_le_bytes());
        result
    }

    fn v128_i64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let eq_result = self.v128_i64x2_eq(a, b);
        self.v128_not(&eq_result)
    }

    fn v128_i64x2_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let a0 = i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let a1 = i64::from_le_bytes([a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]]);
        let b0 = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let b1 = i64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);

        let r0 = if a0 < b0 { -1i64 } else { 0i64 };
        let r1 = if a1 < b1 { -1i64 } else { 0i64 };

        let mut result = [0u8; 16];
        result[0..8].copy_from_slice(&r0.to_le_bytes());
        result[8..16].copy_from_slice(&r1.to_le_bytes());
        result
    }

    fn v128_i64x2_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_i64x2_lt_s(b, a)
    }

    fn v128_i64x2_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let gt_result = self.v128_i64x2_gt_s(a, b);
        self.v128_not(&gt_result)
    }

    fn v128_i64x2_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        let lt_result = self.v128_i64x2_lt_s(a, b);
        self.v128_not(&lt_result)
    }

    // Comparison operations - f32x4
    fn v128_f32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmpeq_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmpneq_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmplt_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmpgt_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmple_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_cmpge_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    // Comparison operations - f64x2
    fn v128_f64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmpeq_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmpneq_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmplt_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmpgt_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmple_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_cmpge_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    // Additional arithmetic operations - absolute values
    fn v128_i8x16_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let neg = _mm_sub_epi8(zero, a_vec);
            let result = _mm_max_epi8(a_vec, neg);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let zero = _mm_setzero_si128();
            let neg = _mm_sub_epi16(zero, a_vec);
            let result = _mm_max_epi16(a_vec, neg);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have _mm_max_epi32, use conditional select
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let abs_val = val.abs();
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
            let abs_val = val.abs();
            result[offset..offset + 8].copy_from_slice(&abs_val.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            // Clear sign bit by ANDing with positive infinity
            let sign_mask = _mm_set1_ps(f32::from_bits(0x7FFFFFFF));
            let result = _mm_and_ps(a_vec, sign_mask);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_min_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_ps(a.as_ptr() as *const f32);
            let b_vec = _mm_loadu_ps(b.as_ptr() as *const f32);
            let result = _mm_max_ps(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // Pseudo-min: propagate NaN from right operand
        self.v128_f32x4_min(a, b)
    }

    fn v128_f32x4_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // Pseudo-max: propagate NaN from right operand
        self.v128_f32x4_max(a, b)
    }

    fn v128_f64x2_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            // Clear sign bit by ANDing with positive infinity
            let sign_mask = _mm_set1_pd(f64::from_bits(0x7FFFFFFFFFFFFFFF));
            let result = _mm_and_pd(a_vec, sign_mask);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_min_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_pd(a.as_ptr() as *const f64);
            let b_vec = _mm_loadu_pd(b.as_ptr() as *const f64);
            let result = _mm_max_pd(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_pd(output.as_mut_ptr() as *mut f64, result);
            output
        }
    }

    fn v128_f64x2_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_f64x2_min(a, b)
    }

    fn v128_f64x2_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.v128_f64x2_max(a, b)
    }

    // Integer min/max operations
    fn v128_i8x16_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_min_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_min_epu8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_max_epi8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i8x16_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_max_epu8(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_min_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have _mm_min_epu16, simulate it
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let sign_flip = _mm_set1_epi16(-32768_i16);
            let a_adj = _mm_xor_si128(a_vec, sign_flip);
            let b_adj = _mm_xor_si128(b_vec, sign_flip);
            let min_adj = _mm_min_epi16(a_adj, b_adj);
            let result = _mm_xor_si128(min_adj, sign_flip);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let result = _mm_max_epi16(a_vec, b_vec);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i16x8_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr() as *const __m128i);
            let sign_flip = _mm_set1_epi16(-32768_i16);
            let a_adj = _mm_xor_si128(a_vec, sign_flip);
            let b_adj = _mm_xor_si128(b_vec, sign_flip);
            let max_adj = _mm_max_epi16(a_adj, b_adj);
            let result = _mm_xor_si128(max_adj, sign_flip);

            let mut output = [0u8; 16];
            _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, result);
            output
        }
    }

    fn v128_i32x4_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have _mm_min_epi32, do it manually
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

    // Conversion operations (simplified implementations)
    fn v128_i32x4_trunc_sat_f32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let f_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f_val = f32::from_bits(f_bits);
            let i_val = if f_val.is_nan() { 0 } else { f_val as i32 };
            result[offset..offset + 4].copy_from_slice(&i_val.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_trunc_sat_f32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let f_bits =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f_val = f32::from_bits(f_bits);
            let u_val = if f_val.is_nan() || f_val < 0.0 { 0u32 } else { f_val as u32 };
            result[offset..offset + 4].copy_from_slice(&u_val.to_le_bytes());
        }
        result
    }

    fn v128_f32x4_convert_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        unsafe {
            let a_vec = _mm_loadu_si128(a.as_ptr() as *const __m128i);
            let result = _mm_cvtepi32_ps(a_vec);

            let mut output = [0u8; 16];
            _mm_storeu_ps(output.as_mut_ptr() as *mut f32, result);
            output
        }
    }

    fn v128_f32x4_convert_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        // SSE2 doesn't have unsigned conversion, do it manually
        let mut result = [0u8; 16];
        for i in 0..4 {
            let offset = i * 4;
            let u_val =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let f_val = u_val as f32;
            result[offset..offset + 4].copy_from_slice(&f_val.to_bits().to_le_bytes());
        }
        result
    }

    // Placeholder implementations for complex conversion operations
    fn v128_i32x4_trunc_sat_f64x2_s_zero(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i32x4_trunc_sat_f64x2_u_zero(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_f64x2_convert_low_i32x4_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_f64x2_convert_low_i32x4_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_f32x4_demote_f64x2_zero(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_f64x2_promote_low_f32x4(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    // Narrow and extend operations (simplified implementations)
    fn v128_i8x16_narrow_i16x8_s(&self, _a: &[u8; 16], _b: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i8x16_narrow_i16x8_u(&self, _a: &[u8; 16], _b: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_narrow_i32x4_s(&self, _a: &[u8; 16], _b: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_narrow_i32x4_u(&self, _a: &[u8; 16], _b: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_extend_low_i8x16_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_extend_high_i8x16_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_extend_low_i8x16_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i16x8_extend_high_i8x16_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i32x4_extend_low_i16x8_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i32x4_extend_high_i16x8_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i32x4_extend_low_i16x8_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i32x4_extend_high_i16x8_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i64x2_extend_low_i32x4_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i64x2_extend_high_i32x4_s(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i64x2_extend_low_i32x4_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    fn v128_i64x2_extend_high_i32x4_u(&self, _a: &[u8; 16]) -> [u8; 16] {
        [0u8; 16] // Simplified placeholder
    }

    // Shift operations
    fn v128_i8x16_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = (count & 7) as u8; // Mask to 0-7 bits
        for i in 0..16 {
            result[i] = a[i].wrapping_shl(shift as u32);
        }
        result
    }

    fn v128_i8x16_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = (count & 7) as u8;
        for i in 0..16 {
            result[i] = ((a[i] as i8) >> shift) as u8;
        }
        result
    }

    fn v128_i8x16_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = (count & 7) as u8;
        for i in 0..16 {
            result[i] = a[i] >> shift;
        }
        result
    }

    fn v128_i16x8_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // Limit to valid range for i16
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val.wrapping_shl(shift);
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // Limit to valid range for i16
        for i in 0..8 {
            let offset = i * 2;
            let val = i16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val.wrapping_shr(shift);
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i16x8_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 15; // Limit to valid range for u16
        for i in 0..8 {
            let offset = i * 2;
            let val = u16::from_le_bytes([a[offset], a[offset + 1]]);
            let shifted = val.wrapping_shr(shift);
            result[offset..offset + 2].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // Limit to valid range for i32
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val.wrapping_shl(shift);
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // Limit to valid range for i32
        for i in 0..4 {
            let offset = i * 4;
            let val = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val.wrapping_shr(shift);
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i32x4_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 31; // Limit to valid range for u32
        for i in 0..4 {
            let offset = i * 4;
            let val = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
            let shifted = val.wrapping_shr(shift);
            result[offset..offset + 4].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 63; // Limit to valid range for i64
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
            let shifted = val.wrapping_shl(shift);
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        // SSE2 doesn't have _mm_srai_epi64, do it manually
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
            let shifted = val >> (count & 63);
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    fn v128_i64x2_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        let mut result = [0u8; 16];
        let shift = count & 63; // Limit to valid range for u64
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
            let shifted = val.wrapping_shr(shift);
            result[offset..offset + 8].copy_from_slice(&shifted.to_le_bytes());
        }
        result
    }

    // Advanced shuffle operations
    fn v128_i8x16_swizzle(&self, a: &[u8; 16], s: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            let idx = s[i] & 0x0F; // Mask to 0-15
            if idx < 16 {
                result[i] = a[idx as usize];
            } else {
                result[i] = 0;
            }
        }
        result
    }

    fn v128_i8x16_shuffle(&self, a: &[u8; 16], b: &[u8; 16], lanes: &[u8; 16]) -> [u8; 16] {
        let mut result = [0u8; 16];
        for i in 0..16 {
            let idx = lanes[i];
            if idx < 16 {
                result[i] = a[idx as usize];
            } else if idx < 32 {
                result[i] = b[(idx - 16) as usize];
            } else {
                result[i] = 0;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_x86_simd_i32x4_add() {
        let provider = X86SimdProvider::new_sse2();
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
    #[cfg(target_arch = "x86_64")]
    fn test_x86_simd_f32x4_mul() {
        let provider = X86SimdProvider::new_sse2();

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
    #[cfg(target_arch = "x86_64")]
    fn test_x86_simd_bitwise_ops() {
        let provider = X86SimdProvider::new_sse2();

        let a = [0xFF; 16];
        let b = [0xAA; 16];

        // Test AND
        let and_result = provider.v128_and(&a, &b);
        assert_eq!(and_result, [0xAA; 16]);

        // Test OR
        let or_result = provider.v128_or(&[0x00; 16], &b);
        assert_eq!(or_result, [0xAA; 16]);

        // Test XOR
        let xor_result = provider.v128_xor(&a, &b);
        assert_eq!(xor_result, [0x55; 16]);

        // Test NOT
        let not_result = provider.v128_not(&a);
        assert_eq!(not_result, [0x00; 16]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_x86_simd_any_true() {
        let provider = X86SimdProvider::new_sse2();

        assert!(!provider.v128_any_true(&[0x00; 16]));
        assert!(provider.v128_any_true(&[
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]));
    }
}
