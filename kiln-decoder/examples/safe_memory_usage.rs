//! Example demonstrating safe memory usage in wrt-decoder

use wrt_decoder::{SafeSlice, module::decode_module};
use wrt_error::Result;
use wrt_foundation::{safe_memory::SafeMemoryHandler, verification::VerificationLevel};

// Sample minimal WebAssembly module (empty module)
const MINIMAL_WASM: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

fn main() -> Result<()> {
    println!("=== Safe Memory Usage Example ===");

    // Create a safe memory handler for the WASM binary
    let mut handler = SafeMemoryHandler::new(MINIMAL_WASM.to_vec());

    // Set the verification level to FULL for maximum safety
    handler.set_verification_level(VerificationLevel::Full);

    // Get a safe slice from the memory handler
    let slice = handler.get_slice(0, handler.size())?;

    // Verify the integrity of the memory
    handler.verify_integrity()?;
    println!("Memory integrity verified!");

    // Decode the module using the safe slice
    let module = decode_module_safe(&slice)?;

    // Display module information
    println!("Decoded WebAssembly module (version {})", module.version);
    println!("Types: {}", module.types.len());
    println!("Imports: {}", module.imports.len());
    println!("Functions: {}", module.functions.len());
    println!("Exports: {}", module.exports.len());

    // Get memory statistics
    let stats = handler.memory_stats();
    println!("\nMemory Statistics:");
    println!("Total size: {} bytes", stats.total_size);
    println!("Access count: {}", stats.access_count);
    println!("Unique regions: {}", stats.unique_regions);
    println!("Max access size: {} bytes", stats.max_access_size);

    println!("\nSafe memory usage completed successfully!");
    Ok(())
}

/// Decode a WebAssembly module using a SafeSlice
fn decode_module_safe(slice: &SafeSlice) -> Result<wrt_decoder::module::Module> {
    // Get the raw data from the safe slice (performs integrity check)
    let data = slice.data()?;

    // Decode the module from the raw data
    decode_module(data)
}
