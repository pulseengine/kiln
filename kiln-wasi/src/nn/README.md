# WASI-NN Implementation for WRT

This module provides a capability-based implementation of WASI-NN (WebAssembly System Interface for Neural Networks) for the WRT runtime.

## Architecture

The implementation is designed to be:
- **Preview-agnostic**: Works with both WASI Preview2 (synchronous) and Preview3 (asynchronous)
- **Capability-based**: Uses WRT's VerificationLevel abstraction instead of direct safety level references
- **Multi-standard**: Supports ASIL (automotive), DO-178C (aerospace), IEC 62304 (medical), and other safety standards

## Components

### Core Modules
- `mod.rs` - Main module exports and initialization
- `capabilities.rs` - Capability definitions mapping verification levels to resource limits
- `backend.rs` - Backend trait abstraction for different ML frameworks
- `tensor.rs` - Tensor representation with capability-aware memory management
- `graph.rs` - Neural network model/graph management
- `execution.rs` - Inference execution context and runtime
- `wit_types.rs` - WIT type conversions for FFI boundary

### Backend Implementations
- `tract_backend.rs` - Tract integration (pure Rust ONNX inference)

### Preview Bridges
- `sync_bridge.rs` - Preview2 synchronous API implementation
- `async_bridge.rs` - Preview3 asynchronous API preparation

## Capability Levels

The implementation maps abstract VerificationLevel to concrete capabilities:

1. **Standard (QM equivalent)**
   - Dynamic model loading
   - No pre-verification required
   - Suitable for development and testing

2. **Sampling (ASIL-A equivalent)**
   - Bounded model and tensor sizes
   - Runtime monitoring
   - Basic safety checks

3. **Continuous (ASIL-B equivalent)**
   - Pre-verified models only
   - Deterministic execution
   - Static memory allocation

## Usage Example

```rust
use wrt_wasi::nn::{
    initialize_nn, capabilities::create_nn_capability,
    sync_bridge::{nn_load, nn_init_execution_context, nn_set_input, nn_compute, nn_get_output},
};
use wrt_foundation::verification::VerificationLevel;

// Initialize with appropriate capability level
let capability = create_nn_capability(VerificationLevel::Standard)?;
initialize_nn(capability)?;

// Load a model
let model_data = std::fs::read("model.onnx")?;
let graph_id = nn_load(model_data, 0, 0)?; // 0=ONNX, 0=CPU

// Create execution context
let context_id = nn_init_execution_context(graph_id)?;

// Set input
let input_data = vec![0.0f32; 224 * 224 * 3];
nn_set_input(context_id, 0, input_data.as_bytes().to_vec(), vec![1, 224, 224, 3], 1)?;

// Run inference
nn_compute(context_id)?;

// Get output
let (output_data, output_dims, output_type) = nn_get_output(context_id, 0)?;
```

## Adding New Backends

To add a new ML framework backend:

1. Implement the `NeuralNetworkBackend` trait
2. Create a `BackendProvider` implementation
3. Register it in the `initialize_backends()` function
4. Add feature flag in Cargo.toml

## Safety Considerations

- All operations are capability-gated
- Memory allocation respects verification level limits
- Model verification required for higher safety levels
- Deterministic execution for ASIL-B and above
- Resource cleanup guaranteed through RAII

## Future Work

- Complete Tract integration with actual model loading
- Add TensorFlow Lite backend
- Implement streaming inference for Preview3
- Add model quantization support
- Implement batch inference optimization