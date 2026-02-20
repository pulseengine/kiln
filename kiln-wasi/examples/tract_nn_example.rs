//! Example demonstrating Tract backend usage with WASI-NN
//!
//! This example shows how the Tract backend integrates with WASI-NN
//! to perform neural network inference on ONNX models.

#[cfg(all(feature = "std", feature = "wasi-nn", feature = "tract"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use wrt_wasi::nn::{
        backend::{
            initialize_backends,
            BackendRegistry,
        },
        capabilities::{
            create_nn_capability,
            NNVerificationLevel as VerificationLevel,
        },
        execution::initialize_context_store,
        graph::initialize_graph_store,
        initialize_nn,
        tract_backend::TractBackendProvider,
        GraphEncoding,
    };

    println!("Initializing WASI-NN with Tract backend...");

    // Initialize WASI-NN with appropriate capability
    let capability = create_nn_capability(VerificationLevel::Standard)?;
    initialize_nn(capability)?;

    // Initialize stores
    initialize_graph_store()?;
    initialize_context_store()?;

    // Initialize backend registry and register Tract
    initialize_backends()?;

    // In a real implementation, this would be done in initialize_backends
    let mut registry = BackendRegistry::new();
    registry.register(GraphEncoding::ONNX, Box::new(TractBackendProvider::new()));

    println!("WASI-NN with Tract backend initialized successfully!");

    // Example: Load an ONNX model (would need real model data)
    // let model_data = std::fs::read("model.onnx")?;
    // let graph_id = wrt_wasi::nn::sync_bridge::nn_load(
    // model_data,
    // GraphEncoding::ONNX as u8,
    // ExecutionTarget::CPU as u8,
    // )?;
    //
    // println!("Model loaded with graph ID: {}", graph_id);
    //
    // Create execution context
    // let context_id =
    // wrt_wasi::nn::sync_bridge::nn_init_execution_context(graph_id)?;
    //
    // println!("Execution context created with ID: {}", context_id);
    //
    // Set inputs, run inference, get outputs...

    println!("\nTract backend features:");
    println!("- Pure Rust implementation");
    println!("- ONNX model support");
    println!("- No external dependencies");
    println!("- Capability-aware resource management");
    println!("- Safety level support (QM, ASIL-A, ASIL-B)");

    Ok(())
}

#[cfg(not(all(feature = "std", feature = "wasi-nn", feature = "tract")))]
fn main() {
    eprintln!("This example requires features: std, wasi-nn, tract");
    std::process::exit(1);
}
