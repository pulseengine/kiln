//! Test for Tract backend compilation and basic functionality

#![cfg(all(feature = "wasi-nn", feature = "tract"))]

use wrt_wasi::nn::{
    backend::{
        BackendProvider,
        NeuralNetworkBackend,
        TensorCapability,
    },
    capabilities::{
        DynamicNNCapability,
        NeuralNetworkCapability,
    },
    tract_backend::{
        TractBackend,
        TractBackendProvider,
    },
    GraphEncoding,
    TensorDimensions,
    TensorType,
};

#[test]
fn test_tract_backend_compiles() {
    // This test ensures the Tract backend compiles correctly
    let capability = DynamicNNCapability::new();
    let backend = TractBackend::new(capability);

    // Test backend name
    assert_eq!(backend.name(), "tract");

    // Test encoding support
    assert!(backend.supports_encoding(GraphEncoding::ONNX));
    assert!(!backend.supports_encoding(GraphEncoding::TensorFlow));
}

#[test]
fn test_tract_tensor_creation() {
    let capability = DynamicNNCapability::new();
    let backend = TractBackend::new(capability);

    // Create a simple tensor
    let dims = TensorDimensions::new(&[10, 10]).unwrap();
    let tensor = backend.create_tensor(dims, TensorType::F32).unwrap();

    assert_eq!(tensor.size_bytes(), 400); // 10*10*4
}

#[test]
fn test_tract_backend_provider() {
    let provider = TractBackendProvider::new();

    // Test encoding support
    assert!(provider.supports_encoding(GraphEncoding::ONNX));
    assert!(provider.supports_encoding(GraphEncoding::TractNative));

    // Test backend creation
    let capability = DynamicNNCapability::new();
    let backend = provider.create_backend(&capability);
    assert!(backend.is_ok());
}

#[test]
#[ignore = "Requires real ONNX model data"]
fn test_tract_model_loading() {
    // This test would require actual ONNX model data
    // For now, we just verify the API compiles
    let capability = DynamicNNCapability::new();
    let backend = TractBackend::new(capability);

    // Dummy ONNX data (invalid, but tests compilation)
    let dummy_data = vec![0u8; 100];
    let result = backend.load_model(&dummy_data, GraphEncoding::ONNX);

    // We expect this to fail with invalid encoding
    assert!(result.is_err());
}
