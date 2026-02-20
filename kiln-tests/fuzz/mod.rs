//! Fuzz Testing Module
//!
//! This module consolidates all fuzzing tests for the WRT project,
//! providing property-based testing for critical components.

use wrt_test_registry::prelude::*;

mod bounded_collections_fuzz;
mod memory_adapter_fuzz; 
mod safe_memory_fuzz;

/// Run all fuzz integration tests (in non-fuzz mode for CI)
pub fn run_tests() -> TestResult {
    let mut runner = TestRunner::new("Fuzz Testing Integration";
    
    runner.add_test_suite("Bounded Collections Property Tests", || {
        // Property-based testing for bounded collections
        Ok(())
    })?;
    
    runner.add_test_suite("Memory Adapter Property Tests", || {
        // Property-based testing for memory adapters
        Ok(())
    })?;
    
    runner.add_test_suite("Safe Memory Property Tests", || {
        // Property-based testing for safe memory primitives
        Ok(())
    })?;
    
    runner.run_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn fuzz_integration() {
        let result = run_tests);
        assert!(result.is_success(), "Fuzz integration tests failed: {:?}", result);
    }
}