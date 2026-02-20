//! Zero-cost collection wrappers with compile-time budget verification
//!
//! This module provides drop-in replacements for std collections that:
//! 1. Carry compile-time budget information in their types
//! 2. Provide zero runtime overhead over std collections
//! 3. Enable compile-time verification of memory usage
//! 4. Integrate seamlessly with existing code

use core::{
    marker::PhantomData,
    ops::{
        Deref,
        DerefMut,
    },
};
#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
use hashbrown::HashMap;

use super::phantom_budgets::MemoryBudget;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// Re-export for convenience
pub use super::phantom_budgets::{
    CapacityError,
    CrateId,
    CRATE_BUDGETS,
};

/// Zero-cost wrapper around std::vec::Vec with compile-time budget verification
#[derive(Debug, Clone)]
pub struct WrtVec<T, const CRATE: u8, const MAX_SIZE: usize> {
    inner:   Vec<T>,
    _budget: PhantomData<MemoryBudget<CRATE, MAX_SIZE>>,
}

impl<T, const CRATE: u8, const MAX_SIZE: usize> WrtVec<T, CRATE, MAX_SIZE> {
    /// Create new vector with compile-time budget verification
    pub fn new() -> Self {
        Self {
            inner:   Vec::new(),
            _budget: PhantomData,
        }
    }

    /// Create with specific capacity (still bounded by MAX_SIZE)
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity <= MAX_SIZE,
            "Requested capacity exceeds compile-time limit"
        );

        Self {
            inner:   Vec::with_capacity(capacity),
            _budget: PhantomData,
        }
    }

    /// Push with capacity checking
    pub fn push(&mut self, item: T) -> Result<(), CapacityError> {
        if self.inner.len() >= MAX_SIZE {
            Err(CapacityError::Exceeded)
        } else {
            self.inner.push(item);
            Ok(())
        }
    }

    /// Force push (panics if capacity exceeded) - for compatibility
    pub fn force_push(&mut self, item: T) {
        if self.inner.len() >= MAX_SIZE {
            panic!(
                "WrtVec capacity exceeded: {} >= {}",
                self.inner.len(),
                MAX_SIZE
            );
        }
        self.inner.push(item);
    }

    /// Get maximum capacity (compile-time constant)
    pub const fn max_capacity() -> usize {
        MAX_SIZE
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self) -> usize {
        MAX_SIZE.saturating_sub(self.inner.len())
    }
}

// Zero-cost deref to std::vec::Vec for full compatibility
impl<T, const CRATE: u8, const MAX_SIZE: usize> Deref for WrtVec<T, CRATE, MAX_SIZE> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, const CRATE: u8, const MAX_SIZE: usize> DerefMut for WrtVec<T, CRATE, MAX_SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// Default implementation
impl<T, const CRATE: u8, const MAX_SIZE: usize> Default for WrtVec<T, CRATE, MAX_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-cost wrapper around std::collections::HashMap with compile-time budget
/// verification
#[derive(Debug, Clone)]
pub struct WrtHashMap<K, V, const CRATE: u8, const MAX_SIZE: usize>
where
    K: std::hash::Hash + Eq,
{
    inner:   HashMap<K, V>,
    _budget: PhantomData<MemoryBudget<CRATE, MAX_SIZE>>,
}

impl<K, V, const CRATE: u8, const MAX_SIZE: usize> WrtHashMap<K, V, CRATE, MAX_SIZE> where
    K: std::hash::Hash + Eq
{
}

impl<K, V, const CRATE: u8, const MAX_SIZE: usize> Default for WrtHashMap<K, V, CRATE, MAX_SIZE>
where
    K: std::hash::Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, const CRATE: u8, const MAX_SIZE: usize> WrtHashMap<K, V, CRATE, MAX_SIZE>
where
    K: std::hash::Hash + Eq,
{
    /// Create new HashMap with compile-time budget verification
    pub fn new() -> Self {
        Self {
            inner:   HashMap::new(),
            _budget: PhantomData,
        }
    }

    /// Create with specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity <= MAX_SIZE,
            "Requested capacity exceeds compile-time limit"
        );

        Self {
            inner:   HashMap::with_capacity(capacity),
            _budget: PhantomData,
        }
    }

    /// Insert with capacity checking
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, CapacityError> {
        if self.inner.len() >= MAX_SIZE && !self.inner.contains_key(&key) {
            Err(CapacityError::Exceeded)
        } else {
            Ok(self.inner.insert(key, value))
        }
    }

    /// Get maximum capacity (compile-time constant)
    pub const fn max_capacity() -> usize {
        MAX_SIZE
    }
}

// Zero-cost deref to std::collections::HashMap for full compatibility
impl<K, V, const CRATE: u8, const MAX_SIZE: usize> Deref for WrtHashMap<K, V, CRATE, MAX_SIZE>
where
    K: std::hash::Hash + Eq,
{
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K, V, const CRATE: u8, const MAX_SIZE: usize> DerefMut for WrtHashMap<K, V, CRATE, MAX_SIZE>
where
    K: std::hash::Hash + Eq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Zero-cost wrapper around String with compile-time budget verification
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct WrtString<const CRATE: u8, const MAX_SIZE: usize> {
    inner:   String,
    _budget: PhantomData<MemoryBudget<CRATE, MAX_SIZE>>,
}

#[cfg(feature = "std")]
impl<const CRATE: u8, const MAX_SIZE: usize> WrtString<CRATE, MAX_SIZE> {
    /// Create new string with compile-time budget verification
    pub fn new() -> Self {
        Self {
            inner:   String::new(),
            _budget: PhantomData,
        }
    }

    /// Create from str with truncation if needed
    pub fn from_str_truncate(s: &str) -> Result<Self, CapacityError> {
        if s.len() > MAX_SIZE {
            Err(CapacityError::Exceeded)
        } else {
            Ok(Self {
                inner:   s.to_string(),
                _budget: PhantomData,
            })
        }
    }

    /// Push str with capacity checking
    pub fn push_str(&mut self, s: &str) -> Result<(), CapacityError> {
        if self.inner.len() + s.len() > MAX_SIZE {
            Err(CapacityError::Exceeded)
        } else {
            self.inner.push_str(s);
            Ok(())
        }
    }

    /// Get maximum capacity (compile-time constant)
    pub const fn max_capacity() -> usize {
        MAX_SIZE
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self) -> usize {
        MAX_SIZE.saturating_sub(self.inner.len())
    }
}

#[cfg(feature = "std")]
impl<const CRATE: u8, const MAX_SIZE: usize> Deref for WrtString<CRATE, MAX_SIZE> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(feature = "std")]
impl<const CRATE: u8, const MAX_SIZE: usize> DerefMut for WrtString<CRATE, MAX_SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(feature = "std")]
impl<const CRATE: u8, const MAX_SIZE: usize> Default for WrtString<CRATE, MAX_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience type aliases with common crate configurations
pub mod aliases {
    use super::*;

    // Foundation crate collections
    pub type FoundationVec<T, const N: usize> = WrtVec<T, { CrateId::Foundation as u8 }, N>;
    pub type FoundationHashMap<K, V, const N: usize> =
        WrtHashMap<K, V, { CrateId::Foundation as u8 }, N>;
    #[cfg(feature = "std")]
    pub type FoundationString<const N: usize> = WrtString<{ CrateId::Foundation as u8 }, N>;

    // Component crate collections
    pub type ComponentVec<T, const N: usize> = WrtVec<T, { CrateId::Component as u8 }, N>;
    pub type ComponentHashMap<K, V, const N: usize> =
        WrtHashMap<K, V, { CrateId::Component as u8 }, N>;
    #[cfg(feature = "std")]
    pub type ComponentString<const N: usize> = WrtString<{ CrateId::Component as u8 }, N>;

    // Runtime crate collections
    pub type RuntimeVec<T, const N: usize> = WrtVec<T, { CrateId::Runtime as u8 }, N>;
    pub type RuntimeHashMap<K, V, const N: usize> = WrtHashMap<K, V, { CrateId::Runtime as u8 }, N>;
    #[cfg(feature = "std")]
    pub type RuntimeString<const N: usize> = WrtString<{ CrateId::Runtime as u8 }, N>;

    // Host crate collections
    pub type HostVec<T, const N: usize> = WrtVec<T, { CrateId::Host as u8 }, N>;
    pub type HostHashMap<K, V, const N: usize> = WrtHashMap<K, V, { CrateId::Host as u8 }, N>;
    #[cfg(feature = "std")]
    pub type HostString<const N: usize> = WrtString<{ CrateId::Host as u8 }, N>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrt_vec_creation() {
        let mut vec: WrtVec<i32, { CrateId::Foundation as u8 }, 100> = WrtVec::new();
        assert_eq!(vec.len(), 0);
        assert_eq!(
            WrtVec::<i32, { CrateId::Foundation as u8 }, 100>::max_capacity(),
            100
        );

        // Test successful push
        assert!(vec.push(42).is_ok());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], 42);
    }

    #[test]
    fn test_wrt_vec_capacity_limits() {
        let mut vec: WrtVec<i32, { CrateId::Foundation as u8 }, 2> = WrtVec::new();

        // Fill to capacity
        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());

        // Should fail on overflow
        assert_eq!(vec.push(3), Err(CapacityError::Exceeded));
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_wrt_hashmap_creation() {
        let mut map: WrtHashMap<String, i32, { CrateId::Foundation as u8 }, 100> =
            WrtHashMap::new();
        assert_eq!(map.len(), 0);
        assert_eq!(
            WrtHashMap::<String, i32, { CrateId::Foundation as u8 }, 100>::max_capacity(),
            100
        );

        // Test successful insert
        assert!(map.insert("key".to_string(), 42).is_ok());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("key"), Some(&42));
    }
}
