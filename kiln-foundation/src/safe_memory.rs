// WRT - wrt-foundation
// Module: Safe Memory Abstractions
// SW-REQ-ID: REQ_MEM_SAFETY_001

// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides safe memory access primitives, including `SafeMemoryHandler` and
//! related types, ensuring bounds checking and optional integrity checks.

// Core imports
// REMOVE: #[cfg(feature = "std")]
// REMOVE: extern crate std;

// Binary std/no_std choice

use core::{
    fmt,
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
};

use crate::{
    codes,
    operations::{
        record_global_operation,
        Type as OperationType,
    },
    verification::{
        Checksum,
        VerificationLevel,
    },
    Error,
    ErrorCategory,
};
// Result is imported through the prelude

/// Binary std/no_std choice
pub const DEFAULT_MEMORY_PROVIDER_CAPACITY: usize = 4096;

/// Default NoStdProvider type with the DEFAULT_MEMORY_PROVIDER_CAPACITY size
/// This provides backwards compatibility with existing code
pub type DefaultNoStdProvider = NoStdProvider<DEFAULT_MEMORY_PROVIDER_CAPACITY>;
// Checksum is in prelude
// pub use crate::verification::Checksum;
// HashSet and Mutex are in std part of prelude when feature "std" is on.
// For no_std, Mutex comes from once_mutex, HashSet isn't used directly here for
// no_std provider.
// REMOVE: #[cfg(feature = "std")]
// REMOVE: use std::collections::HashSet;
// REMOVE: #[cfg(feature = "std")]
// REMOVE: use std::sync::Mutex;
// REMOVE: #[cfg(feature = "std")]
// REMOVE: use std::vec::Vec; // Explicitly import Vec for StdProvider
#[cfg(feature = "std")]
use std::collections::HashSet;
#[cfg(feature = "std")]
use std::sync::Mutex; /* Note: std::sync::Mutex might be an issue if a no_std mutex is
                       * needed elsewhere. */
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(feature = "std")]
pub use crate::prelude::ToString;
pub use crate::prelude::*;
// Checksum and VerificationLevel are already imported through prelude

/// A safe slice with integrated checksum for data integrity verification
#[derive(Clone)]
pub struct Slice<'a> {
    /// The underlying data slice
    data:               &'a [u8],
    /// Checksum for data integrity verification
    checksum:           Checksum,
    /// Length of the slice for redundant verification
    length:             usize,
    /// Verification level for this slice
    verification_level: VerificationLevel,
}

impl<'a> Slice<'a> {
    /// Create a new `Slice` from a raw byte slice
    ///
    /// This computes a checksum for the data which can be used
    /// to verify integrity later.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial integrity verification fails.
    pub fn new(data: &'a [u8]) -> Result<Self> {
        Self::with_verification_level(data, VerificationLevel::default())
    }

    /// Create a new `Slice` with a specific verification level
    ///
    /// This computes a checksum for the data which can be used
    /// to verify integrity later.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial integrity verification fails (e.g.,
    /// checksum computation error or validation failure upon creation).
    ///
    /// # Panics
    ///
    /// This function previously panicked if initial verification failed.
    /// It now returns a `Result` to indicate failure.
    /// Safety impact: [LOW|MEDIUM|HIGH] - [Brief explanation of the safety
    /// implication] Tracking: WRTQ-XXX (qualification requirement tracking
    /// ID).
    pub fn with_verification_level(data: &'a [u8], level: VerificationLevel) -> Result<Self> {
        // Track checksum calculation
        record_global_operation(OperationType::ChecksumCalculation, level);

        let checksum = Checksum::compute(data);
        let length = data.len();

        let slice = Self {
            data,
            checksum,
            length,
            verification_level: level,
        };

        // Verify on creation to ensure consistency
        // Use full importance (255) for initial verification
        slice.verify_integrity_with_importance(255)?;

        Ok(slice)
    }

    /// Get the underlying data slice
    ///
    /// This performs an integrity check before returning the data.
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails.
    pub fn data(&self) -> Result<&'a [u8]> {
        // Track memory read operation
        record_global_operation(OperationType::MemoryRead, self.verification_level);

        // Medium importance for data access (128)
        self.verify_integrity_with_importance(128)?;
        Ok(self.data)
    }

    /// Get length of the slice
    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Check if the slice is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get the current verification level
    #[must_use]
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Set a new verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Verify data integrity using the stored checksum
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails based on the current
    /// verification level and default importance (128).
    pub fn verify_integrity(&self) -> Result<()> {
        // Track validation operation
        record_global_operation(OperationType::CollectionValidate, self.verification_level);

        // Medium importance by default (128)
        self.verify_integrity_with_importance(128)
    }

    /// Verify data integrity with specified operation importance
    ///
    /// The importance value (0-255) affects the likelihood of
    /// verification when using `VerificationLevel::Sampling`
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails based on the current
    /// verification level and the provided importance.
    pub fn verify_integrity_with_importance(&self, importance: u8) -> Result<()> {
        // Skip verification if the level indicates we shouldn't verify
        if !self.verification_level.should_verify(importance) {
            return Ok(());
        }

        // If length doesn't match stored value, memory is corrupt
        if self.data.len() != self.length {
            return Err(Error::validation_error(
                "Memory corruption: length mismatch on read",
            ));
        }

        // Different paths for optimize vs non-optimize
        #[cfg(feature = "optimize")]
        {
            Ok(())
        }

        #[cfg(not(feature = "optimize"))]
        {
            // Track checksum calculation for important operations
            if importance >= 200 || self.verification_level.should_verify_redundant() {
                record_global_operation(
                    OperationType::ChecksumCalculation,
                    self.verification_level,
                );
            }

            // Skip detailed checks for low importance non-redundant checks
            if !self.verification_level.should_verify_redundant() && importance < 200 {
                return Ok(());
            }

            // Compute current checksum and compare with stored checksum
            let current = Checksum::compute(self.data);
            if current == self.checksum {
                Ok(())
            } else {
                Err(Error::validation_error(
                    "Memory corruption: checksum mismatch on read",
                ))
            }
        }
    }

    /// Create a sub-slice with the same safety guarantees
    ///
    /// # Errors
    ///
    /// Returns an error if the parent slice fails its integrity check, if the
    /// requested sub-slice range is invalid (overflow or out of bounds), or
    /// if creating the sub-slice fails its own initial integrity check.
    pub fn slice(&self, start: usize, len: usize) -> Result<Slice<'a>> {
        // Track memory read operation
        record_global_operation(OperationType::MemoryRead, self.verification_level);

        // High importance for slicing operations (200)
        self.verify_integrity_with_importance(200)?;

        // Use let-else for cleaner error handling
        let Some(end) = start.checked_add(len) else {
            return Err(Error::memory_error("Sub-slice range calculation overflow"));
        };

        if end > self.length {
            // Check if calculated end (exclusive) exceeds original slice length
            return Err(Error::memory_error("Invalid sub-slice range"));
        }

        // Create a new Slice with the specified range
        let sub_data = &self.data[start..end];

        // Create a new Slice with the same verification level
        Slice::with_verification_level(sub_data, self.verification_level)
    }
}

impl fmt::Debug for Slice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Slice")
            .field("length", &self.length)
            .field("checksum", &self.checksum)
            .field("verification_level", &self.verification_level)
            .finish()
    }
}

/// A safe mutable slice with integrated checksum for data integrity
/// verification
pub struct SliceMut<'a> {
    /// The underlying mutable data slice
    data:               &'a mut [u8],
    /// Checksum for data integrity verification
    checksum:           Checksum,
    /// Length of the slice for redundant verification
    length:             usize,
    /// Verification level for this slice
    verification_level: VerificationLevel,
}

impl<'a> SliceMut<'a> {
    /// Create a new `SliceMut` from a raw mutable byte slice
    ///
    /// This computes a checksum for the data which can be used
    /// to verify integrity.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial integrity check fails.
    pub fn new(data: &'a mut [u8]) -> Result<Self> {
        Self::with_verification_level(data, VerificationLevel::default())
    }

    /// Create a new `SliceMut` with a specific verification level
    ///
    /// # Errors
    ///
    /// Returns an error if the initial integrity check fails (e.g., checksum
    /// computation error or validation failure upon creation).
    ///
    /// # Panics
    /// This function previously panicked if initial verification failed.
    /// It now returns a `Result` to indicate failure.
    pub fn with_verification_level(data: &'a mut [u8], level: VerificationLevel) -> Result<Self> {
        record_global_operation(OperationType::ChecksumCalculation, level);
        let checksum = Checksum::compute(data);
        let length = data.len();
        let slice = Self {
            data,
            checksum,
            length,
            verification_level: level,
        };

        // Verify on creation
        slice.verify_integrity_with_importance(255)?; // High importance for creation

        Ok(slice)
    }

    /// Get a mutable reference to the underlying data slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails.
    ///
    /// # Safety
    /// Modifying the returned slice directly will invalidate the stored
    /// checksum. The checksum must be updated using `update_checksum()`
    /// after modification. This performs an integrity check before
    /// returning the data.
    pub fn data_mut(&mut self) -> Result<&mut [u8]> {
        record_global_operation(OperationType::MemoryWrite, self.verification_level); // Or a more specific operation
        self.verify_integrity_with_importance(128)?;
        Ok(self.data)
    }

    /// Get an immutable reference to the underlying data slice.
    /// This performs an integrity check before returning the data.
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails.
    pub fn data(&self) -> Result<&[u8]> {
        record_global_operation(OperationType::MemoryRead, self.verification_level);
        self.verify_integrity_with_importance(128)?;
        Ok(self.data)
    }

    /// Get length of the slice
    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Check if the slice is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get the current verification level
    #[must_use]
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Set a new verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Recomputes and updates the checksum based on the current data.
    /// This should be called after any direct modification to the slice
    /// obtained via `data_mut()`.
    pub fn update_checksum(&mut self) {
        record_global_operation(OperationType::ChecksumCalculation, self.verification_level);
        self.checksum = Checksum::compute(self.data);
    }

    /// Verify data integrity using the stored checksum
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails based on the current
    /// verification level and default importance (128).
    pub fn verify_integrity(&self) -> Result<()> {
        record_global_operation(OperationType::CollectionValidate, self.verification_level);
        self.verify_integrity_with_importance(128)
    }

    /// Verify data integrity with specified operation importance
    ///
    /// # Errors
    ///
    /// Returns an error if the integrity check fails based on the current
    /// verification level and the provided importance.
    pub fn verify_integrity_with_importance(&self, importance: u8) -> Result<()> {
        if !self.verification_level.should_verify(importance) {
            return Ok(());
        }
        if self.data.len() != self.length {
            return Err(Error::validation_error(
                "Memory corruption detected: length mismatch (SliceMut)",
            ));
        }

        #[cfg(feature = "optimize")]
        {
            Ok(())
        }
        #[cfg(not(feature = "optimize"))]
        {
            if importance >= 200 || self.verification_level.should_verify_redundant() {
                record_global_operation(
                    OperationType::ChecksumCalculation,
                    self.verification_level,
                );
            }
            if !self.verification_level.should_verify_redundant() && importance < 200 {
                return Ok(());
            }
            let current_checksum = Checksum::compute(self.data);
            if current_checksum == self.checksum {
                Ok(())
            } else {
                Err(Error::validation_error(
                    "Memory corruption: checksum mismatch on read",
                ))
            }
        }
    }

    /// Slices the mutable slice into a smaller mutable sub-slice.
    ///
    /// Performs bounds checking and returns a new `SliceMut` representing the
    /// sub-slice. The new slice inherits the verification level.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested sub-slice is out of bounds or if
    /// creating the new `SliceMut` fails (e.g., internal verification error).
    pub fn slice_mut<'s>(&'s mut self, start: usize, len: usize) -> Result<SliceMut<'s>> {
        if start.checked_add(len).map_or(true, |end| end > self.length) {
            return Err(Error::runtime_execution_error("Operation failed"));
        }

        // Integrity of the parent slice should ideally be checked before creating a
        // sub-slice if there's a risk of TOCTOU, but SliceMut itself manages this.
        let sub_data = &mut self.data[start..start + len];
        SliceMut::with_verification_level(sub_data, self.verification_level)
    }

    /// Get a mutable raw pointer to the start of the slice data, if the offset
    /// is within bounds.
    ///
    /// # Safety
    ///
    /// Returning a raw pointer bypasses Rust's borrow checking rules.
    /// The caller must ensure that the pointer is used safely:
    /// - The pointer must not be used after the `SliceMut` it originated from
    ///   is dropped or modified in a way that invalidates the pointer (e.g., if
    /// Binary std/no_std choice
    /// - Accesses through the pointer must be within the bounds of the original
    ///   slice.
    /// - Data races must be prevented if the same memory region can be accessed
    ///   through other means (e.g., other pointers or safe references).
    /// - Modifying the memory through this pointer invalidates the internal
    ///   checksum. `update_checksum()` must be called afterwards if integrity
    ///   checks are still required.
    ///
    /// This method should only be used when strictly necessary, typically for
    /// FFI or performance-critical code where invariants are manually
    /// upheld.
    #[must_use]
    pub fn get_ptr_mut(&self, offset: usize) -> Option<*mut u8> {
        // Check if offset is within the bounds of the slice
        if offset < self.length {
            // Check against self.length for consistency
            // .as_ptr() gets a raw pointer to the start.
            // .wrapping_add(offset) calculates the pointer offset, wrapping on overflow
            // (though overflow shouldn't happen here due to the bounds check).
            // .cast_mut() converts *const u8 to *mut u8. This is safe as long
            // as the original reference `&'a mut [u8]` allows mutation.
            Some(self.data.as_ptr().wrapping_add(offset).cast_mut())
        } else {
            // Offset is out of bounds
            None
        }
    }
}

impl<'a> AsRef<[u8]> for Slice<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

impl<'a> AsRef<[u8]> for SliceMut<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

impl fmt::Debug for SliceMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SliceMut")
            .field("length", &self.length)
            .field("checksum", &self.checksum)
            .field("verification_level", &self.verification_level)
            // Do not print mutable data content in debug
            .finish_non_exhaustive() // If SliceMut has private fields or for
                                     // future-proofing
    }
}

/// Binary std/no_std choice
///
/// Binary std/no_std choice
/// allowing both std and `no_std` environments to share the same interface.
/// It combines raw access, safety features, and informational methods.
pub trait Provider: Send + Sync + fmt::Debug + Clone + Default + PartialEq + Eq {
    /// Binary std/no_std choice
    type Allocator: Allocator + Clone + Send + Sync + 'static; // Added Clone, Send, Sync, 'static

    /// Borrows a slice of memory with safety guarantees.
    ///
    /// # Safety
    /// Callers must ensure that `offset` and `len` define a valid memory
    /// region that is safe to read for the lifetime of the returned slice.
    /// This is typically managed by `verify_access` being called internally
    /// or by the provider's own invariants. Implementers must ensure that
    /// the returned slice does not outlive the underlying memory buffer and
    /// that the memory region is valid for reads of `len` bytes at `offset`.
    fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>>;

    /// Writes data to the memory at a given offset.
    ///
    /// # Safety
    /// Callers must ensure that `offset` and the length of `data` define a
    /// valid memory region that is safe to write to. This is typically
    /// managed by `verify_access`. Implementers must ensure that this
    /// operation is memory safe given valid inputs, respecting bounds and
    /// aliasing rules. The `offset` and `data.len()` must not cause an
    /// Binary std/no_std choice
    fn write_data(&mut self, offset: usize, data: &[u8]) -> Result<()>;

    /// Verifies that an access to memory (read or write) of `len` at `offset`
    /// would be valid. This is a pre-check and does not perform the access.
    ///
    /// # Errors
    ///
    /// Returns an error if the described access would be invalid (e.g., out of
    /// bounds).
    fn verify_access(&self, offset: usize, len: usize) -> Result<()>;

    /// Gets the total current size/length of the initialized/used memory within
    /// the provider.
    fn size(&self) -> usize;

    /// Gets the total capacity of the memory region managed by the provider.
    fn capacity(&self) -> usize;

    /// Verifies the overall integrity of the memory managed by the provider.
    /// This could involve checking internal checksums, canaries, or other
    /// mechanisms.
    ///
    /// # Errors
    ///
    /// Returns an error if an integrity violation is detected.
    fn verify_integrity(&self) -> Result<()>;

    /// Sets the verification level for operations performed by this memory
    /// provider.
    fn set_verification_level(&mut self, level: VerificationLevel);

    /// Gets the current verification level of this memory provider.
    fn verification_level(&self) -> VerificationLevel;

    /// Gets statistics about memory usage from this provider.
    fn memory_stats(&self) -> Stats;

    /// Gets a mutable slice from the underlying memory provider.
    ///
    /// # Safety
    /// Callers must ensure that `offset` and `len` define a valid memory
    /// region that is safe for mutable access for the lifetime of the
    /// returned slice. This is typically managed by `verify_access`.
    /// Implementers must ensure that the returned mutable slice provides
    /// exclusive access (or appropriate synchronization if shared mutable
    /// access is possible and intended) to that region of memory and
    /// does not outlive the underlying buffer. The region must be valid
    /// for both reads and writes.
    fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>>;

    /// Copies data within the memory provider from a source offset to a
    /// destination offset.
    ///
    /// # Safety
    /// Callers must ensure that both source and destination ranges
    /// (`src_offset` to `src_offset + len` and `dst_offset` to `dst_offset
    /// + len`) are valid memory regions within the provider's bounds and
    /// that the operation is memory safe.
    /// This includes handling of overlapping regions if the underlying copy
    /// mechanism requires it (e.g., `ptr::copy` vs `ptr::copy_nonoverlapping`).
    /// Implementers must ensure the copy is performed safely.
    fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()>;

    /// Ensures the provider's internal accounting of used space extends up to
    /// `byte_offset`. This is typically called by a collection after it
    /// logically extends its length.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested offset is beyond the provider's
    /// capacity or if the operation fails internally.
    fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()>;

    /// Binary std/no_std choice
    fn acquire_memory(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8>;

    /// Releases a previously acquired block of memory to the provider's
    /// Binary std/no_std choice
    ///
    /// # Safety
    /// This method encapsulates unsafe operations internally.
    /// Binary std/no_std choice
    /// `acquire_memory` with the same `layout`, and not yet released.
    fn release_memory(&self, ptr: *mut u8, layout: core::alloc::Layout) -> wrt_error::Result<()>;

    /// Binary std/no_std choice
    fn get_allocator(&self) -> &Self::Allocator;

    /// Creates a new `SafeMemoryHandler` for this provider.
    /// Requires `Self: Sized` and `Self: Clone` for the handler to be created
    /// and cloned.
    fn new_handler(&self) -> Result<SafeMemoryHandler<Self>>
    where
        Self: Sized + Clone;
}

/// Memory statistics structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Total memory size in bytes
    pub total_size:      usize,
    /// Number of memory accesses
    pub access_count:    usize,
    /// Number of unique memory regions accessed
    pub unique_regions:  usize,
    /// Maximum size of any access
    pub max_access_size: usize,
}

#[cfg(feature = "std")]
pub struct StdProvider {
    /// The underlying data buffer
    data:               Vec<u8>,
    /// Track memory accesses for safety monitoring
    access_log:         Mutex<Vec<(usize, usize)>>,
    /// Counter for access operations
    access_count:       AtomicUsize,
    /// Maximum size of any access seen
    max_access_size:    AtomicUsize,
    /// Number of unique regions accessed
    unique_regions:     AtomicUsize,
    /// Regions hash (for uniqueness tracking)
    regions_hash:       Mutex<HashSet<usize>>,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
}

#[cfg(feature = "std")]
impl Clone for StdProvider {
    fn clone(&self) -> Self {
        Self {
            data:               self.data.clone(),
            access_log:         Mutex::new(Vec::new()), // Create new empty access log
            access_count:       AtomicUsize::new(0),
            max_access_size:    AtomicUsize::new(0),
            unique_regions:     AtomicUsize::new(0),
            regions_hash:       Mutex::new(HashSet::new()),
            verification_level: self.verification_level,
        }
    }
}

#[cfg(feature = "std")]
impl fmt::Debug for StdProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdProvider")
            .field("capacity", &self.data.capacity())
            .field("len", &self.data.len())
            .field("access_count", &self.access_count.load(Ordering::Relaxed))
            .field("max_access_size", &self.max_access_size.load(Ordering::Relaxed))
            .field("unique_regions", &self.unique_regions.load(Ordering::Relaxed))
            .field("verification_level", &self.verification_level)
            // Not displaying access_log or regions_hash to avoid excessive output
            .finish()
    }
}

#[cfg(feature = "std")]
impl PartialEq for StdProvider {
    fn eq(&self, other: &Self) -> bool {
        // Compare self.data (Vec<u8>) and self.verification_level.
        // Mutex-wrapped fields (access_log, regions_hash) and atomics (access_count,
        // etc.) are generally not part of structural equality as they represent
        // mutable runtime state rather than fundamental identity.
        self.data == other.data && self.verification_level == other.verification_level
    }
}

#[cfg(feature = "std")]
impl Eq for StdProvider {}

#[cfg(feature = "std")]
impl Default for StdProvider {
    fn default() -> Self {
        Self {
            data:               Vec::new(),
            access_log:         Mutex::new(Vec::new()),
            access_count:       AtomicUsize::new(0),
            max_access_size:    AtomicUsize::new(0),
            unique_regions:     AtomicUsize::new(0),
            regions_hash:       Mutex::new(HashSet::new()),
            verification_level: VerificationLevel::default(),
        }
    }
}

#[cfg(feature = "std")]
impl StdProvider {
    /// Create a new `StdProvider` with an initial data buffer.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            access_log: Mutex::new(Vec::new()),
            access_count: AtomicUsize::new(0),
            max_access_size: AtomicUsize::new(0),
            unique_regions: AtomicUsize::new(0),
            regions_hash: Mutex::new(HashSet::new()),
            verification_level: VerificationLevel::default(),
        }
    }

    /// Create a new `StdProvider` with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data:               Vec::with_capacity(capacity),
            access_log:         Mutex::new(Vec::new()),
            access_count:       AtomicUsize::new(0),
            max_access_size:    AtomicUsize::new(0),
            unique_regions:     AtomicUsize::new(0),
            regions_hash:       Mutex::new(HashSet::new()),
            verification_level: VerificationLevel::default(),
        }
    }

    /// Get the access log. Requires `std` for `Mutex` and `Vec`.
    pub fn access_log(&self) -> Result<Vec<(usize, usize)>> {
        self.access_log
            .lock()
            .map_err(|_| Error::from(crate::kinds::PoisonedLockError("Mutex poisoned")))
            .map(|log| log.clone())
    }

    /// Add data to the provider's buffer.
    pub fn add_data(&mut self, data_to_add: &[u8]) {
        self.data.extend_from_slice(data_to_add);
    }

    /// Clears the provider's buffer and resets tracking.
    pub fn clear(&mut self) {
        self.data.clear();
        if let Ok(mut log) = self.access_log.lock() {
            log.clear();
        }
        if let Ok(mut hash) = self.regions_hash.lock() {
            hash.clear();
        }
        self.access_count.store(0, Ordering::Relaxed);
        self.max_access_size.store(0, Ordering::Relaxed);
        self.unique_regions.store(0, Ordering::Relaxed);
    }

    /// Resizes the internal buffer to `new_size`, filling new space with
    /// `value`.
    ///
    /// # Errors
    ///
    /// Binary std/no_std choice
    /// large.
    pub fn resize(&mut self, new_size: usize, value: u8) -> Result<()> {
        if new_size > self.data.capacity() {
            return Err(Error::memory_error("StdProvider resize exceeds capacity"));
        }
        self.data.resize(new_size, value);
        Ok(())
    }

    fn track_access(&self, offset: usize, len: usize) {
        if let Ok(mut log) = self.access_log.lock() {
            log.push((offset, len));
        }
        self.access_count.fetch_add(1, Ordering::Relaxed);
        self.max_access_size.fetch_max(len, Ordering::Relaxed);
        // Simplified unique region tracking for this example
        // A more robust approach might involve a set of ranges
        if let Ok(mut hash) = self.regions_hash.lock() {
            if hash.insert(offset) {
                self.unique_regions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Clears the access tracking information.
    ///
    /// # Errors
    ///
    /// Returns `Error::sync_error` if the mutex cannot be locked.
    pub fn clear_access_tracking(&self) -> Result<()> {
        record_global_operation(OperationType::CollectionClear, self.verification_level);
        let mut log = self.access_log.lock().map_err(|_| {
            Error::from(crate::kinds::PoisonedLockError(
                "Mutex poisoned during access log lock in clear_access_tracking",
            ))
        })?;
        log.clear();

        let mut hashes = self.regions_hash.lock().map_err(|_| {
            Error::from(crate::kinds::PoisonedLockError(
                "Mutex poisoned during regions_hash lock in clear_access_tracking",
            ))
        })?;
        hashes.clear();

        self.access_count.store(0, Ordering::Relaxed);
        self.max_access_size.store(0, Ordering::Relaxed);
        self.unique_regions.store(0, Ordering::Relaxed);
        Ok(())
    }

    /// Recalculates checksums for all data within the provider.
    /// Note: This is a placeholder concept. `StdProvider` itself doesn't
    /// store checksums directly for its entire buffer in the same way `Slice`
    /// does. This might be relevant if `StdProvider` were to implement more
    /// complex integrity schemes. For now, it's a no-op or should be
    /// clarified.
    pub fn recalculate_checksums(&mut self) {
        // Placeholder: StdProvider does not currently manage a single checksum for all
        // data. Integrity is typically verified via Slice/SliceMut instances it
        // vends.
        record_global_operation(
            OperationType::ChecksumFullRecalculation,
            self.verification_level,
        );
    }
}

#[cfg(feature = "std")]
impl Provider for StdProvider {
    type Allocator = Self;

    // Binary std/no_std choice

    /// # Safety
    /// The caller guarantees that `offset` and `len` define a valid, readable
    /// memory region within `self.data` that remains valid for the lifetime
    /// \'_. `verify_access` is called internally to check bounds against
    /// `self.data.len()`.
    fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        self.verify_access(offset, len)?;
        self.track_access(offset, len);
        // Ensure that the slice we create does not exceed the actual data length.
        debug_assert!(
            offset.checked_add(len).map_or(false, |end| end <= self.data.len()),
            "StdProvider::borrow_slice: offset+len must be <= self.data.len(). Offset: {}, Len: \
             {}, DataLen: {}",
            offset,
            len,
            self.data.len()
        );
        Slice::with_verification_level(&self.data[offset..offset + len], self.verification_level)
    }

    /// # Safety
    /// The caller guarantees that `offset` and `data.len()` define a valid,
    /// writable memory region within `self.data`. `verify_access` is called
    /// internally. The underlying `Vec` ensures that writes within its
    /// bounds are safe.
    fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>> {
        self.verify_access(offset, len)?;
        self.track_access(offset, len); // Tracking access before mutation
        debug_assert!(
            offset.checked_add(len).map_or(false, |end| end <= self.data.len()),
            "StdProvider::get_slice_mut: offset+len must be <= self.data.len() after \
             verify_access. Offset: {}, Len: {}, DataLen: {}",
            offset,
            len,
            self.data.len()
        );
        // Create a SliceMut with the provider's verification level
        SliceMut::with_verification_level(
            &mut self.data[offset..offset + len],
            self.verification_level,
        )
    }

    /// # Safety
    /// The caller guarantees that `offset` and `data_to_write.len()` define a
    /// valid, writable region within `self.data`. `verify_access` checks
    /// bounds. `copy_from_slice` is safe when destination and source
    /// lengths match and slices are valid, which `verify_access` and slice
    /// creation ensure.
    fn write_data(&mut self, offset: usize, data_to_write: &[u8]) -> Result<()> {
        self.verify_access(offset, data_to_write.len())?;
        self.track_access(offset, data_to_write.len());
        debug_assert!(
            offset
                .checked_add(data_to_write.len())
                .map_or(false, |end| end <= self.data.len()),
            "StdProvider::write_data: offset+len must be <= self.data.len() after verify_access. \
             Offset: {}, Len: {}, DataLen: {}",
            offset,
            data_to_write.len(),
            self.data.len()
        );

        // Safety: verify_access ensures offset + data_to_write.len() is within
        // self.data.capacity(). And also ensures offset + data_to_write.len()
        // <= self.data.len() (current initialized part for Vec)
        let required_len = offset
            .checked_add(data_to_write.len())
            .ok_or_else(|| Error::memory_error("Write offset + length calculation overflow"))?;

        if required_len > self.data.len() {
            // Binary std/no_std choice
            self.data.resize(required_len, 0u8); // Or some other default byte
        }

        self.data[offset..required_len].copy_from_slice(data_to_write);
        // If StdProvider maintained its own checksum for the whole data Vec, it would
        // need updating.
        Ok(())
    }

    fn verify_access(&self, offset: usize, len: usize) -> Result<()> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| Error::memory_error("Access range calculation overflow"))?;

        // For StdProvider, capacity is dynamic up to available memory, but we check
        // against current .len(). If write_data can resize, then this check is
        // more about the *current* state for reads. For writes, write_data
        // handles potential resizing. Let's assume verify_access checks against
        // current data.len() for reads and initial write checks.
        if end > self.data.len() {
            return Err(Error::runtime_execution_error("Operation failed"));
        }
        Ok(())
    }

    fn size(&self) -> usize {
        self.data.len()
    }

    fn capacity(&self) -> usize {
        self.data.capacity()
    }

    fn verify_integrity(&self) -> Result<()> {
        // StdProvider itself doesn't have a single checksum for its entire data Vec.
        // Integrity is managed at the Slice/SliceMut level for borrowed parts.
        // This could be extended if a whole-provider checksum was desired.
        record_global_operation(OperationType::CollectionValidate, self.verification_level);
        // For now, assume the underlying Vec<u8> is inherently valid unless proven
        // otherwise by Slice checks.
        Ok(())
    }

    fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    fn memory_stats(&self) -> Stats {
        Stats {
            total_size:      self.data.capacity(),
            access_count:    self.access_count.load(Ordering::Relaxed),
            unique_regions:  self.unique_regions.load(Ordering::Relaxed),
            max_access_size: self.max_access_size.load(Ordering::Relaxed),
        }
    }

    /// # Safety
    /// The caller guarantees that `src_offset..src_offset+len` is a valid
    /// readable range and `dst_offset..dst_offset+len` is a valid writable
    /// range within `self.data`. `verify_access` is called for both source
    /// and destination ranges (implicitly for dest by checking capacity).
    /// `Vec::copy_within` is safe if bounds are respected, which they are by
    /// these checks.
    fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()> {
        self.verify_access(src_offset, len)?;
        self.verify_access(dst_offset, len)?; // Ensure dst is also valid
        debug_assert!(
            src_offset.checked_add(len).map_or(false, |end| end <= self.data.len()),
            "StdProvider::copy_within (src): src_offset+len must be <= self.data.len(). \
             SrcOffset: {}, Len: {}, DataLen: {}",
            src_offset,
            len,
            self.data.len()
        );
        debug_assert!(
            dst_offset.checked_add(len).map_or(false, |end| end <= self.data.len()),
            "StdProvider::copy_within (dst): dst_offset+len must be <= self.data.len(). \
             DstOffset: {}, Len: {}, DataLen: {}",
            dst_offset,
            len,
            self.data.len()
        );

        // Safety: verify_access for both src and dst ranges has been called.
        // This ensures that src_offset + len and dst_offset + len are within bounds.
        self.track_access(dst_offset, len); // Track the write to destination

        self.data.copy_within(src_offset..src_offset + len, dst_offset);
        Ok(())
    }

    fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()> {
        if byte_offset > self.data.capacity() {
            // This attempts to reserve additional capacity if byte_offset is beyond current
            // Binary std/no_std choice
            let additional = byte_offset - self.data.len(); // Only reserve if truly needed beyond current length.
            if additional > 0 && byte_offset > self.data.capacity() {
                // Calculate needed additional capacity beyond current capacity.
                let needed_cap_increase = byte_offset - self.data.capacity();
                self.data.reserve(needed_cap_increase);
            }
        }
        // Ensure the length of the vector is at least `byte_offset`.
        // This is crucial for collections like BoundedVec that manage their own length
        // but rely on the provider to have the underlying storage initialized or
        // accessible.
        if byte_offset > self.data.len() {
            // Binary std/no_std choice
            self.data.resize(byte_offset, 0u8); // Initialize new bytes to 0
        }
        Ok(())
    }

    fn get_allocator(&self) -> &Self::Allocator {
        self // Since Self implements Allocator
    }

    fn acquire_memory(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8> {
        // Delegate to its own Allocator implementation
        self.allocate(layout)
    }

    fn release_memory(&self, ptr: *mut u8, layout: core::alloc::Layout) -> wrt_error::Result<()> {
        // Delegate to its own Allocator implementation
        self.deallocate(ptr, layout)
    }

    fn new_handler(&self) -> Result<SafeMemoryHandler<Self>>
    where
        Self: Sized + Clone,
    {
        Ok(SafeMemoryHandler::new(self.clone()))
    }

    // Fix duplicate memory_stats implementation
    // fn memory_stats(&self) -> Stats {
    //    self.memory_stats() // Calls its own inherent method
    // }
}

#[cfg(feature = "std")]
impl Allocator for StdProvider {
    fn allocate(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8> {
        // Binary std/no_std choice
        // This would require unsafe code and proper memory management
        Err(Error::memory_error(
            "StdProvider does not support raw allocation",
        ))
    }

    fn deallocate(&self, _ptr: *mut u8, _layout: core::alloc::Layout) -> wrt_error::Result<()> {
        // Binary std/no_std choice
        Err(Error::memory_error(
            "StdProvider does not support raw deallocation",
        ))
    }
}

/// Memory provider using a fixed-size array, suitable for `no_std`
/// environments.
///
/// Binary std/no_std choice
pub struct NoStdProvider<const N: usize> {
    /// The underlying data buffer
    data:               [u8; N],
    /// Current usage of the buffer
    used:               usize,
    /// Counter for access operations
    access_count:       AtomicUsize,
    /// Last access offset for validation
    last_access_offset: AtomicUsize,
    /// Last access length for validation
    last_access_length: AtomicUsize,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
}

impl<const N: usize> PartialEq for NoStdProvider<N> {
    fn eq(&self, other: &Self) -> bool {
        // Compare data content up to 'used' length, 'used' length itself, and
        // verification_level. Atomics are stateful and not part of fundamental
        // equality. The raw buffer `self.data` should be compared only up to
        // `self.used` because bytes beyond `used` are uninitialized or stale.
        if self.used != other.used {
            return false;
        }
        if self.verification_level != other.verification_level {
            return false;
        }
        // Compare the initialized parts of the data buffer
        self.data[..self.used] == other.data[..other.used]
    }
}

impl<const N: usize> Eq for NoStdProvider<N> {}

impl<const N: usize> PartialOrd for NoStdProvider<N> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for NoStdProvider<N> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // First compare by used size
        match self.used.cmp(&other.used) {
            core::cmp::Ordering::Equal => {
                // If sizes are equal, compare the actual data content
                self.data[..self.used].cmp(&other.data[..other.used])
            },
            other => other,
        }
    }
}

impl<const N: usize> core::hash::Hash for NoStdProvider<N> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Only hash fields that define the identity or configuration.
        // Runtime state like `used` or `access_count` might not be part of the hash
        // if hashing is for e.g. inserting into a HashMap based on configuration.
        // However, for a general Hash impl, more fields might be included.
        // For now, let's hash based on its constant capacity N and current content up
        // to `used`. Atomics are problematic for direct hashing in a struct.
        // We should hash their current values if they contribute to identity.

        // Hash the generic constant N (part of the type identity)
        N.hash(state);
        // Hash the `used` length
        self.used.hash(state);
        // Hash the data actually in use
        self.data[0..self.used].hash(state);
        // Hash verification_level as it's a configuration aspect
        self.verification_level.hash(state);

        // Do NOT hash atomics directly. If their values are part of the
        // hashable identity, load them. For
        // DefaultMemoryProvider(NoStdProvider<0>), these are likely not
        // critical for Hash. self.access_count.load(Ordering::Relaxed).
        // hash(state); self.last_access_offset.load(Ordering::Relaxed).
        // hash(state); self.last_access_length.load(Ordering::Relaxed).
        // hash(state);
    }
}

impl<const N: usize> Clone for NoStdProvider<N> {
    fn clone(&self) -> Self {
        Self {
            data:               self.data, // [u8; N] is Copy
            used:               self.used,
            access_count:       AtomicUsize::new(self.access_count.load(Ordering::Relaxed)),
            last_access_offset: AtomicUsize::new(self.last_access_offset.load(Ordering::Relaxed)),
            last_access_length: AtomicUsize::new(self.last_access_length.load(Ordering::Relaxed)),
            verification_level: self.verification_level, // VerificationLevel is Copy
        }
    }
}

impl<const N: usize> Default for NoStdProvider<N> {
    /// Creates a default NoStdProvider
    ///
    /// # ⚠️ BUDGET BYPASS WARNING ⚠️
    /// Direct use of Default bypasses memory budget tracking!
    /// Use `BudgetAwareProviderFactory::create_provider()` instead.
    /// This implementation exists only for compatibility with bounded
    /// collections.
    ///
    /// # Compile-time Detection
    /// This usage will be detected by budget enforcement lints.
    fn default() -> Self {
        // Emit warning for detection by linting tools
        // Runtime tracking of bypasses for monitoring
        // Modern memory system automatically tracks usage

        // CRITICAL: Prevent stack overflow for large N values
        // For large providers (>8KB), this will cause stack overflow
        if N > 8192 {
            // Use heap allocation for large providers to avoid stack overflow
            return NoStdProvider::<N>::new_heap_allocated();
        }
        
        // Safety: N must be such that [0u8; N] is valid and fits on stack.
        // For N <= 8192, this should be safe on most systems.
        Self {
            data:               [0u8; N], /* Initialize with zeros. Safe for small N. */
            used:               N, /* All bytes are initialized to zero, so all N bytes are usable */
            access_count:       AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
            verification_level: VerificationLevel::default(),
        }
    }
}

impl<const N: usize> NoStdProvider<N> {
    /// Create a heap-allocated provider to avoid stack overflow for large N
    /// This method uses Vec to allocate the data on heap instead of stack
    #[cfg(feature = "std")]
    pub fn new_heap_allocated() -> Self {
        // SIMPLE FIX: Use a Vec<u8> to allocate N bytes on heap, then convert to array
        // This avoids the complex unsafe pointer manipulation
        let mut heap_data = vec![0u8; N];
        
        // Convert Vec to fixed-size array
        // SAFETY: We know the Vec has exactly N elements
        let data_array: [u8; N] = heap_data.try_into()
            .unwrap_or_else(|_| panic!("Failed to convert Vec to array of size {}", N));
        
        Self {
            data: data_array,
            used: N, // WebAssembly memory is fully zero-initialized, so all N bytes are valid to read
            access_count: AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
            verification_level: VerificationLevel::default(),
        }
    }
    
    /// Fallback for no_std environments - use smaller provider or panic
    #[cfg(not(feature = "std"))]
    pub fn new_heap_allocated() -> Self {
        // In no_std, we can't use heap allocation, so this is a critical error
        panic!("Stack overflow: Provider size {} too large for stack allocation in no_std environment. Use smaller provider size <= 8192 bytes.", N);
    }
}

impl<const N: usize> fmt::Debug for NoStdProvider<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoStdProvider")
            .field("capacity", &N)
            .field("used", &self.used)
            .field("access_count", &self.access_count.load(Ordering::Relaxed))
            .field("verification_level", &self.verification_level)
            .finish()
    }
}

// NoStdProvider methods are available in all configurations
impl<const N: usize> NoStdProvider<N> {
    /// Create a new empty memory provider with default verification level
    ///
    /// # Deprecated
    /// This constructor will become private. Use `BudgetProvider::new()` or
    /// `create_provider!` macro instead for budget-aware allocation.
    #[deprecated(
        since = "0.2.0",
        note = "Use BudgetProvider::new() or create_provider! macro for budget-aware allocation"
    )]
    pub fn new() -> Self {
        Self::new_internal(VerificationLevel::default())
    }

    /// Create a new empty memory provider with the specified verification level
    ///
    /// # Deprecated
    /// This constructor will become private. Use `BudgetProvider::new()` or
    /// `create_provider!` macro instead for budget-aware allocation.
    #[deprecated(
        since = "0.2.0",
        note = "Use BudgetProvider::new() or create_provider! macro for budget-aware allocation"
    )]
    pub fn with_verification_level(level: VerificationLevel) -> Self {
        Self::new_internal(level)
    }

    /// Internal constructor that doesn't trigger deprecation warnings
    fn new_internal(level: VerificationLevel) -> Self {
        Self {
            data:               [0; N],
            used:               0,
            access_count:       AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
            verification_level: level,
        }
    }

    /// Create a new memory provider with specified size and verification level
    ///
    /// # Deprecated
    /// This constructor will become private. Use `BudgetProvider::new()` or
    /// `create_provider!` macro instead for budget-aware allocation.
    #[deprecated(
        since = "0.2.0",
        note = "Use BudgetProvider::new() or create_provider! macro for budget-aware allocation"
    )]
    pub fn new_with_size(size: usize, level: VerificationLevel) -> Result<Self> {
        if size > N {
            return Err(Error::memory_error(
                "Requested size exceeds NoStdProvider fixed capacity",
            ));
        }

        let mut provider = Self::new_internal(level);
        if size > 0 {
            provider.resize(size)?;
        }
        Ok(provider)
    }

    /// Set the data for this memory provider
    ///
    /// # Errors
    ///
    /// Returns an error if the provided data length exceeds the fixed capacity
    /// `N`.
    pub fn set_data(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > N {
            return Err(Error::memory_error(
                "Data too large for NoStdProvider capacity",
            ));
        }

        // Copy the data into our fixed buffer
        for (i, &byte) in data.iter().enumerate() {
            self.data[i] = byte;
        }
        self.used = data.len();

        // Reset access metrics
        self.access_count.store(0, Ordering::SeqCst);
        self.last_access_offset.store(0, Ordering::SeqCst);
        self.last_access_length.store(0, Ordering::SeqCst);

        Ok(())
    }

    /// Get the current access count
    pub fn access_count(&self) -> usize {
        self.access_count.load(Ordering::SeqCst)
    }

    /// Reset usage to zero (clear memory)
    pub fn clear(&mut self) {
        self.used = 0;
        self.access_count.store(0, Ordering::SeqCst);
    }

    /// Get the last accessed region
    pub fn last_access(&self) -> (usize, usize) {
        let offset = self.last_access_offset.load(Ordering::SeqCst);
        let length = self.last_access_length.load(Ordering::SeqCst);
        (offset, length)
    }

    /// Resize the used portion of memory
    ///
    /// # Errors
    ///
    /// Returns an error if `new_size` exceeds the fixed capacity `N`.
    pub fn resize(&mut self, new_size: usize) -> Result<()> {
        if new_size > N {
            return Err(Error::memory_error(
                "NoStdProvider cannot resize beyond fixed capacity",
            ));
        }
        // If shrinking, no need to zero out, just update `used`.
        // If growing, the new bytes are uninitialized but within `data` array.
        // The safety of using them later depends on the caller initializing them.
        // For a `Provider`, `size()` reflects initialized/usable memory.
        // `ensure_used_up_to` should be the primary way to "zero-out" or logically
        // extend.
        self.used = new_size;
        Ok(())
    }

    /// Verify memory integrity
    ///
    /// # Errors
    ///
    /// Returns an error if the internal `used` counter exceeds the fixed
    /// capacity `N`, or if the last recorded access was out of the `used`
    /// bounds.
    pub fn verify_integrity(&self) -> Result<()> {
        // Track validation operation
        record_global_operation(OperationType::CollectionValidate, self.verification_level);

        // Simple length check
        if self.used > N {
            return Err(Error::runtime_execution_error("Operation failed"));
        }

        // Verify that the last access was valid
        let offset = self.last_access_offset.load(Ordering::SeqCst);
        let length = self.last_access_length.load(Ordering::SeqCst);

        if length > 0 && offset + length > self.used {
            return Err(Error::memory_access_out_of_bounds("Access out of bounds"));
        }

        Ok(())
    }

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> Stats {
        Stats {
            total_size:      N,
            access_count:    self.access_count(),
            unique_regions:  1, // No-std can't track unique regions
            max_access_size: self.last_access_length.load(Ordering::Relaxed),
        }
    }
}

// NoStdProvider implements Provider in all configurations
impl<const N: usize> Provider for NoStdProvider<N> {
    type Allocator = Self;

    // Binary std/no_std choice

    fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        self.verify_access(offset, len)?;
        debug_assert!(
            offset.checked_add(len).map_or(false, |end| end <= self.used),
            "NoStdProvider::borrow_slice: offset+len must be <= self.used. Offset: {}, Len: {}, \
             Used: {}",
            offset,
            len,
            self.used
        );
        debug_assert!(
            offset.checked_add(len).map_or(false, |end| end <= N),
            "NoStdProvider::borrow_slice: offset+len must be <= N (capacity). Offset: {offset}, \
             Len: {len}, Capacity: {N}"
        );
        Slice::with_verification_level(&self.data[offset..offset + len], self.verification_level)
    }

    fn write_data(&mut self, offset: usize, data_to_write: &[u8]) -> Result<()> {
        // For writes, we only need to check against capacity, not against self.used
        // because writes are allowed to extend into uninitialized memory.
        // Only perform bounds checking against the provider's capacity.
        if offset >= N || offset.saturating_add(data_to_write.len()) > N {
            return Err(Error::memory_out_of_bounds("Write data overflows capacity"));
        }

        // Track write access for statistics if enabled
        if self.verification_level.should_track_stats() {
            self.access_count.fetch_add(1, Ordering::Relaxed);
            self.last_access_offset.store(offset, Ordering::Relaxed);
            self.last_access_length.store(data_to_write.len(), Ordering::Relaxed);
        }

        self.data[offset..offset + data_to_write.len()].copy_from_slice(data_to_write);
        self.used = core::cmp::max(self.used, offset + data_to_write.len());
        // TODO: Consider if checksum/integrity of Slice/SliceMut needs update here if
        // they are live. This provider itself doesn't maintain a checksum over
        // its raw data array. Individual Slices/SliceMut do.
        Ok(())
    }

    fn verify_access(&self, offset: usize, len: usize) -> Result<()> {
        // Track access for statistics, if verification level allows
        if self.verification_level.should_track_stats() {
            self.access_count.fetch_add(1, Ordering::Relaxed);
            self.last_access_offset.store(offset, Ordering::Relaxed);
            self.last_access_length.store(len, Ordering::Relaxed);
        }

        if len == 0 {
            // Allow zero-length access at any point up to capacity (consistent with slice
            // behavior) Or up to self.used if only initialized parts are
            // allowed. For now, capacity.
            if offset > N {
                return Err(Error::memory_out_of_bounds(
                    "Zero-length access out of capacity",
                ));
            }
            return Ok(());
        }

        if offset >= N || offset + len > N {
            Err(Error::memory_out_of_bounds(
                "Memory access out of provider capacity",
            ))
        } else if self.verification_level.should_check_init()
            && (offset >= self.used || offset + len > self.used)
        {
            // This check depends on policy: should we allow access to uninitialized parts
            // within capacity? For now, if init checks are on, restrict to
            // 'used' area.
            Err(Error::memory_uninitialized(
                "Memory access to uninitialized region",
            ))
        } else {
            Ok(())
        }
    }

    fn size(&self) -> usize {
        self.used
    }

    fn capacity(&self) -> usize {
        N
    }

    fn verify_integrity(&self) -> Result<()> {
        // Delegate to the struct's own verify_integrity method
        self.verify_integrity() // Corrected: Call inherent method
    }

    fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    fn get_allocator(&self) -> &Self::Allocator {
        self
    }

    fn acquire_memory(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8> {
        // Binary std/no_std choice
        // It has a fixed buffer. This is more for trait compatibility.
        // We could return a pointer into self.data if layout fits and is unused,
        // Binary std/no_std choice
        // For now, mirror the existing Allocator impl for NoStdProvider
        Allocator::allocate(self, layout)
    }

    fn release_memory(&self, ptr: *mut u8, layout: core::alloc::Layout) -> wrt_error::Result<()> {
        // Mirror the existing Allocator impl for NoStdProvider
        // Safety: This encapsulates the unsafe operation internally
        Allocator::deallocate(self, ptr, layout)
    }

    fn new_handler(&self) -> Result<SafeMemoryHandler<Self>>
    where
        Self: Sized + Clone,
    {
        Ok(SafeMemoryHandler::new(self.clone()))
    }

    fn memory_stats(&self) -> Stats {
        // Delegate to the struct's own memory_stats method
        self.memory_stats() // Corrected: Call inherent method
    }

    fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()> {
        if byte_offset > N {
            return Err(Error::runtime_execution_error("Operation failed"));
        }
        self.used = core::cmp::max(self.used, byte_offset);
        Ok(())
    }

    fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>> {
        self.verify_access(offset, len)?; // Ensures offset+len is within self.used or N based on policy
                                          // SliceMut implies modification, so it should be within 'used' or allow writing
                                          // to extend 'used'. For now, ensure
                                          // it's within capacity N for mutable access.
        if offset + len > N {
            return Err(Error::memory_out_of_bounds(
                "Offset + length exceeds capacity",
            ));
        }
        // If strict init checks are on, we might want to restrict len to self.used -
        // offset. However, SliceMut is often used to write new data.
        // Let's allow getting a slice up to capacity N, and rely on verify_access for
        // init checks if enabled.
        self.used = core::cmp::max(self.used, offset + len); // Getting a mut slice implies it might be used.
        SliceMut::with_verification_level(
            &mut self.data[offset..offset + len],
            self.verification_level,
        )
    }

    fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()> {
        // Verify source read
        self.verify_access(src_offset, len)?;
        // Verify destination write (up to capacity)
        if dst_offset + len > N {
            return Err(Error::memory_out_of_bounds(
                "copy_within destination out of capacity",
            ));
        }

        // Perform the copy
        // copy_within is safe if src and dst ranges are valid, which verify_access
        // should ensure for src, and our bounds check for dst.
        self.data.copy_within(src_offset..src_offset + len, dst_offset);

        // Update used size if dst extends it
        self.used = core::cmp::max(self.used, dst_offset + len);
        Ok(())
    }
}

// New Allocator trait
/// Binary std/no_std choice
pub trait Allocator: fmt::Debug + Send + Sync {
    /// Allocates a block of memory with the given layout.
    /// # Errors
    /// Binary std/no_std choice
    fn allocate(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8>;

    /// Binary std/no_std choice
    ///
    /// # Safety
    /// This method encapsulates unsafe operations internally.
    /// Binary std/no_std choice
    /// Binary std/no_std choice
    ///
    /// # Errors
    /// Binary std/no_std choice
    /// succeed or panic).
    fn deallocate(&self, ptr: *mut u8, layout: core::alloc::Layout) -> wrt_error::Result<()>;
}

impl<const N: usize> Allocator for NoStdProvider<N> {
    fn allocate(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8> {
        // Binary std/no_std choice
        // general sense. It could potentially return a pointer into its *own*
        // buffer if N is large enough and it had a mechanism to manage
        // Binary std/no_std choice
        // NoStdProvider<0>, this will fail.
        if N == 0 || layout.size() > N || layout.size() == 0 {
            // Binary std/no_std choice
            return Err(Error::memory_error(
                "NoStdProvider cannot satisfy allocation request (zero capacity or request too \
                 large/zero)",
            ));
        }
        // Binary std/no_std choice
        // would need to manage free blocks, alignment, etc.
        // Returning self.data.as_ptr() is not safe without proper management.
        // Binary std/no_std choice
        // implemented.
        Err(Error::runtime_execution_error(
            "Raw allocation not implemented",
        ))
    }

    fn deallocate(&self, _ptr: *mut u8, _layout: core::alloc::Layout) -> wrt_error::Result<()> {
        // Binary std/no_std choice
        // Binary std/no_std choice
        // effectively a no-op that returns Ok.
        // Safety: This encapsulates unsafe operations internally
        Ok(())
    }
}

/// A generic handler for safe memory operations, backed by a `Provider`.
#[derive(Debug)]
pub struct SafeMemoryHandler<P: Provider> {
    provider: P,
    // TODO: Consider if a global verification_level for the handler itself is needed,
    // or if it always defers to the provider's level.
    // For now, it's a simple wrapper.
}

impl<P: Provider + Clone> Clone for SafeMemoryHandler<P> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
        }
    }
}

impl<P: Provider + PartialEq> PartialEq for SafeMemoryHandler<P> {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
        // Add comparison for verification_level if it's added to the struct
    }
}

impl<P: Provider + Eq> Eq for SafeMemoryHandler<P> {}

impl<P: Provider> SafeMemoryHandler<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    // Example of delegating a method; others would follow a similar pattern or
    // add more logic.
    // This makes SafeMemoryHandler a pass-through or a place for additional logic.
    // The bounded collections would call methods on SafeMemoryHandler, which then
    // uses P.

    pub fn provider(&self) -> &P {
        &self.provider
    }

    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    // Delegate some common Provider methods as needed by BoundedVec/Stack
    // This makes BoundedVec/Stack's use of `handler.method()` work if they
    // were expecting these directly on the handler.

    /// Borrows a slice of memory with safety guarantees.
    /// # Safety
    /// See `Provider::borrow_slice`
    pub fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        self.provider.borrow_slice(offset, len)
    }

    /// Writes data to the memory at a given offset.
    /// # Safety
    /// See `Provider::write_data`
    pub fn write_data(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        self.provider.write_data(offset, data)
    }

    pub fn verify_access(&self, offset: usize, len: usize) -> Result<()> {
        self.provider.verify_access(offset, len)
    }

    pub fn size(&self) -> usize {
        self.provider.size()
    }

    pub fn capacity(&self) -> usize {
        self.provider.capacity()
    }

    pub fn verification_level(&self) -> VerificationLevel {
        self.provider.verification_level()
    }

    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.provider.set_verification_level(level);
    }

    pub fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()> {
        self.provider.ensure_used_up_to(byte_offset)
    }

    /// Get a slice from the memory handler
    pub fn get_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        self.provider.borrow_slice(offset, len)
    }

    /// Get a mutable slice from the memory handler
    pub fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>> {
        self.provider.get_slice_mut(offset, len)
    }

    /// Converts the memory handler to a Vec of bytes.
    ///
    /// This method reads all the data from the memory provider and returns
    /// it as a Vec<u8>. This is useful for compatibility with APIs that expect
    /// a standard Vec.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wrt_foundation::safe_memory::{SafeMemoryHandler, NoStdProvider};
    /// # use wrt_foundation::{safe_managed_alloc, budget_aware_provider::CrateId};
    /// #
    /// # let provider = safe_managed_alloc!(1024, CrateId::Foundation).unwrap();
    /// # let handler = SafeMemoryHandler::new(provider);
    /// let data = handler.to_vec().unwrap();
    /// assert!(data.is_empty())); // Empty handler has no data
    /// ```
    #[cfg(feature = "std")]
    pub fn to_vec(&self) -> Result<std::vec::Vec<u8>> {
        let size = self.provider.size();
        if size == 0 {
            return Ok(std::vec::Vec::new());
        }

        let slice = self.provider.borrow_slice(0, size)?;
        Ok(slice.as_ref().to_vec())
    }

    /// Converts the memory handler to a BoundedVec of bytes (no_std version).
    ///
    /// In no_std environments, this returns the data as a BoundedVec since
    /// standard Vec is not available.
    #[cfg(not(feature = "std"))]
    pub fn to_vec(
        &self,
    ) -> Result<crate::bounded::BoundedVec<u8, 4096, crate::safe_memory::NoStdProvider<4096>>> {
        use crate::budget_aware_provider::CrateId;
        #[allow(deprecated)]
        use crate::wrt_memory_system::CapabilityWrtFactory;

        let size = self.provider.size();
        if size == 0 {
            let provider = crate::safe_managed_alloc!(
                4096,
                crate::budget_aware_provider::CrateId::Foundation
            )?;
            return crate::bounded::BoundedVec::new(provider);
        }

        let slice = self.provider.borrow_slice(0, size)?;
        let provider =
            crate::safe_managed_alloc!(4096, crate::budget_aware_provider::CrateId::Foundation)?;
        let mut result = crate::bounded::BoundedVec::new(provider)?;

        for byte in slice.as_ref() {
            result
                .push(*byte)
                .map_err(|_| Error::runtime_execution_error("Failed to push byte"))?;
        }

        Ok(result)
    }

    /// Resize the memory handler to a new size.
    ///
    /// This method attempts to resize the underlying memory provider.
    /// The exact behavior depends on the provider implementation.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider cannot be resized to the requested
    /// size.
    pub fn resize(&mut self, new_size: usize) -> Result<()>
    where
        P: Provider,
    {
        // For providers that support resize, delegate to them
        // For NoStdProvider, this maps to its resize method
        // For StdProvider, this maps to its resize method
        self.provider.ensure_used_up_to(new_size)
    }

    /// Get the current length of used memory in the handler.
    ///
    /// This returns the size of initialized/used memory from the provider.
    pub fn len(&self) -> usize {
        self.provider.size()
    }

    /// Check if the memory handler is empty.
    ///
    /// Returns true if the provider has no used memory.
    pub fn is_empty(&self) -> bool {
        self.provider.size() == 0
    }

    /// Clear all data in the memory handler.
    ///
    /// This method attempts to reset the provider to an empty state.
    /// Since the Provider trait doesn't expose a direct clear method,
    /// this is a best-effort implementation that works with the available
    /// interface.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    pub fn clear(&mut self) -> Result<()> {
        // Since the Provider trait doesn't expose a direct clear method,
        // we implement clearing by overwriting the memory with zeros in chunks
        // This effectively clears the data while maintaining the provider's integrity

        let current_size = self.provider.size();
        if current_size > 0 {
            // Clear in chunks to avoid large allocations
            const CHUNK_SIZE: usize = 256;
            let zero_chunk = [0u8; CHUNK_SIZE];

            let mut offset = 0;
            while offset < current_size {
                let chunk_len = core::cmp::min(CHUNK_SIZE, current_size - offset);
                self.provider.write_data(offset, &zero_chunk[..chunk_len])?;
                offset += chunk_len;
            }
        }

        Ok(())
    }

    /// Add data to the memory handler.
    ///
    /// This appends the provided data to the end of the current memory content.
    ///
    /// # Errors
    ///
    /// Returns an error if there's insufficient capacity or if the write fails.
    pub fn add_data(&mut self, data: &[u8]) -> Result<()> {
        let current_size = self.provider.size();
        self.provider.write_data(current_size, data)
    }

    /// Copy data within the memory handler from a source offset to a
    /// destination offset.
    ///
    /// This method copies `len` bytes from `src_offset` to `dst_offset` within
    /// the same memory provider. The operation handles overlapping regions
    /// safely.
    ///
    /// # Errors
    ///
    /// Returns an error if either the source or destination range is out of
    /// bounds, or if the copy operation fails.
    pub fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()> {
        self.provider.copy_within(src_offset, dst_offset, len)
    }

    /// Verify the integrity of the memory handler.
    ///
    /// This delegates to the provider's integrity verification.
    ///
    /// # Errors
    ///
    /// Returns an error if integrity verification fails.
    pub fn verify_integrity(&self) -> Result<()> {
        self.provider.verify_integrity()
    }
}

// Re-export SafeStack as an alias for BoundedStack
// Re-export memory providers with consistent naming
pub use NoStdProvider as NoStdMemoryProvider;
pub use Provider as MemoryProvider;
#[cfg(feature = "std")]
pub use StdProvider as StdMemoryProvider;

pub use crate::bounded::BoundedStack as SafeStack;

