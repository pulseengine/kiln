//! Runtime Memory Safety Tests - MOVED
//!
//! The memory safety tests for kiln-runtime have been consolidated into
//! the main test suite at: kiln-tests/integration/memory/
//!
//! For the complete memory safety test suite, use:
//! ```
//! cargo test -p kiln-tests memory
//! ```
//!
//! Previously, runtime memory tests were in:
//! - kiln-runtime/src/tests/safe_memory_test.rs (MOVED)
//! - kiln-runtime/tests/memory_safety_tests.rs (MOVED)
//!
//! All functionality is now available in the consolidated test suite.

#[test]
fn runtime_memory_tests_moved_notice() {
    println!("Runtime memory safety tests have been moved to kiln-tests/integration/memory/");
    println!("Run: cargo test -p kiln-tests memory");
}
