//! Example demonstrating WASI-NN integration with wrtd
//!
//! This example shows how to:
//! 1. Configure wrtd with WASI-NN support
//! 2. Initialize WASI-NN with appropriate capability level
//! 3. Load and execute a WebAssembly module that uses WASI-NN

#[cfg(all(feature = "std", feature = "wasi", feature = "wasi-nn"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use wrt_foundation::verification::VerificationLevel;
    use wrt_wasi::{
        nn::{
            capabilities::create_nn_capability,
            initialize_nn,
        },
        WasiCapabilities,
        WasiNeuralNetworkCapabilities,
    };
    use wrtd::{
        EngineMode,
        WrtdConfig,
        WrtdEngine,
    };

    // Create wrtd configuration
    let mut config = WrtdConfig::default();
    config.enable_wasi = true;
    config.engine_mode = EngineMode::Interpreter;

    // Configure WASI capabilities with NN support
    let mut wasi_caps = WasiCapabilities::sandboxed()?;

    // Determine NN capability level based on ASIL feature
    let nn_caps = if cfg!(feature = "asil-b") {
        WasiNeuralNetworkCapabilities::for_verification_level(VerificationLevel::Continuous)?
    } else if cfg!(feature = "asil-a") {
        WasiNeuralNetworkCapabilities::for_verification_level(VerificationLevel::Sampling)?
    } else {
        WasiNeuralNetworkCapabilities::full_access()?
    };

    wasi_caps.nn = nn_caps;
    config.wasi_capabilities = Some(wasi_caps);

    // Create and initialize the engine
    let mut engine = WrtdEngine::new(config)?;

    // Initialize WASI-NN subsystem
    #[cfg(feature = "wasi-nn")]
    {
        // Get the appropriate capability level
        let nn_capability = if cfg!(feature = "asil-b") {
            create_nn_capability(VerificationLevel::Continuous)?
        } else if cfg!(feature = "asil-a") {
            create_nn_capability(VerificationLevel::Sampling)?
        } else {
            create_nn_capability(VerificationLevel::Standard)?
        };

        // Initialize NN with the capability
        initialize_nn(nn_capability)?;

        // Initialize backend registry
        use wrt_wasi::nn::backend::initialize_backends;
        initialize_backends()?;

        // Initialize stores
        use wrt_wasi::nn::{
            execution::initialize_context_store,
            graph::initialize_graph_store,
        };
        initialize_graph_store()?;
        initialize_context_store()?;

        println!("WASI-NN initialized successfully");
    }

    // Example: Load a WebAssembly module that uses WASI-NN
    // This would be a real .wasm file in practice
    let wasm_module = include_bytes!("../../simple_inference.wasm");

    // Execute the module
    match engine.execute(wasm_module) {
        Ok(result) => {
            println!("Module executed successfully");
            println!("Result: {:?}", result);

            // Print runtime statistics
            let stats = engine.get_stats();
            println!("\nRuntime Statistics:");
            println!("  Modules executed: {}", stats.modules_executed);
            println!("  WASI functions called: {}", stats.wasi_functions_called);
            println!("  Peak memory usage: {} bytes", stats.peak_memory);
        },
        Err(e) => {
            eprintln!("Execution failed: {}", e);
            return Err(e.into());
        },
    }

    Ok(())
}

#[cfg(not(all(feature = "std", feature = "wasi", feature = "wasi-nn")))]
fn main() {
    eprintln!("This example requires features: std, wasi, wasi-nn");
    std::process::exit(1);
}

/// Example of creating a simple NN inference module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(all(feature = "std", feature = "wasi", feature = "wasi-nn"))]
    fn test_nn_capability_levels() {
        use wrt_foundation::verification::VerificationLevel;
        use wrt_wasi::nn::capabilities::create_nn_capability;

        // Test QM level
        let qm_cap = create_nn_capability(VerificationLevel::Standard).unwrap();
        assert!(qm_cap.allows_dynamic_loading());

        // Test ASIL-A level
        let asil_a_cap = create_nn_capability(VerificationLevel::Sampling).unwrap();
        assert!(asil_a_cap.allows_dynamic_loading());

        // Test ASIL-B level
        let asil_b_cap = create_nn_capability(VerificationLevel::Continuous).unwrap();
        assert!(!asil_b_cap.allows_dynamic_loading());
    }
}
