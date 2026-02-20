// WRT - wrt-error
// Module: WRT Error Types
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

/// Unified error handling system for WRT
///
/// This module provides a comprehensive error handling system that covers all
/// error cases across the WRT codebase. It includes error types, categories,
/// and helper functions.
use core::fmt;

use crate::{
    FromError, ToErrorCategory, codes, kinds,
    prelude::{Debug, Eq, PartialEq, str},
};

/// `Error` categories for WRT operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorCategory {
    /// Core WebAssembly errors
    Core = 1,
    /// Component model errors
    Component = 2,
    /// Resource errors (memory, tables, etc.)
    Resource = 3,
    /// Memory errors
    Memory = 4,
    /// Validation errors
    Validation = 5,
    /// Type errors
    Type = 6,
    /// Runtime errors (general)
    Runtime = 7,
    /// System errors
    System = 8,
    /// I/O errors
    Io = 20,
    /// Unknown errors
    Unknown = 9,
    /// Parse errors
    Parse = 10,
    /// Concurrency errors
    Concurrency = 11,
    /// Capacity errors
    Capacity = 12,
    /// WebAssembly trap errors (specific runtime errors defined by Wasm spec)
    RuntimeTrap = 13,
    /// Initialization errors
    Initialization = 14,
    /// Not supported operation errors
    NotSupported = 15,
    /// Safety-related errors (ASIL violations, integrity checks, etc.)
    Safety = 16,
    /// Security-related errors (access control, permissions, etc.)
    Security = 17,
    /// Parameter-related errors (invalid arguments, missing parameters, etc.)
    Parameter = 18,
    /// Verification errors (proofs, checksums, integrity, etc.)
    Verification = 19,
    /// Component model runtime operations (threading, resource management)
    ComponentRuntime = 24,
    /// Platform-specific runtime failures (hardware, real-time constraints)
    PlatformRuntime = 25,
    /// Foundation runtime constraint violations (bounded collections, safety)
    FoundationRuntime = 26,
    /// Async/threading runtime errors
    AsyncRuntime = 27,
    /// Platform-specific errors
    Platform = 28,
    /// Invalid state errors
    InvalidState = 29,
    /// Not implemented errors
    NotImplemented = 30,
    /// Invalid input errors
    InvalidInput = 31,
    /// Async operation errors
    Async = 32,
}

/// Base trait for all error types - `no_std` version
pub trait ErrorSource: fmt::Debug + Send + Sync {
    /// Get the error code
    fn code(&self) -> u16;

    /// Get the error message
    fn message(&self) -> &'static str;

    /// Get the error category
    fn category(&self) -> ErrorCategory;
}

/// WRT `Error` type
///
/// This is the main error type for the WebAssembly Runtime.
/// It provides categorized errors with error codes and optional messages.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Error {
    /// `Error` category
    pub category: ErrorCategory,
    /// `Error` code
    pub code: u16,
    /// `Error` message
    pub message: &'static str,
}

impl Error {
    /// Component not found error
    pub const COMPONENT_NOT_FOUND: Self = Self::new(
        ErrorCategory::Component,
        codes::COMPONENT_NOT_FOUND,
        "Component not found",
    );
    /// Insufficient memory error
    pub const INSUFFICIENT_MEMORY: Self = Self::new(
        ErrorCategory::Resource,
        codes::INSUFFICIENT_MEMORY,
        "Insufficient memory for operation",
    );
    /// No WIT definitions found error
    pub const NO_WIT_DEFINITIONS_FOUND: Self = Self::new(
        ErrorCategory::Parse,
        codes::NO_WIT_DEFINITIONS_FOUND,
        "No WIT worlds or interfaces found in input",
    );
    /// Out of memory error
    pub const OUT_OF_MEMORY: Self = Self::new(
        ErrorCategory::Resource,
        codes::OUT_OF_MEMORY,
        "Out of memory",
    );
    /// Stack overflow error
    pub const STACK_OVERFLOW: Self = Self::new(
        ErrorCategory::Runtime,
        codes::STACK_OVERFLOW,
        "Stack overflow",
    );
    /// Too many components error
    pub const TOO_MANY_COMPONENTS: Self = Self::new(
        ErrorCategory::Component,
        codes::TOO_MANY_COMPONENTS,
        "Too many components instantiated",
    );
    // Agent C constant error instances
    /// WIT input too large error
    pub const WIT_INPUT_TOO_LARGE: Self = Self::new(
        ErrorCategory::Parse,
        codes::WIT_INPUT_TOO_LARGE,
        "WIT input too large for parser buffer",
    );
    /// WIT interface limit exceeded error
    pub const WIT_INTERFACE_LIMIT_EXCEEDED: Self = Self::new(
        ErrorCategory::Parse,
        codes::WIT_INTERFACE_LIMIT_EXCEEDED,
        "Too many WIT interfaces for parser limits",
    );
    /// WIT world limit exceeded error
    pub const WIT_WORLD_LIMIT_EXCEEDED: Self = Self::new(
        ErrorCategory::Parse,
        codes::WIT_WORLD_LIMIT_EXCEEDED,
        "Too many WIT worlds for parser limits",
    );

    /// Create a new error.
    #[must_use]
    pub const fn new(category: ErrorCategory, code: u16, message: &'static str) -> Self {
        // ASIL-D: Validate error category and code ranges at compile time
        #[cfg(feature = "asil-d")]
        {
            // Note: const fn limitations prevent runtime assertions, but this
            // documents the expected ranges for each category. In a
            // full implementation, these would be enforced through
            // the type system or compile-time checks.
        }

        Self {
            category,
            code,
            message,
        }
    }

    /// Get the error message
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }

    /// Get the error code
    #[must_use]
    pub const fn code(&self) -> u16 {
        self.code
    }

    /// Get the error category
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        self.category
    }

    /// Create a component error with dynamic context (using static fallback)
    #[must_use]
    pub const fn component_error(_message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::COMPONENT_ERROR,
            "Component error",
        )
    }

    /// Create a WIT parse error with dynamic message (using static fallback)
    #[must_use]
    pub const fn wit_parse_error(_message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Parse,
            codes::WIT_PARSE_ERROR,
            "WIT parse error",
        )
    }

    /// Create an invalid input error with dynamic message (using static
    /// fallback)
    #[must_use]
    pub const fn invalid_input(_message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::INVALID_INPUT,
            "Invalid input",
        )
    }

    /// Create an unsupported error with dynamic message (using static fallback)
    #[must_use]
    pub const fn unsupported(_message: &'static str) -> Self {
        Self::new(
            ErrorCategory::System,
            codes::UNSUPPORTED,
            "Unsupported operation",
        )
    }

    /// Check if this is a resource error
    #[must_use]
    pub fn is_resource_error(&self) -> bool {
        self.category == ErrorCategory::Resource
    }

    /// Check if this is a memory error
    #[must_use]
    pub fn is_memory_error(&self) -> bool {
        self.category == ErrorCategory::Memory
    }

    /// Check if this is a validation error
    #[must_use]
    pub fn is_validation_error(&self) -> bool {
        self.category == ErrorCategory::Validation
    }

    /// Check if this is a type error
    #[must_use]
    pub fn is_type_error(&self) -> bool {
        self.category == ErrorCategory::Type
    }

    /// Check if this is a runtime error
    #[must_use]
    pub fn is_runtime_error(&self) -> bool {
        self.category == ErrorCategory::Runtime
    }

    /// Check if this is a system error
    #[must_use]
    pub fn is_system_error(&self) -> bool {
        self.category == ErrorCategory::System
    }

    /// Check if this is a core error
    #[must_use]
    pub fn is_core_error(&self) -> bool {
        self.category == ErrorCategory::Core
    }

    /// Check if this is a component error
    #[must_use]
    pub fn is_component_error(&self) -> bool {
        self.category == ErrorCategory::Component
    }

    /// Check if this is a safety error
    #[must_use]
    pub fn is_safety_error(&self) -> bool {
        self.category == ErrorCategory::Safety
    }

    /// Check if this is a component runtime error
    #[must_use]
    pub fn is_component_runtime_error(&self) -> bool {
        self.category == ErrorCategory::ComponentRuntime
    }

    /// Check if this is a platform runtime error
    #[must_use]
    pub fn is_platform_runtime_error(&self) -> bool {
        self.category == ErrorCategory::PlatformRuntime
    }

    /// Check if this is a foundation runtime error
    #[must_use]
    pub fn is_foundation_runtime_error(&self) -> bool {
        self.category == ErrorCategory::FoundationRuntime
    }

    /// Check if this is an async runtime error
    #[must_use]
    pub fn is_async_runtime_error(&self) -> bool {
        self.category == ErrorCategory::AsyncRuntime
    }

    /// Get the ASIL level of this error (ASIL-B and above)
    #[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
    #[must_use]
    pub const fn asil_level(&self) -> &'static str {
        match self.category {
            ErrorCategory::Safety | ErrorCategory::FoundationRuntime => "ASIL-D", /* Safety and foundation errors require highest level */
            ErrorCategory::Memory
            | ErrorCategory::RuntimeTrap
            | ErrorCategory::ComponentRuntime => "ASIL-C", /* Memory/trap/component runtime */
            // errors are ASIL-C
            ErrorCategory::Validation
            | ErrorCategory::Type
            | ErrorCategory::PlatformRuntime
            | ErrorCategory::AsyncRuntime
            | ErrorCategory::Async
            | ErrorCategory::InvalidInput => "ASIL-B", // Type safety, input validation, platform runtime, and async runtime are ASIL-B
            _ => "QM", // Other errors are Quality Management level
        }
    }

    /// Check if error requires immediate safe state transition (ASIL-C and
    /// above)
    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    #[must_use]
    pub const fn requires_safe_state(&self) -> bool {
        matches!(
            self.category,
            ErrorCategory::Safety
                | ErrorCategory::Memory
                | ErrorCategory::RuntimeTrap
                | ErrorCategory::ComponentRuntime
                | ErrorCategory::FoundationRuntime
        )
    }

    /// Validate error integrity (ASIL-D only)
    #[cfg(feature = "asil-d")]
    #[must_use]
    pub const fn validate_integrity(&self) -> bool {
        // Check that error code is within valid range for category
        let valid_range = match self.category {
            ErrorCategory::Core => self.code >= 1000 && self.code < 2000,
            ErrorCategory::Component => self.code >= 2000 && self.code < 3000,
            ErrorCategory::Resource => self.code >= 3000 && self.code < 4000,
            ErrorCategory::Memory => self.code >= 4000 && self.code < 5000,
            ErrorCategory::Validation => self.code >= 5000 && self.code < 6000,
            ErrorCategory::Type => self.code >= 6000 && self.code < 7000,
            ErrorCategory::Runtime | ErrorCategory::Safety => self.code >= 7000 && self.code < 8000,
            ErrorCategory::System => self.code >= 8000 && self.code < 9000,
            ErrorCategory::ComponentRuntime => self.code >= 24000 && self.code < 25000,
            ErrorCategory::PlatformRuntime => self.code >= 25000 && self.code < 26000,
            ErrorCategory::FoundationRuntime => self.code >= 26000 && self.code < 27000,
            ErrorCategory::AsyncRuntime => self.code >= 27000 && self.code < 28000,
            ErrorCategory::InvalidInput => self.code >= 31000 && self.code < 32000,
            ErrorCategory::Async => self.code >= 32000 && self.code < 33000,
            _ => self.code >= 9000 && self.code <= 9999,
        };

        // Check that message is not empty (basic integrity check)
        valid_range && !self.message.is_empty()
    }

    // Factory methods

    /// Create a resource error
    #[must_use]
    pub const fn resource_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::RESOURCE_ERROR, message)
    }

    /// Create a memory error
    #[must_use]
    pub const fn memory_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::MEMORY_OUT_OF_BOUNDS, message)
    }

    /// Create a validation error
    #[must_use]
    pub const fn validation_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::VALIDATION_ERROR, message)
    }

    /// Create a type error
    #[must_use]
    pub const fn type_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Type, codes::TYPE_MISMATCH_ERROR, message)
    }

    /// Create a runtime error
    #[must_use]
    pub const fn runtime_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RUNTIME_ERROR, message)
    }

    /// Create a runtime stack overflow error
    #[must_use]
    pub const fn runtime_stack_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::STACK_OVERFLOW, message)
    }

    /// Create a runtime stack underflow error
    #[must_use]
    pub const fn runtime_stack_underflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::STACK_UNDERFLOW, message)
    }

    /// Create a runtime invalid state error
    #[must_use]
    pub const fn runtime_invalid_state(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::INVALID_STATE, message)
    }

    /// Create a runtime execution error
    #[must_use]
    pub const fn runtime_execution_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::EXECUTION_ERROR, message)
    }

    /// Create a runtime unaligned memory access error
    #[must_use]
    pub const fn runtime_unaligned_memory_access(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::UNALIGNED_MEMORY_ACCESS,
            message,
        )
    }

    /// Create a runtime memory access error
    #[must_use]
    pub const fn runtime_memory_access_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::MEMORY_ACCESS_ERROR, message)
    }

    /// Create a runtime integer overflow error
    #[must_use]
    pub const fn runtime_integer_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::INTEGER_OVERFLOW, message)
    }

    /// Create a runtime division by zero error
    #[must_use]
    pub const fn runtime_division_by_zero(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::DIVISION_BY_ZERO, message)
    }

    /// Create a runtime trap error (Runtime category)
    #[must_use]
    pub const fn runtime_trap(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RUNTIME_TRAP_ERROR, message)
    }

    /// Create a runtime out of bounds error
    #[must_use]
    pub const fn runtime_out_of_bounds(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::OUT_OF_BOUNDS_ERROR, message)
    }

    /// Create a runtime type mismatch error
    #[must_use]
    pub const fn runtime_type_mismatch(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::TYPE_MISMATCH_ERROR, message)
    }

    /// Create a poisoned lock error
    #[must_use]
    pub const fn poisoned_lock(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::POISONED_LOCK, message)
    }

    /// Create a runtime poisoned lock error (alias for `poisoned_lock`)
    #[must_use]
    pub const fn runtime_poisoned_lock(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::POISONED_LOCK, message)
    }

    /// Create a runtime function not found error
    #[must_use]
    pub const fn runtime_function_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::FUNCTION_NOT_FOUND, message)
    }

    /// Create a runtime null reference error
    #[must_use]
    pub const fn runtime_null_reference(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::NULL_REFERENCE, message)
    }

    /// Create a runtime invalid parameter error
    #[must_use]
    pub const fn runtime_invalid_parameter(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::INVALID_PARAMETER, message)
    }

    /// Create a resource not found error
    #[must_use]
    pub const fn resource_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RESOURCE_NOT_FOUND, message)
    }

    /// Create a memory not found error
    #[must_use]
    pub const fn memory_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::MEMORY_NOT_FOUND, message)
    }

    /// Create a resource exhausted error
    #[must_use]
    pub const fn resource_exhausted(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::RESOURCE_EXHAUSTED, message)
    }

    /// Create a resource invalid handle error
    #[must_use]
    pub const fn resource_invalid_handle(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::RESOURCE_INVALID_HANDLE,
            message,
        )
    }

    /// Create a WASI invalid file descriptor error
    #[must_use]
    pub const fn wasi_invalid_fd(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::WASI_INVALID_FD, message)
    }

    /// Create a WASI permission denied error
    #[must_use]
    pub const fn wasi_permission_denied(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::WASI_PERMISSION_DENIED,
            message,
        )
    }

    /// Create a WASI resource limit error
    #[must_use]
    pub const fn wasi_resource_limit(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::WASI_RESOURCE_LIMIT, message)
    }

    /// Create a runtime not implemented error
    #[must_use]
    pub const fn runtime_not_implemented(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::NOT_IMPLEMENTED, message)
    }

    /// Create a runtime invalid argument error
    #[must_use]
    pub const fn runtime_invalid_argument(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_INVALID_ARGUMENT_ERROR,
            message,
        )
    }

    /// Create a runtime unsupported operation error
    #[must_use]
    pub const fn runtime_unsupported_operation(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::UNSUPPORTED_OPERATION,
            message,
        )
    }

    /// Create a validation control flow error
    #[must_use]
    pub const fn validation_control_flow_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::VALIDATION_CONTROL_FLOW_ERROR,
            message,
        )
    }

    /// Create a validation value type error
    #[must_use]
    pub const fn validation_value_type_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::VALIDATION_VALUE_TYPE_ERROR,
            message,
        )
    }

    /// Create a runtime table not found error
    #[must_use]
    pub const fn runtime_table_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::TABLE_NOT_FOUND, message)
    }

    /// Create a runtime concurrency error
    #[must_use]
    pub const fn runtime_concurrency_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::CONCURRENCY_ERROR, message)
    }

    /// Create a WASI capability unavailable error
    #[must_use]
    pub const fn wasi_capability_unavailable(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::WASI_CAPABILITY_UNAVAILABLE,
            message,
        )
    }

    /// Create a WASI invalid argument error
    #[must_use]
    pub const fn wasi_invalid_argument(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::WASI_INVALID_ARGUMENT,
            message,
        )
    }

    /// Create a WASI invalid encoding error
    #[must_use]
    pub const fn wasi_invalid_encoding(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::WASI_INVALID_ENCODING,
            message,
        )
    }

    /// Create a WASI runtime error
    #[must_use]
    pub const fn wasi_runtime_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::WASI_RUNTIME_ERROR, message)
    }

    /// Create a WASI resource exhausted error
    #[must_use]
    pub const fn wasi_resource_exhausted(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::WASI_RESOURCE_EXHAUSTED,
            message,
        )
    }

    /// Create a WASI unsupported operation error
    #[must_use]
    pub const fn wasi_unsupported_operation(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::NotSupported,
            codes::WASI_UNSUPPORTED_OPERATION,
            message,
        )
    }

    /// Create a WASI verification failed error
    #[must_use]
    pub const fn wasi_verification_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::WASI_VERIFICATION_FAILED,
            message,
        )
    }

    /// Create a WASI timeout error
    #[must_use]
    pub const fn wasi_timeout(message: &'static str) -> Self {
        Self::new(ErrorCategory::Core, codes::WASI_TIMEOUT, message)
    }

    /// Create a runtime capacity error
    #[must_use]
    pub const fn runtime_capacity_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_CAPACITY_ERROR_CODE,
            message,
        )
    }

    /// Create a validation invalid argument error
    #[must_use]
    pub const fn validation_invalid_argument(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_ARGUMENT, message)
    }

    /// Create a validation invalid type error
    #[must_use]
    pub const fn validation_invalid_type(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_TYPE, message)
    }

    /// Create a validation type mismatch error
    #[must_use]
    pub const fn validation_type_mismatch(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::TYPE_MISMATCH, message)
    }

    /// Create a parse invalid binary error
    #[must_use]
    pub const fn parse_invalid_binary(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parse, codes::INVALID_BINARY, message)
    }

    /// Create a runtime component limit exceeded error
    #[must_use]
    pub const fn runtime_component_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::COMPONENT_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create a runtime debug info error
    #[must_use]
    pub const fn runtime_debug_info_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::DEBUG_INFO_ERROR, message)
    }

    /// Create a validation invalid input error
    #[must_use]
    pub const fn validation_invalid_input(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_INPUT, message)
    }

    /// Create a security access denied error
    #[must_use]
    pub const fn security_access_denied(message: &'static str) -> Self {
        Self::new(ErrorCategory::Security, codes::ACCESS_DENIED, message)
    }

    /// Create a security runtime error
    #[must_use]
    pub const fn security_runtime_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Security, codes::RUNTIME_ERROR, message)
    }

    /// Create a security stack overflow error
    #[must_use]
    pub const fn security_stack_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Security, codes::STACK_OVERFLOW, message)
    }

    /// Create a core stack underflow error
    #[must_use]
    pub const fn core_stack_underflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Core, codes::STACK_UNDERFLOW, message)
    }

    /// Create a core invalid memory access error
    #[must_use]
    pub const fn core_invalid_memory_access(message: &'static str) -> Self {
        Self::new(ErrorCategory::Core, codes::INVALID_MEMORY_ACCESS, message)
    }

    /// Create a parameter invalid parameter error
    #[must_use]
    pub const fn parameter_invalid_parameter(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parameter, codes::INVALID_PARAMETER, message)
    }

    /// Create a parameter validation error
    #[must_use]
    pub const fn parameter_validation_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parameter, codes::VALIDATION_ERROR, message)
    }

    /// Create a parameter WASI invalid fd error
    #[must_use]
    pub const fn parameter_wasi_invalid_fd(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parameter, codes::WASI_INVALID_FD, message)
    }

    /// Create a system I/O error
    #[must_use]
    pub const fn system_io_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::SYSTEM_IO_ERROR_CODE,
            message,
        )
    }

    /// Create a resource limit exceeded error
    #[must_use]
    pub const fn resource_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::RESOURCE_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create a not supported would block error
    #[must_use]
    pub const fn not_supported_would_block(message: &'static str) -> Self {
        Self::new(ErrorCategory::NotSupported, codes::WOULD_BLOCK, message)
    }

    /// Create a platform error
    #[must_use]
    pub const fn platform_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Platform, codes::PLATFORM_ERROR, message)
    }

    /// Create a runtime unaligned memory access error
    #[must_use]
    pub const fn runtime_unaligned_access(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::UNALIGNED_MEMORY_ACCESS,
            message,
        )
    }

    /// Create an initialization error
    #[must_use]
    pub const fn initialization_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Initialization,
            codes::INITIALIZATION_ERROR,
            message,
        )
    }

    /// Create an initialization not implemented error
    #[must_use]
    pub const fn initialization_not_implemented(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Initialization,
            codes::NOT_IMPLEMENTED,
            message,
        )
    }

    /// Create an invalid state error
    #[must_use]
    pub const fn invalid_state_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::InvalidState, codes::INVALID_STATE, message)
    }

    /// Create a not implemented error
    #[must_use]
    pub const fn not_implemented_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::NotImplemented,
            codes::NOT_IMPLEMENTED,
            message,
        )
    }

    /// Create a not supported unsupported operation error
    #[must_use]
    pub const fn not_supported_unsupported_operation(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::NotSupported,
            codes::UNSUPPORTED_OPERATION,
            message,
        )
    }

    /// Create a runtime trap execution error
    #[must_use]
    pub const fn runtime_trap_execution_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::RuntimeTrap, codes::EXECUTION_ERROR, message)
    }

    /// Create a validation function not found error
    #[must_use]
    pub const fn validation_function_not_found(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::FUNCTION_NOT_FOUND,
            message,
        )
    }

    /// Create a validation invalid state error
    #[must_use]
    pub const fn validation_invalid_state(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_STATE, message)
    }

    /// Create a validation invalid parameter error
    #[must_use]
    pub const fn validation_invalid_parameter(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_PARAMETER, message)
    }

    /// Create a validation parse error
    #[must_use]
    pub const fn validation_parse_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::PARSE_ERROR, message)
    }

    /// Create a resource global not found error
    #[must_use]
    pub const fn resource_global_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::GLOBAL_NOT_FOUND, message)
    }

    /// Create a resource table not found error
    #[must_use]
    pub const fn resource_table_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::TABLE_NOT_FOUND, message)
    }

    /// Create a memory integer overflow error
    #[must_use]
    pub const fn memory_integer_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::INTEGER_OVERFLOW, message)
    }

    /// Create a memory access out of bounds error
    #[must_use]
    pub const fn memory_access_out_of_bounds(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_ACCESS_OUT_OF_BOUNDS,
            message,
        )
    }

    /// Create a memory serialization error
    #[must_use]
    pub const fn memory_serialization_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::SERIALIZATION_ERROR, message)
    }

    /// Create a system error
    #[must_use]
    pub const fn system_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::System, codes::SYSTEM_ERROR, message)
    }

    /// Create a core error
    #[must_use]
    pub const fn core_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Core, codes::EXECUTION_ERROR, message)
    }

    /// Create a parse error
    #[must_use]
    pub const fn parse_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parse, codes::PARSE_ERROR, message)
    }

    /// Create an invalid type error
    #[must_use]
    pub const fn invalid_type_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Type, codes::INVALID_TYPE, message)
    }

    /// Create an index out of bounds error
    #[must_use]
    pub const fn index_out_of_bounds(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::OUT_OF_BOUNDS_ERROR, message)
    }

    /// Create a deserialization error
    #[must_use]
    pub const fn deserialization_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parse, codes::DECODING_ERROR, message)
    }

    /// Create a capacity error
    #[must_use]
    pub const fn capacity_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Capacity, codes::CAPACITY_EXCEEDED, message)
    }

    /// Create an internal error
    #[must_use]
    pub const fn internal_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::System, codes::SYSTEM_ERROR, message)
    }

    /// Create a memory out of bounds error
    #[must_use]
    pub const fn memory_out_of_bounds(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::MEMORY_OUT_OF_BOUNDS, message)
    }

    /// Create a memory uninitialized error
    #[must_use]
    pub const fn memory_uninitialized(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::INITIALIZATION_ERROR, message)
    }

    /// Create a new static error with explicit parameters
    #[must_use]
    pub const fn new_static(category: ErrorCategory, code: u16, message: &'static str) -> Self {
        Self::new(category, code, message)
    }

    /// Create a validation unsupported feature error
    #[must_use]
    pub const fn validation_unsupported_feature(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::VALIDATION_UNSUPPORTED_FEATURE,
            message,
        )
    }

    // High-frequency patterns from analysis

    /// Create a resource capacity exceeded error
    #[must_use]
    pub const fn resource_capacity_exceeded(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::CAPACITY_EXCEEDED, message)
    }

    /// Create a platform error with code 1
    #[must_use]
    pub const fn platform_error_1(message: &'static str) -> Self {
        Self::new(ErrorCategory::Platform, 1, message)
    }

    /// Create a system error with code 1  
    #[must_use]
    pub const fn system_error_1(message: &'static str) -> Self {
        Self::new(ErrorCategory::System, 1, message)
    }

    /// Create an invalid operation error
    #[must_use]
    pub const fn invalid_operation(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::INVALID_OPERATION, message)
    }

    /// Create a capacity limit exceeded error
    #[must_use]
    pub const fn capacity_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Capacity,
            codes::CAPACITY_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create an invalid function index error
    #[must_use]
    pub const fn invalid_function_index(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::INVALID_FUNCTION_INDEX,
            message,
        )
    }

    /// Create an out of bounds error
    #[must_use]
    pub const fn out_of_bounds(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::OUT_OF_BOUNDS_ERROR, message)
    }

    /// Create a global not found error
    #[must_use]
    pub const fn global_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::GLOBAL_NOT_FOUND, message)
    }

    /// Create a function not found error
    #[must_use]
    pub const fn function_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::FUNCTION_NOT_FOUND, message)
    }

    /// Create an unimplemented error
    #[must_use]
    pub const fn unimplemented(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::NotImplemented,
            codes::NOT_IMPLEMENTED,
            message,
        )
    }

    /// Create an invalid value error
    #[must_use]
    pub const fn invalid_value(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_VALUE, message)
    }

    // Agent C Component Model error factory methods

    /// Create a WIT input too large error
    #[must_use]
    pub const fn wit_input_too_large(message: &'static str) -> Self {
        Self::new(ErrorCategory::Parse, codes::WIT_INPUT_TOO_LARGE, message)
    }

    /// Create a WIT world limit exceeded error
    #[must_use]
    pub const fn wit_world_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Parse,
            codes::WIT_WORLD_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create a WIT interface limit exceeded error
    #[must_use]
    pub const fn wit_interface_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Parse,
            codes::WIT_INTERFACE_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create a capability violation error
    #[must_use]
    pub const fn capability_violation(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Security,
            crate::codes::ACCESS_DENIED,
            message,
        )
    }

    /// Create a no capability error
    #[must_use]
    pub const fn no_capability(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Security,
            crate::codes::ACCESS_DENIED,
            message,
        )
    }

    /// Create an invalid state error
    #[must_use]
    pub const fn invalid_state(message: &'static str) -> Self {
        Self::new(ErrorCategory::InvalidState, codes::INVALID_STATE, message)
    }

    /// Create a timeout error
    #[must_use]
    pub const fn timeout_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::EXECUTION_TIMEOUT, message)
    }

    // NOTE: parse_error and runtime_stack_underflow functions already defined above

    /// Create a CFI violation error
    #[must_use]
    pub const fn cfi_violation(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::CFI_VIOLATION, message)
    }

    // NOTE: resource_not_found function already defined above

    /// Create an instance not found error
    #[must_use]
    pub const fn instance_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::INSTANCE_NOT_FOUND, message)
    }

    /// Create an execution limit exceeded error
    #[must_use]
    pub const fn execution_limit_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::EXECUTION_LIMIT_EXCEEDED,
            message,
        )
    }

    /// Create an execution timeout error
    #[must_use]
    pub const fn execution_timeout(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::EXECUTION_TIMEOUT, message)
    }

    /// Create a type mismatch error
    #[must_use]
    pub const fn type_mismatch_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Type, codes::TYPE_MISMATCH_ERROR, message)
    }

    /// Create a threading error
    #[must_use]
    pub const fn threading_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RUNTIME_ERROR, message)
    }

    /// Create a resource access error
    #[must_use]
    pub const fn resource_access_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RESOURCE_ACCESS_ERROR,
            message,
        )
    }

    /// Create a cleanup failed error
    #[must_use]
    pub const fn cleanup_failed(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RUNTIME_ERROR, message)
    }

    /// Create a function call failed error
    #[must_use]
    pub const fn function_call_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::VALIDATION_FUNCTION_NOT_FOUND,
            message,
        )
    }

    /// Create a type conversion error
    #[must_use]
    pub const fn type_conversion_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Type, codes::CONVERSION_ERROR, message)
    }

    /// Create a component not found error
    #[must_use]
    pub const fn component_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RESOURCE_NOT_FOUND, message)
    }

    /// Create an async error
    #[must_use]
    pub const fn async_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::AsyncRuntime, codes::ASYNC_ERROR, message)
    }

    /// Create a serialization error
    #[must_use]
    pub const fn serialization_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::SERIALIZATION_ERROR, message)
    }

    /// Create a configuration error
    #[must_use]
    pub const fn configuration_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::System, codes::CONFIGURATION_ERROR, message)
    }

    /// Create an operation cancelled error
    #[must_use]
    pub const fn operation_cancelled(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::OPERATION_CANCELLED, message)
    }

    /// Create a no WIT definitions found error
    #[must_use]
    pub const fn no_wit_definitions_found(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Parse,
            codes::NO_WIT_DEFINITIONS_FOUND,
            message,
        )
    }

    /// Create an insufficient memory error
    #[must_use]
    pub const fn insufficient_memory(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::INSUFFICIENT_MEMORY, message)
    }

    /// Create an out of memory error
    #[must_use]
    pub const fn out_of_memory(message: &'static str) -> Self {
        Self::new(ErrorCategory::Resource, codes::OUT_OF_MEMORY, message)
    }

    /// Create a too many components error
    #[must_use]
    pub const fn too_many_components(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::TOO_MANY_COMPONENTS,
            message,
        )
    }

    /// Create a component error with context
    #[must_use]
    pub const fn component_error_context(message: &'static str) -> Self {
        Self::new(ErrorCategory::Component, codes::COMPONENT_ERROR, message)
    }

    // Component Runtime Error Factory Methods

    /// Create a component thread spawn failed error
    #[must_use]
    pub const fn component_thread_spawn_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::ComponentRuntime,
            codes::COMPONENT_THREAD_SPAWN_FAILED,
            message,
        )
    }

    /// Create a component handle representation error
    #[must_use]
    pub const fn component_handle_representation_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::ComponentRuntime,
            codes::COMPONENT_HANDLE_REPRESENTATION_ERROR,
            message,
        )
    }

    /// Create a component resource lifecycle error
    #[must_use]
    pub const fn component_resource_lifecycle_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::ComponentRuntime,
            codes::COMPONENT_RESOURCE_LIFECYCLE_ERROR,
            message,
        )
    }

    /// Create a component thread join failed error
    #[must_use]
    pub const fn component_thread_join_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::ComponentRuntime,
            codes::COMPONENT_THREAD_JOIN_FAILED,
            message,
        )
    }

    /// Create a component capability denied error
    #[must_use]
    pub const fn component_capability_denied(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::ComponentRuntime,
            codes::COMPONENT_CAPABILITY_DENIED,
            message,
        )
    }

    // Platform Runtime Error Factory Methods

    /// Create a platform memory allocation failed error
    #[must_use]
    pub const fn platform_memory_allocation_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::PlatformRuntime,
            codes::PLATFORM_MEMORY_ALLOCATION_FAILED,
            message,
        )
    }

    /// Create a platform thread creation failed error
    #[must_use]
    pub const fn platform_thread_creation_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::PlatformRuntime,
            codes::PLATFORM_THREAD_CREATION_FAILED,
            message,
        )
    }

    /// Create a platform sync primitive failed error
    #[must_use]
    pub const fn platform_sync_primitive_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::PlatformRuntime,
            codes::PLATFORM_SYNC_PRIMITIVE_FAILED,
            message,
        )
    }

    /// Create a platform realtime constraint violated error
    #[must_use]
    pub const fn platform_realtime_constraint_violated(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::PlatformRuntime,
            codes::PLATFORM_REALTIME_CONSTRAINT_VIOLATED,
            message,
        )
    }

    // Foundation Runtime Error Factory Methods

    /// Create a foundation bounded capacity exceeded error
    #[must_use]
    pub const fn foundation_bounded_capacity_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::FoundationRuntime,
            codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED,
            message,
        )
    }

    /// Create a foundation memory provider failed error
    #[must_use]
    pub const fn foundation_memory_provider_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::FoundationRuntime,
            codes::FOUNDATION_MEMORY_PROVIDER_FAILED,
            message,
        )
    }

    /// Create a foundation safety constraint violated error
    #[must_use]
    pub const fn foundation_safety_constraint_violated(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::FoundationRuntime,
            codes::FOUNDATION_SAFETY_CONSTRAINT_VIOLATED,
            message,
        )
    }

    /// Create a foundation verification failed error
    #[must_use]
    pub const fn foundation_verification_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::FoundationRuntime,
            codes::FOUNDATION_VERIFICATION_FAILED,
            message,
        )
    }

    // Safety Error Factory Methods

    /// Create a safety violation error
    #[must_use]
    pub const fn safety_violation(message: &'static str) -> Self {
        Self::new(ErrorCategory::Safety, codes::SAFETY_VIOLATION, message)
    }

    /// Create a safety ASIL violation error
    #[must_use]
    pub const fn safety_asil_violation(message: &'static str) -> Self {
        Self::new(ErrorCategory::Safety, codes::SAFETY_ASIL_VIOLATION, message)
    }

    /// Create a verification failed error
    #[must_use]
    pub const fn verification_failed(message: &'static str) -> Self {
        Self::new(ErrorCategory::Safety, codes::VERIFICATION_FAILED, message)
    }

    /// Create a CFI validation failed error
    #[must_use]
    pub const fn cfi_validation_failed(message: &'static str) -> Self {
        Self::new(ErrorCategory::Safety, codes::CFI_VALIDATION_FAILED, message)
    }

    /// Create a determinism violation error
    #[must_use]
    pub const fn determinism_violation(message: &'static str) -> Self {
        Self::new(ErrorCategory::Safety, codes::DETERMINISM_VIOLATION, message)
    }

    /// Create a redundancy check failure error
    #[must_use]
    pub const fn redundancy_check_failure(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Safety,
            codes::REDUNDANCY_CHECK_FAILURE,
            message,
        )
    }

    /// Create a memory corruption detected error for Safety category
    #[must_use]
    pub const fn memory_corruption_detected(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Safety,
            codes::MEMORY_CORRUPTION_DETECTED,
            message,
        )
    }

    // Async Runtime Error Factory Methods

    /// Create an async task spawn failed error
    #[must_use]
    pub const fn async_task_spawn_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::AsyncRuntime,
            codes::ASYNC_TASK_SPAWN_FAILED,
            message,
        )
    }

    /// Create an async fuel exhausted error
    #[must_use]
    pub const fn async_fuel_exhausted(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::AsyncRuntime,
            codes::ASYNC_FUEL_EXHAUSTED,
            message,
        )
    }

    /// Create an async deadline exceeded error
    #[must_use]
    pub const fn async_deadline_exceeded(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::AsyncRuntime,
            codes::ASYNC_DEADLINE_EXCEEDED,
            message,
        )
    }

    /// Create an async channel full error
    #[must_use]
    pub const fn async_channel_full(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::AsyncRuntime,
            codes::ASYNC_CHANNEL_FULL,
            message,
        )
    }

    /// Create an async channel closed error
    #[must_use]
    pub const fn async_channel_closed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::AsyncRuntime,
            codes::ASYNC_CHANNEL_CLOSED,
            message,
        )
    }

    /// Create an async executor state violation error
    #[must_use]
    pub const fn async_executor_state_violation(message: &'static str) -> Self {
        Self::new(ErrorCategory::AsyncRuntime, codes::ASYNC_ERROR, message)
    }

    /// Create a capacity exceeded error
    #[must_use]
    pub const fn capacity_exceeded(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::CAPACITY_EXCEEDED, message)
    }

    // Additional factory methods for remaining patterns

    /// Create a component already exists error
    #[must_use]
    pub const fn component_already_exists(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::DUPLICATE_OPERATION,
            message,
        )
    }

    /// Create a component linking error error
    #[must_use]
    pub const fn component_linking_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::COMPONENT_LINKING_ERROR,
            message,
        )
    }

    /// Create a runtime trap error error
    #[must_use]
    pub const fn runtime_trap_error(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::RuntimeTrap,
            codes::RUNTIME_TRAP_ERROR,
            message,
        )
    }

    /// Create a trap integer overflow error
    #[must_use]
    pub const fn trap_integer_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::RuntimeTrap, codes::INTEGER_OVERFLOW, message)
    }

    /// Create a trap divide by zero error
    #[must_use]
    pub const fn trap_divide_by_zero(message: &'static str) -> Self {
        Self::new(ErrorCategory::RuntimeTrap, codes::DIVISION_BY_ZERO, message)
    }

    /// Create a platform memory error error
    #[must_use]
    pub const fn platform_memory_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Platform, codes::MEMORY_ERROR, message)
    }

    /// Create a platform thread error error
    #[must_use]
    pub const fn platform_thread_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Platform, codes::THREADING_ERROR, message)
    }

    /// Create a access denied error
    #[must_use]
    pub const fn access_denied(message: &'static str) -> Self {
        Self::new(ErrorCategory::Security, codes::ACCESS_DENIED, message)
    }

    /// Create a init failed error
    #[must_use]
    pub const fn init_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Initialization,
            codes::INITIALIZATION_ERROR,
            message,
        )
    }

    /// Create a not supported error
    #[must_use]
    pub const fn not_supported(message: &'static str) -> Self {
        Self::new(ErrorCategory::NotSupported, codes::UNSUPPORTED, message)
    }

    /// Create a feature not supported error
    #[must_use]
    pub const fn feature_not_supported(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::NotSupported,
            codes::VALIDATION_UNSUPPORTED_FEATURE,
            message,
        )
    }

    /// Create a operation failed error
    #[must_use]
    pub const fn operation_failed(message: &'static str) -> Self {
        Self::new(ErrorCategory::Runtime, codes::RUNTIME_ERROR, message)
    }

    /// Create a validation failed error
    #[must_use]
    pub const fn validation_failed(message: &'static str) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::VALIDATION_FAILURE,
            message,
        )
    }

    /// Create a invalid argument error
    #[must_use]
    pub const fn invalid_argument(message: &'static str) -> Self {
        Self::new(ErrorCategory::Validation, codes::INVALID_ARGUMENT, message)
    }

    /// Create a conversion error error
    #[must_use]
    pub const fn conversion_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Type, codes::CONVERSION_ERROR, message)
    }

    /// Create a buffer overflow error
    #[must_use]
    pub const fn buffer_overflow(message: &'static str) -> Self {
        Self::new(ErrorCategory::Memory, codes::BUFFER_TOO_SMALL, message)
    }

    /// Create a io error error
    #[must_use]
    pub const fn io_error(message: &'static str) -> Self {
        Self::new(ErrorCategory::Io, codes::IO_ERROR, message)
    }

    /// Create a file not found error
    #[must_use]
    pub const fn file_not_found(message: &'static str) -> Self {
        Self::new(ErrorCategory::Io, codes::RESOURCE_NOT_FOUND, message)
    }

    // Note: Methods like `with_message`, `new_legacy`, `*_with_code`,
    // and `parse_error_from_kind` have been removed as they were
    // Binary std/no_std choice
    // They can be re-added if versions compatible with `&'static str` messages are
    // designed.
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ASIL-C and above: Include ASIL level in error display
        #[cfg(any(feature = "asil-c", feature = "asil-d"))]
        {
            write!(
                f,
                "[{:?}][E{:04X}][{}] {}",
                self.category,
                self.code,
                self.asil_level(),
                self.message
            )
        }
        #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
        {
            write!(
                f,
                "[{:?}][E{:04X}] {}",
                self.category, self.code, self.message
            )
        }
    }
}

impl ErrorSource for Error {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &'static str {
        self.message
    }

    fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Self {
        Self::new(
            ErrorCategory::System,
            codes::SYSTEM_ERROR,
            "Formatting error (static)",
        )
    }
}

// Conversion helpers for error kinds

// -- From<kinds::X> for Error implementations --
impl From<kinds::ValidationError> for Error {
    fn from(_e: kinds::ValidationError) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::VALIDATION_ERROR,
            "Validation error from kind",
        )
    }
}

impl From<kinds::ParseError> for Error {
    fn from(_e: kinds::ParseError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::PARSE_ERROR,
            "Parse error from kind",
        )
    }
}

impl From<kinds::OutOfBoundsError> for Error {
    fn from(_e: kinds::OutOfBoundsError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_OUT_OF_BOUNDS,
            "Out of bounds error from kind",
        )
    }
}

impl From<kinds::InvalidType> for Error {
    fn from(_e: kinds::InvalidType) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::TYPE_MISMATCH_ERROR,
            "Invalid type error from kind",
        )
    }
}

impl From<kinds::ResourceError> for Error {
    fn from(_e: kinds::ResourceError) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::RESOURCE_ERROR,
            "Resource error from kind",
        )
    }
}

impl From<kinds::ComponentError> for Error {
    fn from(_e: kinds::ComponentError) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::COMPONENT_LINKING_ERROR,
            "Component error from kind",
        )
    }
}

impl From<kinds::RuntimeError> for Error {
    fn from(_e: kinds::RuntimeError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_ERROR,
            "Runtime error from kind",
        )
    }
}

impl From<kinds::PoisonedLockError> for Error {
    fn from(_e: kinds::PoisonedLockError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::POISONED_LOCK,
            "Poisoned lock error from kind",
        )
    }
}

impl From<kinds::TypeMismatchError> for Error {
    fn from(_e: kinds::TypeMismatchError) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::TYPE_MISMATCH_ERROR,
            "Type mismatch error from kind",
        )
    }
}

impl From<kinds::MemoryAccessOutOfBoundsError> for Error {
    fn from(_e: kinds::MemoryAccessOutOfBoundsError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_ACCESS_OUT_OF_BOUNDS,
            "Memory access out of bounds error from kind",
        )
    }
}

impl From<kinds::TableAccessOutOfBounds> for Error {
    fn from(_e: kinds::TableAccessOutOfBounds) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::OUT_OF_BOUNDS_ERROR,
            "Table access out of bounds error from kind",
        )
    }
}

impl From<kinds::ConversionError> for Error {
    fn from(_e: kinds::ConversionError) -> Self {
        Self::new(
            ErrorCategory::System,
            codes::CONVERSION_ERROR,
            "Conversion error from kind",
        )
    }
}

impl From<kinds::DivisionByZeroError> for Error {
    fn from(_e: kinds::DivisionByZeroError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_ERROR,
            "Division by zero error from kind",
        )
    }
}

impl From<kinds::IntegerOverflowError> for Error {
    fn from(_e: kinds::IntegerOverflowError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_ERROR,
            "Integer overflow error from kind",
        )
    }
}

impl From<kinds::StackUnderflow> for Error {
    fn from(_e: kinds::StackUnderflow) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::STACK_UNDERFLOW,
            "Stack underflow error from kind",
        )
    }
}

impl From<kinds::TypeMismatch> for Error {
    fn from(_e: kinds::TypeMismatch) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::TYPE_MISMATCH_ERROR,
            "Type mismatch from kind",
        )
    }
}

impl From<kinds::InvalidTableIndexError> for Error {
    fn from(_e: kinds::InvalidTableIndexError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::OUT_OF_BOUNDS_ERROR,
            "Invalid table index error from kind",
        )
    }
}

// Implement the ToErrorCategory trait for Error
impl ToErrorCategory for Error {
    fn to_category(&self) -> ErrorCategory {
        self.category
    }
}

// Implement FromError for Error (self conversion)
impl FromError<Self> for Error {
    fn from_error(error: Self) -> Self {
        error
    }
}

// -- FromError<kinds::X> for Error implementations --
impl FromError<kinds::ValidationError> for Error {
    fn from_error(_error: kinds::ValidationError) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::VALIDATION_ERROR,
            "Validation error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ParseError> for Error {
    fn from_error(_error: kinds::ParseError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::PARSE_ERROR,
            "Parse error from kind (FromError)",
        )
    }
}

impl FromError<kinds::OutOfBoundsError> for Error {
    fn from_error(_error: kinds::OutOfBoundsError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_OUT_OF_BOUNDS,
            "Out of bounds error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidType> for Error {
    fn from_error(_error: kinds::InvalidType) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::TYPE_MISMATCH_ERROR,
            "Invalid type error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ResourceError> for Error {
    fn from_error(_error: kinds::ResourceError) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::RESOURCE_ERROR,
            "Resource error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ComponentError> for Error {
    fn from_error(_error: kinds::ComponentError) -> Self {
        Self::new(
            ErrorCategory::Component,
            codes::COMPONENT_LINKING_ERROR,
            "Component error from kind (FromError)",
        )
    }
}

impl FromError<kinds::RuntimeError> for Error {
    fn from_error(_error: kinds::RuntimeError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RUNTIME_ERROR,
            "Runtime error from kind (FromError)",
        )
    }
}

impl FromError<kinds::PoisonedLockError> for Error {
    fn from_error(_error: kinds::PoisonedLockError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::POISONED_LOCK,
            "Poisoned lock error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ArithmeticError> for Error {
    fn from_error(_error: kinds::ArithmeticError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::EXECUTION_ERROR,
            "Arithmetic error from kind (FromError)",
        )
    }
}

impl FromError<kinds::MemoryAccessError> for Error {
    fn from_error(_error: kinds::MemoryAccessError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_ACCESS_ERROR,
            "Memory access error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ResourceExhaustionError> for Error {
    fn from_error(_error: kinds::ResourceExhaustionError) -> Self {
        Self::new(
            ErrorCategory::Resource,
            codes::RESOURCE_LIMIT_EXCEEDED,
            "Resource exhaustion error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidIndexError> for Error {
    fn from_error(_error: kinds::InvalidIndexError) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::OUT_OF_BOUNDS_ERROR,
            "Invalid index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ExecutionError> for Error {
    fn from_error(_error: kinds::ExecutionError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::EXECUTION_ERROR,
            "Execution error from kind (FromError)",
        )
    }
}

impl FromError<kinds::StackUnderflowError> for Error {
    fn from_error(_error: kinds::StackUnderflowError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::STACK_UNDERFLOW,
            "Stack underflow error from kind (FromError)",
        )
    }
}

impl FromError<kinds::ExportNotFoundError> for Error {
    fn from_error(_error: kinds::ExportNotFoundError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::RESOURCE_NOT_FOUND,
            "Export not found error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidInstanceIndexError> for Error {
    fn from_error(_error: kinds::InvalidInstanceIndexError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::INVALID_INSTANCE_INDEX,
            "Invalid instance index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidFunctionIndexError> for Error {
    fn from_error(_error: kinds::InvalidFunctionIndexError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::INVALID_FUNCTION_INDEX,
            "Invalid function index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidElementIndexError> for Error {
    fn from_error(_error: kinds::InvalidElementIndexError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::OUT_OF_BOUNDS_ERROR,
            "Invalid element index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidMemoryIndexError> for Error {
    fn from_error(_error: kinds::InvalidMemoryIndexError) -> Self {
        Self::new(
            ErrorCategory::Memory,
            codes::MEMORY_OUT_OF_BOUNDS,
            "Invalid memory index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidGlobalIndexError> for Error {
    fn from_error(_error: kinds::InvalidGlobalIndexError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::OUT_OF_BOUNDS_ERROR,
            "Invalid global index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidDataSegmentIndexError> for Error {
    fn from_error(_error: kinds::InvalidDataSegmentIndexError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::OUT_OF_BOUNDS_ERROR,
            "Invalid data segment index error from kind (FromError)",
        )
    }
}

impl FromError<kinds::InvalidFunctionTypeError> for Error {
    fn from_error(_e: kinds::InvalidFunctionTypeError) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::INVALID_FUNCTION_TYPE,
            "Invalid function type error from kind (FromError)",
        )
    }
}

// --- START Wasm 2.0 From Impls ---
impl From<kinds::UnsupportedWasm20Feature> for Error {
    fn from(_e: kinds::UnsupportedWasm20Feature) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::UNSUPPORTED_WASM20_FEATURE_ERROR,
            "Unsupported Wasm 2.0 feature",
        )
    }
}

impl From<kinds::InvalidReferenceTypeUsage> for Error {
    fn from(_e: kinds::InvalidReferenceTypeUsage) -> Self {
        Self::new(
            ErrorCategory::Type,
            codes::INVALID_REFERENCE_TYPE_USAGE_ERROR,
            "Invalid reference type usage",
        )
    }
}

impl From<kinds::BulkOperationError> for Error {
    fn from(_e: kinds::BulkOperationError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::BULK_OPERATION_ERROR,
            "Bulk operation error",
        )
    }
}

impl From<kinds::SimdOperationError> for Error {
    fn from(_e: kinds::SimdOperationError) -> Self {
        Self::new(
            ErrorCategory::Runtime,
            codes::SIMD_OPERATION_ERROR,
            "SIMD operation error",
        )
    }
}

impl From<kinds::TailCallError> for Error {
    fn from(_e: kinds::TailCallError) -> Self {
        Self::new(
            ErrorCategory::Validation,
            codes::TAIL_CALL_ERROR,
            "Tail call error",
        )
    }
}
// --- END Wasm 2.0 From Impls ---

#[cfg(feature = "std")]
impl std::error::Error for Error {}
