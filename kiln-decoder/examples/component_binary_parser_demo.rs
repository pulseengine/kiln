//! WebAssembly Component Binary Parser Demo
//!
//! This example demonstrates how to use the ComponentBinaryParser to parse
//! WebAssembly Component Model binaries with full cross-environment support.

use wrt_decoder::component::{
    ComponentBinaryParser, ValidationLevel, parse_component_binary,
    parse_component_binary_with_validation,
};
use wrt_error::Result;

/// Create a minimal valid component binary for demonstration
fn create_demo_component_binary() -> Vec<u8> {
    let mut binary = Vec::new();

    // Component header
    binary.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // Magic: "\0asm"
    binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Version: 1
    binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Layer: 1 (component)

    // Add a custom section with component name
    binary.push(0); // Custom section ID
    binary.push(12); // Section size

    // Custom section name
    binary.push(4); // Name length
    binary.extend_from_slice(b"name"); // Name: "name"

    // Component name subsection
    binary.push(0); // Subsection ID (component name)
    binary.push(6); // Subsection size
    binary.push(4); // Component name length
    binary.extend_from_slice(b"demo"); // Component name: "demo"

    binary
}

/// Demonstrate basic component parsing
fn demo_basic_parsing() -> Result<()> {
    println!("=== Basic Component Parsing Demo ===");

    let binary = create_demo_component_binary();
    println!("Created demo component binary ({} bytes)", binary.len());

    // Parse using convenience function
    let component = parse_component_binary(&binary)?;
    println!("✅ Successfully parsed component");
    println!("   Component name: {:?}", component.name);
    println!("   Modules: {}", component.modules.len());
    println!("   Types: {}", component.types.len());

    Ok(())
}

/// Demonstrate parsing with different validation levels
fn demo_validation_levels() -> Result<()> {
    println!("\n=== Validation Levels Demo ===");

    let binary = create_demo_component_binary();

    // Minimal validation
    let component1 = parse_component_binary_with_validation(&binary, ValidationLevel::Minimal)?;
    println!("✅ Minimal validation: Success");

    // Standard validation (default)
    let component2 = parse_component_binary_with_validation(&binary, ValidationLevel::Standard)?;
    println!("✅ Standard validation: Success");

    // Strict validation
    let component3 = parse_component_binary_with_validation(&binary, ValidationLevel::Full)?;
    println!("✅ Strict validation: Success");

    println!("   All validation levels accept the demo component");

    Ok(())
}

/// Demonstrate parser API usage
fn demo_parser_api() -> Result<()> {
    println!("\n=== Parser API Demo ===");

    let binary = create_demo_component_binary();

    // Create parser with custom validation level
    let mut parser = ComponentBinaryParser::with_validation_level(ValidationLevel::Full);
    println!("Created parser with strict validation");

    // Parse the component
    let component = parser.parse(&binary)?;
    println!("✅ Parser API: Success");
    println!("   Component parsed successfully");

    Ok(())
}

/// Demonstrate error handling
fn demo_error_handling() {
    println!("\n=== Error Handling Demo ===");

    // Test empty binary
    match parse_component_binary(&[]) {
        Ok(_) => println!("❌ Unexpected success with empty binary"),
        Err(e) => println!("✅ Empty binary error: {}", e.message()),
    }

    // Test invalid magic
    let invalid_magic = vec![
        0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    match parse_component_binary(&invalid_magic) {
        Ok(_) => println!("❌ Unexpected success with invalid magic"),
        Err(e) => println!("✅ Invalid magic error: {}", e.message()),
    }

    // Test too small binary
    let too_small = vec![0x00, 0x61, 0x73]; // Only 3 bytes
    match parse_component_binary(&too_small) {
        Ok(_) => println!("❌ Unexpected success with too small binary"),
        Err(e) => println!("✅ Too small binary error: {}", e.message()),
    }
}

/// Demonstrate cross-environment compatibility
fn demo_cross_environment_compatibility() -> Result<()> {
    println!("\n=== Cross-Environment Compatibility Demo ===");

    let binary = create_demo_component_binary();

    // Binary std/no_std choice
    let component = parse_component_binary(&binary)?;

    #[cfg(feature = "std")]
    println!("✅ Running in std environment");

    #[cfg(all(not(feature = "std")))]
    println!("✅ Running in no_std+alloc environment");

    #[cfg(not(any(feature = "std",)))]
    println!("✅ Running in pure no_std environment");

    println!("   Component parsing successful in current environment");

    Ok(())
}

fn main() -> Result<()> {
    println!("WebAssembly Component Binary Parser Demo");
    println!("=========================================");

    demo_basic_parsing()?;
    demo_validation_levels()?;
    demo_parser_api()?;
    demo_error_handling();
    demo_cross_environment_compatibility()?;

    println!("\n=== Demo Complete ===");
    println!("✅ All component parsing demonstrations completed successfully!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_component_binary_creation() {
        let binary = create_demo_component_binary();
        assert!(binary.len() >= 12); // At least header size
        assert_eq!(&binary[0..4], &[0x00, 0x61, 0x73, 0x6D]); // Magic
    }

    #[test]
    fn test_demo_functions() {
        // Test that all demo functions work without panicking
        assert!(demo_basic_parsing().is_ok());
        assert!(demo_validation_levels().is_ok());
        assert!(demo_parser_api().is_ok());
        assert!(demo_cross_environment_compatibility().is_ok());

        // Error handling demo doesn't return Result, so just call it
        demo_error_handling();
    }
}
