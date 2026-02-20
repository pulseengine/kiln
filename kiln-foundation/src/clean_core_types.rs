//! Clean core WebAssembly type definitions without provider parameters
//!
//! This module provides provider-agnostic core WebAssembly types.
//! These are the actual types used in WebAssembly modules (not component model
//! types).
//!
//! Note: This module requires allocation capabilities (std or alloc feature).

#[cfg(any(feature = "std", feature = "alloc"))]
pub use types::*;

#[cfg(any(feature = "std", feature = "alloc"))]
mod types {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::vec::Vec;
    #[cfg(feature = "std")]
    use std::vec::Vec;

    use crate::types::ValueType;

    /// Clean core WebAssembly function type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct CoreFuncType {
        /// Function parameter types
        pub params:  Vec<ValueType>,
        /// Function result types  
        pub results: Vec<ValueType>,
    }

    /// Clean core WebAssembly memory type without provider parameters
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CoreMemoryType {
        /// Memory limits
        pub limits: crate::types::Limits,
        /// Whether the memory is shared
        pub shared: bool,
    }

    /// Clean core WebAssembly table type without provider parameters
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CoreTableType {
        /// Element type (funcref or externref)
        pub element_type: crate::types::RefType,
        /// Table size limits
        pub limits:       crate::types::Limits,
    }

    /// Clean core WebAssembly global type without provider parameters
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CoreGlobalType {
        /// Value type of the global
        pub value_type: ValueType,
        /// Whether the global is mutable
        pub mutable:    bool,
    }
}
