//! Integration tests for the wrt-format crate.
//!
//! This module contains tests for the format module functionality.

use wrt_format::{
    CustomSection,
    Module,
};

/// Test basic module and custom section functionality
#[test]
#[cfg(feature = "std")]
fn test_custom_sections() {
    // Create a simple module
    let mut module = Module::new();

    // Verify initial state
    assert!(module.custom_sections.is_empty());

    // Create a custom section
    let section1 = CustomSection {
        name: "test-section".to_string(),
        data: vec![1, 2, 3, 4],
    };

    module.add_custom_section(section1);

    // Verify section was added
    assert_eq!(module.custom_sections.len(), 1);

    // Find the custom section
    let found = module.find_custom_section("test-section");
    assert!(found.is_some());
    assert_eq!(found.unwrap().data, vec![1, 2, 3, 4]);

    // Add another section
    let section2 = CustomSection {
        name: "another-section".to_string(),
        data: vec![5, 6, 7, 8],
    };
    module.add_custom_section(section2);

    // Verify both sections exist
    assert_eq!(module.custom_sections.len(), 2);
    assert!(module.find_custom_section("test-section").is_some());
    assert!(module.find_custom_section("another-section").is_some());
}
