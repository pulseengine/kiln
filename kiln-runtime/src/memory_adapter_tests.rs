//! Tests for memory_adapter module

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::memory_adapter::{SafeMemoryAdapter, MemoryAdapter, StdMemoryProvider};
    use crate::types::MemoryType;
    use wrt_foundation::types::Limits;
    use wrt_foundation::verification::VerificationLevel;

    fn create_test_memory_type() -> MemoryType {
        MemoryType {
            limits: Limits { min: 1, max: Some(4) },
        }
    }

    #[test]
    fn test_std_memory_provider_implementation() {
        // Test that StdMemoryProvider properly implements required traits
        let provider = StdMemoryProvider::new(&[];
        
        // Test verification level
        assert_eq!(provider.verification_level(), VerificationLevel::Standard;
        
        // Test that methods return proper errors (not panic)
        let result = provider.borrow_slice(0, 10;
        assert!(result.is_err();
        
        let result = provider.acquire_memory(std::alloc::Layout::from_size_align(1024, 8).unwrap();
        assert!(result.is_err();
    }

    #[test]
    fn test_safe_memory_adapter_creation() {
        let mem_type = create_test_memory_type);
        let adapter = SafeMemoryAdapter::new(mem_type;
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_safe_memory_adapter_basic_operations() {
        let mem_type = create_test_memory_type);
        let adapter = SafeMemoryAdapter::new(mem_type).unwrap();
        
        // Test size
        let size = adapter.size);
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 1); // 1 page
        
        // Test byte size
        let byte_size = adapter.byte_size);
        assert!(byte_size.is_ok());
        assert_eq!(byte_size.unwrap(), 65536); // 64KB
        
        // Test read/write
        let test_data = vec![1u8, 2, 3, 4, 5];
        let write_result = adapter.write_all(0, &test_data;
        assert!(write_result.is_ok());
        
        let read_result = adapter.read_exact(0, test_data.len() as u32;
        assert!(read_result.is_ok());
        
        let read_data = read_result.unwrap();
        assert_eq!(read_data.as_slice(), &test_data[..];
    }

    #[test]
    fn test_safe_memory_adapter_bounds_checking() {
        let mem_type = create_test_memory_type);
        let adapter = SafeMemoryAdapter::new(mem_type).unwrap();
        
        // Test out of bounds read
        let oob_read = adapter.read_exact(65536, 1);
        assert!(oob_read.is_err();
        
        // Test out of bounds write
        let oob_write = adapter.write_all(65536, &[1];
        assert!(oob_write.is_err();
        
        // Test range check
        let range_check = adapter.check_range(0, 65537;
        assert!(range_check.is_err();
        
        let valid_range = adapter.check_range(0, 65536;
        assert!(valid_range.is_ok());
    }

    #[test]
    fn test_safe_memory_adapter_growth() {
        let mem_type = create_test_memory_type);
        let adapter = SafeMemoryAdapter::new(mem_type).unwrap();
        
        // Initial size
        let initial_size = adapter.size().unwrap();
        assert_eq!(initial_size, 1);
        
        // Grow by 1 page
        let prev_size = adapter.grow(1;
        assert!(prev_size.is_ok());
        assert_eq!(prev_size.unwrap(), 1);
        
        // New size should be 2 pages
        let new_size = adapter.size().unwrap();
        assert_eq!(new_size, 2;
        
        // Can now write to the new page
        let write_result = adapter.write_all(65536, &[42];
        assert!(write_result.is_ok());
    }
}