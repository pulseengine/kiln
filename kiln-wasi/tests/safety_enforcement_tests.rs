//! Test that safety features are properly enforced in wrt-wasi
//!
//! This test demonstrates that the safety-aware allocation macros
//! properly enforce allocation limits based on configured safety level.

use wrt_foundation::{
    memory_init::{
        init_crate_memory,
        init_wrt_memory,
    },
    safe_managed_alloc,
    CrateId,
};
use wrt_wasi::{
    wasi_max_allocation_size,
    wasi_safety_level,
    WasiHostProvider,
    WasiProviderBuilder,
    WASI_CRATE_ID,
};

#[test]
fn test_safety_level_detection() {
    // Test that we can detect the current safety level
    let level = wasi_safety_level();
    println!("Current safety level: {}", level);

    // Verify it's one of the expected values
    assert!(matches!(
        level,
        "dynamic-allocation" | "bounded-collections" | "static-memory-safety" | "maximum-safety"
    ));
}

#[test]
fn test_allocation_size_limits() {
    let max_size = wasi_max_allocation_size();
    println!("Maximum allocation size: {} bytes", max_size);

    // Verify size based on safety level
    match wasi_safety_level() {
        "maximum-safety" => assert_eq!(max_size, 16384), // 16KB
        "static-memory-safety" => assert_eq!(max_size, 32768), // 32KB
        "bounded-collections" => assert_eq!(max_size, 65536), // 64KB
        "dynamic-allocation" => assert_eq!(max_size, usize::MAX), // No limit
        _ => panic!("Unknown safety level"),
    }
}

#[test]
fn test_safety_aware_allocation_enforcement() {
    // Initialize memory system first
    init_wrt_memory().unwrap();
    init_crate_memory(CrateId::Wasi).unwrap();

    // Try to allocate within limits
    let small_alloc = safe_managed_alloc!(1024, WASI_CRATE_ID);
    assert!(small_alloc.is_ok(), "Small allocation should succeed");

    // The actual enforcement happens at compile time for maximum-safety
    // For other levels, it happens at runtime
    match wasi_safety_level() {
        "maximum-safety" => {
            // In maximum-safety mode, allocations > 16KB would fail at compile time
            // We can't test compile-time failures in a runtime test
            println!("Maximum-safety mode enforces 16KB limit at compile time");
        },
        "static-memory-safety" => {
            // Try to allocate more than 32KB
            let large_alloc = safe_managed_alloc!(40000, WASI_CRATE_ID);
            assert!(
                large_alloc.is_err(),
                "Large allocation should fail in static-memory-safety mode"
            );
        },
        "bounded-collections" => {
            // Try to allocate more than 64KB
            let large_alloc = safe_managed_alloc!(70000, WASI_CRATE_ID);
            assert!(
                large_alloc.is_err(),
                "Large allocation should fail in bounded-collections mode"
            );
        },
        _ => {
            println!("Dynamic allocation mode has no limits");
        },
    }
}

#[test]
fn test_capability_defaults_by_safety_level() {
    // Test that default capabilities change based on safety level
    let provider = WasiProviderBuilder::new().build().unwrap();
    let caps = provider.capabilities();

    match wasi_safety_level() {
        "maximum-safety" => {
            // Should have minimal capabilities
            assert!(!caps.filesystem.read_access);
            assert!(!caps.filesystem.write_access);
            assert!(!caps.environment.environ_access);
            assert!(caps.clocks.monotonic_access); // Only monotonic clock
                                                   // allowed
        },
        "static-memory-safety" | "bounded-collections" => {
            // Should have sandboxed capabilities
            assert!(caps.filesystem.read_access);
            assert!(!caps.filesystem.write_access);
            assert!(caps.environment.args_access);
            assert!(!caps.environment.environ_access);
        },
        _ => {
            // Dynamic allocation mode gets system utility capabilities
            assert!(caps.filesystem.read_access);
            assert!(caps.filesystem.write_access);
            assert!(caps.environment.environ_access);
        },
    }
}

#[test]
fn test_safety_level_override() {
    // Test that we can override safety level for capability selection
    let provider_minimal =
        WasiProviderBuilder::new().with_safety_level("maximum-safety").build().unwrap();

    let caps_minimal = provider_minimal.capabilities();
    assert!(!caps_minimal.filesystem.read_access);

    let provider_sandbox = WasiProviderBuilder::new()
        .with_safety_level("bounded-collections")
        .build()
        .unwrap();

    let caps_sandbox = provider_sandbox.capabilities();
    assert!(caps_sandbox.filesystem.read_access);
    assert!(!caps_sandbox.filesystem.write_access);
}

#[test]
fn test_bounded_collections_in_capabilities() {
    use wrt_wasi::WasiFileSystemCapabilities;

    // Test that bounded collections properly limit capacity
    let mut fs_caps = WasiFileSystemCapabilities::minimal().unwrap();

    // Should be able to add up to MAX_FILESYSTEM_PATHS (32)
    for i in 0..32 {
        let path = format!("/path/{}", i);
        assert!(
            fs_caps.add_allowed_path(&path).is_ok(),
            "Should add path {}",
            i
        );
    }

    // 33rd path should fail
    let result = fs_caps.add_allowed_path("/path/33");
    assert!(
        result.is_err(),
        "Should fail to add 33rd path due to bounded collection limit"
    );
}

#[cfg(feature = "asil-d")]
#[test]
fn test_asil_d_specific_enforcement() {
    // This test only runs when ASIL-D feature is enabled
    assert_eq!(wasi_safety_level(), "maximum-safety");
    assert_eq!(wasi_max_allocation_size(), 16384);

    // ASIL-D should get minimal capabilities by default
    let provider = WasiProviderBuilder::new().build().unwrap();
    let caps = provider.capabilities();
    assert!(!caps.filesystem.read_access);
    assert!(!caps.io.stdout_access);
}

#[cfg(feature = "qm")]
#[test]
fn test_qm_specific_enforcement() {
    // This test only runs when QM feature is enabled
    assert_eq!(wasi_safety_level(), "dynamic-allocation");
    assert_eq!(wasi_max_allocation_size(), usize::MAX);

    // QM should get full capabilities by default
    let provider = WasiProviderBuilder::new().build().unwrap();
    let caps = provider.capabilities();
    assert!(caps.filesystem.read_access);
    assert!(caps.filesystem.write_access);
    assert!(caps.io.stdout_access);
}
