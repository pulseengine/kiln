//! Safe memory management re-exports from kiln-foundation
//!
//! This module re-exports the safe memory types and functionality from
//! kiln-foundation to provide a consistent interface for both kiln-format and
//! other crates.

// Re-export the safe memory types from kiln-foundation
// Re-export memory providers matching kiln-foundation feature-gating
#[cfg(feature = "std")]
pub use kiln_foundation::safe_memory::StdProvider as StdMemoryProvider;
#[cfg(not(feature = "std"))]
pub use kiln_foundation::NoStdProvider as NoStdMemoryProvider;
// Re-export common memory types always
pub use kiln_foundation::{
    BoundedStack as SafeStack,
    MemoryProvider,
    SafeMemoryHandler,
    SafeSlice,
};

/// Create a safe slice from binary data
pub fn safe_slice(data: &[u8]) -> kiln_error::Result<kiln_foundation::SafeSlice<'_>> {
    kiln_foundation::SafeSlice::new(data)
}

/// Get the default verification level for memory operations
pub fn default_verification_level() -> kiln_foundation::verification::VerificationLevel {
    kiln_foundation::verification::VerificationLevel::Basic
}
