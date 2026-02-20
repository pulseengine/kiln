// Copyright (c) 2025 R T
// SPDX-License-Identifier: MIT
// Project: WRT
// Module: wrt-math (SW-REQ-ID-TBD)

//! Mathematical operations and types for WRT.
//! Provides implementations for WebAssembly numeric instructions.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![deny(clippy::todo, clippy::unimplemented)]
#![warn(clippy::pedantic)]
// Allow specific lints necessary for low-level math/Wasm ops, matching Cargo.toml
#![allow(clippy::float_arithmetic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_cmp)]
// Allow documentation and other noisy lints temporarily to focus on core logic
#![allow(clippy::missing_errors_doc)] // TODO: Add detailed error docs
#![allow(clippy::doc_markdown)] // TODO: Fix all doc markdown issues
#![allow(clippy::items_after_statements)] // TODO: Revisit and move consts to module level properly

// Import std when available
#[cfg(feature = "std")]
extern crate std;

// Binary std/no_std choice
#[cfg(feature = "std")]
extern crate alloc;

// Modules
pub mod float_bits;
pub mod ops;
pub mod prelude;
pub mod safety;
pub mod traits;

// SIMD operations module (requires platform feature)
#[cfg(feature = "platform")]
pub mod simd;

// Re-export key types and potentially functions for easier access
pub use float_bits::{
    FloatBits32,
    FloatBits64,
};
// Re-export all operations from the ops module
pub use ops::*; // Consider selectively exporting if API needs to be controlled
// Re-export safety operations
pub use safety::{
    check_simd_bounds,
    RoundingMode,
    SafeArithmetic,
    SafeFloat,
    SafeRounding,
};
// Re-export SIMD operations when platform feature is enabled
#[cfg(feature = "platform")]
pub use simd::SimdOperations;
// Re-export error type from wrt-error for convenience
pub use wrt_error::Error as WrtMathError; // Alias specific to this crate context
pub use wrt_error::Result as WrtMathResult; // Alias specific to this crate context

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-math is being tested in isolation
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
