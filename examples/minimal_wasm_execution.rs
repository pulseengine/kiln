//! Minimal example showing actual WASM execution with Kiln
//! This demonstrates that we can actually execute WASM code, not just simulate

use kiln::prelude::*;
use kiln::engine::Engine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Kiln Minimal WASM Execution Demo ===\n");
    
    // Create a simple WASM module that adds two numbers
    let wat_code = r#"
        (module
            ;; Function that adds two i32 numbers
            (func $add (export "add") (param $a i32) (param $b i32) (result i32)
                local.get $a
                local.get $b
                i32.add
            )
            
            ;; Function that multiplies two numbers
            (func $multiply (export "multiply") (param $a i32) (param $b i32) (result i32)
                local.get $a
                local.get $b
                i32.mul
            )
        )
    "#;
    
    // Simple add function compiled from WAT
    // (module (func $add (export "add") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))
    let wasm_bytes: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,  // WASM header
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01,  // Type section
        0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,  // Function & Export sections
        0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09,  // Export name "add"
        0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a,  // Code: local.get 0, local.get 1, i32.add
        0x0b                                              // End
    ];
    
    println!("1. Creating Kiln Engine...");
    let mut engine = Engine::new()?;
    
    println!("2. Loading WASM module ({} bytes)...", wasm_bytes.len());
    let module = engine.load_module(wasm_bytes)?;
    
    println!("3. Instantiating module...");
    let instance = engine.instantiate(module)?;
    
    println!("4. Getting exported 'add' function...");
    let add_func = engine.get_function(instance, "add")?;
    
    println!("5. Executing add(5, 3)...");
    let args = vec![Value::I32(5), Value::I32(3)];
    let results = engine.invoke_function(add_func, &args)?;
    
    println!("\n✅ ACTUAL EXECUTION RESULT: {:?}", results);
    println!("   Expected: [I32(8)]");
    println!("   Got:      {:?}", results);
    
    // Verify the result
    if let Some(Value::I32(result)) = results.get(0) {
        if *result == 8 {
            println!("\n🎉 SUCCESS! The WASM function actually executed and returned the correct result!");
        } else {
            println!("\n❌ ERROR: Got unexpected result: {}", result);
        }
    } else {
        println!("\n❌ ERROR: No result returned");
    }
    
    Ok(())
}