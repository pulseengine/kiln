#![cfg(feature = "std")]

#[cfg(test)]
mod tests {
    use kiln_decoder::component::decode_component;
    use kiln_error::Result;
    use kiln_format::component::FormatValType;

    #[test]
    fn test_decode_simple_component() -> Result<()> {
        // This is a mock test since we can't include actual binary data here
        // In a real test, we would decode an actual binary component

        // Create a mock component
        let component = create_mock_component()?;

        // Encode the component
        let binary = kiln_decoder::component::encode_component(&component)?;

        // Decode the binary
        let decoded_component = decode_component(&binary)?;

        // Verify the component structure
        assert_eq!(decoded_component.imports.len(), 1);
        assert_eq!(decoded_component.exports.len(), 1);

        Ok(())
    }

    fn create_mock_component() -> Result<kiln_format::component::Component> {
        let mut component = kiln_format::component::Component::new();

        // Add a simple function import
        component.imports.push(kiln_format::component::Import {
            name: kiln_format::component::ImportName {
                namespace: "env".to_string(),
                name: "import_func".to_string(),
                nested: Vec::new(),
                package: None,
            },
            ty: kiln_format::component::ExternType::Function {
                params: vec![("param".to_string(), FormatValType::S32)],
                results: vec![FormatValType::S32],
            },
        });

        // Add a simple function export
        component.exports.push(kiln_format::component::Export {
            name: kiln_format::component::ExportName {
                name: "export_func".to_string(),
                is_resource: false,
                semver: None,
                integrity: None,
                nested: Vec::new(),
            },
            sort: kiln_format::component::Sort::Function,
            idx: 0,
            ty: Some(kiln_format::component::ExternType::Function {
                params: vec![("param".to_string(), FormatValType::S32)],
                results: vec![FormatValType::S32],
            }),
        });

        Ok(component)
    }
}
