//! Empty crate host for the cross-executor comparison bench (kiln#368).
//!
//! This crate exists only to carry `benches/comparison.rs`; it is excluded from
//! the main workspace so embassy's transitive dependencies never touch the
//! safety-critical runtime's lockfile. See `Cargo.toml`.
