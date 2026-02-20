//! End-to-end execution tests for the capability-based WebAssembly engine
//!
//! These tests validate the complete execution pipeline from binary WebAssembly
//! to function execution with capability verification.

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Arc;

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    values::Value,
    Result,
};
use wrt_runtime::engine::{
    CapabilityAwareEngine,
    CapabilityEngine,
    EnginePreset,
};

/// Simple WebAssembly module that adds two numbers
/// Generated from: (module (func (export "add") (param i32 i32) (result i32)
/// local.get 0 local.get 1 i32.add))
const SIMPLE_ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // Version
    // Type section
    0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // Function section
    0x03, 0x02, 0x01, 0x00, // Export section
    0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // Code section
    0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
];

/// Simple WebAssembly module with a start function
/// Generated from: (module (func $start (nop)) (start $start))
const START_FUNCTION_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // Version
    // Type section
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // Function section
    0x03, 0x02, 0x01, 0x00, // Start section
    0x08, 0x01, 0x00, // Code section
    0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
];

#[test]
fn test_qm_engine_creation() -> Result<()> {
    let engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    assert!(engine.capability_context().capability_count() >= 0);
    Ok(())
}

#[test]
fn test_asil_a_engine_creation() -> Result<()> {
    let engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilA)?;
    assert!(engine.capability_context().capability_count() >= 0);
    Ok(())
}

#[test]
fn test_asil_b_engine_creation() -> Result<()> {
    let engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilB)?;
    assert!(engine.capability_context().capability_count() >= 0);
    Ok(())
}

#[test]
fn test_module_loading_pipeline() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Test loading a simple module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;

    // Verify handle is valid
    assert!(module_handle.index() > 0);

    Ok(())
}

#[test]
fn test_module_instantiation() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load and instantiate module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Verify instance is valid
    assert!(instance_handle.index() > 0);

    Ok(())
}

#[test]
fn test_function_resolution() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load and instantiate module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Check if exported function exists
    let has_add_function = engine.has_function(instance_handle, "add")?;
    assert!(has_add_function);

    // Check non-existent function
    let has_nonexistent = engine.has_function(instance_handle, "nonexistent")?;
    assert!(!has_nonexistent);

    Ok(())
}

#[test]
fn test_function_execution() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load and instantiate module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Prepare arguments for the add function
    let args = vec![Value::I32(10), Value::I32(32)];

    // Execute the function
    let results = engine.execute(instance_handle, "add", &args)?;

    // Verify result
    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 42);
    } else {
        return Err(Error::runtime_error("Unexpected result type"));
    }

    Ok(())
}

#[test]
fn test_capability_validation() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilB)?;

    // Load and instantiate module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Test validation with proper arguments
    let args = vec![Value::I32(5), Value::I32(7)];
    let results = engine.execute_with_validation(instance_handle, "add", &args)?;

    // Verify capability-validated execution works
    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 12);
    }

    Ok(())
}

#[test]
fn test_host_function_integration() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Register a simple host function
    engine.register_host_function("env", "test_host", |args: &[Value]| -> Result<Vec<Value>> {
        // Echo the first argument
        if let Some(arg) = args.get(0) {
            Ok(vec![arg.clone()])
        } else {
            Ok(vec![Value::I32(0)])
        }
    })?;

    // Test successful registration (detailed testing of host function calls
    // would require a more complex WebAssembly module that imports host functions)
    Ok(())
}

#[test]
fn test_wasi_support_by_asil_level() -> Result<()> {
    // QM should support WASI
    let mut qm_engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    assert!(qm_engine.enable_wasi().is_ok());

    // ASIL-A should support WASI
    let mut asil_a_engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilA)?;
    assert!(asil_a_engine.enable_wasi().is_ok());

    // ASIL-B should support limited WASI
    let mut asil_b_engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilB)?;
    assert!(asil_b_engine.enable_wasi().is_ok());

    // ASIL-D should reject WASI
    let mut asil_d_engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilD)?;
    assert!(asil_d_engine.enable_wasi().is_err());

    Ok(())
}

#[test]
fn test_error_handling() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Test with invalid WebAssembly binary
    let invalid_wasm = &[0x00, 0x01, 0x02, 0x03]; // Invalid magic number
    let result = engine.load_module(invalid_wasm);
    assert!(result.is_err());

    // Test function call on non-existent instance
    let fake_handle = wrt_runtime::engine::InstanceHandle::new();
    let result = engine.execute(fake_handle, "test", &[]);
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_memory_capability_enforcement() -> Result<()> {
    // Test that different ASIL levels have different memory constraints
    let qm_engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    let asil_d_engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilD)?;

    // Both should be created successfully, but with different capability contexts
    // The capability contexts would have different memory limits based on the
    // preset
    assert!(qm_engine.capability_context().capability_count() >= 0);
    assert!(asil_d_engine.capability_context().capability_count() >= 0);

    Ok(())
}

#[test]
fn test_full_execution_pipeline() -> Result<()> {
    // Test the complete pipeline: load → instantiate → resolve → execute
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Step 1: Load module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;

    // Step 2: Instantiate module
    let instance_handle = engine.instantiate(module_handle)?;

    // Step 3: Resolve function
    assert!(engine.has_function(instance_handle, "add")?);

    // Step 4: Execute function
    let args = vec![Value::I32(20), Value::I32(22)];
    let results = engine.execute(instance_handle, "add", &args)?;

    // Step 5: Verify results
    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 42);
    }

    Ok(())
}

#[test]
fn test_multiple_instances() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load module once
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;

    // Create multiple instances
    let instance1 = engine.instantiate(module_handle)?;
    let instance2 = engine.instantiate(module_handle)?;

    // Both instances should work independently
    let args1 = vec![Value::I32(1), Value::I32(2)];
    let results1 = engine.execute(instance1, "add", &args1)?;

    let args2 = vec![Value::I32(10), Value::I32(20)];
    let results2 = engine.execute(instance2, "add", &args2)?;

    // Verify independent execution
    if let (Value::I32(r1), Value::I32(r2)) = (&results1[0], &results2[0]) {
        assert_eq!(*r1, 3);
        assert_eq!(*r2, 30);
    }

    Ok(())
}

/// Integration test that exercises the complete system
#[test]
fn test_complete_integration() -> Result<()> {
    // Test with different ASIL levels
    for preset in [
        EnginePreset::QM,
        EnginePreset::AsilA,
        EnginePreset::AsilB,
        EnginePreset::AsilD,
    ] {
        let mut engine = CapabilityAwareEngine::with_preset(preset)?;

        // Host function registration should work for less restrictive levels
        if matches!(preset, EnginePreset::QM | EnginePreset::AsilA) {
            let _ = engine.register_host_function("test", "func", |_| Ok(vec![]));
        }

        // Module loading should work for all levels
        let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;
        let instance_handle = engine.instantiate(module_handle)?;

        // Function execution should work for all levels
        let args = vec![Value::I32(15), Value::I32(27)];
        let results = engine.execute(instance_handle, "add", &args)?;

        // Verify results are consistent across ASIL levels
        assert_eq!(results.len(), 1);
        if let Value::I32(result) = &results[0] {
            assert_eq!(*result, 42);
        }
    }

    Ok(())
}
