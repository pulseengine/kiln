//! Safe memory management re-exports from wrt-foundation
//!
//! This module re-exports the safe memory types and functionality from
//! wrt-foundation to provide a consistent interface for both wrt-format and
//! other crates.

// Re-export the safe memory types from wrt-foundation
// Re-export memory providers matching wrt-foundation feature-gating
#[cfg(feature = "std")]
pub use wrt_foundation::safe_memory::StdProvider as StdMemoryProvider;
#[cfg(not(feature = "std"))]
pub use wrt_foundation::NoStdProvider as NoStdMemoryProvider;
// Re-export common memory types always
pub use wrt_foundation::{
    BoundedStack as SafeStack,
    MemoryProvider,
    SafeMemoryHandler,
    SafeSlice,
};

/// Create a safe slice from binary data
pub fn safe_slice(data: &[u8]) -> wrt_error::Result<wrt_foundation::SafeSlice<'_>> {
    wrt_foundation::SafeSlice::new(data)
}

/// Get the default verification level for memory operations
pub fn default_verification_level() -> wrt_foundation::verification::VerificationLevel {
    wrt_foundation::verification::VerificationLevel::Basic
}
