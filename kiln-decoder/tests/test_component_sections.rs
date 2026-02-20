// Test for component section parsing
use std::fs;

#[test]
fn test_parse_file_ops_component() {
    let bytes = fs::read("../wrt-tests/fixtures/file_ops_component.wasm")
        .expect("Failed to read component file");

    let component =
        wrt_decoder::component::decode_component_binary(&bytes).expect("Failed to parse component");

    // Print section counts for debugging
    println!("\n✓ Component parsed successfully");
    println!("\nSection counts:");
    println!("  - Types:          {}", component.types.len());
    println!("  - Core Modules:   {}", component.modules.len());
    println!("  - Core Instances: {}", component.core_instances.len());
    println!("  - Instances:      {}", component.instances.len());
    println!("  - Aliases:        {}", component.aliases.len());
    println!("  - Canonicals:     {}", component.canonicals.len());
    println!("  - Imports:        {}", component.imports.len());
    println!("  - Exports:        {}", component.exports.len());

    // Basic assertions - component should have some content
    assert!(
        component.types.len() > 0 || component.modules.len() > 0,
        "Component should have at least types or modules"
    );

    if !component.exports.is_empty() {
        println!("\nExports:");
        for export in &component.exports {
            println!(
                "  - {} (sort: {:?}, idx: {})",
                export.name.name, export.sort, export.idx
            );
        }
    }

    if !component.canonicals.is_empty() {
        println!("\nCanonical operations: {}", component.canonicals.len());
    }
}
