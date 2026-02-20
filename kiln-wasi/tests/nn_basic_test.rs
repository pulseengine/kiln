//! Basic tests for WASI-NN implementation

#![cfg(feature = "wasi-nn")]

use wrt_wasi::{
    nn::{
        capabilities::{
            create_nn_capability,
            NNVerificationLevel as VerificationLevel,
        },
        get_nn_capability,
        initialize_nn,
        sync_bridge::{
            nn_compute,
            nn_get_output,
            nn_init_execution_context,
            nn_load,
            nn_set_input,
        },
        ExecutionTarget,
        GraphEncoding,
        NeuralNetworkCapability,
        TensorType,
        WitTypeConversion,
    },
    Error,
    Result,
};

#[test]
fn test_nn_capability_creation() {
    // Test creating capabilities for different verification levels
    let qm_cap = create_nn_capability(VerificationLevel::Standard).unwrap();
    assert_eq!(qm_cap.verification_level(), VerificationLevel::Standard);
    assert!(qm_cap.allows_dynamic_loading());

    let asil_a_cap = create_nn_capability(VerificationLevel::Sampling).unwrap();
    assert_eq!(asil_a_cap.verification_level(), VerificationLevel::Sampling);

    let asil_b_cap = create_nn_capability(VerificationLevel::Continuous).unwrap();
    assert_eq!(
        asil_b_cap.verification_level(),
        VerificationLevel::Continuous
    );
    assert!(!asil_b_cap.allows_dynamic_loading());
}

#[test]
fn test_nn_initialization() {
    use wrt_wasi::nn::is_nn_available;

    // Test initializing the NN subsystem
    let capability = create_nn_capability(VerificationLevel::Standard).unwrap();
    let result = if !is_nn_available() {
        initialize_nn(capability)
    } else {
        Ok(()) // Already initialized
    };
    assert!(result.is_ok());

    // Verify we can get the capability back
    let retrieved_cap = get_nn_capability();
    assert!(retrieved_cap.is_ok());
}

#[test]
fn test_graph_encoding_conversion() {
    use wrt_wasi::nn::wit_types::WitTypeConversion;

    // Test encoding conversions
    let encoding = GraphEncoding::ONNX;
    let wit_value = encoding.to_wit();
    let converted = GraphEncoding::from_wit(wit_value).unwrap();
    assert_eq!(encoding, converted);

    // Test all encodings
    for &enc in &[
        GraphEncoding::ONNX,
        GraphEncoding::TensorFlow,
        GraphEncoding::PyTorch,
        GraphEncoding::OpenVINO,
        GraphEncoding::TractNative,
    ] {
        let wit = enc.to_wit();
        let back = GraphEncoding::from_wit(wit).unwrap();
        assert_eq!(enc, back);
    }
}

#[test]
fn test_tensor_type_conversion() {
    use wrt_wasi::nn::wit_types::WitTypeConversion;

    // Test tensor type conversions
    let tensor_type = TensorType::F32;
    let wit_value = tensor_type.to_wit();
    let converted = TensorType::from_wit(wit_value).unwrap();
    assert_eq!(tensor_type, converted);
}

#[test]
fn test_nn_load_without_backend() {
    use wrt_wasi::nn::is_nn_available;

    // Initialize NN first if not already initialized
    if !is_nn_available() {
        let capability = create_nn_capability(VerificationLevel::Standard).unwrap();
        initialize_nn(capability).unwrap();
    }

    // Try to load a model (will fail without backend initialized)
    let dummy_model = vec![0u8; 100];
    let result = nn_load(
        dummy_model,
        GraphEncoding::ONNX.to_wit(),
        ExecutionTarget::CPU.to_wit(),
    );

    // Should fail because backend registry isn't initialized
    assert!(result.is_err());
}

#[test]
fn test_tensor_dimensions() {
    use wrt_wasi::nn::TensorDimensions;

    // Test creating tensor dimensions
    let dims = TensorDimensions::new(&[1, 224, 224, 3]).unwrap();
    assert_eq!(dims.rank(), 4);
    assert_eq!(dims.num_elements(), 1 * 224 * 224 * 3);
    assert!(dims.is_valid());

    // Test invalid dimensions (too many)
    let too_many = vec![1u32; 10];
    let result = TensorDimensions::new(&too_many);
    assert!(result.is_err());

    // Test empty dimensions
    let empty_dims = TensorDimensions::new(&[]).unwrap();
    assert_eq!(empty_dims.rank(), 0);
}

#[test]
fn test_tensor_creation() {
    use wrt_wasi::nn::{
        capabilities::DynamicNNCapability,
        Tensor,
        TensorDimensions,
    };

    // Create a capability
    let capability = DynamicNNCapability::new();

    // Create a simple tensor
    let dims = TensorDimensions::new(&[2, 3]).unwrap();
    let tensor = Tensor::new(dims, TensorType::F32, &capability).unwrap();

    assert_eq!(tensor.size_bytes(), 2 * 3 * 4); // 2x3 floats = 24 bytes
    assert_eq!(tensor.data_type(), TensorType::F32);
    assert_eq!(tensor.dimensions().num_elements(), 6);
}

#[test]
fn test_error_conversions() {
    use wrt_wasi::nn::wit_types::ErrorCode;

    // Test error code conversions
    let error = Error::wasi_invalid_argument("test");
    let code: ErrorCode = error.into();
    assert_eq!(code, ErrorCode::InvalidArgument);

    let error2 = Error::wasi_resource_exhausted("test");
    let code2: ErrorCode = error2.into();
    assert_eq!(code2, ErrorCode::ResourceExhausted);
}

#[cfg(feature = "tract")]
#[test]
fn test_tract_backend_creation() {
    use wrt_wasi::nn::{
        backend::{
            BackendProvider,
            NeuralNetworkBackend,
        },
        capabilities::DynamicNNCapability,
        tract_backend::{
            TractBackend,
            TractBackendProvider,
        },
    };

    // Create a Tract backend
    let capability = DynamicNNCapability::new();
    let backend = TractBackend::new(capability);
    assert_eq!(backend.name(), "tract");

    // Test backend provider
    let provider = TractBackendProvider::new();
    assert!(provider.supports_encoding(GraphEncoding::ONNX));
    assert!(provider.supports_encoding(GraphEncoding::TractNative));
    assert!(!provider.supports_encoding(GraphEncoding::TensorFlow));
}

#[test]
fn test_capability_limits() {
    use wrt_wasi::nn::capabilities::{
        BoundedNNCapability,
        ModelFormat,
        NNOperation,
    };

    // Create bounded capability
    let capability = BoundedNNCapability::new().unwrap();
    let _limits = capability.resource_limits();

    // Test model size limit
    let load_op = NNOperation::Load {
        size:   100 * 1024 * 1024, // 100MB
        format: ModelFormat::ONNX,
    };
    let result = capability.verify_operation(&load_op);
    assert!(result.is_err()); // Should exceed bounded limit

    // Test within limits
    let small_load = NNOperation::Load {
        size:   10 * 1024 * 1024, // 10MB
        format: ModelFormat::ONNX,
    };
    let result2 = capability.verify_operation(&small_load);
    assert!(result2.is_ok());
}
