//! Integration tests for WASI-NN with wrtd

#![cfg(all(feature = "wasi-nn", feature = "preview2"))]

use wrt_foundation::{
    memory_init::{
        init_crate_memory,
        init_wrt_memory,
    },
    CrateId,
};
use wrt_wasi::{
    nn::{
        backend::initialize_backends,
        capabilities::{
            create_nn_capability,
            BoundedNNCapability,
            DynamicNNCapability,
            NNVerificationLevel as VerificationLevel,
            StaticNNCapability,
        },
        execution::initialize_context_store,
        graph::initialize_graph_store,
        initialize_nn,
        is_nn_available,
        ModelFormat,
        NNOperation,
        NeuralNetworkCapability,
    },
    prelude::*,
    ComponentModelProvider,
    WasiCapabilities,
    WasiNeuralNetworkCapabilities,
};

/// Test full WASI-NN initialization flow
#[test]
fn test_wasi_nn_full_initialization() {
    // Initialize memory system first
    init_wrt_memory().unwrap();
    init_crate_memory(CrateId::Wasi).unwrap();

    // Create capabilities
    let mut caps = WasiCapabilities::sandboxed().unwrap();
    caps.nn = WasiNeuralNetworkCapabilities::full_access().unwrap();

    // Create provider
    let provider = ComponentModelProvider::new(caps).unwrap();

    // The provider should support NN functions when NN capabilities are enabled
    // We can't directly test the functions list, but we can verify the provider
    // was created successfully with NN capabilities
    assert_eq!(provider.capabilities().nn.dynamic_loading, true);
}

/// Test capability-based NN initialization
#[test]
fn test_nn_capability_initialization() {
    // Test each supported verification level
    for level in &[VerificationLevel::Standard, VerificationLevel::Sampling] {
        let capability = create_nn_capability(*level).unwrap();
        let result = if !is_nn_available() { initialize_nn(capability) } else { Ok(()) };
        assert!(result.is_ok());
    }
}

/// Test NN subsystem initialization
#[test]
fn test_nn_subsystem_init() {
    // Initialize capability
    let capability = create_nn_capability(VerificationLevel::Standard).unwrap();
    if !is_nn_available() {
        initialize_nn(capability).unwrap();
    }

    // Initialize backend registry
    initialize_backends().unwrap();

    // Initialize stores
    initialize_graph_store().unwrap();
    initialize_context_store().unwrap();

    // Verify we can get the capability
    let retrieved = wrt_wasi::nn::get_nn_capability();
    assert!(retrieved.is_ok());
}

/// Test WASI-NN capability restrictions
#[test]
fn test_nn_capability_restrictions() {
    use wrt_wasi::nn::capabilities::{
        BoundedNNCapability,
        DynamicNNCapability,
        ModelFormat,
        NNOperation,
        StaticNNCapability,
    };

    // Test dynamic capability (QM)
    let dynamic_cap = DynamicNNCapability::new();
    let load_op = NNOperation::Load {
        size:   50 * 1024 * 1024,
        format: ModelFormat::ONNX,
    };
    assert!(dynamic_cap.verify_operation(&load_op).is_ok());

    // Test bounded capability (ASIL-A)
    let bounded_cap = BoundedNNCapability::new().unwrap();
    let big_load = NNOperation::Load {
        size:   100 * 1024 * 1024,
        format: ModelFormat::ONNX,
    };
    assert!(bounded_cap.verify_operation(&big_load).is_err());

    // Test static capability (ASIL-B)
    let static_cap = StaticNNCapability::new(&[]).unwrap();
    // Static capability should have Continuous verification level
    let foundation_level: wrt_foundation::verification::VerificationLevel =
        static_cap.verification_level().into();
    assert_eq!(
        foundation_level,
        wrt_foundation::verification::VerificationLevel::Full
    );
}

/// Test WASI capabilities integration
#[test]
fn test_wasi_capabilities_with_nn() {
    // Initialize memory system first
    init_wrt_memory().unwrap();
    init_crate_memory(CrateId::Wasi).unwrap();

    // Create minimal capabilities
    let caps = WasiCapabilities::minimal().unwrap();
    assert!(!caps.nn.dynamic_loading);

    // Create sandboxed capabilities
    let caps = WasiCapabilities::sandboxed().unwrap();
    assert!(caps.nn.dynamic_loading);
    assert_eq!(caps.nn.max_model_size, 10 * 1024 * 1024);

    // Create full access capabilities
    let caps = WasiCapabilities::system_utility().unwrap();
    assert!(caps.nn.dynamic_loading);
    assert_eq!(caps.nn.max_model_size, 100 * 1024 * 1024);
}

/// Test verification level mapping
#[test]
fn test_verification_level_mapping() {
    // Test that each level maps correctly
    let foundation_standard: wrt_foundation::verification::VerificationLevel =
        VerificationLevel::Standard.into();
    let qm_caps =
        WasiNeuralNetworkCapabilities::for_verification_level(foundation_standard).unwrap();
    assert_eq!(qm_caps.verification_level, foundation_standard);
    assert!(qm_caps.dynamic_loading);

    let foundation_sampling: wrt_foundation::verification::VerificationLevel =
        VerificationLevel::Sampling.into();
    let asil_a_caps =
        WasiNeuralNetworkCapabilities::for_verification_level(foundation_sampling).unwrap();
    assert_eq!(asil_a_caps.verification_level, foundation_sampling);
    assert!(asil_a_caps.dynamic_loading);

    let foundation_continuous: wrt_foundation::verification::VerificationLevel =
        VerificationLevel::Continuous.into();
    let asil_b_caps =
        WasiNeuralNetworkCapabilities::for_verification_level(foundation_continuous).unwrap();
    assert_eq!(asil_b_caps.verification_level, foundation_continuous);
    assert!(!asil_b_caps.dynamic_loading);
    assert!(asil_b_caps.require_model_approval);
}

/// Test tensor operations with different capabilities
#[test]
fn test_tensor_operations() {
    use wrt_wasi::nn::{
        capabilities::DynamicNNCapability,
        Tensor,
        TensorDimensions,
        TensorType,
    };

    let capability = DynamicNNCapability::new();

    // Create small tensor - should succeed
    let dims = TensorDimensions::new(&[10, 10]).unwrap();
    let tensor = Tensor::new(dims, TensorType::F32, &capability).unwrap();
    assert_eq!(tensor.size_bytes(), 400);

    // Try to create tensor exceeding limits
    let capability =
        DynamicNNCapability::with_limits(wrt_wasi::nn::capabilities::NNResourceLimits {
            max_tensor_memory: 100,
            ..Default::default()
        });

    let dims = TensorDimensions::new(&[10, 10]).unwrap();
    let result = Tensor::new(dims, TensorType::F32, &capability);
    assert!(result.is_err()); // Should fail due to memory limit
}
