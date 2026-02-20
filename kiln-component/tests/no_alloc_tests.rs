//! Test for the no_alloc module of wrt-component
//!
//! This file validates that the no_alloc module works correctly in all
//! environments, particularly in pure no_std without allocation.

// For testing in a no_std environment

// Binary std/no_std choice
#[cfg(test)]
mod no_alloc_tests {
    // Import from wrt-foundation that is available in all environments
    // Binary std/no_std choice
    use wrt_component::no_alloc::{
        COMPONENT_MAGIC, ComponentHeader, ComponentSectionId, ComponentSectionInfo,
        MinimalComponent, ValidationLevel, validate_component_no_alloc,
        validate_component_with_level,
    };
    use wrt_foundation::verification::VerificationLevel;

    // Constants for testing
    // Minimal valid WebAssembly Component - just magic number and version
    const MINIMAL_COMPONENT: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x0A, 0x00, 0x01, 0x00];

    #[test]
    fn test_section_id_conversion() {
        // Test conversion from u8 to ComponentSectionId
        assert_eq!(ComponentSectionId::from(0), ComponentSectionId::Custom);
        assert_eq!(
            ComponentSectionId::from(1),
            ComponentSectionId::ComponentType
        );
        assert_eq!(ComponentSectionId::from(2), ComponentSectionId::CoreModule);
        assert_eq!(ComponentSectionId::from(3), ComponentSectionId::Instance);
        assert_eq!(ComponentSectionId::from(4), ComponentSectionId::Component);
        assert_eq!(ComponentSectionId::from(255), ComponentSectionId::Unknown);
    }

    #[test]
    fn test_validation_levels() {
        // Test basic validation of a minimal component
        let basic_result =
            validate_component_with_level(&MINIMAL_COMPONENT, ValidationLevel::Basic);
        assert!(basic_result.is_ok());

        // Test standard validation of a minimal component
        let standard_result =
            validate_component_with_level(&MINIMAL_COMPONENT, ValidationLevel::Standard);
        assert!(standard_result.is_ok());

        // Test full validation of a minimal component
        let full_result = validate_component_with_level(&MINIMAL_COMPONENT, ValidationLevel::Full);
        assert!(full_result.is_ok());
    }

    #[test]
    fn test_minimal_component() {
        // Create a minimal component with standard verification level
        let component = MinimalComponent::new(&MINIMAL_COMPONENT, VerificationLevel::Standard);
        assert!(component.is_ok());

        // Check properties of the minimal component
        let component = component.unwrap();
        assert_eq!(component.size(), 8);
        assert_eq!(component.export_count(), 0);
        assert_eq!(component.import_count(), 0);
        assert_eq!(component.module_count(), 0);
        assert!(!component.has_start());
    }

    #[test]
    fn test_component_validation() {
        // Test validation of a minimal component
        let result = validate_component_no_alloc(&MINIMAL_COMPONENT);
        assert!(result.is_ok());

        // Invalid component with incorrect magic number
        let invalid_component = [0x01, 0x61, 0x73, 0x6D, 0x0A, 0x00, 0x01, 0x00];
        let result = validate_component_no_alloc(&invalid_component);
        assert!(result.is_err());

        // Component that's too small
        let too_small = [0x00, 0x61];
        let result = validate_component_no_alloc(&too_small);
        assert!(result.is_err());
    }

    #[test]
    fn test_component_header_defaults() {
        // Create a default ComponentHeader
        let header = ComponentHeader::default();

        // Check properties
        assert_eq!(header.size, 0);
        assert_eq!(header.module_count, 0);
        assert_eq!(header.export_count, 0);
        assert_eq!(header.import_count, 0);
        assert!(!header.has_start);

        // All sections should be None
        for section in &header.sections {
            assert!(section.is_none());
        }
    }

    #[test]
    fn test_section_info() {
        // Create a section info
        let section_info = ComponentSectionInfo {
            id: ComponentSectionId::Export,
            size: 100,
            offset: 200,
        };

        // Check properties
        assert_eq!(section_info.id, ComponentSectionId::Export);
        assert_eq!(section_info.size, 100);
        assert_eq!(section_info.offset, 200);
    }
}
