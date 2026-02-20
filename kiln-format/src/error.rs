//! Error handling module for format specification
//!
//! This module provides error handling functionality for the format
//! specification.

#[cfg(feature = "std")]
use std::{boxed::Box, string::String};

use wrt_error::Error;
#[cfg(not(feature = "std"))]
use wrt_foundation::bounded::BoundedString;

/// Module for error codes
pub mod codes {
    /// Parse error code
    pub const PARSE_ERROR: u16 = 1010;
    /// Validation error code
    pub const VALIDATION_ERROR: u16 = 5000;
    /// Type mismatch error code
    pub const TYPE_MISMATCH_ERROR: u16 = 6001;
    /// Runtime error code
    pub const RUNTIME_ERROR: u16 = 7000;
}

/// Create a simple parse error with the given message
pub fn parse_error(message: &'static str) -> Error {
    Error::parse_error(message)
}

/// Create a parse error from a String (for dynamic messages)
/// Note: This leaks the string memory, so use sparingly
#[cfg(feature = "std")]
pub fn parse_error_dynamic(message: String) -> Error {
    let leaked: &'static str = Box::leak(message.into_boxed_str());
    Error::parse_error(leaked)
}

/// Create a new runtime error with the given message
pub fn runtime_error(message: &'static str) -> Error {
    Error::runtime_error(message)
}

/// Create a new validation error with the given message
pub fn validation_error(message: &'static str) -> Error {
    Error::validation_error(message)
}

/// Create a validation error from a String (for dynamic messages)
/// Note: This leaks the string memory, so use sparingly
#[cfg(feature = "std")]
pub fn validation_error_dynamic(message: String) -> Error {
    let leaked: &'static str = Box::leak(message.into_boxed_str());
    Error::validation_error(leaked)
}

/// Create a new type error with the given message
pub fn type_error(message: &'static str) -> Error {
    Error::type_error(message)
}

/// Convert an error to a WRT error
// This function is problematic without format! or a way to get a static string
// from E. For now, it will assume E provides a static str or this function
// needs to be re-evaluated. A common pattern for E: fmt::Display is to have E
// be an enum where each variant can provide a static description.
pub fn to_wrt_error(message: &'static str) -> Error {
    Error::system_error(message)
}

/// Trait for converting a type to a WRT error
pub trait IntoError {
    /// Convert self to a WRT error
    fn into_error(self) -> Error;
}

/// Create a new runtime error with the given message
pub fn wrt_runtime_error(message: &'static str) -> Error {
    Error::runtime_error(message)
}

/// Create a new validation error with the given message
pub fn wrt_validation_error(message: &'static str) -> Error {
    Error::validation_error(message)
}

/// Create a new type error with the given message
pub fn wrt_type_error(message: &'static str) -> Error {
    Error::type_error(message)
}

#[cfg(test)]
mod tests {
    use wrt_error::{ErrorCategory, ErrorSource};

    use super::*;

    #[test]
    fn test_error_creation() {
        let error = parse_error("test error");
        assert_eq!(error.category(), ErrorCategory::Parse);
        assert_eq!(error.code(), codes::PARSE_ERROR);
    }
}
