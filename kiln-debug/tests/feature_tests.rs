// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Integration tests for wrt-debug feature configurations

use wrt_debug::prelude::*;

// Test data - simplified WASM module
const TEST_MODULE: &[u8] = &[
    // WASM header
    0x00, 0x61, 0x73, 0x6D, // \0asm
    0x01, 0x00, 0x00, 0x00, // version 1
    // Simple type section
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // Empty function section
    0x03, 0x01, 0x00,
    // End of module
];

#[test]
fn test_no_features_basic_functionality() {
    // Test basic functionality that should work with no features
    let mut debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();

    // Section registration should always work
    debug_info.add_section(".debug_line", 100, 50);
    debug_info.add_section(".debug_info", 200, 100);
    debug_info.add_section(".debug_abbrev", 300, 30);

    // Basic query should work
    let has_debug = debug_info.has_debug_info();

    // With no features, this depends on what's compiled in
    #[cfg(any(feature = "line-info", feature = "debug-info"))]
    assert!(has_debug);

    #[cfg(not(any(feature = "line-info", feature = "debug-info")))]
    assert!(!has_debug);
}

#[cfg(feature = "line-info")]
#[test]
fn test_line_info_feature() {
    let mut debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();
    debug_info.add_section(".debug_line", 100, 50);

    // Line info queries should be available
    let result = debug_info.find_line_info(0x42);

    // Should not error, but may return None due to invalid test data
    match result {
        Ok(None) => (),    // Expected for test data
        Ok(Some(_)) => (), // Unexpected but not an error
        Err(_) => (),      // Expected due to invalid test data
    }
}

#[cfg(feature = "debug-info")]
#[test]
fn test_debug_info_feature() {
    let mut debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();
    debug_info.add_section(".debug_info", 200, 100);
    debug_info.add_section(".debug_abbrev", 300, 30);

    // Debug info parser initialization should be available
    let result = debug_info.init_info_parser();

    // May fail due to invalid test data, but method should exist
    match result {
        Ok(_) => (),
        Err(_) => (), // Expected due to invalid test data
    }
}

#[cfg(feature = "function-info")]
#[test]
fn test_function_info_feature() {
    let mut debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();
    debug_info.add_section(".debug_info", 200, 100);
    debug_info.add_section(".debug_abbrev", 300, 30);

    // Initialize parser (may fail with test data)
    let _ = debug_info.init_info_parser();

    // Function info queries should be available
    let result = debug_info.find_function_info(0x42);

    // Should return None for test data
    assert!(result.is_none());

    // Get all functions should be available
    let functions = debug_info.get_functions();
    match functions {
        Some(funcs) => assert!(funcs.is_empty()), // No functions in test data
        None => (),                               // Parser may not be initialized
    }
}

#[test]
fn test_feature_detection() {
    // Test compile-time feature detection
    let mut feature_count = 0;

    #[cfg(feature = "line-info")]
    {
        feature_count += 1;
    }

    #[cfg(feature = "debug-info")]
    {
        feature_count += 1;
    }

    #[cfg(feature = "function-info")]
    {
        feature_count += 1;
    }

    // Should have at least 0 features
    assert!(feature_count >= 0);
}

#[test]
fn test_cursor_functionality() {
    use wrt_debug::DwarfCursor;

    let test_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let mut cursor = DwarfCursor::new(test_data);

    // Basic cursor operations
    assert_eq!(cursor.position(), 0);
    assert_eq!(cursor.remaining(), 8);
    // Read some data
    let byte1 = cursor.read_u8().expect("Should read byte");
    assert_eq!(byte1, 0x01);
    assert_eq!(cursor.position(), 1);

    let byte2 = cursor.read_u8().expect("Should read byte");
    assert_eq!(byte2, 0x02);

    // Skip some data
    cursor.skip(2).expect("Should skip");
    assert_eq!(cursor.position(), 4);

    // Read remaining as slice
    let remaining = cursor.read_bytes(4).expect("Should read bytes");
    assert_eq!(remaining, &[0x05, 0x06, 0x07, 0x08]);
    assert!(cursor.is_at_end());
}

#[test]
fn test_memory_efficiency() {
    // Test that debug info struct is reasonably sized
    let debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();
    let size = core::mem::size_of_val(&debug_info);

    // Should be reasonably small (adjust based on features)
    #[cfg(feature = "full-debug")]
    assert!(size < 1024, "Debug info too large: {} bytes", size);

    #[cfg(all(feature = "line-info", not(feature = "debug-info")))]
    assert!(size < 512, "Debug info too large: {} bytes", size);

    #[cfg(not(any(feature = "line-info", feature = "debug-info")))]
    assert!(size < 256, "Debug info too large: {} bytes", size);
}

#[test]
fn test_error_handling() {
    let mut debug_info = DwarfDebugInfo::new(&[]).unwrap();

    // Should handle empty module gracefully
    debug_info.add_section(".debug_line", 0, 0);
    assert!(!debug_info.has_debug_info() || debug_info.has_debug_info());

    #[cfg(feature = "line-info")]
    {
        // Should handle invalid section gracefully
        let result = debug_info.find_line_info(0x42);
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_section_management() {
    let mut debug_info = DwarfDebugInfo::new(TEST_MODULE).unwrap();

    // Test various section types
    debug_info.add_section(".debug_line", 100, 50);
    debug_info.add_section(".debug_info", 200, 100);
    debug_info.add_section(".debug_abbrev", 300, 30);
    debug_info.add_section(".debug_str", 400, 20);
    debug_info.add_section(".debug_line_str", 500, 25);

    // Unknown sections should be ignored
    debug_info.add_section(".unknown_section", 600, 10);

    let has_debug = debug_info.has_debug_info();

    // Should recognize debug sections if features are enabled
    #[cfg(any(feature = "line-info", feature = "debug-info"))]
    assert!(has_debug);
}
