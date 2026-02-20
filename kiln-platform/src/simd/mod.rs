// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! SIMD (Single Instruction, Multiple Data) acceleration support
//!
//! This module provides platform-specific SIMD implementations for WebAssembly
//! vector operations. It includes runtime detection of SIMD capabilities and
//! dispatches to the most efficient implementation available on the target
//! platform.
//!
//! # Architecture
//!
//! The module is organized into:
//! - Runtime capability detection
//! - Platform-agnostic trait definitions
//! - Platform-specific implementations (x86_64, aarch64, etc.)
//! - Fallback scalar implementations
//!
//! # Safety
//!
//! All unsafe SIMD intrinsics are contained within this module and are
//! thoroughly tested. The public API is completely safe to use.

#![allow(missing_docs)]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
use alloc::boxed::Box;
use core::sync::atomic::AtomicBool;

// Platform-specific modules
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

// Fallback scalar implementation
pub mod scalar;

// Test module
#[cfg(test)]
mod test_simd;

// Re-export for convenience
#[cfg(target_arch = "aarch64")]
pub use aarch64::AArch64SimdProvider;
pub use scalar::ScalarSimdProvider;

/// SIMD capability levels supported by the platform
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimdLevel {
    /// No SIMD support, use scalar fallback
    None,
    /// Basic SIMD support (SSE2 on x86, NEON on ARM)
    Basic,
    /// Advanced SIMD support (AVX2 on x86, SVE on ARM)
    Advanced,
    /// Extended SIMD support (AVX-512 on x86, SVE2 on ARM)
    Extended,
}

/// Runtime-detected SIMD capabilities
#[derive(Debug, Clone)]
pub struct SimdCapabilities {
    /// x86_64 features
    #[cfg(target_arch = "x86_64")]
    pub has_sse2:    bool,
    #[cfg(target_arch = "x86_64")]
    pub has_sse3:    bool,
    #[cfg(target_arch = "x86_64")]
    pub has_ssse3:   bool,
    #[cfg(target_arch = "x86_64")]
    pub has_sse41:   bool,
    #[cfg(target_arch = "x86_64")]
    pub has_sse42:   bool,
    #[cfg(target_arch = "x86_64")]
    pub has_avx:     bool,
    #[cfg(target_arch = "x86_64")]
    pub has_avx2:    bool,
    #[cfg(target_arch = "x86_64")]
    pub has_avx512f: bool,

    /// ARM features
    #[cfg(target_arch = "aarch64")]
    pub has_neon: bool,
    #[cfg(target_arch = "aarch64")]
    pub has_sve:  bool,
    #[cfg(target_arch = "aarch64")]
    pub has_sve2: bool,

    /// Highest available SIMD level
    pub level: SimdLevel,
}

impl Default for SimdCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl SimdCapabilities {
    /// Detect SIMD capabilities at runtime
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            return Self::detect_x86_64();
        }

        // TODO: Implement aarch64 detection
        // #[cfg(target_arch = "aarch64")]
        // {
        //     return Self::detect_aarch64);
        // }

        // Fallback for unsupported architectures
        Self {
            #[cfg(target_arch = "x86_64")]
            has_sse2:                                   false,
            #[cfg(target_arch = "x86_64")]
            has_sse3:                                   false,
            #[cfg(target_arch = "x86_64")]
            has_ssse3:                                  false,
            #[cfg(target_arch = "x86_64")]
            has_sse41:                                  false,
            #[cfg(target_arch = "x86_64")]
            has_sse42:                                  false,
            #[cfg(target_arch = "x86_64")]
            has_avx:                                    false,
            #[cfg(target_arch = "x86_64")]
            has_avx2:                                   false,
            #[cfg(target_arch = "x86_64")]
            has_avx512f:                                false,

            #[cfg(target_arch = "aarch64")]
            has_neon:                                 false,
            #[cfg(target_arch = "aarch64")]
            has_sve:                                  false,
            #[cfg(target_arch = "aarch64")]
            has_sve2:                                 false,

            level: SimdLevel::None,
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn detect_x86_64() -> Self {
        use core::arch::x86_64::*;

        // Only use is_x86_feature_detected in std environments
        #[cfg(feature = "std")]
        let has_sse2 = is_x86_feature_detected!("sse2");
        #[cfg(feature = "std")]
        let has_sse3 = is_x86_feature_detected!("sse3");
        #[cfg(feature = "std")]
        let has_ssse3 = is_x86_feature_detected!("ssse3");
        #[cfg(feature = "std")]
        let has_sse41 = is_x86_feature_detected!("sse4.1");
        #[cfg(feature = "std")]
        let has_sse42 = is_x86_feature_detected!("sse4.2");
        #[cfg(feature = "std")]
        let has_avx = is_x86_feature_detected!("avx");
        #[cfg(feature = "std")]
        let has_avx2 = is_x86_feature_detected!("avx2");
        #[cfg(feature = "std")]
        let has_avx512f = is_x86_feature_detected!("avx512f");

        // In no_std, assume basic SSE2 support on x86_64
        #[cfg(not(feature = "std"))]
        let (has_sse2, has_sse3, has_ssse3, has_sse41, has_sse42, has_avx, has_avx2, has_avx512f) =
            (true, false, false, false, false, false, false, false);

        let level = if has_avx512f {
            SimdLevel::Extended
        } else if has_avx2 {
            SimdLevel::Advanced
        } else if has_sse2 {
            SimdLevel::Basic
        } else {
            SimdLevel::None
        };

        Self {
            has_sse2,
            has_sse3,
            has_ssse3,
            has_sse41,
            has_sse42,
            has_avx,
            has_avx2,
            has_avx512f,
            level,
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[allow(dead_code)]
    fn detect_aarch64() -> Self {
        // ARM64 always has NEON
        let has_neon = true;

        // SVE detection would require runtime checks
        // For now, assume no SVE support in no_std
        #[cfg(feature = "std")]
        let has_sve = false; // Would need platform-specific detection
        #[cfg(feature = "std")]
        let has_sve2 = false;

        #[cfg(not(feature = "std"))]
        let (has_sve, has_sve2) = (false, false);

        let level = if has_sve2 {
            SimdLevel::Extended
        } else if has_sve {
            SimdLevel::Advanced
        } else if has_neon {
            SimdLevel::Basic
        } else {
            SimdLevel::None
        };

        Self {
            has_neon,
            has_sve,
            has_sve2,
            level,
        }
    }
}

/// Platform-specific SIMD provider trait
///
/// This trait defines the interface that all SIMD implementations must provide.
/// Each method corresponds to a WebAssembly SIMD instruction.
pub trait SimdProvider: Send + Sync {
    /// Get the SIMD level this provider implements
    fn simd_level(&self) -> SimdLevel;

    /// Check if this provider is available on the current CPU
    fn is_available(&self) -> bool;

    // Arithmetic operations - i8x16
    fn v128_i8x16_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_neg(&self, a: &[u8; 16]) -> [u8; 16];

    // Arithmetic operations - i16x8
    fn v128_i16x8_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_neg(&self, a: &[u8; 16]) -> [u8; 16];

    // Arithmetic operations - i32x4
    fn v128_i32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_neg(&self, a: &[u8; 16]) -> [u8; 16];

    // Arithmetic operations - i64x2
    fn v128_i64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_neg(&self, a: &[u8; 16]) -> [u8; 16];

    // Arithmetic operations - f32x4
    fn v128_f32x4_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_neg(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_sqrt(&self, a: &[u8; 16]) -> [u8; 16];

    // Arithmetic operations - f64x2
    fn v128_f64x2_add(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_sub(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_mul(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_div(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_neg(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_sqrt(&self, a: &[u8; 16]) -> [u8; 16];

    // Bitwise operations
    fn v128_not(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_and(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_or(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_xor(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_andnot(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_bitselect(&self, a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16];

    // Test operations
    fn v128_any_true(&self, a: &[u8; 16]) -> bool;
    fn v128_i8x16_all_true(&self, a: &[u8; 16]) -> bool;
    fn v128_i16x8_all_true(&self, a: &[u8; 16]) -> bool;
    fn v128_i32x4_all_true(&self, a: &[u8; 16]) -> bool;
    fn v128_i64x2_all_true(&self, a: &[u8; 16]) -> bool;

    // Lane access operations - extract_lane
    fn v128_i8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> i8;
    fn v128_u8x16_extract_lane(&self, a: &[u8; 16], idx: u8) -> u8;
    fn v128_i16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> i16;
    fn v128_u16x8_extract_lane(&self, a: &[u8; 16], idx: u8) -> u16;
    fn v128_i32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> i32;
    fn v128_i64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> i64;
    fn v128_f32x4_extract_lane(&self, a: &[u8; 16], idx: u8) -> f32;
    fn v128_f64x2_extract_lane(&self, a: &[u8; 16], idx: u8) -> f64;

    // Lane access operations - replace_lane
    fn v128_i8x16_replace_lane(&self, a: &[u8; 16], idx: u8, val: i8) -> [u8; 16];
    fn v128_i16x8_replace_lane(&self, a: &[u8; 16], idx: u8, val: i16) -> [u8; 16];
    fn v128_i32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: i32) -> [u8; 16];
    fn v128_i64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: i64) -> [u8; 16];
    fn v128_f32x4_replace_lane(&self, a: &[u8; 16], idx: u8, val: f32) -> [u8; 16];
    fn v128_f64x2_replace_lane(&self, a: &[u8; 16], idx: u8, val: f64) -> [u8; 16];

    // Splat operations (create vector from scalar)
    fn v128_i8x16_splat(&self, val: i8) -> [u8; 16];
    fn v128_i16x8_splat(&self, val: i16) -> [u8; 16];
    fn v128_i32x4_splat(&self, val: i32) -> [u8; 16];
    fn v128_i64x2_splat(&self, val: i64) -> [u8; 16];
    fn v128_f32x4_splat(&self, val: f32) -> [u8; 16];
    fn v128_f64x2_splat(&self, val: f64) -> [u8; 16];

    // Comparison operations - i8x16
    fn v128_i8x16_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Comparison operations - i16x8
    fn v128_i16x8_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Comparison operations - i32x4
    fn v128_i32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_lt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_gt_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_le_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_ge_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Comparison operations - i64x2
    fn v128_i64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_lt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_gt_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_le_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_ge_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Comparison operations - f32x4
    fn v128_f32x4_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Comparison operations - f64x2
    fn v128_f64x2_eq(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_ne(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_lt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_gt(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_le(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_ge(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Additional arithmetic operations
    fn v128_i8x16_abs(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_abs(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_abs(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_abs(&self, a: &[u8; 16]) -> [u8; 16];

    fn v128_f32x4_abs(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    fn v128_f64x2_abs(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_min(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_max(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_pmin(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_pmax(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Integer min/max operations
    fn v128_i8x16_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    fn v128_i16x8_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    fn v128_i32x4_min_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_min_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_max_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_max_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Conversion operations
    fn v128_i32x4_trunc_sat_f32x4_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_trunc_sat_f32x4_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_convert_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_convert_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_trunc_sat_f64x2_s_zero(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_trunc_sat_f64x2_u_zero(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_convert_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_convert_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f32x4_demote_f64x2_zero(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_f64x2_promote_low_f32x4(&self, a: &[u8; 16]) -> [u8; 16];

    // Narrow operations
    fn v128_i8x16_narrow_i16x8_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_narrow_i16x8_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_narrow_i32x4_s(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_narrow_i32x4_u(&self, a: &[u8; 16], b: &[u8; 16]) -> [u8; 16];

    // Extend operations
    fn v128_i16x8_extend_low_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_extend_high_i8x16_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_extend_low_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i16x8_extend_high_i8x16_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_extend_low_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_extend_high_i16x8_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_extend_low_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i32x4_extend_high_i16x8_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_extend_low_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_extend_high_i32x4_s(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_extend_low_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16];
    fn v128_i64x2_extend_high_i32x4_u(&self, a: &[u8; 16]) -> [u8; 16];

    // Shift operations
    fn v128_i8x16_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i8x16_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i8x16_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i16x8_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i16x8_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i16x8_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i32x4_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i32x4_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i32x4_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i64x2_shl(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i64x2_shr_s(&self, a: &[u8; 16], count: u32) -> [u8; 16];
    fn v128_i64x2_shr_u(&self, a: &[u8; 16], count: u32) -> [u8; 16];

    // Advanced shuffle operations
    fn v128_i8x16_swizzle(&self, a: &[u8; 16], s: &[u8; 16]) -> [u8; 16];
    fn v128_i8x16_shuffle(&self, a: &[u8; 16], b: &[u8; 16], lanes: &[u8; 16]) -> [u8; 16];
}

/// SIMD runtime that manages provider selection
#[cfg(feature = "std")]
pub struct SimdRuntime {
    provider:     Box<dyn SimdProvider>,
    capabilities: SimdCapabilities,
}

// Global initialization flag
#[allow(dead_code)]
static SIMD_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "std")]
impl SimdRuntime {
    /// Create a new SIMD runtime with automatic provider selection
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        let capabilities = SimdCapabilities::detect();
        let provider = Self::select_provider(&capabilities);

        // Mark as initialized
        SIMD_INITIALIZED.store(true, core::sync::atomic::Ordering::Relaxed);

        Self {
            provider,
            capabilities,
        }
    }

    /// Select the best available provider based on capabilities
    #[cfg(feature = "std")]
    fn select_provider(_capabilities: &SimdCapabilities) -> Box<dyn SimdProvider> {
        #[cfg(target_arch = "x86_64")]
        {
            if _capabilities.has_avx2 {
                return Box::new(x86_64::X86SimdProvider::new_avx2());
            } else if _capabilities.has_sse2 {
                return Box::new(x86_64::X86SimdProvider::new_sse2());
            }
        }

        // TODO: Implement aarch64 provider selection
        // #[cfg(target_arch = "aarch64")]
        // {
        //     if capabilities.has_neon {
        //         return Box::new(aarch64::ArmSimdProvider::new();
        //     }
        // }

        // Fallback to scalar
        Box::new(ScalarSimdProvider::new())
    }

    /// Get the current provider
    pub fn provider(&self) -> &dyn SimdProvider {
        &*self.provider
    }

    /// Get the detected capabilities
    pub fn capabilities(&self) -> &SimdCapabilities {
        &self.capabilities
    }

    /// Check if hardware acceleration is available
    pub fn has_acceleration(&self) -> bool {
        self.capabilities.level > SimdLevel::None
    }
}

#[cfg(feature = "std")]
impl Default for SimdRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_capabilities_detection() {
        let caps = SimdCapabilities::detect();

        // Should always have at least None level
        assert!(caps.level >= SimdLevel::None);

        // On x86_64, we should at least have SSE2
        #[cfg(all(target_arch = "x86_64", feature = "std"))]
        {
            assert!(caps.has_sse2);
            assert!(caps.level >= SimdLevel::Basic);
        }

        // On aarch64, we should have NEON
        #[cfg(target_arch = "aarch64")]
        {
            assert!(caps.has_neon);
            assert!(caps.level >= SimdLevel::Basic);
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_simd_runtime_creation() {
        let runtime = SimdRuntime::new();

        // Should have a valid provider
        assert!(runtime.provider().is_available());

        // Provider level should match capabilities
        assert_eq!(
            runtime.provider().simd_level(),
            runtime.capabilities().level
        );
    }

    #[test]
    fn test_simd_level_ordering() {
        assert!(SimdLevel::None < SimdLevel::Basic);
        assert!(SimdLevel::Basic < SimdLevel::Advanced);
        assert!(SimdLevel::Advanced < SimdLevel::Extended);
    }
}
