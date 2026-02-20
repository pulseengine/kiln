//! Simple test to verify safety feature enforcement without wrt-component
//! dependency

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
};

#[test]
fn test_safety_level_detection() {
    let level = wasi_safety_level();
    println!("Current safety level: {}", level);

    // The level depends on which feature is enabled at compile time
    #[cfg(feature = "qm")]
    assert_eq!(level, "dynamic-allocation");

    #[cfg(feature = "asil-a")]
    assert_eq!(level, "bounded-collections");

    #[cfg(feature = "asil-b")]
    assert_eq!(level, "bounded-collections");

    #[cfg(feature = "asil-c")]
    assert_eq!(level, "static-memory-safety");

    #[cfg(feature = "asil-d")]
    assert_eq!(level, "maximum-safety");
}

#[test]
fn test_allocation_limits() {
    let max_size = wasi_max_allocation_size();
    println!("Maximum allocation size: {} bytes", max_size);

    #[cfg(feature = "asil-d")]
    assert_eq!(max_size, 16384); // 16KB for ASIL-D

    #[cfg(feature = "asil-c")]
    assert_eq!(max_size, 32768); // 32KB for ASIL-C

    #[cfg(all(
        any(feature = "asil-a", feature = "asil-b"),
        not(any(feature = "asil-c", feature = "asil-d"))
    ))]
    assert_eq!(max_size, 65536); // 64KB for ASIL-A/B

    #[cfg(all(
        feature = "qm",
        not(any(
            feature = "asil-a",
            feature = "asil-b",
            feature = "asil-c",
            feature = "asil-d"
        ))
    ))]
    assert_eq!(max_size, usize::MAX); // No limit for QM
}

#[test]
fn test_safety_aware_allocation() {
    // Initialize memory system first
    init_wrt_memory().unwrap();
    init_crate_memory(CrateId::Wasi).unwrap();

    // Small allocation should always work
    let small_result = safe_managed_alloc!(1024, CrateId::Wasi);
    assert!(small_result.is_ok(), "Small allocation should succeed");

    // Test allocation at various sizes
    let max_allowed = wasi_max_allocation_size();

    // Test 8KB allocation
    {
        let size = 8192;
        println!("Testing allocation of {} bytes", size);
        let result = safe_managed_alloc!(8192, CrateId::Wasi);
        if size <= max_allowed {
            assert!(
                result.is_ok(),
                "Allocation of {} bytes should succeed (max: {})",
                size,
                max_allowed
            );
        } else {
            assert!(
                result.is_err(),
                "Allocation of {} bytes should fail (max: {})",
                size,
                max_allowed
            );
        }
    }

    // Test 16KB allocation
    {
        let size = 16384;
        println!("Testing allocation of {} bytes", size);
        let result = safe_managed_alloc!(16384, CrateId::Wasi);
        if size <= max_allowed {
            assert!(
                result.is_ok(),
                "Allocation of {} bytes should succeed (max: {})",
                size,
                max_allowed
            );
        } else {
            assert!(
                result.is_err(),
                "Allocation of {} bytes should fail (max: {})",
                size,
                max_allowed
            );
        }
    }

    // Test 32KB allocation
    {
        let size = 32768;
        println!("Testing allocation of {} bytes", size);
        let result = safe_managed_alloc!(32768, CrateId::Wasi);
        if size <= max_allowed {
            assert!(
                result.is_ok(),
                "Allocation of {} bytes should succeed (max: {})",
                size,
                max_allowed
            );
        } else {
            assert!(
                result.is_err(),
                "Allocation of {} bytes should fail (max: {})",
                size,
                max_allowed
            );
        }
    }

    // Test 64KB allocation
    {
        let size = 65536;
        println!("Testing allocation of {} bytes", size);
        let result = safe_managed_alloc!(65536, CrateId::Wasi);
        if size <= max_allowed {
            assert!(
                result.is_ok(),
                "Allocation of {} bytes should succeed (max: {})",
                size,
                max_allowed
            );
        } else {
            assert!(
                result.is_err(),
                "Allocation of {} bytes should fail (max: {})",
                size,
                max_allowed
            );
        }
    }
}

#[cfg(feature = "asil-d")]
#[test]
fn test_asil_d_strict_limits() {
    // ASIL-D has the strictest limits
    assert_eq!(wasi_safety_level(), "maximum-safety");
    assert_eq!(wasi_max_allocation_size(), 16384);

    // 16KB allocation should work
    let result_16k = safe_managed_alloc!(16384, WASI_CRATE_ID);
    assert!(
        result_16k.is_ok(),
        "16KB allocation should succeed in ASIL-D"
    );

    // 17KB allocation should fail
    let result_17k = safe_managed_alloc!(17408, WASI_CRATE_ID);
    assert!(result_17k.is_err(), "17KB allocation should fail in ASIL-D");
}

#[cfg(feature = "qm")]
#[test]
fn test_qm_no_limits() {
    // QM has no allocation limits
    assert_eq!(wasi_safety_level(), "dynamic-allocation");
    assert_eq!(wasi_max_allocation_size(), usize::MAX);

    // Large allocation should work
    let result_large = safe_managed_alloc!(1048576, WASI_CRATE_ID); // 1MB
    assert!(
        result_large.is_ok(),
        "Large allocation should succeed in QM mode"
    );
}

#[test]
fn test_feature_combinations() {
    // Test that multiple features are handled correctly
    let level = wasi_safety_level();
    let max_size = wasi_max_allocation_size();

    println!("Active features:");
    #[cfg(feature = "qm")]
    println!("  - qm");
    #[cfg(feature = "asil-a")]
    println!("  - asil-a");
    #[cfg(feature = "asil-b")]
    println!("  - asil-b");
    #[cfg(feature = "asil-c")]
    println!("  - asil-c");
    #[cfg(feature = "asil-d")]
    println!("  - asil-d");

    println!("Detected safety level: {}", level);
    println!("Max allocation size: {} bytes", max_size);

    // When multiple features are enabled, the strictest one should win
    #[cfg(all(feature = "asil-d", feature = "qm"))]
    {
        assert_eq!(
            level, "maximum-safety",
            "ASIL-D should take precedence over QM"
        );
        assert_eq!(max_size, 16384);
    }
}
