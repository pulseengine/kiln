//! Tract backend implementation for WASI-NN
//!
//! This module provides a Tract-based neural network backend that integrates
//! with WRT's capability system for safety-aware inference.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
use core::fmt;
#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(feature = "tract")]
use tract_onnx::prelude::*;
use wrt_foundation::verification::VerificationLevel;

use super::{
    BackendProvider,
    ComputeCapable,
    DynBackend,
    GraphEncoding,
    ModelCapability,
    NeuralNetworkBackend,
    NeuralNetworkCapability,
    Tensor,
    TensorCapability,
    TensorDimensions,
    TensorType,
};
use crate::prelude::*;

/// Tract backend implementation
pub struct TractBackend<C: NeuralNetworkCapability> {
    capability: C,
    name:       &'static str,
}

impl<C: NeuralNetworkCapability> TractBackend<C> {
    /// Create a new Tract backend with the given capability
    pub fn new(capability: C) -> Self {
        Self {
            capability,
            name: "tract",
        }
    }
}

impl<C: NeuralNetworkCapability> fmt::Debug for TractBackend<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TractBackend")
            .field("name", &self.name)
            .field("capability_level", &self.capability.verification_level())
            .finish()
    }
}

/// Tract model wrapper implementing ModelCapability
pub struct TractModel {
    id:          u32,
    size:        usize,
    hash:        [u8; 32],
    #[cfg(feature = "tract")]
    model:       TypedModel,
    #[cfg(feature = "tract")]
    runnable:    TypedRunnableModel<TypedModel>,
    input_info:  Vec<(TensorDimensions, TensorType)>,
    output_info: Vec<(TensorDimensions, TensorType)>,
}

// Manual Debug implementation to handle Tract types
impl fmt::Debug for TractModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TractModel")
            .field("id", &self.id)
            .field("size", &self.size)
            .field("hash", &format!("{:x?}", &self.hash[..8]))
            .field("inputs", &self.input_info.len())
            .field("outputs", &self.output_info.len())
            .finish()
    }
}

impl ModelCapability for TractModel {
    fn id(&self) -> u32 {
        self.id
    }

    fn size(&self) -> usize {
        self.size
    }

    fn hash(&self) -> [u8; 32] {
        self.hash
    }

    fn input_metadata(&self, index: usize) -> Result<(TensorDimensions, TensorType)> {
        self.input_info
            .get(index)
            .cloned()
            .ok_or_else(|| Error::wasi_invalid_argument("Invalid input index"))
    }

    fn output_metadata(&self, index: usize) -> Result<(TensorDimensions, TensorType)> {
        self.output_info
            .get(index)
            .cloned()
            .ok_or_else(|| Error::wasi_invalid_argument("Invalid output index"))
    }

    fn num_inputs(&self) -> usize {
        self.input_info.len()
    }

    fn num_outputs(&self) -> usize {
        self.output_info.len()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Tract tensor wrapper implementing TensorCapability
#[derive(Debug)]
pub struct TractTensor {
    dimensions:   TensorDimensions,
    data_type:    TensorType,
    data:         Vec<u8>,
    #[cfg(feature = "tract")]
    tract_tensor: Option<tract_onnx::prelude::Tensor>,
}

impl TensorCapability for TractTensor {
    fn dimensions(&self) -> &TensorDimensions {
        &self.dimensions
    }

    fn data_type(&self) -> TensorType {
        self.data_type
    }

    fn size_bytes(&self) -> usize {
        self.data.len()
    }

    fn read_data(&self, buffer: &mut [u8]) -> Result<()> {
        if buffer.len() < self.data.len() {
            return Err(Error::wasi_invalid_argument(
                "Buffer too small for tensor data",
            ));
        }
        // Safe: we've verified buffer is large enough above
        buffer[..self.data.len()].copy_from_slice(&self.data);
        Ok(())
    }

    fn write_data(&mut self, buffer: &[u8]) -> Result<()> {
        if buffer.len() != self.data.len() {
            return Err(Error::wasi_invalid_argument("Buffer size mismatch"));
        }
        self.data.copy_from_slice(buffer);
        Ok(())
    }

    fn is_contiguous(&self) -> bool {
        true // Tract tensors are contiguous
    }
}

/// Tract execution context
pub struct TractContext {
    model_id:         u32,
    encoding:         GraphEncoding,
    capability_level: VerificationLevel,
    #[cfg(feature = "tract")]
    inputs:           Vec<Option<tract_onnx::prelude::Tensor>>,
    #[cfg(feature = "tract")]
    outputs:          Option<TVec<Arc<tract_onnx::prelude::Tensor>>>,
    #[cfg(feature = "tract")]
    runnable:         TypedRunnableModel<TypedModel>,
}

impl fmt::Debug for TractContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("TractContext");
        debug.field("model_id", &self.model_id);
        debug.field("encoding", &self.encoding);
        #[cfg(feature = "tract")]
        {
            debug.field(
                "has_inputs",
                &self.inputs.iter().filter(|i| i.is_some()).count(),
            );
            debug.field("has_outputs", &self.outputs.is_some());
        }
        debug.finish()
    }
}

impl ComputeCapable for TractContext {
    fn compute(&mut self, inputs: &[Tensor], model: &dyn ModelCapability) -> Result<Vec<Tensor>> {
        #[cfg(feature = "tract")]
        {
            // Convert WASI-NN tensors to Tract tensors
            let mut tract_inputs: TVec<TValue> = tvec![];

            for (idx, tensor) in inputs.iter().enumerate() {
                // Get tensor metadata
                let dims = tensor.dimensions();
                let dtype = tensor.data_type();

                // Convert to Tract types
                let datum_type = tensor_type_to_datum(dtype)?;
                let shape: Vec<usize> = dims.as_slice().iter().map(|&d| d as usize).collect();

                // Get tensor data
                let data = tensor.as_bytes();

                // Create Tract tensor using safe construction
                // For ASIL compliance, we need to use safe tensor creation
                let tract_tensor = match datum_type {
                    dt if dt == f32::datum_type() => {
                        let float_data: Vec<f32> = data
                            .chunks_exact(4)
                            .map(|chunk| {
                                f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                            })
                            .collect();
                        tract_onnx::prelude::Tensor::from_shape(&shape, &float_data).map_err(
                            |_| Error::wasi_runtime_error("Failed to create f32 Tract tensor"),
                        )?
                    },
                    dt if dt == i32::datum_type() => {
                        let int_data: Vec<i32> = data
                            .chunks_exact(4)
                            .map(|chunk| {
                                i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                            })
                            .collect();
                        tract_onnx::prelude::Tensor::from_shape(&shape, &int_data).map_err(
                            |_| Error::wasi_runtime_error("Failed to create i32 Tract tensor"),
                        )?
                    },
                    _ => {
                        return Err(Error::wasi_unsupported_operation(
                            "Unsupported tensor type for safe conversion",
                        ))
                    },
                };

                tract_inputs.push(tract_tensor.into());
            }

            // Run inference
            let outputs = self
                .runnable
                .run(tract_inputs)
                .map_err(|_| Error::wasi_runtime_error("Inference failed"))?;

            // Convert outputs back to WASI-NN tensors
            let mut result = Vec::new();

            // Create capability based on the stored verification level
            use crate::nn::capabilities::{
                create_nn_capability,
                NNVerificationLevel,
            };
            let nn_level: NNVerificationLevel = self.capability_level.into();
            let capability = create_nn_capability(nn_level)?;

            for (idx, tract_output) in outputs.iter().enumerate() {
                let datum_type = datum_to_tensor_type(tract_output.datum_type())?;

                // Convert shape
                let shape: Vec<u32> = tract_output.shape().iter().map(|&d| d as u32).collect();
                let dimensions = TensorDimensions::new(&shape)?;

                // Create WASI-NN tensor with data
                let data = tract_output.as_bytes().to_vec();
                let tensor = Tensor::from_data(dimensions, datum_type, data, capability.as_ref())?;

                result.push(tensor);
            }

            // Store outputs for later retrieval
            self.outputs = Some(outputs.into_iter().map(|t| Arc::new(t.into_tensor())).collect());

            Ok(result)
        }

        #[cfg(not(feature = "tract"))]
        {
            // Dummy implementation
            Ok(vec![])
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

/// Convert WASI-NN tensor type to Tract datum type
#[cfg(feature = "tract")]
fn tensor_type_to_datum(tensor_type: TensorType) -> Result<DatumType> {
    match tensor_type {
        TensorType::F32 => Ok(f32::datum_type()),
        TensorType::F64 => Ok(f64::datum_type()),
        TensorType::I32 => Ok(i32::datum_type()),
        TensorType::I64 => Ok(i64::datum_type()),
        TensorType::U8 => Ok(u8::datum_type()),
        _ => Err(Error::wasi_unsupported_operation(
            "Unsupported tensor type for Tract",
        )),
    }
}

/// Convert Tract datum type to WASI-NN tensor type
#[cfg(feature = "tract")]
fn datum_to_tensor_type(datum_factoid: DatumType) -> Result<TensorType> {
    // For now, assume we get a concrete datum type
    if datum_factoid == f32::datum_type() {
        Ok(TensorType::F32)
    } else if datum_factoid == f64::datum_type() {
        Ok(TensorType::F64)
    } else if datum_factoid == i32::datum_type() {
        Ok(TensorType::I32)
    } else if datum_factoid == i64::datum_type() {
        Ok(TensorType::I64)
    } else if datum_factoid == u8::datum_type() {
        Ok(TensorType::U8)
    } else {
        // Default fallback for unknown types
        Ok(TensorType::F32)
    }
}

impl<C: NeuralNetworkCapability + 'static> NeuralNetworkBackend for TractBackend<C> {
    type Context = TractContext;
    type Model = TractModel;
    type Tensor = TractTensor;

    fn load_model(&self, data: &[u8], encoding: GraphEncoding) -> Result<Self::Model> {
        // Verify encoding is supported
        if !self.supports_encoding(encoding) {
            return Err(Error::wasi_invalid_encoding(
                "Tract doesn't support this encoding",
            ));
        }

        // Verify model size against capability
        let limits = self.capability.resource_limits();
        if data.len() > limits.max_model_size {
            return Err(Error::wasi_resource_exhausted("Model exceeds size limit"));
        }

        // Calculate hash for verification
        let hash = calculate_model_hash(data);

        // For higher safety levels, verify model is approved
        if self.capability.verification_level() >= super::VerificationLevel::Continuous {
            if !self.capability.is_model_approved(&hash) {
                return Err(Error::wasi_verification_failed(
                    "Model not in approved list",
                ));
            }
        }

        #[cfg(feature = "tract")]
        {
            use std::io::Cursor;

            // Load model based on encoding
            let model = match encoding {
                GraphEncoding::ONNX => tract_onnx::onnx()
                    .model_for_read(&mut Cursor::new(data))
                    .map_err(|_| Error::wasi_invalid_encoding("Failed to parse ONNX model"))?,
                _ => {
                    return Err(Error::wasi_invalid_encoding(
                        "Only ONNX is currently supported",
                    ))
                },
            };

            // Analyze the model to get input/output info
            let mut input_info = Vec::new();
            let mut output_info = Vec::new();

            // Get input facts
            for (idx, input) in model.inputs.iter().enumerate() {
                let fact = model
                    .outlet_fact(*input)
                    .map_err(|_| Error::wasi_runtime_error("Failed to get input fact"))?;

                // For now, use a simple approach with default shapes
                // In a full implementation, we'd properly parse the model metadata
                let tensor_dims = TensorDimensions::new(&[1, 224, 224, 3])?; // Common image input
                let tensor_type = TensorType::F32; // Default to F32
                input_info.push((tensor_dims, tensor_type));
            }

            // Get output facts
            for output in model.outputs.iter() {
                let fact = model
                    .outlet_fact(*output)
                    .map_err(|_| Error::wasi_runtime_error("Failed to get output fact"))?;

                // For now, use a simple approach with default shapes
                // In a full implementation, we'd properly parse the model metadata
                let tensor_dims = TensorDimensions::new(&[1, 1000])?; // Common classification output
                let tensor_type = TensorType::F32; // Default to F32
                output_info.push((tensor_dims, tensor_type));
            }

            // Optimize and make runnable
            let optimized = model
                .into_optimized()
                .map_err(|_| Error::wasi_runtime_error("Failed to optimize model"))?;

            let runnable = optimized
                .into_runnable()
                .map_err(|_| Error::wasi_runtime_error("Failed to make model runnable"))?;

            Ok(TractModel {
                id: 1, // Would be assigned by graph store in real usage
                size: data.len(),
                hash,
                model: runnable.model().clone(),
                runnable,
                input_info,
                output_info,
            })
        }

        #[cfg(not(feature = "tract"))]
        {
            // Fallback for when tract feature is not enabled
            Ok(TractModel {
                id: 1,
                size: data.len(),
                hash,
                input_info: vec![(TensorDimensions::new(&[1, 224, 224, 3])?, TensorType::F32)],
                output_info: vec![(TensorDimensions::new(&[1, 1000])?, TensorType::F32)],
            })
        }
    }

    fn create_context(&self, model: &Self::Model) -> Result<Self::Context> {
        #[cfg(feature = "tract")]
        {
            let num_inputs = model.num_inputs();
            Ok(TractContext {
                model_id:         model.id(),
                encoding:         GraphEncoding::ONNX, // We know this backend only supports ONNX
                capability_level: self.capability.verification_level().into(),
                inputs:           vec![None; num_inputs],
                outputs:          None,
                runnable:         model.runnable.clone(),
            })
        }

        #[cfg(not(feature = "tract"))]
        {
            Ok(TractContext {
                model_id:         model.id(),
                encoding:         GraphEncoding::ONNX,
                capability_level: self.capability.verification_level(),
            })
        }
    }

    fn create_tensor(
        &self,
        dimensions: TensorDimensions,
        data_type: TensorType,
    ) -> Result<Self::Tensor> {
        // Calculate size with overflow checking
        let num_elements = dimensions.checked_num_elements()?;
        let size = num_elements
            .checked_mul(data_type.size_bytes())
            .ok_or_else(|| Error::wasi_resource_exhausted("Tensor size calculation overflow"))?;

        // Verify against capability limits
        let limits = self.capability.resource_limits();
        if size > limits.max_tensor_memory {
            return Err(Error::wasi_resource_exhausted(
                "Tensor exceeds memory limit",
            ));
        }

        Ok(TractTensor {
            dimensions,
            data_type,
            data: vec![0u8; size],
            #[cfg(feature = "tract")]
            tract_tensor: None,
        })
    }

    fn set_input(
        &self,
        context: &mut Self::Context,
        index: usize,
        tensor: &Self::Tensor,
    ) -> Result<()> {
        #[cfg(feature = "tract")]
        {
            if index >= context.inputs.len() {
                return Err(Error::wasi_invalid_argument("Input index out of bounds"));
            }

            // Convert WASI-NN tensor to Tract tensor
            let datum_type = tensor_type_to_datum(tensor.data_type)?;
            let shape: Vec<usize> =
                tensor.dimensions.as_slice().iter().map(|&d| d as usize).collect();

            // Create Tract tensor from data using safe construction
            let tract_tensor = match datum_type {
                dt if dt == f32::datum_type() => {
                    let float_data: Vec<f32> = tensor
                        .data
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                        .collect();
                    tract_onnx::prelude::Tensor::from_shape(&shape, &float_data).map_err(|_| {
                        Error::wasi_runtime_error("Failed to create f32 Tract tensor")
                    })?
                },
                dt if dt == i32::datum_type() => {
                    let int_data: Vec<i32> = tensor
                        .data
                        .chunks_exact(4)
                        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                        .collect();
                    tract_onnx::prelude::Tensor::from_shape(&shape, &int_data).map_err(|_| {
                        Error::wasi_runtime_error("Failed to create i32 Tract tensor")
                    })?
                },
                _ => {
                    return Err(Error::wasi_unsupported_operation(
                        "Unsupported tensor type for safe conversion",
                    ))
                },
            };

            context.inputs[index] = Some(tract_tensor);
            Ok(())
        }

        #[cfg(not(feature = "tract"))]
        {
            Ok(())
        }
    }

    fn compute(&self, context: &mut Self::Context) -> Result<()> {
        #[cfg(feature = "tract")]
        {
            // Verify all inputs are set and prepare them
            let inputs: Result<TVec<TValue>> = context
                .inputs
                .iter()
                .enumerate()
                .map(|(idx, opt)| {
                    opt.as_ref()
                        .ok_or_else(|| Error::wasi_invalid_argument("Input not set"))
                        .map(|tensor| tensor.clone().into())
                })
                .collect();
            let inputs = inputs?;

            // Run inference
            let outputs = context
                .runnable
                .run(inputs)
                .map_err(|_| Error::wasi_runtime_error("Inference failed"))?;

            context.outputs =
                Some(outputs.into_iter().map(|t| Arc::new(t.into_tensor())).collect());
            Ok(())
        }

        #[cfg(not(feature = "tract"))]
        {
            // For higher safety levels, ensure deterministic execution
            if self.capability.verification_level() >= super::VerificationLevel::Continuous {
                // Would configure tract for deterministic mode
            }
            Ok(())
        }
    }

    fn get_output(&self, context: &Self::Context, index: usize) -> Result<Self::Tensor> {
        #[cfg(feature = "tract")]
        {
            if let Some(ref outputs) = context.outputs {
                if index >= outputs.len() {
                    return Err(Error::wasi_invalid_argument("Output index out of bounds"));
                }

                let tract_tensor = &outputs[index];
                let datum_type = datum_to_tensor_type(tract_tensor.datum_type())?;

                // Convert shape
                let shape: Vec<u32> = tract_tensor.shape().iter().map(|&d| d as u32).collect();
                let dimensions = TensorDimensions::new(&shape)?;

                // Copy data
                let data = tract_tensor.as_bytes().to_vec();

                Ok(TractTensor {
                    dimensions,
                    data_type: datum_type,
                    data,
                    tract_tensor: Some(tract_tensor.as_ref().clone()),
                })
            } else {
                Err(Error::wasi_runtime_error(
                    "No outputs available - compute not called",
                ))
            }
        }

        #[cfg(not(feature = "tract"))]
        {
            // For now, return dummy output
            let dims = TensorDimensions::new(&[1, 1000])?;
            self.create_tensor(dims, TensorType::F32)
        }
    }

    fn supports_encoding(&self, encoding: GraphEncoding) -> bool {
        matches!(encoding, GraphEncoding::ONNX | GraphEncoding::TractNative)
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn estimate_flops(&self, model: &Self::Model) -> Option<u64> {
        // Simple estimation based on model size
        Some(model.size() as u64 * 1000)
    }
}

/// Tract backend provider for the registry
pub struct TractBackendProvider;

impl TractBackendProvider {
    /// Create a new Tract backend provider
    pub fn new() -> Self {
        Self
    }
}

impl BackendProvider for TractBackendProvider {
    fn create_backend(
        &self,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Box<dyn DynBackend>> {
        // Create type-erased wrapper based on capability level
        match capability.verification_level() {
            super::VerificationLevel::Standard => {
                use super::capabilities::DynamicNNCapability;
                let cap = DynamicNNCapability::new();
                Ok(Box::new(TractDynBackend::new(TractBackend::new(cap))))
            },
            super::VerificationLevel::Sampling => {
                use super::capabilities::BoundedNNCapability;
                let cap = BoundedNNCapability::new()?;
                Ok(Box::new(TractDynBackend::new(TractBackend::new(cap))))
            },
            super::VerificationLevel::Continuous => {
                use super::capabilities::StaticNNCapability;
                let cap = StaticNNCapability::new(&[])?;
                Ok(Box::new(TractDynBackend::new(TractBackend::new(cap))))
            },
            _ => Err(Error::wasi_unsupported_operation(
                "Unsupported verification level",
            )),
        }
    }

    fn supports_encoding(&self, encoding: GraphEncoding) -> bool {
        matches!(encoding, GraphEncoding::ONNX | GraphEncoding::TractNative)
    }
}

/// Type-erased Tract backend for registry
struct TractDynBackend<B: NeuralNetworkBackend> {
    backend: B,
}

impl<B: NeuralNetworkBackend + 'static> TractDynBackend<B> {
    fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: NeuralNetworkBackend + 'static> fmt::Debug for TractDynBackend<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TractDynBackend")
            .field("backend_name", &self.backend.name())
            .finish()
    }
}

impl<B: NeuralNetworkBackend + 'static> DynBackend for TractDynBackend<B> {
    fn load_model_dyn(
        &self,
        data: &[u8],
        encoding: GraphEncoding,
    ) -> Result<Box<dyn ModelCapability>> {
        let model = self.backend.load_model(data, encoding)?;
        Ok(Box::new(model))
    }

    fn create_context_dyn(&self, model: &dyn ModelCapability) -> Result<Box<dyn ComputeCapable>> {
        #[cfg(feature = "tract")]
        {
            // Try to downcast to TractModel
            if let Some(tract_model) = model.as_any().downcast_ref::<TractModel>() {
                let num_inputs = tract_model.num_inputs();
                let context = TractContext {
                    model_id:         tract_model.id(),
                    encoding:         GraphEncoding::ONNX,
                    capability_level: wrt_foundation::verification::VerificationLevel::Standard,
                    inputs:           vec![None; num_inputs],
                    outputs:          None,
                    runnable:         tract_model.runnable.clone(),
                };
                Ok(Box::new(context))
            } else {
                Err(Error::wasi_unsupported_operation(
                    "Model is not a Tract model - cannot create context",
                ))
            }
        }

        #[cfg(not(feature = "tract"))]
        {
            let num_inputs = model.num_inputs();
            let context = TractContext {
                model_id:         model.id(),
                encoding:         GraphEncoding::ONNX,
                capability_level: wrt_foundation::verification::VerificationLevel::Standard,
            };
            Ok(Box::new(context))
        }
    }

    fn compute_dyn(
        &self,
        context: &mut dyn ComputeCapable,
        inputs: &[Tensor],
        model: &dyn ModelCapability,
    ) -> Result<Vec<Tensor>> {
        // Simply delegate to the ComputeCapable trait method
        context.compute(inputs, model)
    }

    fn name(&self) -> &'static str {
        self.backend.name()
    }
}

/// Calculate SHA-256 hash of model data
fn calculate_model_hash(data: &[u8]) -> [u8; 32] {
    super::sha256::sha256(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nn::capabilities::{
        DynamicNNCapability,
        StaticNNCapability,
    };

    #[test]
    fn test_tract_backend_creation() {
        let capability = DynamicNNCapability::new();
        let backend = TractBackend::new(capability);
        assert_eq!(backend.name(), "tract");
    }

    #[test]
    fn test_tract_supports_encoding() {
        let capability = DynamicNNCapability::new();
        let backend = TractBackend::new(capability);

        assert!(backend.supports_encoding(GraphEncoding::ONNX));
        assert!(backend.supports_encoding(GraphEncoding::TractNative));
        assert!(!backend.supports_encoding(GraphEncoding::TensorFlow));
    }

    #[test]
    fn test_tract_tensor_creation() {
        let capability = DynamicNNCapability::new();
        let backend = TractBackend::new(capability);

        let dims = TensorDimensions::new(&[10, 10]).unwrap();
        let tensor = backend.create_tensor(dims, TensorType::F32).unwrap();

        assert_eq!(tensor.size_bytes(), 400); // 10*10*4
        assert!(tensor.is_contiguous());
    }

    #[test]
    fn test_model_hash_sha256() {
        // Test empty model
        let empty_hash = calculate_model_hash(b"");
        let expected_empty = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(empty_hash, expected_empty);

        // Test with some model data
        let model_data = b"ONNX model data";
        let hash = calculate_model_hash(model_data);
        // Verify it produces a 32-byte hash
        assert_eq!(hash.len(), 32);
        // Verify it's deterministic
        let hash2 = calculate_model_hash(model_data);
        assert_eq!(hash, hash2);

        // Test with different data produces different hash
        let different_data = b"Different model data";
        let different_hash = calculate_model_hash(different_data);
        assert_ne!(hash, different_hash);
    }

    #[test]
    fn test_model_hash_approval() {
        // Create a static capability with pre-approved hashes
        let model_data = b"approved model";
        let hash = calculate_model_hash(model_data);

        let capability = StaticNNCapability::new(&[hash]).unwrap();
        assert!(capability.is_model_approved(&hash));

        // Test unapproved model
        let other_data = b"unapproved model";
        let other_hash = calculate_model_hash(other_data);
        assert!(!capability.is_model_approved(&other_hash));
    }
}
