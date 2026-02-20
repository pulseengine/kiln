//! Unit tests for memory optimization utilities

#[cfg(feature = "std")]
mod tests {
    #[test]
    fn test_bounds_checking() {
        use wrt_decoder::memory_optimized::check_bounds_u32;

        // Test successful bounds check
        assert!(check_bounds_u32(10, 20, "test").is_ok());

        // Test failed bounds check
        let result = check_bounds_u32(30, 20, "test");
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(
            error.message().contains("exceeds maximum")
                || error.message().contains("Bounds check failed")
        );
    }

    #[test]
    fn test_safe_usize_conversion() {
        use wrt_decoder::memory_optimized::safe_usize_conversion;

        // Test successful conversion
        assert_eq!(safe_usize_conversion(42, "test").unwrap(), 42);
        assert_eq!(safe_usize_conversion(0, "test").unwrap(), 0);
        assert_eq!(safe_usize_conversion(1000, "test").unwrap(), 1000);
    }

    #[test]
    fn test_memory_optimization_integration() {
        use wrt_decoder::memory_optimized::{check_bounds_u32, safe_usize_conversion};

        // Simulate parsing a section with bounds checking
        let alleged_count = 1000u32;
        let max_allowed = 10000u32;

        // Check bounds first
        assert!(check_bounds_u32(alleged_count, max_allowed, "section count").is_ok());

        // Convert to usize safely
        let count_usize = safe_usize_conversion(alleged_count, "section count").unwrap();
        assert_eq!(count_usize, 1000);

        // Simulate conservative memory reservation
        let reserved_capacity = count_usize.min(1024);
        assert_eq!(reserved_capacity, 1000);
    }

    #[test]
    fn test_bounds_checking_prevents_over_allocation() {
        use wrt_decoder::memory_optimized::check_bounds_u32;

        // Test that maliciously large counts are rejected
        let malicious_count = u32::MAX;
        let reasonable_limit = 10000u32;

        let result = check_bounds_u32(malicious_count, reasonable_limit, "malicious count");
        assert!(result.is_err());

        // Binary std/no_std choice
        println!(
            "Successfully rejected malicious allocation of {} items",
            malicious_count
        );
    }
}

#[cfg(feature = "std")]
mod string_optimization_tests {
    #[test]
    fn test_utf8_validation_without_allocation() {
        use wrt_decoder::optimized_string::validate_utf8_name;

        // Create test data: [length][string_bytes]
        let mut test_data = vec![];
        test_data.push(5u8); // Length
        test_data.extend_from_slice(b"hello");

        let result = validate_utf8_name(&test_data, 0);
        assert!(result.is_ok());

        let (validated_str, new_offset) = result.unwrap();
        assert_eq!(validated_str, "hello");
        assert_eq!(new_offset, 6); // 1 byte length + 5 bytes string
    }

    #[test]
    fn test_invalid_utf8_handling() {
        use wrt_decoder::optimized_string::validate_utf8_name;

        // Create test data with invalid UTF-8
        let mut test_data = vec![];
        test_data.push(4u8); // Length
        test_data.extend_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC]); // Invalid UTF-8

        let result = validate_utf8_name(&test_data, 0);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.message().contains("UTF-8"));
    }
}

#[cfg(all(not(feature = "std")))]
mod no_std_tests {
    use wrt_foundation::NoStdProvider;

    #[test]
    fn test_memory_pool_with_no_std_provider() {
        use wrt_decoder::memory_optimized::MemoryPool;

        let provider = NoStdProvider::<2048>::default();
        let mut pool = MemoryPool::new(provider);

        // Test that we can get and return vectors
        let vec1 = pool.get_instruction_vector();
        assert_eq!(vec1.len(), 0);

        pool.return_instruction_vector(vec1);

        // Test string buffer pool
        let str_buf = pool.get_string_buffer();
        assert_eq!(str_buf.len(), 0);

        pool.return_string_buffer(str_buf);
    }
}
