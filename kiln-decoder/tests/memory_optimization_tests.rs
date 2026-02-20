// Simple test to verify memory optimizations work

#[cfg(feature = "std")]
#[test]
fn test_memory_optimized_parsing() {
    use wrt_decoder::optimized_module::decode_module_with_provider;
    use wrt_foundation::NoStdProvider;

    // Minimal valid WASM module
    let wasm_bytes = [
        0x00, 0x61, 0x73, 0x6D, // magic
        0x01, 0x00, 0x00, 0x00, // version
    ];

    let provider = NoStdProvider::<1024>::default();
    let result = decode_module_with_provider(&wasm_bytes, provider);

    // Should parse without error (even if empty)
    assert!(
        result.is_ok(),
        "Failed to parse minimal WASM module: {:?}",
        result
    );
}

#[cfg(feature = "std")]
#[test]
fn test_memory_optimized_parsing_std() {
    use wrt_decoder::from_binary;

    // Minimal valid WASM module
    let wasm_bytes = [
        0x00, 0x61, 0x73, 0x6D, // magic
        0x01, 0x00, 0x00, 0x00, // version
    ];

    let result = from_binary(&wasm_bytes);

    // Should parse without error (even if empty)
    assert!(
        result.is_ok(),
        "Failed to parse minimal WASM module: {:?}",
        result
    );
}
