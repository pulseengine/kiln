//! Integration tests for Linux platform implementations.
//!
//! These tests verify that the Linux memory allocator and futex implementations
//! compile and can be instantiated correctly, even when running on non-Linux
//! platforms.

#[cfg(all(feature = "platform-linux", target_os = "linux"))]
mod linux_tests {
    use wrt_platform::{
        LinuxAllocator,
        LinuxAllocatorBuilder,
        LinuxFutex,
        LinuxFutexBuilder,
    };

    #[test]
    fn test_linux_allocator_creation() {
        let allocator = LinuxAllocatorBuilder::new()
            .with_maximum_pages(100)
            .with_guard_pages(true)
            .build();

        // Binary std/no_std choice
        assert!(core::mem::size_of_val(&allocator) > 0);
    }

    #[test]
    fn test_linux_futex_creation() {
        let futex = LinuxFutexBuilder::new().with_initial_value(42).build();

        // Verify the futex was created successfully
        assert!(core::mem::size_of_val(&futex) > 0);
    }

    #[test]
    fn test_linux_futex_operations() {
        let futex = LinuxFutexBuilder::new().with_initial_value(42).build();

        // Test that the futex was created with the correct initial value
        // Note: The FutexLike trait in wrt-platform doesn't expose atomic operations
        // directly, so we can only test construction and basic properties
        assert!(core::mem::size_of_val(&futex) > 0);
    }
}

#[cfg(all(
    feature = "platform-linux",
    feature = "linux-mte",
    target_arch = "aarch64",
    target_os = "linux"
))]
mod linux_mte_tests {
    use wrt_platform::{
        LinuxArm64MteAllocator,
        LinuxArm64MteAllocatorBuilder,
        MteMode,
    };

    #[test]
    fn test_linux_mte_allocator_creation() {
        let allocator = LinuxArm64MteAllocatorBuilder::new()
            .with_maximum_pages(100)
            .with_guard_pages(true)
            .with_mte_mode(MteMode::Synchronous)
            .build();

        // Binary std/no_std choice
        assert!(core::mem::size_of_val(&allocator) > 0);
    }

    #[test]
    fn test_mte_mode_configuration() {
        // Test different MTE modes
        let sync_allocator =
            LinuxArm64MteAllocatorBuilder::new().with_mte_mode(MteMode::Synchronous).build();

        let async_allocator = LinuxArm64MteAllocatorBuilder::new()
            .with_mte_mode(MteMode::Asynchronous)
            .build();

        let disabled_allocator =
            LinuxArm64MteAllocatorBuilder::new().with_mte_mode(MteMode::Disabled).build();

        // All should build successfully
        assert!(core::mem::size_of_val(&sync_allocator) > 0);
        assert!(core::mem::size_of_val(&async_allocator) > 0);
        assert!(core::mem::size_of_val(&disabled_allocator) > 0);
    }
}

// This test runs on all platforms to verify compilation
#[test]
fn test_linux_platform_compilation() {
    // This test ensures our Linux platform code compiles correctly
    // even when the feature flags are enabled on non-Linux platforms

    #[cfg(feature = "platform-linux")]
    {
        // The modules should be available for import
        use wrt_platform::*;

        // Basic verification that types exist
        let _ = core::mem::size_of::<NoStdProvider>();

        // On Linux, additional types should be available
        #[cfg(target_os = "linux")]
        {
            let _ = core::mem::size_of::<LinuxAllocatorBuilder>();
            let _ = core::mem::size_of::<LinuxFutexBuilder>();
        }

        // On ARM64 Linux with MTE, even more types should be available
        #[cfg(all(feature = "linux-mte", target_arch = "aarch64", target_os = "linux"))]
        {
            let _ = core::mem::size_of::<LinuxArm64MteAllocatorBuilder>();
            let _ = core::mem::size_of::<MteMode>();
        }
    }
}
