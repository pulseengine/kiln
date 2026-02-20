#![allow(unsafe_code)]
// Binary std/no_std choice
// WRT - wrt-platform
// Module: Linux Memory Management
// SW-REQ-ID: REQ_PLATFORM_001, REQ_MEMORY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Linux-specific `PageAllocator` implementation using direct syscalls without
//! libc.
//!
//! This implementation provides memory allocation for WebAssembly pages using
//! Linux mmap/munmap/mprotect syscalls directly, supporting no_std/no_alloc
//! environments.

use core::ptr::{
    self,
    NonNull,
};

// Safety: NonNull<u8> is safe to send between threads as it's just a pointer
// wrapper
unsafe impl Send for LinuxAllocator {}
unsafe impl Sync for LinuxAllocator {}

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

/// Linux syscall numbers for x86_64
#[cfg(target_arch = "x86_64")]
mod syscalls {
    pub const MMAP: usize = 9;
    pub const MUNMAP: usize = 11;
    pub const MPROTECT: usize = 10;
}

/// Linux syscall numbers for aarch64 (ARM64)
#[cfg(target_arch = "aarch64")]
mod syscalls {
    pub const MMAP: usize = 222;
    pub const MUNMAP: usize = 215;
    pub const MPROTECT: usize = 226;
}

/// Protection flags for memory mapping
const PROT_READ: usize = 0x1;
const PROT_WRITE: usize = 0x2;
const PROT_NONE: usize = 0x0;

/// Mapping flags
const MAP_PRIVATE: usize = 0x02;
const MAP_ANONYMOUS: usize = 0x20;
const MAP_FIXED: usize = 0x10;

/// Error value returned by mmap on failure
const MAP_FAILED: *mut u8 = !0 as *mut u8;

/// A `PageAllocator` implementation for Linux using direct syscalls.
#[derive(Debug)]
pub struct LinuxAllocator {
    base_ptr:                Option<NonNull<u8>>,
    total_reserved_bytes:    usize, // Total bytes reserved
    current_committed_bytes: usize, // Bytes currently committed
    max_capacity_bytes:      usize, // Maximum bytes this instance can manage
    use_guard_pages:         bool,  // Whether to use guard pages
}

impl LinuxAllocator {
    const DEFAULT_MAX_PAGES: u32 = 65536;

    // Corresponds to 4GiB, a common Wasm limit

    /// Creates a new `LinuxAllocator`.
    pub fn new(maximum_pages: Option<u32>, use_guard_pages: bool) -> Self {
        let max_pages_val = maximum_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let max_capacity_bytes = max_pages_val as usize * WASM_PAGE_SIZE;
        Self {
            base_ptr: None,
            total_reserved_bytes: 0,
            current_committed_bytes: 0,
            max_capacity_bytes,
            use_guard_pages,
        }
    }

    fn pages_to_bytes(pages: u32) -> Result<usize> {
        pages
            .checked_mul(WASM_PAGE_SIZE as u32)
            .map(|b| b as usize)
            .ok_or_else(|| Error::memory_error("Page count results in byte overflow"))
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

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "syscall",
            inout("rax") syscalls::MMAP => result,
            in("rdi") addr,
            in("rsi") len,
            in("rdx") prot,
            in("r10") flags,
            in("r8") fd,
            in("r9") offset,
            out("rcx") _,
            out("r11") _,
        );

        #[cfg(target_arch = "aarch64")]
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

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "syscall",
            inout("rax") syscalls::MUNMAP => result,
            in("rdi") addr,
            in("rsi") len,
            out("rcx") _,
            out("r11") _,
        );

        #[cfg(target_arch = "aarch64")]
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

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "syscall",
            inout("rax") syscalls::MPROTECT => result,
            in("rdi") addr,
            in("rsi") len,
            in("rdx") prot,
            out("rcx") _,
            out("r11") _,
        );

        #[cfg(target_arch = "aarch64")]
        core::arch::asm!(
            "svc #0",
            inout("x8") syscalls::MPROTECT => _,
            inout("x0") addr => result,
            in("x1") len,
            in("x2") prot,
        );

        result as i32
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
                "Failed to set up guard pages using mprotect",
            ));
        }

        Ok(())
    }
}

/// Builder for `LinuxAllocator` to provide a fluent configuration API.
#[derive(Debug)]
pub struct LinuxAllocatorBuilder {
    maximum_pages: Option<u32>,
    guard_pages:   bool,
}

impl Default for LinuxAllocatorBuilder {
    fn default() -> Self {
        Self {
            maximum_pages: None,
            guard_pages:   false,
        }
    }
}

impl LinuxAllocatorBuilder {
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

    /// Builds and returns a configured `LinuxAllocator`.
    pub fn build(self) -> LinuxAllocator {
        LinuxAllocator::new(self.maximum_pages, self.guard_pages)
    }
}

impl PageAllocator for LinuxAllocator {
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

        // Direct syscall to mmap
        // SAFETY: We're calling the mmap syscall directly. Arguments are constructed
        // to be valid for anonymous private mapping with read/write access.
        let ptr = unsafe {
            Self::mmap(
                ptr::null_mut(),
                reserve_bytes,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        // Check for mapping failure
        if ptr == MAP_FAILED {
            return Err(Error::runtime_execution_error(
                "Failed to map memory using mmap syscall",
            ));
        }

        // Convert raw pointer to NonNull
        let base_ptr = NonNull::new(ptr)
            .ok_or_else(|| Error::new(ErrorCategory::System, 1, "mmap returned null pointer"))?;

        // Set up guard pages if enabled
        unsafe {
            self.setup_guard_pages(ptr, reserve_bytes)?;
        }

        self.base_ptr = Some(base_ptr);
        self.total_reserved_bytes = reserve_bytes;
        self.current_committed_bytes = initial_bytes;

        Ok((base_ptr, initial_bytes))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
        let Some(base_ptr) = self.base_ptr else {
            return Err(Error::runtime_execution_error(
                "Cannot grow memory: no memory has been allocated",
            ));
        };

        if additional_pages == 0 {
            return Ok();
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

        // Since we've already mapped all the memory with PROT_READ | PROT_WRITE,
        // we just need to update our accounting
        self.current_committed_bytes = new_committed_bytes;
        Ok(())
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()> {
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

        // SAFETY: ptr was obtained from our mmap call and is valid.
        // size is the total size we had reserved.
        let result = Self::munmap(ptr.as_ptr(), size);
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
