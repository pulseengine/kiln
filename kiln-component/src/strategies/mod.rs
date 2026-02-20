// Memory optimization strategies for WebAssembly Component Model
//
// This module provides various strategies for memory optimization in the
// WebAssembly Component Model implementation.

pub mod memory;

// Re-export types for convenience
pub use memory::{
    BoundedCopyStrategy, FullIsolationStrategy, MemoryOptimizationStrategy, ZeroCopyStrategy,
};
