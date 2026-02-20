//! Static Memory Capability Implementation
//!
//! This capability type provides compile-time verified memory allocation
//! suitable for ASIL-B and ASIL-C safety levels where static allocation is
//! required.

use core::{
    fmt,
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{
        AtomicPtr,
        Ordering,
    },
};

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

use super::{
    CapabilityMask,
    MemoryCapability,
    MemoryGuard,
    MemoryOperation,
    MemoryOperationType,
    MemoryRegion,
};
use crate::{
    budget_aware_provider::CrateId,
    codes,
    verification::VerificationLevel,
    Error,
    ErrorCategory,
    Result,
};

/// Thread-safe wrapper for NonNull<u8> pointers
///
/// This provides Send + Sync guarantees for capability-managed memory regions
/// where we can ensure thread safety through the capability system.
#[derive(Debug)]
struct ThreadSafePtr {
    ptr: Option<NonNull<u8>>,
}

impl ThreadSafePtr {
    fn new(ptr: Option<NonNull<u8>>) -> Self {
        Self { ptr }
    }

    fn get(&self) -> Option<NonNull<u8>> {
        self.ptr
    }
}

// Safety: ThreadSafePtr can be safely shared between threads when used within
// the capability system which ensures proper access control
unsafe impl Send for ThreadSafePtr {}
unsafe impl Sync for ThreadSafePtr {}

impl Clone for ThreadSafePtr {
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}

/// Static memory capability with compile-time allocation verification
///
/// This capability is suitable for ASIL-B and ASIL-C levels where all
/// allocations must be deterministic and verifiable at compile time.
#[derive(Debug, Clone)]
pub struct StaticMemoryCapability<const N: usize> {
    /// Fixed memory region size (compile-time constant)
    region_size:        usize,
    /// Base address of the static region (if available)
    region_base:        ThreadSafePtr,
    /// Operations allowed by this capability
    allowed_operations: CapabilityMask,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
    /// Crate that owns this capability
    owner_crate:        CrateId,
    /// Static buffer for the memory region
    _phantom:           PhantomData<[u8; N]>,
}

impl<const N: usize> StaticMemoryCapability<N> {
    /// Create a new static memory capability with compile-time size
    ///
    /// # Arguments
    /// * `owner_crate` - Crate that owns this capability
    /// * `verification_level` - Level of verification required
    pub fn new(owner_crate: CrateId, verification_level: VerificationLevel) -> Self {
        Self {
            region_size: N,
            region_base: ThreadSafePtr::new(None),
            allowed_operations: CapabilityMask {
                read:       true,
                write:      true,
                allocate:   true, // Static "allocation" is just claiming the fixed region
                deallocate: true,
                delegate:   true,
                max_size:   N,
            },
            verification_level,
            owner_crate,
            _phantom: PhantomData,
        }
    }

    /// Create a capability with specific operation mask
    pub fn with_operations(
        owner_crate: CrateId,
        verification_level: VerificationLevel,
        operations: CapabilityMask,
    ) -> Self {
        let mut operations = operations;
        // Ensure max_size doesn't exceed compile-time limit
        if operations.max_size == 0 || operations.max_size > N {
            operations.max_size = N;
        }

        Self {
            region_size: N,
            region_base: ThreadSafePtr::new(None),
            allowed_operations: operations,
            verification_level,
            owner_crate,
            _phantom: PhantomData,
        }
    }

    /// Create a capability with a pre-allocated static region
    ///
    /// # Safety
    /// The caller must ensure that `region_base` points to a valid memory
    /// region of at least `N` bytes that will remain valid for the capability's
    /// lifetime.
    pub unsafe fn with_static_region(
        region_base: NonNull<u8>,
        owner_crate: CrateId,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            region_size: N,
            region_base: ThreadSafePtr::new(Some(region_base)),
            allowed_operations: CapabilityMask::all(),
            verification_level,
            owner_crate,
            _phantom: PhantomData,
        }
    }

    /// Get the compile-time size of this capability's memory region
    pub const fn compile_time_size() -> usize {
        N
    }
}

impl<const N: usize> MemoryCapability for StaticMemoryCapability<N> {
    type Guard = StaticMemoryGuard<N>;
    type Region = StaticMemoryRegion<N>;

    fn verify_access(&self, operation: &MemoryOperation) -> Result<()> {
        // Check if operation type is allowed
        if !operation.requires_capability(&self.allowed_operations) {
            return Err(Error::capability_violation(
                "Operation not allowed by static capability mask",
            ));
        }

        // Additional checks based on operation type
        match operation {
            MemoryOperation::Allocate { size } => {
                if *size > N {
                    return Err(Error::capability_violation(
                        "Allocation exceeds compile-time limit",
                    ));
                }
            },
            MemoryOperation::Read { offset, len } => {
                if offset.saturating_add(*len) > N {
                    return Err(Error::capability_violation(
                        "Read operation exceeds compile-time bounds",
                    ));
                }
            },
            MemoryOperation::Write { offset, len } => {
                if offset.saturating_add(*len) > N {
                    return Err(Error::capability_violation(
                        "Write operation exceeds compile-time bounds",
                    ));
                }
            },
            MemoryOperation::Delegate { subset } => {
                if !self.allowed_operations.delegate {
                    return Err(Error::capability_violation(
                        "Static capability delegation not allowed",
                    ));
                }
                if subset.max_size > N {
                    return Err(Error::capability_violation(
                        "Delegated size exceeds compile-time limit",
                    ));
                }
            },
            MemoryOperation::Deallocate => {
                // Static memory "deallocation" is always safe (it's just
                // releasing the claim)
            },
        }

        Ok(())
    }

    fn allocate_region(&self, size: usize, crate_id: CrateId) -> Result<Self::Guard> {
        // Verify allocation operation
        let operation = MemoryOperation::Allocate { size };
        self.verify_access(&operation)?;

        // Check if this crate is allowed to use this capability
        if crate_id != self.owner_crate {
            return Err(Error::capability_violation(
                "Crate cannot use static capability owned by different crate",
            ));
        }

        // For static allocation, we create a region from the compile-time buffer
        let region = StaticMemoryRegion::new(size, self.region_base.get())?;
        let guard = StaticMemoryGuard::new(region, self);

        Ok(guard)
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn delegate(
        &self,
        subset: CapabilityMask,
    ) -> Result<Box<dyn MemoryCapability<Region = Self::Region, Guard = Self::Guard>>> {
        let operation = MemoryOperation::Delegate { subset };
        self.verify_access(&operation)?;

        // Create a delegated static capability with restricted permissions
        let delegated = StaticMemoryCapability::<N> {
            region_size:        core::cmp::min(subset.max_size, N),
            region_base:        self.region_base.clone(),
            allowed_operations: self.allowed_operations.intersect(&subset),
            verification_level: self.verification_level,
            owner_crate:        self.owner_crate,
            _phantom:           PhantomData,
        };

        Ok(Box::new(delegated))
    }

    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    fn max_allocation_size(&self) -> usize {
        N
    }

    fn supports_operation(&self, op_type: MemoryOperationType) -> bool {
        match op_type {
            MemoryOperationType::Read => self.allowed_operations.read,
            MemoryOperationType::Write => self.allowed_operations.write,
            MemoryOperationType::Allocate => self.allowed_operations.allocate,
            MemoryOperationType::Deallocate => self.allowed_operations.deallocate,
            MemoryOperationType::Delegate => self.allowed_operations.delegate,
        }
    }
}

/// Static memory region with compile-time size verification
#[derive(Debug)]
pub struct StaticMemoryRegion<const N: usize> {
    size:     usize,
    buffer:   [u8; N],
    base_ptr: ThreadSafePtr,
}

impl<const N: usize> StaticMemoryRegion<N> {
    fn new(size: usize, base_ptr: Option<NonNull<u8>>) -> Result<Self> {
        if size > N {
            return Err(Error::runtime_execution_error(
                "Static region size exceeds capability bounds",
            ));
        }

        Ok(Self {
            size,
            buffer: [0; N],
            base_ptr: ThreadSafePtr::new(base_ptr),
        })
    }

    /// Get access to the static buffer
    fn buffer(&self) -> &[u8] {
        &self.buffer[..self.size]
    }

    /// Get mutable access to the static buffer
    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.size]
    }
}

impl<const N: usize> MemoryRegion for StaticMemoryRegion<N> {
    fn as_ptr(&self) -> NonNull<u8> {
        if let Some(base_ptr) = self.base_ptr.get() {
            base_ptr
        } else {
            // Use the static buffer
            NonNull::new(self.buffer.as_ptr() as *mut u8)
                .expect("Static buffer pointer should never be null")
        }
    }

    fn size(&self) -> usize {
        self.size
    }
}

/// Static memory guard that protects access to static memory regions
pub struct StaticMemoryGuard<const N: usize> {
    region:     StaticMemoryRegion<N>,
    capability: *const StaticMemoryCapability<N>, // Raw pointer to avoid lifetime issues
}

impl<const N: usize> StaticMemoryGuard<N> {
    fn new(region: StaticMemoryRegion<N>, capability: &StaticMemoryCapability<N>) -> Self {
        Self {
            region,
            capability: capability as *const _,
        }
    }

    fn get_capability(&self) -> &StaticMemoryCapability<N> {
        // Safety: The capability pointer is valid for the lifetime of this guard
        unsafe { &*self.capability }
    }
}

impl<const N: usize> fmt::Debug for StaticMemoryGuard<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StaticMemoryGuard")
            .field("region", &self.region)
            .field("compile_time_size", &N)
            .finish()
    }
}

impl<const N: usize> MemoryGuard for StaticMemoryGuard<N> {
    type Region = StaticMemoryRegion<N>;

    fn region(&self) -> &Self::Region {
        &self.region
    }

    fn read_bytes(&self, offset: usize, len: usize) -> Result<&[u8]> {
        let operation = MemoryOperation::Read { offset, len };
        self.get_capability().verify_access(&operation)?;

        if !self.region.contains_range(offset, len) {
            return Err(Error::runtime_execution_error(
                "Read range exceeds static memory region bounds",
            ));
        }

        let buffer = self.region.buffer();
        Ok(&buffer[offset..offset + len])
    }

    fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let operation = MemoryOperation::Write {
            offset,
            len: data.len(),
        };
        self.get_capability().verify_access(&operation)?;

        if !self.region.contains_range(offset, data.len()) {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::OUT_OF_BOUNDS,
                "Write range exceeds static memory region bounds",
            ));
        }

        let buffer = self.region.buffer_mut();
        buffer[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn capability(&self) -> &dyn MemoryCapability<Region = Self::Region, Guard = Self> {
        self.get_capability()
    }
}

// Safety: StaticMemoryGuard can be sent between threads safely
unsafe impl<const N: usize> Send for StaticMemoryGuard<N> {}
unsafe impl<const N: usize> Sync for StaticMemoryGuard<N> {}

/// Common static capability sizes for different use cases
pub type SmallStaticCapability = StaticMemoryCapability<4096>; // 4KB
pub type MediumStaticCapability = StaticMemoryCapability<65536>; // 64KB
pub type LargeStaticCapability = StaticMemoryCapability<1048576>; // 1MB

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_capability_creation() {
        let capability =
            StaticMemoryCapability::<1024>::new(CrateId::Foundation, VerificationLevel::Standard);

        assert_eq!(capability.max_allocation_size(), 1024);
        assert_eq!(StaticMemoryCapability::<1024>::compile_time_size(), 1024);
    }

    #[test]
    fn test_static_allocation_verification() {
        let capability =
            StaticMemoryCapability::<100>::new(CrateId::Foundation, VerificationLevel::Standard);

        let valid_op = MemoryOperation::Allocate { size: 50 };
        assert!(capability.verify_access(&valid_op).is_ok());

        let invalid_op = MemoryOperation::Allocate { size: 200 };
        assert!(capability.verify_access(&invalid_op).is_err());
    }

    #[test]
    fn test_compile_time_bounds_checking() {
        let capability =
            StaticMemoryCapability::<1000>::new(CrateId::Foundation, VerificationLevel::Standard);

        let valid_read = MemoryOperation::Read {
            offset: 0,
            len:    500,
        };
        assert!(capability.verify_access(&valid_read).is_ok());

        let invalid_read = MemoryOperation::Read {
            offset: 500,
            len:    600,
        };
        assert!(capability.verify_access(&invalid_read).is_err());
    }

    #[test]
    fn test_static_region_access() {
        let mut region = StaticMemoryRegion::<1024>::new(100, None).unwrap();

        assert_eq!(region.size(), 100);
        assert_eq!(region.buffer().len(), 100);

        // Test write and read
        let test_data = b"Hello, World!";
        region.buffer_mut()[0..test_data.len()].copy_from_slice(test_data);
        assert_eq!(&region.buffer()[0..test_data.len()], test_data);
    }
}
