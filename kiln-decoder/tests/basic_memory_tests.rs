//! Basic memory optimization tests that work with current dependencies

#[cfg(feature = "std")]
mod memory_tests {
    use wrt_decoder::memory_optimized::{MemoryPool, check_bounds_u32, safe_usize_conversion};
    use wrt_foundation::NoStdProvider;

    #[test]
    fn test_bounds_checking() {
        // Test successful bounds check
        assert!(check_bounds_u32(10, 20, "test").is_ok());

        // Test failed bounds check
        assert!(check_bounds_u32(30, 20, "test").is_err());
    }

    #[test]
    fn test_safe_usize_conversion() {
        // Test successful conversion
        assert_eq!(safe_usize_conversion(42, "test").unwrap(), 42);

        // Test with maximum u32 value (should work on 64-bit systems)
        let max_u32 = u32::MAX;
        let result = safe_usize_conversion(max_u32, "test");

        // On 64-bit systems this should succeed, on 32-bit it might fail
        #[cfg(target_pointer_width = "64")]
        assert!(result.is_ok());
    }

    #[test]
    fn test_memory_pool() {
        let provider = NoStdProvider::<1024>::default();
        let mut pool = MemoryPool::new(provider);

        // Get a vector from the pool
        let vec1 = pool.get_instruction_vector();
        assert_eq!(vec1.len(), 0);

        // Return it to the pool
        pool.return_instruction_vector(vec1);

        // Get another vector - should be reused
        let vec2 = pool.get_instruction_vector();
        assert_eq!(vec2.len(), 0);
    }
}

#[cfg(feature = "std")]
mod string_tests {
    use wrt_decoder::optimized_string::parse_utf8_string_inplace;

    #[test]
    fn test_string_parsing() {
        // Create a simple string with LEB128 length prefix
        let mut test_data = vec![5]; // Length 5
        test_data.extend_from_slice(b"hello");

        let result = parse_utf8_string_inplace(&test_data, 0);
        assert!(result.is_ok());

        let (string, offset) = result.unwrap();
        assert_eq!(string, "hello");
        assert_eq!(offset, 6); // 1 byte length + 5 bytes string
    }
}
