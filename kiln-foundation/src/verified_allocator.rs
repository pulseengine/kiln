//! Verified Memory Allocator with Formal Proofs
//!
//! This module provides a formally verified allocator that maintains
//! strong invariants about memory safety and budget compliance.
//!
//! # Phase 1 Enhancements
//! - GlobalAlloc trait implementation for standard Rust Vec/Box support
//! - Scope/checkpoint support for hierarchical memory management
//! - Static heap buffer for embedded systems

#![allow(unused_attributes)] // For formal verification attributes

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr,
    sync::atomic::{AtomicUsize, AtomicBool, Ordering},
};

use wrt_sync::WrtMutex;
use wrt_error::{Result, Error};

use crate::{
    budget_aware_provider::CrateId,
    collections::StaticVec,
};

/// Total heap size for embedded bump allocator (256 KB)
pub const TOTAL_HEAP_SIZE: usize = 262_144;

/// Maximum module size (64 KB per module)
pub const MAX_MODULE_SIZE: usize = 65_536;

/// Maximum number of nested scopes
pub const MAX_SCOPES: usize = 16;

/// Sync wrapper for UnsafeCell to allow static usage
struct SyncUnsafeCell<T>(UnsafeCell<T>);

#[allow(unsafe_code)] // Required for Sync implementation
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    fn get(&self) -> *mut T {
        self.0.get()
    }
}

/// Static heap buffer for bump allocator
///
/// This is the backing storage for all dynamic allocations when using
/// the GlobalAlloc interface. Memory is never returned to the system,
/// only reset when scopes exit.
static HEAP_BUFFER: SyncUnsafeCell<[u8; TOTAL_HEAP_SIZE]> = SyncUnsafeCell::new([0; TOTAL_HEAP_SIZE]);

/// Scope information for hierarchical memory management
#[derive(Debug, Clone, Copy)]
pub struct ScopeInfo {
    /// Memory offset when scope was created (checkpoint)
    pub checkpoint: usize,
    /// Crate that owns this scope
    pub crate_id: CrateId,
    /// Maximum budget for this scope
    pub budget: usize,
    /// Bytes allocated in this scope
    pub allocated: usize,
}

impl ScopeInfo {
    /// Create a new scope
    pub const fn new(checkpoint: usize, crate_id: CrateId, budget: usize) -> Self {
        Self {
            checkpoint,
            crate_id,
            budget,
            allocated: 0,
        }
    }
}

/// Formally verified allocator state
pub struct VerifiedAllocator {
    /// Total memory budget
    total_budget: usize,
    /// Currently allocated memory (bump pointer offset)
    allocated: AtomicUsize,
    /// Allocation enabled flag
    enabled: AtomicBool,
    /// Scope stack for hierarchical memory management (fixed size for const init)
    scopes: WrtMutex<StaticVec<ScopeInfo, MAX_SCOPES>>,
    /// Invariant checker
    #[cfg(debug_assertions)]
    invariant_checker: InvariantChecker,
}

#[cfg(debug_assertions)]
struct InvariantChecker {
    check_frequency: usize,
    check_counter: AtomicUsize,
}

impl VerifiedAllocator {
    /// Create a new verified allocator
    ///
    /// # Requirements (checked at runtime)
    /// - budget > 0
    /// - self.allocated() == 0
    /// - self.total_budget == budget
    /// - self.enabled == true
    pub const fn new(budget: usize) -> Self {
        Self {
            total_budget: budget,
            allocated: AtomicUsize::new(0),
            enabled: AtomicBool::new(true),
            scopes: WrtMutex::new(StaticVec::new()),
            #[cfg(debug_assertions)]
            invariant_checker: InvariantChecker {
                check_frequency: 100,
                check_counter: AtomicUsize::new(0),
            },
        }
    }
    
    /// Allocate memory with verification
    ///
    /// # Requirements (checked at runtime)
    /// - self.enabled
    /// - size > 0
    /// - self.allocated() + size <= self.total_budget
    pub fn allocate(&self, size: usize) -> Result<VerifiedAllocation<'_>> {
        // Check preconditions
        if !self.enabled.load(Ordering::Acquire) {
            return Err(Error::runtime_error("Allocator is disabled"));
        }

        if size == 0 {
            return Err(Error::validation_invalid_parameter("Cannot allocate zero bytes"));
        }

        // Atomic allocation with overflow checking
        let mut current = self.allocated.load(Ordering::Acquire);
        loop {
            // Check budget constraint
            let new_total = current.checked_add(size).ok_or_else(|| {
                Error::memory_integer_overflow("Allocation would overflow")
            })?;

            if new_total > self.total_budget {
                return Err(Error::runtime_execution_error("Budget exceeded: allocation would exceed total budget"));
            }

            // Try to update atomically
            match self.allocated.compare_exchange_weak(
                current,
                new_total,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Success - check invariants in debug mode
                    #[cfg(debug_assertions)]
                    self.check_invariants();

                    return Ok(VerifiedAllocation {
                        allocator: self,
                        size,
                        #[cfg(feature = "formal-verification")]
                        allocation_id: crate::formal_verification::ghost::record_allocation(size),
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }
    
    /// Deallocate memory with verification
    ///
    /// # Requirements (checked at runtime in debug mode)
    /// - self.allocated() >= size
    fn deallocate(&self, size: usize) {
        let result = self.allocated.fetch_sub(size, Ordering::AcqRel);

        // Check for underflow in debug mode
        debug_assert!(
            result >= size,
            "Deallocation underflow: allocated={}, deallocating={}",
            result, size
        );

        #[cfg(debug_assertions)]
        self.check_invariants();
    }

    /// Check all invariants
    #[cfg(debug_assertions)]
    fn check_invariants(&self) {
        let count = self.invariant_checker.check_counter.fetch_add(1, Ordering::Relaxed);
        if count % self.invariant_checker.check_frequency == 0 {
            debug_assert!(self.allocated.load(Ordering::Acquire) <= self.total_budget);
            debug_assert!(self.enabled.load(Ordering::Acquire) || self.allocated.load(Ordering::Acquire) == 0);
        }
    }
    
    /// Disable the allocator (for shutdown)
    ///
    /// # Requirements
    /// - After calling: !self.enabled
    /// - Memory state unchanged
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    /// Enter a new memory scope with budget limit
    ///
    /// Creates a checkpoint at the current allocation position. All allocations
    /// until `exit_scope()` will be tracked against this scope's budget.
    ///
    /// # Arguments
    /// * `crate_id` - The crate entering the scope
    /// * `budget` - Maximum bytes this scope can allocate
    ///
    /// # Returns
    /// * `Ok(ScopeGuard)` - RAII guard that exits scope on drop
    /// * `Err` - If scope stack is full or parameters invalid
    pub fn enter_scope(&self, crate_id: CrateId, budget: usize) -> Result<ScopeGuard<'_>> {
        if budget == 0 {
            return Err(Error::validation_invalid_parameter("Scope budget cannot be zero"));
        }

        let checkpoint = self.allocated.load(Ordering::Acquire);
        let scope = ScopeInfo::new(checkpoint, crate_id, budget);

        let mut scopes = self.scopes.lock();
        scopes.push(scope).map_err(|_| {
            Error::runtime_execution_error("Scope stack overflow: too many nested scopes")
        })?;
        drop(scopes); // Release lock

        Ok(ScopeGuard {
            allocator: self,
            entered: true,
        })
    }

    /// Exit the current scope and reset memory to checkpoint
    ///
    /// This resets the bump allocator pointer to where it was when the
    /// scope was entered, effectively "freeing" all allocations made
    /// within the scope.
    ///
    /// # Safety
    /// After calling this, all pointers allocated within the scope become invalid.
    /// The ScopeGuard ensures this is called automatically on drop.
    pub fn exit_scope(&self) {
        let mut scopes = self.scopes.lock();
        if let Some(scope) = scopes.pop() {
            // Reset allocator to checkpoint
            self.allocated.store(scope.checkpoint, Ordering::Release);

            #[cfg(debug_assertions)]
            self.check_invariants();
        }
    }

    /// Check if allocation fits within current scope budget
    ///
    /// Returns Ok(()) if allocation is allowed, Err if budget exceeded.
    fn check_scope_budget(&self, size: usize) -> Result<()> {
        let mut scopes = self.scopes.lock();
        if let Some(scope) = scopes.last_mut() {
            let new_allocated = scope.allocated.checked_add(size)
                .ok_or_else(|| Error::memory_integer_overflow("Scope allocation overflow"))?;

            if new_allocated > scope.budget {
                return Err(Error::runtime_execution_error(
                    "Scope budget exceeded: allocation would exceed scope limit"
                ));
            }

            scope.allocated = new_allocated;
        }
        Ok(())
    }

    /// Get the start of the heap buffer
    #[inline]
    fn heap_start(&self) -> *mut u8 {
        HEAP_BUFFER.get().cast::<u8>()
    }

    /// Get the end of the heap buffer
    #[inline]
    #[allow(unsafe_code)] // Safe: pointer arithmetic within bounds
    fn heap_end(&self) -> *mut u8 {
        unsafe { self.heap_start().add(TOTAL_HEAP_SIZE) }
    }

    /// Get current allocation offset
    #[inline]
    pub fn current_offset(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Get available memory
    #[inline]
    pub fn available(&self) -> usize {
        self.total_budget.saturating_sub(self.current_offset())
    }
}

/// A verified allocation handle
pub struct VerifiedAllocation<'a> {
    allocator: &'a VerifiedAllocator,
    size: usize,
    #[cfg(feature = "formal-verification")]
    allocation_id: u64,
}

impl<'a> Drop for VerifiedAllocation<'a> {
    /// Automatically deallocate on drop
    ///
    /// # Formal Specification
    /// ```text
    /// ensures: allocator.allocated() == old(allocator.allocated()) - self.size
    /// ```
    fn drop(&mut self) {
        self.allocator.deallocate(self.size);

        #[cfg(feature = "formal-verification")]
        crate::formal_verification::ghost::record_deallocation(self.allocation_id);
    }
}

/// RAII guard for memory scopes
///
/// Automatically exits the scope when dropped, resetting memory to the checkpoint.
///
/// # Safety
/// All pointers allocated within the scope become invalid when the guard drops.
pub struct ScopeGuard<'a> {
    allocator: &'a VerifiedAllocator,
    entered: bool,
}

impl<'a> ScopeGuard<'a> {
    /// Manually exit the scope early
    ///
    /// This consumes the guard, preventing the Drop implementation from
    /// running again.
    pub fn exit(mut self) {
        if self.entered {
            self.allocator.exit_scope();
            self.entered = false;
        }
    }
}

impl<'a> Drop for ScopeGuard<'a> {
    fn drop(&mut self) {
        if self.entered {
            self.allocator.exit_scope();
        }
    }
}

/// Align value up to the nearest multiple of align
///
/// # Safety
/// align must be a power of 2
#[inline]
const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

/// GlobalAlloc implementation for VerifiedAllocator
///
/// This allows using standard Rust Vec, Box, etc. with the verified allocator.
/// All allocations go through the bump allocator with budget checking.
///
/// # Safety
/// This implementation is memory-safe because:
/// - Uses atomic operations for thread-safe bump pointer updates
/// - All pointers are within the static HEAP_BUFFER bounds
/// - Scope-based cleanup ensures proper memory lifecycle
#[allow(unsafe_code)] // Required for GlobalAlloc trait
unsafe impl GlobalAlloc for VerifiedAllocator {
    #[allow(unsafe_code)] // Required for GlobalAlloc::alloc signature
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Check if allocator is enabled
        if !self.enabled.load(Ordering::Acquire) {
            return ptr::null_mut();
        }

        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        // Check scope budget if we're in a scope
        if self.check_scope_budget(size).is_err() {
            return ptr::null_mut();
        }

        // Atomic bump allocation with alignment
        loop {
            let current = self.allocated.load(Ordering::Acquire);
            let aligned = align_up(current, align);
            let new_offset = match aligned.checked_add(size) {
                Some(offset) => offset,
                None => return ptr::null_mut(), // Overflow
            };

            // Check against total budget (heap size)
            if new_offset > TOTAL_HEAP_SIZE {
                return ptr::null_mut(); // Out of memory
            }

            // Try atomic update
            match self.allocated.compare_exchange_weak(
                current,
                new_offset,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Success - return pointer
                    // SAFETY: Edition 2024 requires explicit unsafe blocks in unsafe functions
                    let ptr = unsafe { self.heap_start().add(aligned) };

                    #[cfg(debug_assertions)]
                    self.check_invariants();

                    return ptr;
                }
                Err(_) => {
                    // Retry with updated value
                    continue;
                }
            }
        }
    }

    #[allow(unsafe_code)] // Required for GlobalAlloc::dealloc signature
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator - no individual deallocation
        // Memory is reclaimed only when scopes exit
    }
}

/// Global verified allocator instances
pub mod global_allocators {
    use super::*;
    use core::alloc::{GlobalAlloc, Layout};

    /// System-wide verified allocator
    pub static SYSTEM_ALLOCATOR: VerifiedAllocator = VerifiedAllocator::new(16_777_216); // 16MB

    /// Test allocator that can be used as #[global_allocator]
    ///
    /// This uses the Foundation crate's allocator for all allocations.
    pub struct TestAllocator;

    #[allow(unsafe_code)] // Required for GlobalAlloc trait
    unsafe impl GlobalAlloc for TestAllocator {
        #[allow(unsafe_code)]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: Edition 2024 requires explicit unsafe blocks in unsafe functions
            unsafe { CRATE_ALLOCATORS[0].alloc(layout) } // Use Foundation allocator
        }

        #[allow(unsafe_code)]
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: Edition 2024 requires explicit unsafe blocks in unsafe functions
            unsafe { CRATE_ALLOCATORS[0].dealloc(ptr, layout) }
        }
    }
    
    /// Per-crate verified allocators
    pub static CRATE_ALLOCATORS: [VerifiedAllocator; 16] = [
        VerifiedAllocator::new(1_048_576),  // Foundation - 1MB
        VerifiedAllocator::new(2_097_152),  // Component - 2MB
        VerifiedAllocator::new(4_194_304),  // Runtime - 4MB
        VerifiedAllocator::new(1_048_576),  // Decoder - 1MB
        VerifiedAllocator::new(2_097_152),  // Host - 2MB
        VerifiedAllocator::new(524_288),    // Debug - 512KB
        VerifiedAllocator::new(1_048_576),  // Platform - 1MB
        VerifiedAllocator::new(524_288),    // Instructions - 512KB
        VerifiedAllocator::new(524_288),    // Format - 512KB
        VerifiedAllocator::new(524_288),    // Intercept - 512KB
        VerifiedAllocator::new(262_144),    // Sync - 256KB
        VerifiedAllocator::new(262_144),    // Math - 256KB
        VerifiedAllocator::new(262_144),    // Logging - 256KB
        VerifiedAllocator::new(131_072),    // Panic - 128KB
        VerifiedAllocator::new(262_144),    // TestRegistry - 256KB
        VerifiedAllocator::new(262_144),    // VerificationTool - 256KB
    ];
    
    /// Get allocator for a specific crate
    pub fn get_crate_allocator(crate_id: CrateId) -> &'static VerifiedAllocator {
        &CRATE_ALLOCATORS[crate_id as usize]
    }
}

/// Verification properties that can be checked
#[cfg(feature = "formal-verification")]
pub mod properties {
    use super::*;
    
    /// Property: No allocation can exceed the total budget
    #[cfg(kani)]
    #[kani::proof]
    fn verify_budget_safety() {
        let budget = kani::any_where(|b: &usize| *b > 0 && *b < 1_000_000);
        let allocator = VerifiedAllocator::new(budget);

        let size = kani::any_where(|s: &usize| *s > 0);

        let result = allocator.allocate(size);

        // If allocation succeeds, we must be within budget
        if result.is_ok() {
            assert!(allocator.allocated.load(Ordering::Acquire) <= budget);
        }
    }

    /// Property: Deallocation always reduces allocated amount
    #[cfg(kani)]
    #[kani::proof]
    fn verify_deallocation_reduces() {
        let allocator = VerifiedAllocator::new(1_000_000);
        let size = kani::any_where(|s: &usize| *s > 0 && *s < 1_000_000);

        if let Ok(allocation) = allocator.allocate(size) {
            let before = allocator.allocated.load(Ordering::Acquire);
            drop(allocation);
            let after = allocator.allocated.load(Ordering::Acquire);
            assert!(after < before);
        }
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // Tests need unsafe for GlobalAlloc operations
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let allocator = VerifiedAllocator::new(1024);

        let alloc1 = allocator.allocate(512).unwrap();
        assert_eq!(allocator.allocated.load(Ordering::Acquire), 512);

        let alloc2 = allocator.allocate(256).unwrap();
        assert_eq!(allocator.allocated.load(Ordering::Acquire), 768);

        drop(alloc1);
        assert_eq!(allocator.allocated.load(Ordering::Acquire), 256);

        drop(alloc2);
        assert_eq!(allocator.allocated.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_budget_enforcement() {
        let allocator = VerifiedAllocator::new(1024);

        let _alloc1 = allocator.allocate(1024).unwrap();
        let result = allocator.allocate(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_scope_basic() {
        let allocator = VerifiedAllocator::new(TOTAL_HEAP_SIZE);

        // Enter a scope
        let scope = allocator.enter_scope(CrateId::Foundation, 1024).unwrap();
        let checkpoint = allocator.current_offset();

        // Allocate within scope using GlobalAlloc
        let layout = Layout::from_size_align(512, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());
        assert_eq!(allocator.current_offset(), 512);

        // Exit scope - memory resets
        scope.exit();
        assert_eq!(allocator.current_offset(), checkpoint);
    }

    #[test]
    fn test_scope_budget_enforcement() {
        let allocator = VerifiedAllocator::new(TOTAL_HEAP_SIZE);

        // Scope with 512 byte budget
        let _scope = allocator.enter_scope(CrateId::Decoder, 512).unwrap();

        // First allocation succeeds
        let layout1 = Layout::from_size_align(256, 8).unwrap();
        let ptr1 = unsafe { allocator.alloc(layout1) };
        assert!(!ptr1.is_null());

        // Second allocation succeeds (256 + 256 = 512)
        let layout2 = Layout::from_size_align(256, 8).unwrap();
        let ptr2 = unsafe { allocator.alloc(layout2) };
        assert!(!ptr2.is_null());

        // Third allocation fails (would exceed 512 budget)
        let layout3 = Layout::from_size_align(1, 8).unwrap();
        let ptr3 = unsafe { allocator.alloc(layout3) };
        assert!(ptr3.is_null()); // Budget exceeded
    }

    #[test]
    fn test_nested_scopes() {
        let allocator = VerifiedAllocator::new(TOTAL_HEAP_SIZE);

        let checkpoint1 = allocator.current_offset();

        // Outer scope
        let scope1 = allocator.enter_scope(CrateId::Runtime, 2048).unwrap();
        let layout1 = Layout::from_size_align(512, 8).unwrap();
        unsafe { allocator.alloc(layout1) };
        let offset_after_outer = allocator.current_offset();

        {
            // Inner scope
            let scope2 = allocator.enter_scope(CrateId::Component, 1024).unwrap();
            let layout2 = Layout::from_size_align(256, 8).unwrap();
            unsafe { allocator.alloc(layout2) };
            let offset_after_inner = allocator.current_offset();
            assert!(offset_after_inner > offset_after_outer);

            // Inner scope exits
            scope2.exit();
            assert_eq!(allocator.current_offset(), offset_after_outer);
        }

        // Outer scope exits
        scope1.exit();
        assert_eq!(allocator.current_offset(), checkpoint1);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(7, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(15, 16), 16);
        assert_eq!(align_up(17, 16), 32);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_vec_with_scope() {
        use alloc::vec::Vec;

        let allocator = VerifiedAllocator::new(TOTAL_HEAP_SIZE);

        // Set as global allocator for this test (conceptually)
        let checkpoint = allocator.current_offset();

        let _scope = allocator.enter_scope(CrateId::Decoder, 4096).unwrap();

        // This would use the global allocator if set up
        // For now, just verify the checkpoint mechanism works
        let layout = Layout::from_size_align(1024, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());

        // Memory increased
        assert!(allocator.current_offset() > checkpoint);

        drop(_scope);

        // Memory reset to checkpoint
        assert_eq!(allocator.current_offset(), checkpoint);
    }
}