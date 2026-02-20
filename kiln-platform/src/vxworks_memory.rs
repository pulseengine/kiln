use core::ptr::NonNull;

use wrt_error::{
    Error,
    ErrorKind,
};

use crate::{
    PageAllocator,
    WASM_PAGE_SIZE,
};

#[cfg(target_os = "vxworks")]
unsafe extern "C" {
    // Binary std/no_std choice
    fn memPartAlloc(mem_part_id: usize, size: usize) -> *mut u8;
    fn memPartAlignedAlloc(mem_part_id: usize, size: usize, alignment: usize) -> *mut u8;
    fn memPartFree(mem_part_id: usize, ptr: *mut u8) -> i32;
    fn memPartCreate(pool: *mut u8, pool_size: usize) -> usize;
    fn memPartDestroy(mem_part_id: usize) -> i32;

    // Standard C memory functions (available in both contexts)
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn aligned_alloc(alignment: usize, size: usize) -> *mut u8;

    // Memory information
    fn memInfoGet() -> i32;
    fn sysMemTop() -> *mut u8;
}

/// VxWorks execution context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VxWorksContext {
    /// Loadable Kernel Module (LKM) - running in kernel space
    Lkm,
    /// Real-Time Process (RTP) - running in user space
    Rtp,
}

/// Binary std/no_std choice
#[derive(Debug, Clone)]
pub struct VxWorksMemoryConfig {
    pub context:                 VxWorksContext,
    pub max_pages:               usize,
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

/// Binary std/no_std choice
pub struct VxWorksAllocator {
    config:          VxWorksMemoryConfig,
    allocated_pages: usize,
    mem_part_id:     Option<usize>,
    _pool_memory:    Option<Vec<u8>>,
}

impl VxWorksAllocator {
    /// Binary std/no_std choice
    pub fn new(config: VxWorksMemoryConfig) -> Result<Self, Error> {
        let mut allocator = Self {
            config,
            allocated_pages: 0,
            mem_part_id: None,
            _pool_memory: None,
        };

        // Create dedicated memory partition if requested
        if config.use_dedicated_partition {
            allocator.create_memory_partition()?;
        }

        Ok(allocator)
    }

    /// Create a dedicated memory partition for WASM pages
    fn create_memory_partition(&mut self) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            let partition_size =
                self.config.partition_size.unwrap_or(self.config.max_pages * WASM_PAGE_SIZE);

            // Allocate pool memory
            let mut pool_memory = vec![0u8; partition_size];
            let pool_ptr = pool_memory.as_mut_ptr();

            // Create memory partition
            let mem_part_id = unsafe { memPartCreate(pool_ptr, partition_size) };
            if mem_part_id == 0 {
                return Err(Error::runtime_execution_error(
                    "Failed to create VxWorks memory partition",
                ));
            }

            self.mem_part_id = Some(mem_part_id);
            self._pool_memory = Some(pool_memory);
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks memory partition creation not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Allocate memory using the appropriate VxWorks API based on context
    fn allocate_memory(&self, size: usize, alignment: usize) -> Result<*mut u8, Error> {
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
                return Err(Error::runtime_execution_error("Failed to allocate memory"));
            }

            Ok(ptr)
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            Err(Error::runtime_execution_error(
                "VxWorks memory allocation not supported on this platform",
            ))
        }
    }

    /// Free memory using the appropriate VxWorks API
    fn free_memory(&self, ptr: *mut u8) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            match self.mem_part_id {
                Some(mem_part_id) => {
                    let result = unsafe { memPartFree(mem_part_id, ptr) };
                    if result != 0 {
                        return Err(Error::runtime_execution_error(
                            "Failed to free memory from partition",
                        ));
                    }
                },
                None => {
                    unsafe { free(ptr) };
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks memory free not supported on this platform",
            ));
        }

        Ok(())
    }
}

impl PageAllocator for VxWorksAllocator {
    fn allocate_pages(&mut self, pages: usize) -> Result<NonNull<u8>, Error> {
        if self.allocated_pages + pages > self.config.max_pages {
            return Err(Error::new(
                ErrorKind::Memory,
                "Maximum page allocation exceeded",
            ));
        }

        let size = pages * WASM_PAGE_SIZE;
        let alignment = WASM_PAGE_SIZE; // 64KB alignment for WASM pages

        let ptr = self.allocate_memory(size, alignment)?;

        // Binary std/no_std choice
        unsafe {
            core::ptr::write_bytes(ptr, 0, size);
        }

        self.allocated_pages += pages;

        NonNull::new(ptr).ok_or_else(|| Error::runtime_execution_error("Memory allocation failed"))
    }

    fn deallocate_pages(&mut self, ptr: NonNull<u8>, pages: usize) -> Result<(), Error> {
        if pages > self.allocated_pages {
            return Err(Error::new(
                ErrorKind::Memory,
                "Invalid page count for deallocation",
            ));
        }

        self.free_memory(ptr.as_ptr())?;
        self.allocated_pages -= pages;

        Ok(())
    }

    fn grow_pages(
        &mut self,
        old_ptr: NonNull<u8>,
        old_pages: usize,
        new_pages: usize,
    ) -> Result<NonNull<u8>, Error> {
        if new_pages <= old_pages {
            return Ok(old_ptr);
        }

        let additional_pages = new_pages - old_pages;
        if self.allocated_pages + additional_pages > self.config.max_pages {
            return Err(Error::runtime_execution_error(
                "Cannot grow pages beyond maximum limit",
            ));
        }

        // Binary std/no_std choice
        let new_ptr = self.allocate_pages(new_pages)?;

        // Copy old data
        let old_size = old_pages * WASM_PAGE_SIZE;
        unsafe {
            core::ptr::copy_nonoverlapping(old_ptr.as_ptr(), new_ptr.as_ptr(), old_size);
        }

        // Free old memory
        self.allocated_pages -= old_pages; // Binary std/no_std choice
        self.deallocate_pages(old_ptr, old_pages)?;

        Ok(new_ptr)
    }

    fn allocated_pages(&self) -> usize {
        self.allocated_pages
    }

    fn max_pages(&self) -> usize {
        self.config.max_pages
    }
}

impl Drop for VxWorksAllocator {
    fn drop(&mut self) {
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

/// Binary std/no_std choice
pub struct VxWorksAllocatorBuilder {
    config: VxWorksMemoryConfig,
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

    pub fn max_pages(mut self, max_pages: usize) -> Self {
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

    pub fn build(self) -> Result<VxWorksAllocator, Error> {
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
