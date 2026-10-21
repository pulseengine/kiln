// Simple test to verify memory optimizations work

#[cfg(feature = "std")]
#[test]
fn test_memory_optimized_parsing_std() {
    use kiln_decoder::decoder::decode_module;

    // Minimal valid WASM module
    let wasm_bytes = [
        0x00, 0x61, 0x73, 0x6D, // magic
        0x01, 0x00, 0x00, 0x00, // version
    ];

    let result = decode_module(&wasm_bytes);

    // Should parse without error (even if empty)
    assert!(
        result.is_ok(),
        "Failed to parse minimal WASM module: {:?}",
        result
    );
}
