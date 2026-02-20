//! WIT type conversions and mappings
//!
//! This module provides conversions between WASI-NN WIT types and internal
//! Rust types, ensuring type safety across the FFI boundary.

use super::{
    ExecutionTarget,
    GraphEncoding,
    TensorDimensions,
    TensorType,
};
use crate::prelude::*;

/// Error codes from WIT interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    /// Invalid argument provided
    InvalidArgument      = 1,
    /// Invalid model encoding
    InvalidEncoding      = 2,
    /// Runtime error during execution
    RuntimeError         = 3,
    /// Resource limits exceeded
    ResourceExhausted    = 4,
    /// Operation not supported
    UnsupportedOperation = 5,
    /// Model verification failed
    VerificationFailed   = 6,
    /// Timeout during execution
    Timeout              = 7,
}

impl From<Error> for ErrorCode {
    fn from(err: Error) -> Self {
        use wrt_error::codes;
        match err.code {
            codes::INVALID_ARGUMENT | codes::WASI_INVALID_ARGUMENT => ErrorCode::InvalidArgument,
            codes::UNSUPPORTED => ErrorCode::UnsupportedOperation,
            codes::RESOURCE_LIMIT_EXCEEDED
            | codes::WASI_RESOURCE_EXHAUSTED
            | codes::WASI_RESOURCE_LIMIT => ErrorCode::ResourceExhausted,
            // codes::TIMEOUT => ErrorCode::Timeout, // No timeout code exists yet
            codes::VERIFICATION_FAILED => ErrorCode::VerificationFailed,
            _ => ErrorCode::RuntimeError,
        }
    }
}

impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::InvalidArgument => Error::wasi_invalid_argument("Invalid argument"),
            ErrorCode::InvalidEncoding => Error::wasi_invalid_argument("Invalid encoding"),
            ErrorCode::RuntimeError => Error::wasi_runtime_error("Runtime error"),
            ErrorCode::ResourceExhausted => Error::wasi_resource_exhausted("Resource exhausted"),
            ErrorCode::UnsupportedOperation => {
                Error::wasi_unsupported_operation("Unsupported operation")
            },
            ErrorCode::VerificationFailed => Error::wasi_verification_failed("Verification failed"),
            ErrorCode::Timeout => Error::wasi_timeout("Operation timeout"),
        }
    }
}

/// Trait for converting between WIT types and internal types
pub trait WitTypeConversion: Sized {
    /// WIT representation type
    type WitType;

    /// Convert from WIT type
    fn from_wit(wit: Self::WitType) -> Result<Self>;

    /// Convert to WIT type
    fn to_wit(&self) -> Self::WitType;
}

// Implement conversions for tensor types
impl WitTypeConversion for TensorType {
    type WitType = u8;

    fn from_wit(wit: u8) -> Result<Self> {
        match wit {
            0 => Ok(TensorType::F16),
            1 => Ok(TensorType::F32),
            2 => Ok(TensorType::F64),
            3 => Ok(TensorType::U8),
            4 => Ok(TensorType::I8),
            5 => Ok(TensorType::U16),
            6 => Ok(TensorType::I16),
            7 => Ok(TensorType::U32),
            8 => Ok(TensorType::I32),
            9 => Ok(TensorType::U64),
            10 => Ok(TensorType::I64),
            11 => Ok(TensorType::Bool),
            _ => Err(Error::wasi_invalid_argument("Invalid tensor type")),
        }
    }

    fn to_wit(&self) -> u8 {
        match self {
            TensorType::F16 => 0,
            TensorType::F32 => 1,
            TensorType::F64 => 2,
            TensorType::U8 => 3,
            TensorType::I8 => 4,
            TensorType::U16 => 5,
            TensorType::I16 => 6,
            TensorType::U32 => 7,
            TensorType::I32 => 8,
            TensorType::U64 => 9,
            TensorType::I64 => 10,
            TensorType::Bool => 11,
        }
    }
}

// Implement conversions for graph encoding
impl WitTypeConversion for GraphEncoding {
    type WitType = u8;

    fn from_wit(wit: u8) -> Result<Self> {
        match wit {
            0 => Ok(GraphEncoding::ONNX),
            1 => Ok(GraphEncoding::TensorFlow),
            2 => Ok(GraphEncoding::PyTorch),
            3 => Ok(GraphEncoding::OpenVINO),
            4 => Ok(GraphEncoding::TractNative),
            _ => Err(Error::wasi_invalid_encoding("Invalid graph encoding")),
        }
    }

    fn to_wit(&self) -> u8 {
        match self {
            GraphEncoding::ONNX => 0,
            GraphEncoding::TensorFlow => 1,
            GraphEncoding::PyTorch => 2,
            GraphEncoding::OpenVINO => 3,
            GraphEncoding::TractNative => 4,
        }
    }
}

// Implement conversions for execution target
impl WitTypeConversion for ExecutionTarget {
    type WitType = u8;

    fn from_wit(wit: u8) -> Result<Self> {
        match wit {
            0 => Ok(ExecutionTarget::CPU),
            1 => Ok(ExecutionTarget::GPU),
            2 => Ok(ExecutionTarget::TPU),
            3 => Ok(ExecutionTarget::NPU),
            _ => Err(Error::wasi_invalid_argument("Invalid execution target")),
        }
    }

    fn to_wit(&self) -> u8 {
        match self {
            ExecutionTarget::CPU => 0,
            ExecutionTarget::GPU => 1,
            ExecutionTarget::TPU => 2,
            ExecutionTarget::NPU => 3,
        }
    }
}

/// Convert a list of u32 dimensions from WIT
pub fn dimensions_from_wit(wit_dims: &[u32]) -> Result<TensorDimensions> {
    // Additional validation for WIT boundary
    if wit_dims.is_empty() {
        return Err(Error::wasi_invalid_argument(
            "Dimensions array cannot be empty at WIT boundary",
        ));
    }

    // Validate dimension count at WIT boundary
    if wit_dims.len() > 16 {
        // Conservative limit for WIT interface
        return Err(Error::wasi_invalid_argument(
            "Too many dimensions at WIT boundary",
        ));
    }

    // Additional validation for very large dimensions at WIT boundary
    for (idx, &dim) in wit_dims.iter().enumerate() {
        if dim > 1_000_000 {
            // Very conservative limit for WIT
            return Err(Error::wasi_invalid_argument(
                "Dimension too large at WIT boundary",
            ));
        }
    }

    TensorDimensions::new(wit_dims)
}

/// Convert dimensions to WIT representation
pub fn dimensions_to_wit(dims: &TensorDimensions) -> Vec<u32> {
    dims.as_slice().to_vec()
}

/// WIT result type helper
pub type WitResult<T> = core::result::Result<T, ErrorCode>;

/// Convert internal Result to WIT Result
pub fn to_wit_result<T>(result: Result<T>) -> WitResult<T> {
    result.map_err(|e| e.into())
}

/// Helper for converting tensor data between representations
pub struct TensorDataConverter;

impl TensorDataConverter {
    /// Convert raw bytes to typed tensor data
    ///
    /// For safety compliance, we return the raw bytes and require explicit
    /// type conversion by the caller using safe methods.
    pub fn bytes_to_typed<T: Copy>(bytes: &[u8]) -> Result<Vec<T>> {
        // Validate input
        if bytes.is_empty() {
            return Err(Error::wasi_invalid_argument(
                "Cannot convert empty byte array",
            ));
        }

        // Validate alignment and size
        let type_size = core::mem::size_of::<T>();
        if type_size == 0 {
            return Err(Error::wasi_invalid_argument(
                "Cannot convert to zero-sized type",
            ));
        }

        if bytes.len() % type_size != 0 {
            return Err(Error::wasi_invalid_argument(
                "Byte array length not aligned to target type size",
            ));
        }

        // For ASIL compliance, unsafe conversions are not allowed
        // Callers should use safe conversion methods appropriate for their data types
        Err(Error::wasi_unsupported_operation(
            "Direct type conversion not supported in safe mode. Use type-specific conversion \
             functions.",
        ))
    }

    /// Convert typed tensor data to bytes
    ///
    /// For safety compliance, we use safe conversion methods only.
    pub fn typed_to_bytes<T: Copy>(data: &[T]) -> Result<Vec<u8>> {
        // Validate input
        if data.is_empty() {
            return Err(Error::wasi_invalid_argument(
                "Cannot convert empty data array",
            ));
        }

        let type_size = core::mem::size_of::<T>();
        if type_size == 0 {
            return Err(Error::wasi_invalid_argument(
                "Cannot convert from zero-sized type",
            ));
        }

        // Check for reasonable size limits
        let total_bytes = data
            .len()
            .checked_mul(type_size)
            .ok_or_else(|| Error::wasi_resource_exhausted("Data too large for conversion"))?;

        if total_bytes > 100 * 1024 * 1024 {
            // 100MB limit
            return Err(Error::wasi_resource_exhausted(
                "Data size exceeds conversion limit",
            ));
        }

        // For ASIL compliance, unsafe conversions are not allowed
        // Return error and require callers to use safe conversion methods
        Err(Error::wasi_unsupported_operation(
            "Direct type conversion not supported in safe mode. Use type-specific conversion \
             functions.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_type_conversion() {
        let tensor_type = TensorType::F32;
        let wit_type = tensor_type.to_wit();
        let converted = TensorType::from_wit(wit_type).unwrap();
        assert_eq!(tensor_type, converted);
    }

    #[test]
    fn test_error_code_conversion() {
        let error = Error::wasi_invalid_argument("test");
        let code: ErrorCode = error.into();
        assert_eq!(code, ErrorCode::InvalidArgument);

        let error2: Error = code.into();
        assert_eq!(error2.category, ErrorCategory::Validation);
    }

    #[test]
    fn test_tensor_data_conversion() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0];

        // Test that conversion is properly rejected in safe mode
        let result = TensorDataConverter::typed_to_bytes(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("safe mode"));

        // Test bytes to typed also rejects unsafe conversion
        let bytes = vec![0u8; 16];
        let result: Result<Vec<f32>> = TensorDataConverter::bytes_to_typed(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("safe mode"));
    }
}
