//! Unified Type System for WRT Foundation
//!
//! This module provides platform-configurable bounded collections that resolve
//! type conflicts across the WRT ecosystem. It establishes a unified type
//! hierarchy that can be configured for different platform constraints while
//! maintaining type consistency.


use core::marker::PhantomData;

use crate::{
    bounded::{BoundedString, BoundedVec},
    safe_memory::{NoStdProvider, DefaultNoStdProvider},
    Error, wrt_error::Result,
};

/// Platform capacity configuration for unified types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapacities {
    pub small_capacity: usize,
    pub medium_capacity: usize,
    pub large_capacity: usize,
    pub memory_provider_size: usize,
}

impl Default for PlatformCapacities {
    fn default() -> Self {
        Self {
            small_capacity: 64,
            medium_capacity: 1024,
            large_capacity: 65536,
            memory_provider_size: 8192,
        }
    }
}

impl PlatformCapacities {
    pub const fn embedded() -> Self {
        Self {
            small_capacity: 16,
            medium_capacity: 128,
            large_capacity: 1024,
            memory_provider_size: 2048,
        }
    }

    pub const fn desktop() -> Self {
        Self {
            small_capacity: 256,
            medium_capacity: 4096,
            large_capacity: 1048576,
            memory_provider_size: 65536,
        }
    }

    pub const fn safety_critical() -> Self {
        Self {
            small_capacity: 32,
            medium_capacity: 256,
            large_capacity: 8192,
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

/// Unified type system with platform-configurable bounded collections
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
            small_capacity: SMALL,
            medium_capacity: MEDIUM,
            large_capacity: LARGE,
            memory_provider_size: 8192,
        }
    }
}

/// Default unified types configuration
pub type DefaultTypes = UnifiedTypes<64, 1024, 65536>;

/// Embedded systems configuration
pub type EmbeddedTypes = UnifiedTypes<16, 128, 1024>;

/// Desktop/server configuration
pub type DesktopTypes = UnifiedTypes<256, 4096, 1048576>;

/// Safety-critical configuration
pub type SafetyCriticalTypes = UnifiedTypes<32, 256, 8192>;

/// Helper trait for creating unified type collections
pub trait UnifiedTypeFactory<const SMALL: usize, const MEDIUM: usize, const LARGE: usize> {
    fn create_small_vec<T>() -> wrt_error::Result<BoundedVec<T, SMALL, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes;

    fn create_medium_vec<T>() -> wrt_error::Result<BoundedVec<T, MEDIUM, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes;

    fn create_large_vec<T>() -> wrt_error::Result<BoundedVec<T, LARGE, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes;

    fn create_runtime_string() -> wrt_error::Result<BoundedString<MEDIUM, DefaultNoStdProvider>>;
}

impl<const SMALL: usize, const MEDIUM: usize, const LARGE: usize> 
    UnifiedTypeFactory<SMALL, MEDIUM, LARGE> for UnifiedTypes<SMALL, MEDIUM, LARGE>
{
    fn create_small_vec<T>() -> wrt_error::Result<BoundedVec<T, SMALL, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        let provider = safe_managed_alloc!(SMALL * core::mem::size_of::<T>(), CrateId::Foundation)?;
        BoundedVec::new(provider)
    }

    fn create_medium_vec<T>() -> wrt_error::Result<BoundedVec<T, MEDIUM, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        let provider = safe_managed_alloc!(MEDIUM * core::mem::size_of::<T>(), CrateId::Foundation)?;
        BoundedVec::new(provider)
    }

    fn create_large_vec<T>() -> wrt_error::Result<BoundedVec<T, LARGE, DefaultNoStdProvider>>
    where
        T: Clone + core::fmt::Debug + Default + PartialEq + Eq + crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        let provider = safe_managed_alloc!(LARGE * core::mem::size_of::<T>(), CrateId::Foundation)?;
        BoundedVec::new(provider)
    }

    fn create_runtime_string() -> wrt_error::Result<BoundedString<MEDIUM, DefaultNoStdProvider>> {
        let provider = safe_managed_alloc!(MEDIUM, CrateId::Foundation)?;
        BoundedString::try_from_str("", provider).map_err(|_| Error::runtime_execution_error("Failed to create runtime string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capacities_validation() {
        let valid_caps = PlatformCapacities::default());
        assert!(valid_caps.validate();

        let embedded_caps = PlatformCapacities::embedded);
        assert!(embedded_caps.validate();

        let desktop_caps = PlatformCapacities::desktop);
        assert!(desktop_caps.validate();

        let safety_caps = PlatformCapacities::safety_critical);
        assert!(safety_caps.validate();
    }

    #[test]
    fn test_unified_types_configuration_validation() {
        assert!(DefaultTypes::validate_configuration();
        assert!(EmbeddedTypes::validate_configuration();
        assert!(DesktopTypes::validate_configuration();
        assert!(SafetyCriticalTypes::validate_configuration();
    }

    #[test]
    fn test_capacities() {
        let default_caps = DefaultTypes::capacities);
        assert_eq!(default_caps.small_capacity, 64;
        assert_eq!(default_caps.medium_capacity, 1024;
        assert_eq!(default_caps.large_capacity, 65536;

        let embedded_caps = EmbeddedTypes::capacities);
        assert_eq!(embedded_caps.small_capacity, 16;
        assert_eq!(embedded_caps.medium_capacity, 128;
        assert_eq!(embedded_caps.large_capacity, 1024;
    }
}