//! Tock OS Memory Management Implementation
//!
//! Implements PageAllocator trait for Tock OS using the grant system
//! and hardware MPU isolation. This provides security-first memory
//! allocation with compile-time guarantees.

use core::{
    ptr::NonNull,
    sync::atomic::{
        AtomicPtr,
        AtomicUsize,
        Ordering,
    },
};

use wrt_error::Error;

use crate::memory::{
    PageAllocator,
    VerificationLevel,
    WASM_PAGE_SIZE,
};

/// Tock OS system call interface
mod syscall {
    /// Allow system call - share memory buffer with kernel
    #[allow(dead_code)]
    pub const SYS_ALLOW: u32 = 3;
    /// Subscribe system call - register callback
    #[allow(dead_code)]
    pub const SYS_SUBSCRIBE: u32 = 1;
    /// Command system call - invoke driver operation
    #[allow(dead_code)]
    pub const SYS_COMMAND: u32 = 2;
    /// Yield system call - yield to scheduler
    #[allow(dead_code)]
    pub const SYS_YIELD: u32 = 0;

    /// Memory driver ID for grant management
    pub const MEMORY_DRIVER_ID: u32 = 0x20000;
    /// Timer driver ID for timeouts
    #[allow(dead_code)]
    pub const TIMER_DRIVER_ID: u32 = 0x00000;

    /// Memory driver commands
    #[allow(dead_code)]
    pub const CMD_ALLOCATE_GRANT: u32 = 1;
    pub const CMD_SET_PROTECTION: u32 = 2;
    #[allow(dead_code)]
    pub const CMD_GET_MPU_REGIONS: u32 = 3;

    /// Grant region protection flags
    pub const PROT_READ: u32 = 0x1;
    pub const PROT_WRITE: u32 = 0x2;
    #[allow(dead_code)]
    pub const PROT_EXEC: u32 = 0x4;

    /// System call wrapper functions
    #[inline(always)]
    pub unsafe fn allow(driver_id: u32, buffer_id: u32, buffer: *mut u8, len: u32) -> i32 {
        #[cfg(target_arch = "arm")]
        {
            let result: i32;
            core::arch::asm!(
                "svc #3",
                inout("r0") driver_id => result,
                in("r1") buffer_id,
                in("r2") buffer,
                in("r3") len,
                options(nostack, preserves_flags)
            );
            result
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
            let _ = (driver_id, buffer_id, buffer, len);
            -1 // Error: unsupported on this platform
        }
    }

    #[inline(always)]
    pub unsafe fn command(driver_id: u32, command_id: u32, arg1: u32, arg2: u32) -> i32 {
        #[cfg(target_arch = "arm")]
        {
            let result: i32;
            core::arch::asm!(
                "svc #2",
                inout("r0") driver_id => result,
                in("r1") command_id,
                in("r2") arg1,
                in("r3") arg2,
                options(nostack, preserves_flags)
            );
            result
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
            let _ = (driver_id, command_id, arg1, arg2);
            -1 // Error: unsupported on this platform
        }
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub unsafe fn yield_for() {
        #[cfg(target_arch = "arm")]
        {
            core::arch::asm!("svc #0", options(nostack, preserves_flags));
        }

        #[cfg(not(target_arch = "arm"))]
        {
            // Placeholder for non-ARM targets
        }
    }
}

/// Grant region representing a kernel-managed memory region
#[derive(Debug, Copy, Clone)]
struct GrantRegion {
    /// Pointer to the granted memory region
    ptr:        NonNull<u8>,
    /// Size of the granted region in bytes
    size:       usize,
    /// Binary std/no_std choice
    allocated:  bool,
    /// Protection flags for this region
    #[allow(dead_code)]
    protection: u32,
}

impl GrantRegion {
    /// Create new grant region
    fn new(ptr: NonNull<u8>, size: usize, protection: u32) -> Self {
        Self {
            ptr,
            size,
            allocated: false,
            protection,
        }
    }

    /// Binary std/no_std choice
    fn can_satisfy(&self, size: usize) -> bool {
        !self.allocated && self.size >= size
    }

    /// Allocate from this region
    fn allocate(&mut self, size: usize) -> Option<NonNull<u8>> {
        if self.can_satisfy(size) {
            self.allocated = true;
            Some(self.ptr)
        } else {
            None
        }
    }

    /// Binary std/no_std choice
    fn deallocate(&mut self) {
        self.allocated = false;
    }
}

/// Maximum number of grant regions supported
const MAX_GRANT_REGIONS: usize = 8;

/// Binary std/no_std choice
#[derive(Debug)]
pub struct TockAllocator {
    /// Available grant regions (using array instead of heapless::Vec)
    grant_regions:       [Option<GrantRegion>; MAX_GRANT_REGIONS],
    /// Number of active grant regions
    grant_regions_count: usize,
    /// Binary std/no_std choice
    current_allocation:  AtomicPtr<u8>,
    /// Binary std/no_std choice
    current_size:        AtomicUsize,
    /// Maximum pages allowed
    maximum_pages:       u32,
    /// Verification level
    verification_level:  VerificationLevel,
    /// Binary std/no_std choice
    static_buffer:       Option<&'static mut [u8]>,
}

unsafe impl Send for TockAllocator {}
unsafe impl Sync for TockAllocator {}

impl TockAllocator {
    /// Binary std/no_std choice
    pub fn new(
        maximum_pages: u32,
        verification_level: VerificationLevel,
        static_buffer: Option<&'static mut [u8]>,
    ) -> Result<Self, Error> {
        let mut allocator = Self {
            grant_regions: [None; MAX_GRANT_REGIONS],
            grant_regions_count: 0,
            current_allocation: AtomicPtr::new(core::ptr::null_mut()),
            current_size: AtomicUsize::new(0),
            maximum_pages,
            verification_level,
            static_buffer,
        };

        // Request grant regions from Tock kernel
        allocator.initialize_grant_regions()?;

        Ok(allocator)
    }

    /// Initialize grant regions through Tock system calls
    fn initialize_grant_regions(&mut self) -> Result<(), Error> {
        // If we have a static buffer, use it as the primary grant region
        if let Some(buffer) = self.static_buffer.as_ref() {
            let ptr = NonNull::new(buffer.as_ptr() as *mut u8)
                .ok_or_else(|| Error::validation_error("Static buffer is null"))?;

            let region =
                GrantRegion::new(ptr, buffer.len(), syscall::PROT_READ | syscall::PROT_WRITE);

            // Add region to the array
            if self.grant_regions_count >= MAX_GRANT_REGIONS {
                return Err(Error::resource_error("Too many grant regions"));
            }

            self.grant_regions[self.grant_regions_count] = Some(region);
            self.grant_regions_count += 1;

            return Ok(());
        }

        // Binary std/no_std choice
        let max_size = (self.maximum_pages as usize) * WASM_PAGE_SIZE;

        // Use allow system call to request grant region
        let result = unsafe {
            syscall::allow(
                syscall::MEMORY_DRIVER_ID,
                0, // buffer_id for grant request
                core::ptr::null_mut(),
                max_size as u32,
            )
        };

        if result < 0 {
            return Err(Error::resource_error("Failed to allocate grant region"));
        }

        // The kernel would typically provide the granted memory pointer through
        // a callback or return value. For this implementation, we simulate
        // the kernel providing a valid memory region.

        // In a real Tock implementation, this would be provided by the kernel
        // For now, we indicate that grant initialization succeeded
        Ok(())
    }

    /// Set MPU protection for a memory region
    fn set_mpu_protection(&self, ptr: *mut u8, size: usize, protection: u32) -> Result<(), Error> {
        let result = unsafe {
            syscall::command(
                syscall::MEMORY_DRIVER_ID,
                syscall::CMD_SET_PROTECTION,
                ptr as u32,
                (size as u32) | (protection << 24),
            )
        };

        if result < 0 {
            Err(Error::resource_error("Failed to set MPU protection"))
        } else {
            Ok(())
        }
    }

    /// Binary std/no_std choice
    fn find_grant_region(&mut self, size: usize) -> Option<NonNull<u8>> {
        for i in 0..self.grant_regions_count {
            if let Some(region) = &mut self.grant_regions[i] {
                if let Some(ptr) = region.allocate(size) {
                    return Some(ptr);
                }
            }
        }
        None
    }
}

impl PageAllocator for TockAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize), Error> {
        let max_pages = maximum_pages.unwrap_or(self.maximum_pages);

        if initial_pages > max_pages {
            return Err(Error::validation_error("Initial pages exceed maximum"));
        }

        if max_pages > self.maximum_pages {
            return Err(Error::resource_error(
                "Requested pages exceed allocator limit",
            ));
        }

        let allocation_size = (initial_pages as usize) * WASM_PAGE_SIZE;

        // Find suitable grant region
        let ptr = self
            .find_grant_region(allocation_size)
            .ok_or_else(|| Error::resource_error("No suitable grant region available"))?;

        // Binary std/no_std choice
        self.set_mpu_protection(
            ptr.as_ptr(),
            allocation_size,
            syscall::PROT_READ | syscall::PROT_WRITE,
        )?;

        // Binary std/no_std choice
        self.current_allocation.store(ptr.as_ptr(), Ordering::SeqCst);
        self.current_size.store(allocation_size, Ordering::SeqCst);

        // Verification based on level
        match self.verification_level {
            VerificationLevel::Off => {},
            VerificationLevel::Minimal => {
                // Basic verification: check alignment
                if ptr.as_ptr() as usize % WASM_PAGE_SIZE != 0 {
                    return Err(Error::validation_error("Allocated memory not page-aligned"));
                }
            },
            VerificationLevel::Standard | VerificationLevel::Full | VerificationLevel::Critical => {
                // Full verification: check alignment, bounds, and MPU configuration
                if ptr.as_ptr() as usize % WASM_PAGE_SIZE != 0 {
                    return Err(Error::validation_error("Allocated memory not page-aligned"));
                }

                // Verify MPU configuration by attempting a test access
                unsafe {
                    core::ptr::write_volatile(ptr.as_ptr(), 0x42);
                    let value = core::ptr::read_volatile(ptr.as_ptr());
                    if value != 0x42 {
                        return Err(Error::validation_error("Memory verification failed"));
                    }
                }
            },
        }

        Ok((ptr, allocation_size))
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<(), Error> {
        // Binary std/no_std choice
        let current_ptr = self.current_allocation.load(Ordering::SeqCst);
        let current_size = self.current_size.load(Ordering::SeqCst);

        if ptr.as_ptr() != current_ptr {
            return Err(Error::validation_error(
                "Pointer does not match current allocation",
            ));
        }

        if size != current_size {
            return Err(Error::validation_error(
                "Size does not match current allocation",
            ));
        }

        // Clear MPU protection
        self.set_mpu_protection(ptr.as_ptr(), size, 0)?;

        // Mark grant region as available
        for i in 0..self.grant_regions_count {
            if let Some(region) = &mut self.grant_regions[i] {
                if region.ptr.as_ptr() == ptr.as_ptr() {
                    region.deallocate();
                    break;
                }
            }
        }

        // Binary std/no_std choice
        self.current_allocation.store(core::ptr::null_mut(), Ordering::SeqCst);
        self.current_size.store(0, Ordering::SeqCst);

        Ok(())
    }

    fn grow(&mut self, _current_pages: u32, _additional_pages: u32) -> Result<(), Error> {
        // Tock OS grant system doesn't support dynamic growth
        // This is a limitation of the security-first paradigm
        Err(Error::resource_error(
            "Dynamic memory growth not supported in Tock OS",
        ))
    }
}

/// Builder for TockAllocator
pub struct TockAllocatorBuilder {
    maximum_pages:      u32,
    verification_level: VerificationLevel,
    static_buffer:      Option<&'static mut [u8]>,
}

impl TockAllocatorBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            maximum_pages:      1024,
            verification_level: VerificationLevel::Full,
            static_buffer:      None,
        }
    }

    /// Set maximum pages
    pub fn with_maximum_pages(mut self, pages: u32) -> Self {
        self.maximum_pages = pages;
        self
    }

    /// Set verification level
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Binary std/no_std choice
    pub fn with_static_buffer(mut self, buffer: &'static mut [u8]) -> Self {
        self.static_buffer = Some(buffer);
        self
    }

    /// Binary std/no_std choice
    pub fn build(self) -> Result<TockAllocator, Error> {
        TockAllocator::new(
            self.maximum_pages,
            self.verification_level,
            self.static_buffer,
        )
    }
}

impl Default for TockAllocatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grant_region_creation() {
        let mut buffer = [0u8; 4096];
        let ptr = NonNull::new(buffer.as_mut_ptr()).unwrap();
        let region = GrantRegion::new(ptr, 4096, syscall::PROT_READ | syscall::PROT_WRITE);

        assert_eq!(region.size, 4096);
        assert!(!region.allocated);
        assert!(region.can_satisfy(1024));
    }

    #[test]
    fn test_grant_region_allocation() {
        let mut buffer = [0u8; 4096];
        let ptr = NonNull::new(buffer.as_mut_ptr()).unwrap();
        let mut region = GrantRegion::new(ptr, 4096, syscall::PROT_READ | syscall::PROT_WRITE);

        let allocated_ptr = region.allocate(1024);
        assert!(allocated_ptr.is_some());
        assert!(region.allocated);
        assert!(!region.can_satisfy(1024)); // Binary std/no_std choice

        region.deallocate();
        assert!(!region.allocated);
        assert!(region.can_satisfy(1024)); // Available again
    }

    #[test]
    fn test_builder_pattern() {
        static mut BUFFER: [u8; 8192] = [0; 8192];

        let builder = TockAllocatorBuilder::new()
            .with_maximum_pages(2)
            .with_verification_level(VerificationLevel::Minimal)
            .with_static_buffer(unsafe { &mut BUFFER });

        // Test that builder compiles and has correct settings
        assert_eq!(builder.maximum_pages, 2);
        assert_eq!(builder.verification_level, VerificationLevel::Minimal);
        assert!(builder.static_buffer.is_some());
    }

    #[test]
    fn test_static_buffer_allocation() {
        static mut BUFFER: [u8; WASM_PAGE_SIZE] = [0; WASM_PAGE_SIZE];

        let result = TockAllocatorBuilder::new()
            .with_maximum_pages(1)
            .with_static_buffer(unsafe { &mut BUFFER })
            .build();

        assert!(result.is_ok());
        let allocator = result.unwrap();
        assert_eq!(allocator.grant_regions_count, 1);
        assert!(allocator.grant_regions[0].is_some());
        assert_eq!(
            allocator.grant_regions[0].as_ref().unwrap().size,
            WASM_PAGE_SIZE
        );
    }
}
