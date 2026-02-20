// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Test file for SIMD implementation
//!
//! This demonstrates the usage of our SIMD abstraction layer.

#[cfg(test)]
mod simd_tests {
    use super::super::*;

    #[test]
    fn test_scalar_simd_provider() {
        let provider = ScalarSimdProvider::new();

        assert_eq!(provider.simd_level(), SimdLevel::None);
        assert!(provider.is_available());

        // Test i32x4 addition
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
    fn test_simd_capabilities_detection() {
        let caps = SimdCapabilities::detect();

        // Should always have at least None level
        assert!(caps.level >= SimdLevel::None);

        // On x86_64, we should at least have SSE2 when std is available
        #[cfg(all(target_arch = "x86_64", feature = "std"))]
        {
            assert!(caps.has_sse2);
            assert!(caps.level >= SimdLevel::Basic);
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_simd_runtime() {
        let runtime = SimdRuntime::new();

        // Should have a valid provider
        assert!(runtime.provider().is_available());

        // Provider level should match capabilities
        assert_eq!(
            runtime.provider().simd_level(),
            runtime.capabilities().level
        );

        // Test that hardware acceleration detection works
        if runtime.has_acceleration() {
            assert!(runtime.capabilities().level > SimdLevel::None);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_x86_simd_provider_if_available() {
        // Only test if we're actually on x86_64
        let provider = x86_64::X86SimdProvider::new_sse2();

        assert_eq!(provider.simd_level(), SimdLevel::Basic);
        assert!(provider.is_available());

        // Test a simple operation
        let a = [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
        let b = [5, 0, 0, 0, 6, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0];
        let result = provider.v128_i32x4_add(&a, &b);

        assert_eq!(&result[0..4], &[6, 0, 0, 0]);
        assert_eq!(&result[4..8], &[8, 0, 0, 0]);
        assert_eq!(&result[8..12], &[10, 0, 0, 0]);
        assert_eq!(&result[12..16], &[12, 0, 0, 0]);
    }

    #[test]
    fn test_bitwise_operations() {
        let provider = ScalarSimdProvider::new();

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
    fn test_float_operations() {
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

        // Extract results and check they're close (floating point comparison)
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

        assert!((r0 - 1.0).abs() < 0.001);
        assert!((r1 - 6.0).abs() < 0.001);
        assert!((r2 - 1.0).abs() < 0.001);
        assert!((r3 - 1.0).abs() < 0.001);
    }
}
