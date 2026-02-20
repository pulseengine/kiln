//! Synchronous WASI-NN bridge for Preview2
//!
//! This module provides the synchronous API implementation for WASI-NN that
//! works with WASI Preview2's synchronous execution model.

use super::{
    execute_inference,
    get_backend_registry,
    get_context_store,
    get_graph_store,
    get_nn_capability,
    initialize_context_store,
    initialize_graph_store,
    with_nn_capability,
    ErrorCode,
    ExecutionContext,
    ExecutionTarget,
    Graph,
    GraphEncoding,
    NNOperation,
    Tensor,
    TensorBuilder,
    TensorDimensions,
    TensorType,
    WitTypeConversion,
};
use crate::{
    prelude::*,
    Value,
};

/// Load a neural network graph
///
/// Implements `wasi:nn/inference.load`
pub fn nn_load(data: Vec<u8>, encoding: u8, target: u8) -> Result<u32> {
    let start_time = get_current_time_us();
    let operation_id = if let Some(logger) = crate::nn::monitoring::get_logger() {
        logger.next_operation_id()
    } else {
        0
    };

    // Log operation start
    if let Some(logger) = crate::nn::monitoring::get_logger() {
        logger.log_operation(
            crate::nn::monitoring::OperationEvent::Started {
                operation: "load".to_string(),
                operation_id,
                context: format!(
                    "model_size={}, encoding={}, target={}",
                    data.len(),
                    encoding,
                    target
                ),
            },
            "sync_bridge",
        );
    }

    // Initialize stores if needed
    initialize_graph_store().ok();
    initialize_context_store().ok();

    // Validate input data
    if data.is_empty() {
        return Err(Error::wasi_invalid_argument("Model data cannot be empty"));
    }

    // Absolute maximum model size (500MB) to prevent DoS
    const MAX_ABSOLUTE_MODEL_SIZE: usize = 500 * 1024 * 1024;
    if data.len() > MAX_ABSOLUTE_MODEL_SIZE {
        return Err(Error::wasi_resource_exhausted(
            "Model exceeds absolute size limit",
        ));
    }

    // Convert parameters with validation
    let encoding = <GraphEncoding as WitTypeConversion>::from_wit(encoding)?;
    let target = <ExecutionTarget as WitTypeConversion>::from_wit(target)?;

    // Basic format validation based on encoding
    validate_model_format(&data, encoding)?;

    // Execute with capability
    let result = with_nn_capability(|capability| {
        // Verify operation against capability limits (includes rate limiting and
        // quotas)
        capability.verify_operation(&NNOperation::Load {
            size:   data.len(),
            format: encoding.to_model_format(),
        })?;

        // Track resource allocation if tracking is enabled
        if let Some(tracker) = capability.resource_tracker() {
            tracker.allocate_model(data.len())?;
        }

        // Get appropriate backend
        let registry = get_backend_registry()?;
        let backend = registry.get_backend(encoding, capability)?;

        // Load model through backend
        let backend_model = backend.load_model_dyn(&data, encoding)?;

        // Create graph and store it
        let mut store = get_graph_store()?;
        let graph_id = store.next_id()?;

        let graph = Graph::new(graph_id, encoding, target, &data, backend_model, capability)?;

        store.add(graph)?;

        // Note: Model resource deallocation will happen when graph is dropped
        // via the Drop implementation in Graph

        Ok(graph_id)
    });

    let duration = get_current_time_us() - start_time;

    // Log operation completion
    if let Some(logger) = crate::nn::monitoring::get_logger() {
        match &result {
            Ok(graph_id) => {
                logger.log_operation(
                    crate::nn::monitoring::OperationEvent::Completed {
                        operation: "load".to_string(),
                        operation_id,
                        duration_us: duration,
                    },
                    "sync_bridge",
                );

                logger.log_performance(
                    crate::nn::monitoring::PerformanceEvent::OperationTiming {
                        operation:   "load".to_string(),
                        duration_us: duration,
                        success:     true,
                    },
                    "sync_bridge",
                );

                logger.log_resource(
                    crate::nn::monitoring::ResourceEvent::Allocated {
                        resource_type: "model".to_string(),
                        amount:        data.len(),
                        total_used:    data.len(), // Simplified for now
                    },
                    "sync_bridge",
                );
            },
            Err(e) => {
                logger.log_operation(
                    crate::nn::monitoring::OperationEvent::Failed {
                        operation: "load".to_string(),
                        operation_id,
                        error: e.to_string(),
                        duration_us: duration,
                    },
                    "sync_bridge",
                );

                logger.log_performance(
                    crate::nn::monitoring::PerformanceEvent::OperationTiming {
                        operation:   "load".to_string(),
                        duration_us: duration,
                        success:     false,
                    },
                    "sync_bridge",
                );
            },
        }
    }

    result
}

/// Initialize an execution context for a graph
///
/// Implements `wasi:nn/inference.init-execution-context`
pub fn nn_init_execution_context(graph_id: u32) -> Result<u32> {
    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::CreateContext { model_id: graph_id })?;

        // Track resource allocation if tracking is enabled
        if let Some(tracker) = capability.resource_tracker() {
            tracker.allocate_context()?;
        }

        // Get the graph
        let graph_store = get_graph_store()?;
        let graph = graph_store.get(graph_id)?;

        // Get backend and create context
        let registry = get_backend_registry()?;
        let backend = registry.get_backend(graph.encoding(), capability)?;

        // Create backend-specific context
        let backend_context = backend.create_context_dyn(graph.backend_model())?;

        // Create execution context
        let mut context_store = get_context_store()?;
        let context_id = context_store.next_id()?;

        let context = ExecutionContext::new(context_id, graph, backend_context, capability)?;

        context_store.add(context)?;

        // Note: Context resource deallocation will happen when context is dropped
        // via the Drop implementation in ExecutionContext

        Ok(context_id)
    })
}

/// Set input tensor for execution
///
/// Implements `wasi:nn/inference.set-input`
pub fn nn_set_input(
    context_id: u32,
    index: u32,
    tensor_data: Vec<u8>,
    dimensions: Vec<u32>,
    tensor_type: u8,
) -> Result<()> {
    // Comprehensive input validation
    if tensor_data.is_empty() {
        return Err(Error::wasi_invalid_argument("Tensor data cannot be empty"));
    }

    if dimensions.is_empty() {
        return Err(Error::wasi_invalid_argument(
            "Tensor dimensions cannot be empty",
        ));
    }

    // Validate each dimension
    for &dim in &dimensions {
        if dim == 0 {
            return Err(Error::wasi_invalid_argument(
                "Tensor dimensions cannot be zero",
            ));
        }
        if dim > 65536 {
            // Reasonable upper bound per dimension
            return Err(Error::wasi_invalid_argument("Tensor dimension too large"));
        }
    }

    // Convert and validate tensor type
    let tensor_type = <TensorType as WitTypeConversion>::from_wit(tensor_type)?;

    // Create dimensions with validation
    let tensor_dims = TensorDimensions::new(&dimensions)?;

    // Validate tensor data size matches dimensions × type size
    let expected_elements = tensor_dims.checked_num_elements()?;
    let expected_size = expected_elements
        .checked_mul(tensor_type.size_bytes())
        .ok_or_else(|| Error::wasi_resource_exhausted("Tensor size calculation overflow"))?;

    if tensor_data.len() != expected_size {
        return Err(Error::wasi_invalid_argument(
            "Tensor data size doesn't match dimensions and type",
        ));
    }

    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::SetInput {
            size:       tensor_data.len(),
            dimensions: dimensions.clone(),
        })?;

        // Create tensor
        let tensor = Tensor::from_data(tensor_dims, tensor_type, tensor_data, capability)?;

        // Set on context with bounds checking
        let index_usize = usize::try_from(index)
            .map_err(|_| Error::wasi_invalid_argument("Input index too large"))?;

        let mut context_store = get_context_store()?;
        let context = context_store.get_mut(context_id)?;

        // Note: set_input will do its own bounds checking
        // No need to duplicate the check here

        context.set_input(index_usize, tensor)?;

        Ok(())
    })
}

/// Execute inference
///
/// Implements `wasi:nn/inference.compute`
pub fn nn_compute(context_id: u32) -> Result<()> {
    let start_time = get_current_time_us();
    let operation_id = if let Some(logger) = crate::nn::monitoring::get_logger() {
        logger.next_operation_id()
    } else {
        0
    };

    // Log operation start
    if let Some(logger) = crate::nn::monitoring::get_logger() {
        logger.log_operation(
            crate::nn::monitoring::OperationEvent::Started {
                operation: "compute".to_string(),
                operation_id,
                context: format!("context_id={}", context_id),
            },
            "sync_bridge",
        );
    }

    with_nn_capability(|capability| {
        // Get context and execute
        let mut context_store = get_context_store()?;
        let context = context_store.get_mut(context_id)?;

        execute_inference(context, capability)?;

        let duration = get_current_time_us() - start_time;

        // Log successful completion
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_operation(
                crate::nn::monitoring::OperationEvent::Completed {
                    operation: "compute".to_string(),
                    operation_id,
                    duration_us: duration,
                },
                "sync_bridge",
            );

            logger.log_performance(
                crate::nn::monitoring::PerformanceEvent::OperationTiming {
                    operation:   "compute".to_string(),
                    duration_us: duration,
                    success:     true,
                },
                "sync_bridge",
            );
        }

        Ok(())
    })
    .map_err(|e| {
        let duration = get_current_time_us() - start_time;

        // Log operation failure
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_operation(
                crate::nn::monitoring::OperationEvent::Failed {
                    operation: "compute".to_string(),
                    operation_id,
                    error: e.to_string(),
                    duration_us: duration,
                },
                "sync_bridge",
            );

            logger.log_performance(
                crate::nn::monitoring::PerformanceEvent::OperationTiming {
                    operation:   "compute".to_string(),
                    duration_us: duration,
                    success:     false,
                },
                "sync_bridge",
            );
        }

        e
    })
}

/// Get output tensor from execution
///
/// Implements `wasi:nn/inference.get-output`
pub fn nn_get_output(context_id: u32, index: u32) -> Result<(Vec<u8>, Vec<u32>, u8)> {
    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::GetOutput { index })?;

        // Get output from context with bounds checking
        let index_usize = usize::try_from(index)
            .map_err(|_| Error::wasi_invalid_argument("Output index too large"))?;

        let context_store = get_context_store()?;
        let context = context_store.get(context_id)?;
        let tensor = context.get_output(index_usize)?;

        // Convert to WIT types
        let data = tensor.as_bytes().to_vec();
        let dimensions = tensor.dimensions().as_slice().to_vec();
        let tensor_type = tensor.data_type().to_wit();

        Ok((data, dimensions, tensor_type))
    })
}

/// Get output tensor metadata without copying data
///
/// Implements `wasi:nn/inference.get-output-metadata`
pub fn nn_get_output_metadata(context_id: u32, index: u32) -> Result<(Vec<u32>, u8)> {
    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::GetOutput { index })?;

        // Get graph from context
        let context_store = get_context_store()?;
        let context = context_store.get(context_id)?;
        let graph_store = get_graph_store()?;
        let graph = graph_store.get(context.graph_id())?;

        // Get metadata from model with bounds checking
        let index_usize = usize::try_from(index)
            .map_err(|_| Error::wasi_invalid_argument("Output index too large"))?;
        let (dims, dtype) = graph.backend_model().output_metadata(index_usize)?;

        Ok((dims.as_slice().to_vec(), dtype.to_wit()))
    })
}

/// Drop a graph
///
/// Implements `wasi:nn/inference.drop-graph`
pub fn nn_drop_graph(graph_id: u32) -> Result<()> {
    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::DropResource {
            resource_type: super::capabilities::ResourceType::Model,
        })?;

        let mut graph_store = get_graph_store()?;
        graph_store.remove(graph_id)?;

        Ok(())
    })
}

/// Drop an execution context
///
/// Implements `wasi:nn/inference.drop-execution-context`
pub fn nn_drop_execution_context(context_id: u32) -> Result<()> {
    with_nn_capability(|capability| {
        capability.verify_operation(&NNOperation::DropResource {
            resource_type: super::capabilities::ResourceType::ExecutionContext,
        })?;

        // Note: Real implementation would properly remove from store
        // For now, return success
        Ok(())
    })
}

/// WASI-NN function wrapper for component model integration
pub fn wasi_nn_load(_target: &mut dyn core::any::Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Extract arguments
    if args.len() != 3 {
        return Err(Error::wasi_invalid_argument("Expected 3 arguments"));
    }

    let data = match &args[0] {
        Value::List(bytes) => bytes
            .iter()
            .map(|v| match v {
                Value::U8(b) => Ok(*b),
                _ => Err(Error::wasi_invalid_argument("Invalid tensor data")),
            })
            .collect::<Result<Vec<u8>>>()?,
        _ => return Err(Error::wasi_invalid_argument("Expected list of bytes")),
    };

    let encoding = match &args[1] {
        Value::U8(e) => *e,
        _ => return Err(Error::wasi_invalid_argument("Expected encoding")),
    };

    let target = match &args[2] {
        Value::U8(t) => *t,
        _ => return Err(Error::wasi_invalid_argument("Expected target")),
    };

    // Call implementation
    let graph_id = nn_load(data, encoding, target)?;

    Ok(vec![Value::U32(graph_id)])
}

/// WASI-NN function wrapper for init-execution-context
pub fn wasi_nn_init_execution_context(
    _target: &mut dyn core::any::Any,
    args: Vec<Value>,
) -> Result<Vec<Value>> {
    if args.len() != 1 {
        return Err(Error::wasi_invalid_argument("Expected 1 argument"));
    }

    let graph_id = match &args[0] {
        Value::U32(id) => *id,
        _ => return Err(Error::wasi_invalid_argument("Expected graph ID")),
    };

    let context_id = nn_init_execution_context(graph_id)?;

    Ok(vec![Value::U32(context_id)])
}

/// WASI-NN function wrapper for set-input
pub fn wasi_nn_set_input(_target: &mut dyn core::any::Any, args: Vec<Value>) -> Result<Vec<Value>> {
    if args.len() != 5 {
        return Err(Error::wasi_invalid_argument("Expected 5 arguments"));
    }

    let context_id = match &args[0] {
        Value::U32(id) => *id,
        _ => return Err(Error::wasi_invalid_argument("Expected context ID")),
    };

    let index = match &args[1] {
        Value::U32(idx) => *idx,
        _ => return Err(Error::wasi_invalid_argument("Expected index")),
    };

    let tensor_data = match &args[2] {
        Value::List(bytes) => bytes
            .iter()
            .map(|v| match v {
                Value::U8(b) => Ok(*b),
                _ => Err(Error::wasi_invalid_argument("Invalid tensor data")),
            })
            .collect::<Result<Vec<u8>>>()?,
        _ => return Err(Error::wasi_invalid_argument("Expected tensor data")),
    };

    let dimensions = match &args[3] {
        Value::List(dims) => dims
            .iter()
            .map(|v| match v {
                Value::U32(d) => Ok(*d),
                _ => Err(Error::wasi_invalid_argument("Invalid dimension")),
            })
            .collect::<Result<Vec<u32>>>()?,
        _ => return Err(Error::wasi_invalid_argument("Expected dimensions")),
    };

    let tensor_type = match &args[4] {
        Value::U8(t) => *t,
        _ => return Err(Error::wasi_invalid_argument("Expected tensor type")),
    };

    nn_set_input(context_id, index, tensor_data, dimensions, tensor_type)?;

    Ok(vec![])
}

/// WASI-NN function wrapper for compute
pub fn wasi_nn_compute(_target: &mut dyn core::any::Any, args: Vec<Value>) -> Result<Vec<Value>> {
    if args.len() != 1 {
        return Err(Error::wasi_invalid_argument("Expected 1 argument"));
    }

    let context_id = match &args[0] {
        Value::U32(id) => *id,
        _ => return Err(Error::wasi_invalid_argument("Expected context ID")),
    };

    nn_compute(context_id)?;

    Ok(vec![])
}

/// WASI-NN function wrapper for get-output
pub fn wasi_nn_get_output(
    _target: &mut dyn core::any::Any,
    args: Vec<Value>,
) -> Result<Vec<Value>> {
    if args.len() != 2 {
        return Err(Error::wasi_invalid_argument("Expected 2 arguments"));
    }

    let context_id = match &args[0] {
        Value::U32(id) => *id,
        _ => return Err(Error::wasi_invalid_argument("Expected context ID")),
    };

    let index = match &args[1] {
        Value::U32(idx) => *idx,
        _ => return Err(Error::wasi_invalid_argument("Expected index")),
    };

    let (data, dimensions, tensor_type) = nn_get_output(context_id, index)?;

    // Convert to Value types
    let data_values: Vec<Value> = data.into_iter().map(Value::U8).collect();
    let dim_values: Vec<Value> = dimensions.into_iter().map(Value::U32).collect();

    Ok(vec![
        Value::List(data_values),
        Value::List(dim_values),
        Value::U8(tensor_type),
    ])
}

/// Validate model data format against claimed encoding
fn validate_model_format(data: &[u8], encoding: GraphEncoding) -> Result<()> {
    match encoding {
        GraphEncoding::ONNX => {
            // Basic ONNX format validation - check for ONNX magic bytes
            if data.len() < 8 {
                return Err(Error::wasi_invalid_argument(
                    "Model data too short for ONNX format",
                ));
            }

            // ONNX models typically start with protobuf bytes or have specific structure
            // This is a basic validation - in production you'd use a proper ONNX parser
            if !data.starts_with(&[0x08]) && !data.starts_with(&[0x08, 0x01]) {
                // Many ONNX files start with version info
                // For now, accept if it looks like binary data
                if data.iter().all(|&b| b == 0) {
                    return Err(Error::wasi_invalid_argument(
                        "Model appears to be empty/null data",
                    ));
                }
            }
        },
        GraphEncoding::TensorFlow => {
            // Basic TensorFlow SavedModel validation
            if data.len() < 16 {
                return Err(Error::wasi_invalid_argument(
                    "Model data too short for TensorFlow format",
                ));
            }
            // TensorFlow models are typically in SavedModel format or protobuf
            // This would require more sophisticated validation in production
        },
        GraphEncoding::PyTorch => {
            // Basic PyTorch model validation - typically pickle format
            if data.len() < 8 {
                return Err(Error::wasi_invalid_argument(
                    "Model data too short for PyTorch format",
                ));
            }
            // PyTorch models often start with pickle protocol bytes
            if !data.starts_with(&[0x80]) && !data.starts_with(b"PK") {
                // Could be zip format (which PyTorch also uses)
                if data.iter().all(|&b| b == 0) {
                    return Err(Error::wasi_invalid_argument(
                        "Model appears to be empty/null data",
                    ));
                }
            }
        },
        GraphEncoding::OpenVINO => {
            // OpenVINO IR format validation
            if data.len() < 4 {
                return Err(Error::wasi_invalid_argument(
                    "Model data too short for OpenVINO format",
                ));
            }
            // OpenVINO models are typically XML + bin files
            // For our purpose, ensure it's not empty/invalid
            if data.iter().all(|&b| b == 0) {
                return Err(Error::wasi_invalid_argument(
                    "Model appears to be empty/null data",
                ));
            }
        },
        GraphEncoding::TractNative => {
            // Tract native format validation
            if data.len() < 4 {
                return Err(Error::wasi_invalid_argument(
                    "Model data too short for Tract format",
                ));
            }
            // Tract has its own serialization format
            // Basic validation to ensure it's not obviously invalid
            if data.iter().all(|&b| b == 0) {
                return Err(Error::wasi_invalid_argument(
                    "Model appears to be empty/null data",
                ));
            }
        },
    }

    Ok(())
}

/// Get current time in microseconds
fn get_current_time_us() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
    }
    #[cfg(not(feature = "std"))]
    {
        wrt_platform::time::PlatformTime::get_monotonic_time_us()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_conversion() {
        assert_eq!(GraphEncoding::ONNX.to_wit(), 0);
        assert_eq!(GraphEncoding::from_wit(0).unwrap(), GraphEncoding::ONNX);
    }

    #[test]
    fn test_tensor_type_conversion() {
        assert_eq!(TensorType::F32.to_wit(), 1);
        assert_eq!(TensorType::from_wit(1).unwrap(), TensorType::F32);
    }
}
