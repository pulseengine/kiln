//! Compile-time memory sizing strategy for efficient allocation
//!
//! This module provides a systematic approach to memory allocation sizes,
//! reducing waste from static allocation while maintaining no_std
//! compatibility.

use crate::safe_memory::NoStdProvider;

/// Memory size classes for common allocation patterns
///
/// These sizes are carefully chosen based on analysis of actual usage patterns
/// to minimize waste while providing adequate capacity for common operations.
pub mod size_classes {
    /// Tiny allocations (< 256 bytes) - for small metadata, flags, counters
    pub const TINY: usize = 256;

    /// Small allocations (< 1KB) - for bounded strings, small collections
    pub const SMALL: usize = 1024;

    /// Medium allocations (< 4KB) - for moderate collections, buffers
    pub const MEDIUM: usize = 4096;

    /// Large allocations (< 16KB) - for larger data structures
    pub const LARGE: usize = 16384;

    /// Extra large allocations (< 64KB) - for component models, large buffers
    pub const XLARGE: usize = 65536;

    /// Huge allocations (< 256KB) - for entire modules or very large data
    pub const HUGE: usize = 262144;
}

/// Type aliases for common provider sizes
pub type TinyProvider = NoStdProvider<{ size_classes::TINY }>;
pub type SmallProvider = NoStdProvider<{ size_classes::SMALL }>;
pub type MediumProvider = NoStdProvider<{ size_classes::MEDIUM }>;
pub type LargeProvider = NoStdProvider<{ size_classes::LARGE }>;
pub type XLargeProvider = NoStdProvider<{ size_classes::XLARGE }>;
pub type HugeProvider = NoStdProvider<{ size_classes::HUGE }>;

/// Helper trait to select appropriate provider size at compile time
pub trait SizeSelector {
    /// The selected provider size
    const SIZE: usize;

    /// Type alias for the provider with this size
    type Provider;
}

/// Macro to implement SizeSelector for specific types
#[macro_export]
macro_rules! impl_size_selector {
    ($type:ty, $size:expr) => {
        impl $crate::memory_sizing::SizeSelector for $type {
            type Provider = $crate::safe_memory::NoStdProvider<$size>;

            const SIZE: usize = $size;
        }
    };
}

/// Size recommendations for common WRT types
pub mod recommendations {
    use super::size_classes;

    /// BoundedString typical sizes
    pub const STRING_SHORT: usize = size_classes::TINY; // 256B for identifiers
    pub const STRING_MEDIUM: usize = size_classes::SMALL; // 1KB for descriptions
    pub const STRING_LONG: usize = size_classes::MEDIUM; // 4KB for text content

    /// BoundedVec typical sizes
    pub const VEC_SMALL: usize = size_classes::SMALL; // 1KB for small lists
    pub const VEC_MEDIUM: usize = size_classes::MEDIUM; // 4KB for moderate lists
    pub const VEC_LARGE: usize = size_classes::LARGE; // 16KB for large collections

    /// Function and type representations
    pub const FUNC_TYPE: usize = size_classes::SMALL; // 1KB for function signatures
    pub const MODULE_TYPES: usize = size_classes::XLARGE; // 64KB for module type tables

    /// Debug information
    pub const DEBUG_ABBREV: usize = size_classes::MEDIUM; // 4KB for DWARF abbreviations
    pub const DEBUG_INFO: usize = size_classes::LARGE; // 16KB for debug
                                                       // sections
}

/// Calculate required size based on expected usage
/// This allows compile-time size optimization
pub const fn calculate_required_size(
    expected_elements: usize,
    element_size: usize,
    overhead_percent: usize,
) -> usize {
    let base_size = expected_elements * element_size;
    let overhead = base_size * overhead_percent / 100;
    let total = base_size + overhead;

    // Round up to nearest size class
    if total <= size_classes::TINY {
        size_classes::TINY
    } else if total <= size_classes::SMALL {
        size_classes::SMALL
    } else if total <= size_classes::MEDIUM {
        size_classes::MEDIUM
    } else if total <= size_classes::LARGE {
        size_classes::LARGE
    } else if total <= size_classes::XLARGE {
        size_classes::XLARGE
    } else {
        size_classes::HUGE
    }
}

/// Macro to create a sized provider based on expected usage
#[macro_export]
macro_rules! sized_provider {
    // For collections with known element count and size
    (collection: $elements:expr, $elem_size:expr) => {
        $crate::safe_memory::NoStdProvider::<
            { $crate::memory_sizing::calculate_required_size($elements, $elem_size, 20) },
        >::default()
    };

    // For strings with maximum length
    (string: $max_len:expr) => {
        $crate::safe_memory::NoStdProvider::<
            { $crate::memory_sizing::calculate_required_size($max_len, 1, 10) },
        >::default()
    };

    // For buffers with specific size
    (buffer: $size:expr) => {
        $crate::safe_memory::NoStdProvider::<
            { $crate::memory_sizing::calculate_required_size($size, 1, 0) },
        >::default()
    };

    // Use predefined size class
    (class: tiny) => {
        $crate::memory_sizing::TinyProvider::default()
    };
    (class: small) => {
        $crate::memory_sizing::SmallProvider::default()
    };
    (class: medium) => {
        $crate::memory_sizing::MediumProvider::default()
    };
    (class: large) => {
        $crate::memory_sizing::LargeProvider::default()
    };
    (class: xlarge) => {
        $crate::memory_sizing::XLargeProvider::default()
    };
    (class: huge) => {
        $crate::memory_sizing::HugeProvider::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_calculation() {
        // Test that size calculation rounds up appropriately
        assert_eq!(calculate_required_size(10, 10, 20), size_classes::TINY); // 120 -> 256
        assert_eq!(calculate_required_size(100, 8, 20), size_classes::SMALL); // 960 -> 1024
        assert_eq!(calculate_required_size(1000, 4, 20), size_classes::MEDIUM); // 4800 -> 4096 (already over)
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_size_classes() {
        // Verify size classes follow expected pattern
        // These are compile-time constant checks that verify the sizing hierarchy
        assert!(size_classes::TINY < size_classes::SMALL);
        assert!(size_classes::SMALL < size_classes::MEDIUM);
        assert!(size_classes::MEDIUM < size_classes::LARGE);
        assert!(size_classes::LARGE < size_classes::XLARGE);
        assert!(size_classes::XLARGE < size_classes::HUGE);
    }
}
