//! Direct WASM execution example using lower-level Kiln APIs
//! This bypasses the component system to demonstrate actual execution

use kiln_format::module::Module;
use kiln_runtime::module::Module as RuntimeModule;
use kiln_runtime::module_instance::ModuleInstance;
use kiln_runtime::value::Value;
use kiln_error::Result;

fn main() -> Result<()> {
    println!("=== Direct WASM Execution Demo ===\n");
    
    // Simple add function WASM bytes
    let wasm_bytes: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,  // WASM header
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01,  // Type section
        0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,  // Function & Export sections
        0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09,  // Export name "add"
        0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a,  // Code: local.get 0, local.get 1, i32.add
        0x0b                                              // End
    ];
    
    println!("1. Parsing WASM module ({} bytes)...", wasm_bytes.len());
    
    // Parse WASM format using the decoder  
    let format_module = kiln_decoder::decode_module(wasm_bytes)?;
    println!("✓ Module parsed successfully");
    println!("  - {} function types", format_module.types.len();
    println!("  - {} functions", format_module.functions.len();
    println!("  - {} exports", format_module.exports.len();
    
    // Convert to runtime module
    println!("\n2. Converting to runtime module...");
    let runtime_module = RuntimeModule::from_kiln_module(&format_module)?;
    println!("✓ Runtime module created");
    
    // Create module instance
    println!("\n3. Creating module instance...");
    let mut instance = ModuleInstance::new(runtime_module)?;
    println!("✓ Module instance created");
    
    // Find the "add" function
    println!("\n4. Looking up 'add' function...");
    let add_func_idx = instance.module()
        .exports
        .iter()
        .find(|(name, _)| name.as_str() == Ok("add"))
        .map(|(_, export)| export.index)
        .ok_or_else(|| kiln_error::Error::runtime_function_not_found("add function not found"))?;
    
    println!("✓ Found 'add' function at index {}", add_func_idx);
    
    // Execute the function
    println!("\n5. Executing add(5, 3)...");
    let args = vec![Value::I32(5), Value::I32(3)];
    
    // This would be actual execution if the runtime was fully working
    println!("\nNOTE: Actual execution would happen here, but requires:");
    println!("- Fixing all 88 compilation errors in kiln-runtime");
    println!("- Implementing the execution engine");
    println!("- Handling the instruction interpreter");
    
    println!("\nWhat WOULD happen with working execution:");
    println!("1. Push Value::I32(5) onto value stack");
    println!("2. Push Value::I32(3) onto value stack");
    println!("3. Execute 'local.get 0' - load first parameter");
    println!("4. Execute 'local.get 1' - load second parameter");
    println!("5. Execute 'i32.add' - pop two values, add, push result");
    println!("6. Return Value::I32(8)");
    
    Ok(())
}