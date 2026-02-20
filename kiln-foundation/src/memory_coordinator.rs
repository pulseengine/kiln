//! Generic Memory Coordination System
//!
//! This module provides a generic, reusable memory coordination system that can
//! be used by any crate or project. It's designed to be flexible while
//! enforcing memory budgets and safety requirements.
//!
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement
//! SW-REQ-ID: REQ_MEM_003 - Automatic cleanup

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::vec::Vec;
use core::{
    marker::PhantomData,
    sync::atomic::{
        AtomicBool,
        AtomicUsize,
        Ordering,
    },
};
#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_error::{
    helpers::memory_limit_exceeded_error,
    Result,
};

use crate::{
    codes,
    Error,
    ErrorCategory,
};

/// Generic crate identifier trait
///
/// Any type can implement this to serve as a crate/module identifier
pub trait CrateIdentifier: Copy + Clone + Eq + core::hash::Hash + 'static {
    /// Get a unique numeric ID for array indexing
    fn as_index(&self) -> usize;

    /// Get a human-readable name
    fn name(&self) -> &'static str;

    /// Get the total number of possible crate IDs
    fn count() -> usize;
}

/// Generic memory coordinator that works with any CrateIdentifier
pub struct GenericMemoryCoordinator<C: CrateIdentifier, const MAX_CRATES: usize> {
    /// Per-crate allocation tracking
    crate_allocations:  [AtomicUsize; MAX_CRATES],
    /// Per-crate budgets
    crate_budgets:      [AtomicUsize; MAX_CRATES],
    /// Total system allocation
    total_allocated:    AtomicUsize,
    /// Total system budget
    total_budget:       AtomicUsize,
    /// Whether the system is initialized
    initialized:        AtomicBool,
    /// Next allocation ID
    next_allocation_id: AtomicUsize,
    /// Phantom data for crate type
    _phantom:           PhantomData<C>,
}

impl<C: CrateIdentifier, const MAX_CRATES: usize> Default
    for GenericMemoryCoordinator<C, MAX_CRATES>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C: CrateIdentifier, const MAX_CRATES: usize> GenericMemoryCoordinator<C, MAX_CRATES> {
    /// Create a new coordinator with zero budgets
    pub fn new() -> Self {
        Self {
            crate_allocations:  core::array::from_fn(|_| AtomicUsize::new(0)),
            crate_budgets:      core::array::from_fn(|_| AtomicUsize::new(0)),
            total_allocated:    AtomicUsize::new(0),
            total_budget:       AtomicUsize::new(0),
            initialized:        AtomicBool::new(false),
            next_allocation_id: AtomicUsize::new(1),
            _phantom:           PhantomData,
        }
    }

    /// Initialize the coordinator with budgets
    ///
    /// # Arguments
    /// * `budgets` - Iterator of (CrateId, budget_size) pairs
    /// * `total_budget` - Total system memory budget
    pub fn initialize<I>(&self, budgets: I, total_budget: usize) -> Result<()>
    where
        I: IntoIterator<Item = (C, usize)>,
    {
        // Check if already initialized
        if self.initialized.swap(true, Ordering::AcqRel) {
            return Err(Error::runtime_execution_error(
                "Memory coordinator already initialized",
            ));
        }

        // Set total budget
        self.total_budget.store(total_budget, Ordering::Release);

        // Set individual crate budgets
        let mut total_assigned = 0;
        for (crate_id, budget) in budgets {
            let index = crate_id.as_index();
            if index >= MAX_CRATES {
                return Err(Error::new(
                    ErrorCategory::Capacity,
                    codes::OUT_OF_BOUNDS_ERROR,
                    "Crate index exceeds maximum supported crates",
                ));
            }
            self.crate_budgets[index].store(budget, Ordering::Release);
            total_assigned += budget;
        }

        // Verify total doesn't exceed system budget
        if total_assigned > total_budget {
            self.initialized.store(false, Ordering::Release);
            return Err(memory_limit_exceeded_error(
                "Total crate budgets exceed system budget",
            ));
        }

        Ok(())
    }

    /// Register a new allocation
    pub fn register_allocation(&self, crate_id: C, size: usize) -> Result<AllocationId> {
        let index = crate_id.as_index();
        if index >= MAX_CRATES {
            return Err(Error::runtime_execution_error(
                "Crate index out of bounds for allocation registration",
            ));
        }

        // Check crate budget
        let crate_budget = self.crate_budgets[index].load(Ordering::Acquire);
        let crate_current = self.crate_allocations[index].load(Ordering::Acquire);

        if crate_current.saturating_add(size) > crate_budget {
            return Err(memory_limit_exceeded_error(
                "Crate allocation would exceed budget",
            ));
        }

        // Check total budget
        let total_current = self.total_allocated.load(Ordering::Acquire);
        let total_budget = self.total_budget.load(Ordering::Acquire);

        if total_current.saturating_add(size) > total_budget {
            return Err(memory_limit_exceeded_error("Total system budget exceeded"));
        }

        // Try to atomically update both counters
        loop {
            let current_crate = self.crate_allocations[index].load(Ordering::Acquire);
            let new_crate = current_crate.saturating_add(size);

            match self.crate_allocations[index].compare_exchange_weak(
                current_crate,
                new_crate,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update total
                    self.total_allocated.fetch_add(size, Ordering::AcqRel);

                    // Generate allocation ID
                    let id = self.next_allocation_id.fetch_add(1, Ordering::AcqRel);
                    return Ok(AllocationId(id));
                },
                Err(_) => continue, // Retry
            }
        }
    }

    /// Return an allocation to the budget
    pub fn return_allocation(
        &self,
        crate_id: C,
        _allocation_id: AllocationId,
        size: usize,
    ) -> Result<()> {
        let index = crate_id.as_index();
        if index >= MAX_CRATES {
            return Err(Error::runtime_execution_error(
                "Crate index out of bounds for allocation return",
            ));
        }

        // Atomically decrease allocations
        let previous = self.crate_allocations[index].fetch_sub(size, Ordering::AcqRel);
        if previous < size {
            // Underflow - this is a serious error
            self.crate_allocations[index].fetch_add(size, Ordering::AcqRel);
            return Err(Error::new(
                ErrorCategory::Runtime,
                codes::INVALID_STATE,
                "Allocation underflow detected - returning more memory than allocated",
            ));
        }

        // Update total
        self.total_allocated.fetch_sub(size, Ordering::AcqRel);

        Ok(())
    }

    /// Get current allocation for a crate
    pub fn get_crate_allocation(&self, crate_id: C) -> usize {
        let index = crate_id.as_index();
        if index >= MAX_CRATES {
            return 0;
        }
        self.crate_allocations[index].load(Ordering::Acquire)
    }

    /// Get budget for a crate
    pub fn get_crate_budget(&self, crate_id: C) -> usize {
        let index = crate_id.as_index();
        if index >= MAX_CRATES {
            return 0;
        }
        self.crate_budgets[index].load(Ordering::Acquire)
    }

    /// Get total system allocation
    pub fn get_total_allocation(&self) -> usize {
        self.total_allocated.load(Ordering::Acquire)
    }

    /// Get total system budget
    pub fn get_total_budget(&self) -> usize {
        self.total_budget.load(Ordering::Acquire)
    }

    /// Check if coordinator is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
}

/// Allocation identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationId(pub usize);

/// Generic budget configuration
pub struct BudgetConfig<C: CrateIdentifier> {
    pub crate_id: C,
    pub budget:   usize,
}

/// Builder for setting up memory coordination
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct MemoryCoordinatorBuilder<C: CrateIdentifier, const MAX_CRATES: usize> {
    budgets:      Vec<BudgetConfig<C>>,
    total_budget: Option<usize>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<C: CrateIdentifier, const MAX_CRATES: usize> Default
    for MemoryCoordinatorBuilder<C, MAX_CRATES>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<C: CrateIdentifier, const MAX_CRATES: usize> MemoryCoordinatorBuilder<C, MAX_CRATES> {
    pub fn new() -> Self {
        Self {
            budgets:      Vec::new(),
            total_budget: None,
        }
    }

    /// Add a crate budget
    pub fn add_crate_budget(mut self, crate_id: C, budget: usize) -> Self {
        self.budgets.push(BudgetConfig { crate_id, budget });
        self
    }

    /// Set total system budget
    pub fn total_budget(mut self, budget: usize) -> Self {
        self.total_budget = Some(budget);
        self
    }

    /// Build and initialize the coordinator
    pub fn build(self, coordinator: &GenericMemoryCoordinator<C, MAX_CRATES>) -> Result<()> {
        let total = self
            .total_budget
            .ok_or_else(|| Error::initialization_error("Total budget not specified"))?;

        coordinator.initialize(
            self.budgets.into_iter().map(|b| (b.crate_id, b.budget)),
            total,
        )
    }
}

// Re-export for convenience
pub use self::AllocationId as AllocId;

