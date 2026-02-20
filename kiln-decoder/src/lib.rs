// WRT - wrt-decoder
// Module: WebAssembly Binary Decoder
// SW-REQ-ID: REQ_013
// SW-REQ-ID: REQ_SAFETY_DECODE_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)] // Rule 2

//! WebAssembly module decoder for wrt runtime
//!
//! This crate provides a high-level API for decoding WebAssembly binary modules
//! into structured representations that can be used by the wrt runtime.
//!
//! The decoder sits between the low-level binary format handling in
//! `wrt-format` and the runtime execution in `wrt`. It properly converts
//! between format types and runtime types, ensuring type consistency across the
//! system.
//!
//! # Features
//!
//! - Decoding WebAssembly modules from binary format
//! - Encoding modules back to binary format
//! - Validating module structure
//! - Memory-efficient handling of WASM modules
//! - Component model support
//! - No_std and std environment support
//!
//! ## Feature Flags
//!
//! - `std` (default): Enable standard library support
//! - `alloc`: Enable allocation support (required for no_std)
//! - `component`: Enable WebAssembly Component Model support
//! - `safe_memory`: Enable memory safety features

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(clippy::missing_panics_doc)]
#![allow(missing_docs)] // Temporarily disabled for build

// Import core
extern crate core;

// Import std when available
#[cfg(feature = "std")]
extern crate std;

// Binary std/no_std choice
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Note: Panic handler removed to avoid conflicts with std library

// Module exports
// Core memory optimization modules (always available)
pub mod decoder;
pub mod format_detection_tests;
pub mod lazy_detection;
pub mod memory_optimized;
pub mod optimized_string;
pub mod prelude;
pub mod shared_cache;
pub mod streaming_decoder;
pub mod streaming_validation;
pub mod streaming_validator;
pub mod unified_loader;

// Bounded infrastructure for static memory allocation
pub mod bounded_decoder_infra;

// Simple types using StaticVec (Provider-free migration)
pub mod simple_types;

// Section parsing - use bounded version in no_std
#[cfg(feature = "std")]
pub mod sections;
#[cfg(not(feature = "std"))]
pub mod sections_no_std;
#[cfg(not(feature = "std"))]
pub use sections_no_std as sections;

// Conditionally include other modules
pub mod component;
#[cfg(feature = "std")]
pub mod utils;

// Safety-critical memory limits
#[cfg(feature = "safety-critical")]
pub mod memory_limits;

// Binary std/no_std choice
pub mod decoder_no_alloc;

// Binary std/no_std choice
#[cfg(feature = "std")]
pub mod branch_hint_section;
#[cfg(feature = "std")]
pub mod custom_section_handler;

// Resource limits section - now ASIL-D compatible (no external dependencies)
pub mod resource_limits_section;

// TOML configuration parser for resource limits (std only for tooling)
#[cfg(feature = "std")]
pub mod toml_config;

// Most re-exports temporarily disabled for demo - keep only essential ones
// Component functionality (std only)
#[cfg(feature = "std")]
pub use component::decode_no_alloc;
pub use decoder_no_alloc::{
    MAX_MODULE_SIZE, SectionId, SectionInfo, ValidatorType, WasmModuleHeader,
    create_memory_provider, decode_module_header, extract_section_info, validate_module_no_alloc,
    verify_wasm_header,
};
// Lazy detection exports
pub use lazy_detection::{
    ComponentDetection, DetectionConfig, LazyDetector, create_fast_detector,
    create_thorough_detector,
};
// Shared cache exports
pub use shared_cache::{
    CacheManager, CacheStats, DecodedCache, SectionData, create_cache_with_size,
    create_default_cache,
};
// Streaming validator exports
pub use streaming_validator::{
    CodeSection, ComprehensivePlatformLimits, MemorySection, PlatformId,
    PlatformWasmValidatorFactory, Section, StreamingWasmValidator, WasmConfiguration,
    WasmRequirements,
};
// Unified loader exports
pub use unified_loader::{
    ComponentInfo, ExportInfo, ExportType, ImportInfo, ImportType, ModuleInfo, WasmFormat,
    WasmInfo, load_wasm_unified,
};
pub use wrt_error::{Error, Result, codes, kinds};
// Essential re-exports only
#[cfg(feature = "std")]
pub use wrt_foundation::safe_memory::StdProvider as StdMemoryProvider;
pub use wrt_foundation::safe_memory::{MemoryProvider, SafeSlice};

/// Validate WebAssembly header
///
/// # Arguments
///
/// * `bytes` - WebAssembly binary data
///
/// # Returns
///
/// * `Result<()>` - Success or error
pub fn validate_header(bytes: &[u8]) -> Result<()> {
    verify_wasm_header(bytes)
}

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-decoder is being tested in isolation
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
