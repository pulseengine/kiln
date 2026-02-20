// WRT - wrt-error
// Module: WRT Error Handling
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WRT Error handling library
//!
//! This library provides a comprehensive error handling system for the WRT
//! runtime. It includes error types, helper functions, and utilities for
//! creating and managing errors.
//!
//! # Error Categories
//!
//! Errors are organized into several categories, each with its own range of
//! error codes:
//!
//! ## Core Errors (1000-1008)
//! - Stack underflow
//! - Memory access violations
//! - Instance index errors
//! - Execution errors
//! - Type mismatches
//!
//! ## Runtime Errors (2000-2021)
//! - Invalid indices (local, global, function)
//! - Memory and table access errors
//! - Resource exhaustion
//! - Validation failures
//! - Parse errors
//!
//! ## Component Errors (3000-3013)
//! - Function index errors
//! - Type mismatches
//! - Resource limits
//! - Component lifecycle errors
//! - ABI errors
//!
//! ## System Errors (0x1000-0x1001)
//! - Async operation errors
//! - Threading errors
//!
//! # Usage
//!
//! The library provides both low-level error types and high-level helper
//! functions:
//!
//! ```
//! // Binary std/no_std choice
//! use wrt_error::{
//!     kinds,
//!     Error,
//! };
//!
//! // Using helper functions for common errors
//! let error = Error::new(
//!     wrt_error::ErrorCategory::Core,
//!     wrt_error::codes::INVALID_FUNCTION_INDEX,
//!     "Invalid function index: 42",
//! );
//!
//! // Using kind functions for common errors
//! let index_error = kinds::invalid_index_error("function");
//! let memory_error = kinds::memory_access_error("Memory access out of bounds");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)] // Rule 2
#![deny(clippy::all)]
#![deny(clippy::perf)]
#![deny(clippy::nursery)]
#![allow(clippy::cargo)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![deny(missing_docs)]
#![allow(clippy::negative_feature_names)]
#![allow(clippy::module_name_repetitions)]

// Standard library support
#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Error codes for wrt
pub mod codes;
/// Error and error handling types
pub mod errors;
/// Error kind definitions
pub mod kinds;

// Modules
pub mod context;
pub mod helpers;
pub mod prelude;
pub mod recovery;

// ASIL safety support (enabled for ASIL-B and above)
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
pub mod asil;

// Macros for ASIL-aware error handling
#[macro_use]
pub mod macros;

// Include verification module conditionally, but exclude during coverage builds
#[cfg(all(not(coverage), doc))]
pub mod verify;

// Re-export key types
pub use errors::{Error, ErrorCategory, ErrorSource};

/// A specialized `Result` type for WRT operations.
///
/// This type alias uses `wrt_error::Error` as the error type.
/// It is suitable for `no_std` environments as `wrt_error::Error`
/// Binary `std/no_std` choice
pub type Result<T> = core::result::Result<T, Error>;

// Re-export error kinds for convenience
pub use kinds::{
    ComponentError, InvalidType, OutOfBoundsError, ParseError, PoisonedLockError, ResourceError,
    RuntimeError, ValidationError, component_error, invalid_type, out_of_bounds_error, parse_error,
    poisoned_lock_error, resource_error, runtime_error, validation_error,
};

/// Error conversion trait for converting between error types
///
/// This trait provides a standardized way to convert between error types
/// across the WRT codebase. It is used to ensure a consistent error
/// handling approach across all crates.
pub trait FromError<E> {
    /// Convert from the source error type to the target error type
    fn from_error(error: E) -> Self;
}

/// Error conversion trait for converting to specific error categories
///
/// This trait provides a way to convert any error to a specific error
/// category, which is useful for creating category-specific errors.
pub trait ToErrorCategory {
    /// Convert the error to a specific category
    fn to_category(&self) -> ErrorCategory;
}

// Re-export additional helpers
#[cfg(any(feature = "asil-c", feature = "asil-d"))]
pub use asil::SafetyMonitor;
#[cfg(feature = "asil-d")]
pub use asil::validate_error_consistency;
// Re-export ASIL types when enabled
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
pub use asil::{AsilErrorContext, AsilLevel};
pub use helpers::*;

/// A placeholder function.
pub const fn placeholder() {}

// Panic handler disabled to avoid conflicts with other crates
// The main wrt crate should provide the panic handler
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
