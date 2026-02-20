// WRT - wrt-foundation
// Module: Simplified Unified Type System
// SW-REQ-ID: REQ_TYPE_UNIFIED_001, REQ_TYPE_PLATFORM_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Simplified Unified Type System for WRT Foundation
//!
//! This module provides a simplified version of the unified type system
//! that avoids complex type alias issues while still providing the
//! core functionality for Agent A deliverables.

use core::marker::PhantomData;

use crate::Error;

/// Platform capacity configuration for unified types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapacities {
    pub small_capacity:       usize,
    pub medium_capacity:      usize,
    pub large_capacity:       usize,
    pub memory_provider_size: usize,
}

impl Default for PlatformCapacities {
    fn default() -> Self {
        Self {
            small_capacity:       64,
            medium_capacity:      1024,
            large_capacity:       65536,
            memory_provider_size: 8192,
        }
    }
}

impl PlatformCapacities {
    pub const fn embedded() -> Self {
        Self {
            small_capacity:       16,
            medium_capacity:      128,
            large_capacity:       1024,
            memory_provider_size: 2048,
        }
    }

    pub const fn desktop() -> Self {
        Self {
            small_capacity:       256,
            medium_capacity:      4096,
            large_capacity:       1048576,
            memory_provider_size: 65536,
        }
    }

    pub const fn safety_critical() -> Self {
        Self {
            small_capacity:       32,
            medium_capacity:      256,
            large_capacity:       8192,
            memory_provider_size: 4096,
        }
    }

    pub const fn validate(&self) -> bool {
        self.small_capacity > 0
            && self.medium_capacity > self.small_capacity
            && self.large_capacity > self.medium_capacity
            && self.memory_provider_size >= self.large_capacity / 8
    }
}

/// Simplified unified type system
#[derive(Debug)]
pub struct UnifiedTypes<const SMALL: usize, const MEDIUM: usize, const LARGE: usize> {
    _phantom: PhantomData<()>,
}

impl<const SMALL: usize, const MEDIUM: usize, const LARGE: usize>
    UnifiedTypes<SMALL, MEDIUM, LARGE>
{
    pub const fn validate_configuration() -> bool {
        SMALL > 0 && MEDIUM > SMALL && LARGE > MEDIUM
    }

    pub const fn capacities() -> PlatformCapacities {
        PlatformCapacities {
            small_capacity:       SMALL,
            medium_capacity:      MEDIUM,
            large_capacity:       LARGE,
            memory_provider_size: 8192,
        }
    }
}

// Type aliases for different platform configurations
pub type DefaultTypes = UnifiedTypes<64, 1024, 65536>;
pub type EmbeddedTypes = UnifiedTypes<16, 128, 1024>;
pub type DesktopTypes = UnifiedTypes<256, 4096, 1048576>;
pub type SafetyCriticalTypes = UnifiedTypes<32, 256, 8192>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capacities_validation() {
        let valid_caps = PlatformCapacities::default();
        assert!(valid_caps.validate());

        let invalid_caps = PlatformCapacities {
            small_capacity:       100,
            medium_capacity:      50,
            large_capacity:       200,
            memory_provider_size: 1024,
        };
        assert!(!invalid_caps.validate());
    }

    #[test]
    fn test_unified_types_configuration() {
        assert!(DefaultTypes::validate_configuration());
        assert!(EmbeddedTypes::validate_configuration());
        assert!(DesktopTypes::validate_configuration());
        assert!(SafetyCriticalTypes::validate_configuration());
    }

    #[test]
    fn test_capacities() {
        let default_caps = DefaultTypes::capacities();
        assert_eq!(default_caps.small_capacity, 64);
        assert_eq!(default_caps.medium_capacity, 1024);
        assert_eq!(default_caps.large_capacity, 65536);

        let embedded_caps = EmbeddedTypes::capacities();
        assert_eq!(embedded_caps.small_capacity, 16);
        assert_eq!(embedded_caps.medium_capacity, 128);
        assert_eq!(embedded_caps.large_capacity, 1024);
    }
}
