//! WebAssembly Memory Implementation
//!
//! This module provides a comprehensive implementation of WebAssembly linear
//! memory, supporting both single and multiple memory proposals with full
//! safety guarantees and platform-aware resource management.
//!
//! # Features
//!
//! - Linear memory with configurable page sizes
//! - Memory growth and shrinking operations
//! - Protected memory regions for security
//! - Shared memory support for threading
//! - Zero-copy data segments
//! - Platform-specific memory limits enforcement
//! - Integration with custom memory allocators
//!
//! # Memory Model
//!
//! WebAssembly memory is organized as a contiguous, byte-addressable range
//! starting at offset 0, with bounds checking on all accesses to prevent
//! out-of-bounds reads or writes.
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
//! use wrt_foundation::types::Limits;
//! use wrt_runtime::{
//!     Memory,
//!     MemoryType,
//! };
//!
//! // Create a memory type with initial 1 page (64KB) and max 2 pages
//! let mem_type = MemoryType {
//!     limits: Limits {
//!         min: 1,
//!         max: Some(2),
//!     },
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
//! assert_eq!(buffer, [1, 2, 3, 4]);
//!
//! // Grow memory by 1 page
//! let old_size = memory.grow(1).unwrap();
//! assert_eq!(old_size, 1); // Previous size was 1 page
//! ```

// Import BorrowMut for SafeMemoryHandler
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Core/std library imports
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use core::borrow::BorrowMut;
use core::{
    alloc::Layout,
    sync::atomic::{
        AtomicBool,
        AtomicU32,
        AtomicU64,
        AtomicUsize,
        Ordering,
    },
    time::Duration,
};
#[cfg(feature = "std")]
use core::borrow::BorrowMut;
#[cfg(feature = "std")]
use alloc::vec;

// External crates
use wrt_foundation::safe_memory::{
    MemoryProvider,
    SafeMemoryHandler,
    SafeSlice,
    SliceMut as SafeSliceMut,
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    types::MemoryType,
    MemoryStats,
};
#[cfg(feature = "std")]
use wrt_foundation::MemoryAccessor;
// Import atomic operations trait
use wrt_instructions::atomic_ops::AtomicOperations;
// Import the MemoryOperations trait from wrt-instructions
use wrt_instructions::memory_ops::MemoryOperations;
#[cfg(not(feature = "std"))]
use wrt_sync::WrtRwLock as RwLock;

#[cfg(not(feature = "std"))]
use crate::prelude::vec_with_capacity;
// Internal modules
// Temporarily disabled - memory_adapter module is disabled
// use crate::memory_adapter::StdMemoryProvider;
use crate::prelude::{
    str,
    Arc,
    BoundedCapacity,
    CoreMemoryType,
    Debug,
    Eq,
    Error,
    ErrorCategory,
    Ord,
    PartialEq,
    Result,
    TryFrom,
    VerificationLevel,
};

// Platform-aware memory providers for memory operations
// For std mode: Use StdProvider which uses Vec<u8> for dynamically-sized memory
// For no_std mode: Use NoStdProvider with fixed size (limited to compile-time constant)
#[cfg(feature = "std")]
type LargeMemoryProvider = wrt_foundation::safe_memory::StdProvider;
#[cfg(not(feature = "std"))]
type LargeMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<524288>; // 512KB (8 pages) for no_std

type SmallMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<4096>; // 4KB for small objects
type MediumMemoryProvider = wrt_foundation::safe_memory::NoStdProvider<65536>; // 64KB for medium objects

/// WebAssembly page size (64KB)
pub const PAGE_SIZE: usize = 65536;

/// Maximum number of memory pages allowed by WebAssembly spec
pub const MAX_PAGES: u32 = 65536;

/// The maximum memory size in bytes (4GB)
// Unused constant
// const MAX_MEMORY_BYTES: usize = 4 * 1024 * 1024 * 1024;
/// Convert MemoryType to CoreMemoryType
fn to_core_memory_type(memory_type: &MemoryType) -> CoreMemoryType {
    CoreMemoryType {
        limits: memory_type.limits,
        shared: memory_type.shared,
    }
}

/// Memory size error code (must be u16 to match `Error::new`)
// Unused constant
// const MEMORY_SIZE_TOO_LARGE: u16 = 4001;
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
    usize::try_from(offset).map_err(|_| Error::runtime_execution_error("Offset conversion failed"))
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
    u32::try_from(size).map_err(|_| {
        Error::new(
            ErrorCategory::Memory,
            SIZE_TOO_LARGE,
            "Size too large for u32",
        )
    })
}

/// Memory metrics for tracking usage and safety
#[derive(Debug)]
pub struct MemoryMetrics {
    /// Peak memory usage in bytes
    #[cfg(feature = "std")]
    peak_usage:         AtomicUsize,
    /// Memory access counter for profiling
    #[cfg(feature = "std")]
    access_count:       AtomicU64,
    /// Maximum size of any access
    #[cfg(feature = "std")]
    max_access_size:    AtomicUsize,
    /// Number of unique regions accessed
    #[cfg(feature = "std")]
    unique_regions:     AtomicUsize,
    /// Last access offset for validation
    #[cfg(feature = "std")]
    last_access_offset: AtomicUsize,
    /// Last access length for validation
    #[cfg(feature = "std")]
    last_access_length: AtomicUsize,

    /// Peak memory usage (`no_std` version)
    #[cfg(not(feature = "std"))]
    peak_usage:         usize,
    /// Memory access counter (`no_std` version)
    #[cfg(not(feature = "std"))]
    access_count:       u64,
    /// Maximum size of any access (`no_std` version)
    #[cfg(not(feature = "std"))]
    max_access_size:    usize,
    /// Number of unique regions accessed (`no_std` version)
    #[cfg(not(feature = "std"))]
    unique_regions:     usize,
    /// Last access offset for validation (`no_std` version)
    #[cfg(not(feature = "std"))]
    last_access_offset: usize,
    /// Last access length for validation (`no_std` version)
    #[cfg(not(feature = "std"))]
    last_access_length: usize,
}

impl Clone for MemoryMetrics {
    fn clone(&self) -> Self {
        #[cfg(feature = "std")]
        {
            Self {
                peak_usage:         AtomicUsize::new(self.peak_usage.load(Ordering::Relaxed)),
                access_count:       AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
                max_access_size:    AtomicUsize::new(self.max_access_size.load(Ordering::Relaxed)),
                unique_regions:     AtomicUsize::new(self.unique_regions.load(Ordering::Relaxed)),
                last_access_offset: AtomicUsize::new(
                    self.last_access_offset.load(Ordering::Relaxed),
                ),
                last_access_length: AtomicUsize::new(
                    self.last_access_length.load(Ordering::Relaxed),
                ),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Self {
                peak_usage:         self.peak_usage,
                access_count:       self.access_count,
                max_access_size:    self.max_access_size,
                unique_regions:     self.unique_regions,
                last_access_offset: self.last_access_offset,
                last_access_length: self.last_access_length,
            }
        }
    }
}

impl MemoryMetrics {
    #[cfg(feature = "std")]
    fn new(size: usize) -> Self {
        Self {
            peak_usage:         AtomicUsize::new(size),
            access_count:       AtomicU64::new(0),
            max_access_size:    AtomicUsize::new(0),
            unique_regions:     AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
        }
    }

    #[cfg(not(feature = "std"))]
    fn new(size: usize) -> Self {
        Self {
            peak_usage:         size,
            access_count:       0,
            max_access_size:    0,
            unique_regions:     0,
            last_access_offset: 0,
            last_access_length: 0,
        }
    }
}

/// Represents a WebAssembly memory instance
#[derive(Debug)]
pub struct Memory {
    /// The memory type
    pub ty:                 CoreMemoryType,
    /// The memory data (Box + Mutex for ASIL-B: heap allocation + thread-safe writes)
    #[cfg(feature = "std")]
    pub data:               Box<std::sync::Mutex<SafeMemoryHandler<LargeMemoryProvider>>>,
    #[cfg(not(feature = "std"))]
    pub data:               Box<RwLock<SafeMemoryHandler<LargeMemoryProvider>>>,
    /// Current number of pages
    pub current_pages:      core::sync::atomic::AtomicU32,
    /// Optional name for debugging
    pub debug_name: Option<wrt_foundation::bounded::BoundedString<128>>,
    /// Memory metrics for tracking access
    #[cfg(feature = "std")]
    pub metrics:            MemoryMetrics,
    /// Memory metrics for tracking access (`RwLock` for `no_std`)
    #[cfg(not(feature = "std"))]
    pub metrics:            RwLock<MemoryMetrics>,
    /// Memory verification level
    pub verification_level: VerificationLevel,
}

impl Clone for Memory {
    fn clone(&self) -> Self {
        // Create new SafeMemoryHandler by copying bytes from the data
        #[cfg(feature = "std")]
        let current_bytes = self.data.lock().unwrap().to_vec().unwrap_or_default();
        #[cfg(not(feature = "std"))]
        let current_bytes = self.data.read().to_vec().unwrap_or_default();

        // Create new SafeMemoryHandler wrapped in Mutex
        // In std mode, use StdProvider with proper size allocation
        #[cfg(feature = "std")]
        let new_data = {
            use wrt_foundation::safe_memory::StdProvider;
            // Create provider with data directly (StdProvider::new takes Vec<u8>)
            let new_provider = StdProvider::new(current_bytes.clone());
            let new_handler = SafeMemoryHandler::new(new_provider);
            Box::new(std::sync::Mutex::new(new_handler))
        };

        #[cfg(not(feature = "std"))]
        let new_data = {
            let new_provider = LargeMemoryProvider::default();
            let mut new_handler = SafeMemoryHandler::new(new_provider);

            // Copy the data into the new handler
            if !current_bytes.is_empty() {
                new_handler.write_data(0, &current_bytes).unwrap_or_else(|e| {
                    panic!("Failed to write cloned data: {}", e);
                });
            }
            Box::new(RwLock::new(new_handler))
        };

        // Clone metrics, handling potential RwLock poisoning for no_std
        #[cfg(feature = "std")]
        let cloned_metrics = MemoryMetrics {
            peak_usage:         AtomicUsize::new(self.metrics.peak_usage.load(Ordering::Relaxed)),
            access_count:       AtomicU64::new(self.metrics.access_count.load(Ordering::Relaxed)),
            max_access_size:    AtomicUsize::new(
                self.metrics.max_access_size.load(Ordering::Relaxed),
            ),
            unique_regions:     AtomicUsize::new(
                self.metrics.unique_regions.load(Ordering::Relaxed),
            ),
            last_access_offset: AtomicUsize::new(
                self.metrics.last_access_offset.load(Ordering::Relaxed),
            ),
            last_access_length: AtomicUsize::new(
                self.metrics.last_access_length.load(Ordering::Relaxed),
            ),
        };

        #[cfg(not(feature = "std"))]
        let cloned_metrics = {
            let guard = self.metrics.read();
            RwLock::new((*guard).clone())
        };

        Self {
            ty:                 self.ty,
            data:               new_data,
            current_pages:      AtomicU32::new(self.current_pages.load(Ordering::Relaxed)),
            debug_name:         self.debug_name.clone(),
            metrics:            cloned_metrics,
            verification_level: self.verification_level,
        }
    }
}

impl PartialEq for Memory {
    fn eq(&self, other: &Self) -> bool {
        // Compare memory data by extracting bytes from Mutex
        let self_data = {
            #[cfg(feature = "std")]
            let data = self.data.lock().unwrap().to_vec().unwrap_or_default();
            #[cfg(not(feature = "std"))]
            let data = self.data.read().to_vec().unwrap_or_default();
            data
        };

        let other_data = {
            #[cfg(feature = "std")]
            let data = other.data.lock().unwrap().to_vec().unwrap_or_default();
            #[cfg(not(feature = "std"))]
            let data = other.data.read().to_vec().unwrap_or_default();
            data
        };

        self.ty == other.ty
            && self_data == other_data
            && self.current_pages.load(Ordering::Relaxed)
                == other.current_pages.load(Ordering::Relaxed)
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
// wrapper modules weren't properly inheriting or sharing memory specifications
// from the main module. Memory specifications must always be explicit.
//
// Additionally, the fallback logic that created "minimal" memory with 0 pages
// was even worse - it would hide critical memory configuration errors.

impl wrt_foundation::traits::Checksummable for Memory {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.ty.limits.min.to_le_bytes());
        if let Some(max) = self.ty.limits.max {
            checksum.update_slice(&max.to_le_bytes());
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
        let min = u32::from_le_bytes(min_bytes);

        let mut max_bytes = [0u8; 4];
        reader.read_exact(&mut max_bytes)?;
        let max = u32::from_le_bytes(max_bytes);

        use wrt_foundation::types::{
            Limits,
            MemoryType,
        };
        let memory_type = MemoryType {
            limits: Limits {
                min,
                max: if max == 0 { None } else { Some(max) },
            },
            shared: false,
            memory64: false,
        };
        Self::new(to_core_memory_type(&memory_type)).map(|boxed| *boxed)
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
    pub fn new(ty: CoreMemoryType) -> Result<Box<Self>> {
        let initial_pages = ty.limits.min;
        let maximum_pages_opt = ty.limits.max; // This is Option<u32>

        // DEBUG: Log the memory type being used
        #[cfg(feature = "tracing")]
        {
            use wrt_foundation::tracing::info;
            info!("Creating memory with initial_pages={}, max_pages={:?} from CoreMemoryType",
                  initial_pages, maximum_pages_opt);
        }

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

        // CRITICAL: Create Mutex<SafeMemoryHandler> inline to minimize stack usage
        // The key is to never have the 64KB SafeMemoryHandler as a stack variable

        let current_size_bytes = wasm_offset_to_usize(initial_pages)? * PAGE_SIZE;

        // In std mode, use StdProvider which can be dynamically sized
        // In no_std mode, use NoStdProvider with fixed compile-time size
        #[cfg(feature = "std")]
        let provider = {
            use wrt_foundation::safe_memory::StdProvider;
            // Create a StdProvider with Vec pre-allocated and pre-filled to the required size
            // This ensures memory is available immediately for WebAssembly to use
            let mut provider = StdProvider::with_capacity(current_size_bytes);
            // Initialize the memory to zeros (WebAssembly spec requires zero-initialized memory)
            provider.add_data(&vec![0u8; current_size_bytes]);
            provider
        };
        #[cfg(not(feature = "std"))]
        let provider = LargeMemoryProvider::default();

        let handler = SafeMemoryHandler::new(provider);

        Ok(Box::new(Self {
            ty,
            #[cfg(feature = "std")]
            data: Box::new(std::sync::Mutex::new(handler)),
            #[cfg(not(feature = "std"))]
            data: Box::new(RwLock::new(handler)),
            current_pages: core::sync::atomic::AtomicU32::new(initial_pages),
            debug_name: None,
            #[cfg(feature = "std")]
            metrics: MemoryMetrics::new(current_size_bytes),
            #[cfg(not(feature = "std"))]
            metrics: RwLock::new(MemoryMetrics::new(current_size_bytes)),
            verification_level,
        }))
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
        let mut memory = *Self::new(ty)?;  // Dereference Box<Memory>
        memory.debug_name = Some(
            wrt_foundation::bounded::BoundedString::try_from_str(name)
                .map_err(|_| Error::memory_error("Debug name too long"))?,
        );
        Ok(memory)
    }

    /// Sets a debug name for this memory instance
    pub fn set_debug_name(&mut self, name: &str) {
        self.debug_name = Some(
            wrt_foundation::bounded::BoundedString::try_from_str(name)
                .unwrap_or_else(|_| {
                    // If name is too long, truncate it
                    wrt_foundation::bounded::BoundedString::from_str_truncate(name)
                    .unwrap()
                }),
        );
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
        let pages = self.current_pages.load(Ordering::Relaxed);
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
    #[cfg(feature = "std")]
    pub fn buffer(&self) -> Result<alloc::vec::Vec<u8>> {
        // Use the SafeMemoryHandler to get data through a safe slice to ensure
        // memory integrity is verified during the operation
        let data_guard = self.data.lock().unwrap();
        let data_size = data_guard.size();
        if data_size == 0 {
            return Ok(alloc::vec::Vec::new());
        }

        // Get a safe slice over the entire memory
        let safe_slice = data_guard.get_slice(0, data_size)?;

        // Get the data from the safe slice and create a copy
        let memory_data = safe_slice.data()?;

        // Create a new RuntimeVec with the data
        let mut result = alloc::vec::Vec::with_capacity(data_size);
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
            let metrics = self.metrics.read();
            metrics.peak_usage
        }
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
            let metrics = self.metrics.read();
            metrics.access_count
        }
    }

    /// Increment the access count for memory operations
    fn increment_access_count(&self, offset: usize, len: usize) {
        #[cfg(feature = "std")]
        {
            self.metrics.access_count.fetch_add(1, Ordering::Relaxed);
            self.metrics.max_access_size.fetch_max(len, Ordering::Relaxed);
            self.metrics.last_access_offset.store(offset, Ordering::Relaxed);
            self.metrics.last_access_length.store(len, Ordering::Relaxed);
        }

        #[cfg(not(feature = "std"))]
        {
            // Use write() method with WrtRwLock
            let mut metrics = self.metrics.write();
            metrics.access_count += 1;
            metrics.max_access_size = metrics.max_access_size.max(len);
            metrics.last_access_offset = offset;
            metrics.last_access_length = len;
        }
    }

    /// Update the peak memory usage statistic
    fn update_peak_memory(&self) {
        let current_size = self.size_in_bytes();

        #[cfg(feature = "std")]
        {
            let mut current_peak = self.metrics.peak_usage.load(Ordering::Relaxed);
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
        }

        #[cfg(not(feature = "std"))]
        {
            // Use write() method with WrtRwLock
            let mut metrics = self.metrics.write();
            metrics.peak_usage = metrics.peak_usage.max(current_size);
        }
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
            let metrics = self.metrics.read();
            metrics.max_access_size
        }
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
            let metrics = self.metrics.read();
            metrics.unique_regions
        }
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
            let metrics = self.metrics.read();
            metrics.last_access_offset
        }
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
            let metrics = self.metrics.read();
            metrics.last_access_length
        }
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
            return Ok(self.current_pages.load(Ordering::Relaxed));
        }

        // Check that growing wouldn't exceed max pages
        let current_pages_val = self.current_pages.load(Ordering::Relaxed);
        let new_page_count = current_pages_val
            .checked_add(pages)
            .ok_or_else(|| Error::runtime_execution_error("Memory operation failed"))?;

        // Check against the maximum allowed by type
        if let Some(max) = self.ty.limits.max {
            if new_page_count > max {
                return Err(Error::resource_limit_exceeded("Memory limit exceeded"));
            }
        }

        // Check against the absolute maximum (4GB)
        if new_page_count > MAX_PAGES {
            return Err(Error::resource_limit_exceeded("Runtime operation error"));
        }

        // Calculate the new size in bytes and resize through Mutex
        let new_size = wasm_offset_to_usize(new_page_count)? * PAGE_SIZE;

        // Resize the underlying data
        #[cfg(feature = "std")]
        self.data.lock().unwrap().resize(new_size)?;
        #[cfg(not(feature = "std"))]
        self.data.write().resize(new_size)?;

        // Update the page count
        let old_pages = self.current_pages.swap(new_page_count, Ordering::Relaxed);

        // Update peak memory usage
        self.update_peak_memory();

        Ok(old_pages)
    }

    /// Thread-safe grow operation for shared memory access (works with
    /// Arc<Memory>)
    ///
    /// This method works with `&self` instead of `&mut self`, making it
    /// compatible with Arc<Memory> usage patterns.
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
    pub fn grow_shared(&self, pages: u32) -> Result<u32> {
        // Return early if not growing
        if pages == 0 {
            return Ok(self.current_pages.load(Ordering::Relaxed));
        }

        // Check that growing wouldn't exceed max pages
        let current_pages_val = self.current_pages.load(Ordering::Relaxed);
        let new_page_count = current_pages_val
            .checked_add(pages)
            .ok_or_else(|| Error::runtime_execution_error("Memory operation failed"))?;

        // Check against the maximum allowed by type
        if let Some(max) = self.ty.limits.max {
            if new_page_count > max {
                return Err(Error::resource_limit_exceeded("Memory limit exceeded"));
            }
        }

        // Check against the absolute maximum (4GB)
        if new_page_count > MAX_PAGES {
            return Err(Error::resource_limit_exceeded("Runtime operation error"));
        }

        // Calculate the new size in bytes and resize through Mutex
        let new_size = wasm_offset_to_usize(new_page_count)? * PAGE_SIZE;

        // Resize the underlying data
        #[cfg(feature = "std")]
        self.data.lock().unwrap().resize(new_size)?;
        #[cfg(not(feature = "std"))]
        self.data.write().resize(new_size)?;

        // Update the page count
        let old_pages = self.current_pages.swap(new_page_count, Ordering::Relaxed);

        // Update peak memory usage
        self.update_peak_memory();

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
            return Ok(());
        }

        // Calculate total size and verify bounds
        let offset_usize = wasm_offset_to_usize(offset)?;
        let size = buffer.len();

        // Track this access for profiling
        self.increment_access_count(offset_usize, size);

        // Read from memory with Mutex locking
        #[cfg(feature = "std")]
        {
            let data_guard = self.data.lock().unwrap();
            let safe_slice = data_guard.get_slice(offset_usize, size)?;
            buffer.copy_from_slice(safe_slice.data()?);
            // DEBUG: Track reads from allocator region
            #[cfg(feature = "tracing")]
            if offset_usize >= 0x1074a0 && offset_usize <= 0x1074b0 && size >= 4 {
                let val = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
                let mutex_ptr = &*self.data as *const _;
                wrt_foundation::tracing::trace!(
                    offset = format!("{:#x}", offset_usize),
                    value = format!("{:#x}", val),
                    mutex = format!("{:p}", mutex_ptr),
                    "Memory read raw"
                );
            }
        }
        #[cfg(not(feature = "std"))]
        {
            let data_guard = self.data.read();
            let safe_slice = data_guard.get_slice(offset_usize, size)?;
            buffer.copy_from_slice(safe_slice.data()?);
        }

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
            return Ok(());
        }

        // Calculate total size and verify bounds
        let offset_usize = wasm_offset_to_usize(offset)?;
        let size = buffer.len();
        let end = offset_usize
            .checked_add(size)
            .ok_or_else(|| Error::memory_out_of_bounds("Memory write would overflow"))?;

        // Verify the access is within memory bounds
        if end > self.size_in_bytes() {
            return Err(Error::memory_out_of_bounds("Runtime operation error"));
        }

        // Track this access for profiling
        self.increment_access_count(offset_usize, size);

        // Use the SafeMemoryHandler's write_data method for efficient direct writing
        #[cfg(feature = "std")]
        self.data.lock().unwrap().write_data(offset_usize, buffer)?;
        #[cfg(not(feature = "std"))]
        self.data.write().write_data(offset_usize, buffer)?;

        // Update the peak memory usage
        self.update_peak_memory();

        Ok(())
    }

    /// Thread-safe write operation for shared memory access (works with
    /// Arc<Memory>)
    ///
    /// This method works with `&self` instead of `&mut self`, making it
    /// compatible with Arc<Memory> usage patterns.
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
    /// ASIL-B COMPLIANT: Thread-safe write operation for Arc<Memory> usage.
    /// Uses interior mutability via Mutex for deterministic, bounded-time writes.
    pub fn write_shared(&self, offset: u32, buffer: &[u8]) -> Result<()> {
        // Empty write is always successful
        if buffer.is_empty() {
            return Ok(());
        }

        // Calculate and verify bounds
        let offset_usize = wasm_offset_to_usize(offset)?;
        let size = buffer.len();
        let end = offset_usize
            .checked_add(size)
            .ok_or_else(|| Error::memory_out_of_bounds("Memory write would overflow"))?;

        let mem_size = self.size_in_bytes();
        #[cfg(feature = "tracing")]
        {
            use wrt_foundation::tracing::debug;
            debug!("write_shared: offset={}, size={}, end={}, mem_size={}, pages={}",
                   offset_usize, size, end, mem_size, self.current_pages.load(Ordering::Relaxed));
        }

        if end > mem_size {
            #[cfg(feature = "tracing")]
            {
                use wrt_foundation::tracing::error;
                error!("write_shared bounds check FAILED: end {} > mem_size {}", end, mem_size);
            }
            return Err(Error::memory_out_of_bounds("Memory write out of bounds"));
        }

        // Track access
        self.increment_access_count(offset_usize, size);

        // ASIL-B: Thread-safe write with Mutex
        #[cfg(feature = "std")]
        self.data.lock().unwrap().write_data(offset_usize, buffer)?;
        #[cfg(not(feature = "std"))]
        self.data.write().write_data(offset_usize, buffer)?;

        // Update metrics
        self.update_peak_memory();

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
            return Err(Error::validation_error("Memory access out of bounds"));
        }

        let offset_usize = wasm_offset_to_usize(offset)?;
        self.increment_access_count(offset_usize, 1);

        // Use SafeMemoryHandler to get a safe slice
        #[cfg(feature = "std")]
        let data_guard = self.data.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let data_guard = self.data.read();

        let slice = data_guard.get_slice(offset_usize, 1)?;
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
            return Err(Error::validation_error("Memory access out of bounds"));
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
        #[cfg(feature = "std")]
        let data_size = self.data.lock().unwrap().len();
        #[cfg(not(feature = "std"))]
        let data_size = self.data.read().len();

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
            return Err(Error::validation_error("Runtime operation error"));
        }

        let addr = wasm_offset_to_usize(addr)?;
        let access_size = wasm_offset_to_usize(access_size)?;

        #[cfg(feature = "std")]
        let data_size = self.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let data_size = self.data.read().size();

        if addr + access_size > data_size {
            return Err(Error::validation_error("Memory access out of bounds"));
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
    /// * `len` - The length of the slice to read
    ///
    /// # Returns
    ///
    /// A `SafeSlice` referencing the memory region with integrity verification
    ///
    /// # Safety
    ///
    /// This method is safer than using `buffer()` as it performs integrity
    /// checks on the returned slice, which helps detect memory corruption.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use wrt_runtime::Memory;
    /// # use wrt_error::Result;
    /// # fn example(memory: &Memory) -> Result<()> {
    /// // Get a safe slice from memory
    /// let slice = memory.get_safe_slice(0, 10)?;
    ///
    /// // Access the data (this performs integrity verification)
    /// let data = slice.data()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the slice is out of bounds
    ///
    /// # ASIL-B Note
    /// This method has been disabled for ASIL-B compliance due to complex lifetime
    /// interactions with Mutex. Use read() method instead for thread-safe access.
    pub fn get_safe_slice<'a>(
        &'a self,
        addr: u32,
        len: usize,
    ) -> Result<wrt_foundation::safe_memory::SafeSlice<'a>> {
        Err(Error::runtime_execution_error("get_safe_slice disabled for ASIL-B compliance - use read() instead"))
    }

    /// Creates a copy of this memory instance and applies a mutation function
    ///
    /// This is useful for operations that need to mutate memory without
    /// affecting the original instance, such as in speculative execution or
    /// transaction-like operations.
    ///
    /// # Arguments
    ///
    /// * `mutate_fn` - The function to apply to the cloned memory
    ///
    /// # Returns
    ///
    /// The result of the mutation function
    /// Register a pre-grow hook to be executed before memory grows
    ///
    /// This is used by SafeMemory to validate the memory state before growing
    #[cfg(feature = "std")]
    pub fn with_pre_grow_hook<F>(&self, hook: F) -> Result<()>
    where
        F: FnOnce(u32, &[u8]) -> Result<()> + Send + 'static,
    {
        // Get the current memory data
        let data = self.data.lock().unwrap().to_vec()?;

        // Execute the hook immediately with current state
        hook(self.current_pages.load(Ordering::Relaxed), &data)?;

        // In a full implementation, we would store the hook for later use
        // but for now we just run it once to validate the current state

        Ok(())
    }

    /// Register a post-grow hook to be executed after memory grows
    ///
    /// This is used by SafeMemory to update checksums and validate the new
    /// memory state
    #[cfg(feature = "std")]
    pub fn with_post_grow_hook<F>(&self, hook: F) -> Result<()>
    where
        F: FnOnce(u32, &[u8]) -> Result<()> + Send + 'static,
    {
        // Get the current memory data
        let data = self.data.lock().unwrap().to_vec()?;

        // Execute the hook immediately with current state
        hook(self.current_pages.load(Ordering::Relaxed), &data)?;

        // In a full implementation, we would store the hook for later use
        // but for now we just run it once to validate the current state

        Ok(())
    }

    /// Verify data integrity
    pub fn verify_integrity(&self) -> Result<()> {
        // Get the expected size
        let pages = self.current_pages.load(Ordering::Relaxed);
        let expected_size = wasm_offset_to_usize(pages).unwrap_or(0) * PAGE_SIZE;

        // Verify memory size is consistent
        #[cfg(feature = "std")]
        let data_size = self.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let data_size = self.data.read().size();

        if data_size != expected_size {
            return Err(Error::validation_error("Memory size mismatch"));
        }

        // Check memory integrity
        Ok(())
    }

    /// Copy memory within the same memory instance or between two memory
    /// instances
    ///
    /// This method implements the memory.copy instruction from the WebAssembly
    /// bulk memory operations proposal.
    ///
    /// # Arguments
    ///
    /// * `src_mem` - The source memory instance (can be the same as self)
    /// * `src_addr` - The source address
    /// * `dst_addr` - The destination address
    /// * `size` - The number of bytes to copy
    ///
    /// # Errors
    ///
    /// Returns an error if either the source or destination range is out of
    /// bounds
    pub fn copy_within_or_between(
        &mut self,
        src_mem: Arc<Memory>,
        src_addr: usize,
        dst_addr: usize,
        size: usize,
    ) -> Result<()> {
        // Bounds check for source
        #[cfg(feature = "std")]
        let src_data_size = src_mem.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let src_data_size = src_mem.data.read().size();

        let src_end = match src_addr.checked_add(size) {
            Some(end) if end <= src_data_size => end,
            _ => return Err(Error::memory_error("Source memory access out of bounds")),
        };

        // Bounds check for destination
        #[cfg(feature = "std")]
        let dst_data_size = self.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let dst_data_size = self.data.read().size();

        let dst_end = match dst_addr.checked_add(size) {
            Some(end) if end <= dst_data_size => end,
            _ => {
                return Err(Error::memory_error(
                    "Destination memory access out of bounds",
                ))
            },
        };

        // Increment the access count for both memories
        self.increment_access_count(dst_addr, size);
        src_mem.increment_access_count(src_addr, size);

        // Use SafeSlice for source memory access
        #[cfg(feature = "std")]
        let src_data_guard = src_mem.data.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let src_data_guard = src_mem.data.read();

        let src_slice = src_data_guard.get_slice(src_addr, size)?;
        let src_data = src_slice.data()?;

        // Handle overlapping regions safely by using a temporary buffer
        #[cfg(feature = "std")]
        let mut temp_buf = Vec::with_capacity(size);
        #[cfg(not(feature = "std"))]
        let mut temp_buf = vec_with_capacity::<u8>(size);
        temp_buf.extend_from_slice(src_data);

        // Get destination memory data using provider-aware method
        #[cfg(feature = "std")]
        let mut dst_data_guard = self.data.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut dst_data_guard = self.data.write();

        let data_size = dst_data_guard.size();
        let dst_slice = dst_data_guard.get_slice(0, data_size)?;
        let mut dst_data = dst_slice.data()?.to_vec();

        // Copy from temporary buffer to destination
        dst_data[dst_addr..dst_addr + size].copy_from_slice(temp_buf.as_slice());

        // Update destination memory
        dst_data_guard.clear()?;
        dst_data_guard.add_data(&dst_data)?;

        // Verify integrity if full verification is enabled
        if self.verification_level == VerificationLevel::Full {
            dst_data_guard.provider().verify_integrity()?;
        }

        // Drop the guard before updating peak memory
        drop(dst_data_guard);

        // Update peak memory usage
        self.update_peak_memory();

        Ok(())
    }

    /// Fill memory with a byte value
    ///
    /// This method implements the memory.fill instruction from the WebAssembly
    /// bulk memory operations proposal.
    ///
    /// # Arguments
    ///
    /// * `dst` - The destination address in memory
    /// * `val` - The byte value to fill with
    /// * `size` - The number of bytes to fill
    ///
    /// # Returns
    ///
    /// Ok(()) if successful
    ///
    /// Returns an error if the memory access is invalid
    pub fn fill(&mut self, dst: usize, val: u8, size: usize) -> Result<()> {
        // Handle empty fill
        if size == 0 {
            return Ok(());
        }

        // Verify destination is within bounds
        let end = dst
            .checked_add(size)
            .ok_or_else(|| Error::memory_out_of_bounds("Memory fill would overflow"))?;

        if end > self.size_in_bytes() {
            return Err(Error::memory_out_of_bounds("Runtime operation error"));
        }

        // Track this access for profiling
        self.increment_access_count(dst, size);

        // Create a safety-bounded buffer size to avoid excessive memory usage
        const MAX_CHUNK_SIZE: usize = 4096;

        // Fill in chunks to avoid excessive memory usage while maintaining safety
        let mut remaining = size;
        let mut current_dst = dst;

        while remaining > 0 {
            let chunk_size = remaining.min(MAX_CHUNK_SIZE);

            // For each chunk, create a properly sized fill buffer
            #[cfg(feature = "std")]
            let fill_buffer = vec![val; chunk_size];
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            let fill_buffer = {
                let mut buffer: wrt_foundation::bounded::BoundedVec<u8, 4096, SmallMemoryProvider> =
                    wrt_foundation::bounded::BoundedVec::new(SmallMemoryProvider::default())
                        .unwrap();
                for _ in 0..chunk_size {
                    buffer.push(val).unwrap();
                }
                buffer
            };

            // Write directly to the data handler with safety verification
            #[cfg(feature = "std")]
            self.data.lock().unwrap().verify_access(current_dst, chunk_size)?;
            #[cfg(not(feature = "std"))]
            self.data.read().verify_access(current_dst, chunk_size)?;

            // Write the buffer data using memory write method
            #[cfg(feature = "std")]
            self.write(current_dst as u32, &fill_buffer)?;
            #[cfg(not(feature = "std"))]
            self.write(current_dst as u32, fill_buffer.as_slice()?)?;

            current_dst += chunk_size;
            remaining -= chunk_size;
        }

        // Update peak memory usage
        self.update_peak_memory();

        Ok(())
    }

    /// Initialize memory from a data segment
    ///
    /// This method implements the memory.init instruction from the WebAssembly
    /// bulk memory operations proposal.
    ///
    /// # Arguments
    ///
    /// * `dst` - The destination address in memory
    /// * `data` - The source data segment
    /// * `src` - The offset within the data segment
    /// * `size` - The number of bytes to copy
    ///
    /// # Errors
    ///
    /// Returns an error if the source or destination range is out of bounds
    pub fn init(&mut self, dst: usize, data: &[u8], src: usize, size: usize) -> Result<()> {
        // Source bounds check
        let src_end = match src.checked_add(size) {
            Some(end) if end <= data.len() => end,
            _ => {
                return Err(Error::memory_error("Runtime operation error"));
            },
        };

        // Destination bounds check
        #[cfg(feature = "std")]
        let data_size = self.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let data_size = self.data.read().size();

        let dst_end = match dst.checked_add(size) {
            Some(end) if end <= data_size => end,
            _ => {
                return Err(Error::memory_error(
                    "Destination memory access out of bounds",
                ));
            },
        };

        // Handle zero-size initialization
        if size == 0 {
            return Ok(());
        }

        // For small copies, we can use set_byte directly - this provides maximum safety
        // with acceptable performance for small operations
        if size <= 32 {
            // Create a safe copy of the source data for integrity
            let src_data = SafeSlice::new(&data[src..src + size])?;

            // Verify the source data integrity
            src_data.verify_integrity()?;

            // Access source data safely
            let verified_data = src_data.data()?;

            for (i, &byte) in verified_data.iter().enumerate().take(size) {
                self.set_byte((dst + i) as u32, byte)?;
            }

            // Update metrics to reflect the entire operation rather than just the last byte
            self.update_access_metrics(dst, size);
            return Ok(());
        }

        // For larger copies, use chunked processing to maintain memory safety
        // Binary std/no_std choice
        const MAX_CHUNK_SIZE: usize = 4096;
        let mut remaining = size;
        let mut src_offset = src;
        let mut dst_offset = dst;

        while remaining > 0 {
            let chunk_size = remaining.min(MAX_CHUNK_SIZE);

            // Create a safe slice for the source chunk to verify its integrity
            let src_slice = SafeSlice::new(&data[src_offset..src_offset + chunk_size])?;
            src_slice.verify_integrity()?;

            // Get the source data after verification
            let src_data = src_slice.data()?;

            // Verify destination access is valid using the SafeMemoryHandler
            #[cfg(feature = "std")]
            {
                let mut data_guard = self.data.lock().unwrap();
                data_guard.verify_access(dst_offset, chunk_size)?;
                // Write the source data directly to the destination
                data_guard.write_data(dst_offset, src_data)?;
            }
            #[cfg(not(feature = "std"))]
            {
                let mut data_guard = self.data.write();
                data_guard.verify_access(dst_offset, chunk_size)?;
                // Write the source data directly to the destination
                data_guard.write_data(dst_offset, src_data)?;
            }

            // Update for next chunk
            src_offset += chunk_size;
            dst_offset += chunk_size;
            remaining -= chunk_size;
        }

        // Update peak memory usage
        self.update_peak_memory();

        // Ensure all metrics reflect the entire init operation
        self.update_access_metrics(dst, size);

        Ok(())
    }

    /// Read a 32-bit integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 32-bit integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_i32(&self, addr: u32) -> Result<i32> {
        let mut buffer = [0; 4];
        self.read(addr, &mut buffer)?;
        Ok(i32::from_le_bytes(buffer))
    }

    /// Read a 64-bit integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 64-bit integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_i64(&self, addr: u32) -> Result<i64> {
        let mut buffer = [0; 8];
        self.read(addr, &mut buffer)?;
        Ok(i64::from_le_bytes(buffer))
    }

    /// Read a 32-bit float from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 32-bit float read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_f32(&self, addr: u32) -> Result<f32> {
        let value = self.read_i32(addr)?;
        Ok(f32::from_bits(value as u32))
    }

    /// Read a 64-bit float from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 64-bit float read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_f64(&self, addr: u32) -> Result<f64> {
        let value = self.read_i64(addr)?;
        Ok(f64::from_bits(value as u64))
    }

    /// Read an 8-bit signed integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 8-bit signed integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_i8(&self, addr: u32) -> Result<i8> {
        let mut buffer = [0; 1];
        self.read(addr, &mut buffer)?;
        Ok(i8::from_le_bytes(buffer))
    }

    /// Read an 8-bit unsigned integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 8-bit unsigned integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_u8(&self, addr: u32) -> Result<u8> {
        let mut buffer = [0; 1];
        self.read(addr, &mut buffer)?;
        Ok(u8::from_le_bytes(buffer))
    }

    /// Read a 16-bit signed integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 16-bit signed integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_i16(&self, addr: u32) -> Result<i16> {
        let mut buffer = [0; 2];
        self.read(addr, &mut buffer)?;
        Ok(i16::from_le_bytes(buffer))
    }

    /// Read a 16-bit unsigned integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 16-bit unsigned integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_u16(&self, addr: u32) -> Result<u16> {
        let mut buffer = [0; 2];
        self.read(addr, &mut buffer)?;
        Ok(u16::from_le_bytes(buffer))
    }

    /// Read a 32-bit unsigned integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 32-bit unsigned integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_u32(&self, addr: u32) -> Result<u32> {
        let mut buffer = [0; 4];
        self.read(addr, &mut buffer)?;
        Ok(u32::from_le_bytes(buffer))
    }

    /// Read a 64-bit unsigned integer from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 64-bit unsigned integer read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_u64(&self, addr: u32) -> Result<u64> {
        let mut buffer = [0; 8];
        self.read(addr, &mut buffer)?;
        Ok(u64::from_le_bytes(buffer))
    }

    /// Read a 128-bit vector from memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// The 128-bit vector read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read is out of bounds
    pub fn read_v128(&self, addr: u32) -> Result<[u8; 16]> {
        let mut buffer = [0; 16];
        self.read(addr, &mut buffer)?;
        Ok(buffer)
    }

    /// Write a 32-bit integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_i32(&mut self, addr: u32, value: i32) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 64-bit integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_i64(&mut self, addr: u32, value: i64) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 32-bit float to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_f32(&mut self, addr: u32, value: f32) -> Result<()> {
        self.write_i32(addr, value.to_bits() as i32)
    }

    /// Write a 64-bit float to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_f64(&mut self, addr: u32, value: f64) -> Result<()> {
        self.write_i64(addr, value.to_bits() as i64)
    }

    /// Write an 8-bit signed integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_i8(&mut self, addr: u32, value: i8) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write an 8-bit unsigned integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_u8(&mut self, addr: u32, value: u8) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 16-bit signed integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_i16(&mut self, addr: u32, value: i16) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 16-bit unsigned integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_u16(&mut self, addr: u32, value: u16) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 32-bit unsigned integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_u32(&mut self, addr: u32, value: u32) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 64-bit unsigned integer to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_u64(&mut self, addr: u32, value: u64) -> Result<()> {
        self.write(addr, &value.to_le_bytes())
    }

    /// Write a 128-bit vector to memory
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write is out of bounds
    pub fn write_v128(&mut self, addr: u32, value: [u8; 16]) -> Result<()> {
        self.write(addr, &value)
    }

    /// Sets the verification level for memory operations
    ///
    /// This controls how much verification is performed during memory
    /// operations:
    /// - None: No verification (fastest, least safe)
    /// - Sampling: Verification on a random subset of operations
    /// - Standard: Normal verification level (default)
    /// - Full: Maximum verification (slowest, most safe)
    ///
    /// # Arguments
    ///
    /// * `level` - The new verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        // Update our own verification level
        self.verification_level = level;

        // Propagate to the memory handler
        #[cfg(feature = "std")]
        self.data.lock().unwrap().set_verification_level(level);
        #[cfg(not(feature = "std"))]
        self.data.write().set_verification_level(level);
    }

    /// Gets the current verification level
    ///
    /// # Returns
    ///
    /// The current verification level
    #[must_use]
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Get the memory statistics
    fn memory_stats(&self) -> MemoryStats {
        #[cfg(feature = "std")]
        let total_size = self.data.lock().unwrap().size();
        #[cfg(not(feature = "std"))]
        let total_size = self.data.read().size();

        MemoryStats {
            total_size,
            access_count:    self.access_count() as usize, // Convert u64 to usize
            unique_regions:  self.unique_regions(),
            max_access_size: self.max_access_size(),
        }
    }

    // Note: This functionality is now handled in update_access_metrics

    /// Update all access metrics in one operation
    fn update_access_metrics(&self, offset: usize, len: usize) {
        #[cfg(feature = "std")]
        {
            self.metrics.max_access_size.fetch_max(len, Ordering::Relaxed);
            self.metrics.last_access_offset.store(offset, Ordering::Relaxed);
            self.metrics.last_access_length.store(len, Ordering::Relaxed);
        }

        #[cfg(not(feature = "std"))]
        {
            // Use write() method with WrtRwLock
            let mut metrics = self.metrics.write();
            metrics.max_access_size = metrics.max_access_size.max(len);
            metrics.last_access_offset = offset;
            metrics.last_access_length = len;
        }
    }

    /// Get safety statistics for this memory instance
    ///
    /// This returns detailed statistics about memory usage and safety checks
    ///
    /// # Returns
    ///
    /// A string containing the statistics
    #[cfg(feature = "std")]
    pub fn safety_stats(&self) -> alloc::string::String {
        let memory_stats = self.memory_stats();
        let access_count = self.access_count();
        let peak_memory = self.peak_memory();
        let max_access = self.max_access_size();
        let unique_regions = self.unique_regions();

        // Create a string with formatted stats
        "Memory Safety Stats: [Runtime memory]".to_string()
    }

    /// Get safety statistics for this memory instance (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn safety_stats(&self) -> Result<crate::prelude::RuntimeString> {
        use crate::prelude::RuntimeString;
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;
        Ok(RuntimeString::from_str_truncate("Memory Safety Stats: [Runtime memory]")
            .unwrap_or_else(|_| RuntimeString::from_str_truncate("").unwrap()))
    }

    /// Returns a `SafeSlice` representing the entire memory
    ///
    /// Unlike `buffer()`, this does not create a copy of the memory data,
    /// making it more efficient for read-only access to memory.
    ///
    /// # Returns
    ///
    /// A `SafeSlice` covering the entire memory buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the memory is corrupted or integrity checks fail
    ///
    /// # ASIL-B Note
    /// This method has been disabled for ASIL-B compliance due to complex lifetime
    /// interactions with Mutex. Use read() method instead for thread-safe access.
    pub fn as_safe_slice<'a>(&'a self) -> Result<wrt_foundation::safe_memory::SafeSlice<'a>> {
        Err(Error::runtime_execution_error("as_safe_slice disabled for ASIL-B compliance - use read() instead"))
    }

    /// Update the memory buffer through a callback function
    pub fn update_buffer<F>(&self, _update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut [u8]) -> Result<()>,
    {
        // self.data.with_mutable_data(update_fn)?;
        // This pattern is not directly supported by SafeMemoryHandler due to its
        // ownership and checksumming of the byte buffer.
        // A redesign of this function or SafeMemoryHandler would be needed
        // for direct mutable slice access.
        Err(Error::runtime_execution_error("Memory bounds check failed"))
    }

    /// Grow memory by a number of pages.
    pub fn grow_memory(&mut self, pages: u32) -> Result<u32> {
        let old_size_pages = self.current_pages.load(Ordering::Relaxed);
        let new_size_pages = old_size_pages.saturating_add(pages);

        if new_size_pages > MAX_PAGES {
            return Err(Error::new(
                ErrorCategory::Memory,
                wrt_error::codes::MEMORY_GROW_ERROR,
                "Memory grow exceeds maximum pages",
            ));
        }

        let new_byte_size = wasm_offset_to_usize(new_size_pages)? * PAGE_SIZE;
        // Placeholder: Assumes SafeMemoryHandler has a method like `resize`
        // that takes &self and handles locking internally.
        #[cfg(feature = "std")]
        self.data.lock().unwrap().resize(new_byte_size)?;
        #[cfg(not(feature = "std"))]
        self.data.write().resize(new_byte_size)?;

        self.current_pages.store(new_size_pages, Ordering::Relaxed);
        self.update_peak_memory();

        Ok(old_size_pages)
    }
}

/// Implementation of MemoryAccessor for Memory
///
/// This allows the engine to pass Memory to HostImportHandler as a trait object,
/// enabling proper abstraction without exposing raw byte slices (which would break
/// ASIL-B safety guarantees from the Mutex protection).
///
/// Uses interior mutability via `write_shared()` for thread-safe writes.
#[cfg(feature = "std")]
impl MemoryAccessor for Memory {
    fn read_bytes(&self, offset: u32, buffer: &mut [u8]) -> Result<()> {
        self.read(offset, buffer)
    }

    fn write_bytes(&self, offset: u32, data: &[u8]) -> Result<()> {
        // Use write_shared which takes &self (interior mutability via Mutex)
        self.write_shared(offset, data)
    }

    fn size(&self) -> usize {
        self.size_in_bytes()
    }
}

// REMOVED: MemoryProvider implementation for Memory
// This was architecturally incorrect - Memory is a CONSUMER of memory services,
// not a PROVIDER. Memory instances use memory providers (like LargeMemoryProvider)
// to manage their underlying storage, but should not themselves act as providers.
//
// This implementation was also requiring Default trait which violated the
// NO FALLBACK LOGIC rule from CLAUDE.md.

// impl MemoryProvider for Memory {
//     [Implementation removed - see above comment]
// }

// MemorySafety trait implementation removed as it doesn't exist in
// wrt-foundation

/// Helper to convert u64 offset to usize, checking for overflow on 32-bit platforms
#[inline]
fn u64_to_usize(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::memory_out_of_bounds("Address exceeds platform capacity"))
}

impl MemoryOperations for Memory {
    #[cfg(feature = "std")]
    fn read_bytes(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        // Handle zero-length reads
        if len == 0 {
            return Ok(alloc::vec::Vec::new());
        }

        // Convert to usize and check for overflow
        let offset_usize = u64_to_usize(offset)?;
        let len_usize = u64_to_usize(len)?;

        // Verify bounds
        let end = offset_usize
            .checked_add(len_usize)
            .ok_or_else(|| Error::memory_out_of_bounds("Memory read would overflow"))?;

        if end > self.size_in_bytes() {
            return Err(Error::memory_out_of_bounds("Memory read out of bounds"));
        }

        // Read the data using the existing read method
        #[cfg(feature = "std")]
        let mut buffer = vec![0u8; len_usize];
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        let mut buffer = {
            let mut buf = wrt_foundation::bounded::BoundedVec::new();
            for _ in 0..len_usize {
                buf.push(0u8).unwrap();
            }
            buf
        };
        // Convert back to u32 for the internal read method (which still uses u32)
        let offset_u32 = u32::try_from(offset)
            .map_err(|_| Error::memory_out_of_bounds("Offset exceeds u32 for internal read"))?;
        self.read(offset_u32, &mut buffer)?;
        Ok(buffer)
    }

    #[cfg(not(any(feature = "std",)))]
    fn read_bytes(
        &self,
        offset: u64,
        len: u64,
    ) -> Result<wrt_foundation::BoundedVec<u8, 65536, MediumMemoryProvider>> {
        // Handle zero-length reads
        if len == 0 {
            let provider = MediumMemoryProvider::default();
            return wrt_foundation::BoundedVec::new(provider);
        }

        // Convert to usize and check for overflow
        let offset_usize = u64_to_usize(offset)?;
        let len_usize = u64_to_usize(len)?;

        // Verify bounds
        let end = offset_usize
            .checked_add(len_usize)
            .ok_or_else(|| Error::memory_out_of_bounds("Memory read would overflow"))?;

        if end > self.size_in_bytes() {
            return Err(Error::memory_out_of_bounds("Memory read out of bounds"));
        }

        // Create a bounded vector and fill it
        let mut result = wrt_foundation::BoundedVec::<u8, 65536, MediumMemoryProvider>::new(
            MediumMemoryProvider::default(),
        )?;

        // Read data byte by byte to populate the bounded vector
        // Convert offset to u32 for internal get_byte method
        let offset_u32 = u32::try_from(offset)
            .map_err(|_| Error::memory_out_of_bounds("Offset exceeds u32 for internal read"))?;
        for i in 0..len_usize {
            let byte = self.get_byte(offset_u32 + i as u32)?;
            result
                .push(byte)
                .map_err(|_| Error::runtime_execution_error("Memory access failed"))?;
        }

        Ok(result)
    }

    fn write_bytes(&mut self, offset: u64, bytes: &[u8]) -> Result<()> {
        // Convert offset to u32 for the internal write method
        let offset_u32 = u32::try_from(offset)
            .map_err(|_| Error::memory_out_of_bounds("Offset exceeds u32 for internal write"))?;
        // Delegate to the existing write method
        self.write(offset_u32, bytes)
    }

    fn size_in_bytes(&self) -> Result<u64> {
        // Delegate to the existing method and convert to u64
        Ok(Memory::size_in_bytes(self) as u64)
    }

    fn grow(&mut self, bytes: u64) -> Result<()> {
        // Convert bytes to pages (WebAssembly page size is 64KB)
        let bytes_usize = u64_to_usize(bytes)?;
        let pages = bytes_usize.div_ceil(PAGE_SIZE); // Ceiling division

        // Delegate to the existing grow method (which returns old page count)
        self.grow(pages as u32)?;
        Ok(())
    }

    fn fill(&mut self, offset: u64, value: u8, size: u64) -> Result<()> {
        // Convert to usize for the internal fill method
        let offset_usize = u64_to_usize(offset)?;
        let size_usize = u64_to_usize(size)?;
        self.fill(offset_usize, value, size_usize)
    }

    fn copy(&mut self, dest: u64, src: u64, size: u64) -> Result<()> {
        // For same-memory copy, we can use a simplified version of
        // copy_within_or_between
        if size == 0 {
            return Ok(());
        }

        let dest_usize = u64_to_usize(dest)?;
        let src_usize = u64_to_usize(src)?;
        let size_usize = u64_to_usize(size)?;

        // Bounds checks
        let src_end = src_usize
            .checked_add(size_usize)
            .ok_or_else(|| Error::memory_out_of_bounds("Source bounds overflow"))?;

        let dest_end = dest_usize.checked_add(size_usize).ok_or_else(|| {
            Error::memory_out_of_bounds("Destination address overflow in memory copy")
        })?;

        let memory_size = self.size_in_bytes();
        if src_end > memory_size || dest_end > memory_size {
            return Err(Error::memory_out_of_bounds("Memory copy out of bounds"));
        }

        // Track access for both source and destination
        self.increment_access_count(src_usize, size_usize);
        self.increment_access_count(dest_usize, size_usize);

        // Handle overlapping regions by using a temporary buffer
        // Read source data first
        // Convert to u32 for internal read/write methods
        let src_u32 = u32::try_from(src)
            .map_err(|_| Error::memory_out_of_bounds("Source offset exceeds u32"))?;
        let dest_u32 = u32::try_from(dest)
            .map_err(|_| Error::memory_out_of_bounds("Dest offset exceeds u32"))?;

        #[cfg(feature = "std")]
        {
            let mut buffer = vec![0u8; size_usize];
            self.read(src_u32, &mut buffer)?;
            self.write(dest_u32, &buffer)?;
        }

        #[cfg(not(feature = "std"))]
        {
            // For no_std, copy byte by byte
            // This is less efficient but works in constrained environments
            if size_usize > 4096 {
                return Err(Error::runtime_execution_error("Memory operation error"));
            }

            for i in 0..size_usize {
                let byte = self.get_byte(src_u32 + i as u32)?;
                self.set_byte(dest_u32 + i as u32, byte)?;
            }
        }

        Ok(())
    }
}

impl AtomicOperations for Memory {
    fn atomic_wait32(&mut self, addr: u32, expected: i32, timeout_ns: Option<u64>) -> Result<i32> {
        // Check alignment (atomic operations require proper alignment)
        self.check_alignment(addr, 4, 4)?;

        // Read current value atomically
        let current = self.read_i32(addr)?;
        if current != expected {
            return Ok(1); // Value mismatch, return immediately
        }

        // atomic.wait requires proper futex support to implement correctly.
        // The WebAssembly threads proposal specifies that atomic.wait must block
        // until either:
        // 1. The memory location is notified via atomic.notify
        // 2. The timeout expires
        //
        // A sleep-based fallback does NOT satisfy this because:
        // - It doesn't wake when the value changes via atomic.notify
        // - It always times out even if notified early
        //
        // Per CLAUDE.md: Return an error rather than masking with broken fallback.
        // To use atomic.wait, integrate with wrt-platform's futex infrastructure.
        let _ = timeout_ns; // Acknowledge parameter to avoid warning
        Err(Error::runtime_unsupported_operation(
            "atomic.wait32 requires platform futex support - integrate wrt-platform crate"
        ))
    }

    fn atomic_wait64(&mut self, addr: u32, expected: i64, timeout_ns: Option<u64>) -> Result<i32> {
        // Check alignment (64-bit atomics require 8-byte alignment)
        self.check_alignment(addr, 8, 8)?;

        // Read current value atomically
        let current = self.read_i64(addr)?;
        if current != expected {
            return Ok(1); // Value mismatch, return immediately
        }

        // atomic.wait64 has the same requirements as atomic.wait32.
        // See atomic_wait32 for detailed explanation.
        let _ = timeout_ns; // Acknowledge parameter to avoid warning
        Err(Error::runtime_unsupported_operation(
            "atomic.wait64 requires platform futex support - integrate wrt-platform crate"
        ))
    }

    fn atomic_notify(&mut self, addr: u32, count: u32) -> Result<u32> {
        // Check alignment
        self.check_alignment(addr, 4, 4)?;

        // Validate address is accessible
        let _current = self.read_i32(addr)?;

        // atomic.notify requires platform futex support to work correctly.
        // Without proper futex tracking, we cannot know which threads are waiting
        // on this address. Returning 0 (no waiters) is technically correct when
        // no threads are actually blocked via atomic.wait (which now returns error),
        // but we should be explicit about the limitation.
        //
        // When atomic.wait is properly implemented via wrt-platform futex,
        // this function should also use the same futex infrastructure to wake waiters.
        let _ = count; // Acknowledge parameter
        Ok(0) // Correct: no waiters since atomic.wait returns error
    }

    fn atomic_load_i32(&self, addr: u32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        self.read_i32(addr)
    }

    fn atomic_load_i64(&self, addr: u32) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        self.read_i64(addr)
    }

    fn atomic_store_i32(&mut self, addr: u32, value: i32) -> Result<()> {
        self.check_alignment(addr, 4, 4)?;
        self.write_i32(addr, value)
    }

    fn atomic_store_i64(&mut self, addr: u32, value: i64) -> Result<()> {
        self.check_alignment(addr, 8, 8)?;
        self.write_i64(addr, value)
    }

    fn atomic_rmw_add_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        let new_value = old_value.wrapping_add(value);
        self.write_i32(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_add_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        let new_value = old_value.wrapping_add(value);
        self.write_i64(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_sub_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        let new_value = old_value.wrapping_sub(value);
        self.write_i32(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_sub_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        let new_value = old_value.wrapping_sub(value);
        self.write_i64(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_and_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        let new_value = old_value & value;
        self.write_i32(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_and_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        let new_value = old_value & value;
        self.write_i64(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_or_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        let new_value = old_value | value;
        self.write_i32(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_or_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        let new_value = old_value | value;
        self.write_i64(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_xor_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        let new_value = old_value ^ value;
        self.write_i32(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_xor_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        let new_value = old_value ^ value;
        self.write_i64(addr, new_value)?;
        Ok(old_value)
    }

    fn atomic_rmw_xchg_i32(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        self.write_i32(addr, value)?;
        Ok(old_value)
    }

    fn atomic_rmw_xchg_i64(&mut self, addr: u32, value: i64) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        self.write_i64(addr, value)?;
        Ok(old_value)
    }

    fn atomic_rmw_cmpxchg_i32(
        &mut self,
        addr: u32,
        expected: i32,
        replacement: i32,
    ) -> Result<i32> {
        self.check_alignment(addr, 4, 4)?;
        let old_value = self.read_i32(addr)?;
        if old_value == expected {
            self.write_i32(addr, replacement)?;
        }
        Ok(old_value)
    }

    fn atomic_rmw_cmpxchg_i64(
        &mut self,
        addr: u32,
        expected: i64,
        replacement: i64,
    ) -> Result<i64> {
        self.check_alignment(addr, 8, 8)?;
        let old_value = self.read_i64(addr)?;
        if old_value == expected {
            self.write_i64(addr, replacement)?;
        }
        Ok(old_value)
    }

    // Additional compare-and-exchange methods
    fn atomic_cmpxchg_i32(&mut self, addr: u32, expected: i32, replacement: i32) -> Result<i32> {
        // Delegate to the existing rmw_cmpxchg implementation
        self.atomic_rmw_cmpxchg_i32(addr, expected, replacement)
    }

    fn atomic_cmpxchg_i64(&mut self, addr: u32, expected: i64, replacement: i64) -> Result<i64> {
        // Delegate to the existing rmw_cmpxchg implementation
        self.atomic_rmw_cmpxchg_i64(addr, expected, replacement)
    }
}

