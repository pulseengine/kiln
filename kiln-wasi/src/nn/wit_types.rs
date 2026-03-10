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

/// Error codes from WASI-NN 0.2.0-rc-2024-10-28 WIT interface
///
/// These match the `error-code` enum in the spec:
///   invalid-argument, invalid-encoding, timeout, runtime-error,
///   unsupported-operation, too-large, not-found, security, unknown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    /// Invalid argument provided
    InvalidArgument      = 0,
    /// Invalid model encoding
    InvalidEncoding      = 1,
    /// Operation timed out
    Timeout              = 2,
    /// Runtime error during execution
    RuntimeError         = 3,
    /// Operation not supported by this backend
    UnsupportedOperation = 4,
    /// Input or model too large for available resources
    TooLarge             = 5,
    /// Requested resource not found (graph, context, etc.)
    NotFound             = 6,
    /// Security policy violation
    Security             = 7,
    /// Unknown or unclassified error
    Unknown              = 8,
}

impl From<Error> for ErrorCode {
    fn from(err: Error) -> Self {
        use kiln_error::codes;
        match err.code {
            codes::INVALID_ARGUMENT | codes::WASI_INVALID_ARGUMENT => ErrorCode::InvalidArgument,
            codes::UNSUPPORTED => ErrorCode::UnsupportedOperation,
            codes::RESOURCE_LIMIT_EXCEEDED
            | codes::WASI_RESOURCE_EXHAUSTED
            | codes::WASI_RESOURCE_LIMIT => ErrorCode::TooLarge,
            codes::VERIFICATION_FAILED => ErrorCode::Security,
            _ => ErrorCode::RuntimeError,
        }
    }
}

impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::InvalidArgument => Error::wasi_invalid_argument("Invalid argument"),
            ErrorCode::InvalidEncoding => Error::wasi_invalid_argument("Invalid encoding"),
            ErrorCode::Timeout => Error::wasi_timeout("Operation timeout"),
            ErrorCode::RuntimeError => Error::wasi_runtime_error("Runtime error"),
            ErrorCode::UnsupportedOperation => {
                Error::wasi_unsupported_operation("Unsupported operation")
            },
            ErrorCode::TooLarge => Error::wasi_resource_exhausted("Input or model too large"),
            ErrorCode::NotFound => Error::wasi_invalid_argument("Resource not found"),
            ErrorCode::Security => Error::wasi_verification_failed("Security policy violation"),
            ErrorCode::Unknown => Error::wasi_runtime_error("Unknown error"),
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
// Matches WASI-NN 0.2.0-rc-2024-10-28 spec enum order:
//   FP16=0, FP32=1, FP64=2, BF16=3, U8=4, I32=5, I64=6
// Extended with additional types for Kiln backend support
impl WitTypeConversion for TensorType {
    type WitType = u8;

    fn from_wit(wit: u8) -> Result<Self> {
        match wit {
            0 => Ok(TensorType::F16),
            1 => Ok(TensorType::F32),
            2 => Ok(TensorType::F64),
            3 => Ok(TensorType::BF16),
            4 => Ok(TensorType::U8),
            5 => Ok(TensorType::I32),
            6 => Ok(TensorType::I64),
            // Extended types (Kiln extensions, not in base spec)
            128 => Ok(TensorType::I8),
            129 => Ok(TensorType::U16),
            130 => Ok(TensorType::I16),
            131 => Ok(TensorType::U32),
            132 => Ok(TensorType::U64),
            133 => Ok(TensorType::Bool),
            _ => Err(Error::wasi_invalid_argument("Invalid tensor type")),
        }
    }

    fn to_wit(&self) -> u8 {
        match self {
            TensorType::F16 => 0,
            TensorType::F32 => 1,
            TensorType::F64 => 2,
            TensorType::BF16 => 3,
            TensorType::U8 => 4,
            TensorType::I32 => 5,
            TensorType::I64 => 6,
            // Extended types
            TensorType::I8 => 128,
            TensorType::U16 => 129,
            TensorType::I16 => 130,
            TensorType::U32 => 131,
            TensorType::U64 => 132,
            TensorType::Bool => 133,
        }
    }
}

// Implement conversions for graph encoding
// Matches WASI-NN 0.2.0-rc-2024-10-28 spec enum order:
//   openvino=0, onnx=1, tensorflow=2, pytorch=3, tensorflowlite=4, ggml=5, autodetect=6
impl WitTypeConversion for GraphEncoding {
    type WitType = u8;

    fn from_wit(wit: u8) -> Result<Self> {
        match wit {
            0 => Ok(GraphEncoding::OpenVINO),
            1 => Ok(GraphEncoding::ONNX),
            2 => Ok(GraphEncoding::TensorFlow),
            3 => Ok(GraphEncoding::PyTorch),
            4 => Ok(GraphEncoding::TensorFlowLite),
            5 => Ok(GraphEncoding::GGML),
            6 => Ok(GraphEncoding::Autodetect),
            255 => Ok(GraphEncoding::TractNative),
            _ => Err(Error::wasi_invalid_encoding("Invalid graph encoding")),
        }
    }

    fn to_wit(&self) -> u8 {
        match self {
            GraphEncoding::OpenVINO => 0,
            GraphEncoding::ONNX => 1,
            GraphEncoding::TensorFlow => 2,
            GraphEncoding::PyTorch => 3,
            GraphEncoding::TensorFlowLite => 4,
            GraphEncoding::GGML => 5,
            GraphEncoding::Autodetect => 6,
            GraphEncoding::TractNative => 255,
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
    fn test_tensor_type_roundtrip() {
        // Test all spec types roundtrip correctly
        let spec_types = [
            TensorType::F16,
            TensorType::F32,
            TensorType::F64,
            TensorType::BF16,
            TensorType::U8,
            TensorType::I32,
            TensorType::I64,
        ];
        for tt in &spec_types {
            let wit = tt.to_wit();
            let converted = TensorType::from_wit(wit).unwrap();
            assert_eq!(*tt, converted, "Roundtrip failed for {:?}", tt);
        }
    }

    #[test]
    fn test_graph_encoding_roundtrip() {
        // Test all spec encodings roundtrip correctly
        let encodings = [
            GraphEncoding::OpenVINO,
            GraphEncoding::ONNX,
            GraphEncoding::TensorFlow,
            GraphEncoding::PyTorch,
            GraphEncoding::TensorFlowLite,
            GraphEncoding::GGML,
            GraphEncoding::Autodetect,
            GraphEncoding::TractNative,
        ];
        for enc in &encodings {
            let wit = enc.to_wit();
            let converted = GraphEncoding::from_wit(wit).unwrap();
            assert_eq!(*enc, converted, "Roundtrip failed for {:?}", enc);
        }
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
    fn test_error_code_spec_values() {
        // Verify error code discriminant values match the WASI-NN spec
        assert_eq!(ErrorCode::InvalidArgument as u8, 0);
        assert_eq!(ErrorCode::InvalidEncoding as u8, 1);
        assert_eq!(ErrorCode::Timeout as u8, 2);
        assert_eq!(ErrorCode::RuntimeError as u8, 3);
        assert_eq!(ErrorCode::UnsupportedOperation as u8, 4);
        assert_eq!(ErrorCode::TooLarge as u8, 5);
        assert_eq!(ErrorCode::NotFound as u8, 6);
        assert_eq!(ErrorCode::Security as u8, 7);
        assert_eq!(ErrorCode::Unknown as u8, 8);
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
