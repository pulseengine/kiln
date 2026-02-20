// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Minimal example demonstrating wrt-debug with no features enabled
//!
//! This example shows the absolute minimum debug functionality when all
//! optional features are disabled. Only basic section registration is
//! available.

#![no_std]
#![no_main]

use wrt_debug::prelude::*;

// Example module bytes
const MODULE_BYTES: &[u8] = &[
    0x00, 0x61, 0x73, 0x6D, // \0asm
    0x01, 0x00, 0x00, 0x00, // version 1
];

#[no_mangle]
pub extern "C" fn minimal_debug_usage() -> bool {
    // Create debug info parser - returns Result now
    let mut debug_info = match DwarfDebugInfo::new(MODULE_BYTES) {
        Ok(info) => info,
        Err(_) => return false,
    };

    // Register sections - this always works
    debug_info.add_section(".debug_line", 0x1000, 0x500);
    debug_info.add_section(".debug_info", 0x1500, 0x800);

    // Check if debug info is available - this always works
    debug_info.has_debug_info()
}

/// Example showing conditional compilation based on features
#[no_mangle]
pub extern "C" fn conditional_debug_features() -> u32 {
    let mut debug_info = match DwarfDebugInfo::new(MODULE_BYTES) {
        Ok(info) => info,
        Err(_) => return 0,
    };
    debug_info.add_section(".debug_line", 0x1000, 0x500);

    let mut feature_count = 0;

    // This code only compiles if line-info feature is enabled
    #[cfg(feature = "line-info")]
    {
        if let Ok(Some(_line_info)) = debug_info.find_line_info(0x42) {
            feature_count += 1;
        }
    }

    // This code only compiles if function-info feature is enabled
    #[cfg(feature = "function-info")]
    {
        if let Some(_func_info) = debug_info.find_function_info(0x42) {
            feature_count += 10;
        }
    }

    feature_count
}

// Panic handler removed - provided by wrt-platform crate
