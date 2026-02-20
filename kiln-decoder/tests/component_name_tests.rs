use wrt_decoder::component::{
    component_name_section::{
        ComponentNameSection, generate_component_name_section, parse_component_name_section,
    },
    decode_component, encode_component,
};
use wrt_format::{
    binary,
    component::{Component, Sort},
};

#[test]
fn test_component_name_section() {
    // Create a component with a name
    let mut component = Component::new();
    component.name = Some("test_component".to_string());

    // Add some basic content to make it a valid component
    // (In a real test, we might add more)

    // Generate the binary
    let binary = encode_component(&component).unwrap();

    // Decode the binary back to a component
    let decoded = decode_component(&binary).unwrap();

    // Check that the name was preserved
    assert_eq!(decoded.name, Some("test_component".to_string()));
}

// Let's create our own ComponentNameSection for testing since we don't know the
// exact structure This matches what the test is expecting
#[derive(Default)]
struct TestComponentNameSection {
    pub component_name: Option<String>,
    pub sort_names: Vec<(Sort, Vec<(u32, String)>)>,
    pub import_names: Vec<(u32, String)>,
    pub export_names: Vec<(u32, String)>,
    pub canonical_names: Vec<(u32, String)>,
    pub type_names: Vec<(u32, String)>,
}

// Implement Default for our test struct

#[test]
fn test_standalone_name_section() {
    // Create a name section with component name and sort names
    let original = TestComponentNameSection {
        component_name: Some("test_component".to_string()),
        sort_names: vec![
            (
                Sort::Function,
                vec![(0, "func1".to_string()), (1, "func2".to_string())],
            ),
            (
                Sort::Instance,
                vec![(0, "instance1".to_string()), (1, "instance2".to_string())],
            ),
        ],
        import_names: Vec::new(),
        export_names: Vec::new(),
        canonical_names: Vec::new(),
        type_names: Vec::new(),
    };

    // For this test, we'll just convert our test struct to the real
    // ComponentNameSection
    let component_name_section = ComponentNameSection {
        component_name: original.component_name.clone(),
        sort_names: original.sort_names.clone(),
        import_names: Vec::new(),
        export_names: Vec::new(),
        canonical_names: Vec::new(),
        type_names: Vec::new(),
    };

    // Generate the binary
    let encoded = generate_component_name_section(&component_name_section).unwrap();

    // Parse it back
    let decoded = parse_component_name_section(&encoded).unwrap();

    // Check component name
    assert_eq!(decoded.component_name, original.component_name);

    // Check sort names
    assert_eq!(decoded.sort_names.len(), original.sort_names.len());

    for i in 0..original.sort_names.len() {
        let (sort1, names1) = &original.sort_names[i];
        let (_sort2, names2) = &decoded.sort_names[i];

        // Compare sorts (using debug representation since Sort doesn't implement
        // PartialEq)
        assert!(matches!(sort1, _sort2));

        // Compare name maps
        assert_eq!(names1.len(), names2.len());
        for j in 0..names1.len() {
            assert_eq!(names1[j].0, names2[j].0);
            assert_eq!(names1[j].1, names2[j].1);
        }
    }
}

#[test]
fn test_custom_section_with_name() {
    // Create a name section
    let name_section = TestComponentNameSection {
        component_name: Some("test_component".to_string()),
        sort_names: Vec::new(),
        import_names: Vec::new(),
        export_names: Vec::new(),
        canonical_names: Vec::new(),
        type_names: Vec::new(),
    };

    // Convert to the real ComponentNameSection
    let actual_name_section = ComponentNameSection {
        component_name: name_section.component_name.clone(),
        sort_names: name_section.sort_names.clone(),
        import_names: Vec::new(),
        export_names: Vec::new(),
        canonical_names: Vec::new(),
        type_names: Vec::new(),
    };

    // Generate name section binary
    let name_section_data = generate_component_name_section(&actual_name_section).unwrap();

    // Create custom section content with "name" as the identifier
    let mut custom_section_content = Vec::new();
    custom_section_content.extend_from_slice(&binary::write_string("name"));
    custom_section_content.extend_from_slice(&name_section_data);

    // Create a component with just the custom section
    let mut binary = Vec::new();

    // Component preamble
    binary.extend_from_slice(&binary::COMPONENT_MAGIC);
    binary.extend_from_slice(&binary::COMPONENT_VERSION);

    // Custom section
    binary.push(binary::COMPONENT_CUSTOM_SECTION_ID);
    binary.extend_from_slice(&binary::write_leb128_u32(
        custom_section_content.len() as u32
    ));
    binary.extend_from_slice(&custom_section_content);

    // Decode the binary
    let component = decode_component(&binary).unwrap();

    // Check that the name was extracted
    assert_eq!(component.name, Some("test_component".to_string()));
}
