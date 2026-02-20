#![allow(clippy::doc_markdown)]
//! No-std compatibility test reference for wrt-error
//!
//! This file references the consolidated `no_std` tests in
//! wrt-tests/integration/no_std/ The actual `no_std` tests for wrt-error are
//! now part of the centralized test suite.
//!
//! To run the `no_std` tests for wrt-error specifically:
//! ```
//! cargo test -p wrt-tests --test consolidated_no_std_tests wrt_error_tests
//! ```
//!
//! To run all `no_std` tests across the entire WRT ecosystem:
//! ```
//! cargo test -p wrt-tests --no-default-features --features alloc
//! ```

#[cfg(test)]
mod tests {
    #[test]
    fn no_std_tests_moved_to_centralized_location() {
        // The `no_std` compatibility tests for wrt-error have been moved to:
        // wrt-tests/integration/no_std/consolidated_no_std_tests.rs
        //
        // This consolidation eliminates duplication and provides a single
        // location for all `no_std` testing across the WRT ecosystem.

        println!("`no_std` tests for wrt-error are in wrt-tests/integration/no_std/");
        println!("Run: cargo test -p wrt-tests consolidated_no_std_tests::wrt_error_tests");
    }
}
