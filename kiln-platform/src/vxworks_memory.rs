use core::ptr::NonNull;

use kiln_error::{
    Error,
    ErrorCategory,
    Result,
    codes,
};

use crate::memory::{
    PageAllocator,
    WASM_PAGE_SIZE,
};

#[cfg(target_os = "vxworks")]
unsafe extern "C" {
    fn memPartAlloc(mem_part_id: usize, size: usize) -> *mut u8;
    fn memPartAlignedAlloc(mem_part_id: usize, size: usize, alignment: usize) -> *mut u8;
    fn memPartFree(mem_part_id: usize, ptr: *mut u8) -> i32;
    fn memPartCreate(pool: *mut u8, pool_size: usize) -> usize;
    fn memPartDestroy(mem_part_id: usize) -> i32;

    // Standard C memory functions (available in both contexts)
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn aligned_alloc(alignment: usize, size: usize) -> *mut u8;
}

/// VxWorks execution context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VxWorksContext {
    /// Loadable Kernel Module (LKM) - running in kernel space
    Lkm,
    /// Real-Time Process (RTP) - running in user space
    Rtp,
}

/// Configuration for VxWorks memory allocation
#[derive(Debug, Clone)]
pub struct VxWorksMemoryConfig {
    pub context:                 VxWorksContext,
    pub max_pages:               u32,
    pub use_dedicated_partition: bool,
    pub partition_size:          Option<usize>,
    pub enable_guard_pages:      bool,
}

impl Default for VxWorksMemoryConfig {
    fn default() -> Self {
        Self {
            context:                 VxWorksContext::Rtp,
            max_pages:               1024,
            use_dedicated_partition: false,
            partition_size:          None,
            enable_guard_pages:      false,
        }
    }
}

/// VxWorks page allocator supporting both LKM and RTP contexts
#[derive(Debug)]
pub struct VxWorksAllocator {
    config:             VxWorksMemoryConfig,
    mem_part_id:        Option<usize>,
    current_allocation: Option<NonNull<u8>>,
    current_size:       usize,
    current_pages:      u32,
    maximum_pages:      Option<u32>,
    #[cfg(target_os = "vxworks")]
    _pool_memory:       Option<Vec<u8>>,
}

// Safety: VxWorksAllocator only contains pointers to VxWorks kernel objects
// which are thread-safe, and the allocator is used single-threaded per instance.
unsafe impl Send for VxWorksAllocator {}
unsafe impl Sync for VxWorksAllocator {}

impl VxWorksAllocator {
    /// Create a new VxWorks allocator with the given configuration
    pub fn new(config: VxWorksMemoryConfig) -> Result<Self> {
        let mut allocator = Self {
            config:             config.clone(),
            mem_part_id:        None,
            current_allocation: None,
            current_size:       0,
            current_pages:      0,
            maximum_pages:      None,
            #[cfg(target_os = "vxworks")]
            _pool_memory:       None,
        };

        // Create dedicated memory partition if requested
        if config.use_dedicated_partition {
            allocator.create_memory_partition()?;
        }

        Ok(allocator)
    }

    /// Create a dedicated memory partition for WASM pages
    fn create_memory_partition(&mut self) -> Result<()> {
        #[cfg(target_os = "vxworks")]
        {
            let partition_size = self
                .config
                .partition_size
                .unwrap_or(self.config.max_pages as usize * WASM_PAGE_SIZE);

            // Allocate pool memory
            let mut pool_memory = vec![0u8; partition_size];
            let pool_ptr = pool_memory.as_mut_ptr();

            // Create memory partition
            let mem_part_id = unsafe { memPartCreate(pool_ptr, partition_size) };
            if mem_part_id == 0 {
                return Err(Error::platform_memory_allocation_failed(
                    "Failed to create VxWorks memory partition",
                ));
            }

            self.mem_part_id = Some(mem_part_id);
            self._pool_memory = Some(pool_memory);
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        Err(Error::platform_memory_allocation_failed(
            "VxWorks memory partition creation not supported on this platform",
        ))
    }

    /// Allocate memory using the appropriate VxWorks API based on context
    fn allocate_memory(&self, size: usize, alignment: usize) -> Result<*mut u8> {
        #[cfg(target_os = "vxworks")]
        {
            let ptr = match (self.mem_part_id, alignment) {
                // Use dedicated partition with alignment
                (Some(mem_part_id), align) if align > 1 => unsafe {
                    memPartAlignedAlloc(mem_part_id, size, align)
                },
                // Use dedicated partition without alignment
                (Some(mem_part_id), _) => unsafe { memPartAlloc(mem_part_id, size) },
                // Use system memory with alignment
                (None, align) if align > 1 => unsafe { aligned_alloc(align, size) },
                // Use system memory without alignment
                (None, _) => unsafe { malloc(size) },
            };

            if ptr.is_null() {
                return Err(Error::platform_memory_allocation_failed(
                    "Failed to allocate memory",
                ));
            }

            Ok(ptr)
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            let _ = (size, alignment);
            Err(Error::platform_memory_allocation_failed(
                "VxWorks memory allocation not supported on this platform",
            ))
        }
    }

    /// Free memory using the appropriate VxWorks API
    fn free_memory(&self, ptr: *mut u8) -> Result<()> {
        #[cfg(target_os = "vxworks")]
        {
            match self.mem_part_id {
                Some(mem_part_id) => {
                    let result = unsafe { memPartFree(mem_part_id, ptr) };
                    if result != 0 {
                        return Err(Error::platform_memory_allocation_failed(
                            "Failed to free memory from partition",
                        ));
                    }
                },
                None => {
                    unsafe { free(ptr) };
                },
            }
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            let _ = ptr;
            Err(Error::platform_memory_allocation_failed(
                "VxWorks memory free not supported on this platform",
            ))
        }
    }

    /// Convert page count to byte size with overflow check
    fn pages_to_bytes(pages: u32) -> Result<usize> {
        (pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Page count overflow"))
    }
}

impl PageAllocator for VxWorksAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)> {
        if self.current_allocation.is_some() {
            return Err(Error::memory_error(
                "Memory already allocated; deallocate first",
            ));
        }

        if initial_pages == 0 {
            return Err(Error::memory_error("Initial pages cannot be zero"));
        }

        let max_pages = maximum_pages
            .unwrap_or(self.config.max_pages)
            .min(self.config.max_pages);

        if initial_pages > max_pages {
            return Err(Error::memory_error(
                "Initial pages exceed maximum page limit",
            ));
        }

        let size = Self::pages_to_bytes(initial_pages)?;
        let alignment = WASM_PAGE_SIZE; // 64KB alignment for WASM pages

        let ptr = self.allocate_memory(size, alignment)?;

        // Zero-initialize memory
        unsafe {
            core::ptr::write_bytes(ptr, 0, size);
        }

        let nonnull = NonNull::new(ptr).ok_or_else(|| {
            Error::platform_memory_allocation_failed("Memory allocation returned null")
        })?;

        self.current_allocation = Some(nonnull);
        self.current_size = size;
        self.current_pages = initial_pages;
        self.maximum_pages = Some(max_pages);

        Ok((nonnull, size))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
        let Some(old_ptr) = self.current_allocation else {
            return Err(Error::memory_error("No current allocation to grow"));
        };

        if additional_pages == 0 {
            return Ok(());
        }

        let current_bytes = Self::pages_to_bytes(current_pages)?;
        if current_bytes != self.current_size {
            return Err(Error::memory_error(
                "Current page count does not match internal state",
            ));
        }

        let new_pages = current_pages
            .checked_add(additional_pages)
            .ok_or_else(|| Error::memory_error("Page count overflow during grow"))?;

        if let Some(max) = self.maximum_pages {
            if new_pages > max {
                return Err(Error::memory_error(
                    "Cannot grow memory beyond maximum pages",
                ));
            }
        }

        let new_size = Self::pages_to_bytes(new_pages)?;
        let alignment = WASM_PAGE_SIZE;

        // Allocate new region, copy, free old
        let new_ptr = self.allocate_memory(new_size, alignment)?;

        unsafe {
            // Zero-initialize entire new region
            core::ptr::write_bytes(new_ptr, 0, new_size);
            // Copy old data
            core::ptr::copy_nonoverlapping(old_ptr.as_ptr(), new_ptr, current_bytes);
        }

        // Free old allocation
        self.free_memory(old_ptr.as_ptr())?;

        let nonnull = NonNull::new(new_ptr).ok_or_else(|| {
            Error::platform_memory_allocation_failed("Grow allocation returned null")
        })?;

        self.current_allocation = Some(nonnull);
        self.current_size = new_size;
        self.current_pages = new_pages;

        Ok(())
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, _size: usize) -> Result<()> {
        let Some(current_ptr) = self.current_allocation.take() else {
            return Err(Error::memory_error("No memory allocated to deallocate"));
        };

        if ptr.as_ptr() != current_ptr.as_ptr() {
            self.current_allocation = Some(current_ptr); // Restore
            return Err(Error::memory_error(
                "Attempted to deallocate with mismatched pointer",
            ));
        }

        self.free_memory(ptr.as_ptr())?;
        self.current_size = 0;
        self.current_pages = 0;

        Ok(())
    }
}

impl Drop for VxWorksAllocator {
    fn drop(&mut self) {
        // Free current allocation if any
        if let Some(ptr) = self.current_allocation.take() {
            let _ = self.free_memory(ptr.as_ptr());
        }

        #[cfg(target_os = "vxworks")]
        {
            if let Some(mem_part_id) = self.mem_part_id {
                unsafe {
                    memPartDestroy(mem_part_id);
                }
            }
        }
    }
}

/// Builder for VxWorks allocator
pub struct VxWorksAllocatorBuilder {
    config: VxWorksMemoryConfig,
}

impl core::fmt::Debug for VxWorksAllocatorBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VxWorksAllocatorBuilder")
            .field("config", &self.config)
            .finish()
    }
}

impl VxWorksAllocatorBuilder {
    pub fn new() -> Self {
        Self {
            config: VxWorksMemoryConfig::default(),
        }
    }

    pub fn context(mut self, context: VxWorksContext) -> Self {
        self.config.context = context;
        self
    }

    pub fn max_pages(mut self, max_pages: u32) -> Self {
        self.config.max_pages = max_pages;
        self
    }

    pub fn use_dedicated_partition(mut self, use_partition: bool) -> Self {
        self.config.use_dedicated_partition = use_partition;
        self
    }

    pub fn partition_size(mut self, size: usize) -> Self {
        self.config.partition_size = Some(size);
        self
    }

    pub fn enable_guard_pages(mut self, enable: bool) -> Self {
        self.config.enable_guard_pages = enable;
        self
    }

    pub fn build(self) -> Result<VxWorksAllocator> {
        VxWorksAllocator::new(self.config)
    }
}

impl Default for VxWorksAllocatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vxworks_allocator_builder() {
        let allocator = VxWorksAllocatorBuilder::new()
            .context(VxWorksContext::Lkm)
            .max_pages(512)
            .use_dedicated_partition(true)
            .enable_guard_pages(true)
            .build();

        #[cfg(target_os = "vxworks")]
        assert!(allocator.is_ok());

        #[cfg(not(target_os = "vxworks"))]
        assert!(allocator.is_err());
    }

    #[test]
    fn test_context_types() {
        let lkm_config = VxWorksMemoryConfig {
            context: VxWorksContext::Lkm,
            ..Default::default()
        };

        let rtp_config = VxWorksMemoryConfig {
            context: VxWorksContext::Rtp,
            ..Default::default()
        };

        assert_eq!(lkm_config.context, VxWorksContext::Lkm);
        assert_eq!(rtp_config.context, VxWorksContext::Rtp);
    }
}
