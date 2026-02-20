// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! AArch64 SIMD implementation using ARM NEON
//!
//! This module provides optimized SIMD implementations for AArch64 processors
//! using ARM NEON instructions. All operations are implemented using intrinsics
//! for maximum performance.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

use super::{
    scalar::ScalarSimdProvider,
    SimdLevel,
    SimdProvider,
};

/// AArch64 SIMD provider with NEON support
#[derive(Debug, Clone)]
pub struct AArch64SimdProvider {
    level:           SimdLevel,
    has_neon:        bool,
    #[allow(dead_code)]
    has_sve:         bool,
    scalar_fallback: ScalarSimdProvider,
}

impl AArch64SimdProvider {
    /// Create a new NEON-only provider
    pub const fn new_neon() -> Self {
        Self {
            level:           SimdLevel::Basic,
            has_neon:        true,
            has_sve:         false,
            scalar_fallback: ScalarSimdProvider::new(),
        }
    }

    /// Create a new SVE provider (includes NEON)
    pub const fn new_sve() -> Self {
        Self {
            level:           SimdLevel::Advanced,
            has_neon:        true,
            has_sve:         true,
            scalar_fallback: ScalarSimdProvider::new(),
        }
    }
}

impl SimdProvider for AArch64SimdProvider {
    fn simd_level(&self) -> SimdLevel {
        self.level
    }

    fn is_available(&self) -> bool {
        // NEON is mandatory on AArch64
        self.has_neon
    }

    // Arithmetic operations - i8x16
    fn v128_i8x16_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u8(a.as_ptr());
            let b_vec = vld1q_u8(b.as_ptr());
            let result = vaddq_u8(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u8(output.as_mut_ptr(), result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i8x16_add(a, b)
        }
    }

    fn v128_i8x16_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u8(a.as_ptr());
            let b_vec = vld1q_u8(b.as_ptr());
            let result = vsubq_u8(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u8(output.as_mut_ptr(), result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i8x16_sub(a, b)
        }
    }

    fn v128_i8x16_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_s8(a.as_ptr() as *const i8);
            let result = vnegq_s8(a_vec);

            let mut output = [0u8; 16];
            vst1q_s8(output.as_mut_ptr() as *mut i8, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i8x16_neg(a)
        }
    }

    // Arithmetic operations - i16x8
    fn v128_i16x8_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u16(a.as_ptr() as *const u16);
            let b_vec = vld1q_u16(b.as_ptr() as *const u16);
            let result = vaddq_u16(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u16(output.as_mut_ptr() as *mut u16, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i16x8_add(a, b)
        }
    }

    fn v128_i16x8_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u16(a.as_ptr() as *const u16);
            let b_vec = vld1q_u16(b.as_ptr() as *const u16);
            let result = vsubq_u16(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u16(output.as_mut_ptr() as *mut u16, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i16x8_sub(a, b)
        }
    }

    fn v128_i16x8_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u16(a.as_ptr() as *const u16);
            let b_vec = vld1q_u16(b.as_ptr() as *const u16);
            let result = vmulq_u16(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u16(output.as_mut_ptr() as *mut u16, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i16x8_mul(a, b)
        }
    }

    fn v128_i16x8_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_s16(a.as_ptr() as *const i16);
            let result = vnegq_s16(a_vec);

            let mut output = [0u8; 16];
            vst1q_s16(output.as_mut_ptr() as *mut i16, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i16x8_neg(a)
        }
    }

    // Arithmetic operations - i32x4
    fn v128_i32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u32(a.as_ptr() as *const u32);
            let b_vec = vld1q_u32(b.as_ptr() as *const u32);
            let result = vaddq_u32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u32(output.as_mut_ptr() as *mut u32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i32x4_add(a, b)
        }
    }

    fn v128_i32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u32(a.as_ptr() as *const u32);
            let b_vec = vld1q_u32(b.as_ptr() as *const u32);
            let result = vsubq_u32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u32(output.as_mut_ptr() as *mut u32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i32x4_sub(a, b)
        }
    }

    fn v128_i32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u32(a.as_ptr() as *const u32);
            let b_vec = vld1q_u32(b.as_ptr() as *const u32);
            let result = vmulq_u32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u32(output.as_mut_ptr() as *mut u32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i32x4_mul(a, b)
        }
    }

    fn v128_i32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_s32(a.as_ptr() as *const i32);
            let result = vnegq_s32(a_vec);

            let mut output = [0u8; 16];
            vst1q_s32(output.as_mut_ptr() as *mut i32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i32x4_neg(a)
        }
    }

    // Arithmetic operations - i64x2
    fn v128_i64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u64(a.as_ptr() as *const u64);
            let b_vec = vld1q_u64(b.as_ptr() as *const u64);
            let result = vaddq_u64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u64(output.as_mut_ptr() as *mut u64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i64x2_add(a, b)
        }
    }

    fn v128_i64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_u64(a.as_ptr() as *const u64);
            let b_vec = vld1q_u64(b.as_ptr() as *const u64);
            let result = vsubq_u64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_u64(output.as_mut_ptr() as *mut u64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i64x2_sub(a, b)
        }
    }

    fn v128_i64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        // i64 multiplication is not available in base NEON, delegate to scalar
        self.scalar_fallback.v128_i64x2_mul(a, b)
    }

    fn v128_i64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_s64(a.as_ptr() as *const i64);
            let result = vnegq_s64(a_vec);

            let mut output = [0u8; 16];
            vst1q_s64(output.as_mut_ptr() as *mut i64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_i64x2_neg(a)
        }
    }

    // Arithmetic operations - f32x4
    fn v128_f32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let b_vec = vld1q_f32(b.as_ptr() as *const f32);
            let result = vaddq_f32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_add(a, b)
        }
    }

    fn v128_f32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let b_vec = vld1q_f32(b.as_ptr() as *const f32);
            let result = vsubq_f32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_sub(a, b)
        }
    }

    fn v128_f32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let b_vec = vld1q_f32(b.as_ptr() as *const f32);
            let result = vmulq_f32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_mul(a, b)
        }
    }

    fn v128_f32x4_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let b_vec = vld1q_f32(b.as_ptr() as *const f32);
            let result = vdivq_f32(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_div(a, b)
        }
    }

    fn v128_f32x4_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let result = vnegq_f32(a_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_neg(a)
        }
    }

    fn v128_f32x4_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f32(a.as_ptr() as *const f32);
            let result = vsqrtq_f32(a_vec);

            let mut output = [0u8; 16];
            vst1q_f32(output.as_mut_ptr() as *mut f32, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f32x4_sqrt(a)
        }
    }

    // Arithmetic operations - f64x2
    fn v128_f64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let b_vec = vld1q_f64(b.as_ptr() as *const f64);
            let result = vaddq_f64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_add(a, b)
        }
    }

    fn v128_f64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let b_vec = vld1q_f64(b.as_ptr() as *const f64);
            let result = vsubq_f64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_sub(a, b)
        }
    }

    fn v128_f64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let b_vec = vld1q_f64(b.as_ptr() as *const f64);
            let result = vmulq_f64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_mul(a, b)
        }
    }

    fn v128_f64x2_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let b_vec = vld1q_f64(b.as_ptr() as *const f64);
            let result = vdivq_f64(a_vec, b_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_div(a, b)
        }
    }

    fn v128_f64x2_neg(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let result = vnegq_f64(a_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_neg(a)
        }
    }

    fn v128_f64x2_sqrt(&self, a: &[u8; 16]) -> [u8; 16] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let a_vec = vld1q_f64(a.as_ptr() as *const f64);
            let result = vsqrtq_f64(a_vec);

            let mut output = [0u8; 16];
            vst1q_f64(output.as_mut_ptr() as *mut f64, result);
            output
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            self.scalar_fallback.v128_f64x2_sqrt(a)
        }
    }

    // Delegate all remaining operations to the scalar provider for now
    // This allows the AArch64 provider to be functional while we implement
    // the most common operations with NEON intrinsics first

    // Bitwise operations
    fn v128_not(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_not(a)
    }

    fn v128_and(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_and(a, b)
    }

    fn v128_or(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_or(a, b)
    }

    fn v128_xor(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_xor(a, b)
    }

    fn v128_andnot(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_andnot(a, b)
    }

    fn v128_bitselect(&self, a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_bitselect(a, b, c)
    }

    // Test operations
    fn v128_any_true(&self, a: &[u8; 16]) -> bool {
        self.scalar_fallback.v128_any_true(a)
    }

    fn v128_i8x16_all_true(&self, a: &[u8; 16]) -> bool {
        self.scalar_fallback.v128_i8x16_all_true(a)
    }

    fn v128_i16x8_all_true(&self, a: &[u8; 16]) -> bool {
        self.scalar_fallback.v128_i16x8_all_true(a)
    }

    fn v128_i32x4_all_true(&self, a: &[u8; 16]) -> bool {
        self.scalar_fallback.v128_i32x4_all_true(a)
    }

    fn v128_i64x2_all_true(&self, a: &[u8; 16]) -> bool {
        self.scalar_fallback.v128_i64x2_all_true(a)
    }

    // Lane access operations - extract_lane
    fn v128_i8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> i8 {
        self.scalar_fallback.v128_i8x16_extract_lane(a, idx)
    }

    fn v128_u8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> u8 {
        self.scalar_fallback.v128_u8x16_extract_lane(a, idx)
    }

    fn v128_i16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> i16 {
        self.scalar_fallback.v128_i16x8_extract_lane(a, idx)
    }

    fn v128_u16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> u16 {
        self.scalar_fallback.v128_u16x8_extract_lane(a, idx)
    }

    fn v128_i32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> i32 {
        self.scalar_fallback.v128_i32x4_extract_lane(a, idx)
    }

    fn v128_i64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> i64 {
        self.scalar_fallback.v128_i64x2_extract_lane(a, idx)
    }

    fn v128_f32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> f32 {
        self.scalar_fallback.v128_f32x4_extract_lane(a, idx)
    }

    fn v128_f64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> f64 {
        self.scalar_fallback.v128_f64x2_extract_lane(a, idx)
    }

    // Lane access operations - replace_lane
    fn v128_i8x16_replace_lane(&self, a: &[u8; 16], idx: u8, val: i8) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_replace_lane(a, idx, val)
    }

    fn v128_i16x8_replace_lane(&self, a: &[u8; 16], idx: u8, val: i16) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_replace_lane(a, idx, val)
    }

    fn v128_i32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: i32) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_replace_lane(a, idx, val)
    }

    fn v128_i64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: i64) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_replace_lane(a, idx, val)
    }

    fn v128_f32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: f32) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_replace_lane(a, idx, val)
    }

    fn v128_f64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: f64) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_replace_lane(a, idx, val)
    }

    // Splat operations (create vector from scalar)
    fn v128_i8x16_splat(&self, val: i8) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_splat(val)
    }

    fn v128_i16x8_splat(&self, val: i16) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_splat(val)
    }

    fn v128_i32x4_splat(&self, val: i32) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_splat(val)
    }

    fn v128_i64x2_splat(&self, val: i64) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_splat(val)
    }

    fn v128_f32x4_splat(&self, val: f32) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_splat(val)
    }

    fn v128_f64x2_splat(&self, val: f64) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_splat(val)
    }

    // All comparison operations delegate to scalar for now
    fn v128_i8x16_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_eq(a, b)
    }

    fn v128_i8x16_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_ne(a, b)
    }

    fn v128_i8x16_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_lt_s(a, b)
    }

    fn v128_i8x16_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_lt_u(a, b)
    }

    fn v128_i8x16_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_gt_s(a, b)
    }

    fn v128_i8x16_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_gt_u(a, b)
    }

    fn v128_i8x16_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_le_s(a, b)
    }

    fn v128_i8x16_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_le_u(a, b)
    }

    fn v128_i8x16_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_ge_s(a, b)
    }

    fn v128_i8x16_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_ge_u(a, b)
    }

    // i16x8 comparisons (delegated)
    fn v128_i16x8_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_eq(a, b)
    }

    fn v128_i16x8_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_ne(a, b)
    }

    fn v128_i16x8_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_lt_s(a, b)
    }

    fn v128_i16x8_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_lt_u(a, b)
    }

    fn v128_i16x8_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_gt_s(a, b)
    }

    fn v128_i16x8_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_gt_u(a, b)
    }

    fn v128_i16x8_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_le_s(a, b)
    }

    fn v128_i16x8_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_le_u(a, b)
    }

    fn v128_i16x8_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_ge_s(a, b)
    }

    fn v128_i16x8_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_ge_u(a, b)
    }

    // i32x4 comparisons (delegated)
    fn v128_i32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_eq(a, b)
    }

    fn v128_i32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_ne(a, b)
    }

    fn v128_i32x4_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_lt_s(a, b)
    }

    fn v128_i32x4_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_lt_u(a, b)
    }

    fn v128_i32x4_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_gt_s(a, b)
    }

    fn v128_i32x4_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_gt_u(a, b)
    }

    fn v128_i32x4_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_le_s(a, b)
    }

    fn v128_i32x4_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_le_u(a, b)
    }

    fn v128_i32x4_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_ge_s(a, b)
    }

    fn v128_i32x4_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_ge_u(a, b)
    }

    // i64x2 comparisons (delegated)
    fn v128_i64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_eq(a, b)
    }

    fn v128_i64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_ne(a, b)
    }

    fn v128_i64x2_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_lt_s(a, b)
    }

    fn v128_i64x2_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_gt_s(a, b)
    }

    fn v128_i64x2_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_le_s(a, b)
    }

    fn v128_i64x2_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_ge_s(a, b)
    }

    // f32x4 comparisons (delegated)
    fn v128_f32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_eq(a, b)
    }

    fn v128_f32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_ne(a, b)
    }

    fn v128_f32x4_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_lt(a, b)
    }

    fn v128_f32x4_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_gt(a, b)
    }

    fn v128_f32x4_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_le(a, b)
    }

    fn v128_f32x4_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_ge(a, b)
    }

    // f64x2 comparisons (delegated)
    fn v128_f64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_eq(a, b)
    }

    fn v128_f64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_ne(a, b)
    }

    fn v128_f64x2_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_lt(a, b)
    }

    fn v128_f64x2_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_gt(a, b)
    }

    fn v128_f64x2_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_le(a, b)
    }

    fn v128_f64x2_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_ge(a, b)
    }

    // All additional operations delegate to scalar for now
    fn v128_i8x16_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_abs(a)
    }

    fn v128_i16x8_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_abs(a)
    }

    fn v128_i32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_abs(a)
    }

    fn v128_i64x2_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_abs(a)
    }

    fn v128_f32x4_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_abs(a)
    }

    fn v128_f32x4_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_min(a, b)
    }

    fn v128_f32x4_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_max(a, b)
    }

    fn v128_f32x4_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_pmin(a, b)
    }

    fn v128_f32x4_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_pmax(a, b)
    }

    fn v128_f64x2_abs(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_abs(a)
    }

    fn v128_f64x2_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_min(a, b)
    }

    fn v128_f64x2_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_max(a, b)
    }

    fn v128_f64x2_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_pmin(a, b)
    }

    fn v128_f64x2_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_pmax(a, b)
    }

    // Integer min/max operations (delegated)
    fn v128_i8x16_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_min_s(a, b)
    }

    fn v128_i8x16_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_min_u(a, b)
    }

    fn v128_i8x16_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_max_s(a, b)
    }

    fn v128_i8x16_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_max_u(a, b)
    }

    fn v128_i16x8_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_min_s(a, b)
    }

    fn v128_i16x8_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_min_u(a, b)
    }

    fn v128_i16x8_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_max_s(a, b)
    }

    fn v128_i16x8_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_max_u(a, b)
    }

    fn v128_i32x4_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_min_s(a, b)
    }

    fn v128_i32x4_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_min_u(a, b)
    }

    fn v128_i32x4_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_max_s(a, b)
    }

    fn v128_i32x4_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_max_u(a, b)
    }

    // All remaining operations delegate to scalar implementation
    // This is a comprehensive delegation for the rest of the trait
    fn v128_i32x4_trunc_sat_f32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_trunc_sat_f32x4_s(a)
    }

    fn v128_i32x4_trunc_sat_f32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_trunc_sat_f32x4_u(a)
    }

    fn v128_f32x4_convert_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_convert_i32x4_s(a)
    }

    fn v128_f32x4_convert_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_convert_i32x4_u(a)
    }

    fn v128_i32x4_trunc_sat_f64x2_s_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_trunc_sat_f64x2_s_zero(a)
    }

    fn v128_i32x4_trunc_sat_f64x2_u_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_trunc_sat_f64x2_u_zero(a)
    }

    fn v128_f64x2_convert_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_convert_low_i32x4_s(a)
    }

    fn v128_f64x2_convert_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_convert_low_i32x4_u(a)
    }

    fn v128_f32x4_demote_f64x2_zero(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f32x4_demote_f64x2_zero(a)
    }

    fn v128_f64x2_promote_low_f32x4(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_f64x2_promote_low_f32x4(a)
    }

    fn v128_i8x16_narrow_i16x8_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_narrow_i16x8_s(a, b)
    }

    fn v128_i8x16_narrow_i16x8_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_narrow_i16x8_u(a, b)
    }

    fn v128_i16x8_narrow_i32x4_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_narrow_i32x4_s(a, b)
    }

    fn v128_i16x8_narrow_i32x4_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_narrow_i32x4_u(a, b)
    }

    fn v128_i16x8_extend_low_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_extend_low_i8x16_s(a)
    }

    fn v128_i16x8_extend_high_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_extend_high_i8x16_s(a)
    }

    fn v128_i16x8_extend_low_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_extend_low_i8x16_u(a)
    }

    fn v128_i16x8_extend_high_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_extend_high_i8x16_u(a)
    }

    fn v128_i32x4_extend_low_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_extend_low_i16x8_s(a)
    }

    fn v128_i32x4_extend_high_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_extend_high_i16x8_s(a)
    }

    fn v128_i32x4_extend_low_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_extend_low_i16x8_u(a)
    }

    fn v128_i32x4_extend_high_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_extend_high_i16x8_u(a)
    }

    fn v128_i64x2_extend_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_extend_low_i32x4_s(a)
    }

    fn v128_i64x2_extend_high_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_extend_high_i32x4_s(a)
    }

    fn v128_i64x2_extend_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_extend_low_i32x4_u(a)
    }

    fn v128_i64x2_extend_high_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_extend_high_i32x4_u(a)
    }

    fn v128_i8x16_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_shl(a, count)
    }

    fn v128_i8x16_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_shr_s(a, count)
    }

    fn v128_i8x16_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_shr_u(a, count)
    }

    fn v128_i16x8_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_shl(a, count)
    }

    fn v128_i16x8_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_shr_s(a, count)
    }

    fn v128_i16x8_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i16x8_shr_u(a, count)
    }

    fn v128_i32x4_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_shl(a, count)
    }

    fn v128_i32x4_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_shr_s(a, count)
    }

    fn v128_i32x4_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i32x4_shr_u(a, count)
    }

    fn v128_i64x2_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_shl(a, count)
    }

    fn v128_i64x2_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_shr_s(a, count)
    }

    fn v128_i64x2_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16] {
        self.scalar_fallback.v128_i64x2_shr_u(a, count)
    }

    fn v128_i8x16_swizzle(&self, a: &[u8; 16], s: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_swizzle(a, s)
    }

    fn v128_i8x16_shuffle(&self, a: &[u8; 16], b: &[u8; 16], lanes: &[u8; 16]) -> [u8; 16] {
        self.scalar_fallback.v128_i8x16_shuffle(a, b, lanes)
    }
}
