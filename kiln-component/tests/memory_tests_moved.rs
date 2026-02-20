//! Component Memory Optimization Tests - MOVED
//!
//! The memory optimization tests for kiln-component have been consolidated into
//! the main test suite at: kiln-tests/integration/memory/
//!
//! For the complete memory safety test suite, use:
//! ```
//! cargo test -p kiln-tests memory
//! ```
//!
//! Previously, component memory tests were in:
//! - kiln-component/tests/memory_optimization_tests.rs (MOVED)
//!
//! All functionality is now available in the consolidated test suite.

#[test]
fn component_memory_tests_moved_notice() {
    println!(
        "Component memory optimization tests have been moved to kiln-tests/integration/memory/"
    );
    println!("Run: cargo test -p kiln-tests memory");
}
