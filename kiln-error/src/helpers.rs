// WRT - wrt-error
// Module: WRT Error Helpers
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Error helper functions for common error patterns.
//!
//! This module provides helper functions for creating common error types,
//! including foundation-specific errors for the new unified type system,
//! memory providers, and safety primitives.

// Re-export error kind creation functions
pub use crate::kinds::*;
use crate::{Error, ErrorCategory, codes};

/// Create a safety violation error
#[must_use]
pub const fn safety_violation_error(message: &'static str) -> Error {
    Error::safety_violation(message)
}

/// Create a safety ASIL violation error
#[must_use]
pub const fn safety_asil_violation_error(message: &'static str) -> Error {
    Error::safety_asil_violation(message)
}

/// Create a memory corruption detected error
#[must_use]
pub const fn memory_corruption_error(message: &'static str) -> Error {
    Error::memory_corruption_detected(message)
}

/// Create a verification failed error
#[must_use]
pub const fn verification_failed_error(message: &'static str) -> Error {
    Error::verification_failed(message)
}

/// Create a unified type configuration error
#[must_use]
pub const fn unified_type_config_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Type,
        codes::UNIFIED_TYPE_CONFIG_ERROR,
        message,
    )
}

/// Create a platform capacity mismatch error
#[must_use]
pub const fn platform_capacity_mismatch_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Capacity,
        codes::PLATFORM_CAPACITY_MISMATCH,
        message,
    )
}

/// Create a memory provider creation error
#[must_use]
pub const fn memory_provider_creation_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Memory,
        codes::MEMORY_PROVIDER_CREATION_ERROR,
        message,
    )
}

/// Create a memory allocation failed error
#[must_use]
pub const fn memory_allocation_failed_error(message: &'static str) -> Error {
    Error::platform_memory_allocation_failed(message)
}

/// Create a memory provider capacity exceeded error
#[must_use]
pub const fn memory_provider_capacity_exceeded_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Capacity,
        codes::MEMORY_PROVIDER_CAPACITY_EXCEEDED,
        message,
    )
}

/// Create a bounded collection capacity exceeded error
#[must_use]
pub const fn bounded_collection_capacity_exceeded_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Capacity,
        codes::BOUNDED_COLLECTION_CAPACITY_EXCEEDED,
        message,
    )
}

/// Create a bounded collection invalid capacity error
#[must_use]
pub const fn bounded_collection_invalid_capacity_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Validation,
        codes::BOUNDED_COLLECTION_INVALID_CAPACITY,
        message,
    )
}

/// Create a bounded collection conversion error
#[must_use]
pub const fn bounded_collection_conversion_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Type,
        codes::BOUNDED_COLLECTION_CONVERSION_ERROR,
        message,
    )
}

/// Create an invalid value error
#[must_use]
pub const fn invalid_value_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::Validation, codes::INVALID_VALUE, message)
}

/// Create an unimplemented feature error
#[must_use]
pub const fn unimplemented_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::NotSupported, codes::UNIMPLEMENTED, message)
}

/// Create a conversion error
#[must_use]
pub const fn conversion_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::Type, codes::CONVERSION_ERROR, message)
}

// Agent B helper stubs
/// Create a platform detection failed error
#[must_use]
pub const fn platform_detection_failed_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::System,
        codes::PLATFORM_DETECTION_FAILED,
        message,
    )
}

/// Create a memory limit exceeded error
#[must_use]
pub const fn memory_limit_exceeded_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Capacity,
        codes::MEMORY_LIMIT_EXCEEDED,
        message,
    )
}

/// Create a stack limit exceeded error
#[must_use]
pub const fn stack_limit_exceeded_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Capacity,
        codes::STACK_LIMIT_EXCEEDED,
        message,
    )
}

// Agent C helper stubs
/// Create a WIT input too large error
#[must_use]
pub const fn wit_input_too_large_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::Parse, codes::WIT_INPUT_TOO_LARGE, message)
}

/// Create an insufficient memory error
#[must_use]
pub const fn insufficient_memory_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::Memory, codes::INSUFFICIENT_MEMORY, message)
}

/// Create a resource type limit exceeded error
#[must_use]
pub const fn resource_type_limit_exceeded_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Resource,
        codes::RESOURCE_TYPE_LIMIT_EXCEEDED,
        message,
    )
}

// Agent D helper stubs
/// Create a CFI validation failed error
#[must_use]
pub const fn cfi_validation_failed_error(message: &'static str) -> Error {
    Error::cfi_validation_failed(message)
}

/// Create a CFI unsupported error
#[must_use]
pub const fn cfi_unsupported_error(message: &'static str) -> Error {
    Error::new(ErrorCategory::NotSupported, codes::CFI_UNSUPPORTED, message)
}

/// Create an execution engine error
#[must_use]
pub const fn execution_engine_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Runtime,
        codes::EXECUTION_ENGINE_ERROR,
        message,
    )
}

// ASIL-specific error helpers

/// Create an ASIL violation error (ASIL-B and above)
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
#[must_use]
pub const fn asil_violation_error(_level: &'static str, message: &'static str) -> Error {
    Error::safety_asil_violation(message)
}

/// Create a safety-critical memory error (ASIL-C and above)
#[cfg(any(feature = "asil-c", feature = "asil-d"))]
#[must_use]
pub const fn safety_critical_memory_error(message: &'static str) -> Error {
    Error::new(
        ErrorCategory::Memory,
        codes::MEMORY_CORRUPTION_DETECTED,
        message,
    )
}

/// Create a determinism violation error (ASIL-D only)
#[cfg(feature = "asil-d")]
#[must_use]
pub const fn determinism_violation_error(message: &'static str) -> Error {
    Error::determinism_violation(message)
}

/// Create a redundancy check failure error (ASIL-D only)
#[cfg(feature = "asil-d")]
#[must_use]
pub const fn redundancy_check_failure_error(message: &'static str) -> Error {
    Error::redundancy_check_failure(message)
}
