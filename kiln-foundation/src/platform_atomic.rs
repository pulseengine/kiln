//! Platform abstraction for safe atomic operations
//!
//! This module provides safe abstractions for atomic operations that avoid
//! unsafe code in safety-critical paths. It uses platform-specific
//! implementations that guarantee safety through type system constraints.

use core::sync::atomic::{
    AtomicU16,
    AtomicU32,
    AtomicU64,
    AtomicU8,
    Ordering,
};

use crate::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Safe atomic memory view that provides bounds-checked atomic operations
#[derive(Debug)]
pub struct SafeAtomicMemory {
    /// Internal representation - implementation detail
    _private: (),
}

/// Platform-specific atomic memory provider
pub trait PlatformAtomicProvider: Send + Sync + core::fmt::Debug {
    /// Create a safe atomic view of memory
    fn create_atomic_view(&self, base: *mut u8, size: usize) -> Result<SafeAtomicMemory>;

    /// Load u32 atomically with bounds checking
    fn atomic_load_u32(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        ordering: Ordering,
    ) -> Result<u32>;

    /// Store u32 atomically with bounds checking
    fn atomic_store_u32(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        value: u32,
        ordering: Ordering,
    ) -> Result<()>;

    /// Compare and exchange u32 atomically with bounds checking
    fn atomic_cmpxchg_u32(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        expected: u32,
        new: u32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u32>;

    /// Fetch and add u32 atomically with bounds checking
    fn atomic_fetch_add_u32(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        val: u32,
        ordering: Ordering,
    ) -> Result<u32>;

    /// Load u64 atomically with bounds checking
    fn atomic_load_u64(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        ordering: Ordering,
    ) -> Result<u64>;

    /// Store u64 atomically with bounds checking
    fn atomic_store_u64(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        value: u64,
        ordering: Ordering,
    ) -> Result<()>;

    /// Compare and exchange u64 atomically with bounds checking
    fn atomic_cmpxchg_u64(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        expected: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64>;

    /// Fetch and add u64 atomically with bounds checking
    fn atomic_fetch_add_u64(
        &self,
        view: &SafeAtomicMemory,
        offset: usize,
        val: u64,
        ordering: Ordering,
    ) -> Result<u64>;
}

/// No-op implementation for platforms without atomic support
#[derive(Debug)]
pub struct NoAtomicProvider;

impl PlatformAtomicProvider for NoAtomicProvider {
    fn create_atomic_view(&self, _base: *mut u8, _size: usize) -> Result<SafeAtomicMemory> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_load_u32(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _ordering: Ordering,
    ) -> Result<u32> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_store_u32(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _value: u32,
        _ordering: Ordering,
    ) -> Result<()> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_cmpxchg_u32(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _expected: u32,
        _new: u32,
        _success: Ordering,
        _failure: Ordering,
    ) -> Result<u32> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_fetch_add_u32(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _val: u32,
        _ordering: Ordering,
    ) -> Result<u32> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_load_u64(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _ordering: Ordering,
    ) -> Result<u64> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_store_u64(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _value: u64,
        _ordering: Ordering,
    ) -> Result<()> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_cmpxchg_u64(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _expected: u64,
        _new: u64,
        _success: Ordering,
        _failure: Ordering,
    ) -> Result<u64> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }

    fn atomic_fetch_add_u64(
        &self,
        _view: &SafeAtomicMemory,
        _offset: usize,
        _val: u64,
        _ordering: Ordering,
    ) -> Result<u64> {
        Err(Error::runtime_not_implemented(
            "Atomic operations not supported on this platform",
        ))
    }
}

/// Get the platform atomic provider for the current platform
pub fn get_platform_atomic_provider() -> &'static dyn PlatformAtomicProvider {
    // For now, return the no-op provider
    // In a real implementation, this would return platform-specific providers
    &NoAtomicProvider
}

/// Safe atomic operations wrapper
#[derive(Debug)]
pub struct SafeAtomicOps<'a> {
    provider: &'a dyn PlatformAtomicProvider,
    view:     SafeAtomicMemory,
}

impl<'a> SafeAtomicOps<'a> {
    /// Create new safe atomic operations wrapper
    pub fn new(base: *mut u8, size: usize) -> Result<Self> {
        let provider = get_platform_atomic_provider();
        let view = provider.create_atomic_view(base, size)?;
        Ok(Self { provider, view })
    }

    /// Load u32 atomically
    pub fn load_u32(&self, offset: usize, ordering: Ordering) -> Result<u32> {
        self.provider.atomic_load_u32(&self.view, offset, ordering)
    }

    /// Store u32 atomically
    pub fn store_u32(&self, offset: usize, value: u32, ordering: Ordering) -> Result<()> {
        self.provider.atomic_store_u32(&self.view, offset, value, ordering)
    }

    /// Compare and exchange u32
    pub fn cmpxchg_u32(
        &self,
        offset: usize,
        expected: u32,
        new: u32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u32> {
        self.provider
            .atomic_cmpxchg_u32(&self.view, offset, expected, new, success, failure)
    }

    /// Fetch and add u32
    pub fn fetch_add_u32(&self, offset: usize, val: u32, ordering: Ordering) -> Result<u32> {
        self.provider.atomic_fetch_add_u32(&self.view, offset, val, ordering)
    }

    /// Load u64 atomically
    pub fn load_u64(&self, offset: usize, ordering: Ordering) -> Result<u64> {
        self.provider.atomic_load_u64(&self.view, offset, ordering)
    }

    /// Store u64 atomically
    pub fn store_u64(&self, offset: usize, value: u64, ordering: Ordering) -> Result<()> {
        self.provider.atomic_store_u64(&self.view, offset, value, ordering)
    }

    /// Compare and exchange u64
    pub fn cmpxchg_u64(
        &self,
        offset: usize,
        expected: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64> {
        self.provider
            .atomic_cmpxchg_u64(&self.view, offset, expected, new, success, failure)
    }

    /// Fetch and add u64
    pub fn fetch_add_u64(&self, offset: usize, val: u64, ordering: Ordering) -> Result<u64> {
        self.provider.atomic_fetch_add_u64(&self.view, offset, val, ordering)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_atomic_provider() {
        let provider = NoAtomicProvider;
        let result = provider.create_atomic_view(core::ptr::null_mut(), 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_platform_provider() {
        let provider = get_platform_atomic_provider();
        let result = provider.create_atomic_view(core::ptr::null_mut(), 0);
        assert!(result.is_err()); // NoAtomicProvider returns error
    }
}
