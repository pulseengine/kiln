//! Error formatting utilities for `no_std` compatibility

// Error formatting may truncate large values for display - acceptable behavior.
#![allow(clippy::cast_possible_truncation)]

use wrt_error::{
    Error,
    ErrorCategory,
};

/// Error context for instruction operations
#[derive(Debug, Clone, Copy)]
pub enum InstructionErrorContext {
    /// Type mismatch in operation
    TypeMismatch {
        /// Expected type
        expected: &'static str,
        /// Actual type found
        actual:   &'static str,
    },
    /// Stack underflow
    StackUnderflow {
        /// Required stack items
        required:  usize,
        /// Available stack items
        available: usize,
    },
    /// Invalid memory access
    InvalidMemoryAccess {
        /// Memory offset
        offset: u32,
        /// Access size
        size:   u32,
    },
    /// Division by zero
    DivisionByZero,
    /// Integer overflow
    IntegerOverflow,
    /// Invalid conversion
    InvalidConversion {
        /// Source type
        from: &'static str,
        /// Target type
        to:   &'static str,
    },
    /// Table out of bounds
    TableOutOfBounds {
        /// Table index
        index: u32,
        /// Table size
        size:  u32,
    },
    /// Invalid reference
    InvalidReference,
    /// Function not found
    FunctionNotFound {
        /// Function index
        index: u32,
    },
    /// Invalid branch target
    InvalidBranchTarget {
        /// Branch depth
        depth: u32,
    },
}

/// Binary `std/no_std` choice
#[cfg(feature = "std")]
#[must_use] 
pub fn format_error(category: ErrorCategory, code: u32, context: InstructionErrorContext) -> Error {
    use std::format;

    let _message = match context {
        InstructionErrorContext::TypeMismatch { expected, actual } => {
            format!("Expected {expected}, got {actual}")
        },
        InstructionErrorContext::StackUnderflow {
            required,
            available,
        } => {
            format!(
                "Stack underflow: required {required}, available {available}"
            )
        },
        InstructionErrorContext::InvalidMemoryAccess { offset, size } => {
            format!(
                "Invalid memory access at offset {offset} with size {size}"
            )
        },
        InstructionErrorContext::DivisionByZero => "Division by zero".into(),
        InstructionErrorContext::IntegerOverflow => "Integer overflow".into(),
        InstructionErrorContext::InvalidConversion { from, to } => {
            format!("Invalid conversion from {from} to {to}")
        },
        InstructionErrorContext::TableOutOfBounds { index, size } => {
            format!("Table index {index} out of bounds (size: {size})")
        },
        InstructionErrorContext::InvalidReference => "Invalid reference".into(),
        InstructionErrorContext::FunctionNotFound { index } => {
            format!("Function {index} not found")
        },
        InstructionErrorContext::InvalidBranchTarget { depth } => {
            format!("Invalid branch target depth: {depth}")
        },
    };

    // Use a static message since Error::new requires &'static str
    let static_message = match context {
        InstructionErrorContext::TypeMismatch { .. } => "Type mismatch",
        InstructionErrorContext::StackUnderflow { .. } => "Stack underflow",
        InstructionErrorContext::InvalidMemoryAccess { .. } => "Invalid memory access",
        InstructionErrorContext::DivisionByZero => "Division by zero",
        InstructionErrorContext::IntegerOverflow => "Integer overflow",
        InstructionErrorContext::InvalidConversion { .. } => "Invalid conversion",
        InstructionErrorContext::TableOutOfBounds { .. } => "Table index out of bounds",
        InstructionErrorContext::InvalidReference => "Invalid reference",
        InstructionErrorContext::FunctionNotFound { .. } => "Function not found",
        InstructionErrorContext::InvalidBranchTarget { .. } => "Invalid branch target depth",
    };
    Error::new(category, code as u16, static_message)
}

/// Binary `std/no_std` choice
#[cfg(not(feature = "std"))]
#[must_use]
pub fn format_error(category: ErrorCategory, code: u32, context: InstructionErrorContext) -> Error {
    let _message = match context {
        InstructionErrorContext::TypeMismatch { expected, .. } => expected,
        InstructionErrorContext::StackUnderflow { .. } => "Stack underflow",
        InstructionErrorContext::InvalidMemoryAccess { .. } => "Invalid memory access",
        InstructionErrorContext::DivisionByZero => "Division by zero",
        InstructionErrorContext::IntegerOverflow => "Integer overflow",
        InstructionErrorContext::InvalidConversion { from, .. } => from,
        InstructionErrorContext::TableOutOfBounds { .. } => "Table index out of bounds",
        InstructionErrorContext::InvalidReference => "Invalid reference",
        InstructionErrorContext::FunctionNotFound { .. } => "Function not found",
        InstructionErrorContext::InvalidBranchTarget { .. } => "Invalid branch target",
    };

    // Use a static message since Error::new requires &'static str
    let static_message = match context {
        InstructionErrorContext::TypeMismatch { .. } => "Type mismatch",
        InstructionErrorContext::StackUnderflow { .. } => "Stack underflow",
        InstructionErrorContext::InvalidMemoryAccess { .. } => "Invalid memory access",
        InstructionErrorContext::DivisionByZero => "Division by zero",
        InstructionErrorContext::IntegerOverflow => "Integer overflow",
        InstructionErrorContext::InvalidConversion { .. } => "Invalid conversion",
        InstructionErrorContext::TableOutOfBounds { .. } => "Table index out of bounds",
        InstructionErrorContext::InvalidReference => "Invalid reference",
        InstructionErrorContext::FunctionNotFound { .. } => "Function not found",
        InstructionErrorContext::InvalidBranchTarget { .. } => "Invalid branch target depth",
    };
    Error::new(category, code as u16, static_message)
}

/// Helper macro for creating instruction errors
#[macro_export]
macro_rules! instruction_error {
    ($category:expr, $code:expr, $context:expr) => {
        $crate::error_utils::format_error($category, $code, $context)
    };
}

/// Type name helper for error messages
pub fn type_name(value: &crate::prelude::Value) -> &'static str {
    match value {
        crate::prelude::Value::I32(_) => "I32",
        crate::prelude::Value::I64(_) => "I64",
        crate::prelude::Value::F32(_) => "F32",
        crate::prelude::Value::F64(_) => "F64",
        crate::prelude::Value::FuncRef(_) => "FuncRef",
        crate::prelude::Value::ExternRef(_) => "ExternRef",
        crate::prelude::Value::V128(_) => "V128",
        crate::prelude::Value::Ref(_) => "Ref",
        crate::prelude::Value::I16x8(_) => "I16x8",
        crate::prelude::Value::StructRef(_) => "StructRef",
        crate::prelude::Value::ArrayRef(_) => "ArrayRef",
        crate::prelude::Value::ExnRef(_) => "ExnRef",
        crate::prelude::Value::I31Ref(_) => "I31Ref",
        // Component Model types
        crate::prelude::Value::Bool(_) => "Bool",
        crate::prelude::Value::S8(_) => "S8",
        crate::prelude::Value::U8(_) => "U8",
        crate::prelude::Value::S16(_) => "S16",
        crate::prelude::Value::U16(_) => "U16",
        crate::prelude::Value::S32(_) => "S32",
        crate::prelude::Value::U32(_) => "U32",
        crate::prelude::Value::S64(_) => "S64",
        crate::prelude::Value::U64(_) => "U64",
        crate::prelude::Value::Char(_) => "Char",
        crate::prelude::Value::String(_) => "String",
        crate::prelude::Value::List(_) => "List",
        crate::prelude::Value::Tuple(_) => "Tuple",
        crate::prelude::Value::Record(_) => "Record",
        crate::prelude::Value::Variant(_, _) => "Variant",
        crate::prelude::Value::Enum(_) => "Enum",
        crate::prelude::Value::Option(_) => "Option",
        crate::prelude::Value::Result(_) => "Result",
        crate::prelude::Value::Flags(_) => "Flags",
        crate::prelude::Value::Own(_) => "Own",
        crate::prelude::Value::Borrow(_) => "Borrow",
        crate::prelude::Value::Void => "Void",
        crate::prelude::Value::Stream(_) => "Stream",
        crate::prelude::Value::Future(_) => "Future",
    }
}
