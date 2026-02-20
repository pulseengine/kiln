//! Debug version of bounded WIT parser to understand parsing issues

use wrt_format::wit_parser_bounded::BoundedWitParser;
use wrt_foundation::NoStdProvider;

fn main() -> Result<(), wrt_error::Error> {
    println!("=== Debug Bounded WIT Parser ===\n");

    // Simple test case
    let input = "world test-world {}";
    println!("Testing input: '{}'", input);
    println!("Input length: {}", input.len());
    println!("Input bytes: {:?}", input.as_bytes());
    println!();

    let mut parser = BoundedWitParser::<NoStdProvider<4096>>::new(NoStdProvider::default())?;

    // Manual debugging: check what gets stored in the buffer
    match parser.parse(input) {
        Ok(()) => {
            println!("✓ Parse completed successfully");
            println!("Worlds found: {}", parser.world_count);
            println!("Interfaces found: {}", parser.interface_count);

            for world in parser.worlds() {
                if let Ok(name) = world.name.as_str() {
                    println!("  World name: '{}'", name);
                }
            }
        },
        Err(e) => {
            println!("✗ Parse failed: {:?}", e);
        },
    }

    // Test an even simpler case
    println!("\nTesting very simple input:");
    let simple = "world test";
    println!("Input: '{}'", simple);

    let mut simple_parser = BoundedWitParser::<NoStdProvider<4096>>::new(NoStdProvider::default())?;
    match simple_parser.parse(simple) {
        Ok(()) => {
            println!("✓ Simple parse completed");
            println!("Worlds found: {}", simple_parser.world_count);
        },
        Err(e) => {
            println!("✗ Simple parse failed: {:?}", e);
        },
    }

    Ok(())
}
