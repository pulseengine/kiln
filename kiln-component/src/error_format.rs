//! Error formatting utilities for no_std compatibility
//!
//! This module provides alternatives to the format! macro for error messages
//! in no_std environments.

#[cfg(not(feature = "std"))]
extern crate alloc;

use wrt_error::{Error, ErrorCategory};

use crate::prelude::*;

/// Error context for canonical ABI operations
#[derive(Debug, Clone, Copy)]
pub enum CanonicalErrorContext {
    OutOfBounds { addr: u32, size: usize },
    InvalidUtf8,
    InvalidCodePoint { code_point: u32 },
    InvalidDiscriminant { discriminant: u32 },
    NotImplemented(&'static str),
    TypeMismatch,
    ResourceNotFound { handle: u32 },
    InvalidAlignment { addr: u32, align: u32 },
    InvalidSize { expected: usize, actual: usize },
}

/// Format an error message for the given context
#[cfg(feature = "std")]
pub fn format_error(category: ErrorCategory, code: u32, context: CanonicalErrorContext) -> Error {

    let message = match context {
        CanonicalErrorContext::OutOfBounds { addr, size } => {
            format!("Memory access out of bounds at address {:#x}, size {}", addr, size)
        }
        CanonicalErrorContext::InvalidUtf8 => "Invalid UTF-8 string",
        CanonicalErrorContext::InvalidCodePoint { code_point } => {
            format!("Invalid Unicode code point: {:#x}", code_point)
        }
        CanonicalErrorContext::InvalidDiscriminant { discriminant } => {
            format!("Invalid discriminant value: {}", discriminant)
        }
        CanonicalErrorContext::NotImplemented(feature) => {
            format!("Feature not implemented: {}", feature)
        }
        CanonicalErrorContext::TypeMismatch => "Type mismatch",
        CanonicalErrorContext::ResourceNotFound { handle } => {
            format!("Resource not found with handle: {}", handle)
        }
        CanonicalErrorContext::InvalidAlignment { addr, align } => {
            format!("Invalid alignment: address {:#x} not aligned to {}", addr, align)
        }
        CanonicalErrorContext::InvalidSize { expected, actual } => {
            format!("Invalid size: expected {}, got {}", expected, actual)
        }
    };

    Error::runtime_execution_error("Error occurred")
}

/// Format an error message for the given context (no_std version with static messages)
#[cfg(not(feature = "std"))]
pub fn format_error(category: ErrorCategory, code: u32, context: CanonicalErrorContext) -> Error {
    let message = match context {
        CanonicalErrorContext::OutOfBounds { .. } => "Memory access out of bounds",
        CanonicalErrorContext::InvalidUtf8 => "Invalid UTF-8 string",
        CanonicalErrorContext::InvalidCodePoint { .. } => "Invalid Unicode code point",
        CanonicalErrorContext::InvalidDiscriminant { .. } => "Invalid discriminant value",
        CanonicalErrorContext::NotImplemented(_) => "Feature not implemented",
        CanonicalErrorContext::TypeMismatch => "Type mismatch",
        CanonicalErrorContext::ResourceNotFound { .. } => "Resource not found",
        CanonicalErrorContext::InvalidAlignment { .. } => "Invalid alignment",
        CanonicalErrorContext::InvalidSize { .. } => "Invalid size",
    };

    Error::runtime_execution_error("Error occurred")
}


/// Component error context
#[derive(Debug, Clone, Copy)]
pub enum ComponentErrorContext {
    ImportNotFound(&'static str),
    ExportNotFound(&'static str),
    InvalidComponentType,
    LinkingFailed,
    InstantiationFailed,
    ResourceLimitExceeded,
}

/// Format a component error
#[cfg(feature = "std")]
pub fn format_component_error(
    category: ErrorCategory,
    code: u32,
    context: ComponentErrorContext,
) -> Error {

    let message = match context {
        ComponentErrorContext::ImportNotFound(name) => {
            format!("Import not found: {}", name)
        }
        ComponentErrorContext::ExportNotFound(name) => {
            format!("Export not found: {}", name)
        }
        ComponentErrorContext::InvalidComponentType => "Invalid component type",
        ComponentErrorContext::LinkingFailed => "Component linking failed",
        ComponentErrorContext::InstantiationFailed => "Component instantiation failed",
        ComponentErrorContext::ResourceLimitExceeded => "Resource limit exceeded",
    };

    Error::runtime_execution_error("Error occurred")
}

/// Format a component error (no_std version)
#[cfg(not(feature = "std"))]
pub fn format_component_error(
    category: ErrorCategory,
    code: u32,
    context: ComponentErrorContext,
) -> Error {
    let message = match context {
        ComponentErrorContext::ImportNotFound(name) => name,
        ComponentErrorContext::ExportNotFound(name) => name,
        ComponentErrorContext::InvalidComponentType => "Invalid component type",
        ComponentErrorContext::LinkingFailed => "Component linking failed",
        ComponentErrorContext::InstantiationFailed => "Component instantiation failed",
        ComponentErrorContext::ResourceLimitExceeded => "Resource limit exceeded",
    };

    Error::runtime_execution_error("Error occurred")
}

/// Helper macro to create errors with context
#[macro_export]
macro_rules! canonical_error {
    ($category:expr, $code:expr, $context:expr) => {
        $crate::error_format::format_error($category, $code, $context)
    };
}

/// Helper macro to create component errors
#[macro_export]
macro_rules! component_error {
    ($category:expr, $code:expr, $context:expr) => {
        $crate::error_format::format_component_error($category, $code, $context)
    };

}
