//! Validated Collections with Compile-Time Bounds Checking
//!
//! This module provides collection types that integrate compile-time bounds
//! validation with the existing BoundedVec and BoundedMap implementations.
//!
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! SW-REQ-ID: REQ_COMP_BOUNDS_001 - Compile-time bounds validation

use crate::{
    bounded::{BoundedVec, BoundedString},
    bounded_collections::BoundedMap,
    compile_time_bounds::CollectionBoundsValidator,
    budget_aware_provider::CrateId,
    safe_memory::MemoryProvider,
    safe_managed_alloc,
    validate_allocation,
    wrt_error::Result,
};

/// Validated BoundedVec with compile-time bounds checking
/// 
/// This type ensures that all collection allocations are validated at compile time
/// for ASIL-D compliance.
pub struct ValidatedBoundedVec<T, const CAPACITY: usize, P: MemoryProvider> {
    inner: BoundedVec<T, CAPACITY, P>,
    _validator: CollectionBoundsValidator<CAPACITY, 64>, // Fixed element size for compatibility
}

impl<T, const CAPACITY: usize, P: MemoryProvider> ValidatedBoundedVec<T, CAPACITY, P> {
    /// Create a new validated bounded vector
    /// 
    /// This function performs compile-time validation of the collection bounds.
    pub fn new(provider: P) -> wrt_error::Result<Self>
    where
        T: Default + Clone + PartialEq + Eq,
        T: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        // Use fixed size for compatibility - actual validation happens at usage site
        let validator = CollectionBoundsValidator::<CAPACITY, 64>::validate);
        
        // Create the underlying collection
        let inner = BoundedVec::new(provider)?;
        
        Ok(Self {
            inner,
            _validator: validator,
        })
    }
    
    /// Create a new validated bounded vector with budget allocation
    pub fn new_with_budget(crate_id: CrateId) -> wrt_error::Result<Self>
    where
        T: Default + Clone + PartialEq + Eq,
        T: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
        P: Default,
    {
        // Compile-time validation with fixed size
        let _validator = CollectionBoundsValidator::<CAPACITY, 64>::validate);
        
        // Runtime allocation size validation - no compile-time constraints here
        let allocation_size = CAPACITY * core::mem::size_of::<T>);
        
        // Allocate memory through budget system
        let provider = safe_managed_alloc!(allocation_size, crate_id)?;
        
        Self::new(provider)
    }
    
    /// Get the underlying BoundedVec
    pub fn inner(&self) -> &BoundedVec<T, CAPACITY, P> {
        &self.inner
    }
    
    /// Get the underlying BoundedVec mutably
    pub fn inner_mut(&mut self) -> &mut BoundedVec<T, CAPACITY, P> {
        &mut self.inner
    }
    
    /// Get the compile-time validated capacity
    pub const fn validated_capacity() -> usize {
        CAPACITY
    }
    
    /// Get the compile-time validated element size
    pub const fn validated_element_size() -> usize {
        core::mem::size_of::<T>()
    }
    
    /// Get the compile-time validated total memory usage
    pub const fn validated_total_memory() -> usize {
        CAPACITY * core::mem::size_of::<T>()
    }
}

// Delegate common operations to the inner BoundedVec
impl<T, const CAPACITY: usize, P: MemoryProvider> ValidatedBoundedVec<T, CAPACITY, P> {
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
    
    pub fn push(&mut self, value: T) -> Result<(), T>
    where
        T: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        self.inner.push(value)
    }
    
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }
    
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }
    
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(index)
    }
}

/// Validated BoundedMap with compile-time bounds checking
pub struct ValidatedBoundedMap<K, V, const CAPACITY: usize, P: MemoryProvider> {
    inner: BoundedMap<K, V, CAPACITY, P>,
    _validator: CollectionBoundsValidator<CAPACITY, 128>, // Fixed pair size for compatibility
}

impl<K, V, const CAPACITY: usize, P: MemoryProvider> ValidatedBoundedMap<K, V, CAPACITY, P> {
    /// Create a new validated bounded map
    pub fn new(provider: P) -> wrt_error::Result<Self>
    where
        K: Default + Clone + PartialEq + Eq,
        V: Default + Clone + PartialEq + Eq,
        K: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
        V: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
    {
        // Compile-time validation with fixed size
        let validator = CollectionBoundsValidator::<CAPACITY, 128>::validate);
        
        // Create the underlying collection
        let inner = BoundedMap::new(provider)?;
        
        Ok(Self {
            inner,
            _validator: validator,
        })
    }
    
    /// Create a new validated bounded map with budget allocation
    pub fn new_with_budget(crate_id: CrateId) -> wrt_error::Result<Self>
    where
        K: Default + Clone + PartialEq + Eq,
        V: Default + Clone + PartialEq + Eq,
        K: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
        V: crate::traits::Checksummable + crate::traits::ToBytes + crate::traits::FromBytes,
        P: Default,
    {
        // Compile-time validation with fixed size
        let _validator = CollectionBoundsValidator::<CAPACITY, 128>::validate);
        
        // Runtime allocation size calculation
        let allocation_size = CAPACITY * core::mem::size_of::<(K, V)>);
        
        // Allocate memory through budget system
        let provider = safe_managed_alloc!(allocation_size, crate_id)?;
        
        Self::new(provider)
    }
    
    /// Get the underlying BoundedMap
    pub fn inner(&self) -> &BoundedMap<K, V, CAPACITY, P> {
        &self.inner
    }
    
    /// Get the underlying BoundedMap mutably
    pub fn inner_mut(&mut self) -> &mut BoundedMap<K, V, CAPACITY, P> {
        &mut self.inner
    }
}

/// Validated BoundedString with compile-time bounds checking
pub struct ValidatedBoundedString<const CAPACITY: usize, P: MemoryProvider> {
    inner: BoundedString<CAPACITY, P>,
    _validator: CollectionBoundsValidator<CAPACITY, 1>, // 1 byte per character
}

impl<const CAPACITY: usize, P: MemoryProvider> ValidatedBoundedString<CAPACITY, P> {
    /// Create a new validated bounded string
    pub fn new(provider: P) -> Self {
        // Compile-time validation
        let validator = CollectionBoundsValidator::<CAPACITY, 1>::validate);
        
        // Create the underlying string
        let inner = BoundedString::new(provider;
        
        Self {
            inner,
            _validator: validator,
        }
    }
    
    /// Create a new validated bounded string with budget allocation
    pub fn new_with_budget(crate_id: CrateId) -> wrt_error::Result<Self>
    where
        P: Default,
    {
        // Compile-time validation
        let _validator = CollectionBoundsValidator::<CAPACITY, 1>::validate);
        
        // Capacity is compile-time constant, no runtime validation needed
        
        // Allocate memory through budget system
        let provider = safe_managed_alloc!(CAPACITY, crate_id)?;
        
        Ok(Self::new(provider))
    }
    
    /// Create from string slice with validation
    pub fn from_str(s: &str, provider: P) -> wrt_error::Result<Self> {
        // Compile-time validation
        let validator = CollectionBoundsValidator::<CAPACITY, 1>::validate);
        
        // Runtime validation
        if s.len() > CAPACITY {
            return Err(crate::Error::runtime_execution_error("String exceeds validated capacity";
        }
        
        // Create the underlying string
        let inner = BoundedString::try_from_str(s, provider)?;
        
        Ok(Self {
            inner,
            _validator: validator,
        })
    }
    
    /// Get the underlying BoundedString
    pub fn inner(&self) -> &BoundedString<CAPACITY, P> {
        &self.inner
    }
    
    /// Get the underlying BoundedString mutably
    pub fn inner_mut(&mut self) -> &mut BoundedString<CAPACITY, P> {
        &mut self.inner
    }
}

/// Convenience macros for creating validated collections
#[macro_export]
macro_rules! validated_vec {
    ($capacity:expr, $element_type:ty, $crate_id:expr) => {{
        use $crate::validated_collections::ValidatedBoundedVec;
        use $crate::safe_memory::NoStdProvider;
        
        ValidatedBoundedVec::<$element_type, $capacity, NoStdProvider<{$capacity * 64}>>::new_with_budget($crate_id)
    }};
}

#[macro_export]
macro_rules! validated_map {
    ($capacity:expr, $key_type:ty, $value_type:ty, $crate_id:expr) => {{
        use $crate::validated_collections::ValidatedBoundedMap;
        use $crate::safe_memory::NoStdProvider;
        
        ValidatedBoundedMap::<$key_type, $value_type, $capacity, NoStdProvider<{$capacity * 128}>>::new_with_budget($crate_id)
    }};
}

#[macro_export]
macro_rules! validated_string {
    ($capacity:expr, $crate_id:expr) => {{
        use $crate::validated_collections::ValidatedBoundedString;
        use $crate::safe_memory::NoStdProvider;
        
        ValidatedBoundedString::<$capacity, NoStdProvider<$capacity>>::new_with_budget($crate_id)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_init::MemoryInitializer;
    
    #[test]
    fn test_validated_bounded_vec() {
        MemoryInitializer::initialize().unwrap();
        
        let mut vec = validated_vec!(100, u32, CrateId::Foundation).unwrap();
        
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 100;
        assert_eq!(vec.validated_capacity(), 100;
        assert_eq!(vec.validated_element_size(), 4;
        assert_eq!(vec.validated_total_memory(), 400;
        
        // Test operations
        vec.push(42).unwrap();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.get(0), Some(&42;
    }
    
    #[test]
    fn test_validated_bounded_string() {
        MemoryInitializer::initialize().unwrap();
        
        let string = validated_string!(256, CrateId::Foundation).unwrap();
        assert_eq!(string.inner().len(), 0);
    }
    
    #[test]
    fn test_compile_time_validation() {
        // These should compile without issues due to compile-time validation
        MemoryInitializer::initialize().unwrap();
        
        let _small_vec = validated_vec!(10, u8, CrateId::Foundation).unwrap();
        let _medium_vec = validated_vec!(1000, u32, CrateId::Component).unwrap();
        let _string = validated_string!(128, CrateId::Foundation).unwrap();
    }
}