//! WebAssembly GC (Garbage Collection) Proposal Support
//!
//! This module implements the WebAssembly GC proposal, providing:
//! - Managed heap for struct and array allocations
//! - Object representation with type information
//! - Reference tracking for garbage collection
//! - Mark-and-sweep garbage collector
//!
//! # Design
//!
//! The GC heap uses a capability-based memory system compatible with no_std
//! environments. Objects are allocated from a fixed-size heap with headers
//! containing type information and GC metadata.
//!
//! # Object Layout
//!
//! ```text
//! +----------------+----------------+----------------+
//! | Header (8B)    | Type Idx (4B)  | Fields/Elements|
//! | mark | size    |                |                |
//! +----------------+----------------+----------------+
//! ```
//!
//! # Garbage Collection
//!
//! The collector uses a mark-and-sweep algorithm:
//! 1. Mark phase: Traverse from roots, marking reachable objects
//! 2. Sweep phase: Reclaim unmarked objects
//!
//! For no_std compatibility, the mark stack is fixed-size.

mod collector;
mod heap;
mod object;

pub use collector::{GcCollector, GcStats, RootSet};
pub use heap::GcHeap;
pub use object::{GcObject, GcObjectRef, ObjectHeader};

use wrt_error::Result;
use wrt_foundation::types::HeapType;

/// Maximum size of the GC heap in bytes (1MB default)
pub const DEFAULT_GC_HEAP_SIZE: usize = 1024 * 1024;

/// Alignment for GC objects (8 bytes for 64-bit compatibility)
pub const GC_OBJECT_ALIGNMENT: usize = 8;

/// GC reference - a handle to a garbage-collected object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcRef {
    /// Offset into the GC heap (0 = null reference)
    offset: u32,
}

impl GcRef {
    /// Create a null GC reference
    #[inline]
    pub const fn null() -> Self {
        Self { offset: 0 }
    }

    /// Create a GC reference from a heap offset
    #[inline]
    pub const fn from_offset(offset: u32) -> Self {
        Self { offset }
    }

    /// Check if this is a null reference
    #[inline]
    pub const fn is_null(&self) -> bool {
        self.offset == 0
    }

    /// Get the heap offset (returns None for null)
    #[inline]
    pub const fn offset(&self) -> Option<u32> {
        if self.offset == 0 {
            None
        } else {
            Some(self.offset)
        }
    }
}

impl Default for GcRef {
    fn default() -> Self {
        Self::null()
    }
}

/// i31ref - an unboxed 31-bit signed integer reference
///
/// In WebAssembly GC, i31ref is a special reference type that stores
/// a 31-bit signed integer directly in the reference, without heap allocation.
/// The top bit is used to distinguish it from heap references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I31Ref {
    /// The 31-bit value stored directly
    value: i32,
}

impl I31Ref {
    /// Maximum value for i31 (2^30 - 1)
    pub const MAX: i32 = 0x3FFF_FFFF;

    /// Minimum value for i31 (-2^30)
    pub const MIN: i32 = -0x4000_0000;

    /// Create an i31ref from a 32-bit integer, truncating to 31 bits
    #[inline]
    pub const fn new(value: i32) -> Self {
        // Sign-extend from 31 bits
        let truncated = (value << 1) >> 1;
        Self { value: truncated }
    }

    /// Get the signed 31-bit value
    #[inline]
    pub const fn get_s(&self) -> i32 {
        self.value
    }

    /// Get the unsigned 31-bit value
    #[inline]
    pub const fn get_u(&self) -> u32 {
        (self.value & 0x7FFF_FFFF) as u32
    }
}

impl Default for I31Ref {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_ref_null() {
        let r = GcRef::null();
        assert!(r.is_null());
        assert_eq!(r.offset(), None);
    }

    #[test]
    fn test_gc_ref_from_offset() {
        let r = GcRef::from_offset(100);
        assert!(!r.is_null());
        assert_eq!(r.offset(), Some(100));
    }

    #[test]
    fn test_i31_ref() {
        let r = I31Ref::new(42);
        assert_eq!(r.get_s(), 42);
        assert_eq!(r.get_u(), 42);

        let neg = I31Ref::new(-1);
        assert_eq!(neg.get_s(), -1);

        // Test truncation to 31 bits
        let overflow = I31Ref::new(I31Ref::MAX + 1);
        assert_eq!(overflow.get_s(), I31Ref::MIN);
    }
}
