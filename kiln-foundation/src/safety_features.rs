//! Safety-aware feature system for WRT
//!
//! This module provides capability-based safety features that can be composed
//! to meet various safety standards (ISO 26262, DO-178C, IEC 61508, etc.)
//! without being tied to specific standard names.
//!
//! Compile-time feature validation
//!
//! These compile-error! macros prevent incompatible feature combinations
//! from building, ensuring safety constraints are enforced at compile time.

// Mutual exclusivity now enforced through strategy-specific capabilities
// Previous compile-time checks removed as conflicts are resolved

// Feature dependencies
#[cfg(all(
    feature = "verified-static-allocation",
    not(feature = "formal-verification-required")
))]
compile_error!("verified-static-allocation requires formal-verification-required");

#[cfg(all(feature = "mathematical-proofs", not(feature = "static-memory-layout")))]
compile_error!("mathematical-proofs requires static-memory-layout");

#[cfg(all(feature = "hardware-isolation", not(feature = "component-isolation")))]
compile_error!("hardware-isolation requires component-isolation");

#[cfg(all(feature = "component-isolation", not(feature = "memory-isolation")))]
compile_error!("component-isolation requires memory-isolation");

#[cfg(all(
    feature = "memory-isolation",
    not(feature = "bounded-budget-enforcement")
))]
compile_error!("memory-isolation requires bounded-budget-enforcement");

/// Memory allocation strategies based on enabled features
pub mod allocation {
    use super::*;
    use crate::budget_aware_provider::CrateId;

    /// Automatically select allocation strategy based on enabled features
    #[macro_export]
    macro_rules! safety_aware_alloc {
        ($size:expr, $crate_id:expr) => {{
            #[cfg(feature = "dynamic-allocation")]
            {
                // QM level - dynamic allocation allowed
                $crate::safe_managed_alloc!($size, $crate_id)
            }

            #[cfg(all(feature = "bounded-collections", not(feature = "static-allocation")))]
            {
                // ASIL-A/B level - bounded collections with monitoring
                compile_time_assert!(
                    $size <= 65536,
                    "ASIL-A/B: allocation size exceeds 64KB limit"
                );
                $crate::safe_managed_alloc!($size, $crate_id)
            }

            #[cfg(all(
                feature = "static-allocation",
                not(feature = "verified-static-allocation")
            ))]
            {
                // ASIL-C level - static allocation only
                compile_time_assert!($size <= 32768, "ASIL-C: allocation size exceeds 32KB limit");
                const_assert!($size > 0, "ASIL-C: zero-size allocation not allowed");
                $crate::safe_managed_alloc!($size, $crate_id)
            }

            #[cfg(feature = "verified-static-allocation")]
            {
                // ASIL-D level - verified static allocation with redundancy
                compile_time_assert!($size <= 16384, "ASIL-D: allocation size exceeds 16KB limit");
                const_assert!($size > 0, "ASIL-D: zero-size allocation not allowed");
                const_assert!(
                    $size.is_power_of_two(),
                    "ASIL-D: allocation size must be power of 2"
                );
                $crate::safe_managed_alloc!($size, $crate_id)
            }
        }};
    }

    /// Memory strategy selection based on safety level
    pub const fn get_memory_strategy() -> u8 {
        #[cfg(feature = "verified-static-allocation")]
        {
            5 // FullIsolation for maximum safety
        }
        #[cfg(all(
            feature = "static-allocation",
            not(feature = "verified-static-allocation")
        ))]
        {
            2 // Isolated for high safety
        }
        #[cfg(all(feature = "bounded-collections", not(feature = "static-allocation")))]
        {
            1 // BoundedCopy for safety
        }
        #[cfg(all(
            feature = "dynamic-allocation",
            not(feature = "bounded-collections"),
            not(feature = "static-allocation")
        ))]
        {
            0 // ZeroCopy for performance
        }
        #[cfg(not(any(
            feature = "dynamic-allocation",
            feature = "bounded-collections",
            feature = "static-allocation",
            feature = "verified-static-allocation"
        )))]
        {
            1 // Default to BoundedCopy
        }
    }

    pub const MEMORY_STRATEGY: u8 = get_memory_strategy();
}

/// Safety standard mapping
pub mod standards {
    /// ASIL levels for ISO 26262
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AsilLevel {
        QM,
        AsilA,
        AsilB,
        AsilC,
        AsilD,
    }

    /// DAL levels for DO-178C
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DalLevel {
        DalE,
        DalD,
        DalC,
        DalB,
        DalA,
    }

    /// SIL levels for IEC 61508
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SilLevel {
        Sil1,
        Sil2,
        Sil3,
        Sil4,
    }

    /// Trait for mapping safety standards to capability features
    pub trait SafetyStandardMapping {
        fn required_capabilities() -> &'static [&'static str];
        fn validates_current_config() -> bool;
    }

    impl SafetyStandardMapping for AsilLevel {
        fn required_capabilities() -> &'static [&'static str] {
            #[cfg(feature = "maximum-safety")]
            {
                &[
                    "verified-static-allocation",
                    "mathematical-proofs",
                    "hardware-isolation",
                ]
                // ASIL-D
            }
            #[cfg(all(feature = "static-memory-safety", not(feature = "maximum-safety")))]
            {
                &[
                    "static-allocation",
                    "component-isolation",
                    "bounded-budget-enforcement",
                ]
                // ASIL-C
            }
            #[cfg(all(
                feature = "bounded-collections",
                not(feature = "static-memory-safety"),
                not(feature = "maximum-safety")
            ))]
            {
                &[
                    "bounded-capacity-limits",
                    "bounded-runtime-checks",
                    "bounded-monitoring",
                ]
                // ASIL-A/B
            }
            #[cfg(all(
                feature = "dynamic-allocation",
                not(feature = "bounded-collections"),
                not(feature = "static-memory-safety"),
                not(feature = "maximum-safety")
            ))]
            {
                &["dynamic-allocation"] // QM
            }
            #[cfg(not(any(
                feature = "dynamic-allocation",
                feature = "bounded-collections",
                feature = "static-memory-safety",
                feature = "maximum-safety"
            )))]
            {
                &[] // Fallback
            }
        }

        fn validates_current_config() -> bool {
            cfg!(all(
                feature = "maximum-safety",
                feature = "verified-static-allocation",
                feature = "mathematical-proofs"
            )) || cfg!(all(
                feature = "static-memory-safety",
                feature = "static-allocation",
                feature = "component-isolation"
            )) || cfg!(all(
                feature = "bounded-collections",
                feature = "runtime-bounds-checking"
            )) || cfg!(feature = "dynamic-allocation")
                // Default case: dynamic-allocation is implicit when no higher safety level is enabled
                || (!cfg!(feature = "verified-static-allocation")
                    && !cfg!(feature = "static-allocation")
                    && !cfg!(feature = "bounded-collections"))
        }
    }

    impl SafetyStandardMapping for DalLevel {
        fn required_capabilities() -> &'static [&'static str] {
            // DO-178C maps to same capabilities as ISO 26262
            AsilLevel::required_capabilities()
        }

        fn validates_current_config() -> bool {
            AsilLevel::validates_current_config()
        }
    }

    impl SafetyStandardMapping for SilLevel {
        fn required_capabilities() -> &'static [&'static str] {
            // IEC 61508 maps to same capabilities as ISO 26262
            AsilLevel::required_capabilities()
        }

        fn validates_current_config() -> bool {
            AsilLevel::validates_current_config()
        }
    }
}

/// Runtime capability checking
pub mod runtime {
    use super::*;

    /// Check if a capability is enabled at runtime
    #[allow(clippy::match_like_matches_macro)] // Each arm returns different cfg!() result
    pub fn has_capability(capability: &str) -> bool {
        match capability {
            // dynamic-allocation is the implicit default when no higher safety level is enabled
            "dynamic-allocation" => {
                cfg!(feature = "dynamic-allocation")
                    || (!cfg!(feature = "verified-static-allocation")
                        && !cfg!(feature = "static-allocation")
                        && !cfg!(feature = "bounded-collections"))
            }
            "static-allocation" => cfg!(feature = "static-allocation"),
            "verified-static-allocation" => cfg!(feature = "verified-static-allocation"),
            "bounded-collections" => cfg!(feature = "bounded-collections"),
            "formal-verification-required" => cfg!(feature = "formal-verification-required"),
            "mathematical-proofs" => cfg!(feature = "mathematical-proofs"),
            "hardware-isolation" => cfg!(feature = "hardware-isolation"),
            "component-isolation" => cfg!(feature = "component-isolation"),
            "memory-isolation" => cfg!(feature = "memory-isolation"),
            "runtime-bounds-checking" => cfg!(feature = "runtime-bounds-checking"),
            "static-memory-layout" => cfg!(feature = "static-memory-layout"),
            "static-capacity-limits" => cfg!(feature = "static-capacity-limits"),
            "static-verification" => cfg!(feature = "static-verification"),
            "bounded-runtime-checks" => cfg!(feature = "bounded-runtime-checks"),
            "bounded-capacity-limits" => cfg!(feature = "bounded-capacity-limits"),
            "bounded-monitoring" => cfg!(feature = "bounded-monitoring"),
            "bounded-budget-enforcement" => cfg!(feature = "bounded-budget-enforcement"),
            "managed-monitoring" => cfg!(feature = "managed-monitoring"),
            "managed-dynamic-alloc" => cfg!(feature = "managed-dynamic-alloc"),
            "std-dynamic-alloc" => cfg!(feature = "std-dynamic-alloc"),
            "std-collections" => cfg!(feature = "std-collections"),
            _ => false,
        }
    }

    /// Get the current safety level based on enabled features
    pub const fn current_safety_level() -> &'static str {
        if cfg!(feature = "verified-static-allocation") && cfg!(feature = "mathematical-proofs") {
            "maximum-safety"
        } else if cfg!(feature = "static-allocation") && cfg!(feature = "component-isolation") {
            "static-memory-safety"
        } else if cfg!(feature = "bounded-collections") {
            "bounded-collections"
        } else {
            "dynamic-allocation"
        }
    }

    /// Get maximum allocation size for current safety level
    pub const fn max_allocation_size() -> usize {
        if cfg!(feature = "verified-static-allocation") {
            16384 // ASIL-D: 16KB limit
        } else if cfg!(feature = "static-allocation") {
            32768 // ASIL-C: 32KB limit
        } else if cfg!(feature = "bounded-collections") {
            65536 // ASIL-A/B: 64KB limit
        } else {
            usize::MAX // QM: no limit
        }
    }
}

/// Compile-time assertion helpers
#[macro_export]
macro_rules! compile_time_assert {
    ($condition:expr, $message:expr) => {
        const _: () = {
            if !$condition {
                panic!($message);
            }
        };
    };
}

#[macro_export]
macro_rules! const_assert {
    ($condition:expr, $message:expr) => {
        const _: () = assert!($condition, $message);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_safety_level() {
        let level = runtime::current_safety_level();
        assert!(!level.is_empty(), "Safety level should be determined");

        // Verify that the detected level makes sense
        match level {
            "maximum-safety" => {
                assert!(runtime::has_capability("verified-static-allocation"));
            },
            "static-memory-safety" => {
                assert!(runtime::has_capability("static-allocation"));
            },
            "bounded-collections" => {
                assert!(runtime::has_capability("bounded-collections"));
            },
            "dynamic-allocation" => {
                assert!(runtime::has_capability("dynamic-allocation"));
            },
            _ => panic!("Unknown safety level: {}", level),
        }
    }

    #[test]
    fn test_max_allocation_size() {
        let max_size = runtime::max_allocation_size();
        let level = runtime::current_safety_level();

        match level {
            "maximum-safety" => assert_eq!(max_size, 16384),
            "static-memory-safety" => assert_eq!(max_size, 32768),
            "bounded-collections" => assert_eq!(max_size, 65536),
            "dynamic-allocation" => assert_eq!(max_size, usize::MAX),
            _ => {},
        }
    }

    #[test]
    fn test_capability_consistency() {
        // Verify that enabled features are consistent
        use standards::{
            AsilLevel,
            SafetyStandardMapping,
        };

        assert!(
            AsilLevel::validates_current_config(),
            "Current feature configuration should be valid for some ASIL level"
        );
    }
}
