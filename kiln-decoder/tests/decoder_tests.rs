use wrt_error::{Error, ErrorCategory, codes};

// Custom Result type for our tests
type Result<T> = wrt_error::Result<T>;

// Helper function to convert wat::Error to wrt_error::Error
fn wat_to_wrt_error(e: wat::Error) -> Error {
    Error::runtime_execution_error(&format!("WAT parse error: {}", e))
}

#[test]
fn test_basic_module_decoding() -> Result<()> {
    // Create a basic WebAssembly module
    let wasm_bytes = wat::parse_str(
        r#"
        (module
          ;; Import a function
          (import "env" "imported_func" (func (param i32) (result i32)))
          
          ;; Define a memory
          (memory (export "memory") 1)
          
          ;; Define a global
          (global (export "global") (mut i32) (i32.const 42))
          
          ;; Define a function
          (func (export "add") (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.add
          )
          
          ;; Define a table
          (table (export "table") 10 funcref)
        )
        "#,
    )
    .map_err(wat_to_wrt_error)?;

    // Decode the module
    let module = wrt_decoder::wasm::decode(&wasm_bytes)?;

    // Verify that the module is properly decoded
    assert_eq!(module.functions.len(), 2); // One imported, one defined
    assert_eq!(module.imports.len(), 1);
    assert_eq!(module.exports.len(), 4); // memory, global, function, table
    assert_eq!(module.memories.len(), 1);
    assert_eq!(module.globals.len(), 1);
    assert_eq!(module.tables.len(), 1);

    // More detailed assertions could be added here

    Ok(())
}

#[test]
#[ignore] // Temporarily ignore this test
fn test_complex_module_decoding() -> Result<()> {
    // Create a more complex WebAssembly module
    let wasm_bytes = wat::parse_str(
        r#"
        (module
          ;; Import functions, memory, and global
          (import "env" "memory" (memory 1))
          (import "env" "global" (global $g (mut i32)))
          (import "env" "log" (func $log (param i32)))
          
          ;; Define types
          (type $add_type (func (param i32 i32) (result i32)))
          
          ;; Define functions
          (func $add (type $add_type)
            local.get 0
            local.get 1
            i32.add
          )
          
          (func $mul (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.mul
          )
          
          ;; Export functions
          (export "add" (func $add))
          (export "mul" (func $mul))
          
          ;; Start function
          (start $log)
          
          ;; Data section
          (data (i32.const 0) "Hello, WebAssembly!")
        )
        "#,
    )
    .map_err(wat_to_wrt_error)?;

    // Decode the module
    let module = wrt_decoder::wasm::decode(&wasm_bytes)?;

    // Verify module structure
    assert_eq!(module.functions.len(), 3); // 1 imported, 2 defined
    assert_eq!(module.types.len(), 1);
    assert_eq!(module.imports.len(), 3); // memory, global, function
    assert_eq!(module.exports.len(), 2); // add, mul
    assert_eq!(module.memories.len(), 1);
    assert_eq!(module.globals.len(), 1);
    assert_eq!(module.data.len(), 1);
    assert!(module.start.is_some());

    Ok(())
}

#[test]
#[ignore] // Temporarily ignore this test
fn test_invalid_module() {
    // Create an invalid WebAssembly module (truncated magic bytes)
    let invalid_bytes = vec![0x00, 0x61, 0x73];

    // Attempt to decode, should return an error
    let result = wrt_decoder::wasm::decode(&invalid_bytes);
    assert!(result.is_err());

    // Test with truncated module
    let truncated = vec![
        // Magic bytes + version
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, // Truncated type section
        0x01, 0x05, 0x01, 0x60,
    ];
    let result = wrt_decoder::wasm::decode(&truncated);
    assert!(result.is_err());
}

#[test]
#[ignore] // Temporarily ignore this test
fn test_module_with_complex_instructions() -> Result<()> {
    // Module with more complex instructions
    let wasm_bytes = wat::parse_str(
        r#"
        (module
          (memory 1)
          (func (export "complex") (param i32 i32) (result i32)
            (local i32 i64 f32 f64)
            
            ;; Block, loop, if structures
            block
              local.get 0
              i32.eqz
              br_if 0
              
              loop
                local.get 0
                i32.const 1
                i32.sub
                local.set 0
                
                local.get 0
                i32.eqz
                br_if 1
                br 0
              end
            end
            
            ;; Numeric operations
            local.get 0
            local.get 1
            i32.add
            
            ;; Return result
            return
          )
        )
        "#,
    )
    .map_err(wat_to_wrt_error)?;

    // Decode the module
    let module = wrt_decoder::wasm::decode(&wasm_bytes)?;

    // Basic structure checks
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.memories.len(), 1);

    Ok(())
}
