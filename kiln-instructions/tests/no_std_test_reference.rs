#![allow(clippy::doc_markdown)]
//! No-std compatibility test reference for kiln-instructions
//!
//! This file references the consolidated no_std tests in
//! kiln-tests/integration/no_std/ The actual no_std tests for kiln-instructions
//! are now part of the centralized test suite.

#[cfg(test)]
mod tests {
    #[test]
    fn no_std_tests_moved_to_centralized_location() {
        println!("No-std tests for kiln-instructions are in kiln-tests/integration/no_std/");
        println!("Run: cargo test -p kiln-tests consolidated_no_std_tests");
    }
}
