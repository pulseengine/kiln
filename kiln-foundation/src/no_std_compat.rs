//! No-std compatibility utilities
//!
//! This module provides compatibility macros and utilities for no_std environments,
//! including alternatives to std macros like vec! and format!.

use crate::prelude::*;
use crate::bounded::{BoundedVec, BoundedString};
use crate::traits::Checksummable;
use crate::NoStdProvider;

/// Creates a `BoundedVec` in no_std environments, similar to `vec!` macro
/// 
/// # Examples
/// ```
/// # use wrt_foundation::no_std_compat::bounded_vec;
/// # use wrt_foundation::NoStdProvider;
/// let v = bounded_vec![NoStdProvider::default(); 1, 2, 3];
/// ```
#[macro_export]
macro_rules! bounded_vec {
    // Empty vector
    ($provider:expr) => {
        $crate::bounded::BoundedVec::new($provider)
    };
    
    // Vector with elements
    ($provider:expr; $($elem:expr),* $(,)?) => {{
        let provider = $provider;
        let mut vec = $crate::bounded::BoundedVec::new(provider)
            .expect("Failed to create BoundedVec");
        $(
            vec.push($elem).expect("Failed to push to BoundedVec");
        )*
        vec
    }};
    
    // Vector with repeated element
    ($provider:expr; $elem:expr; $n:expr) => {{
        let provider = $provider;
        let mut vec = $crate::bounded::BoundedVec::new(provider)
            .expect("Failed to create BoundedVec");
        for _ in 0..$n {
            vec.push($elem.clone()).expect("Failed to push to BoundedVec");
        }
        vec
    }};
}

/// Creates a formatted `BoundedString` in no_std environments
/// 
/// Note: This is a simplified version that only supports basic formatting
#[macro_export]
macro_rules! bounded_format {
    // Just a literal string
    ($provider:expr, $lit:literal) => {{
        $crate::bounded::BoundedString::try_from_str($lit, $provider)
            .expect("Failed to create BoundedString")
    }};
    
    // For now, more complex formatting returns a static error message
    ($provider:expr, $fmt:literal, $($arg:expr),*) => {{
        // Binary std/no_std choice
        // Return a placeholder message
        $crate::bounded::BoundedString::try_from_str(
            "[formatting not available in no_std]", 
            $provider
        ).expect("Failed to create BoundedString")
    }};
}

// Remove problematic type aliases, provide concrete helpers instead

/// Helper to create a `BoundedVec` with standard capacity and default provider
pub fn create_bounded_vec<T>() -> wrt_error::Result<BoundedVec<T, 1024, NoStdProvider<{1024 * 8}>>>
where
    T: Sized + Checksummable + crate::traits::ToBytes + crate::traits::FromBytes + Default + Clone + PartialEq + Eq,
{
    BoundedVec::new(NoStdProvider::default()).map_err(|e| {
        crate::Error::runtime_execution_error("Failed to create bounded vector with default provider")
    })
}

/// Helper to create an empty `BoundedString` with default provider  
pub fn create_bounded_string() -> wrt_error::Result<BoundedString<256, NoStdProvider<256>>> {
    BoundedString::from_str_truncate("").map_err(|e| {
        crate::Error::runtime_execution_error("Failed to create empty bounded string")
    })
}

/// Helper to create `BoundedString` from `&str`
pub fn create_bounded_string_from(s: &str) -> wrt_error::Result<BoundedString<256, NoStdProvider<256>>> {
    BoundedString::try_from_str(s, NoStdProvider::default()).map_err(|e| {
        crate::Error::new(
            crate::ErrorCategory::Parse,
            crate::codes::SERIALIZATION_ERROR,
            "Failed to create bounded string from input string")
    })
}

// Re-export for convenience
pub use bounded_vec;
pub use bounded_format;