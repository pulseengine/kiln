#![allow(clippy::doc_markdown)]
//! No-std compatibility test reference for kiln-error
//!
//! This file references the consolidated `no_std` tests in
//! kiln-tests/integration/no_std/ The actual `no_std` tests for kiln-error are
//! now part of the centralized test suite.
//!
//! To run the `no_std` tests for kiln-error specifically:
//! ```
//! cargo test -p kiln-tests --test consolidated_no_std_tests kiln_error_tests
//! ```
//!
//! To run all `no_std` tests across the entire Kiln ecosystem:
//! ```
//! cargo test -p kiln-tests --no-default-features --features alloc
//! ```

#[cfg(test)]
mod tests {
    #[test]
    fn no_std_tests_moved_to_centralized_location() {
        // The `no_std` compatibility tests for kiln-error have been moved to:
        // kiln-tests/integration/no_std/consolidated_no_std_tests.rs
        //
        // This consolidation eliminates duplication and provides a single
        // location for all `no_std` testing across the Kiln ecosystem.

        println!("`no_std` tests for kiln-error are in kiln-tests/integration/no_std/");
        println!("Run: cargo test -p kiln-tests consolidated_no_std_tests::kiln_error_tests");
    }
}
