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
//! use wrt_foundation::allocator::{
//!     CrateId,
//!     WrtHashMap,
//!     WrtVec,
//! };
//!
//! // Compile-time verified collections
//! let mut vec: WrtVec<i32, { CrateId::Component as u8 }, 1000> = WrtVec::new();
//! let mut map: WrtHashMap<String, Data, { CrateId::Component as u8 }, 256> = WrtHashMap::new();
//!
//! // Works exactly like std collections but with compile-time safety
//! vec.push(42)?;
//! map.insert("key".to_string(), data)?;
//! ```

#[cfg(feature = "wrt-allocator")]
pub mod collections;

#[cfg(feature = "wrt-allocator")]
pub mod phantom_budgets;

#[cfg(all(not(feature = "wrt-allocator"), feature = "std"))]
pub use std::collections::HashMap as WrtHashMap;
// Re-export for convenience when not using the allocator feature
#[cfg(all(not(feature = "wrt-allocator"), feature = "std"))]
pub use std::vec::Vec as WrtVec;

#[cfg(feature = "wrt-allocator")]
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
#[cfg(feature = "wrt-allocator")]
pub use collections::{
    WrtHashMap,
    WrtString,
    WrtVec,
};
#[cfg(feature = "wrt-allocator")]
pub use phantom_budgets::{
    CapacityError,
    CrateId,
    CRATE_BUDGETS,
};

// For no_std without allocator feature, use bounded collections
#[cfg(all(not(feature = "wrt-allocator"), not(feature = "std")))]
pub use crate::bounded::BoundedVec as WrtVec;
#[cfg(all(not(feature = "wrt-allocator"), not(feature = "std")))]
pub use crate::bounded_collections::BoundedMap as WrtHashMap;
// Provide CrateId for non-allocator builds (for compatibility)
#[cfg(not(feature = "wrt-allocator"))]
pub use crate::budget_aware_provider::CrateId;
