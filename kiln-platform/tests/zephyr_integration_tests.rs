//! Integration tests for Zephyr RTOS platform implementations.
//!
//! These tests verify that the Zephyr memory allocator and futex
//! implementations compile and can be instantiated correctly for embedded
//! systems.

#[cfg(feature = "platform-zephyr")]
mod zephyr_tests {
    use wrt_platform::{
        ZephyrAllocator,
        ZephyrAllocatorBuilder,
        ZephyrFutex,
        ZephyrFutexBuilder,
        ZephyrMemoryFlags,
        ZephyrSemaphoreFutex,
    };

    #[test]
    fn test_zephyr_allocator_creation() {
        let allocator = ZephyrAllocatorBuilder::new()
            .with_maximum_pages(100)
            .with_memory_domains(true)
            .with_guard_regions(true)
            .build();

        // Binary std/no_std choice
        assert!(core::mem::size_of_val(&allocator) > 0);
    }

    #[test]
    fn test_zephyr_allocator_configuration() {
        // Test different memory attribute configurations
        let read_only_allocator = ZephyrAllocatorBuilder::new()
            .with_memory_attributes(ZephyrMemoryFlags::Read)
            .build();

        let read_write_allocator = ZephyrAllocatorBuilder::new()
            .with_memory_attributes(ZephyrMemoryFlags::Write)
            .build();

        // All should build successfully
        assert!(core::mem::size_of_val(&read_only_allocator) > 0);
        assert!(core::mem::size_of_val(&read_write_allocator) > 0);
    }

    #[test]
    fn test_zephyr_futex_creation() {
        let futex = ZephyrFutexBuilder::new().with_initial_value(42).build();

        // Verify the futex was created successfully
        assert!(core::mem::size_of_val(&futex) > 0);
    }

    #[test]
    fn test_zephyr_semaphore_futex_creation() {
        let futex = ZephyrSemaphoreFutex::new(0);

        // Verify the semaphore futex was created successfully
        assert!(core::mem::size_of_val(&futex) > 0);
    }

    #[test]
    fn test_zephyr_memory_flags() {
        // Test that memory flags can be combined
        let read_write = ZephyrMemoryFlags::Read as u32 | ZephyrMemoryFlags::Write as u32;
        assert_eq!(read_write, 0x03);

        let cacheable_rw = read_write | ZephyrMemoryFlags::Cacheable as u32;
        assert_eq!(cacheable_rw, 0x0B);
    }

    #[test]
    fn test_zephyr_allocator_with_custom_heap() {
        let allocator = ZephyrAllocatorBuilder::new()
            .with_custom_heap(true)
            .with_maximum_pages(50)
            .build();

        // Binary std/no_std choice
        assert!(core::mem::size_of_val(&allocator) > 0);
    }

    #[test]
    fn test_zephyr_allocator_without_memory_domains() {
        let allocator = ZephyrAllocatorBuilder::new()
            .with_memory_domains(false)
            .with_guard_regions(false)
            .build();

        // Binary std/no_std choice
        assert!(core::mem::size_of_val(&allocator) > 0);
    }
}

// This test runs on all platforms to verify compilation
#[test]
fn test_zephyr_platform_compilation() {
    // This test ensures our Zephyr platform code compiles correctly
    // even when the feature flags are enabled on non-Zephyr platforms

    #[cfg(feature = "platform-zephyr")]
    {
        // The modules should be available for import
        use wrt_platform::*;

        // Basic verification that types exist
        let _ = core::mem::size_of::<NoStdProvider<1024>>();

        // Zephyr-specific types should be available
        let _ = core::mem::size_of::<ZephyrAllocatorBuilder>();
        let _ = core::mem::size_of::<ZephyrFutexBuilder>();
        let _ = core::mem::size_of::<ZephyrMemoryFlags>();
    }
}
