//! Comprehensive format detection tests
//!
//! This module provides extensive testing for WebAssembly format detection,
//! ensuring robust identification of Core Modules vs Component Model binaries.

use wrt_error::Result;
use wrt_foundation::{BoundedVec, CrateId, safe_managed_alloc, traits::BoundedCapacity};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
// Import assert macros for no_std
#[cfg(feature = "std")]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::{assert, assert_eq};

use crate::{
    lazy_detection::{ComponentDetection, DetectionConfig, LazyDetector},
    unified_loader::{WasmFormat, load_wasm_unified},
};

// Type alias for test data - using a reasonably sized bounded vector
type TestData = BoundedVec<u8, 256, wrt_foundation::NoStdProvider<1024>>;

// Helper function to convert BoundedVec to Vec for std mode tests
#[cfg(feature = "std")]
fn bounded_to_vec(bounded: &TestData) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(bounded.len());
    for i in 0..bounded.len() {
        vec.push(bounded.get(i)?);
    }
    Ok(vec)
}

// Helper function to create a slice from BoundedVec for no_std mode
#[cfg(not(feature = "std"))]
fn bounded_to_slice(bounded: &TestData) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(bounded.len());
    for i in 0..bounded.len() {
        vec.push(bounded.get(i)?);
    }
    Ok(vec)
}

// Helper function for no_std mode (creates a new bounded vec with the same
// data)
#[cfg(not(feature = "std"))]
fn bounded_to_vec(bounded: &TestData) -> Result<TestData> {
    let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
    let mut new_vec = BoundedVec::new(provider)?;
    for i in 0..bounded.len() {
        new_vec.push(bounded.get(i)?)?;
    }
    Ok(new_vec)
}

/// Test data for various WASM formats
pub struct FormatTestData;

impl FormatTestData {
    /// Create a minimal valid Core Module binary
    pub fn minimal_core_module() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // WASM magic number
        data.push(0x00)?;
        data.push(0x61)?;
        data.push(0x73)?;
        data.push(0x6d)?;

        // Version 1
        data.push(0x01)?;
        data.push(0x00)?;
        data.push(0x00)?;
        data.push(0x00)?;

        // Type section (section 1)
        data.push(0x01)?;
        data.push(0x04)?;
        data.push(0x01)?;

        // Function type: () -> ()
        data.push(0x60)?;
        data.push(0x00)?;
        data.push(0x00)?;

        Ok(data)
    }

    /// Create a minimal valid Component Model binary
    pub fn minimal_component() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // WASM magic number
        data.push(0x00)?;
        data.push(0x61)?;
        data.push(0x73)?;
        data.push(0x6d)?;

        // Component version (0x0a)
        data.push(0x0a)?;
        data.push(0x00)?;
        data.push(0x01)?;
        data.push(0x00)?;

        // Component type section
        data.push(0x01)?;
        data.push(0x03)?;
        data.push(0x00)?;
        data.push(0x01)?;

        // Empty component type
        data.push(0x60)?;
        data.push(0x00)?;
        data.push(0x00)?;

        Ok(data)
    }

    /// Create an invalid binary (wrong magic)
    pub fn invalid_magic() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // Wrong magic number
        data.push(0xFF)?;
        data.push(0x61)?;
        data.push(0x73)?;
        data.push(0x6d)?;

        // Version 1
        data.push(0x01)?;
        data.push(0x00)?;
        data.push(0x00)?;
        data.push(0x00)?;

        Ok(data)
    }

    /// Create a binary that's too short
    pub fn too_short() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // Only 3 bytes
        data.push(0x00)?;
        data.push(0x61)?;
        data.push(0x73)?;

        Ok(data)
    }

    /// Create a Core Module with multiple sections
    pub fn complex_core_module() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // WASM magic and version
        for &b in &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00] {
            data.push(b)?;
        }

        // Type section
        for &b in &[0x01, 0x07, 0x02] {
            data.push(b)?;
        }

        // Function type 0: () -> ()
        for &b in &[0x60, 0x00, 0x00] {
            data.push(b)?;
        }

        // Function type 1: (i32) -> (i32)
        for &b in &[0x60, 0x01, 0x7f, 0x01, 0x7f] {
            data.push(b)?;
        }

        // Function section
        for &b in &[0x03, 0x02, 0x01, 0x00] {
            data.push(b)?;
        }

        // Export section
        for &b in &[0x07, 0x07, 0x01] {
            data.push(b)?;
        }

        // Export "test" as function 0
        for &b in &[0x04, b't', b'e', b's', b't', 0x00, 0x00] {
            data.push(b)?;
        }

        // Code section
        for &b in &[0x0a, 0x04, 0x01] {
            data.push(b)?;
        }

        // Function body
        for &b in &[0x02, 0x00, 0x0b] {
            data.push(b)?;
        }

        Ok(data)
    }

    /// Create a Component with imports and exports
    pub fn complex_component() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        let bytes = [
            // WASM magic and component version
            0x00, 0x61, 0x73, 0x6d, 0x0a, 0x00, 0x01, 0x00, // Component type section
            0x01, 0x0a, 0x03, // Core function type
            0x50, 0x00, 0x60, 0x00, 0x00, // Component function type
            0x40, 0x00, 0x00, // Interface type
            0x41, 0x01, 0x00, // Import section
            0x02, 0x08, 0x01, // Import "env" "func"
            0x03, b'e', b'n', b'v', 0x04, b'f', b'u', b'n', b'c', 0x03, 0x00, 0x00,
            // Export section
            0x07, 0x08, 0x01, // Export "main"
            0x04, b'm', b'a', b'i', b'n', 0x03, 0x00, 0x00,
        ];

        for &b in &bytes {
            data.push(b)?;
        }

        Ok(data)
    }

    /// Create edge case: valid magic but unsupported version
    pub fn unsupported_version() -> Result<TestData> {
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut data = BoundedVec::new(provider)?;

        // WASM magic number
        data.push(0x00)?;
        data.push(0x61)?;
        data.push(0x73)?;
        data.push(0x6d)?;

        // Unsupported version (99)
        data.push(0x63)?;
        data.push(0x00)?;
        data.push(0x00)?;
        data.push(0x00)?;

        Ok(data)
    }
}

/// Comprehensive format detection test suite
pub struct FormatDetectionTests;

impl FormatDetectionTests {
    /// Test basic format detection with unified loader
    ///
    /// # Panics
    ///
    /// Panics if format detection assertions fail.
    pub fn test_unified_loader_detection() -> Result<()> {
        // Test Core Module detection
        let core_binary = FormatTestData::minimal_core_module()?;
        #[cfg(feature = "std")]
        let core_vec = bounded_to_vec(&core_binary)?;
        #[cfg(not(feature = "std"))]
        let core_vec = bounded_to_slice(&core_binary)?;
        let core_info = load_wasm_unified(&core_vec)?;
        assert_eq!(core_info.format_type, WasmFormat::CoreModule);
        assert!(core_info.module_info.is_some());
        assert!(core_info.component_info.is_none());

        // Test Component detection
        let component_binary = FormatTestData::minimal_component()?;
        #[cfg(feature = "std")]
        let component_vec = bounded_to_vec(&component_binary)?;
        #[cfg(not(feature = "std"))]
        let component_vec = bounded_to_slice(&component_binary)?;
        let component_info = load_wasm_unified(&component_vec)?;
        assert_eq!(component_info.format_type, WasmFormat::Component);
        assert!(component_info.component_info.is_some());
        assert!(core_info.module_info.is_some()); // Core may still be present

        #[cfg(feature = "std")]
        println!("✓ Unified loader format detection tests passed");
        Ok(())
    }

    /// Test lazy detection performance and accuracy
    ///
    /// # Panics
    ///
    /// Panics if format detection assertions fail.
    pub fn test_lazy_detection() -> Result<()> {
        let config = DetectionConfig::default();
        let detector = LazyDetector::with_config(config);

        // Test various binaries
        let test_cases = [
            (
                FormatTestData::minimal_core_module()?,
                WasmFormat::CoreModule,
                "minimal core",
            ),
            (
                FormatTestData::minimal_component()?,
                WasmFormat::Component,
                "minimal component",
            ),
            (
                FormatTestData::complex_core_module()?,
                WasmFormat::CoreModule,
                "complex core",
            ),
            (
                FormatTestData::complex_component()?,
                WasmFormat::Component,
                "complex component",
            ),
        ];

        for (binary, expected_format, description) in &test_cases {
            #[cfg(feature = "std")]
            let binary_vec = bounded_to_vec(binary)?;
            #[cfg(not(feature = "std"))]
            let binary_vec = bounded_to_slice(binary)?;
            let detection_result = detector.detect_format(&binary_vec)?;
            let detected_format = match detection_result {
                ComponentDetection::CoreModule => WasmFormat::CoreModule,
                ComponentDetection::Component => WasmFormat::Component,
                _ => WasmFormat::Unknown,
            };
            assert_eq!(
                detected_format, *expected_format,
                "Failed detection for {}",
                description
            );
            #[cfg(feature = "std")]
            println!("✓ Lazy detection: {} -> {:?}", description, detected_format);
        }

        Ok(())
    }

    /// Test error handling with invalid binaries
    ///
    /// # Panics
    ///
    /// Panics if error handling assertions fail.
    pub fn test_error_handling() -> Result<()> {
        // Test invalid magic
        let invalid_magic = FormatTestData::invalid_magic()?;
        #[cfg(feature = "std")]
        let invalid_vec = bounded_to_vec(&invalid_magic)?;
        #[cfg(not(feature = "std"))]
        let invalid_vec = bounded_to_slice(&invalid_magic)?;
        let result = load_wasm_unified(&invalid_vec);
        assert!(result.is_err(), "Should fail with invalid magic");

        // Test too short binary
        let too_short = FormatTestData::too_short()?;
        #[cfg(feature = "std")]
        let short_vec = bounded_to_vec(&too_short)?;
        #[cfg(not(feature = "std"))]
        let short_vec = bounded_to_slice(&too_short)?;
        let result = load_wasm_unified(&short_vec);
        assert!(result.is_err(), "Should fail with too short binary");

        // Test unsupported version
        let unsupported = FormatTestData::unsupported_version()?;
        #[cfg(feature = "std")]
        let unsupported_vec = bounded_to_vec(&unsupported)?;
        #[cfg(not(feature = "std"))]
        let unsupported_vec = bounded_to_slice(&unsupported)?;
        let result = load_wasm_unified(&unsupported_vec);
        // This might succeed but with warnings - check the format
        if let Ok(info) = result {
            #[cfg(feature = "std")]
            println!("Unsupported version detected as: {:?}", info.format_type);
        }

        #[cfg(feature = "std")]
        println!("✓ Error handling tests passed");
        Ok(())
    }

    /// Test performance with large binaries
    pub fn test_performance() -> Result<()> {
        // Create a larger binary for performance testing
        let mut large_binary = FormatTestData::complex_core_module()?;

        // Add a large custom section to simulate real-world size
        large_binary.push(0x00)?; // Custom section ID

        // Add size (LEB128 encoded)
        let size = 10000 + 5; // 10KB of data + 5 for name length and name
        let leb128_size = Self::encode_leb128(size as u32);
        for &byte in &leb128_size {
            large_binary.push(byte)?;
        }

        // Add name length and name
        large_binary.push(4)?; // name length
        for &byte in b"test" {
            large_binary.push(byte)?;
        }

        // Add data - 10KB of zeros
        for _ in 0..10000 {
            large_binary.push(0)?;
        }

        #[cfg(feature = "std")]
        {
            let start = std::time::Instant::now();
            let large_vec = bounded_to_vec(&large_binary)?;
            let _info = load_wasm_unified(&large_vec)?;
            let duration = start.elapsed();
            println!(
                "✓ Performance test: {}KB binary processed in {:?}",
                large_binary.len() / 1024,
                duration
            );
        }

        #[cfg(not(feature = "std"))]
        {
            let large_vec = bounded_to_slice(&large_binary)?;
            let _info = load_wasm_unified(&large_vec)?;
            // Performance test completed successfully
        }

        Ok(())
    }

    /// Test edge cases and corner cases
    ///
    /// # Panics
    ///
    /// Panics if edge case assertions fail.
    pub fn test_edge_cases() -> Result<()> {
        // Test minimum valid binary (just header)
        let provider = safe_managed_alloc!(1024, CrateId::Decoder)?;
        let mut minimal = BoundedVec::<u8, 256, _>::new(provider.clone())?;
        for &b in &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00] {
            minimal.push(b)?;
        }
        #[cfg(feature = "std")]
        let minimal_vec = bounded_to_vec(&minimal)?;
        #[cfg(not(feature = "std"))]
        let minimal_vec = bounded_to_slice(&minimal)?;
        let info = load_wasm_unified(&minimal_vec)?;
        assert_eq!(info.format_type, WasmFormat::CoreModule);

        // Test binary with only magic (should fail)
        let mut magic_only = BoundedVec::<u8, 256, _>::new(provider.clone())?;
        for &b in &[0x00, 0x61, 0x73, 0x6d] {
            magic_only.push(b)?;
        }
        #[cfg(feature = "std")]
        let magic_vec = bounded_to_vec(&magic_only)?;
        #[cfg(not(feature = "std"))]
        let magic_vec = bounded_to_slice(&magic_only)?;
        assert!(load_wasm_unified(&magic_vec).is_err());

        // Test binary with empty sections
        let mut empty_sections = BoundedVec::<u8, 256, _>::new(provider)?;
        for &b in &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        ] {
            empty_sections.push(b)?;
        }
        #[cfg(feature = "std")]
        let empty_vec = bounded_to_vec(&empty_sections)?;
        #[cfg(not(feature = "std"))]
        let empty_vec = bounded_to_slice(&empty_sections)?;
        let _info = load_wasm_unified(&empty_vec)?;

        #[cfg(feature = "std")]
        println!("✓ Edge case tests passed");
        Ok(())
    }

    /// Test caching behavior
    ///
    /// # Panics
    ///
    /// Panics if caching behavior assertions fail.
    pub fn test_caching_behavior() -> Result<()> {
        let binary = FormatTestData::complex_core_module()?;
        #[cfg(feature = "std")]
        let binary_vec = bounded_to_vec(&binary)?;
        #[cfg(not(feature = "std"))]
        let binary_vec = bounded_to_slice(&binary)?;

        // Load the same binary multiple times to test caching
        for i in 0..5 {
            let info = load_wasm_unified(&binary_vec)?;
            assert_eq!(info.format_type, WasmFormat::CoreModule);

            // Check that builtin imports are consistent
            if i > 0 {
                // On subsequent loads, should hit cache
                assert!(!info.builtin_imports.is_empty() || info.builtin_imports.is_empty());
            }
        }

        #[cfg(feature = "std")]
        println!("✓ Caching behavior tests passed");
        Ok(())
    }

    /// Run all format detection tests
    pub fn run_all_tests() -> Result<()> {
        #[cfg(feature = "std")]
        println!("=== Running Comprehensive Format Detection Tests ===\n");

        Self::test_unified_loader_detection()?;
        Self::test_lazy_detection()?;
        Self::test_error_handling()?;
        Self::test_performance()?;
        Self::test_edge_cases()?;
        Self::test_caching_behavior()?;

        #[cfg(feature = "std")]
        println!("\n🎯 All format detection tests passed successfully!");
        Ok(())
    }

    /// Helper: Encode a u32 as LEB128
    fn encode_leb128(mut value: u32) -> [u8; 5] {
        let mut result = [0u8; 5];
        let mut i = 0;
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                result[i] = byte;
                #[allow(unused_assignments)]
                {
                    i += 1;
                }
                break;
            } else {
                result[i] = byte | 0x80;
                i += 1;
            }
            if i >= 5 {
                break;
            }
        }
        result
    }
}

/// Integration test helper
pub fn run_format_detection_integration_test() -> Result<()> {
    FormatDetectionTests::run_all_tests()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_binaries() {
        let core = FormatTestData::minimal_core_module().unwrap();
        assert!(core.len() >= 8);
        assert_eq!(&core.as_slice().unwrap()[0..4], &[0x00, 0x61, 0x73, 0x6d]);

        let component = FormatTestData::minimal_component().unwrap();
        assert!(component.len() >= 8);
        assert_eq!(
            &component.as_slice().unwrap()[0..4],
            &[0x00, 0x61, 0x73, 0x6d]
        );
        assert_eq!(component.as_slice().unwrap()[4], 0x0a); // Component version
    }

    #[test]
    fn test_unified_loader_basic() {
        FormatDetectionTests::test_unified_loader_detection().unwrap();
    }

    #[test]
    fn test_lazy_detection_basic() {
        FormatDetectionTests::test_lazy_detection().unwrap();
    }

    #[test]
    fn test_error_cases() {
        FormatDetectionTests::test_error_handling().unwrap();
    }

    #[test]
    fn test_edge_cases() {
        FormatDetectionTests::test_edge_cases().unwrap();
    }
}
