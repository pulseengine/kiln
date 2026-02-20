//! Tensor representation and operations
//!
//! This module provides the core tensor abstraction used throughout WASI-NN.
//! Tensors are capability-aware and respect memory limits based on verification
//! level.

use core::fmt;

use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    BoundedVec,
};

use super::{
    NeuralNetworkCapability,
    VerificationLevel,
};
use crate::prelude::*;

/// Maximum number of tensor dimensions
pub const MAX_TENSOR_DIMS: usize = 8;

/// Tensor data types supported by WASI-NN
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TensorType {
    /// 16-bit floating point
    F16,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// Unsigned 8-bit integer
    U8,
    /// Signed 8-bit integer
    I8,
    /// Unsigned 16-bit integer
    U16,
    /// Signed 16-bit integer
    I16,
    /// Unsigned 32-bit integer
    U32,
    /// Signed 32-bit integer
    I32,
    /// Unsigned 64-bit integer
    U64,
    /// Signed 64-bit integer
    I64,
    /// Boolean (1 bit, stored as u8)
    Bool,
}

impl TensorType {
    /// Get the size in bytes of this tensor type
    pub fn size_bytes(&self) -> usize {
        match self {
            TensorType::F16 => 2,
            TensorType::F32 => 4,
            TensorType::F64 => 8,
            TensorType::U8 | TensorType::I8 | TensorType::Bool => 1,
            TensorType::U16 | TensorType::I16 => 2,
            TensorType::U32 | TensorType::I32 => 4,
            TensorType::U64 | TensorType::I64 => 8,
        }
    }

    /// Check if this is a floating point type
    pub fn is_float(&self) -> bool {
        matches!(self, TensorType::F16 | TensorType::F32 | TensorType::F64)
    }

    /// Check if this is an integer type
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            TensorType::U8
                | TensorType::I8
                | TensorType::U16
                | TensorType::I16
                | TensorType::U32
                | TensorType::I32
                | TensorType::U64
                | TensorType::I64
        )
    }
}

/// Tensor dimensions representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorDimensions {
    dims: Vec<u32>,
}

impl TensorDimensions {
    /// Create new tensor dimensions
    pub fn new(dims: &[u32]) -> Result<Self> {
        if dims.is_empty() {
            return Err(Error::wasi_invalid_argument(
                "Tensor must have at least one dimension",
            ));
        }
        if dims.len() > MAX_TENSOR_DIMS {
            return Err(Error::wasi_invalid_argument("Too many tensor dimensions"));
        }

        // Validate each dimension
        for &dim in dims {
            if dim == 0 {
                return Err(Error::wasi_invalid_argument(
                    "Tensor dimensions cannot be zero",
                ));
            }
            if dim > 100_000 {
                // Reasonable upper bound per dimension
                return Err(Error::wasi_invalid_argument("Tensor dimension too large"));
            }
        }

        // Validate total element count using checked arithmetic
        let _ = Self::calculate_elements_checked(dims)?;

        let dims = dims.to_vec();

        Ok(Self { dims })
    }

    /// Get the dimensions as a slice
    pub fn as_slice(&self) -> &[u32] {
        &self.dims
    }

    /// Get the number of dimensions
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Calculate total number of elements
    pub fn num_elements(&self) -> usize {
        // For backward compatibility, return saturated value
        // Use checked_num_elements() for error handling
        self.checked_num_elements().unwrap_or(usize::MAX)
    }

    /// Calculate total number of elements with overflow checking
    pub fn checked_num_elements(&self) -> Result<usize> {
        let mut result = 1usize;
        for &dim in &self.dims {
            let dim_usize = usize::try_from(dim)
                .map_err(|_| Error::wasi_invalid_argument("Dimension value too large"))?;
            result = result.checked_mul(dim_usize).ok_or_else(|| {
                Error::wasi_resource_exhausted("Tensor too large - dimension overflow")
            })?;
        }
        Ok(result)
    }

    /// Check if dimensions are valid (non-zero)
    pub fn is_valid(&self) -> bool {
        !self.dims.is_empty() && self.dims.iter().all(|&d| d > 0)
    }

    /// Helper function to calculate elements with overflow checking (static
    /// version)
    fn calculate_elements_checked(dims: &[u32]) -> Result<usize> {
        let mut result = 1usize;
        for &dim in dims {
            let dim_usize = usize::try_from(dim)
                .map_err(|_| Error::wasi_invalid_argument("Dimension value too large"))?;
            result = result.checked_mul(dim_usize).ok_or_else(|| {
                Error::wasi_resource_exhausted("Tensor too large - dimension overflow")
            })?;
        }
        Ok(result)
    }
}

/// Core tensor structure with capability-aware memory management
#[derive(Clone)]
pub struct Tensor {
    /// Tensor dimensions
    dimensions:       TensorDimensions,
    /// Data type
    data_type:        TensorType,
    /// Raw data storage
    data:             Vec<u8>,
    /// Capability level this tensor was created with
    capability_level: VerificationLevel,
}

impl Tensor {
    /// Create a new tensor with given dimensions and type
    pub fn new(
        dimensions: TensorDimensions,
        data_type: TensorType,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Self> {
        if !dimensions.is_valid() {
            return Err(Error::wasi_invalid_argument("Invalid tensor dimensions"));
        }

        // Calculate size with overflow checking
        let num_elements = dimensions.checked_num_elements()?;
        let size_bytes = num_elements
            .checked_mul(data_type.size_bytes())
            .ok_or_else(|| Error::wasi_resource_exhausted("Tensor size calculation overflow"))?;

        // Verify against capability limits
        let limits = capability.resource_limits();
        if size_bytes > limits.max_tensor_memory {
            return Err(Error::wasi_resource_exhausted(
                "Tensor size exceeds memory limit",
            ));
        }

        // Allocate data buffer
        let verification_level = capability.verification_level();
        let data = match verification_level {
            VerificationLevel::Standard => {
                // Dynamic allocation
                vec![0u8; size_bytes]
            },
            VerificationLevel::Sampling | VerificationLevel::Continuous => {
                // Bounded allocation with pre-checking
                let mut vec = Vec::new();
                vec.try_reserve_exact(size_bytes).map_err(|_| {
                    Error::wasi_resource_exhausted("Failed to allocate tensor memory")
                })?;
                vec.resize(size_bytes, 0);
                vec
            },
            _ => {
                return Err(Error::wasi_unsupported_operation(
                    "Higher verification levels not supported in wrtd",
                ));
            },
        };

        Ok(Self {
            dimensions,
            data_type,
            data,
            capability_level: verification_level,
        })
    }

    /// Create a tensor from existing data
    pub fn from_data(
        dimensions: TensorDimensions,
        data_type: TensorType,
        data: Vec<u8>,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Self> {
        if !dimensions.is_valid() {
            return Err(Error::wasi_invalid_argument("Invalid tensor dimensions"));
        }

        // Calculate expected size with overflow checking
        let num_elements = dimensions.checked_num_elements()?;
        let expected_size = num_elements
            .checked_mul(data_type.size_bytes())
            .ok_or_else(|| Error::wasi_resource_exhausted("Tensor size calculation overflow"))?;
        if data.len() != expected_size {
            return Err(Error::wasi_invalid_argument(
                "Data size doesn't match tensor dimensions",
            ));
        }

        // Verify against capability limits
        let limits = capability.resource_limits();
        if data.len() > limits.max_tensor_memory {
            return Err(Error::wasi_resource_exhausted(
                "Tensor data exceeds memory limit",
            ));
        }

        let verification_level = capability.verification_level();
        Ok(Self {
            dimensions,
            data_type,
            data,
            capability_level: verification_level,
        })
    }

    /// Get tensor dimensions
    pub fn dimensions(&self) -> &TensorDimensions {
        &self.dimensions
    }

    /// Get tensor data type
    pub fn data_type(&self) -> TensorType {
        self.data_type
    }

    /// Get raw data as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable raw data as bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Get the size in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Get the capability level this tensor was created with
    pub fn capability_level(&self) -> VerificationLevel {
        self.capability_level
    }

    /// Reshape the tensor (changes dimensions but not data)
    pub fn reshape(&mut self, new_dimensions: TensorDimensions) -> Result<()> {
        // Check if element counts match for reshape
        let new_elements = new_dimensions.checked_num_elements()?;
        let current_elements = self.dimensions.checked_num_elements()?;
        if new_elements != current_elements {
            return Err(Error::wasi_invalid_argument(
                "Reshape dimensions don't match element count",
            ));
        }

        self.dimensions = new_dimensions;
        Ok(())
    }

    /// Clone the tensor data into a new vector
    pub fn to_vec(&self) -> Vec<u8> {
        self.data.clone()
    }
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tensor")
            .field("dimensions", &self.dimensions)
            .field("data_type", &self.data_type)
            .field("size_bytes", &self.data.len())
            .field("capability_level", &self.capability_level)
            .finish()
    }
}

/// Builder for creating tensors with validation
pub struct TensorBuilder {
    dimensions: Option<TensorDimensions>,
    data_type:  Option<TensorType>,
    data:       Option<Vec<u8>>,
}

impl TensorBuilder {
    /// Create a new tensor builder
    pub fn new() -> Self {
        Self {
            dimensions: None,
            data_type:  None,
            data:       None,
        }
    }

    /// Set dimensions
    pub fn dimensions(mut self, dims: &[u32]) -> Result<Self> {
        self.dimensions = Some(TensorDimensions::new(dims)?);
        Ok(self)
    }

    /// Set data type
    pub fn data_type(mut self, dtype: TensorType) -> Self {
        self.data_type = Some(dtype);
        self
    }

    /// Set data
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = Some(data);
        self
    }

    /// Build the tensor
    pub fn build(self, capability: &dyn NeuralNetworkCapability) -> Result<Tensor> {
        let dimensions = self
            .dimensions
            .ok_or_else(|| Error::wasi_invalid_argument("Tensor dimensions not set"))?;
        let data_type = self
            .data_type
            .ok_or_else(|| Error::wasi_invalid_argument("Tensor data type not set"))?;

        match self.data {
            Some(data) => Tensor::from_data(dimensions, data_type, data, capability),
            None => Tensor::new(dimensions, data_type, capability),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nn::capabilities::DynamicNNCapability;

    #[test]
    fn test_tensor_type_sizes() {
        assert_eq!(TensorType::F32.size_bytes(), 4);
        assert_eq!(TensorType::U8.size_bytes(), 1);
        assert_eq!(TensorType::I64.size_bytes(), 8);
    }

    #[test]
    fn test_tensor_dimensions() {
        let dims = TensorDimensions::new(&[2, 3, 4]).unwrap();
        assert_eq!(dims.rank(), 3);
        assert_eq!(dims.num_elements(), 24);
        assert!(dims.is_valid());
    }

    #[test]
    fn test_tensor_creation() {
        let capability = DynamicNNCapability::new();
        let dims = TensorDimensions::new(&[10, 10]).unwrap();
        let tensor = Tensor::new(dims, TensorType::F32, &capability).unwrap();

        assert_eq!(tensor.size_bytes(), 400); // 10*10*4
        assert_eq!(tensor.capability_level(), VerificationLevel::Standard);
    }

    #[test]
    fn test_tensor_builder() {
        let capability = DynamicNNCapability::new();
        let tensor = TensorBuilder::new()
            .dimensions(&[5, 5])
            .unwrap()
            .data_type(TensorType::U8)
            .build(&capability)
            .unwrap();

        assert_eq!(tensor.dimensions().num_elements(), 25);
        assert_eq!(tensor.data_type(), TensorType::U8);
    }
}
