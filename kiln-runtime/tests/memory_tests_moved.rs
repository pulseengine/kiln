//! Runtime Memory Safety Tests - MOVED
//!
//! The memory safety tests for wrt-runtime have been consolidated into
//! the main test suite at: wrt-tests/integration/memory/
//!
//! For the complete memory safety test suite, use:
//! ```
//! cargo test -p wrt-tests memory
//! ```
//!
//! Previously, runtime memory tests were in:
//! - wrt-runtime/src/tests/safe_memory_test.rs (MOVED)
//! - wrt-runtime/tests/memory_safety_tests.rs (MOVED)
//!
//! All functionality is now available in the consolidated test suite.

#[test]
fn runtime_memory_tests_moved_notice() {
    println!("Runtime memory safety tests have been moved to wrt-tests/integration/memory/");
    println!("Run: cargo test -p wrt-tests memory");
}
