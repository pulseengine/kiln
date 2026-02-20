#![allow(clippy::doc_markdown)]
//! No-std compatibility test reference for wrt-component
//!
//! This file references the consolidated no_std tests in
//! wrt-tests/integration/no_std/ The actual no_std tests for wrt-component are
//! now part of the centralized test suite.

#[cfg(test)]
mod tests {
    #[test]
    fn no_std_tests_moved_to_centralized_location() {
        println!("No-std tests for wrt-component are in wrt-tests/integration/no_std/");
        println!("Run: cargo test -p wrt-tests consolidated_no_std_tests");
    }
}
