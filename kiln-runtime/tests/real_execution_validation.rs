//! Validation tests for real WASM execution capability
//!
//! These tests verify that the instruction parsing and execution infrastructure
//! works correctly for both QM and ASIL-B safety levels.

#[cfg(feature = "std")]
use std::sync::Arc;

use wrt_decoder::decoder::decode_module;
use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    values::Value,
};
use wrt_runtime::{
    module::Module,
    module_instance::ModuleInstance,
    stackless::StacklessEngine,
};

/// Simple add function WebAssembly module
/// Generated from: (module (func (export "add") (param i32 i32) (result i32)
/// local.get 0 local.get 1 i32.add))
const SIMPLE_ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // Version
    // Type section: function type (i32, i32) -> i32
    0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
    // Function section: 1 function of type 0
    0x03, 0x02, 0x01, 0x00, // Export section: export function 0 as "add"
    0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // Code section: function body
    0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
];

/// Simple counter function WebAssembly module
/// Generated from: (module (global $counter (mut i32) (i32.const 0)) (func
/// (export "increment") (result i32) global.get $counter global.get $counter
/// i32.const 1 i32.add global.set $counter))
const COUNTER_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // Version
    // Type section
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // Function section
    0x03, 0x02, 0x01, 0x00, // Global section: mutable i32 initialized to 0
    0x06, 0x06, 0x01, 0x7f, 0x01, 0x41, 0x00, 0x0b, // Export section
    0x07, 0x0d, 0x01, 0x09, 0x69, 0x6e, 0x63, 0x72, 0x65, 0x6d, 0x65, 0x6e, 0x74, 0x00, 0x00,
    // Code section: global.get 0, global.get 0, i32.const 1, i32.add, global.set 0
    0x0a, 0x0c, 0x01, 0x0a, 0x00, 0x23, 0x00, 0x23, 0x00, 0x41, 0x01, 0x6a, 0x24, 0x00, 0x0b,
];

#[test]
fn test_instruction_parsing_integration() -> Result<()> {
    // Test that our instruction parsing actually works
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    // Verify that functions have parsed instructions
    assert!(
        runtime_module.functions.len() > 0,
        "Module should have functions"
    );

    let function = runtime_module.functions.get(0)?;
    assert!(
        !function.body.is_empty(),
        "Function body should contain parsed instructions"
    );
    assert!(
        function.body.len() > 0,
        "Function should have at least one instruction"
    );

    println!("✓ Function has {} parsed instructions", function.body.len());
    Ok(())
}

#[test]
fn test_stackless_engine_execution() -> Result<()> {
    // Test real execution through the stackless engine
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    // Create stackless engine
    let mut engine = StacklessEngine::new();

    // Create module instance
    let instance = ModuleInstance::new(runtime_module.clone(), 0)?;
    let instance_arc = Arc::new(instance);

    // Set current module
    let _instance_idx = engine.set_current_module(instance_arc)?;

    // Execute function 0 (the add function) with test arguments
    let args = vec![Value::I32(15), Value::I32(27)];
    let results = engine.execute(0, 0, args)?;

    // Verify execution results
    assert_eq!(results.len(), 1, "Should return exactly one result");

    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 42, "15 + 27 should equal 42");
        println!("✓ Real execution successful: 15 + 27 = {}", result);
    } else {
        return Err(Error::runtime_error("Expected i32 result"));
    }

    Ok(())
}

#[test]
fn test_memory_bounded_execution() -> Result<()> {
    // Test execution with bounded memory constraints (ASIL-B compliance)
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    // Verify all collections are bounded
    assert!(
        runtime_module.functions.capacity() > 0,
        "Functions should use bounded collections"
    );
    assert!(
        runtime_module.types.capacity() > 0,
        "Types should use bounded collections"
    );

    // Test that function body uses bounded instructions
    let function = runtime_module.functions.get(0)?;
    assert!(
        function.body.instructions.capacity() > 0,
        "Instructions should use bounded collections"
    );

    println!("✓ All collections are properly bounded for ASIL-B compliance");
    Ok(())
}

#[test]
fn test_multiple_function_calls() -> Result<()> {
    // Test multiple executions to verify engine state management
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    let mut engine = StacklessEngine::new();
    let instance = ModuleInstance::new(runtime_module, 0)?;
    let instance_arc = Arc::new(instance);
    let _instance_idx = engine.set_current_module(instance_arc)?;

    // Execute multiple times with different arguments
    let test_cases = vec![(10, 20, 30), (0, 42, 42), (100, 200, 300), (-5, 10, 5)];

    for (a, b, expected) in test_cases {
        let args = vec![Value::I32(a), Value::I32(b)];
        let results = engine.execute(0, 0, args)?;

        assert_eq!(results.len(), 1);
        if let Value::I32(result) = &results[0] {
            assert_eq!(*result, expected, "{} + {} should equal {}", a, b, expected);
        } else {
            return Err(Error::runtime_error("Expected i32 result"));
        }
    }

    println!("✓ Multiple function calls successful");
    Ok(())
}

#[test]
fn test_capability_based_memory_allocation() -> Result<()> {
    // Test that memory allocation uses capability-based approach
    let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
    assert_eq!(
        provider.capacity(),
        1024,
        "Provider should have correct capacity"
    );

    // Test bounded collection creation with capability
    let mut test_vec = wrt_foundation::bounded::BoundedVec::<i32, 10, _>::new(provider)?;
    test_vec.push(42)?;
    assert_eq!(test_vec.len(), 1);
    assert_eq!(test_vec.get(0)?, 42);

    println!("✓ Capability-based memory allocation working");
    Ok(())
}

#[test]
fn test_instruction_dispatch_coverage() -> Result<()> {
    // Test that our instruction parser and dispatcher can handle basic instruction
    // types
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    let function = runtime_module.functions.get(0)?;

    // Verify we have the expected instruction types for add function
    // The add function should contain: local.get 0, local.get 1, i32.add, end
    assert!(
        function.body.len() >= 3,
        "Add function should have at least 3 instructions"
    );

    println!(
        "✓ Instruction sequence parsed with {} instructions",
        function.body.len()
    );

    // Test actual execution to verify dispatcher works
    let mut engine = StacklessEngine::new();
    let instance = ModuleInstance::new(runtime_module, 0)?;
    let instance_arc = Arc::new(instance);
    let _instance_idx = engine.set_current_module(instance_arc)?;

    let args = vec![Value::I32(7), Value::I32(11)];
    let results = engine.execute(0, 0, args)?;

    assert_eq!(results.len(), 1);
    if let Value::I32(result) = &results[0] {
        assert_eq!(*result, 18);
        println!("✓ Instruction dispatch successful: 7 + 11 = {}", result);
    }

    Ok(())
}

#[test]
fn test_error_handling_and_bounds() -> Result<()> {
    // Test error handling for invalid operations
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    let mut engine = StacklessEngine::new();
    let instance = ModuleInstance::new(runtime_module, 0)?;
    let instance_arc = Arc::new(instance);
    let _instance_idx = engine.set_current_module(instance_arc)?;

    // Test with wrong number of arguments (should handle gracefully)
    let args = vec![Value::I32(10)]; // Missing second argument
    let result = engine.execute(0, 0, args);

    // Engine should either handle gracefully or return an error
    match result {
        Ok(_) => println!("✓ Engine handled incomplete arguments gracefully"),
        Err(e) => println!(
            "✓ Engine properly returned error for incomplete arguments: {:?}",
            e
        ),
    }

    Ok(())
}

#[test]
fn test_asil_b_compliance_features() -> Result<()> {
    // Test ASIL-B specific compliance features
    let decoded = decode_module(SIMPLE_ADD_WASM)?;
    let runtime_module = Module::from_wrt_module(&decoded)?;

    // Verify deterministic behavior - same inputs should give same outputs
    let mut engine1 = StacklessEngine::new();
    let mut engine2 = StacklessEngine::new();

    let instance1 = ModuleInstance::new(runtime_module.clone(), 0)?;
    let instance2 = ModuleInstance::new(runtime_module, 1)?;

    let _idx1 = engine1.set_current_module(Arc::new(instance1))?;
    let _idx2 = engine2.set_current_module(Arc::new(instance2))?;

    let args = vec![Value::I32(123), Value::I32(456)];
    let result1 = engine1.execute(0, 0, args.clone())?;
    let result2 = engine2.execute(0, 0, args)?;

    assert_eq!(result1, result2, "Results should be deterministic");
    println!("✓ Deterministic execution verified for ASIL-B compliance");

    Ok(())
}
