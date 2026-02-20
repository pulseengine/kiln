//! QNX-specific arena-based memory allocation.
//!
//! Provides a memory allocator implementation that leverages QNX Neutrino's
//! native arena allocator, designed for efficient memory management in
//! safety-critical systems with no_std and no_alloc requirements.
//!
//! This module interfaces with QNX's arena allocation system to provide
//! WebAssembly memory that is both efficient and well-integrated with the OS.

use core::{
    fmt::{
        self,
        Debug,
    },
    ptr::NonNull,
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
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

/// Binary std/no_std choice
/// These map to mallopt() parameters
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnxMallocOption {
    /// Arena size (default 32KB)
    ArenaSize           = 1,
    /// Maximum number of cached arena blocks
    ArenaCacheMaxBlocks = 2,
    /// Maximum size of arena cache
    ArenaCacheMaxSize   = 3,
    /// Free mode (LIFO vs FIFO)
    FifoFree            = 4,
    /// Hold memory (never release to OS)
    MemoryHold          = 5,
    /// Trim threshold
    TrimThreshold       = 6,
}

/// Binary std/no_std choice
#[allow(non_camel_case_types)]
#[cfg(all(feature = "platform-qnx", target_os = "nto"))]
mod ffi {
    use core::ffi::c_void;

    // QNX-specific types
    pub type qnx_size_t = usize;
    pub type qnx_ssize_t = isize;
    pub type qnx_pid_t = i32;
    pub type qnx_off_t = i64;

    // SAFETY: Edition 2024 requires unsafe extern blocks
    unsafe extern "C" {
        // Binary std/no_std choice
        pub fn malloc(size: qnx_size_t) -> *mut c_void;
        pub fn calloc(nmemb: qnx_size_t, size: qnx_size_t) -> *mut c_void;
        pub fn realloc(ptr: *mut c_void, size: qnx_size_t) -> *mut c_void;
        pub fn free(ptr: *mut c_void);

        // Binary std/no_std choice
        pub fn mallopt(cmd: i32, value: i32) -> i32;
        pub fn mallinfo() -> MallocInfo;

        // Low-level memory management
        pub fn mmap(
            addr: *mut c_void,
            len: qnx_size_t,
            prot: u32,
            flags: u32,
            fd: i32,
            offset: qnx_off_t,
        ) -> *mut c_void;

        pub fn munmap(addr: *mut c_void, len: qnx_size_t) -> i32;

        pub fn mprotect(addr: *mut c_void, len: qnx_size_t, prot: u32) -> i32;

        // Memory locking functions
        pub fn mlock(addr: *const c_void, len: qnx_size_t) -> i32;
        pub fn munlock(addr: *const c_void, len: qnx_size_t) -> i32;

        // Misc memory functions
        pub fn posix_memalign(
            memptr: *mut *mut c_void,
            alignment: qnx_size_t,
            size: qnx_size_t,
        ) -> i32;
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct MallocInfo {
        pub arena:    i32, // Binary std/no_std choice
        pub ordblks:  i32, // Number of free chunks
        pub smblks:   i32, // Number of fast bins
        pub hblks:    i32, // Number of mmapped regions
        pub hblkhd:   i32, // Binary std/no_std choice
        pub usmblks:  i32, // Binary std/no_std choice
        pub fsmblks:  i32, // Space in freed fastbin blocks
        pub uordblks: i32, // Binary std/no_std choice
        pub fordblks: i32, // Total free space
        pub keepcost: i32, // Top-most, releasable space
    }
}

// Mock implementation for non-QNX targets for build compatibility
#[cfg(not(all(feature = "platform-qnx", target_os = "nto")))]
mod ffi {
    use core::ffi::c_void;

    // Mock types
    pub type qnx_size_t = usize;
    pub type qnx_ssize_t = isize;
    pub type qnx_pid_t = i32;
    pub type qnx_off_t = i64;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct MallocInfo {
        pub arena:    i32,
        pub ordblks:  i32,
        pub smblks:   i32,
        pub hblks:    i32,
        pub hblkhd:   i32,
        pub usmblks:  i32,
        pub fsmblks:  i32,
        pub uordblks: i32,
        pub fordblks: i32,
        pub keepcost: i32,
    }

    // Mock implementations for build compatibility
    // These functions are never called outside of QNX targets
    #[allow(unused)]
    pub unsafe fn malloc(_size: qnx_size_t) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[allow(unused)]
    pub unsafe fn free(_ptr: *mut c_void) {}

    #[allow(unused)]
    pub unsafe fn realloc(_ptr: *mut c_void, _size: qnx_size_t) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[allow(unused)]
    pub unsafe fn mallopt(_cmd: i32, _value: i32) -> i32 {
        0
    }

    #[allow(unused)]
    pub unsafe fn mallinfo() -> MallocInfo {
        MallocInfo {
            arena:    0,
            ordblks:  0,
            smblks:   0,
            hblks:    0,
            hblkhd:   0,
            usmblks:  0,
            fsmblks:  0,
            uordblks: 0,
            fordblks: 0,
            keepcost: 0,
        }
    }

    #[allow(unused)]
    pub unsafe fn mmap(
        _addr: *mut c_void,
        _len: qnx_size_t,
        _prot: u32,
        _flags: u32,
        _fd: i32,
        _offset: qnx_off_t,
    ) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[allow(unused)]
    pub unsafe fn munmap(_addr: *mut c_void, _len: qnx_size_t) -> i32 {
        0
    }

    #[allow(unused)]
    pub unsafe fn mprotect(_addr: *mut c_void, _len: qnx_size_t, _prot: u32) -> i32 {
        0
    }

    #[allow(unused)]
    pub unsafe fn mlock(_addr: *const c_void, _len: qnx_size_t) -> i32 {
        0
    }

    #[allow(unused)]
    pub unsafe fn munlock(_addr: *const c_void, _len: qnx_size_t) -> i32 {
        0
    }

    #[allow(unused)]
    pub unsafe fn posix_memalign(
        _memptr: *mut *mut c_void,
        _alignment: qnx_size_t,
        _size: qnx_size_t,
    ) -> i32 {
        0
    }
}

/// Configuration for QnxArenaAllocator
#[derive(Debug, Clone)]
pub struct QnxArenaAllocatorConfig {
    /// Size of each arena in bytes (default 32KB, must be multiple of 4KB)
    pub arena_size:             usize,
    /// Maximum number of arena blocks to cache
    pub arena_cache_max_blocks: usize,
    /// Maximum size of arena cache in bytes
    pub arena_cache_max_size:   usize,
    /// Whether to use LIFO free strategy (default is FIFO)
    pub use_lifo_free:          bool,
    /// Whether to hold memory (never release to OS)
    pub memory_hold:            bool,
    /// Binary std/no_std choice
    pub use_guard_pages:        bool,
    /// Memory protection flags for guard pages
    pub guard_page_prot:        QnxProtFlags,
    /// Memory protection flags for data pages
    pub data_page_prot:         QnxProtFlags,
    /// Verification level for memory operations
    pub verification_level:     VerificationLevel,
}

impl Default for QnxArenaAllocatorConfig {
    fn default() -> Self {
        Self {
            arena_size:             32 * 1024,  // 32KB default in QNX
            arena_cache_max_blocks: 8,          // Default in QNX
            arena_cache_max_size:   256 * 1024, // 256KB
            use_lifo_free:          false,
            memory_hold:            false,
            use_guard_pages:        true,
            guard_page_prot:        QnxProtFlags::None,
            data_page_prot:         QnxProtFlags::ReadWrite,
            verification_level:     VerificationLevel::Standard,
        }
    }
}

/// Builder for QnxArenaAllocator
#[derive(Debug, Default)]
pub struct QnxArenaAllocatorBuilder {
    config: QnxArenaAllocatorConfig,
}

impl QnxArenaAllocatorBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the arena size (must be multiple of 4KB, max 256KB)
    pub fn with_arena_size(mut self, size: usize) -> Self {
        // Ensure arena size is a multiple of page size (4KB)
        let page_size = 4 * 1024;
        let aligned_size = (size + page_size - 1) / page_size * page_size;
        // Cap at 256KB (QNX limit)
        let capped_size = aligned_size.min(256 * 1024);
        self.config.arena_size = capped_size;
        self
    }

    /// Set the maximum number of arena blocks to cache
    pub fn with_arena_cache_max_blocks(mut self, blocks: usize) -> Self {
        self.config.arena_cache_max_blocks = blocks;
        self
    }

    /// Set the maximum size of arena cache
    pub fn with_arena_cache_max_size(mut self, size: usize) -> Self {
        self.config.arena_cache_max_size = size;
        self
    }

    /// Configure whether to use LIFO free strategy
    pub fn with_lifo_free(mut self, use_lifo: bool) -> Self {
        self.config.use_lifo_free = use_lifo;
        self
    }

    /// Configure whether to hold memory (never release to OS)
    pub fn with_memory_hold(mut self, hold: bool) -> Self {
        self.config.memory_hold = hold;
        self
    }

    /// Configure whether to use guard pages
    pub fn with_guard_pages(mut self, use_guard_pages: bool) -> Self {
        self.config.use_guard_pages = use_guard_pages;
        self
    }

    /// Set memory protection flags for data pages
    pub fn with_data_protection(mut self, prot: QnxProtFlags) -> Self {
        self.config.data_page_prot = prot;
        self
    }

    /// Set guard page protection flags
    pub fn with_guard_protection(mut self, prot: QnxProtFlags) -> Self {
        self.config.guard_page_prot = prot;
        self
    }

    /// Set verification level for memory operations
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.config.verification_level = level;
        self
    }

    /// Build the QnxArenaAllocator with the configured settings
    pub fn build(self) -> Result<QnxArenaAllocator> {
        QnxArenaAllocator::new(self.config)
    }
}

/// Binary std/no_std choice
#[derive(Debug)]
pub struct QnxArenaAllocator {
    /// Binary std/no_std choice
    config:             QnxArenaAllocatorConfig,
    /// Binary std/no_std choice
    current_allocation: Option<NonNull<u8>>,
    /// Binary std/no_std choice
    current_size:       AtomicUsize,
    /// Binary std/no_std choice
    current_pages:      AtomicUsize,
    /// Binary std/no_std choice
    maximum_pages:      Option<u32>,
    /// Binary std/no_std choice
    initialized:        bool,
}

impl QnxArenaAllocator {
    /// Create a new QnxArenaAllocator with the specified configuration
    pub fn new(config: QnxArenaAllocatorConfig) -> Result<Self> {
        // Binary std/no_std choice
        Self::configure_arena_allocator(&config)?;

        Ok(Self {
            config,
            current_allocation: None,
            current_size: AtomicUsize::new(0),
            current_pages: AtomicUsize::new(0),
            maximum_pages: None,
            initialized: true,
        })
    }

    /// Binary std/no_std choice
    fn configure_arena_allocator(config: &QnxArenaAllocatorConfig) -> Result<()> {
        // Set arena size
        let result =
            unsafe { ffi::mallopt(QnxMallocOption::ArenaSize as i32, config.arena_size as i32) };
        if result != 0 {
            return Err(Error::runtime_execution_error("Failed to set arena size"));
        }

        // Set arena cache max blocks
        let result = unsafe {
            ffi::mallopt(
                QnxMallocOption::ArenaCacheMaxBlocks as i32,
                config.arena_cache_max_blocks as i32,
            )
        };
        if result != 0 {
            return Err(Error::new(
                ErrorCategory::Platform,
                1,
                "Failed to set arena cache max blocks",
            ));
        }

        // Set arena cache max size
        let result = unsafe {
            ffi::mallopt(
                QnxMallocOption::ArenaCacheMaxSize as i32,
                config.arena_cache_max_size as i32,
            )
        };
        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to set arena cache max size",
            ));
        }

        // Set LIFO free strategy if requested
        if config.use_lifo_free {
            let result = unsafe {
                ffi::mallopt(
                    QnxMallocOption::FifoFree as i32,
                    0, // 0 for LIFO, 1 for FIFO
                )
            };
            if result != 0 {
                return Err(Error::new(
                    ErrorCategory::Platform,
                    1,
                    "Failed to set LIFO free strategy",
                ));
            }
        }

        // Set memory hold if requested
        if config.memory_hold {
            let result = unsafe {
                ffi::mallopt(
                    QnxMallocOption::MemoryHold as i32,
                    1, // 1 to hold memory
                )
            };
            if result != 0 {
                return Err(Error::runtime_execution_error("Failed to set memory hold"));
            }
        }

        Ok(())
    }

    /// Binary std/no_std choice
    fn calculate_total_size(&self, pages: u32) -> Result<usize> {
        let data_size = (pages as usize)
            .checked_mul(WASM_PAGE_SIZE)
            .ok_or_else(|| Error::memory_error("Data page size calculation overflow"))?;

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
            unsafe {
                // Binary std/no_std choice
                ffi::free(ptr.as_ptr() as *mut _);
            }

            self.current_size.store(0, Ordering::SeqCst);
            self.current_pages.store(0, Ordering::SeqCst);
        }

        Ok(())
    }

    /// Binary std/no_std choice
    pub fn memory_info(&self) -> Result<ffi::MallocInfo> {
        Ok(unsafe { ffi::mallinfo() })
    }
}

impl Drop for QnxArenaAllocator {
    fn drop(&mut self) {
        // Binary std/no_std choice
        let _ = self.free_current_allocation();
    }
}

impl PageAllocator for QnxArenaAllocator {
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

        // Binary std/no_std choice
        // Binary std/no_std choice
        let arenas_needed = (total_size + self.config.arena_size - 1) / self.config.arena_size;
        let aligned_size = arenas_needed * self.config.arena_size;

        // Binary std/no_std choice
        // This helps reduce fragmentation for WebAssembly modules
        if aligned_size > 256 * 1024 {
            // Binary std/no_std choice
            // Binary std/no_std choice
            unsafe {
                ffi::mallopt(
                    QnxMallocOption::ArenaCacheMaxSize as i32,
                    (aligned_size / 2) as i32, // Binary std/no_std choice
                );
            }
        }

        // Allocate memory using arenas (posix_memalign for alignment)
        let mut ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        let alignment = if self.config.use_guard_pages {
            WASM_PAGE_SIZE // Align to WASM page size for guard pages
        } else {
            16 // Default alignment
        };

        // Binary std/no_std choice
        let result = unsafe { ffi::posix_memalign(&mut ptr, alignment, total_size) };

        // Binary std/no_std choice
        if (result != 0 || ptr.is_null()) && total_size > 0 {
            ptr = unsafe { ffi::malloc(total_size) };
        }

        if ptr.is_null() {
            // Reset arena cache size if we modified it
            if aligned_size > 256 * 1024 {
                unsafe {
                    ffi::mallopt(
                        QnxMallocOption::ArenaCacheMaxSize as i32,
                        self.config.arena_cache_max_size as i32,
                    );
                }
            }

            return Err(Error::memory_error(
                "Failed to allocate memory using arena allocator",
            ));
        }

        // Lock memory in physical RAM if this is a large, performance-critical
        // Binary std/no_std choice
        if initial_pages > 16 {
            unsafe {
                ffi::mlock(ptr, total_size);
            }
        }

        // Set up guard pages if enabled
        if self.config.use_guard_pages {
            // Protect the lower guard page
            let lower_guard = ptr;
            let lower_result = unsafe {
                ffi::mprotect(
                    lower_guard,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Protect the upper guard page
            let upper_guard =
                unsafe { (ptr as *mut u8).add((initial_pages as usize) * WASM_PAGE_SIZE) };
            let upper_result = unsafe {
                ffi::mprotect(
                    upper_guard as *mut _,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Check if protection succeeded
            if lower_result != 0 || upper_result != 0 {
                // Binary std/no_std choice
                unsafe {
                    ffi::free(ptr);
                }

                return Err(Error::memory_error("Failed to set up guard pages"));
            }
        }

        // Calculate the data area pointer (after lower guard page if enabled)
        let data_ptr = if self.config.use_guard_pages {
            unsafe { (ptr as *mut u8).add(WASM_PAGE_SIZE) }
        } else {
            ptr as *mut u8
        };

        // Binary std/no_std choice
        let data_ptr_nonnull = NonNull::new(data_ptr)
            .ok_or_else(|| Error::memory_error("Failed to allocate memory (null pointer)"))?;

        self.current_allocation = Some(data_ptr_nonnull);
        self.current_size.store(total_size, Ordering::SeqCst);
        self.current_pages.store(initial_pages as usize, Ordering::SeqCst);

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

        // Calculate new total size
        let new_total_size = self.calculate_total_size(new_pages)?;

        // Pre-calculate the number of arenas needed for the additional memory
        // Binary std/no_std choice
        let current_total_size = self.current_size.load(Ordering::SeqCst);
        let additional_size = new_total_size.checked_sub(current_total_size).unwrap_or(0);
        let additional_arenas_needed =
            (additional_size + self.config.arena_size - 1) / self.config.arena_size;

        // Binary std/no_std choice
        // Get the current base pointer (before guard page if any)
        let current_ptr = self.current_allocation.unwrap().as_ptr();
        let base_ptr = if self.config.use_guard_pages {
            unsafe { current_ptr.sub(WASM_PAGE_SIZE) }
        } else {
            current_ptr
        };

        // For large growth operations, pre-expand the arena cache to reduce
        // fragmentation
        if additional_arenas_needed > 4 {
            let cache_size = additional_arenas_needed * self.config.arena_size;
            unsafe {
                // Temporarily increase arena cache size
                ffi::mallopt(QnxMallocOption::ArenaCacheMaxSize as i32, cache_size as i32);

                // Temporarily increase arena cache max blocks
                ffi::mallopt(
                    QnxMallocOption::ArenaCacheMaxBlocks as i32,
                    (additional_arenas_needed + 2) as i32,
                );
            }
        }

        // Binary std/no_std choice
        // First try with the exact size we need
        let mut new_ptr = unsafe { ffi::realloc(base_ptr as *mut _, new_total_size) };

        // If that fails, try with a larger size that's aligned to arena boundaries
        // Binary std/no_std choice
        if new_ptr.is_null() && additional_arenas_needed > 0 {
            let aligned_size =
                current_total_size + additional_arenas_needed * self.config.arena_size;
            new_ptr = unsafe { ffi::realloc(base_ptr as *mut _, aligned_size) };
        }

        // Binary std/no_std choice
        if new_ptr.is_null() {
            // Binary std/no_std choice
            new_ptr = unsafe { ffi::malloc(new_total_size) };

            if !new_ptr.is_null() {
                // Copy data from old buffer to new buffer
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        base_ptr as *const u8,
                        new_ptr as *mut u8,
                        current_total_size,
                    );

                    // Free the old buffer
                    ffi::free(base_ptr as *mut _);
                }
            }
        }

        // Reset arena cache configuration if we modified it
        if additional_arenas_needed > 4 {
            unsafe {
                // Restore original arena cache settings
                ffi::mallopt(
                    QnxMallocOption::ArenaCacheMaxSize as i32,
                    self.config.arena_cache_max_size as i32,
                );

                ffi::mallopt(
                    QnxMallocOption::ArenaCacheMaxBlocks as i32,
                    self.config.arena_cache_max_blocks as i32,
                );
            }
        }

        if new_ptr.is_null() {
            return Err(Error::memory_error(
                "Failed to grow memory using arena allocator",
            ));
        }

        // Binary std/no_std choice
        if additional_pages > 8 {
            let current_size = self.current_size.load(Ordering::SeqCst);
            if new_total_size > current_size {
                unsafe {
                    let additional_memory = (new_ptr as *mut u8).add(current_size);
                    ffi::mlock(additional_memory as *const _, new_total_size - current_size);
                }
            }
        }

        // Calculate new data pointer
        let new_data_ptr = if self.config.use_guard_pages {
            unsafe { (new_ptr as *mut u8).add(WASM_PAGE_SIZE) }
        } else {
            new_ptr as *mut u8
        };

        // Binary std/no_std choice
        if self.config.use_guard_pages {
            // Protect the lower guard page
            let lower_guard = new_ptr;
            let lower_result = unsafe {
                ffi::mprotect(
                    lower_guard,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Protect the upper guard page
            let upper_guard =
                unsafe { (new_ptr as *mut u8).add((new_pages as usize) * WASM_PAGE_SIZE) };
            let upper_result = unsafe {
                ffi::mprotect(
                    upper_guard as *mut _,
                    WASM_PAGE_SIZE,
                    self.config.guard_page_prot as u32,
                )
            };

            // Check if protection succeeded
            if lower_result != 0 || upper_result != 0 {
                // Try to restore the original size or at least keep it valid
                unsafe {
                    ffi::realloc(new_ptr, self.current_size.load(Ordering::SeqCst));
                }

                return Err(Error::memory_error(
                    "Failed to set up guard pages after growth",
                ));
            }
        }

        // Binary std/no_std choice
        let new_data_ptr_nonnull = NonNull::new(new_data_ptr)
            .ok_or_else(|| Error::memory_error("Failed to grow memory (null pointer)"))?;

        self.current_allocation = Some(new_data_ptr_nonnull);
        self.current_size.store(new_total_size, Ordering::SeqCst);
        self.current_pages.store(new_pages as usize, Ordering::SeqCst);

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
        "QNX-arena"
    }

    fn allocated_pages(&self) -> u32 {
        self.current_pages.load(Ordering::SeqCst) as u32
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
            let data_size = self.current_pages.load(Ordering::SeqCst) * WASM_PAGE_SIZE;

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

        // Apply protection
        let result = unsafe { ffi::mprotect(addr.as_ptr() as *mut _, size, prot) };

        if result != 0 {
            return Err(Error::memory_error("Failed to apply memory protection"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests would only run on QNX, so they're marked as ignore
    // In a real implementation, you might use conditional compilation
    // to only include these tests when targeting QNX

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_arena_allocator_basic() {
        // Binary std/no_std choice
        let mut allocator = QnxArenaAllocatorBuilder::new()
            .with_arena_size(64 * 1024) // 64KB arenas
            .with_guard_pages(true)
            .build()
            .expect("Failed to create arena allocator");

        // Allocate 2 pages
        let result = allocator.allocate(2, Some(4));
        assert!(result.is_ok());

        // Binary std/no_std choice
        let (ptr, size) = result.unwrap();
        assert!(!ptr.as_ptr().is_null());
        assert_eq!(size, 2 * WASM_PAGE_SIZE);

        // Get memory info
        let info = allocator.memory_info().unwrap();
        println!("Arena size: {}", info.arena);
        println!("Allocated space: {}", info.uordblks);

        // Clean up
        let free_result = allocator.free();
        assert!(free_result.is_ok());
    }

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_arena_allocator_grow() {
        // Binary std/no_std choice
        let mut allocator = QnxArenaAllocatorBuilder::new()
            .with_arena_size(32 * 1024) // 32KB arenas
            .with_guard_pages(false) // No guard pages for simpler testing
            .build()
            .expect("Failed to create arena allocator");

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
    fn test_qnx_arena_allocator_protection() {
        // Binary std/no_std choice
        let mut allocator = QnxArenaAllocatorBuilder::new()
            .with_guard_pages(true)
            .with_data_protection(QnxProtFlags::ReadWrite)
            .build()
            .expect("Failed to create arena allocator");

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

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_arena_allocator_config() {
        // Binary std/no_std choice
        let mut allocator = QnxArenaAllocatorBuilder::new()
            .with_arena_size(16 * 1024) // 16KB arenas
            .with_arena_cache_max_blocks(4)
            .with_arena_cache_max_size(128 * 1024)
            .with_lifo_free(true)
            .with_memory_hold(true)
            .build()
            .expect("Failed to create arena allocator");

        // Binary std/no_std choice
        let result1 = allocator.allocate(1, None);
        assert!(result1.is_ok());
        allocator.free().unwrap();

        let result2 = allocator.allocate(2, None);
        assert!(result2.is_ok());
        allocator.free().unwrap();

        // Get memory info - should show held memory with the memory_hold option
        let info = allocator.memory_info().unwrap();
        assert!(
            info.keepcost > 0,
            "Memory should be held in the arena cache"
        );
    }
}
