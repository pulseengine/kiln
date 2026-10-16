//! # kilnd — deprecated alias of the `kiln` binary
//!
//! Shipped in v0.6.0 and v0.7.0 so existing callers keep working; it behaves exactly
//! like `kiln` but prints a deprecation warning on stderr. Removed in v0.8.0
//! (SR-64, issue #486 ask 3).

#![deny(unsafe_code)]

fn main() -> kiln_error::Result<()> {
    kilnd::run_on_large_stack(env!("CARGO_BIN_NAME"))
}
