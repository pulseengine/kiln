//! Type conversion utilities for the Kiln runtime
//!
//! This module provides conversion functions between different type
//! representations used throughout the Kiln execution pipeline.

pub mod locals_conversion;
pub mod slice_adapter;

pub use locals_conversion::{convert_locals_to_bounded, convert_locals_to_bounded_with_provider};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use locals_conversion::expand_locals_to_flat;
pub use slice_adapter::{
    adapt_slice_to_bounded,
    SliceAdapter,
};
