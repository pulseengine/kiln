//! WRT Compile-Time Allocator System
//!
//! This module provides the revolutionary compile-time memory allocation system
//! that enables A+ functional safety compliance with zero runtime overhead.
//!
//! # Features
//!
//! - **Compile-time budget verification** - Memory violations caught at build
//!   time
//! - **Zero runtime overhead** - No performance cost for safety
//! - **Type-level memory tracking** - Phantom types carry budget information
//! - **Industry-leading safety** - Exceeds AUTOSAR, DO-178C, QNX standards
//!
//! # Usage
//!
//! ```rust
//! use kiln_foundation::allocator::{
//!     CrateId,
//!     KilnHashMap,
//!     KilnVec,
//! };
//!
//! // Compile-time verified collections
//! let mut vec: KilnVec<i32, { CrateId::Component as u8 }, 1000> = KilnVec::new();
//! let mut map: KilnHashMap<String, Data, { CrateId::Component as u8 }, 256> = KilnHashMap::new();
//!
//! // Works exactly like std collections but with compile-time safety
//! vec.push(42)?;
//! map.insert("key".to_string(), data)?;
//! ```

#[cfg(feature = "kiln-allocator")]
pub mod collections;

#[cfg(feature = "kiln-allocator")]
pub mod phantom_budgets;

#[cfg(all(not(feature = "kiln-allocator"), feature = "std"))]
pub use std::collections::HashMap as KilnHashMap;
// Re-export for convenience when not using the allocator feature
#[cfg(all(not(feature = "kiln-allocator"), feature = "std"))]
pub use std::vec::Vec as KilnVec;

#[cfg(feature = "kiln-allocator")]
pub use collections::aliases::{
    ComponentHashMap,
    ComponentString,
    ComponentVec,
    FoundationHashMap,
    FoundationString,
    FoundationVec,
    HostHashMap,
    HostString,
    HostVec,
    RuntimeHashMap,
    RuntimeString,
    RuntimeVec,
};
#[cfg(feature = "kiln-allocator")]
pub use collections::{
    KilnHashMap,
    KilnString,
    KilnVec,
};
#[cfg(feature = "kiln-allocator")]
pub use phantom_budgets::{
    CapacityError,
    CrateId,
    CRATE_BUDGETS,
};

// For no_std without allocator feature, use bounded collections
#[cfg(all(not(feature = "kiln-allocator"), not(feature = "std")))]
pub use crate::bounded::BoundedVec as KilnVec;
#[cfg(all(not(feature = "kiln-allocator"), not(feature = "std")))]
pub use crate::bounded_collections::BoundedMap as KilnHashMap;
// Provide CrateId for non-allocator builds (for compatibility)
#[cfg(not(feature = "kiln-allocator"))]
pub use crate::budget_aware_provider::CrateId;
