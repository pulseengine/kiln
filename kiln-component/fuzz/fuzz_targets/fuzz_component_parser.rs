#![no_main]

use libfuzzer_sys::fuzz_target;
use wrt_component::{
    parser,
    parser_integration::{ComponentLoader, ValidationLevel},
    ComponentInstanceId,
};

fuzz_target!(|data: &[u8]| {
    // Test raw component parsing
    let _ = parser::parse_component(data;
    
    // Test getting required builtins
    let _ = parser::get_required_builtins(data;
    
    // Test component loader with different validation levels
    let mut loader = ComponentLoader::new();
    
    // Try loading with no validation
    loader.set_validation_level(ValidationLevel::None;
    let _ = loader.load_from_bytes(data;
    
    // Try loading with basic validation
    loader.set_validation_level(ValidationLevel::Basic;
    let _ = loader.load_from_bytes(data;
    
    // Try loading with full validation
    loader.set_validation_level(ValidationLevel::Full;
    let _ = loader.load_from_bytes(data;
    
    // Test specific byte patterns that might trigger edge cases
    if data.len() >= 8 {
        // Check for WASM magic number
        if &data[0..4] == b"\0asm" {
            // This looks like it might be a WASM module
            let _ = loader.load_from_bytes(data;
        }
    }
    
    // Test empty and very small inputs
    if data.is_empty() {
        let _ = parser::parse_component(data;
    }
    
    if data.len() == 1 {
        let _ = parser::parse_component(data;
    }
};