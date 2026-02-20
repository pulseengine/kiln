//! Comprehensive tests for ComponentBinaryParser
//!
//! This module provides extensive test coverage for the WebAssembly Component
//! Model binary parser, including edge cases, error conditions, and
//! cross-environment compatibility.

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::format;
    use alloc::vec;
    use alloc::vec::Vec;

    use wrt_error::ErrorCategory;

    use super::super::binary_parser::*;

    // Test data generators for creating valid component binaries

    /// Create a minimal valid component binary for testing
    fn create_minimal_component_binary() -> Vec<u8> {
        let mut binary = Vec::new();

        // Add component magic
        binary.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // Component magic

        // Add version (1 in little-endian)
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Version 1

        // Add layer (1 in little-endian)
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Layer 1

        binary
    }

    /// Create a component binary with a custom section
    fn create_component_with_custom_section() -> Vec<u8> {
        let mut binary = create_minimal_component_binary();

        // Add custom section
        binary.push(0); // Custom section ID
        binary.push(5); // Section size (5 bytes)

        // Custom section name length and name
        binary.push(4); // Name length
        binary.extend_from_slice(b"test"); // Name

        binary
    }

    /// Create a component binary with invalid magic
    fn create_invalid_magic_binary() -> Vec<u8> {
        let mut binary = Vec::new();

        // Add invalid magic
        binary.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Invalid magic
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Version 1
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Layer 1

        binary
    }

    /// Create a component binary with invalid version
    fn create_invalid_version_binary() -> Vec<u8> {
        let mut binary = Vec::new();

        binary.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // Valid magic
        binary.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Invalid version
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Layer 1

        binary
    }

    /// Create a component binary with invalid layer
    fn create_invalid_layer_binary() -> Vec<u8> {
        let mut binary = Vec::new();

        binary.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // Valid magic
        binary.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Version 1
        binary.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // Invalid layer (0)

        binary
    }

    // Basic parser functionality tests

    #[test]
    fn test_parse_minimal_valid_component() {
        let binary = create_minimal_component_binary();
        let mut parser = ComponentBinaryParser::new();

        let result = parser.parse(&binary);
        assert!(result.is_ok());

        let component = result.unwrap();
        assert!(component.name.is_none());
        assert!(component.modules.is_empty());
        assert!(component.types.is_empty());
    }

    #[test]
    fn test_parse_component_with_custom_section() {
        let binary = create_component_with_custom_section();
        let mut parser = ComponentBinaryParser::new();

        let result = parser.parse(&binary);
        assert!(result.is_ok());

        // Custom sections should be parsed but ignored in basic implementation
        let _component = result.unwrap();
    }

    // Error condition tests

    #[test]
    fn test_parse_empty_binary() {
        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&[]);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.category(), ErrorCategory::Parse);
    }

    #[test]
    fn test_parse_too_small_binary() {
        let mut parser = ComponentBinaryParser::new();

        // Binary smaller than minimum header size (12 bytes)
        let small_binary = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00];
        let result = parser.parse(&small_binary);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.category(), ErrorCategory::Parse);
    }

    #[test]
    fn test_parse_invalid_magic() {
        let binary = create_invalid_magic_binary();
        let mut parser = ComponentBinaryParser::new();

        let result = parser.parse(&binary);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.category(), ErrorCategory::Parse);
        assert!(error.message().contains("magic"));
    }

    #[test]
    fn test_parse_invalid_version() {
        let binary = create_invalid_version_binary();
        let mut parser = ComponentBinaryParser::new();

        let result = parser.parse(&binary);

        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.category(), ErrorCategory::Parse);
        assert!(error.message().contains("version"));
    }

    #[test]
    fn test_parse_invalid_layer() {
        let binary = create_invalid_layer_binary();
        let mut parser = ComponentBinaryParser::new();

        let result = parser.parse(&binary);

        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.category(), ErrorCategory::Parse);
        assert!(error.message().contains("layer"));
    }

    // Validation level tests

    #[test]
    fn test_validation_levels() {
        let binary = create_minimal_component_binary();

        // Test basic validation
        let mut basic_parser = ComponentBinaryParser::with_validation_level(ValidationLevel::Basic);
        let result1 = basic_parser.parse(&binary);
        assert!(result1.is_ok());

        // Test standard validation
        let mut standard_parser =
            ComponentBinaryParser::with_validation_level(ValidationLevel::Standard);
        let result2 = standard_parser.parse(&binary);
        assert!(result2.is_ok());

        // Test strict validation
        let mut strict_parser = ComponentBinaryParser::with_validation_level(ValidationLevel::Full);
        let result3 = strict_parser.parse(&binary);
        assert!(result3.is_ok());
    }

    // Convenience function tests

    #[test]
    fn test_convenience_functions() {
        let binary = create_minimal_component_binary();

        // Test basic parsing function
        let result1 = parse_component_binary(&binary);
        assert!(result1.is_ok());

        // Test parsing with different validation levels
        let result2 = parse_component_binary_with_validation(&binary, ValidationLevel::Basic);
        assert!(result2.is_ok());

        let result3 = parse_component_binary_with_validation(&binary, ValidationLevel::Standard);
        assert!(result3.is_ok());

        let result4 = parse_component_binary_with_validation(&binary, ValidationLevel::Full);
        assert!(result4.is_ok());
    }

    // Section ID tests

    #[test]
    fn test_component_section_id_conversions() {
        // Test valid section IDs
        assert_eq!(
            ComponentSectionId::from_u8(0),
            Some(ComponentSectionId::Custom)
        );
        assert_eq!(
            ComponentSectionId::from_u8(1),
            Some(ComponentSectionId::CoreModule)
        );
        assert_eq!(
            ComponentSectionId::from_u8(2),
            Some(ComponentSectionId::CoreInstance)
        );
        assert_eq!(
            ComponentSectionId::from_u8(3),
            Some(ComponentSectionId::CoreType)
        );
        assert_eq!(
            ComponentSectionId::from_u8(4),
            Some(ComponentSectionId::Component)
        );
        assert_eq!(
            ComponentSectionId::from_u8(5),
            Some(ComponentSectionId::Instance)
        );
        assert_eq!(
            ComponentSectionId::from_u8(6),
            Some(ComponentSectionId::Alias)
        );
        assert_eq!(
            ComponentSectionId::from_u8(7),
            Some(ComponentSectionId::Type)
        );
        assert_eq!(
            ComponentSectionId::from_u8(8),
            Some(ComponentSectionId::Canon)
        );
        assert_eq!(
            ComponentSectionId::from_u8(9),
            Some(ComponentSectionId::Start)
        );
        assert_eq!(
            ComponentSectionId::from_u8(10),
            Some(ComponentSectionId::Import)
        );
        assert_eq!(
            ComponentSectionId::from_u8(11),
            Some(ComponentSectionId::Export)
        );
        assert_eq!(
            ComponentSectionId::from_u8(12),
            Some(ComponentSectionId::Value)
        );

        // Test invalid section IDs
        assert_eq!(ComponentSectionId::from_u8(13), None);
        assert_eq!(ComponentSectionId::from_u8(255), None);
    }

    #[test]
    fn test_component_section_names() {
        assert_eq!(ComponentSectionId::Custom.name(), "custom");
        assert_eq!(ComponentSectionId::CoreModule.name(), "core-module");
        assert_eq!(ComponentSectionId::CoreInstance.name(), "core-instance");
        assert_eq!(ComponentSectionId::CoreType.name(), "core-type");
        assert_eq!(ComponentSectionId::Component.name(), "component");
        assert_eq!(ComponentSectionId::Instance.name(), "instance");
        assert_eq!(ComponentSectionId::Alias.name(), "alias");
        assert_eq!(ComponentSectionId::Type.name(), "type");
        assert_eq!(ComponentSectionId::Canon.name(), "canon");
        assert_eq!(ComponentSectionId::Start.name(), "start");
        assert_eq!(ComponentSectionId::Import.name(), "import");
        assert_eq!(ComponentSectionId::Export.name(), "export");
        assert_eq!(ComponentSectionId::Value.name(), "value");
    }

    #[test]
    fn test_component_section_display() {
        let section = ComponentSectionId::Custom;
        assert_eq!(format!("{}", section), "custom");

        let section = ComponentSectionId::CoreModule;
        assert_eq!(format!("{}", section), "core-module");
    }

    // Header validation tests

    #[test]
    fn test_component_header_validation() {
        // Valid header
        let valid_header = ComponentHeader {
            magic: [0x00, 0x61, 0x73, 0x6D],
            version: 1,
            layer: 1,
        };
        assert!(valid_header.validate().is_ok());

        // Invalid magic
        let invalid_magic_header = ComponentHeader {
            magic: [0xFF, 0xFF, 0xFF, 0xFF],
            version: 1,
            layer: 1,
        };
        assert!(invalid_magic_header.validate().is_err());

        // Invalid version
        let invalid_version_header = ComponentHeader {
            magic: [0x00, 0x61, 0x73, 0x6D],
            version: 999,
            layer: 1,
        };
        assert!(invalid_version_header.validate().is_err());

        // Invalid layer
        let invalid_layer_header = ComponentHeader {
            magic: [0x00, 0x61, 0x73, 0x6D],
            version: 1,
            layer: 0, // Should be 1 for components
        };
        assert!(invalid_layer_header.validate().is_err());
    }

    // Edge case tests

    #[test]
    fn test_parse_component_with_unknown_section() {
        let mut binary = create_minimal_component_binary();

        // Add an unknown section (ID 255)
        binary.push(255); // Unknown section ID
        binary.push(0); // Empty section

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);

        // Should succeed but ignore unknown section
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_component_with_oversized_section() {
        let mut binary = create_minimal_component_binary();

        // Add a section with size larger than remaining data
        binary.push(0); // Custom section ID
        binary.push(100); // Large section size (but only few bytes follow)
        binary.push(1); // Only 1 byte of data

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);

        // Should fail due to oversized section
        assert!(result.is_err());
    }

    // Cross-environment compatibility tests

    #[cfg(feature = "std")]
    #[test]
    fn test_std_compatibility() {
        let binary = create_minimal_component_binary();
        let result = parse_component_binary(&binary);
        assert!(result.is_ok());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_alloc_compatibility() {
        let binary = create_minimal_component_binary();
        let result = parse_component_binary(&binary);
        assert!(result.is_ok());
    }

    #[cfg(not(any(feature = "std",)))]
    #[test]
    fn test_no_std_compatibility() {
        let binary = create_minimal_component_binary();
        let result = parse_component_binary(&binary);
        assert!(result.is_ok());
    }

    // Performance and memory safety tests

    #[test]
    fn test_large_binary_handling() {
        // Create a component with a reasonably large custom section
        let mut binary = create_minimal_component_binary();

        // Add a custom section with some data
        binary.push(0); // Custom section ID
        binary.push(10); // Section size
        binary.push(4); // Name length
        binary.extend_from_slice(b"test"); // Name
        binary.extend_from_slice(&[0; 5]);

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_sections() {
        let mut binary = create_minimal_component_binary();

        // Add multiple custom sections
        for i in 0..3 {
            binary.push(0); // Custom section ID
            binary.push(6); // Section size
            binary.push(1); // Name length
            binary.push(b'a' + i); // Name
            binary.extend_from_slice(&[0; 4]);
        }

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);
        assert!(result.is_ok());
    }

    // Regression tests for potential issues

    #[test]
    fn test_zero_size_section() {
        let mut binary = create_minimal_component_binary();

        // Add a section with zero size
        binary.push(0); // Custom section ID
        binary.push(0); // Zero section size

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exact_boundary_conditions() {
        // Test binary that ends exactly at the end of a section
        let mut binary = create_minimal_component_binary();

        binary.push(0); // Custom section ID
        binary.push(1); // Section size
        binary.push(0); // One byte of data

        let mut parser = ComponentBinaryParser::new();
        let result = parser.parse(&binary);
        assert!(result.is_ok());
    }
}
