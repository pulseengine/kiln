//! Neural Network Backend Abstraction
//!
//! This module defines the traits that neural network backends must implement
//! to work with WASI-NN. It provides a capability-aware abstraction that allows
//! different backends (Tract, ONNX Runtime, etc.) to be used interchangeably.

use core::fmt::Debug;
use std::sync::{
    Mutex,
    OnceLock,
};

use super::{
    GraphEncoding,
    NeuralNetworkCapability,
    Tensor,
    TensorDimensions,
    TensorType,
};
use crate::prelude::*;

/// Trait for model capabilities tied to specific backends
pub trait ModelCapability: Send + Sync + Debug {
    /// Get the model's unique identifier
    fn id(&self) -> u32;

    /// Get the model's size in bytes
    fn size(&self) -> usize;

    /// Get the model's hash for verification
    fn hash(&self) -> [u8; 32];

    /// Get input tensor metadata
    fn input_metadata(&self, index: usize) -> Result<(TensorDimensions, TensorType)>;

    /// Get output tensor metadata
    fn output_metadata(&self, index: usize) -> Result<(TensorDimensions, TensorType)>;

    /// Get number of inputs
    fn num_inputs(&self) -> usize;

    /// Get number of outputs
    fn num_outputs(&self) -> usize;

    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn core::any::Any;
}

/// Trait for tensor capabilities tied to specific backends
pub trait TensorCapability: Send + Sync + Debug {
    /// Get tensor dimensions
    fn dimensions(&self) -> &TensorDimensions;

    /// Get tensor data type
    fn data_type(&self) -> TensorType;

    /// Get size in bytes
    fn size_bytes(&self) -> usize;

    /// Read tensor data into a buffer
    fn read_data(&self, buffer: &mut [u8]) -> Result<()>;

    /// Write tensor data from a buffer
    fn write_data(&mut self, buffer: &[u8]) -> Result<()>;

    /// Check if tensor is contiguous in memory
    fn is_contiguous(&self) -> bool;
}

/// Core trait that all neural network backends must implement
pub trait NeuralNetworkBackend: Send + Sync + Debug {
    /// The concrete model type for this backend
    type Model: ModelCapability;

    /// The concrete tensor type for this backend
    type Tensor: TensorCapability;

    /// The concrete execution context type
    type Context: Send + Sync + Debug;

    /// Load a model from bytes
    fn load_model(&self, data: &[u8], encoding: GraphEncoding) -> Result<Self::Model>;

    /// Create an execution context for a model
    fn create_context(&self, model: &Self::Model) -> Result<Self::Context>;

    /// Create a tensor with given dimensions and type
    fn create_tensor(
        &self,
        dimensions: TensorDimensions,
        data_type: TensorType,
    ) -> Result<Self::Tensor>;

    /// Set input tensor for execution
    fn set_input(
        &self,
        context: &mut Self::Context,
        index: usize,
        tensor: &Self::Tensor,
    ) -> Result<()>;

    /// Execute inference
    fn compute(&self, context: &mut Self::Context) -> Result<()>;

    /// Get output tensor from execution
    fn get_output(&self, context: &Self::Context, index: usize) -> Result<Self::Tensor>;

    /// Check if backend supports a specific encoding
    fn supports_encoding(&self, encoding: GraphEncoding) -> bool;

    /// Get backend name for diagnostics
    fn name(&self) -> &'static str;

    /// Estimate FLOPS for a model (for resource planning)
    fn estimate_flops(&self, model: &Self::Model) -> Option<u64> {
        None // Default: unknown
    }
}

/// Backend registry for managing available backends
pub struct BackendRegistry {
    backends: Vec<(GraphEncoding, Box<dyn BackendProvider>)>,
}

/// Trait for backend providers that can create backend instances
pub trait BackendProvider: Send + Sync {
    /// Create a backend instance with the given capability
    fn create_backend(
        &self,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Box<dyn DynBackend>>;

    /// Check if this provider supports the given encoding
    fn supports_encoding(&self, encoding: GraphEncoding) -> bool;
}

/// Type-erased backend for the registry
pub trait DynBackend: Send + Sync + Debug {
    /// Load a model
    fn load_model_dyn(
        &self,
        data: &[u8],
        encoding: GraphEncoding,
    ) -> Result<Box<dyn ModelCapability>>;

    /// Create execution context
    fn create_context_dyn(&self, model: &dyn ModelCapability) -> Result<Box<dyn ComputeCapable>>;

    /// Execute inference with type-erased tensors
    fn compute_dyn(
        &self,
        context: &mut dyn ComputeCapable,
        inputs: &[Tensor],
        model: &dyn ModelCapability,
    ) -> Result<Vec<Tensor>>;

    /// Get backend name
    fn name(&self) -> &'static str;
}

/// Trait for execution contexts that can perform compute
pub trait ComputeCapable: Debug + Send + Sync {
    /// Execute inference with the given inputs
    fn compute(&mut self, inputs: &[Tensor], model: &dyn ModelCapability) -> Result<Vec<Tensor>>;

    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn core::any::Any;

    /// Allow mutable downcasting to concrete types
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any;
}

impl BackendRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Register a backend provider
    pub fn register(
        &mut self,
        encoding: GraphEncoding,
        provider: Box<dyn BackendProvider>,
    ) -> Result<()> {
        // Validate that provider actually supports the claimed encoding
        if !provider.supports_encoding(encoding) {
            return Err(Error::wasi_invalid_argument(
                "Backend provider does not support the specified encoding",
            ));
        }

        // Check for duplicate registrations
        for (existing_encoding, _) in &self.backends {
            if *existing_encoding == encoding {
                return Err(Error::wasi_capability_unavailable(
                    "Backend for this encoding already registered",
                ));
            }
        }

        self.backends.push((encoding, provider));
        Ok(())
    }

    /// Get a backend for the given encoding and capability
    pub fn get_backend(
        &self,
        encoding: GraphEncoding,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Box<dyn DynBackend>> {
        // Validate capability before creating backend
        let limits = capability.resource_limits();
        if limits.max_tensor_memory == 0
            || limits.max_concurrent_models == 0
            || limits.max_concurrent_contexts == 0
        {
            return Err(Error::wasi_invalid_argument(
                "Capability has invalid resource limits",
            ));
        }

        for (enc, provider) in &self.backends {
            if *enc == encoding && provider.supports_encoding(encoding) {
                let backend = provider.create_backend(capability)?;

                // Validate that the created backend works correctly
                if backend.name().is_empty() {
                    return Err(Error::wasi_runtime_error("Backend has invalid name"));
                }

                return Ok(backend);
            }
        }
        Err(Error::wasi_unsupported_operation(
            "No backend available for encoding",
        ))
    }
}

/// Global backend registry instance
static BACKEND_REGISTRY: OnceLock<Mutex<BackendRegistry>> = OnceLock::new();

/// Initialize the backend registry (call once at startup)
pub fn initialize_backends() -> Result<()> {
    let mut registry = BackendRegistry::new();

    // Register available backends
    #[cfg(feature = "tract")]
    {
        use crate::nn::tract_backend::TractBackendProvider;
        registry.register(GraphEncoding::ONNX, Box::new(TractBackendProvider::new()))?;
        registry.register(
            GraphEncoding::TractNative,
            Box::new(TractBackendProvider::new()),
        )?;
    }

    let mutex = Mutex::new(registry);
    BACKEND_REGISTRY
        .set(mutex)
        .map_err(|_| Error::wasi_capability_unavailable("Backend registry already initialized"))
}

/// Get the global backend registry
pub fn get_backend_registry() -> Result<std::sync::MutexGuard<'static, BackendRegistry>> {
    let mutex = BACKEND_REGISTRY
        .get()
        .ok_or_else(|| Error::wasi_capability_unavailable("Backend registry not initialized"))?;

    mutex
        .lock()
        .map_err(|_| Error::wasi_runtime_error("Backend registry mutex poisoned"))
}

/// Get the global backend registry for mutable operations
pub fn get_backend_registry_mut() -> Result<std::sync::MutexGuard<'static, BackendRegistry>> {
    get_backend_registry()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_registry() {
        let mut registry = BackendRegistry::new();
        assert!(registry.backends.is_empty());
    }
}
