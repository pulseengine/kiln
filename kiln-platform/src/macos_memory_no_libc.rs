#![allow(unsafe_code)]
// Binary std/no_std choice
// WRT - wrt-platform
// Module: macOS Memory Management (No libc)
// SW-REQ-ID: REQ_PLATFORM_001, REQ_MEMORY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! macOS-specific `PageAllocator` implementation using direct syscalls without
//! libc.

use core::ptr::{
    self,
    NonNull,
};

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};

use crate::memory::{
    PageAllocator,
    WASM_PAGE_SIZE,
};

/// Constants for macOS mmap syscall
const SYSCALL_MMAP: usize = 197;
const SYSCALL_MUNMAP: usize = 73;

/// Protection flags for memory mapping
const PROT_READ: usize = 0x01;
const PROT_WRITE: usize = 0x02;

/// Mapping flags
const MAP_PRIVATE: usize = 0x0002;
const MAP_ANON: usize = 0x1000;

/// A `PageAllocator` implementation for macOS using direct syscalls.
#[derive(Debug)]
pub struct MacOsAllocator {
    base_ptr:                Option<NonNull<u8>>,
    total_reserved_bytes:    usize, // Total bytes reserved
    current_committed_bytes: usize, // Bytes currently committed
    max_capacity_bytes:      usize, // Maximum bytes this instance can manage
}

// Safety: MacOsAllocator can be shared between threads safely because:
// 1. All operations use atomic syscalls
// 2. Memory is properly synchronized through the OS
// 3. NonNull is used safely for owned memory regions
unsafe impl Send for MacOsAllocator {}
unsafe impl Sync for MacOsAllocator {}

impl MacOsAllocator {
    const DEFAULT_MAX_PAGES: u32 = 65536;

    // Corresponds to 4GiB, a common Wasm limit

    /// Creates a new `MacOsAllocator`.
    pub fn new(maximum_pages: Option<u32>) -> Self {
        let max_pages_val = maximum_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let max_capacity_bytes = max_pages_val as usize * WASM_PAGE_SIZE;
        Self {
            base_ptr: None,
            total_reserved_bytes: 0,
            current_committed_bytes: 0,
            max_capacity_bytes,
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
        let ret: *mut u8;

        unsafe {
            #[cfg(target_arch = "x86_64")]
            core::arch::asm!(
                "syscall",
                inout("rax") SYSCALL_MMAP => _,
                in("rdi") addr,
                in("rsi") len,
                in("rdx") prot,
                in("r10") flags,
                in("r8") fd,
                in("r9") offset,
                lateout("rax") ret,
                out("rcx") _,
                out("r11") _,
            );
            #[cfg(target_arch = "aarch64")]
            core::arch::asm!(
                "svc #0x80",
                inout("x8") SYSCALL_MMAP => _,
                in("x0") addr,
                in("x1") len,
                in("x2") prot,
                in("x3") flags,
                in("x4") fd,
                in("x5") offset,
                lateout("x0") ret,
            );
        }

        ret
    }

    /// Performs the munmap syscall directly without libc
    unsafe fn munmap(addr: *mut u8, len: usize) -> i32 {
        let ret: i32;

        unsafe {
            #[cfg(target_arch = "x86_64")]
            core::arch::asm!(
                "syscall",
                inout("rax") SYSCALL_MUNMAP => _,
                in("rdi") addr,
                in("rsi") len,
                lateout("rax") ret,
                out("rcx") _,
                out("r11") _,
            );
            #[cfg(target_arch = "aarch64")]
            core::arch::asm!(
                "svc #0x80",
                inout("x8") SYSCALL_MUNMAP => _,
                in("x0") addr,
                in("x1") len,
                lateout("x0") ret,
            );
        }

        ret
    }
}

/// Builder for `MacOsAllocator` to provide a fluent configuration API.
#[derive(Debug)]
pub struct MacOsAllocatorBuilder {
    maximum_pages:  Option<u32>,
    guard_pages:    bool,
    memory_tagging: bool,
}

impl Default for MacOsAllocatorBuilder {
    fn default() -> Self {
        Self {
            maximum_pages:  None,
            guard_pages:    false,
            memory_tagging: false,
        }
    }
}

impl MacOsAllocatorBuilder {
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
    ///
    /// This is a no-op in the current implementation, but reserved for future
    /// use.
    pub fn with_guard_pages(mut self, enable: bool) -> Self {
        self.guard_pages = enable;
        self
    }

    /// Enables memory tagging for enhanced memory safety (e.g., MTE on Arm).
    ///
    /// This is a no-op in the current implementation, but reserved for future
    /// use.
    pub fn with_memory_tagging(mut self, enable: bool) -> Self {
        self.memory_tagging = enable;
        self
    }

    /// Builds and returns a configured `MacOsAllocator`.
    pub fn build(self) -> MacOsAllocator {
        // Currently, we only support configuring maximum pages
        // The other options are reserved for future implementation
        MacOsAllocator::new(self.maximum_pages)
    }
}

impl PageAllocator for MacOsAllocator {
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)> {
        if self.base_ptr.is_some() {
            return Err(Error::runtime_execution_error(
                "Memory allocation failed: out of memory",
            ));
        }

        if initial_pages == 0 {
            return Err(Error::memory_error("Cannot allocate zero pages"));
        }

        let initial_bytes = Self::pages_to_bytes(initial_pages)?;
        let max_pages_hint = maximum_pages.unwrap_or(initial_pages).max(initial_pages);
        let reserve_bytes = Self::pages_to_bytes(max_pages_hint)?.max(initial_bytes);

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
                MAP_PRIVATE | MAP_ANON,
                -1,
                0,
            )
        };

        // Check for mapping failure (mmap returns MAP_FAILED which is -1 as pointer)
        if ptr as isize == -1 {
            return Err(Error::runtime_execution_error("mmap syscall failed"));
        }

        // Convert raw pointer to NonNull
        let base_ptr = NonNull::new(ptr)
            .ok_or_else(|| Error::new(ErrorCategory::System, 1, "mmap returned null pointer"))?;

        self.base_ptr = Some(base_ptr);
        self.total_reserved_bytes = reserve_bytes;
        self.current_committed_bytes = initial_bytes;

        Ok((base_ptr, initial_bytes))
    }

    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
        let Some(_base_ptr) = self.base_ptr else {
            return Err(Error::runtime_execution_error(
                "Grow called before allocate",
            ));
        };

        if additional_pages == 0 {
            return Ok(());
        }

        let current_bytes_from_arg = Self::pages_to_bytes(current_pages)?;
        if current_bytes_from_arg != self.current_committed_bytes {
            return Err(Error::memory_error("Current page count mismatch"));
        }

        let new_total_pages = current_pages
            .checked_add(additional_pages)
            .ok_or_else(|| Error::memory_error("Page count overflow during grow"))?;

        let new_committed_bytes = Self::pages_to_bytes(new_total_pages)?;

        if new_committed_bytes > self.total_reserved_bytes {
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
        let result = unsafe { Self::munmap(ptr.as_ptr(), size) };
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
