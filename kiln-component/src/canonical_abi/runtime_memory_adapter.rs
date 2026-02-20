//! Runtime Memory Adapter for Canonical ABI
//!
//! NOTE: This module has been moved to wrt-wasi to avoid dependency cycles.
//! The RuntimeMemoryAdapter is now in wrt-wasi/src/canonical_host.rs.
//!
//! This stub remains for backwards compatibility documentation.
//!
//! The CanonicalMemory trait (defined in canonical_abi.rs) is the interface
//! that adapters must implement. Concrete implementations that need runtime
//! Memory access should be defined in crates that can depend on both
//! wrt-component and wrt-runtime (like wrt-wasi).
