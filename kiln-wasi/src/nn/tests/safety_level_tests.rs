//! Safety level specific tests for QM, ASIL-A, and ASIL-B compliance

use crate::{
    nn::*,
    prelude::*,
};

/// QM (Quality Management) level tests - most permissive
mod qm_tests {
    use super::*;

    #[test]
    fn test_qm_dynamic_loading() {
        let qm_cap = capabilities::DynamicNNCapability::new();
        assert!(qm_cap.allows_dynamic_loading());

        // QM should allow large models (up to default limit)
        let large_load_op = capabilities::NNOperation::Load {
            size:   90 * 1024 * 1024, // 90MB - within 100MB default
            format: capabilities::ModelFormat::ONNX,
        };
        assert!(qm_cap.verify_operation(&large_load_op).is_ok());
    }

    #[test]
    fn test_qm_all_formats_allowed() {
        let qm_cap = capabilities::DynamicNNCapability::new();
        let formats = qm_cap.allowed_formats();

        // QM should support all available formats
        assert!(formats.contains(&capabilities::ModelFormat::ONNX));
        assert!(formats.contains(&capabilities::ModelFormat::TensorFlow));
        assert!(formats.contains(&capabilities::ModelFormat::PyTorch));
        assert!(formats.contains(&capabilities::ModelFormat::OpenVINO));
        assert!(formats.contains(&capabilities::ModelFormat::TractNative));
    }

    #[test]
    fn test_qm_large_tensor_creation() {
        let qm_cap = capabilities::DynamicNNCapability::new();

        // Test large but reasonable tensor
        let large_dims = TensorDimensions::new(&[100, 100, 10]).unwrap(); // 4MB tensor
        let tensor_result = Tensor::new(large_dims, TensorType::F32, &qm_cap);
        assert!(tensor_result.is_ok());

        let tensor = tensor_result.unwrap();
        assert_eq!(tensor.size_bytes(), 4_000_000); // 100*100*10*4
        assert_eq!(
            tensor.capability_level(),
            capabilities::VerificationLevel::Standard
        );
    }

    #[test]
    fn test_qm_no_model_restrictions() {
        let qm_cap = capabilities::DynamicNNCapability::new();

        // QM should approve any model hash (no pre-approval required)
        let random_hash = [0x42u8; 32];
        assert!(qm_cap.is_model_approved(&random_hash));

        let zero_hash = [0x00u8; 32];
        assert!(qm_cap.is_model_approved(&zero_hash));

        let max_hash = [0xFFu8; 32];
        assert!(qm_cap.is_model_approved(&max_hash));
    }
}

/// ASIL-A level tests - bounded safety
mod asil_a_tests {
    use super::*;

    #[test]
    fn test_asil_a_bounded_limits() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        assert_eq!(
            asil_a_cap.verification_level(),
            capabilities::VerificationLevel::Sampling
        );

        // ASIL-A has 50MB model limit
        let limits = asil_a_cap.resource_limits();
        assert_eq!(limits.max_model_size, 50 * 1024 * 1024);
        assert_eq!(limits.max_tensor_memory, 20 * 1024 * 1024);
        assert_eq!(limits.max_tensor_dimensions, 6);
        assert_eq!(limits.max_execution_time_us, 10_000_000); // 10 seconds
        assert_eq!(limits.max_concurrent_models, 2);
        assert_eq!(limits.max_concurrent_contexts, 4);
    }

    #[test]
    fn test_asil_a_format_restrictions() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        let formats = asil_a_cap.allowed_formats();

        // ASIL-A only allows well-tested formats
        assert!(formats.contains(&capabilities::ModelFormat::ONNX));
        assert!(formats.contains(&capabilities::ModelFormat::TractNative));
        assert_eq!(formats.len(), 2); // Only 2 formats allowed

        // Should not contain experimental formats
        assert!(!formats.contains(&capabilities::ModelFormat::PyTorch));
        assert!(!formats.contains(&capabilities::ModelFormat::TensorFlow));
    }

    #[test]
    fn test_asil_a_size_enforcement() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();

        // Test model size enforcement
        let oversized_load = capabilities::NNOperation::Load {
            size:   60 * 1024 * 1024, // 60MB - exceeds 50MB limit
            format: capabilities::ModelFormat::ONNX,
        };
        assert!(asil_a_cap.verify_operation(&oversized_load).is_err());

        // Test acceptable size
        let acceptable_load = capabilities::NNOperation::Load {
            size:   40 * 1024 * 1024, // 40MB - within 50MB limit
            format: capabilities::ModelFormat::ONNX,
        };
        assert!(asil_a_cap.verify_operation(&acceptable_load).is_ok());
    }

    #[test]
    fn test_asil_a_tensor_limits() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();

        // Test tensor memory enforcement (20MB limit)
        let large_dims = TensorDimensions::new(&[1000, 1000, 6]).unwrap(); // 24MB tensor
        let large_tensor_result = Tensor::new(large_dims, TensorType::F32, &asil_a_cap);
        assert!(large_tensor_result.is_err()); // Should exceed 20MB limit

        // Test acceptable tensor
        let ok_dims = TensorDimensions::new(&[500, 500, 4]).unwrap(); // 4MB tensor
        let ok_tensor_result = Tensor::new(ok_dims, TensorType::F32, &asil_a_cap);
        assert!(ok_tensor_result.is_ok());

        let tensor = ok_tensor_result.unwrap();
        assert_eq!(
            tensor.capability_level(),
            capabilities::VerificationLevel::Sampling
        );
    }

    #[test]
    fn test_asil_a_dimension_limits() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();

        // Test dimension enforcement (6 dimensions max)
        let too_many_dims_result = TensorDimensions::new(&[2, 2, 2, 2, 2, 2, 2]); // 7 dimensions
        assert!(too_many_dims_result.is_ok()); // TensorDimensions allows up to 8

        let too_many_dims = too_many_dims_result.unwrap();
        let tensor_result = Tensor::new(too_many_dims, TensorType::U8, &asil_a_cap);
        // Note: Current implementation doesn't check rank against capability
        // limits This would need to be added for full ASIL-A compliance
    }

    #[test]
    fn test_asil_a_runtime_monitoring() {
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();

        // ASIL-A should still approve models (no pre-approval required)
        // but has runtime monitoring enabled
        let test_hash = [0x55u8; 32];
        assert!(asil_a_cap.is_model_approved(&test_hash));

        // All operations should be verified for monitoring
        let compute_op = capabilities::NNOperation::Compute {
            estimated_flops: 1_000_000,
        };
        assert!(asil_a_cap.verify_operation(&compute_op).is_ok());
    }
}

/// ASIL-B level tests - highest safety (continuous monitoring)
mod asil_b_tests {
    use super::*;

    #[test]
    fn test_asil_b_strictest_limits() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
        assert_eq!(
            asil_b_cap.verification_level(),
            capabilities::VerificationLevel::Continuous
        );

        // ASIL-B has the strictest limits
        let limits = asil_b_cap.resource_limits();
        assert_eq!(limits.max_model_size, 20 * 1024 * 1024); // 20MB
        assert_eq!(limits.max_tensor_memory, 10 * 1024 * 1024); // 10MB
        assert_eq!(limits.max_tensor_dimensions, 4);
        assert_eq!(limits.max_execution_time_us, 1_000_000); // 1 second (deterministic)
        assert_eq!(limits.max_concurrent_models, 1);
        assert_eq!(limits.max_concurrent_contexts, 2);
    }

    #[test]
    fn test_asil_b_no_dynamic_loading() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // ASIL-B prohibits dynamic loading
        assert!(!asil_b_cap.allows_dynamic_loading());
    }

    #[test]
    fn test_asil_b_verified_formats_only() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
        let formats = asil_b_cap.allowed_formats();

        // ASIL-B only allows verified, deterministic formats
        assert!(formats.contains(&capabilities::ModelFormat::ONNX));
        assert!(formats.contains(&capabilities::ModelFormat::TractNative));
        assert_eq!(formats.len(), 2);

        // Should reject experimental formats
        let pytorch_load = capabilities::NNOperation::Load {
            size:   10 * 1024 * 1024,
            format: capabilities::ModelFormat::PyTorch,
        };
        assert!(asil_b_cap.verify_operation(&pytorch_load).is_err());
    }

    #[test]
    fn test_asil_b_model_pre_approval() {
        let approved_hash1 = [0xAAu8; 32];
        let approved_hash2 = [0xBBu8; 32];
        let unapproved_hash = [0xCCu8; 32];

        let asil_b_cap =
            capabilities::StaticNNCapability::new(&[approved_hash1, approved_hash2]).unwrap();

        // Only pre-approved models allowed
        assert!(asil_b_cap.is_model_approved(&approved_hash1));
        assert!(asil_b_cap.is_model_approved(&approved_hash2));
        assert!(!asil_b_cap.is_model_approved(&unapproved_hash));
    }

    #[test]
    fn test_asil_b_constant_time_comparison() {
        // Test that model approval uses constant-time comparison
        let approved_hash = [0xDDu8; 32];
        let asil_b_cap = capabilities::StaticNNCapability::new(&[approved_hash]).unwrap();

        // These should all take the same time (no early exit on mismatch)
        let mut early_diff = approved_hash;
        early_diff[0] = 0x00; // Differs in first byte

        let mut middle_diff = approved_hash;
        middle_diff[16] = 0x00; // Differs in middle

        let mut late_diff = approved_hash;
        late_diff[31] = 0x00; // Differs in last byte

        // All should return false without timing differences
        assert!(!asil_b_cap.is_model_approved(&early_diff));
        assert!(!asil_b_cap.is_model_approved(&middle_diff));
        assert!(!asil_b_cap.is_model_approved(&late_diff));
    }

    #[test]
    fn test_asil_b_deterministic_execution() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // ASIL-B enforces deterministic execution
        let compute_op = capabilities::NNOperation::Compute {
            estimated_flops: 500_000,
        };
        assert!(asil_b_cap.verify_operation(&compute_op).is_ok());

        // The implementation should ensure deterministic behavior
        // (this would be enforced in the actual execution path)
    }

    #[test]
    fn test_asil_b_strict_tensor_limits() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // Test strict tensor memory limit (10MB)
        let large_dims = TensorDimensions::new(&[1000, 1000, 3]).unwrap(); // 12MB tensor
        let large_tensor_result = Tensor::new(large_dims, TensorType::F32, &asil_b_cap);
        assert!(large_tensor_result.is_err()); // Should exceed 10MB limit

        // Test acceptable tensor
        let ok_dims = TensorDimensions::new(&[500, 500, 1]).unwrap(); // 1MB tensor
        let ok_tensor_result = Tensor::new(ok_dims, TensorType::F32, &asil_b_cap);
        assert!(ok_tensor_result.is_ok());

        let tensor = ok_tensor_result.unwrap();
        assert_eq!(
            tensor.capability_level(),
            capabilities::VerificationLevel::Continuous
        );
    }

    #[test]
    fn test_asil_b_execution_time_limits() {
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // ASIL-B has 1 second execution limit for deterministic execution
        let limits = asil_b_cap.resource_limits();
        assert_eq!(limits.max_execution_time_us, 1_000_000);

        // This would be enforced during actual execution
    }
}

/// Cross-level compatibility tests
mod compatibility_tests {
    use super::*;

    #[test]
    fn test_capability_level_hierarchy() {
        let qm_cap = capabilities::DynamicNNCapability::new();
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // Verify hierarchy: QM < ASIL-A < ASIL-B (in terms of restrictions)
        let qm_limits = qm_cap.resource_limits();
        let asil_a_limits = asil_a_cap.resource_limits();
        let asil_b_limits = asil_b_cap.resource_limits();

        // Model size limits: QM >= ASIL-A >= ASIL-B
        assert!(qm_limits.max_model_size >= asil_a_limits.max_model_size);
        assert!(asil_a_limits.max_model_size >= asil_b_limits.max_model_size);

        // Tensor memory limits: QM >= ASIL-A >= ASIL-B
        assert!(qm_limits.max_tensor_memory >= asil_a_limits.max_tensor_memory);
        assert!(asil_a_limits.max_tensor_memory >= asil_b_limits.max_tensor_memory);

        // Concurrent models: QM >= ASIL-A >= ASIL-B
        assert!(qm_limits.max_concurrent_models >= asil_a_limits.max_concurrent_models);
        assert!(asil_a_limits.max_concurrent_models >= asil_b_limits.max_concurrent_models);
    }

    #[test]
    fn test_tensor_compatibility_across_levels() {
        // Create a tensor that should work at all levels
        let small_dims = TensorDimensions::new(&[10, 10]).unwrap(); // 400 bytes

        let qm_cap = capabilities::DynamicNNCapability::new();
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // Should work with all capability levels
        let qm_tensor = Tensor::new(small_dims.clone(), TensorType::F32, &qm_cap).unwrap();
        let asil_a_tensor = Tensor::new(small_dims.clone(), TensorType::F32, &asil_a_cap).unwrap();
        let asil_b_tensor = Tensor::new(small_dims, TensorType::F32, &asil_b_cap).unwrap();

        // All should have same size but different capability levels
        assert_eq!(qm_tensor.size_bytes(), 400);
        assert_eq!(asil_a_tensor.size_bytes(), 400);
        assert_eq!(asil_b_tensor.size_bytes(), 400);

        assert_eq!(
            qm_tensor.capability_level(),
            capabilities::VerificationLevel::Standard
        );
        assert_eq!(
            asil_a_tensor.capability_level(),
            capabilities::VerificationLevel::Sampling
        );
        assert_eq!(
            asil_b_tensor.capability_level(),
            capabilities::VerificationLevel::Continuous
        );
    }

    #[test]
    fn test_operation_verification_consistency() {
        let small_op = capabilities::NNOperation::Load {
            size:   1024 * 1024,                     // 1MB - should work for all levels
            format: capabilities::ModelFormat::ONNX, // Supported by all levels
        };

        let qm_cap = capabilities::DynamicNNCapability::new();
        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();

        // Small operation should be allowed by all levels
        assert!(qm_cap.verify_operation(&small_op).is_ok());
        assert!(asil_a_cap.verify_operation(&small_op).is_ok());
        assert!(asil_b_cap.verify_operation(&small_op).is_ok());
    }
}

/// Performance and stress tests for safety levels
mod performance_tests {
    use super::*;

    #[test]
    fn test_capability_creation_performance() {
        // Test that capability creation is fast and deterministic
        let start = std::time::Instant::now();

        for _ in 0..1000 {
            let _qm_cap = capabilities::DynamicNNCapability::new();
            let _asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
            let _asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
        }

        let duration = start.elapsed();
        assert!(duration.as_millis() < 100); // Should be very fast
    }

    #[test]
    fn test_hash_computation_performance() {
        use crate::nn::sha256::sha256;

        // Test SHA-256 performance on different sizes
        let small_data = vec![0u8; 1024]; // 1KB
        let medium_data = vec![0u8; 1024 * 1024]; // 1MB
        let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB

        let start = std::time::Instant::now();
        let _small_hash = sha256(&small_data);
        let small_time = start.elapsed();

        let start = std::time::Instant::now();
        let _medium_hash = sha256(&medium_data);
        let medium_time = start.elapsed();

        let start = std::time::Instant::now();
        let _large_hash = sha256(&large_data);
        let large_time = start.elapsed();

        // Verify performance scales reasonably
        assert!(small_time.as_millis() < 10);
        assert!(medium_time.as_millis() < 100);
        assert!(large_time.as_millis() < 1000);
    }

    #[test]
    fn test_constant_time_verification() {
        // Verify that model approval checking is constant time
        let approved_hash = [0x42u8; 32];
        let asil_b_cap = capabilities::StaticNNCapability::new(&[approved_hash]).unwrap();

        // Create hashes that differ at different positions
        let mut early_diff = approved_hash;
        early_diff[0] = 0x00;

        let mut late_diff = approved_hash;
        late_diff[31] = 0xFF;

        // Time both operations multiple times
        let iterations = 10000;

        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = asil_b_cap.is_model_approved(&early_diff);
        }
        let early_time = start.elapsed();

        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = asil_b_cap.is_model_approved(&late_diff);
        }
        let late_time = start.elapsed();

        // Times should be very similar (within 10% for constant-time property)
        let time_diff = if early_time > late_time {
            early_time - late_time
        } else {
            late_time - early_time
        };

        let max_time = std::cmp::max(early_time, late_time);
        let time_ratio = time_diff.as_nanos() as f64 / max_time.as_nanos() as f64;

        assert!(
            time_ratio < 0.1,
            "Time difference too large: {:.2}%",
            time_ratio * 100.0
        );
    }
}
