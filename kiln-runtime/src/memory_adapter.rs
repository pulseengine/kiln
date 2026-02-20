//! Memory adapter for wrt-runtime
//!
//! This adapter provides safe, bounded memory access
//! with integrated memory safety features for WebAssembly memory instances.

// Use our prelude for consistent imports
// alloc is imported in lib.rs with proper feature gates

// Import Arc from the correct location to match ArcMemoryExt
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::format;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Arc;
// Import format! macro for string formatting
#[cfg(feature = "std")]
use std::format;
#[cfg(feature = "std")]
use alloc::sync::Arc;

use crate::{
    memory::Memory,
    prelude::*,
};

/// Invalid offset error code
const INVALID_OFFSET: u16 = 4006;
/// Size too large error code  
const SIZE_TOO_LARGE: u16 = 4007;

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
            "Size too large for WebAssembly u32",
        )
    })
}

/// Memory adapter interface for working with memory
pub trait MemoryAdapter: Debug + Send + Sync {
    /// Get the memory backing this adapter
    fn memory(&self) -> Arc<Memory>;

    /// Read bytes from memory at the given offset
    fn read_exact(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<BoundedVec<u8, 65_536, StdMemoryProvider>>;

    /// Write bytes to memory at the given offset
    fn write_all(&self, offset: u32, bytes: &[u8]) -> Result<()>;

    /// Get the size of the memory in pages
    fn size(&self) -> Result<u32>;

    /// Grow the memory by the given number of pages
    fn grow(&self, pages: u32) -> Result<u32>;

    /// Get the number of bytes in the memory
    fn byte_size(&self) -> Result<usize>;

    /// Check if a range is valid for the memory
    fn check_range(&self, offset: u32, size: u32) -> Result<()>;

    /// Borrow a slice of memory with integrity verification
    fn borrow_slice(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<BoundedVec<u8, 65_536, StdMemoryProvider>>;
}

/// Safe memory adapter implementation
#[derive(Debug)]
pub struct SafeMemoryAdapter {
    /// The underlying memory
    memory:   Arc<Memory>,
    /// The memory provider for safety checks
    provider: StdMemoryProvider,
}

/// Standard memory provider implementation
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StdMemoryProvider {
    /// Verification level for memory safety checks
    verification_level: VerificationLevel,
}

impl wrt_foundation::MemoryProvider for StdMemoryProvider {
    type Allocator = Self;

    fn borrow_slice(
        &self,
        _offset: usize,
        _len: usize,
    ) -> Result<wrt_foundation::safe_memory::Slice<'_>> {
        // StdMemoryProvider doesn't manage its own memory buffer
        // It's used as a provider for BoundedVec operations
        // Return an error indicating this operation is not supported
        Err(wrt_error::Error::runtime_execution_error(
            "Memory read not supported for StdMemoryProvider ",
        ))
    }

    fn write_data(&mut self, _offset: usize, _data: &[u8]) -> Result<()> {
        // For StdMemoryProvider, this is a placeholder
        Ok(())
    }

    fn verify_access(&self, _offset: usize, _len: usize) -> Result<()> {
        // For StdMemoryProvider, this is a placeholder
        Ok(())
    }

    fn size(&self) -> usize {
        0
    }

    fn capacity(&self) -> usize {
        // For std mode, we can use large capacities
        1024 * 1024 // 1MB
    }

    fn verify_integrity(&self) -> Result<()> {
        Ok(())
    }

    fn set_verification_level(&mut self, level: wrt_foundation::verification::VerificationLevel) {
        self.verification_level = level;
    }

    fn verification_level(&self) -> wrt_foundation::verification::VerificationLevel {
        self.verification_level
    }

    fn memory_stats(&self) -> wrt_foundation::MemoryStats {
        wrt_foundation::MemoryStats::default()
    }

    fn get_slice_mut(
        &mut self,
        _offset: usize,
        _len: usize,
    ) -> Result<wrt_foundation::safe_memory::SliceMut<'_>> {
        Err(wrt_error::Error::new(
            wrt_error::ErrorCategory::Memory,
            wrt_error::codes::NOT_IMPLEMENTED,
            "Not implemented",
        ))
    }

    fn copy_within(&mut self, _src: usize, _dst: usize, _len: usize) -> Result<()> {
        Ok(())
    }

    fn ensure_used_up_to(&mut self, _offset: usize) -> Result<()> {
        Ok(())
    }

    fn acquire_memory(&self, _layout: core::alloc::Layout) -> Result<*mut u8> {
        Err(wrt_error::Error::runtime_execution_error(
            "Memory acquisition unsupported ",
        ))
    }

    fn release_memory(&self, _ptr: *mut u8, _layout: core::alloc::Layout) -> Result<()> {
        Ok(())
    }

    fn get_allocator(&self) -> &Self::Allocator {
        self
    }

    fn new_handler(&self) -> Result<wrt_foundation::safe_memory::SafeMemoryHandler<Self>>
    where
        Self: Clone,
    {
        Ok(wrt_foundation::safe_memory::SafeMemoryHandler::new(
            self.clone(),
        ))
    }
}

impl wrt_foundation::safe_memory::Allocator for StdMemoryProvider {
    fn allocate(&self, _layout: core::alloc::Layout) -> Result<*mut u8> {
        Err(wrt_error::Error::new(
            wrt_error::ErrorCategory::Memory,
            wrt_error::codes::NOT_IMPLEMENTED,
            "Not implemented",
        ))
    }

    fn deallocate(&self, _ptr: *mut u8, _layout: core::alloc::Layout) -> Result<()> {
        Ok(())
    }
}

impl StdMemoryProvider {
    /// Create a new standard memory provider
    pub fn new(_data: &[u8]) -> Self {
        Self {
            verification_level: VerificationLevel::Standard,
        }
    }

    /// Get the current verification level
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Set the verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Create a safe slice of memory with verification
    pub fn create_safe_slice(
        &self,
        buffer: &[u8],
        offset: usize,
        len: usize,
    ) -> Result<BoundedVec<u8, 65_536, StdMemoryProvider>> {
        if offset + len > buffer.len() {
            return Err(Error::memory_error("Memory access out of bounds"));
        }

        // Instead of returning a reference, copy the data into a BoundedVec
        let mut bounded_vec =
            BoundedVec::with_verification_level(self.clone(), self.verification_level())?;

        for i in offset..(offset + len) {
            bounded_vec
                .push(buffer[i])
                .map_err(|_| Error::memory_error("Failed to push byte to bounded vector"))?;
        }

        Ok(bounded_vec)
    }
}

impl SafeMemoryAdapter {
    /// Create a new memory adapter with the given memory type
    pub fn new(memory_type: CoreMemoryType) -> Result<Arc<SafeMemoryAdapter>> {
        let memory = Memory::new(memory_type)?;

        // Create a new adapter with the memory
        let arc_memory: Arc<Memory> = Arc::from(memory);  // Convert Box<Memory> to Arc<Memory>

        #[cfg(feature = "std")]
        let provider = {
            let data = arc_memory.buffer()?;
            StdMemoryProvider::new(data.as_slice())
        };

        #[cfg(not(feature = "std"))]
        let provider = StdMemoryProvider::default();

        // Return a Memory adapter
        let adapter = SafeMemoryAdapter {
            memory: arc_memory,
            provider,
        };

        Ok(Arc::new(adapter))
    }

    /// Verify memory safety
    pub fn verify_memory_safety(&self) -> Result<()> {
        Ok(())
    }
}

// Implement the MemorySafety trait for SafeMemoryAdapter
// MemorySafety trait implementation removed as it doesn't exist in
// wrt-foundation

// Implement the MemoryAdapter trait for SafeMemoryAdapter
impl MemoryAdapter for SafeMemoryAdapter {
    fn memory(&self) -> Arc<Memory> {
        self.memory.clone()
    }

    fn read_exact(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<BoundedVec<u8, 65_536, StdMemoryProvider>> {
        // Check that the range is valid
        self.check_range(offset, len)?;

        // Get a safe slice instead of buffer
        let safe_slice = self.memory.get_safe_slice(offset, len as usize)?;
        let data = safe_slice.data()?;

        // Create a new BoundedVec with the data
        let mut bounded_vec = BoundedVec::with_verification_level(
            self.provider.clone(),
            self.provider.verification_level(),
        )?;

        // Copy the data from the slice into the bounded vector
        for &byte in data {
            bounded_vec
                .push(byte)
                .map_err(|_| Error::memory_error("Failed to push byte to bounded vector"))?;
        }

        Ok(bounded_vec)
    }

    fn write_all(&self, offset: u32, bytes: &[u8]) -> Result<()> {
        #[cfg(feature = "tracing")]
        use wrt_foundation::tracing::{debug, trace};

        // Check that the range is valid
        self.check_range(offset, bytes.len() as u32)?;

        // Use the thread-safe write_shared method designed for Arc<Memory>
        #[cfg(feature = "tracing")]
        trace!("Writing {} bytes to memory at offset {:#x}", bytes.len(), offset);
        self.memory.write_shared(offset, bytes)?;

        #[cfg(feature = "tracing")]
        debug!("Successfully wrote {} bytes to memory at offset {:#x}", bytes.len(), offset);
        Ok(())
    }

    fn size(&self) -> Result<u32> {
        // Wrap the direct u32 return in a Result
        Ok(self.memory.size())
    }

    fn grow(&self, pages: u32) -> Result<u32> {
        #[cfg(feature = "tracing")]
        use wrt_foundation::tracing::warn;

        // Get the current size
        let result = self.memory.size();

        // TODO: The grow_shared method requires &mut self, but we only have Arc<Memory>
        // This requires either:
        // 1. Adding interior mutability to Memory for the grow operation
        // 2. Refactoring to use Arc<Mutex<Memory>> or Arc<RwLock<Memory>>
        // 3. Adding a new grow method that takes &self and uses interior mutability
        #[cfg(feature = "tracing")]
        warn!("Memory grow operation requested but not yet implemented for Arc<Memory> (needs {} pages)", pages);
        Err(Error::runtime_execution_error(
            "Grow operation not yet implemented for Arc<Memory> - requires interior mutability",
        ))
    }

    fn byte_size(&self) -> Result<usize> {
        // Convert WebAssembly page count to byte size safely
        let pages = self.memory.size();
        let page_size_bytes = wasm_offset_to_usize(pages)? * 65_536;
        Ok(page_size_bytes)
    }

    fn check_range(&self, offset: u32, size: u32) -> Result<()> {
        let mem_size = self.byte_size()?;
        let end_offset = wasm_offset_to_usize(offset)? + wasm_offset_to_usize(size)?;

        if end_offset > mem_size {
            Err(Error::memory_error("Memory access out of bounds"))
        } else {
            Ok(())
        }
    }

    // Change the return type to BoundedVec instead of SafeSlice to avoid lifetime
    // issues
    fn borrow_slice(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<BoundedVec<u8, 65_536, StdMemoryProvider>> {
        // Check that the range is valid
        self.check_range(offset, len)?;

        // Get a safe slice instead of buffer
        let safe_slice = self.memory.get_safe_slice(offset, len as usize)?;
        let data = safe_slice.data()?;

        // Create a new BoundedVec with the copied data
        let mut bounded_vec = BoundedVec::with_verification_level(
            self.provider.clone(),
            self.provider.verification_level(),
        )?;
        for &byte in data {
            bounded_vec
                .push(byte)
                .map_err(|_| Error::memory_error("Failed to push byte to bounded vector"))?;
        }
        Ok(bounded_vec)
    }
}
