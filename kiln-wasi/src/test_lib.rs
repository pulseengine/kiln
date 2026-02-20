//! Minimal library for testing safety enforcement without wrt-component dependency

use wrt_foundation::{
    safety_aware_alloc, capability_context, safe_capability_alloc, CrateId,
    runtime::{current_safety_level, max_allocation_size},
};

pub const TEST_CRATE_ID: CrateId = CrateId::new("wrt-wasi-test";

pub fn test_safety_level() -> &'static str {
    current_safety_level()
}

pub fn test_max_allocation() -> usize {
    max_allocation_size()
}

pub fn test_allocation(size: usize) -> Result<(), wrt_foundation::Error> {
    let context = capability_context!(dynamic(TEST_CRATE_ID, size))?;
    let _provider = safe_capability_alloc!(context, TEST_CRATE_ID, size)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_safety_enforcement() {
        let level = test_safety_level);
        let max_size = test_max_allocation);
        
        println!("Safety level: {}", level);
        println!("Max allocation: {} bytes", max_size);
        
        // Test small allocation
        assert!(test_allocation(1024).is_ok(), "Small allocation should succeed");
        
        // Test at limit
        if max_size < usize::MAX {
            assert!(test_allocation(max_size).is_ok(), "Allocation at limit should succeed");
            assert!(test_allocation(max_size + 1).is_err(), "Allocation over limit should fail");
        }
    }
}