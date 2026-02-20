#![allow(clippy::doc_markdown)]
//! No-std compatibility test reference for kiln-foundation
//!
//! This file references the consolidated no_std tests in
//! kiln-tests/integration/no_std/ The actual no_std tests for kiln-foundation are
//! now part of the centralized test suite.
//!
//! To run the no_std tests for kiln-foundation specifically:
//! ```
//! cargo test -p kiln-tests --test consolidated_no_std_tests kiln_foundation_tests
//! ```

#[cfg(test)]
mod tests {
    #[test]
    fn no_std_tests_moved_to_centralized_location() {
        println!("No-std tests for kiln-foundation are in kiln-tests/integration/no_std/");
        println!("Run: cargo test -p kiln-tests consolidated_no_std_tests::kiln_foundation_tests");
    }
}
