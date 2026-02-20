#![allow(unsafe_code)]
// Binary std/no_std choice
// WRT - wrt-platform
// Module: Linux ARM64 Memory Management with MTE
// SW-REQ-ID: REQ_PLATFORM_001, REQ_MEMORY_001, REQ_SAFETY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Linux ARM64-specific `PageAllocator` implementation with Memory Tagging
//! Extension (MTE) support using direct syscalls without libc.
//!
//! This implementation provides enhanced memory safety through hardware memory
//! tagging on ARM64 platforms that support MTE, while falling back to standard
//! protection mechanisms on systems without MTE support.

use core::ptr::{
    self,
    NonNull,
};

// Safety: NonNull<u8> is safe to send between threads as it's just a pointer
// wrapper
unsafe impl Send for LinuxArm64MteAllocator {}
unsafe impl Sync for LinuxArm64MteAllocator {}

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

/// ARM64 syscall numbers
mod syscalls {
    pub const MMAP: usize = 222;
    pub const MUNMAP: usize = 215;
    pub const MPROTECT: usize = 226;
    pub const PRCTL: usize = 167;
}

/// Protection flags for memory mapping
const PROT_READ: usize = 0x1;
const PROT_WRITE: usize = 0x2;
const PROT_NONE: usize = 0x0;
const PROT_MTE: usize = 0x20; // ARM64 Memory Tagging Extension

/// Mapping flags
const MAP_PRIVATE: usize = 0x02;
const MAP_ANONYMOUS: usize = 0x20;
const MAP_FIXED: usize = 0x10;

/// prctl constants for MTE
const PR_SET_TAGGED_ADDR_CTRL: usize = 55;
const PR_GET_TAGGED_ADDR_CTRL: usize = 56;
const PR_TAGGED_ADDR_ENABLE: usize = 1 << 0;
const PR_MTE_TCF_SHIFT: usize = 1;
const PR_MTE_TCF_NONE: usize = 0 << PR_MTE_TCF_SHIFT;
const PR_MTE_TCF_SYNC: usize = 1 << PR_MTE_TCF_SHIFT;
const PR_MTE_TCF_ASYNC: usize = 2 << PR_MTE_TCF_SHIFT;
const PR_MTE_TAG_SHIFT: usize = 3;
const PR_MTE_TAG_MASK: usize = 0xffff << PR_MTE_TAG_SHIFT;

/// Error value returned by mmap on failure
const MAP_FAILED: *mut u8 = !0 as *mut u8;

/// MTE configuration options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MteMode {
    /// MTE disabled
    Disabled,
    /// Synchronous MTE (immediate fault on tag mismatch)
    Synchronous,
    /// Asynchronous MTE (delayed fault reporting)
    Asynchronous,
}

/// A `PageAllocator` implementation for Linux ARM64 with MTE support.
#[derive(Debug)]
pub struct LinuxArm64MteAllocator {
    base_ptr:                Option<NonNull<u8>>,
    total_reserved_bytes:    usize,   // Total bytes reserved
    current_committed_bytes: usize,   // Bytes currently committed
    max_capacity_bytes:      usize,   // Maximum bytes this instance can manage
    use_guard_pages:         bool,    // Whether to use guard pages
    mte_mode:                MteMode, // MTE configuration
    mte_available:           bool,    // Whether MTE is available on this system
    current_tag:             u8,      // Current memory tag (0-15)
}

impl LinuxArm64MteAllocator {
    const DEFAULT_MAX_PAGES: u32 = 65536;

    // Corresponds to 4GiB, a common Wasm limit

    /// Creates a new `LinuxArm64MteAllocator`.
    pub fn new(maximum_pages: Option<u32>, use_guard_pages: bool, mte_mode: MteMode) -> Self {
        let max_pages_val = maximum_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let max_capacity_bytes = max_pages_val as usize * WASM_PAGE_SIZE;

        let mut allocator = Self {
            base_ptr: None,
            total_reserved_bytes: 0,
            current_committed_bytes: 0,
            max_capacity_bytes,
            use_guard_pages,
            mte_mode,
            mte_available: false,
            current_tag: 1, // Start with tag 1 (tag 0 is typically untagged)
        };

        // Check if MTE is available and configure it if requested
        if mte_mode != MteMode::Disabled {
            allocator.mte_available = allocator.configure_mte().is_ok();
        }

        allocator
    }

    fn pages_to_bytes(pages: u32) -> Result<usize> {
        pages
            .checked_mul(WASM_PAGE_SIZE as u32)
            .map(|b| b as usize)
            .ok_or_else(|| Error::memory_error("Page count results in byte overflow"))
    }

    /// Configure MTE for the current process
    fn configure_mte(&self) -> Result<()> {
        let mte_flags = match self.mte_mode {
            MteMode::Disabled => return Ok(()),
            MteMode::Synchronous => PR_TAGGED_ADDR_ENABLE | PR_MTE_TCF_SYNC,
            MteMode::Asynchronous => PR_TAGGED_ADDR_ENABLE | PR_MTE_TCF_ASYNC,
        };

        // Set MTE configuration
        let result = unsafe { Self::prctl(PR_SET_TAGGED_ADDR_CTRL, mte_flags, 0, 0, 0) };

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "MTE tagging not supported on this system",
            ));
        }

        Ok(())
    }

    /// Performs the prctl syscall for MTE configuration
    unsafe fn prctl(option: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32 {
        let result: isize;

        core::arch::asm!(
            "svc #0",
            inout("x8") syscalls::PRCTL => _,
            inout("x0") option => result,
            in("x1") arg2,
            in("x2") arg3,
            in("x3") arg4,
            in("x4") arg5,
        );

        result as i32
    }

    /// Performs the mmap syscall directly without libc
    unsafe fn mmap(
        addr: *mut u8,
        len: usize,
        prot: usize,
        flags: usize,
        fd: i32,
        offset: usize,
    ) -> *mut u8 {
        let result: isize;

        core::arch::asm!(
            "svc #0",
            inout("x8") syscalls::MMAP => _,
            inout("x0") addr => result,
            in("x1") len,
            in("x2") prot,
            in("x3") flags,
            in("x4") fd,
            in("x5") offset,
        );

        // Linux syscalls return negative errno on error
        if result < 0 && result >= -4095 {
            MAP_FAILED
        } else {
            result as *mut u8
        }
    }

    /// Performs the munmap syscall directly without libc
    unsafe fn munmap(addr: *mut u8, len: usize) -> i32 {
        let result: isize;

        core::arch::asm!(
            "svc #0",
            inout("x8") syscalls::MUNMAP => _,
            inout("x0") addr => result,
            in("x1") len,
        );

        result as i32
    }

    /// Performs the mprotect syscall directly without libc
    unsafe fn mprotect(addr: *mut u8, len: usize, prot: usize) -> i32 {
        let result: isize;

        core::arch::asm!(
            "svc #0",
            inout("x8") syscalls::MPROTECT => _,
            inout("x0") addr => result,
            in("x1") len,
            in("x2") prot,
        );

        result as i32
    }

    /// Create tagged pointer for MTE
    unsafe fn create_tagged_pointer(&mut self, ptr: *mut u8) -> *mut u8 {
        if !self.mte_available {
            return ptr;
        }

        // ARM64 MTE uses the top 4 bits of the pointer for tagging
        let tagged_ptr = ((self.current_tag as usize) << 56) | (ptr as usize);

        // Binary std/no_std choice
        self.current_tag = (self.current_tag + 1) & 0xF;
        if self.current_tag == 0 {
            self.current_tag = 1; // Skip tag 0
        }

        tagged_ptr as *mut u8
    }

    /// Set memory tags using MTE instructions
    unsafe fn set_memory_tags(&self, ptr: *mut u8, size: usize, tag: u8) -> Result<()> {
        if !self.mte_available {
            return Ok();
        }

        // Set memory tags in 16-byte chunks (MTE granule size)
        let tag_granule_size = 16;
        let num_granules = (size + tag_granule_size - 1) / tag_granule_size;

        for i in 0..num_granules {
            let granule_ptr = ptr.add(i * tag_granule_size);

            // Binary std/no_std choice
            core::arch::asm!(
                "st2g {ptr}, [{ptr}]",
                ptr = in(reg) granule_ptr,
                options(nostack),
            );
        }

        Ok(())
    }

    /// Binary std/no_std choice
    unsafe fn setup_guard_pages(&self, base_ptr: *mut u8, total_size: usize) -> Result<()> {
        if !self.use_guard_pages {
            return Ok();
        }

        // Binary std/no_std choice
        let guard_page_addr = base_ptr.add(total_size - WASM_PAGE_SIZE);
        let result = Self::mprotect(guard_page_addr, WASM_PAGE_SIZE, PROT_NONE);

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to apply MTE tag to memory region",
            ));
        }

        Ok(())
    }
}

/// Builder for `LinuxArm64MteAllocator` to provide a fluent configuration API.
#[derive(Debug)]
pub struct LinuxArm64MteAllocatorBuilder {
    maximum_pages: Option<u32>,
    guard_pages:   bool,
    mte_mode:      MteMode,
}

impl Default for LinuxArm64MteAllocatorBuilder {
    fn default() -> Self {
        Self {
            maximum_pages: None,
            guard_pages:   false,
            mte_mode:      MteMode::Disabled,
        }
    }
}

impl LinuxArm64MteAllocatorBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of WebAssembly pages (64 KiB) that can be
    /// Binary std/no_std choice
    pub fn with_maximum_pages(mut self, pages: u32) -> Self {
        self.maximum_pages = Some(pages);
        self
    }

    /// Enables guard pages for detecting out-of-bounds memory access.
    pub fn with_guard_pages(mut self, enable: bool) -> Self {
        self.guard_pages = enable;
        self
    }

    /// Configures Memory Tagging Extension (MTE) mode.
    pub fn with_mte_mode(mut self, mode: MteMode) -> Self {
        self.mte_mode = mode;
        self
    }

    /// Builds and returns a configured `LinuxArm64MteAllocator`.
    pub fn build(self) -> LinuxArm64MteAllocator {
        LinuxArm64MteAllocator::new(self.maximum_pages, self.guard_pages, self.mte_mode)
    }
}

impl PageAllocator for LinuxArm64MteAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)> {
        if self.base_ptr.is_some() {
            return Err(Error::new(
                ErrorCategory::System,
                1,
                "Memory allocator already initialized with allocated memory",
            ));
        }

        if initial_pages == 0 {
            return Err(Error::memory_error("Initial pages cannot be zero"));
        }

        let initial_bytes = Self::pages_to_bytes(initial_pages)?;
        let max_pages_hint = maximum_pages.unwrap_or(initial_pages).max(initial_pages);
        let mut reserve_bytes = Self::pages_to_bytes(max_pages_hint)?.max(initial_bytes);

        // Add space for guard pages if enabled
        if self.use_guard_pages {
            reserve_bytes = reserve_bytes
                .checked_add(WASM_PAGE_SIZE)
                .ok_or_else(|| Error::memory_error("Guard page size overflow"))?;
        }

        if reserve_bytes > self.max_capacity_bytes {
            return Err(Error::memory_error(
                "Requested reservation size exceeds allocator's maximum capacity",
            ));
        }

        // Determine protection flags
        let mut prot_flags = PROT_READ | PROT_WRITE;
        if self.mte_available {
            prot_flags |= PROT_MTE;
        }

        // Direct syscall to mmap
        // SAFETY: We're calling the mmap syscall directly. Arguments are constructed
        // to be valid for anonymous private mapping with read/write access and MTE if
        // available.
        let ptr = unsafe {
            Self::mmap(
                ptr::null_mut(),
                reserve_bytes,
                prot_flags,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        // Check for mapping failure
        if ptr == MAP_FAILED {
            return Err(Error::runtime_execution_error(
                "Failed to validate MTE tagged memory",
            ));
        }

        // Create tagged pointer if MTE is available
        let tagged_ptr = unsafe { self.create_tagged_pointer(ptr) };

        // Convert raw pointer to NonNull (use original untagged pointer for storage)
        let base_ptr = NonNull::new(ptr)
            .ok_or_else(|| Error::new(ErrorCategory::System, 1, "mmap returned null pointer"))?;

        // Set memory tags if MTE is available
        unsafe {
            if self.mte_available {
                let tag = ((tagged_ptr as usize) >> 56) as u8;
                self.set_memory_tags(ptr, initial_bytes, tag)?;
            }
        }

        // Set up guard pages if enabled
        unsafe {
            self.setup_guard_pages(ptr, reserve_bytes)?;
        }

        self.base_ptr = Some(base_ptr);
        self.total_reserved_bytes = reserve_bytes;
        self.current_committed_bytes = initial_bytes;

        // Return tagged pointer for use, but store untagged pointer internally
        let result_ptr = NonNull::new(tagged_ptr).unwrap_or(base_ptr);
        Ok((result_ptr, initial_bytes))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
        let Some(base_ptr) = self.base_ptr else {
            return Err(Error::runtime_execution_error(
                "Grow called before allocate",
            ));
        };

        if additional_pages == 0 {
            return Ok();
        }

        let current_bytes_from_arg = Self::pages_to_bytes(current_pages)?;
        if current_bytes_from_arg != self.current_committed_bytes {
            return Err(Error::memory_error("Current page count mismatch"));
        }

        let new_total_pages = current_pages
            .checked_add(additional_pages)
            .ok_or_else(|| Error::memory_error("Page count overflow during grow"))?;

        let new_committed_bytes = Self::pages_to_bytes(new_total_pages)?;

        // Account for guard pages in space calculation
        let available_space = if self.use_guard_pages {
            self.total_reserved_bytes.saturating_sub(WASM_PAGE_SIZE)
        } else {
            self.total_reserved_bytes
        };

        if new_committed_bytes > available_space {
            return Err(Error::memory_error(
                "Grow request exceeds total reserved memory space",
            ));
        }

        // Set memory tags for the newly accessible region if MTE is available
        unsafe {
            if self.mte_available {
                let grow_start = base_ptr.as_ptr().add(self.current_committed_bytes);
                let grow_size = new_committed_bytes - self.current_committed_bytes;
                self.set_memory_tags(grow_start, grow_size, self.current_tag)?;
            }
        }

        // Since we've already mapped all the memory, we just need to update our
        // accounting
        self.current_committed_bytes = new_committed_bytes;
        Ok(())
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()> {
        // Strip any MTE tags from the provided pointer
        let untagged_ptr = NonNull::new((ptr.as_ptr() as usize & 0x00FF_FFFF_FFFF_FFFF) as *mut u8)
            .ok_or_else(|| Error::memory_error("Invalid pointer after tag stripping"))?;

        // Validate that untagged ptr matches our base_ptr
        let Some(base_ptr) = self.base_ptr.take() else {
            return Err(Error::memory_error("No memory allocated to deallocate"));
        };

        if untagged_ptr.as_ptr() != base_ptr.as_ptr() {
            self.base_ptr = Some(base_ptr); // Restore base_ptr
            return Err(Error::memory_error(
                "Attempted to deallocate with mismatched pointer",
            ));
        }

        // SAFETY: ptr was obtained from our mmap call and is valid.
        // size is the total size we had reserved.
        let result = Self::munmap(base_ptr.as_ptr(), size);
        if result != 0 {
            // munmap failed, need to restore base_ptr
            self.base_ptr = Some(base_ptr);
            return Err(Error::runtime_execution_error(
                "Memory unmapping failed due to OS error",
            ));
        }

        // Reset internal state
        self.total_reserved_bytes = 0;
        self.current_committed_bytes = 0;
        Ok(())
    }
}
