//! WebAssembly memory implementation.
//!
//! This module provides a comprehensive implementation of WebAssembly linear
//! memory.
//!
//! # Memory Architecture
//!
//! The `Memory` struct is the core implementation for WebAssembly linear
//! memory. It represents a memory instance as defined in the WebAssembly
//! specification. Key features include:
//!
//! - Thread-safe access with internal synchronization
//! - Performance metrics tracking (access counts, peak usage)
//! - Debug name support for easier debugging
//! - Support for both `std` and `no_std` environments
//! - Memory safety checks and bounds validation
//! - Integrated checksum verification for data integrity
//!
//! # Memory Operations
//!
//! The implementation provides methods for all standard WebAssembly memory
//! operations:
//!
//! - Growing memory (`grow`)
//! - Reading from memory (`read`, `get_byte`)
//! - Writing to memory (`write`, `set_byte`)
//! - Additional safety methods for alignment and bounds checking
//!   (`check_alignment`)
//!
//! # Safety Features
//!
//! The implementation includes several safety features:
//!
//! - Checksum verification for data integrity
//! - Bounds checking for all memory operations
//! - Alignment validation
//! - Thread safety guarantees
//! - Memory access tracking
//!
//! # Performance Monitoring
//!
//! Memory instances automatically track usage metrics:
//!
//! - Peak memory usage via `peak_memory()`
//! - Access counts via `access_count()`
//! - Memory access patterns via `memory_stats()`
//!
//! These metrics are updated automatically when memory operations are
//! performed.
//!
//! # Thread Safety
//!
//! Memory operations use appropriate synchronization primitives based on the
//! environment:
//!
//! - In `std` environments, atomic variables are used for metrics
//! - In `no_std` environments, `RwLock` is used for metrics
//!
//! # Usage
//!
//! ```no_run
//! use wrt_runtime::{Memory, MemoryType};
//! use wrt_foundation::types::Limits;
//!
//! // Create a memory type with initial 1 page (64KB) and max 2 pages
//! let mem_type = MemoryType {
//!     limits: Limits { min: 1, max: Some(2) },
//! };
//!
//! // Create a new memory instance
//! let mut memory = Memory::new(mem_type).unwrap();
//!
//! // Write data to memory
//! memory.write(0, &[1, 2, 3, 4]).unwrap();
//!
//! // Read data from memory
//! let mut buffer = [0; 4];
//! memory.read(0, &mut buffer).unwrap();
//! assert_eq!(buffer, [1, 2, 3, 4];
//!
//! // Grow memory by 1 page
//! let old_size = memory.grow(1).unwrap();
//! assert_eq!(old_size, 1); // Previous size was 1 page
//! ```

// Import BorrowMut for SafeMemoryHandler
// alloc is imported in lib.rs with proper feature gates

// Core/std library imports
use core::alloc::Layout;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use core::time::Duration;

#[cfg(not(feature = "std"))]
use core::borrow::BorrowMut;
#[cfg(feature = "std")]
use std::borrow::BorrowMut;

#[cfg(feature = "std")]
use std::vec;
#[cfg(not(feature = "std"))]
use alloc::vec;

// External crates
use wrt_foundation::safe_memory::{
    MemoryProvider, SafeMemoryHandler, SafeSlice, SliceMut as SafeSliceMut,
};
use wrt_foundation::MemoryStats;

#[cfg(not(feature = "std"))]
use wrt_sync::WrtRwLock as RwLock;

// Internal modules
// Temporarily disabled - memory_adapter module is disabled
// use crate::memory_adapter::StdMemoryProvider;
use crate::prelude::{Arc, BoundedCapacity, CoreMemoryType, Debug, Eq, Error, ErrorCategory, Ord, PartialEq, Result, TryFrom, VerificationLevel, str};
#[cfg(not(feature = "std"))]
use crate::prelude::vec_with_capacity;

// Import the MemoryOperations trait from wrt-instructions
use wrt_instructions::memory_ops::MemoryOperations;
// Import atomic operations trait
use wrt_instructions::atomic_ops::AtomicOperations;

// Platform-aware memory providers for memory operations
type LargeMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<67108864>;  // 64MB for memory data
type SmallMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<4096>;  // 4KB for small objects
type MediumMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<65536>;  // 64KB for medium objects

/// WebAssembly page size (64KB)
pub const PAGE_SIZE: usize = 65536;

/// Maximum number of memory pages allowed by WebAssembly spec
pub const MAX_PAGES: u32 = 65536;

/// The maximum memory size in bytes (4GB)
const MAX_MEMORY_BYTES: usize = 4 * 1024 * 1024 * 1024;

/// Memory size error code (must be u16 to match `Error::new`)
const MEMORY_SIZE_TOO_LARGE: u16 = 4001;
/// Invalid offset error code
const INVALID_OFFSET: u16 = 4002;
/// Size too large error code  
const SIZE_TOO_LARGE: u16 = 4003;

/// Safe conversion from WebAssembly u32 offset to Rust usize
/// 
/// # Arguments
/// 
/// * `offset` - WebAssembly offset as u32
/// 
/// # Returns
/// 
/// Ok(usize) if conversion is safe, error otherwise
fn wasm_offset_to_usize(offset: u32) -> Result<usize> {
    usize::try_from(offset).map_err(|_| Error::runtime_execution_error("Runtime execution error"
}

/// Safe conversion from Rust usize to WebAssembly u32
/// 
/// # Arguments
/// 
/// * `size` - Rust size as usize
/// 
/// # Returns
/// 
/// Ok(u32) if conversion is safe, error otherwise  
fn usize_to_wasm_u32(size: usize) -> Result<u32> {
    u32::try_from(size).map_err(|_| Error::new(
        ErrorCategory::Memory, 
        SIZE_TOO_LARGE, 
        "))
}

/// Memory metrics for tracking usage and safety
#[derive(Debug)]
struct MemoryMetrics {
    /// Peak memory usage in bytes
    #[cfg(feature = "std")]
    peak_usage: AtomicUsize,
    /// Memory access counter for profiling
    #[cfg(feature = "std")]
    access_count: AtomicU64,
    /// Maximum size of any access
    #[cfg(feature = "std")]
    max_access_size: AtomicUsize,
    /// Number of unique regions accessed
    #[cfg(feature = "std")]
    unique_regions: AtomicUsize,
    /// Last access offset for validation
    #[cfg(feature = "std")]
    last_access_offset: AtomicUsize,
    /// Last access length for validation
    #[cfg(feature = "std")]
    last_access_length: AtomicUsize,

    /// Peak memory usage (`no_std` version)
    #[cfg(not(feature = "std"))]
    peak_usage: usize,
    /// Memory access counter (`no_std` version)
    #[cfg(not(feature = "std"))]
    access_count: u64,
    /// Maximum size of any access (`no_std` version)
    #[cfg(not(feature = "std"))]
    max_access_size: usize,
    /// Number of unique regions accessed (`no_std` version)
    #[cfg(not(feature = "std"))]
    unique_regions: usize,
    /// Last access offset for validation (`no_std` version)
    #[cfg(not(feature = "std"))]
    last_access_offset: usize,
    /// Last access length for validation (`no_std` version)
    #[cfg(not(feature = "std"))]
    last_access_length: usize,
}

#[cfg(feature = "std")]
impl Clone for MemoryMetrics {
    fn clone(&self) -> Self {
        Self {
            peak_usage: AtomicUsize::new(self.peak_usage.load(Ordering::Relaxed)),
            access_count: AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
            max_access_size: AtomicUsize::new(self.max_access_size.load(Ordering::Relaxed)),
            unique_regions: AtomicUsize::new(self.unique_regions.load(Ordering::Relaxed)),
            last_access_offset: AtomicUsize::new(self.last_access_offset.load(Ordering::Relaxed)),
            last_access_length: AtomicUsize::new(self.last_access_length.load(Ordering::Relaxed)),
        }
}

#[cfg(not(feature = "std"))]
impl Clone for MemoryMetrics {
    fn clone(&self) -> Self {
        // For no_std, fields are directly cloneable (usize, u64)
        Self {
            peak_usage: self.peak_usage,
            access_count: self.access_count,
            max_access_size: self.max_access_size,
            unique_regions: self.unique_regions,
            last_access_offset: self.last_access_offset,
            last_access_length: self.last_access_length,
        }
}

impl MemoryMetrics {
    #[cfg(feature = "std")]
    fn new(size: usize) -> Self {
        Self {
            peak_usage: AtomicUsize::new(size),
            access_count: AtomicU64::new(0),
            max_access_size: AtomicUsize::new(0),
            unique_regions: AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
        }

    #[cfg(not(feature = "std"))]
    fn new(size: usize) -> Self {
        Self {
            peak_usage: size,
            access_count: 0,
            max_access_size: 0,
            unique_regions: 0,
            last_access_offset: 0,
            last_access_length: 0,
        }
}

/// Represents a WebAssembly memory instance
#[derive(Debug)]
pub struct Memory {
    /// The memory type
    pub ty: CoreMemoryType,
    /// The memory data
    #[cfg(feature = "std")]
    pub data: SafeMemoryHandler<LargeMemoryProvider>,
    /// The memory data for `no_std` environments
    #[cfg(not(feature = "std"))]
    pub data: SafeMemoryHandler<LargeMemoryProvider>,
    /// Current number of pages
    pub current_pages: core::sync::atomic::AtomicU32,
    /// Optional name for debugging
    pub debug_name: Option<wrt_foundation::bounded::BoundedString<128, SmallMemoryProvider>>,
    /// Memory metrics for tracking access
    #[cfg(feature = "std")]
    pub metrics: MemoryMetrics,
    /// Memory metrics for tracking access (`RwLock` for `no_std`)
    #[cfg(not(feature = "std"))]
    pub metrics: RwLock<MemoryMetrics>,
    /// Memory verification level
    pub verification_level: VerificationLevel,
}

impl Clone for Memory {
    fn clone(&self) -> Self {
        // Create new SafeMemoryHandler by copying bytes
        let current_bytes =
            self.data.to_vec().unwrap_or_else(|e| panic!("Failed to clone memory data: {}", e;
        // Convert BoundedVec to appropriate provider
        let new_data = {
            #[cfg(feature = "std")]
            {
                // Use LargeMemoryProvider for consistency with struct definition
                let new_provider = LargeMemoryProvider::default());
                SafeMemoryHandler::new(new_provider)
            }
            #[cfg(not(feature = "std"))]
            {
                // Use LargeMemoryProvider for consistency with struct definition
                let new_provider = LargeMemoryProvider::default());
                SafeMemoryHandler::new(new_provider)
            }
        };

        // Clone metrics, handling potential RwLock poisoning for no_std
        #[cfg(feature = "std")]
        let cloned_metrics = MemoryMetrics {
            peak_usage: AtomicUsize::new(self.metrics.peak_usage.load(Ordering::Relaxed)),
            access_count: AtomicU64::new(self.metrics.access_count.load(Ordering::Relaxed)),
            max_access_size: AtomicUsize::new(self.metrics.max_access_size.load(Ordering::Relaxed)),
            unique_regions: AtomicUsize::new(self.metrics.unique_regions.load(Ordering::Relaxed)),
            last_access_offset: AtomicUsize::new(
                self.metrics.last_access_offset.load(Ordering::Relaxed),
            ),
            last_access_length: AtomicUsize::new(
                self.metrics.last_access_length.load(Ordering::Relaxed),
            ),
        };

        #[cfg(not(feature = "std"))]
        let cloned_metrics = {
            let guard = self.metrics.read);
            RwLock::new((*guard).clone()) // Assuming MemoryMetrics is Clone
        };

        Self {
            ty: self.ty,
            data: new_data,
            current_pages: AtomicU32::new(self.current_pages.load(Ordering::Relaxed)),
            debug_name: self.debug_name.clone(),
            metrics: cloned_metrics,
            verification_level: self.verification_level, // Assuming VerificationLevel is Copy
        }
}

impl PartialEq for Memory {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            // && self.data == other.data // SafeMemoryHandler is not PartialEq. Comparing by bytes for now.
            && self.data.to_vec().unwrap_or_default() == other.data.to_vec().unwrap_or_default()
            && self.current_pages.load(Ordering::Relaxed) == other.current_pages.load(Ordering::Relaxed)
            && self.debug_name == other.debug_name
            && self.verification_level == other.verification_level
    }
}

impl Eq for Memory {}

// REMOVED: Default implementation for Memory
// This violates the NO FALLBACK LOGIC rule from CLAUDE.md:
// "NEVER add fallback code that masks bugs or incomplete implementations"
// "FAIL LOUD AND EARLY: If data is missing or incorrect, return an error immediately"
//
// The hardcoded limits (min: 1, max: Some(1)) were masking a bug where
// wrapper modules weren't properly inheriting memory specifications from
// the main module. Memory specifications must always be explicit.

impl wrt_foundation::traits::Checksummable for Memory {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.ty.limits.min.to_le_bytes);
        if let Some(max) = self.ty.limits.max {
            checksum.update_slice(&max.to_le_bytes);
        }
    }
}

impl wrt_foundation::traits::ToBytes for Memory {
    fn serialized_size(&self) -> usize {
        16 // simplified
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.ty.limits.min.to_le_bytes())?;
        writer.write_all(&self.ty.limits.max.unwrap_or(0).to_le_bytes())?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for Memory {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut min_bytes = [0u8; 4];
        reader.read_exact(&mut min_bytes)?;
        let min = u32::from_le_bytes(min_bytes;
        
        let mut max_bytes = [0u8; 4];
        reader.read_exact(&mut max_bytes)?;
        let max = u32::from_le_bytes(max_bytes;
        
        use wrt_foundation::types::{Limits, MemoryType};
        let memory_type = MemoryType {
            limits: Limits { min, max: if max == 0 { None } else { Some(max) } },
            shared: false,
        };
        Self::new(memory_type)
    }
}

impl Memory {
    /// Creates a new memory instance from a type
    ///
    /// # Arguments
    ///
    /// * `ty` - The memory type
    ///
    /// # Returns
    ///
    /// A new memory instance
    ///
    /// # Errors
    ///
    /// Returns an error if the memory type is invalid
    pub fn new(ty: CoreMemoryType) -> Result<Self> {
        let initial_pages = ty.limits.min;
        let maximum_pages_opt = ty.limits.max; // This is Option<u32>

        // Wasm MVP allows up to 65536 pages (4GiB).
        // Binary std/no_std choice
        // PalMemoryProvider::new will pass these pages to the PageAllocator.

        let verification_level = VerificationLevel::Standard; // Or from config

        // Choose and instantiate the PageAllocator
        // The cfg attributes here depend on features enabled for the wrt-platform
        // crate. It's assumed the build system/top-level crate configures these
        // features for wrt-platform.

        // It's better to create a Box<dyn PageAllocator> or use an enum
        // Binary std/no_std choice
        // For compile-time selection based on features, direct instantiation is okay
        // but leads to more complex cfg blocks.
        // Let's try to instantiate the provider directly.

        // Create memory provider based on available features
        #[cfg(feature = "std")]
        let data_handler = {
            let provider = LargeMemoryProvider::default());
            SafeMemoryHandler::new(provider)
        };

        #[cfg(not(feature = "std"))]
        let data_handler = {
            let provider = LargeMemoryProvider::default());
            SafeMemoryHandler::new(provider)
        };

        // Binary std/no_std choice
        // initial_pages. Wasm spec implies memory is zero-initialized. mmap
        // MAP_ANON does this. FallbackAllocator using Vec::resize(val, 0) also
        // does this. So, an explicit resize/zeroing like `data.resize(size, 0)`
        // might be redundant if the provider ensures zeroing. The Provider
        // trait and PalMemoryProvider implementation should ensure this.
        // Binary std/no_std choice
        // should provide zeroed memory for the initial pages.

        let current_size_bytes = wasm_offset_to_usize(initial_pages)? * PAGE_SIZE;

        Ok(Self {
            ty,
            data: data_handler,
            current_pages: core::sync::atomic::AtomicU32::new(initial_pages),
            debug_name: None,
            #[cfg(feature = "std")]
            metrics: MemoryMetrics::new(current_size_bytes),
            #[cfg(not(feature = "std"))]
            metrics: RwLock::new(MemoryMetrics::new(current_size_bytes)),
            verification_level,
        })
    }

    /// Creates a new memory instance with a debug name
    ///
    /// # Arguments
    ///
    /// * `ty` - The memory type
    /// * `name` - The debug name for the memory
    ///
    /// # Returns
    ///
    /// A new Memory instance with the given type and name
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be created
    pub fn new_with_name(ty: CoreMemoryType, name: &str) -> Result<Self> {
        let mut memory = Self::new(ty)?;
        memory.debug_name = Some(wrt_foundation::bounded::BoundedString::try_from_str(
            name, 
            SmallMemoryProvider::default()
        ).map_err(|_| Error::memory_error("Debug name too long"))?;
        Ok(memory)
    }

    /// Sets a debug name for this memory instance
    pub fn set_debug_name(&mut self, name: &str) {
        self.debug_name = Some(wrt_foundation::bounded::BoundedString::try_from_str(
            name, 
            SmallMemoryProvider::default()
        ).unwrap_or_else(|_| {
            // If name is too long, truncate it
            wrt_foundation::bounded::BoundedString::from_str_truncate(
                name,
                SmallMemoryProvider::default()
            ).unwrap()
        };
    }

    /// Returns the debug name of this memory instance, if any
    #[must_use]
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_ref().and_then(|s| s.as_str().ok())
    }

    /// Gets the current size of the memory in pages
    ///
    /// # Returns
    ///
    /// The current size in pages
    #[must_use]
    pub fn size(&self) -> u32 {
        self.current_pages.load(Ordering::Relaxed)
    }

    /// Gets the current size of the memory in bytes
    ///
    /// # Returns
    ///
    /// The current size in bytes
    #[must_use]
    pub fn size_in_bytes(&self) -> usize {
        let pages = self.current_pages.load(Ordering::Relaxed;
        wasm_offset_to_usize(pages).unwrap_or(0) * PAGE_SIZE
    }

    /// A reference to the memory data as a `Vec<u8>`
    ///
    /// # Warning
    ///
    /// This method is primarily for compatibility with existing code and should
    /// be avoided in new code. It creates a full copy of the memory data
    /// which is inefficient.
    ///
    /// For memory-safe access, prefer using `get_safe_slice()` or
    /// `as_safe_slice()` methods instead.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        // Use the SafeMemoryHandler to get data through a safe slice to ensure
        // memory integrity is verified during the operation
        let data_size = self.data.size);
        if data_size == 0 {
            return Ok(Vec::new();
        }

        // Get a safe slice over the entire memory
        let safe_slice = self.data.get_slice(0, data_size)?;

        // Get the data from the safe slice and create a copy
        let memory_data = safe_slice.data()?;

        // Create a new RuntimeVec with the data
        let mut result = Vec::with_capacity(data_size;
        for &byte in memory_data.iter().take(result.capacity()) {
            result.push(byte);
            }

        Ok(result)
    }

    /// Returns the peak memory usage in bytes
    pub fn peak_memory(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.metrics.peak_usage.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.peak_usage
        }

    /// Returns the total number of memory accesses
    pub fn access_count(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            self.metrics.access_count.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.access_count
        }

    /// Increment the access count for memory operations
    fn increment_access_count(&self, offset: usize, len: usize) {
        #[cfg(feature = "std")]
        {
            self.metrics.access_count.fetch_add(1, Ordering::Relaxed;
            self.metrics.max_access_size.fetch_max(len, Ordering::Relaxed;
            self.metrics.last_access_offset.store(offset, Ordering::Relaxed;
            self.metrics.last_access_length.store(len, Ordering::Relaxed;
        }

        #[cfg(not(feature = "std"))]
        {
            // Use write() method with WrtRwLock
            let mut metrics = self.metrics.write);
            metrics.access_count += 1;
            metrics.max_access_size = metrics.max_access_size.max(len;
            metrics.last_access_offset = offset;
            metrics.last_access_length = len;
        }

    /// Update the peak memory usage statistic
    fn update_peak_memory(&self) {
        let current_size = self.size_in_bytes);

        #[cfg(feature = "std")]
        {
            let mut current_peak = self.metrics.peak_usage.load(Ordering::Relaxed;
            while current_size > current_peak {
                match self.metrics.peak_usage.compare_exchange(
                    current_peak,
                    current_size,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => current_peak = actual,
                }
        }

        #[cfg(not(feature = "std"))]
        {
            // Use write() method with WrtRwLock
            let mut metrics = self.metrics.write);
            metrics.peak_usage = metrics.peak_usage.max(current_size;
        }

    /// Returns the maximum size of any memory access
    pub fn max_access_size(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.metrics.max_access_size.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.max_access_size
        }

    /// Returns the number of unique memory regions accessed
    pub fn unique_regions(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.metrics.unique_regions.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.unique_regions
        }

    /// Returns the offset of the most recent memory access
    pub fn last_access_offset(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.metrics.last_access_offset.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.last_access_offset
        }

    /// Returns the length of the most recent memory access
    pub fn last_access_length(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.metrics.last_access_length.load(Ordering::Relaxed)
        }

        #[cfg(not(feature = "std"))]
        {
            // Use read() method with WrtRwLock
            let metrics = self.metrics.read);
            metrics.last_access_length
        }

    /// Grows memory by the given number of pages
    ///
    /// # Arguments
    ///
    /// * `pages` - The number of pages to grow by
    ///
    /// # Returns
    ///
    /// The previous number of pages if successful, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be grown
    pub fn grow(&mut self, pages: u32) -> Result<u32> {
        // Return early if not growing
        if pages == 0 {
            return Ok(self.current_pages.load(Ordering::Relaxed;
        }

        // Check that growing wouldn't exceed max pages
        let current_pages_val = self.current_pages.load(Ordering::Relaxed;
        let new_page_count = current_pages_val.checked_add(pages).ok_or_else(|| {
            Error::runtime_execution_error(",
            )
        })?;

        // Check against the maximum allowed by type
        if let Some(max) = self.ty.limits.max {
            if new_page_count > max {
                return Err(Error::resource_limit_exceeded(";
            }

        // Check against the absolute maximum (4GB)
        if new_page_count > MAX_PAGES {
            return Err(Error::resource_limit_exceeded("Runtime operation error";
        }

        // Calculate the new size in bytes
        let old_size = self.data.size);
        let new_size = wasm_offset_to_usize(new_page_count)? * PAGE_SIZE;

        // Resize the underlying data
        self.data.resize(new_size)?;

        // Update the page count
        let old_pages = self.current_pages.swap(new_page_count, Ordering::Relaxed;

        // Update peak memory usage
        self.update_peak_memory);

        Ok(old_pages)
    }

    /// Read data from memory into a buffer
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to read from
    /// * `buffer` - The buffer to read into
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if the memory access is invalid
    pub fn read(&self, offset: u32, buffer: &mut [u8]) -> Result<()> {
        // Empty read is always successful
        if buffer.is_empty() {
            return Ok();
        }

        // Calculate total size and verify bounds
        let offset_usize = wasm_offset_to_usize(offset)?;
        let size = buffer.len();

        // Track this access for profiling
        self.increment_access_count(offset_usize, size;

        // Use safe memory get_slice to get a verified slice
        let safe_slice = self.data.get_slice(offset_usize, size)?;

        // Copy from the safe slice to the buffer
        buffer.copy_from_slice(safe_slice.data()?;

        Ok(())
    }

    /// Write data from a buffer into memory
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to write to
    /// * `buffer` - The buffer to write from
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if the memory access is invalid
    pub fn write(&mut self, offset: u32, buffer: &[u8]) -> Result<()> {
        // Empty write is always successful
        if buffer.is_empty() {
            return Ok();
        }

        // Calculate total size and verify bounds
        let offset_usize = wasm_offset_to_usize(offset)?;
        let size = buffer.len();
        let end = offset_usize.checked_add(size).ok_or_else(|| {
            Error::memory_out_of_bounds("Memory write would overflow")
        })?;

        // Verify the access is within memory bounds
        if end > self.size_in_bytes() {
            return Err(Error::memory_out_of_bounds("Runtime operation error";
        }

        // Track this access for profiling
        self.increment_access_count(offset_usize, size;

        // Use the SafeMemoryHandler's write_data method for efficient direct writing
        self.data.write_data(offset_usize, buffer)?;

        // Update the peak memory usage
        self.update_peak_memory);

        Ok(())
    }

    /// Gets a byte from memory
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to read from
    ///
    /// # Returns
    ///
    /// The byte at the given offset
    ///
    /// # Errors
    ///
    /// Returns an error if the offset is out of bounds
    pub fn get_byte(&self, offset: u32) -> Result<u8> {
        if !self.verify_bounds(offset, 1) {
            return Err(Error::validation_error("Memory access out of bounds";
        }

        let offset_usize = wasm_offset_to_usize(offset)?;
        self.increment_access_count(offset_usize, 1);

        // Use SafeMemoryHandler to get a safe slice
        let slice = self.data.get_slice(offset_usize, 1)?;
        let data = slice.data()?;
        Ok(data[0])
    }

    /// Sets a byte in memory
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write was successful
    ///
    /// # Errors
    ///
    /// Returns an error if the offset is out of bounds
    pub fn set_byte(&mut self, offset: u32, value: u8) -> Result<()> {
        if !self.verify_bounds(offset, 1) {
            return Err(Error::validation_error("Memory access out of bounds";
        }

        let offset_usize = wasm_offset_to_usize(offset)?;
        self.increment_access_count(offset_usize, 1);

        // This is a simpler case - just write a single byte
        // using the write method which handles all the safety checks
        self.write(offset, &[value])
    }

    /// Verifies that a memory access is within bounds
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to access
    /// * `len` - The length to access
    ///
    /// # Returns
    ///
    /// True if the access is within bounds, false otherwise
    fn verify_bounds(&self, offset: u32, len: u32) -> bool {
        if len == 0 {
            return true;
        }

        // Get current data size
        let data_size = self.data.len();

        // Get the last byte that would be accessed
        let end_offset = match offset.checked_add(len) {
            Some(end) => match wasm_offset_to_usize(end) {
                Ok(end_usize) => end_usize,
                Err(_) => return false, // Conversion error
            },
            None => return false, // Overflow
        };

        // Check if the end offset is within bounds (inclusive)
        end_offset <= data_size
    }

    /// Check alignment for memory accesses
    pub fn check_alignment(&self, addr: u32, access_size: u32, align: u32) -> Result<()> {
        if addr % align != 0 {
            return Err(Error::validation_error("Runtime operation error";
        }

        let addr = wasm_offset_to_usize(addr)?;
        let access_size = wasm_offset_to_usize(access_size)?;
        if addr + access_size > self.data.size() {
            return Err(Error::validation_error("Memory access out of bounds";
        }

        Ok(())
    }

    /// Gets a memory-safe slice from memory at the specified address
    ///
    /// This is the preferred method for accessing memory when safety is
    /// important. The returned `SafeSlice` includes integrity verification to
    /// prevent memory corruption.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
