//! QNX-specific memory allocation and management.
//!
//! Provides a custom `PageAllocator` implementation for QNX Neutrino RTOS,
//! designed for safety-critical systems with no_std and no_alloc requirements.
//!
//! This module interfaces with QNX mmap, mprotect, and memory partition APIs
//! to provide secure and isolated memory regions for WebAssembly execution.

use core::{
    fmt::{
        self,
        Debug,
    },
    ptr::NonNull,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::memory::{
    PageAllocator,
    VerificationLevel,
    WASM_PAGE_SIZE,
};

/// QNX memory protection flags (compatible with mmap/mprotect)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnxProtFlags {
    /// No access
    None             = 0,
    /// Read access
    Read             = 1,
    /// Write access
    Write            = 2,
    /// Execute access
    Execute          = 4,
    /// Read and write access
    ReadWrite        = 3,
    /// Read and execute access
    ReadExecute      = 5,
    /// Read, write, and execute access
    ReadWriteExecute = 7,
}

/// QNX memory mapping flags (compatible with mmap)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnxMapFlags {
    /// Memory is shared
    Shared    = 1,
    /// Memory is private (copy-on-write)
    Private   = 2,
    /// Memory is fixed at the specified address
    Fixed     = 16,
    /// Memory is anonymous (not backed by a file)
    Anonymous = 4096,
}

/// FFI declarations for QNX system calls needed for memory management
#[allow(non_camel_case_types)]
mod ffi {
    use core::ffi::c_void;

    // QNX-specific types
    pub type qnx_off_t = i64;
    pub type qnx_size_t = usize;
    pub type qnx_pid_t = i32;
    pub type qnx_mode_t = u32;
    pub type mem_partition_id_t = u32;

    // SAFETY: Edition 2024 requires unsafe extern blocks
    unsafe extern "C" {
        // Binary std/no_std choice
        pub fn mmap(
            addr: *mut c_void,
            len: qnx_size_t,
            prot: u32,
            flags: u32,
            fd: i32,
            offset: qnx_off_t,
        ) -> *mut c_void;

        // Binary std/no_std choice
        pub fn munmap(addr: *mut c_void, len: qnx_size_t) -> i32;

        // mprotect for changing memory protection
        pub fn mprotect(addr: *mut c_void, len: qnx_size_t, prot: u32) -> i32;

        // QNX memory partition functions
        pub fn mem_partition_create(
            flags: u32,
            name: *const u8,
            parent: mem_partition_id_t,
        ) -> mem_partition_id_t;

        pub fn mem_partition_destroy(id: mem_partition_id_t) -> i32;

        pub fn mem_partition_getid() -> mem_partition_id_t;

        pub fn mem_partition_setcurrent(id: mem_partition_id_t) -> i32;
    }
}

/// Binary std/no_std choice
#[derive(Debug, Clone)]
pub struct QnxAllocatorConfig {
    /// Binary std/no_std choice
    pub use_guard_pages:    bool,
    /// Memory protection flags for guard pages
    pub guard_page_prot:    QnxProtFlags,
    /// Memory protection flags for data pages
    pub data_page_prot:     QnxProtFlags,
    /// Memory mapping flags
    pub map_flags:          QnxMapFlags,
    /// Whether to create a dedicated memory partition
    pub create_partition:   bool,
    /// Verification level for memory operations
    pub verification_level: VerificationLevel,
}

impl Default for QnxAllocatorConfig {
    fn default() -> Self {
        Self {
            use_guard_pages:    true,
            guard_page_prot:    QnxProtFlags::None,
            data_page_prot:     QnxProtFlags::ReadWrite,
            map_flags:          QnxMapFlags::Private | QnxMapFlags::Anonymous,
            create_partition:   false,
            verification_level: VerificationLevel::Standard,
        }
    }
}

/// Builder for QnxAllocator
#[derive(Debug, Default)]
pub struct QnxAllocatorBuilder {
    config: QnxAllocatorConfig,
}

impl QnxAllocatorBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Binary std/no_std choice
    pub fn with_guard_pages(mut self, use_guard_pages: bool) -> Self {
        self.config.use_guard_pages = use_guard_pages;
        self
    }

    /// Set memory protection flags for data pages
    pub fn with_data_protection(mut self, prot: QnxProtFlags) -> Self {
        self.config.data_page_prot = prot;
        self
    }

    /// Set memory mapping flags
    pub fn with_map_flags(mut self, flags: QnxMapFlags) -> Self {
        self.config.map_flags = flags;
        self
    }

    /// Configure whether to create a dedicated memory partition
    pub fn with_dedicated_partition(mut self, create_partition: bool) -> Self {
        self.config.create_partition = create_partition;
        self
    }

    /// Set verification level for memory operations
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.config.verification_level = level;
        self
    }

    /// Build the QnxAllocator with the configured settings
    pub fn build(self) -> QnxAllocator {
        QnxAllocator::new(self.config)
    }
}

/// Binary std/no_std choice
#[derive(Debug)]
pub struct QnxAllocator {
    /// Binary std/no_std choice
    config:             QnxAllocatorConfig,
    /// Memory partition ID if a dedicated partition was created
    partition_id:       Option<u32>,
    /// Binary std/no_std choice
    current_allocation: Option<NonNull<u8>>,
    /// Binary std/no_std choice
    current_size:       usize,
    /// Binary std/no_std choice
    current_pages:      u32,
    /// Binary std/no_std choice
    maximum_pages:      Option<u32>,
}

impl QnxAllocator {
    /// Create a new QnxAllocator with the specified configuration
    pub fn new(config: QnxAllocatorConfig) -> Self {
        // Create a memory partition if requested
        let partition_id = if config.create_partition {
            // Try to create a partition, but don't fail if it doesn't work
            // In a real implementation, you might want to be more strict
            unsafe {
                let id = ffi::mem_partition_create(
                    0, // no special flags
                    b"wrt_memory\0".as_ptr(),
                    ffi::mem_partition_getid(),
                );
                if id != 0 {
                    Some(id)
                } else {
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            partition_id,
            current_allocation: None,
            current_size: 0,
            current_pages: 0,
            maximum_pages: None,
        }
    }

    /// Activate the memory partition (if created)
    fn activate_partition(&self) -> Result<()> {
        if let Some(id) = self.partition_id {
            let result = unsafe { ffi::mem_partition_setcurrent(id) };
            if result != 0 {
                return Err(Error::runtime_execution_error(
                    "QNX memory allocation failed",
                ));
            }
        }
        Ok(())
    }

    /// Restore the previous memory partition
    fn restore_partition(&self) -> Result<()> {
        if self.partition_id.is_some() {
            // Get the system (parent) partition ID
            let parent_id = unsafe { ffi::mem_partition_getid() };
            let result = unsafe { ffi::mem_partition_setcurrent(parent_id) };
            if result != 0 {
                return Err(Error::new(
                    ErrorCategory::Platform,
                    1,
                    "Failed to set memory partition",
                ));
            }
        }
        Ok(())
    }

    /// Binary std/no_std choice
    fn calculate_total_size(&self, pages: u32) -> Result<usize> {
        let data_size = (pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Memory size calculation overflow"))?;

        let guard_pages = if self.config.use_guard_pages { 2 } else { 0 };
        let guard_size = guard_pages
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Guard page size calculation overflow"))?;

        data_size
            .checked_add(guard_size)
            .ok_or_else(|| Error::memory_error("Total memory size calculation overflow"))
    }

    /// Binary std/no_std choice
    fn free_current_allocation(&mut self) -> Result<()> {
        if let Some(ptr) = self.current_allocation.take() {
            self.activate_partition()?;
            let result = unsafe { ffi::munmap(ptr.as_ptr() as *mut _, self.current_size) };
            self.restore_partition()?;

            if result != 0 {
                return Err(Error::memory_error("Failed to unmap memory"));
            }

            self.current_size = 0;
            self.current_pages = 0;
        }

        Ok(())
    }
}

impl Drop for QnxAllocator {
    fn drop(&mut self) {
        // Binary std/no_std choice
        let _ = self.free_current_allocation();

        // Destroy partition if created
        if let Some(id) = self.partition_id {
            unsafe {
                let _ = ffi::mem_partition_destroy(id);
            }
        }
    }
}

impl PageAllocator for QnxAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)> {
        // Binary std/no_std choice
        self.free_current_allocation()?;

        // Store maximum pages for future reference
        self.maximum_pages = maximum_pages;

        // Calculate total size including guard pages
        let total_size = self.calculate_total_size(initial_pages)?;

        // Activate partition if needed
        self.activate_partition()?;

        // Allocate memory using mmap
        let addr = unsafe {
            ffi::mmap(
                core::ptr::null_mut(),
                total_size,
                self.config.data_page_prot as u32,
                self.config.map_flags as u32,
                -1, // No file descriptor
                0,  // No offset
            )
        };

        // Restore partition
        self.restore_partition()?;

        // Binary std/no_std choice
        if addr == core::ptr::null_mut() || addr == usize::MAX as *mut _ {
            return Err(Error::memory_error("Failed to allocate memory"));
        }

        // Set up guard pages if enabled
        if self.config.use_guard_pages {
            self.activate_partition()?;

            // Protect the lower guard page
            let lower_guard = addr;
            let lower_result = unsafe {
                ffi::mprotect(
                    lower_guard,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Protect the upper guard page
            let upper_guard =
                unsafe { (addr as *mut u8).add((initial_pages as usize) * WASM_PAGE_SIZE) };
            let upper_result = unsafe {
                ffi::mprotect(
                    upper_guard as *mut _,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            self.restore_partition()?;

            // Check if protection succeeded
            if lower_result != 0 || upper_result != 0 {
                // Binary std/no_std choice
                unsafe {
                    ffi::munmap(addr, total_size);
                }

                return Err(Error::memory_error("Failed to set up guard pages"));
            }
        }

        // Calculate the data area pointer (after lower guard page if enabled)
        let data_ptr = if self.config.use_guard_pages {
            unsafe { (addr as *mut u8).add(WASM_PAGE_SIZE) }
        } else {
            addr as *mut u8
        };

        // Binary std/no_std choice
        let data_ptr_nonnull = NonNull::new(data_ptr)
            .ok_or_else(|| Error::memory_error("Failed to allocate memory (null pointer)"))?;

        self.current_allocation = Some(data_ptr_nonnull);
        self.current_size = total_size;
        self.current_pages = initial_pages;

        // Return data pointer and size
        let data_size = (initial_pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Memory size calculation overflow"))?;

        Ok((data_ptr_nonnull, data_size))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<(NonNull<u8>, usize)> {
        // Binary std/no_std choice
        if self.current_allocation.is_none() {
            return Err(Error::memory_error("No current allocation to grow"));
        }

        // Calculate new size
        let new_pages = current_pages
            .checked_add(additional_pages)
            .ok_or_else(|| Error::memory_error("Page count overflow when growing memory"))?;

        // Check against maximum if set
        if let Some(max) = self.maximum_pages {
            if new_pages > max {
                return Err(Error::memory_error(
                    "Cannot grow memory beyond maximum pages",
                ));
            }
        }

        // Binary std/no_std choice
        // memory and copy the contents
        let new_total_size = self.calculate_total_size(new_pages)?;

        // Activate partition if needed
        self.activate_partition()?;

        // Allocate new memory
        let new_addr = unsafe {
            ffi::mmap(
                core::ptr::null_mut(),
                new_total_size,
                self.config.data_page_prot as u32,
                self.config.map_flags as u32,
                -1, // No file descriptor
                0,  // No offset
            )
        };

        // Restore partition
        self.restore_partition()?;

        // Binary std/no_std choice
        if new_addr == core::ptr::null_mut() || new_addr == usize::MAX as *mut _ {
            return Err(Error::memory_error("Failed to allocate memory for growth"));
        }

        // Calculate new data pointer
        let new_data_ptr = if self.config.use_guard_pages {
            unsafe { (new_addr as *mut u8).add(WASM_PAGE_SIZE) }
        } else {
            new_addr as *mut u8
        };

        // Copy existing data to new memory
        let current_ptr = self.current_allocation.unwrap().as_ptr();
        let copy_size = (current_pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Memory size calculation overflow"))?;

        // Binary std/no_std choice
        unsafe {
            core::ptr::copy_nonoverlapping(current_ptr, new_data_ptr, copy_size);
        }

        // Set up guard pages if enabled
        if self.config.use_guard_pages {
            self.activate_partition()?;

            // Protect the lower guard page
            let lower_guard = new_addr;
            let lower_result = unsafe {
                ffi::mprotect(
                    lower_guard,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Protect the upper guard page
            let upper_guard =
                unsafe { (new_addr as *mut u8).add((new_pages as usize) * WASM_PAGE_SIZE) };
            let upper_result = unsafe {
                ffi::mprotect(
                    upper_guard as *mut _,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            self.restore_partition()?;

            // Check if protection succeeded
            if lower_result != 0 || upper_result != 0 {
                // Binary std/no_std choice
                unsafe {
                    ffi::munmap(new_addr, new_total_size);
                }

                return Err(Error::memory_error("Failed to set up guard pages"));
            }
        }

        // Binary std/no_std choice
        self.activate_partition()?;
        let old_addr = if self.config.use_guard_pages {
            unsafe { (self.current_allocation.unwrap().as_ptr()).sub(WASM_PAGE_SIZE) }
        } else {
            self.current_allocation.unwrap().as_ptr()
        };
        unsafe {
            ffi::munmap(old_addr as *mut _, self.current_size);
        }
        self.restore_partition()?;

        // Binary std/no_std choice
        let new_data_ptr_nonnull = NonNull::new(new_data_ptr).ok_or_else(|| {
            Error::memory_error("Failed to allocate memory for growth (null pointer)")
        })?;

        self.current_allocation = Some(new_data_ptr_nonnull);
        self.current_size = new_total_size;
        self.current_pages = new_pages;

        // Return data pointer and size
        let data_size = (new_pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Memory size calculation overflow"))?;

        Ok((new_data_ptr_nonnull, data_size))
    }

    fn free(&mut self) -> Result<()> {
        self.free_current_allocation()
    }

    fn memory_type(&self) -> &'static str {
        "QNX-mmap"
    }

    fn allocated_pages(&self) -> u32 {
        self.current_pages
    }

    fn maximum_pages(&self) -> Option<u32> {
        self.maximum_pages
    }

    fn protect(
        &mut self,
        addr: NonNull<u8>,
        size: usize,
        is_readable: bool,
        is_writable: bool,
        is_executable: bool,
    ) -> Result<()> {
        // Binary std/no_std choice
        if let Some(current) = self.current_allocation {
            let current_addr = current.as_ptr() as usize;
            let addr_val = addr.as_ptr() as usize;
            let data_size = (self.current_pages as usize) * WASM_PAGE_SIZE;

            if addr_val < current_addr || addr_val >= current_addr + data_size {
                return Err(Error::memory_error(
                    "Address to protect is outside allocated memory",
                ));
            }

            if addr_val + size > current_addr + data_size {
                return Err(Error::memory_error(
                    "Protection region extends beyond allocated memory",
                ));
            }
        } else {
            return Err(Error::memory_error("No current allocation to protect"));
        }

        // Determine protection flags
        let mut prot = QnxProtFlags::None as u32;
        if is_readable {
            prot |= QnxProtFlags::Read as u32;
        }
        if is_writable {
            prot |= QnxProtFlags::Write as u32;
        }
        if is_executable {
            prot |= QnxProtFlags::Execute as u32;
        }

        // Activate partition if needed
        self.activate_partition()?;

        // Apply protection
        let result = unsafe { ffi::mprotect(addr.as_ptr() as *mut _, size, prot) };

        // Restore partition
        self.restore_partition()?;

        if result != 0 {
            return Err(Error::memory_error("Failed to apply memory protection"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests would only run on QNX, so they're marked as no_run
    // In a real implementation, you might use conditional compilation
    // to only include these tests when targeting QNX

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_allocator_basic() {
        // Binary std/no_std choice
        let mut allocator = QnxAllocatorBuilder::new().with_guard_pages(true).build().unwrap();

        // Allocate 2 pages
        let result = allocator.allocate(2, Some(4));
        assert!(result.is_ok());

        // Binary std/no_std choice
        let (ptr, size) = result.unwrap();
        assert!(!ptr.as_ptr().is_null());
        assert_eq!(size, 2 * WASM_PAGE_SIZE);

        // Clean up
        let free_result = allocator.free();
        assert!(free_result.is_ok());
    }

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_allocator_grow() {
        // Binary std/no_std choice
        let mut allocator = QnxAllocatorBuilder::new()
            .with_guard_pages(false) // No guard pages for simpler testing
            .build().unwrap();

        // Allocate 1 page
        let result = allocator.allocate(1, Some(4));
        assert!(result.is_ok());

        // Write a test pattern to verify data preservation after grow
        let (ptr, _) = result.unwrap();
        let test_pattern = [0xDE, 0xAD, 0xBE, 0xEF];
        unsafe {
            core::ptr::copy_nonoverlapping(test_pattern.as_ptr(), ptr.as_ptr(), 4);
        }

        // Grow by 1 page
        let grow_result = allocator.grow(1, 1);
        assert!(grow_result.is_ok());

        // Verify the data was preserved
        let (new_ptr, new_size) = grow_result.unwrap();
        assert!(!new_ptr.as_ptr().is_null());
        assert_eq!(new_size, 2 * WASM_PAGE_SIZE);

        let mut preserved_data = [0u8; 4];
        unsafe {
            core::ptr::copy_nonoverlapping(new_ptr.as_ptr(), preserved_data.as_mut_ptr(), 4);
        }
        assert_eq!(preserved_data, test_pattern);

        // Clean up
        let free_result = allocator.free();
        assert!(free_result.is_ok());
    }

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_allocator_protection() {
        // Binary std/no_std choice
        let mut allocator = QnxAllocatorBuilder::new()
            .with_guard_pages(true)
            .with_data_protection(QnxProtFlags::ReadWrite)
            .build()
            .unwrap();

        // Allocate 2 pages
        let result = allocator.allocate(2, None);
        assert!(result.is_ok());

        let (ptr, size) = result.unwrap();

        // Change protection on the second page to read-only
        let second_page_ptr = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(WASM_PAGE_SIZE)) };
        let protect_result = allocator.protect(second_page_ptr, WASM_PAGE_SIZE, true, false, false);
        assert!(protect_result.is_ok());

        // Clean up
        let free_result = allocator.free();
        assert!(free_result.is_ok());
    }
}
