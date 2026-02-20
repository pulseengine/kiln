//! Bounded infrastructure for intercept system
//!
//! This module provides bounded alternatives for intercept collections
//! ensuring static memory allocation.

#[cfg(not(feature = "std"))]
extern crate alloc;

use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    bounded_collections::BoundedMap,
    safe_memory::NoStdProvider,
};

/// Default memory provider for intercept
pub type InterceptProvider = NoStdProvider<16384>; // 16KB for intercept

/// Maximum number of intercepted functions
pub const MAX_INTERCEPTED_FUNCTIONS: usize = 256;

/// Maximum number of function stats
pub const MAX_FUNCTION_STATS: usize = 512;

/// Maximum number of executing functions
pub const MAX_EXECUTING_FUNCTIONS: usize = 64;

/// Maximum function name length
pub const MAX_FUNCTION_NAME_LEN: usize = 128;

/// Bounded map for function stats
pub type BoundedStatsMap = BoundedMap<
    BoundedString<MAX_FUNCTION_NAME_LEN>,
    crate::strategies::FunctionStats,
    MAX_FUNCTION_STATS,
    InterceptProvider,
>;

/// Bounded map for executing functions
pub type BoundedExecutingMap = BoundedMap<
    u64, // Thread ID
    BoundedString<MAX_FUNCTION_NAME_LEN>,
    MAX_EXECUTING_FUNCTIONS,
    InterceptProvider,
>;

/// Bounded vector for results
pub type BoundedResultVec<T> = BoundedVec<T, 256, InterceptProvider>;

/// Create a new bounded stats map
///
/// # Errors
/// Returns error if provider allocation fails
pub fn new_stats_map() -> Result<BoundedStatsMap, wrt_error::Error> {
    // For safety-critical code that forbids unsafe, use direct provider creation
    let provider = InterceptProvider::default();
    BoundedMap::new(provider)
}

/// Create a new bounded executing map
///
/// # Errors
/// Returns error if provider allocation fails
pub fn new_executing_map() -> Result<BoundedExecutingMap, wrt_error::Error> {
    // For safety-critical code that forbids unsafe, use direct provider creation
    let provider = InterceptProvider::default();
    BoundedMap::new(provider)
}

/// Create a new bounded result vector
///
/// # Errors
/// Returns error if provider allocation fails
pub fn new_result_vec<T>() -> Result<BoundedResultVec<T>, wrt_error::Error>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    // For safety-critical code that forbids unsafe, use direct provider creation
    let provider = InterceptProvider::default();
    BoundedVec::new(provider)
}
