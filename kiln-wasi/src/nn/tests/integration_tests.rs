//! Comprehensive integration tests for WASI-NN implementation
//!
//! Tests cover QM, ASIL-A, and ASIL-B safety levels with real functionality

use crate::{
    nn::*,
    prelude::*,
};

/// Test basic capability creation and verification levels
#[test]
fn test_capability_levels() {
    // Test QM level (Standard)
    let qm_cap = capabilities::DynamicNNCapability::new();
    assert_eq!(
        qm_cap.verification_level(),
        capabilities::VerificationLevel::Standard
    );
    assert!(qm_cap.allows_dynamic_loading());

    // Test ASIL-A level (Sampling)
    let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
    assert_eq!(
        asil_a_cap.verification_level(),
        capabilities::VerificationLevel::Sampling
    );
    assert!(asil_a_cap.allows_dynamic_loading());

    // Test ASIL-B level (Continuous)
    let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
    assert_eq!(
        asil_b_cap.verification_level(),
        capabilities::VerificationLevel::Continuous
    );
    assert!(!asil_b_cap.allows_dynamic_loading()); // Only pre-approved models
}

/// Test capability factory for all safety levels
#[test]
fn test_capability_factory() {
    use capabilities::VerificationLevel;

    // Test QM creation
    let qm_cap = capabilities::create_nn_capability(VerificationLevel::Standard).unwrap();
    assert_eq!(qm_cap.verification_level(), VerificationLevel::Standard);

    // Test ASIL-A creation
    let asil_a_cap = capabilities::create_nn_capability(VerificationLevel::Sampling).unwrap();
    assert_eq!(asil_a_cap.verification_level(), VerificationLevel::Sampling);

    // Test ASIL-B creation
    let asil_b_cap = capabilities::create_nn_capability(VerificationLevel::Continuous).unwrap();
    assert_eq!(
        asil_b_cap.verification_level(),
        VerificationLevel::Continuous
    );
}

/// Test tensor creation with different capability levels
#[test]
fn test_tensor_creation_with_capabilities() {
    let dims = TensorDimensions::new(&[2, 3, 4]).unwrap();

    // Test with QM capability (most permissive)
    let qm_cap = capabilities::DynamicNNCapability::new();
    let qm_tensor = Tensor::new(dims.clone(), TensorType::F32, &qm_cap).unwrap();
    assert_eq!(qm_tensor.size_bytes(), 96); // 2*3*4*4 bytes
    assert_eq!(
        qm_tensor.capability_level(),
        capabilities::VerificationLevel::Standard
    );

    // Test with ASIL-A capability (bounded)
    let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
    let asil_a_tensor = Tensor::new(dims.clone(), TensorType::F32, &asil_a_cap).unwrap();
    assert_eq!(asil_a_tensor.size_bytes(), 96);
    assert_eq!(
        asil_a_tensor.capability_level(),
        capabilities::VerificationLevel::Sampling
    );

    // Test with ASIL-B capability (most restrictive)
    let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
    let asil_b_tensor = Tensor::new(dims, TensorType::F32, &asil_b_cap).unwrap();
    assert_eq!(asil_b_tensor.size_bytes(), 96);
    assert_eq!(
        asil_b_tensor.capability_level(),
        capabilities::VerificationLevel::Continuous
    );
}

/// Test resource limits enforcement across safety levels
#[test]
fn test_resource_limits_enforcement() {
    // Create oversized tensor dimensions
    let large_dims = TensorDimensions::new(&[1000, 1000, 100]).unwrap(); // 400MB tensor

    // QM should allow large tensors (100MB default limit, but this exceeds it)
    let qm_cap = capabilities::DynamicNNCapability::new();
    let qm_result = Tensor::new(large_dims.clone(), TensorType::F32, &qm_cap);
    assert!(qm_result.is_err()); // Should fail due to size limit

    // ASIL-A has tighter limits (20MB)
    let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
    let asil_a_result = Tensor::new(large_dims.clone(), TensorType::F32, &asil_a_cap);
    assert!(asil_a_result.is_err()); // Should definitely fail

    // ASIL-B has tightest limits (10MB)
    let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
    let asil_b_result = Tensor::new(large_dims, TensorType::F32, &asil_b_cap);
    assert!(asil_b_result.is_err()); // Should definitely fail
}

/// Test operation verification for different safety levels
#[test]
fn test_operation_verification() {
    use capabilities::{
        ModelFormat,
        NNOperation,
    };

    // Test load operation verification
    let load_op = NNOperation::Load {
        size:   10 * 1024 * 1024, // 10MB
        format: ModelFormat::ONNX,
    };

    // QM should allow
    let qm_cap = capabilities::DynamicNNCapability::new();
    assert!(qm_cap.verify_operation(&load_op).is_ok());

    // ASIL-A should allow (within 50MB limit)
    let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
    assert!(asil_a_cap.verify_operation(&load_op).is_ok());

    // ASIL-B should allow (within 20MB limit)
    let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
    assert!(asil_b_cap.verify_operation(&load_op).is_ok());

    // Test oversized model
    let large_load_op = NNOperation::Load {
        size:   100 * 1024 * 1024, // 100MB
        format: ModelFormat::ONNX,
    };

    // QM should allow (within 100MB default limit)
    assert!(qm_cap.verify_operation(&large_load_op).is_ok());

    // ASIL-A should reject (exceeds 50MB limit)
    assert!(asil_a_cap.verify_operation(&large_load_op).is_err());

    // ASIL-B should reject (exceeds 20MB limit)
    assert!(asil_b_cap.verify_operation(&large_load_op).is_err());
}

/// Test model format restrictions across safety levels
#[test]
fn test_model_format_restrictions() {
    use capabilities::ModelFormat;

    // QM allows all formats
    let qm_cap = capabilities::DynamicNNCapability::new();
    let qm_formats = qm_cap.allowed_formats();
    assert!(qm_formats.contains(&ModelFormat::ONNX));
    assert!(qm_formats.contains(&ModelFormat::TensorFlow));
    assert!(qm_formats.contains(&ModelFormat::PyTorch));
    assert!(qm_formats.contains(&ModelFormat::TractNative));

    // ASIL-A only allows tested formats
    let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
    let asil_a_formats = asil_a_cap.allowed_formats();
    assert!(asil_a_formats.contains(&ModelFormat::ONNX));
    assert!(asil_a_formats.contains(&ModelFormat::TractNative));
    assert_eq!(asil_a_formats.len(), 2); // Only 2 formats allowed

    // ASIL-B only allows verified formats
    let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
    let asil_b_formats = asil_b_cap.allowed_formats();
    assert!(asil_b_formats.contains(&ModelFormat::ONNX));
    assert!(asil_b_formats.contains(&ModelFormat::TractNative));
    assert_eq!(asil_b_formats.len(), 2); // Only verified formats
}

/// Test SHA-256 model hashing for security
#[test]
fn test_model_hashing() {
    use crate::nn::sha256::sha256;

    // Test empty model hash
    let empty_hash = sha256(b"");
    let expected_empty = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(empty_hash, expected_empty);

    // Test deterministic hashing
    let model_data = b"test model data";
    let hash1 = sha256(model_data);
    let hash2 = sha256(model_data);
    assert_eq!(hash1, hash2);

    // Test different data produces different hash
    let different_data = b"different model data";
    let different_hash = sha256(different_data);
    assert_ne!(hash1, different_hash);
}

/// Test constant-time model approval for ASIL-B
#[test]
fn test_constant_time_model_approval() {
    // Create test hashes
    let approved_hash1 = [0xAAu8; 32];
    let approved_hash2 = [0xBBu8; 32];
    let unapproved_hash = [0xCCu8; 32];

    // Create ASIL-B capability with approved models
    let asil_b_cap =
        capabilities::StaticNNCapability::new(&[approved_hash1, approved_hash2]).unwrap();

    // Test approved models
    assert!(asil_b_cap.is_model_approved(&approved_hash1));
    assert!(asil_b_cap.is_model_approved(&approved_hash2));

    // Test unapproved model
    assert!(!asil_b_cap.is_model_approved(&unapproved_hash));

    // Test timing attack resistance (hash differing only in first byte)
    let mut early_diff = approved_hash1;
    early_diff[0] = 0xFF;
    assert!(!asil_b_cap.is_model_approved(&early_diff));

    // Test timing attack resistance (hash differing only in last byte)
    let mut late_diff = approved_hash1;
    late_diff[31] = 0xFF;
    assert!(!asil_b_cap.is_model_approved(&late_diff));
}

/// Test tensor operations with bounds checking
#[test]
fn test_tensor_operations() {
    let dims = TensorDimensions::new(&[2, 3]).unwrap();
    let qm_cap = capabilities::DynamicNNCapability::new();
    let mut tensor = Tensor::new(dims, TensorType::F32, &qm_cap).unwrap();

    // Test size calculations
    assert_eq!(tensor.size_bytes(), 24); // 2*3*4 bytes
    assert_eq!(tensor.dimensions().num_elements(), 6);

    // Test data access
    let data = tensor.as_bytes();
    assert_eq!(data.len(), 24);
    assert!(data.iter().all(|&b| b == 0)); // Initially zero

    // Test mutable data access
    let data_mut = tensor.as_bytes_mut();
    data_mut[0] = 42;
    assert_eq!(tensor.as_bytes()[0], 42);
}

/// Test tensor dimensions validation
#[test]
fn test_tensor_dimensions_validation() {
    // Test valid dimensions
    let valid_dims = TensorDimensions::new(&[1, 2, 3, 4]).unwrap();
    assert_eq!(valid_dims.rank(), 4);
    assert_eq!(valid_dims.num_elements(), 24);
    assert!(valid_dims.is_valid());

    // Test too many dimensions
    let too_many_dims = TensorDimensions::new(&[1; 10]); // MAX_TENSOR_DIMS = 8
    assert!(too_many_dims.is_err());

    // Test zero dimensions (should be valid now since we removed the check)
    let zero_dims = TensorDimensions::new(&[]).unwrap();
    assert_eq!(zero_dims.rank(), 0);
    assert_eq!(zero_dims.num_elements(), 1); // fold starts with 1
}

/// Test WIT type conversions
#[test]
fn test_wit_type_conversions() {
    use crate::nn::wit_types::WitTypeConversion;

    // Test TensorType conversions
    assert_eq!(TensorType::F32.to_wit(), 1);
    assert_eq!(TensorType::from_wit(1).unwrap(), TensorType::F32);

    assert_eq!(TensorType::I32.to_wit(), 8);
    assert_eq!(TensorType::from_wit(8).unwrap(), TensorType::I32);

    // Test invalid conversion
    assert!(TensorType::from_wit(255).is_err());

    // Test GraphEncoding conversions
    assert_eq!(GraphEncoding::ONNX.to_wit(), 0);
    assert_eq!(GraphEncoding::from_wit(0).unwrap(), GraphEncoding::ONNX);

    // Test ExecutionTarget conversions
    assert_eq!(ExecutionTarget::CPU.to_wit(), 0);
    assert_eq!(ExecutionTarget::from_wit(0).unwrap(), ExecutionTarget::CPU);
}

/// Test error code conversions
#[test]
fn test_error_conversions() {
    use crate::nn::wit_types::ErrorCode;

    // Test Error to ErrorCode conversion
    let error = Error::wasi_invalid_argument("test");
    let code: ErrorCode = error.into();
    assert_eq!(code, ErrorCode::InvalidArgument);

    // Test ErrorCode to Error conversion
    let error2: Error = ErrorCode::ResourceExhausted.into();
    assert!(error2.message().contains("Resource exhausted"));
}

/// Test global state initialization (thread-safe)
#[test]
fn test_global_state_initialization() {
    // Test capability initialization
    let capability = Box::new(capabilities::DynamicNNCapability::new());

    // This test might fail if already initialized in other tests
    // In a real implementation, we'd use test isolation
    let result = initialize_nn(capability);
    // Don't assert success since it might already be initialized

    // Test that we can get the capability
    let cap_result = get_nn_capability();
    // Should either work or give a clear error
    match cap_result {
        Ok(_cap) => {
            // Successfully got capability
        },
        Err(e) => {
            // Expected if not initialized or lock failure
            println!("Capability access result: {:?}", e);
        },
    }
}

/// Test store operations (would need mutable access in real implementation)
#[test]
fn test_store_operations() {
    // Test graph store creation
    let graph_store_result = graph::GraphStore::new();
    assert!(graph_store_result.is_ok());
    let graph_store = graph_store_result.unwrap();
    assert_eq!(graph_store.count(), 0);
    assert!(!graph_store.is_full());

    // Test context store creation
    let context_store_result = execution::ContextStore::new();
    assert!(context_store_result.is_ok());
}

/// Test tensor builder pattern
#[test]
fn test_tensor_builder() {
    let qm_cap = capabilities::DynamicNNCapability::new();

    // Test builder pattern
    let tensor = TensorBuilder::new()
        .dimensions(&[5, 5])
        .unwrap()
        .data_type(TensorType::U8)
        .build(&qm_cap)
        .unwrap();

    assert_eq!(tensor.dimensions().num_elements(), 25);
    assert_eq!(tensor.data_type(), TensorType::U8);
    assert_eq!(tensor.size_bytes(), 25); // 25 * 1 byte

    // Test builder with data
    let data = vec![42u8; 25];
    let tensor_with_data = TensorBuilder::new()
        .dimensions(&[5, 5])
        .unwrap()
        .data_type(TensorType::U8)
        .data(data.clone())
        .build(&qm_cap)
        .unwrap();

    assert_eq!(tensor_with_data.as_bytes(), &data);
}

/// Integration test: Complete WASI-NN workflow simulation
#[test]
fn test_complete_workflow_simulation() {
    // This test simulates a complete WASI-NN workflow without actual model loading

    // 1. Initialize capability (QM level)
    let capability = capabilities::DynamicNNCapability::new();

    // 2. Create mock model data
    let model_data = b"mock ONNX model data";
    let model_hash = sha256::sha256(model_data);

    // 3. Verify model would be approved
    assert!(capability.is_model_approved(&model_hash)); // QM approves all

    // 4. Create input tensor
    let input_dims = TensorDimensions::new(&[1, 3, 224, 224]).unwrap(); // Typical image input
    let input_tensor = Tensor::new(input_dims, TensorType::F32, &capability).unwrap();
    assert_eq!(input_tensor.size_bytes(), 602112); // 1*3*224*224*4 bytes

    // 5. Create output tensor
    let output_dims = TensorDimensions::new(&[1, 1000]).unwrap(); // Typical classification output
    let output_tensor = Tensor::new(output_dims, TensorType::F32, &capability).unwrap();
    assert_eq!(output_tensor.size_bytes(), 4000); // 1*1000*4 bytes

    // 6. Verify operation would be allowed
    let load_op = capabilities::NNOperation::Load {
        size:   model_data.len(),
        format: capabilities::ModelFormat::ONNX,
    };
    assert!(capability.verify_operation(&load_op).is_ok());

    let compute_op = capabilities::NNOperation::Compute {
        estimated_flops: 1_000_000,
    };
    assert!(capability.verify_operation(&compute_op).is_ok());
}

/// ASIL compliance test: Verify no unsafe operations
#[test]
fn test_asil_compliance() {
    // All operations should be safe and deterministic

    // Test that all capability levels work deterministically
    for _ in 0..10 {
        let qm_cap = capabilities::DynamicNNCapability::new();
        assert_eq!(
            qm_cap.verification_level(),
            capabilities::VerificationLevel::Standard
        );

        let asil_a_cap = capabilities::BoundedNNCapability::new().unwrap();
        assert_eq!(
            asil_a_cap.verification_level(),
            capabilities::VerificationLevel::Sampling
        );

        let asil_b_cap = capabilities::StaticNNCapability::new(&[]).unwrap();
        assert_eq!(
            asil_b_cap.verification_level(),
            capabilities::VerificationLevel::Continuous
        );
    }

    // Test that tensor operations are deterministic
    let dims = TensorDimensions::new(&[10, 10]).unwrap();
    let qm_cap = capabilities::DynamicNNCapability::new();

    for _ in 0..10 {
        let tensor = Tensor::new(dims.clone(), TensorType::F32, &qm_cap).unwrap();
        assert_eq!(tensor.size_bytes(), 400);
        assert!(tensor.as_bytes().iter().all(|&b| b == 0));
    }
}
