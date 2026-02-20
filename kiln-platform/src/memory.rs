// WRT - wrt-platform
// Module: Platform Memory Management Abstraction
// SW-REQ-ID: REQ_PLATFORM_001, REQ_MEMORY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides implementations for platform-specific memory management.

use core::{
    fmt::Debug,
    ptr::NonNull,
};

use wrt_error::Result;

// Definitions are now local to this file.
// REMOVED: use wrt_foundation::memory_traits::{PageAllocator, WASM_PAGE_SIZE};

// Import verification level from wrt-foundation if available, otherwise define
// our own Define our own VerificationLevel since we don't depend on
// wrt-foundation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Verification level for resource and memory safety
pub enum VerificationLevel {
    /// No verification
    Off,
    /// Minimal verification (lightweight checksums only)
    Minimal,
    /// Standard verification (includes bounds checking)
    Standard,
    /// Full verification (includes comprehensive validation)
    Full,
    /// Critical verification (includes redundant checks)
    Critical,
}

impl Default for VerificationLevel {
    fn default() -> Self {
        Self::Standard
    }
}

// START DEFINITIONS MOVED FROM wrt-foundation/src/memory_traits.rs
// (and originally present here before being moved out)

/// Represents a single WebAssembly page (64 `KiB`).
pub const WASM_PAGE_SIZE: usize = 65536; // 64 * 1024

/// Binary std/no_std choice
///
/// Binary std/no_std choice
/// regions suitable for WebAssembly linear memory.
///
/// # Safety
/// Implementations involving `unsafe` (e.g., interacting with OS memory APIs)
/// must ensure they uphold memory safety guarantees, properly handle alignment,
/// and manage memory lifetimes correctly.
// Note: #![forbid(unsafe_code)] applies to the crate, specific unsafe blocks
// would need justification and potentially move to a dedicated `hal` submodule
// if complex.
pub trait PageAllocator: Debug + Send + Sync {
    /// Allocate a region of memory capable of holding `initial_pages`.
    ///
    /// Binary std/no_std choice
    /// Implementations may reserve address space beyond `initial_pages` up to
    /// `maximum_pages` if applicable.
    ///
    /// # Arguments
    ///
    /// * `initial_pages`: The number of Wasm pages (64 `KiB`) to make initially
    ///   accessible.
    /// * `maximum_pages`: An optional hint for the maximum number of pages the
    ///   memory might grow to.
    ///
    /// # Returns
    ///
    /// Binary std/no_std choice
    /// region and the total committed memory size in bytes, or an `Error`
    /// on failure.
    ///
    /// # Errors
    ///
    /// Binary std/no_std choice
    /// exceeds limits, or invalid arguments).
    fn allocate(
        &mut self,
        initial_pages: u32,
        maximum_pages: Option<u32>,
    ) -> Result<(NonNull<u8>, usize)>;

    /// Binary std/no_std choice
    ///
    /// Binary std/no_std choice
    /// `current_pages + additional_pages` in size.
    ///
    /// # Arguments
    ///
    /// * `current_pages`: The current size of the memory region in Wasm pages.
    /// * `additional_pages`: The number of Wasm pages to add.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an `Error` if the memory cannot be grown (e.g.,
    /// Binary std/no_std choice
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the memory cannot be grown (e.g.,
    /// Binary std/no_std choice
    fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()>;

    /// Binary std/no_std choice
    ///
    /// # Safety
    /// The caller must ensure that the `ptr` and `size` correspond exactly to a
    /// Binary std/no_std choice
    /// that no references to the memory region exist after this call. The
    /// caller also guarantees that `ptr` points to memory that was
    /// Binary std/no_std choice
    /// Binary std/no_std choice
    ///
    /// # Errors
    ///
    /// Binary std/no_std choice
    /// violated (though safety violations should ideally panic or be caught
    /// by other means).
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()>;

    // Add methods for memory protection (e.g., MTE, guard pages) later if needed.
    // fn protect(...) -> Result<(), Error>;
}
/// Memory Provider trait for memory operations.
///
/// This trait defines the interface for memory providers that can be used
/// with the platform optimization system. It provides methods for reading
/// and writing data, as well as managing verification levels.
pub trait MemoryProvider: Send + Sync {
    /// Returns the capacity of this provider in bytes.
    fn capacity(&self) -> usize;

    /// Returns the current verification level.
    fn verification_level(&self) -> VerificationLevel;

    /// Sets the verification level.
    fn set_verification_level(&mut self, level: VerificationLevel);

    /// Writes data to the specified offset.
    ///
    /// # Errors
    ///
    /// Returns an error if the write would exceed memory bounds.
    fn write_data(&mut self, offset: usize, data: &[u8]) -> wrt_error::Result<usize>;

    /// Reads data from the specified offset into the provided buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the read would exceed memory bounds.
    fn read_data(&self, offset: usize, buffer: &mut [u8]) -> wrt_error::Result<usize>;
}

/// A simple in-memory provider for platforms that don't need special
/// optimizations.
///
/// This implementation uses a static buffer to store data, making it suitable
/// Binary std/no_std choice
#[derive(Debug)]
pub struct NoStdProvider {
    /// The underlying buffer for storing data
    buffer:             &'static mut [u8],
    /// The current verification level
    verification_level: VerificationLevel,
}

impl NoStdProvider {
    /// Creates a new `NoStdProvider` with the specified size and verification
    /// level.
    pub fn new(size: usize, verification_level: VerificationLevel) -> Self {
        // Binary std/no_std choice
        // For this stub, we just create a dummy static buffer
        static mut DUMMY_BUFFER: [u8; 4096] = [0; 4096];

        let actual_size = core::cmp::min(size, 4096);

        Self {
            buffer: unsafe { &mut DUMMY_BUFFER[0..actual_size] },
            verification_level,
        }
    }

    /// Creates a new `NoStdProvider` with the specified verification level and
    /// default size.
    pub fn with_verification_level(verification_level: VerificationLevel) -> Self {
        Self::new(4096, verification_level)
    }
}

/// Builder for `NoStdProvider` to provide a fluent configuration API.
///
/// # Deprecated
/// Use `WrtProviderFactory::create_provider()` for budget-aware allocation
/// instead.
#[deprecated(
    since = "0.3.0",
    note = "Use WrtProviderFactory::create_provider() from wrt-foundation for new code"
)]
#[derive(Debug)]
pub struct NoStdProviderBuilder {
    size:               usize,
    verification_level: VerificationLevel,
}

#[allow(deprecated)]
impl Default for NoStdProviderBuilder {
    fn default() -> Self {
        Self {
            size:               4096,
            verification_level: VerificationLevel::Standard,
        }
    }
}

#[allow(deprecated)]
impl NoStdProviderBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the size of the internal buffer.
    ///
    /// Note that the actual size is capped at 4096 bytes in the current
    /// implementation.
    pub fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    /// Sets the verification level for memory operations.
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Builds and returns a configured `NoStdProvider`.
    pub fn build(self) -> NoStdProvider {
        // Note: This is a temporary workaround for the deprecated NoStdProvider
        // This entire module should be migrated to use wrt-foundation's memory system
        NoStdProvider {
            buffer:             unsafe {
                static mut DUMMY_BUFFER: [u8; 4096] = [0; 4096];
                let actual_size = core::cmp::min(self.size, 4096);
                &mut DUMMY_BUFFER[0..actual_size]
            },
            verification_level: self.verification_level,
        }
    }
}

impl MemoryProvider for NoStdProvider {
    fn capacity(&self) -> usize {
        self.buffer.len()
    }

    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    fn write_data(&mut self, offset: usize, data: &[u8]) -> wrt_error::Result<usize> {
        if offset >= self.buffer.len() {
            return Err(wrt_error::Error::runtime_execution_error(
                "Write offset out of bounds",
            ));
        }

        let available = self.buffer.len() - offset;
        let write_size = core::cmp::min(available, data.len());

        self.buffer[offset..offset + write_size].copy_from_slice(&data[0..write_size]);

        Ok(write_size)
    }

    fn read_data(&self, offset: usize, buffer: &mut [u8]) -> wrt_error::Result<usize> {
        if offset >= self.buffer.len() {
            return Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Memory,
                1,
                "Read offset out of bounds",
            ));
        }

        let available = self.buffer.len() - offset;
        let read_size = core::cmp::min(available, buffer.len());

        buffer[0..read_size].copy_from_slice(&self.buffer[offset..offset + read_size]);

        Ok(read_size)
    }
}

// END DEFINITIONS MOVED FROM wrt-foundation/src/memory_traits.rs

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)] // Allow panic/unwrap in tests
mod tests {
    use wrt_error::{
        codes,
        Error,
        ErrorCategory,
    };

    use super::*;

    // Example of a mock PageAllocator for tests if needed when std is not available
    #[cfg(not(feature = "std"))]
    #[derive(Debug)]
    struct MockAllocator {
        allocated_ptr:  Option<NonNull<u8>>,
        allocated_size: usize,
        max_pages:      Option<u32>,
    }

    #[cfg(not(feature = "std"))]
    impl Default for MockAllocator {
        fn default() -> Self {
            Self {
                allocated_ptr:  None,
                allocated_size: 0,
                max_pages:      None,
            }
        }
    }

    // Implementing Send and Sync is safe because we manage the NonNull safely
    #[cfg(not(feature = "std"))]
    unsafe impl Send for MockAllocator {}

    #[cfg(not(feature = "std"))]
    unsafe impl Sync for MockAllocator {}

    #[cfg(not(feature = "std"))]
    impl PageAllocator for MockAllocator {
        fn allocate(
            &mut self,
            initial_pages: u32,
            max_pages: Option<u32>,
        ) -> Result<(NonNull<u8>, usize)> {
            if self.allocated_ptr.is_some() {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Memory already allocated",
                ));
            }
            let size = initial_pages as usize * WASM_PAGE_SIZE;

            // Binary std/no_std choice
            let ptr = if size == 0 {
                NonNull::dangling()
            } else {
                // Use a dummy pointer - this is unsafe but acceptable in tests
                // as we never dereference it or use it outside of identity checks
                NonNull::new(1 as *mut u8).unwrap()
            };

            self.allocated_ptr = Some(ptr);
            self.allocated_size = size;
            self.max_pages = max_pages;
            Ok((self.allocated_ptr.unwrap(), size))
        }

        fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()> {
            if self.allocated_ptr.is_none() {
                return Err(wrt_error::Error::new(
                    wrt_error::ErrorCategory::System,
                    1,
                    "Memory not allocated",
                ));
            }
            let new_total_pages = current_pages + additional_pages;
            if let Some(max) = self.max_pages {
                if new_total_pages > max {
                    return Err(wrt_error::Error::runtime_execution_error(
                        "Memory growth would exceed maximum pages",
                    ));
                }
            }
            let new_size = new_total_pages as usize * WASM_PAGE_SIZE;

            // Binary std/no_std choice
            if new_size > 5 * WASM_PAGE_SIZE {
                return Err(wrt_error::Error::new(
                    wrt_error::ErrorCategory::Memory,
                    1,
                    "Allocation exceeds limit",
                ));
            }

            // Binary std/no_std choice
            // Binary std/no_std choice
            self.allocated_size = new_size;
            Ok(())
        }

        unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()> {
            if self.allocated_ptr.is_none()
                || self.allocated_ptr.unwrap() != ptr
                || self.allocated_size != size
            {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Deallocation mismatch",
                ));
            }
            self.allocated_ptr = None;
            self.allocated_size = 0;
            Ok(())
        }
    }
}
