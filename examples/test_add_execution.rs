//! Example: Test WebAssembly execution with test_add.wasm
//!
//! This example demonstrates loading and executing a simple WebAssembly
//! module that exports an "add" function.

use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== WebAssembly Add Function Test ===\n");
    
    // Read the WebAssembly binary
    let wasm_path = Path::new("test_add.wasm";
    let wasm_bytes = fs::read(wasm_path)?;
    println!("Loaded {} bytes from {}", wasm_bytes.len(), wasm_path.display());
    
    // Test with kiln-execution feature
    #[cfg(all(feature = "std", feature = "kiln-execution"))]
    {
        use kiln::engine::{CapabilityAwareEngine, EnginePreset};
        use kiln_foundation::values::Value;
        
        println!("\n🚀 Running with actual WebAssembly execution...\n");
        
        // Create execution engine
        let mut engine = CapabilityAwareEngine::new(EnginePreset::QM)?;
        println!("✓ Created execution engine");
        
        // Load the module
        let module_handle = engine.load_module(&wasm_bytes)?;
        println!("✓ Loaded WebAssembly module");
        
        // Instantiate the module
        let instance_handle = engine.instantiate(module_handle)?;
        println!("✓ Instantiated module");
        
        // Test the add function with different inputs
        let test_cases = vec![
            (5, 3, 8),
            (10, 20, 30),
            (0, 0, 0),
            (-5, 5, 0),
            (100, 200, 300),
        ];
        
        println!("\nTesting 'add' function:");
        println!("-----------------------");
        
        for (a, b, expected) in test_cases {
            let args = vec![Value::I32(a), Value::I32(b)];
            let results = engine.execute(instance_handle, "add", &args)?;
            
            if let Some(Value::I32(result)) = results.get(0) {
                let status = if *result == expected { "✅" } else { "❌" };
                println!("{} add({}, {}) = {} (expected: {})", 
                         status, a, b, result, expected;
            } else {
                println!("❌ add({}, {}) = ERROR: No result or wrong type", a, b);
            }
        }
        
        println!("\n✨ WebAssembly execution test completed!");
    }
    
    #[cfg(not(all(feature = "std", feature = "kiln-execution")))]
    {
        println!("\n⚠️  Running in simulation mode (kiln-execution feature not enabled)");
        println!("To enable actual execution, compile with:");
        println!("  cargo run --features std,kiln-execution --example test_add_execution");
    }
    
    Ok(())
}