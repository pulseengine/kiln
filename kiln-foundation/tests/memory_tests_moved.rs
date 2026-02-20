//! Foundation Memory Safety Tests - MOVED
//!
//! The memory safety tests for wrt-foundation have been consolidated into
//! the main test suite at: wrt-tests/integration/memory/
//!
//! For the complete memory safety test suite, use:
//! ```
//! cargo test -p wrt-tests memory
//! ```
//!
//! Previously, foundation memory tests were in:
//! - wrt-foundation/tests/safe_memory_test.rs (MOVED)
//! - wrt-foundation/tests/safe_memory_tests.rs (MOVED)
//!
//! All functionality is now available in the consolidated test suite.

#[test]
fn foundation_memory_tests_moved_notice() {
    println!("Foundation memory safety tests have been moved to wrt-tests/integration/memory/");
    println!("Run: cargo test -p wrt-tests memory");
}
