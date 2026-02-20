// WRT - wrt-foundation
// Module: Atomic Memory Operations
// SW-REQ-ID: REQ_MEM_SAFETY_004

// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides atomic memory operations with integrated checksumming for stronger
//! memory safety guarantees, protecting against bit flips between write and
//! checksum operations.

// Import WrtMutex from wrt-sync
use wrt_sync::mutex::WrtMutex;

#[cfg(feature = "std")]
use crate::prelude::Vec;
use crate::{
    operations::{
        record_global_operation,
        Type as OperationType,
    },
    prelude::{
        Clone,
        Debug,
        Eq,
        PartialEq,
        Result,
        Sized,
    },
    safe_memory::{
        Provider,
        SafeMemoryHandler,
    },
    verification::VerificationLevel,
};

/// An atomic memory operation handler that ensures write operations and
/// checksum calculations are performed atomically.
///
/// This structure uses a mutex to guarantee that no bit flips can occur
/// between write operations and checksum calculations, providing stronger
/// integrity guarantees than the standard `SafeMemoryHandler`.
#[derive(Debug)]
pub struct AtomicMemoryOps<P: Provider> {
    /// The underlying memory handler wrapped in a mutex for atomic operations
    handler:            WrtMutex<SafeMemoryHandler<P>>,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
}

impl<P: Provider + Clone> Clone for AtomicMemoryOps<P> {
    fn clone(&self) -> Self {
        Self {
            handler:            WrtMutex::new(self.handler.lock().clone()),
            verification_level: self.verification_level,
        }
    }
}

impl<P: Provider + PartialEq> PartialEq for AtomicMemoryOps<P> {
    fn eq(&self, other: &Self) -> bool {
        // Compare the underlying handlers (requires locking both)
        let self_handler = self.handler.lock();
        let other_handler = other.handler.lock();
        *self_handler == *other_handler && self.verification_level == other.verification_level
    }
}

impl<P: Provider + Eq> Eq for AtomicMemoryOps<P> {}

impl<P: Provider> AtomicMemoryOps<P> {
    /// Creates a new `AtomicMemoryOps` with the provided memory handler.
    ///
    /// This wraps the handler in a mutex to ensure atomic operations.
    pub fn new(handler: SafeMemoryHandler<P>) -> Self {
        let verification_level = handler.verification_level();
        Self {
            handler: WrtMutex::new(handler),
            verification_level,
        }
    }

    /// Creates a new `AtomicMemoryOps` with the provided provider.
    ///
    /// This creates a new `SafeMemoryHandler` from the provider and wraps it
    /// in a mutex to ensure atomic operations.
    pub fn from_provider(provider: P) -> Result<Self>
    where
        P: Sized + Clone,
    {
        let handler = SafeMemoryHandler::new(provider);
        let verification_level = handler.verification_level();
        Ok(Self {
            handler: WrtMutex::new(handler),
            verification_level,
        })
    }

    /// Reads data from memory with safety guarantees and atomic access.
    ///
    /// This operation acquires the mutex to ensure atomic access.
    /// Returns owned data instead of a borrowed slice to avoid lifetime
    /// issues with the lock guard.
    ///
    /// # Errors
    ///
    /// Returns an error if the memory access is invalid or if the
    /// integrity verification fails.
    #[cfg(feature = "std")]
    pub fn read_data(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        // Lock the handler for atomic access
        let handler = self.handler.lock();
        record_global_operation(OperationType::MemoryRead, self.verification_level);

        // Get the slice and copy the data to avoid lifetime issues
        let slice = handler.borrow_slice(offset, len)?;
        Ok(slice.data()?.to_vec())
    }

    /// Writes data to the memory at a given offset and updates the checksum
    /// atomically.
    ///
    /// This performs the write operation and checksum update in a single atomic
    /// operation, ensuring that no bit flips can occur between these steps.
    ///
    /// # Safety Features
    ///
    /// - Acquires a mutex lock to ensure exclusive access
    /// - Performs write operation while holding the lock
    /// - Updates checksums while still holding the lock
    /// - Uses memory barriers to ensure proper ordering
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails or if integrity
    /// verification fails.
    pub fn atomic_write_with_checksum(&self, offset: usize, data: &[u8]) -> Result<()> {
        // Lock the handler for atomic access with Acquire ordering
        let mut handler = self.handler.lock();
        record_global_operation(OperationType::MemoryWrite, self.verification_level);

        // Verify that the access is valid
        handler.verify_access(offset, data.len())?;

        // Get a mutable slice that covers the region we're about to write to
        let mut slice = handler.provider_mut().get_slice_mut(offset, data.len())?;

        // Get a mutable reference to the underlying data
        let slice_data = slice.data_mut()?;

        // Copy the data while holding the lock
        slice_data.copy_from_slice(data);

        // Update the checksum while still holding the lock
        // This ensures no bit flips can occur between write and checksum update
        slice.update_checksum();

        // Lock is released automatically when handler goes out of scope

        Ok(())
    }

    /// Copies data within the memory provider from a source offset to a
    /// destination offset atomically with checksum updates.
    ///
    /// # Errors
    ///
    /// Returns an error if the copy operation fails or if integrity
    /// verification fails.
    pub fn atomic_copy_within(
        &self,
        src_offset: usize,
        dst_offset: usize,
        len: usize,
    ) -> Result<()> {
        // Lock the handler for atomic access
        let mut handler = self.handler.lock();
        record_global_operation(OperationType::MemoryCopy, self.verification_level);

        // Verify that the source access is valid
        handler.verify_access(src_offset, len)?;

        // Verify that the destination access is valid
        handler.verify_access(dst_offset, len)?;

        // Handle copying in chunks to avoid overlapping borrows
        let mut remaining = len;
        let mut src_pos = 0;
        let mut dst_pos = 0;

        while remaining > 0 {
            let chunk_size = core::cmp::min(remaining, 256); // Use fixed buffer size

            // Read source data in a scoped block to drop the immutable borrow
            let mut buffer = [0u8; 256];
            {
                let source_slice = handler.borrow_slice(src_offset + src_pos, chunk_size)?;
                let source_data = source_slice.data()?;
                buffer[..chunk_size].copy_from_slice(&source_data[..chunk_size]);
            } // source_slice is dropped here, releasing immutable borrow

            // Write to the destination atomically with checksum update
            let mut dst_slice =
                handler.provider_mut().get_slice_mut(dst_offset + dst_pos, chunk_size)?;
            let dst_data = dst_slice.data_mut()?;
            dst_data.copy_from_slice(&buffer[..chunk_size]);
            dst_slice.update_checksum();

            remaining -= chunk_size;
            src_pos += chunk_size;
            dst_pos += chunk_size;
        }

        Ok(())
    }

    /// Gets the current verification level for this memory handler.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets a new verification level for this memory handler.
    ///
    /// This updates both the handler's internal level and the level in the
    /// underlying provider.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        let mut handler = self.handler.lock();
        handler.set_verification_level(level);
    }

    /// Gets the underlying memory provider's size.
    pub fn size(&self) -> usize {
        let handler = self.handler.lock();
        handler.size()
    }

    /// Gets the underlying memory provider's capacity.
    pub fn capacity(&self) -> usize {
        let handler = self.handler.lock();
        handler.capacity()
    }

    /// Gets a mutable reference to the underlying memory handler.
    ///
    /// # Safety
    ///
    /// This bypasses the atomic safety mechanisms provided by this struct.
    /// Only use this method when you know that no concurrent access to the
    /// memory will occur, or when you need to perform multiple operations
    /// atomically within a single critical section.
    pub fn get_handler_mut(&mut self) -> &mut WrtMutex<SafeMemoryHandler<P>> {
        &mut self.handler
    }

    /// Verifies the overall integrity of the memory managed by the handler.
    ///
    /// # Errors
    ///
    /// Returns an error if an integrity violation is detected.
    pub fn verify_integrity(&self) -> Result<()> {
        let handler = self.handler.lock();
        handler.provider().verify_integrity()
    }
}

/// Trait extension for Provider to create atomic memory operations
pub trait AtomicMemoryExt: Provider + Sized {
    /// Creates a new AtomicMemoryOps from this provider
    fn into_atomic_ops(self) -> Result<AtomicMemoryOps<Self>>
    where
        Self: Clone,
    {
        AtomicMemoryOps::from_provider(self)
    }
}

// Implement the extension trait for all types that implement Provider
impl<T: Provider> AtomicMemoryExt for T {}

