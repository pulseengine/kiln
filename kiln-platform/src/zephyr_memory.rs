#![allow(unsafe_code)]
#![allow(dead_code)]
// Allow unsafe FFI calls to Zephyr kernel
// WRT - wrt-platform
// Module: Zephyr Memory Management
// SW-REQ-ID: REQ_PLATFORM_001, REQ_MEMORY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Zephyr RTOS-specific `PageAllocator` implementation using Zephyr kernel
//! APIs.
//!
//! This implementation provides memory allocation for WebAssembly pages using
//! Zephyr's heap management and memory domains, supporting no_std/no_alloc
//! environments on embedded systems.

use core::ptr::NonNull;

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::memory::{
    PageAllocator,
    WASM_PAGE_SIZE,
};

/// Zephyr kernel timeout values
const K_NO_WAIT: i32 = 0;
#[allow(dead_code)]
const K_FOREVER: i32 = -1;

/// Zephyr error codes
#[allow(dead_code)]
const EAGAIN: i32 = -11;
#[allow(dead_code)]
const ENOMEM: i32 = -12;
#[allow(dead_code)]
const EINVAL: i32 = -22;

/// Memory protection flags for Zephyr memory domains
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZephyrMemoryFlags {
    /// Read access
    Read      = 0x01,
    /// Write access  
    Write     = 0x02,
    /// Execute access
    Execute   = 0x04,
    /// Cacheable memory
    Cacheable = 0x08,
    /// Device memory (uncacheable)
    Device    = 0x10,
}

/// Zephyr heap handle (opaque pointer to k_heap structure)
#[repr(C)]
struct ZephyrHeap {
    _private: [u8; 0],
}

/// Zephyr memory domain handle
#[repr(C)]
struct ZephyrMemDomain {
    _private: [u8; 0],
}

/// Zephyr memory partition
#[repr(C)]
#[derive(Debug)]
struct ZephyrMemPartition {
    start: usize,
    size:  usize,
    attr:  u32,
}

// FFI declarations for Zephyr kernel APIs
unsafe extern "C" {
    /// Allocate aligned memory from a heap
    fn k_heap_aligned_alloc(
        heap: *mut ZephyrHeap,
        align: usize,
        size: usize,
        timeout: i32,
    ) -> *mut u8;

    /// Binary std/no_std choice
    fn k_heap_free(heap: *mut ZephyrHeap, mem: *mut u8);

    /// Initialize a memory domain
    fn k_mem_domain_init(
        domain: *mut ZephyrMemDomain,
        num_parts: u8,
        parts: *mut ZephyrMemPartition,
    ) -> i32;

    /// Add a partition to a memory domain
    fn k_mem_domain_add_partition(
        domain: *mut ZephyrMemDomain,
        part: *mut ZephyrMemPartition,
    ) -> i32;

    /// Remove a partition from a memory domain
    fn k_mem_domain_remove_partition(
        domain: *mut ZephyrMemDomain,
        part: *mut ZephyrMemPartition,
    ) -> i32;

    /// Get the system heap
    fn k_heap_sys_get() -> *mut ZephyrHeap;

    /// Get memory usage statistics
    fn k_heap_size_get(heap: *mut ZephyrHeap) -> usize;
}

/// Binary std/no_std choice
#[derive(Debug, Clone)]
pub struct ZephyrAllocatorConfig {
    /// Whether to use memory domains for isolation
    pub use_memory_domains: bool,
    /// Memory protection attributes
    pub memory_attributes:  ZephyrMemoryFlags,
    /// Whether to use guard regions
    pub use_guard_regions:  bool,
    /// Custom heap to use (None = system heap)
    pub custom_heap:        bool,
}

impl Default for ZephyrAllocatorConfig {
    fn default() -> Self {
        Self {
            use_memory_domains: true,
            memory_attributes:  ZephyrMemoryFlags::Read,
            use_guard_regions:  true,
            custom_heap:        false,
        }
    }
}

/// A `PageAllocator` implementation for Zephyr RTOS.
#[derive(Debug)]
pub struct ZephyrAllocator {
    config:                  ZephyrAllocatorConfig,
    heap:                    *mut ZephyrHeap,
    memory_domain:           Option<NonNull<ZephyrMemDomain>>,
    base_ptr:                Option<NonNull<u8>>,
    total_reserved_bytes:    usize,
    current_committed_bytes: usize,
    max_capacity_bytes:      usize,
    current_partition:       Option<ZephyrMemPartition>,
}

// Safety: ZephyrAllocator only contains pointers to Zephyr kernel objects which
// are thread-safe
unsafe impl Send for ZephyrAllocator {}
unsafe impl Sync for ZephyrAllocator {}

impl ZephyrAllocator {
    const DEFAULT_MAX_PAGES: u32 = 65536;

    // 4GiB limit

    /// Creates a new `ZephyrAllocator` with the given configuration.
    pub fn new(config: ZephyrAllocatorConfig, maximum_pages: Option<u32>) -> Self {
        let max_pages_val = maximum_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let max_capacity_bytes = max_pages_val as usize * WASM_PAGE_SIZE;

        // Get the appropriate heap
        let heap = if config.custom_heap {
            // In a real implementation, this would use a custom heap
            // For now, fall back to system heap
            unsafe { k_heap_sys_get() }
        } else {
            unsafe { k_heap_sys_get() }
        };

        Self {
            config,
            heap,
            memory_domain: None,
            base_ptr: None,
            total_reserved_bytes: 0,
            current_committed_bytes: 0,
            max_capacity_bytes,
            current_partition: None,
        }
    }

    fn pages_to_bytes(pages: u32) -> Result<usize> {
        pages
            .checked_mul(WASM_PAGE_SIZE as u32)
            .map(|b| b as usize)
            .ok_or_else(|| Error::memory_error("Page count results in byte overflow"))
    }

    /// Set up memory domain isolation if enabled
    unsafe fn setup_memory_domain(&mut self, ptr: *mut u8, size: usize) -> Result<()> {
        if !self.config.use_memory_domains {
            return Ok(());
        }

        // Binary std/no_std choice
        // Binary std/no_std choice
        // we'll use a placeholder approach that would work in the actual embedded
        // context.

        // Note: In real usage, domain would be a static or stack variable:
        // static K_MEM_DOMAIN_DEFINE(my_domain);
        // For now, we'll use a null pointer to indicate this limitation
        let domain: *mut ZephyrMemDomain = core::ptr::null_mut();

        if domain.is_null() {
            // Memory domains not available - log this for debugging in real
            // implementation For now, continue without memory
            // domain isolation
        } else {
            // Create memory partition
            let partition = ZephyrMemPartition {
                start: ptr as usize,
                size,
                attr: self.config.memory_attributes as u32,
            };

            // Initialize memory domain
            let result = unsafe { k_mem_domain_init(domain, 1, &partition as *const _ as *mut _) };
            if result != 0 {
                return Err(Error::runtime_execution_error(
                    "Zephyr memory pool allocation failed",
                ));
            }

            self.memory_domain = NonNull::new(domain);
            self.current_partition = Some(partition);
        }

        Ok(())
    }

    /// Clean up memory domain
    unsafe fn cleanup_memory_domain(&mut self) -> Result<()> {
        if let Some(domain) = self.memory_domain.take() {
            if let Some(partition) = self.current_partition.take() {
                // Remove partition from domain
                unsafe { k_mem_domain_remove_partition(domain.as_ptr(), &partition as *const _ as *mut _) };
            }

            // In real Zephyr implementation, static domains don't need explicit
            // cleanup The kernel handles this automatically
        }
        Ok(())
    }

    /// Binary std/no_std choice
    unsafe fn setup_guard_regions(&self, _base_ptr: *mut u8, _total_size: usize) -> Result<()> {
        if !self.config.use_guard_regions {
            return Ok(());
        }

        // In a real implementation, this would set up MPU/MMU regions
        // Binary std/no_std choice
        // For now, this is a placeholder
        Ok(())
    }
}

/// Builder for `ZephyrAllocator` to provide a fluent configuration API.
#[derive(Debug)]
pub struct ZephyrAllocatorBuilder {
    config:        ZephyrAllocatorConfig,
    maximum_pages: Option<u32>,
}

impl Default for ZephyrAllocatorBuilder {
    fn default() -> Self {
        Self {
            config:        ZephyrAllocatorConfig::default(),
            maximum_pages: None,
        }
    }
}

impl ZephyrAllocatorBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binary std/no_std choice
    pub fn with_maximum_pages(mut self, pages: u32) -> Self {
        self.maximum_pages = Some(pages);
        self
    }

    /// Enables or disables memory domains for isolation.
    pub fn with_memory_domains(mut self, enable: bool) -> Self {
        self.config.use_memory_domains = enable;
        self
    }

    /// Sets memory protection attributes.
    pub fn with_memory_attributes(mut self, attrs: ZephyrMemoryFlags) -> Self {
        self.config.memory_attributes = attrs;
        self
    }

    /// Enables or disables guard regions.
    pub fn with_guard_regions(mut self, enable: bool) -> Self {
        self.config.use_guard_regions = enable;
        self
    }

    /// Uses a custom heap instead of the system heap.
    pub fn with_custom_heap(mut self, enable: bool) -> Self {
        self.config.custom_heap = enable;
        self
    }

    /// Builds and returns a configured `ZephyrAllocator`.
    pub fn build(self) -> ZephyrAllocator {
        ZephyrAllocator::new(self.config, self.maximum_pages)
    }
}

impl PageAllocator for ZephyrAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)> {
        if self.base_ptr.is_some() {
            return Err(Error::new(
                ErrorCategory::System,
                1,
                "Memory domain setup failed",
            ));
        }

        if initial_pages == 0 {
            return Err(Error::memory_error("Initial pages cannot be zero"));
        }

        let initial_bytes = Self::pages_to_bytes(initial_pages)?;
        let max_pages_hint = maximum_pages.unwrap_or(initial_pages).max(initial_pages);
        let mut reserve_bytes = Self::pages_to_bytes(max_pages_hint)?.max(initial_bytes);

        // Add space for guard regions if enabled
        if self.config.use_guard_regions {
            reserve_bytes = reserve_bytes
                .checked_add(2 * WASM_PAGE_SIZE)
                .ok_or_else(|| Error::memory_error("Guard region size overflow"))?;
        }

        if reserve_bytes > self.max_capacity_bytes {
            return Err(Error::memory_error(
                "Requested reservation size exceeds allocator's maximum capacity",
            ));
        }

        // Allocate aligned memory from Zephyr heap
        // Binary std/no_std choice
        let ptr = unsafe {
            k_heap_aligned_alloc(
                self.heap,
                WASM_PAGE_SIZE, // alignment
                reserve_bytes,  // size
                K_NO_WAIT,      // don't block
            )
        };

        if ptr.is_null() {
            return Err(Error::runtime_execution_error(
                "Zephyr memory alignment requirement not met",
            ));
        }

        // Convert raw pointer to NonNull
        let base_ptr = NonNull::new(ptr).ok_or_else(|| {
            Error::new(ErrorCategory::System, 1, "Failed to create NonNull pointer")
        })?;

        // Set up memory domain isolation if enabled
        unsafe {
            if let Err(e) = self.setup_memory_domain(ptr, reserve_bytes) {
                // Binary std/no_std choice
                k_heap_free(self.heap, ptr);
                return Err(e);
            }
        }

        // Set up guard regions if enabled
        unsafe {
            if let Err(e) = self.setup_guard_regions(ptr, reserve_bytes) {
                // Binary std/no_std choice
                let _ = self.cleanup_memory_domain();
                k_heap_free(self.heap, ptr);
                return Err(e);
            }
        }

        self.base_ptr = Some(base_ptr);
        self.total_reserved_bytes = reserve_bytes;
        self.current_committed_bytes = initial_bytes;

        Ok((base_ptr, initial_bytes))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
        let Some(_base_ptr) = self.base_ptr else {
            return Err(Error::runtime_execution_error(
                "Zephyr memory pool exhausted",
            ));
        };

        if additional_pages == 0 {
            return Ok(());
        }

        let current_bytes_from_arg = Self::pages_to_bytes(current_pages)?;
        if current_bytes_from_arg != self.current_committed_bytes {
            return Err(Error::memory_error(
                "Current page count does not match internal state",
            ));
        }

        let new_total_pages = current_pages
            .checked_add(additional_pages)
            .ok_or_else(|| Error::memory_error("Page count overflow during grow"))?;

        let new_committed_bytes = Self::pages_to_bytes(new_total_pages)?;

        // Account for guard regions in space calculation
        let available_space = if self.config.use_guard_regions {
            self.total_reserved_bytes.saturating_sub(2 * WASM_PAGE_SIZE)
        } else {
            self.total_reserved_bytes
        };

        if new_committed_bytes > available_space {
            return Err(Error::memory_error(
                "Grow request exceeds total reserved memory space",
            ));
        }

        // Since we already reserved the memory, just update our accounting
        self.current_committed_bytes = new_committed_bytes;
        Ok(())
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, _size: usize) -> Result<()> {
        // Validate that ptr matches our base_ptr
        let Some(base_ptr) = self.base_ptr.take() else {
            return Err(Error::memory_error("No memory allocated to deallocate"));
        };

        if ptr.as_ptr() != base_ptr.as_ptr() {
            self.base_ptr = Some(base_ptr); // Restore base_ptr
            return Err(Error::memory_error(
                "Attempted to deallocate with mismatched pointer",
            ));
        }

        // Clean up memory domain first
        if let Err(e) = unsafe { self.cleanup_memory_domain() } {
            // Binary std/no_std choice
            self.base_ptr = Some(base_ptr);
            return Err(e);
        }

        // Free the memory using Zephyr's heap API
        // Binary std/no_std choice
        unsafe { k_heap_free(self.heap, ptr.as_ptr()) };

        // Reset internal state
        self.total_reserved_bytes = 0;
        self.current_committed_bytes = 0;
        Ok(())
    }
}

impl Drop for ZephyrAllocator {
    fn drop(&mut self) {
        // Binary std/no_std choice
        if let Some(base_ptr) = self.base_ptr.take() {
            unsafe {
                let _ = self.cleanup_memory_domain();
                k_heap_free(self.heap, base_ptr.as_ptr());
            }
        }
    }
}
