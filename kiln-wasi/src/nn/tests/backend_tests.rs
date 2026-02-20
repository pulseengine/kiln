//! Backend implementation tests for WASI-NN

use crate::{
    nn::*,
    prelude::*,
};

/// Test basic backend functionality
mod backend_basic_tests {
    use super::*;

    #[test]
    fn test_backend_registry_initialization() {
        // Test that backend registry can be created
        let registry = backend::BackendRegistry::new();

        // Registry should start empty
        // Note: In the current implementation, we can't easily test the
        // internal state but we can verify the registry exists and can
        // be used
    }

    #[test]
    fn test_tract_backend_creation() {
        let capability = capabilities::DynamicNNCapability::new();
        let tract_backend = tract_backend::TractBackend::new(capability);

        assert_eq!(tract_backend.name(), "tract");

        // Test supported encodings
        assert!(tract_backend.supports_encoding(GraphEncoding::ONNX));
        assert!(tract_backend.supports_encoding(GraphEncoding::TractNative));
        assert!(!tract_backend.supports_encoding(GraphEncoding::TensorFlow));
        assert!(!tract_backend.supports_encoding(GraphEncoding::PyTorch));
    }

    #[test]
    fn test_tract_backend_provider() {
        let provider = tract_backend::TractBackendProvider::new();

        // Test encoding support
        assert!(provider.supports_encoding(GraphEncoding::ONNX));
        assert!(provider.supports_encoding(GraphEncoding::TractNative));
        assert!(!provider.supports_encoding(GraphEncoding::TensorFlow));
    }

    #[test]
    fn test_backend_tensor_creation() {
        let capability = capabilities::DynamicNNCapability::new();
        let tract_backend = tract_backend::TractBackend::new(capability);

        let dims = TensorDimensions::new(&[10, 10]).unwrap();
        let tensor_result = tract_backend.create_tensor(dims, TensorType::F32);

        assert!(tensor_result.is_ok());
        let tensor = tensor_result.unwrap();
        assert_eq!(tensor.size_bytes(), 400); // 10*10*4
        assert!(tensor.is_contiguous());
    }

    #[test]
    fn test_tensor_capability_trait() {
        let capability = capabilities::DynamicNNCapability::new();
        let tract_backend = tract_backend::TractBackend::new(capability);

        let dims = TensorDimensions::new(&[5, 5]).unwrap();
        let tensor = tract_backend.create_tensor(dims.clone(), TensorType::F32).unwrap();

        // Test TensorCapability trait methods
        assert_eq!(tensor.dimensions().as_slice(), dims.as_slice());
        assert_eq!(tensor.data_type(), TensorType::F32);
        assert_eq!(tensor.size_bytes(), 100); // 5*5*4

        // Test data read/write
        let mut buffer = vec![0u8; 100];
        assert!(tensor.read_data(&mut buffer).is_ok());
        assert!(buffer.iter().all(|&b| b == 0)); // Initially zero
    }
}

/// Test model loading capabilities
mod model_loading_tests {
    use super::*;

    #[test]
    fn test_model_hash_calculation() {
        // Test SHA-256 hash calculation in tract backend
        let test_data = b"test model data";
        let hash1 = tract_backend::calculate_model_hash(test_data);
        let hash2 = tract_backend::calculate_model_hash(test_data);

        // Should be deterministic
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // SHA-256 produces 32 bytes

        // Different data should produce different hash
        let different_data = b"different model data";
        let different_hash = tract_backend::calculate_model_hash(different_data);
        assert_ne!(hash1, different_hash);
    }

    #[test]
    fn test_model_size_validation() {
        let capability = capabilities::BoundedNNCapability::new().unwrap(); // 50MB limit
        let tract_backend = tract_backend::TractBackend::new(capability);

        // Test oversized model data
        let large_data = vec![0u8; 60 * 1024 * 1024]; // 60MB
        let large_model_result = tract_backend.load_model(&large_data, GraphEncoding::ONNX);
        assert!(large_model_result.is_err()); // Should fail due to size limit

        // Test acceptable model data
        let small_data = vec![0u8; 10 * 1024 * 1024]; // 10MB
                                                      // Note: This will still fail because it's not valid ONNX data, but size check
                                                      // should pass
        let small_model_result = tract_backend.load_model(&small_data, GraphEncoding::ONNX);
        // The error should be about invalid encoding, not size
        if let Err(e) = small_model_result {
            assert!(e.message().contains("encoding") || e.message().contains("parse"));
        }
    }

    #[test]
    fn test_model_approval_checking() {
        // Test ASIL-B model approval
        let approved_hash = [0x12u8; 32];
        let capability = capabilities::StaticNNCapability::new(&[approved_hash]).unwrap();
        let tract_backend = tract_backend::TractBackend::new(capability);

        // Create data that would produce the approved hash
        // Note: In practice, this would be real model data
        let test_data = b"test"; // This won't actually produce the approved hash

        // The load_model would check the hash against approved list
        let model_result = tract_backend.load_model(test_data, GraphEncoding::ONNX);
        // Should fail because hash doesn't match approved list
        assert!(model_result.is_err());
    }

    #[test]
    fn test_unsupported_encoding() {
        let capability = capabilities::DynamicNNCapability::new();
        let tract_backend = tract_backend::TractBackend::new(capability);

        let test_data = b"test model data";

        // Test unsupported encodings
        let tf_result = tract_backend.load_model(test_data, GraphEncoding::TensorFlow);
        assert!(tf_result.is_err());

        let pytorch_result = tract_backend.load_model(test_data, GraphEncoding::PyTorch);
        assert!(pytorch_result.is_err());
    }
}

/// Test execution context functionality
mod execution_tests {
    use super::*;

    #[test]
    fn test_execution_context_creation() {
        // Create a mock model capability
        struct MockModel {
            id:          u32,
            num_inputs:  usize,
            num_outputs: usize,
        }

        impl backend::ModelCapability for MockModel {
            fn id(&self) -> u32 {
                self.id
            }

            fn size(&self) -> usize {
                1024
            }

            fn hash(&self) -> [u8; 32] {
                [0u8; 32]
            }

            fn input_metadata(&self, _index: usize) -> Result<(TensorDimensions, TensorType)> {
                Ok((TensorDimensions::new(&[1, 3, 224, 224])?, TensorType::F32))
            }

            fn output_metadata(&self, _index: usize) -> Result<(TensorDimensions, TensorType)> {
                Ok((TensorDimensions::new(&[1, 1000])?, TensorType::F32))
            }

            fn num_inputs(&self) -> usize {
                self.num_inputs
            }

            fn num_outputs(&self) -> usize {
                self.num_outputs
            }

            fn as_any(&self) -> &dyn core::any::Any {
                self
            }
        }

        // Create a mock compute capable context
        struct MockContext;

        impl backend::ComputeCapable for MockContext {
            fn compute(
                &mut self,
                _inputs: &[Tensor],
                _model: &dyn backend::ModelCapability,
            ) -> Result<Vec<Tensor>> {
                Ok(vec![])
            }

            fn as_any(&self) -> &dyn core::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
                self
            }
        }

        impl std::fmt::Debug for MockContext {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("MockContext").finish()
            }
        }

        // Create mock graph
        let model = MockModel {
            id:          1,
            num_inputs:  1,
            num_outputs: 1,
        };
        let capability = capabilities::DynamicNNCapability::new();

        // Note: We can't easily test Graph creation without fixing the type
        // issues This demonstrates the structure but would need graph
        // creation to work
    }

    #[test]
    fn test_execution_stats() {
        let mut stats = execution::ExecutionStats::default();

        assert_eq!(stats.inference_count, 0);
        assert_eq!(stats.total_time_us, 0);
        assert_eq!(stats.peak_memory_bytes, 0);

        // Simulate updating stats
        stats.inference_count += 1;
        stats.total_time_us += 1000;
        stats.peak_memory_bytes = 1024;

        assert_eq!(stats.inference_count, 1);
        assert_eq!(stats.total_time_us, 1000);
        assert_eq!(stats.peak_memory_bytes, 1024);

        // Simulate another inference
        stats.inference_count += 1;
        stats.total_time_us += 1500;
        stats.peak_memory_bytes = stats.peak_memory_bytes.max(2048);

        assert_eq!(stats.inference_count, 2);
        assert_eq!(stats.total_time_us, 2500);
        assert_eq!(stats.peak_memory_bytes, 2048);
    }

    #[test]
    fn test_compute_error_conversion() {
        use execution::ComputeError;

        let missing_input_error: Error = ComputeError::MissingInput(0).into();
        assert!(missing_input_error.message().contains("Input 0 not set"));

        let invalid_index_error: Error = ComputeError::InvalidInputIndex(5).into();
        assert!(invalid_index_error.message().contains("Invalid input index: 5"));

        let timeout_error: Error = ComputeError::Timeout.into();
        assert!(timeout_error.message().contains("timeout"));
    }
}

/// Test store functionality
mod store_tests {
    use super::*;

    #[test]
    fn test_graph_store_operations() {
        let mut store = graph::GraphStore::new().unwrap();

        // Test initial state
        assert_eq!(store.count(), 0);
        assert!(!store.is_full());

        // Test ID generation
        let id1 = store.next_id();
        let id2 = store.next_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Test wraparound protection
        // Note: We can't easily test this without modifying internal state
    }

    #[test]
    fn test_context_store_operations() {
        let mut store = execution::ContextStore::new().unwrap();

        // Test ID generation
        let id1 = store.next_id();
        let id2 = store.next_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Test wraparound protection
        // This ensures IDs never become 0 (which might be used as invalid)
    }

    #[test]
    fn test_store_capacity_limits() {
        // Test that stores respect their maximum capacity
        // This is now enforced by the Vec length checks rather than BoundedVec

        let store = graph::GraphStore::new().unwrap();
        assert!(!store.is_full()); // Should not be full initially

        // Note: Testing actual capacity limits would require creating many
        // graphs, which is complex without proper graph creation
        // infrastructure
    }
}

/// Test thread safety and concurrency
#[cfg(feature = "std")]
mod concurrency_tests {
    use std::{
        sync::Arc,
        thread,
    };

    use super::*;

    #[test]
    fn test_capability_thread_safety() {
        // Test that capabilities can be shared across threads safely
        let capability = Arc::new(capabilities::DynamicNNCapability::new());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let cap = Arc::clone(&capability);
                thread::spawn(move || {
                    // Each thread performs some operations
                    let dims = TensorDimensions::new(&[10, 10]).unwrap();
                    let tensor_result = Tensor::new(dims, TensorType::F32, cap.as_ref);
                    assert!(tensor_result.is_ok());

                    let op = capabilities::NNOperation::Load {
                        size:   1024 * i, // Different sizes per thread
                        format: capabilities::ModelFormat::ONNX,
                    };
                    assert!(cap.verify_operation(&op).is_ok());

                    i * 42 // Return some value
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result < 500); // All should complete successfully
        }
    }

    #[test]
    fn test_hash_computation_thread_safety() {
        use crate::nn::sha256::sha256;

        // Test that SHA-256 computation is thread-safe
        let test_data = Arc::new(b"test data for threading".to_vec());

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let data = Arc::clone(&test_data);
                thread::spawn(move || sha256(&data))
            })
            .collect();

        let mut hashes = Vec::new();
        for handle in handles {
            hashes.push(handle.join().unwrap());
        }

        // All hashes should be identical (deterministic)
        let first_hash = &hashes[0];
        for hash in &hashes[1..] {
            assert_eq!(hash, first_hash);
        }
    }

    #[test]
    fn test_model_approval_thread_safety() {
        // Test that model approval checking is thread-safe
        let approved_hash = [0x33u8; 32];
        let capability = Arc::new(capabilities::StaticNNCapability::new(&[approved_hash]).unwrap());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let cap = Arc::clone(&capability);
                thread::spawn(move || {
                    let mut test_hash = approved_hash;
                    test_hash[0] = i as u8; // Make each hash different

                    let is_approved = cap.is_model_approved(&test_hash);
                    (i, is_approved)
                })
            })
            .collect();

        for handle in handles {
            let (i, is_approved) = handle.join().unwrap();
            if i == 0x33 {
                assert!(is_approved); // Should match the approved hash
            } else {
                assert!(!is_approved); // Should not match
            }
        }
    }
}

/// Test error handling and edge cases
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_tensor_dimensions() {
        let capability = capabilities::DynamicNNCapability::new();

        // Test empty dimensions
        let empty_dims_result = TensorDimensions::new(&[]);
        assert!(empty_dims_result.is_ok()); // Empty dimensions are now allowed

        // Test too many dimensions
        let too_many_dims = vec![1u32; 20]; // More than MAX_TENSOR_DIMS
        let result = TensorDimensions::new(&too_many_dims);
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_size_overflow() {
        let capability = capabilities::DynamicNNCapability::new();

        // Test dimensions that would cause size overflow
        let huge_dims = TensorDimensions::new(&[u32::MAX / 2, u32::MAX / 2]).unwrap();
        let tensor_result = Tensor::new(huge_dims, TensorType::F32, &capability);

        // Should fail due to size calculation overflow or resource limits
        assert!(tensor_result.is_err());
    }

    #[test]
    fn test_capability_limit_enforcement() {
        // Test that each capability level properly enforces its limits

        // ASIL-B with very restrictive custom limits
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
        let limits = asil_b_cap.resource_limits();

        // Test tensor that exceeds limits
        let large_tensor_size = limits.max_tensor_memory + 1;
        let num_elements = large_tensor_size / 4; // F32 = 4 bytes
        let side_length = (num_elements as f64).sqrt() as u32;

        if side_length > 0 {
            let large_dims = TensorDimensions::new(&[side_length, side_length]).unwrap();
            let tensor_result = Tensor::new(large_dims, TensorType::F32, &asil_b_cap);
            assert!(tensor_result.is_err());
        }
    }

    #[test]
    fn test_invalid_wit_conversions() {
        use crate::nn::wit_types::WitTypeConversion;

        // Test invalid tensor type conversion
        let invalid_tensor_type = TensorType::from_wit(255);
        assert!(invalid_tensor_type.is_err());

        // Test invalid graph encoding conversion
        let invalid_encoding = GraphEncoding::from_wit(255);
        assert!(invalid_encoding.is_err());

        // Test invalid execution target conversion
        let invalid_target = ExecutionTarget::from_wit(255);
        assert!(invalid_target.is_err());
    }

    #[test]
    fn test_backend_error_propagation() {
        let capability = capabilities::DynamicNNCapability::new();
        let tract_backend = tract_backend::TractBackend::new(capability);

        // Test that backend properly propagates errors
        let invalid_data = b"not a valid model";
        let result = tract_backend.load_model(invalid_data, GraphEncoding::ONNX);

        // Should fail with appropriate error
        assert!(result.is_err());
        let error = result.unwrap_err();
        // Error should indicate parsing/encoding issue
        assert!(
            error.message().contains("encoding")
                || error.message().contains("parse")
                || error.message().contains("model")
        );
    }
}
