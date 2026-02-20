//! Test the complete execution pipeline with real WebAssembly

use wrt_error::Result;
use wrt_foundation::values::Value;
use wrt_runtime::engine::{
    CapabilityAwareEngine,
    CapabilityEngine,
    EnginePreset,
};

/// Simple WebAssembly module that adds two numbers
/// (module (func (export "add") (param i32 i32) (result i32) local.get 0
/// local.get 1 i32.add))
const SIMPLE_ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic number
    0x01, 0x00, 0x00, 0x00, // Version 1
    // Type section: function signature (i32, i32) -> i32
    0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
    // Function section: one function of type 0
    0x03, 0x02, 0x01, 0x00, // Export section: export function 0 as "add"
    0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // Code section: function body
    0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
];

/// Simple constant function that returns 42
/// (module (func (export "get42") (result i32) i32.const 42))
const CONST_42_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic number
    0x01, 0x00, 0x00, 0x00, // Version 1
    // Type section: function signature () -> i32
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // Function section: one function of type 0
    0x03, 0x02, 0x01, 0x00, // Export section: export function 0 as "get42"
    0x07, 0x09, 0x01, 0x05, 0x67, 0x65, 0x74, 0x34, 0x32, 0x00, 0x00,
    // Code section: function body (i32.const 42)
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b,
];

#[test]
fn test_pipeline_with_const_function() -> Result<()> {
    // Create engine with QM preset for maximum compatibility
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load the constant module
    let module_handle = engine.load_module(CONST_42_WASM)?;

    // Instantiate the module
    let instance_handle = engine.instantiate(module_handle)?;

    // Execute the constant function (no arguments)
    let results = engine.execute(instance_handle, "get42", &[])?;

    // Verify the result
    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 42);
        println!("✅ Constant function test passed: got {}", result);
    } else {
        panic!("Expected I32 result, got {:?}", results[0]);
    }

    Ok(())
}

#[test]
fn test_pipeline_with_add_function() -> Result<()> {
    // Create engine with QM preset
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;

    // Load the add module
    let module_handle = engine.load_module(SIMPLE_ADD_WASM)?;

    // Instantiate the module
    let instance_handle = engine.instantiate(module_handle)?;

    // Execute the add function with arguments 10 + 32 = 42
    let args = vec![Value::I32(10), Value::I32(32)];
    let results = engine.execute(instance_handle, "add", &args)?;

    // Verify the result
    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 42);
        println!("✅ Add function test passed: 10 + 32 = {}", result);
    } else {
        panic!("Expected I32 result, got {:?}", results[0]);
    }

    Ok(())
}

#[test]
fn test_pipeline_across_asil_levels() -> Result<()> {
    // Test that the same module works across different ASIL levels
    for preset in [
        EnginePreset::QM,
        EnginePreset::AsilA,
        EnginePreset::AsilB,
        EnginePreset::AsilD,
    ] {
        println!("Testing preset: {:?}", preset);

        let mut engine = CapabilityAwareEngine::with_preset(preset)?;
        let module_handle = engine.load_module(CONST_42_WASM)?;
        let instance_handle = engine.instantiate(module_handle)?;

        let results = engine.execute(instance_handle, "get42", &[])?;

        assert_eq!(results.len(), 1);
        if let Value::I32(result) = &results[0] {
            assert_eq!(*result, 42);
        } else {
            panic!("Expected I32 result for {:?}, got {:?}", preset, results[0]);
        }
    }

    println!("✅ All ASIL levels work correctly");
    Ok(())
}

#[test]
fn test_function_existence_checking() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    let module_handle = engine.load_module(CONST_42_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Should find the exported function
    assert!(engine.has_function(instance_handle, "get42")?);

    // Should not find a non-existent function
    assert!(!engine.has_function(instance_handle, "nonexistent")?);

    println!("✅ Function existence checking works correctly");
    Ok(())
}

#[test]
fn test_error_handling() -> Result<()> {
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    let module_handle = engine.load_module(CONST_42_WASM)?;
    let instance_handle = engine.instantiate(module_handle)?;

    // Should fail gracefully for non-existent function
    let result = engine.execute(instance_handle, "nonexistent", &[]);
    assert!(result.is_err());

    println!("✅ Error handling works correctly");
    Ok(())
}
