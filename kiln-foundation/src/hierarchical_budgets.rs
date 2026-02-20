//! Hierarchical Budget System
//!
//! This module provides a hierarchical budget allocation system that allows
//! complex memory management with sub-budgets and priority-based allocation.
//!
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! SW-REQ-ID: REQ_RESOURCE_002 - Resource limits

use core::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use wrt_error::{
    helpers::memory_limit_exceeded_error,
    Result,
};

// Capability-based imports
use crate::wrt_memory_system::CapabilityWrtFactory;
use crate::{
    budget_aware_provider::CrateId,
    capabilities::{
        MemoryCapability,
        MemoryCapabilityContext,
        StaticMemoryCapability,
    },
    codes,
    memory_coordinator::CrateIdentifier,
    safe_memory::NoStdProvider,
    verification::VerificationLevel,
    Error,
    ErrorCategory,
};

/// Priority levels for memory allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPriority {
    /// Critical allocations that must succeed
    Critical = 0,
    /// High priority allocations
    High     = 1,
    /// Normal priority allocations
    Normal   = 2,
    /// Low priority allocations (can be evicted)
    Low      = 3,
}

/// Sub-budget within a crate's allocation
#[derive(Debug)]
pub struct SubBudget {
    /// Name of this sub-budget
    pub name:           &'static str,
    /// Maximum allocation for this sub-budget
    pub max_allocation: usize,
    /// Currently allocated bytes
    pub allocated:      AtomicUsize,
    /// Priority of allocations in this sub-budget
    pub priority:       MemoryPriority,
}

impl SubBudget {
    /// Create a new sub-budget
    pub const fn new(name: &'static str, max_allocation: usize, priority: MemoryPriority) -> Self {
        Self {
            name,
            max_allocation,
            allocated: AtomicUsize::new(0),
            priority,
        }
    }

    /// Try to allocate from this sub-budget
    pub fn try_allocate(&self, size: usize) -> Result<()> {
        let current = self.allocated.load(Ordering::Acquire);
        let new_total = current
            .checked_add(size)
            .ok_or_else(|| Error::memory_error("Allocation would overflow"))?;

        if new_total > self.max_allocation {
            return Err(memory_limit_exceeded_error("Sub-budget exceeded"));
        }

        // Try atomic update
        match self.allocated.compare_exchange_weak(
            current,
            new_total,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::runtime_execution_error(
                "Failed to update sub-budget allocation: race condition",
            )),
        }
    }

    /// Return allocation to this sub-budget
    pub fn deallocate(&self, size: usize) -> Result<()> {
        let current = self.allocated.load(Ordering::Acquire);
        if current < size {
            return Err(Error::new(
                ErrorCategory::Runtime,
                codes::INVALID_STATE,
                "Attempted to deallocate more than was allocated",
            ));
        }

        self.allocated.store(current - size, Ordering::Release);
        Ok(())
    }

    /// Get current allocation
    pub fn current_allocation(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Get available space
    pub fn available(&self) -> usize {
        self.max_allocation.saturating_sub(self.current_allocation())
    }
}

/// Hierarchical budget manager for a crate
pub struct HierarchicalBudget<const MAX_SUB_BUDGETS: usize> {
    /// Parent crate ID
    pub crate_id:     CrateId,
    /// Total budget for this crate
    pub total_budget: usize,
    /// Sub-budgets within this crate
    pub sub_budgets:  [Option<SubBudget>; MAX_SUB_BUDGETS],
    /// Number of active sub-budgets
    pub active_count: AtomicUsize,
}

impl<const MAX_SUB_BUDGETS: usize> HierarchicalBudget<MAX_SUB_BUDGETS> {
    /// Create a new hierarchical budget
    pub const fn new(crate_id: CrateId, total_budget: usize) -> Self {
        Self {
            crate_id,
            total_budget,
            sub_budgets: [const { None }; MAX_SUB_BUDGETS],
            active_count: AtomicUsize::new(0),
        }
    }

    /// Add a sub-budget
    pub fn add_sub_budget(
        &mut self,
        name: &'static str,
        allocation: usize,
        priority: MemoryPriority,
    ) -> Result<usize> {
        if allocation > self.total_budget {
            return Err(memory_limit_exceeded_error(
                "Sub-budget exceeds total budget",
            ));
        }

        let count = self.active_count.load(Ordering::Acquire);
        if count >= MAX_SUB_BUDGETS {
            return Err(memory_limit_exceeded_error("Maximum sub-budgets reached"));
        }

        // Find empty slot
        for (i, slot) in self.sub_budgets.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(SubBudget::new(name, allocation, priority));
                self.active_count.store(count + 1, Ordering::Release);
                return Ok(i);
            }
        }

        Err(Error::runtime_execution_error(
            "No available sub-budget slots",
        ))
    }

    /// Allocate from the best available sub-budget
    pub fn allocate_prioritized<const N: usize>(
        &self,
        size: usize,
        min_priority: MemoryPriority,
    ) -> Result<(crate::safe_memory::NoStdProvider<N>, usize)> {
        // Find the best sub-budget based on priority and availability
        let mut best_idx = None;
        let mut best_priority = MemoryPriority::Low;

        for (i, sub_budget) in self.sub_budgets.iter().enumerate() {
            if let Some(budget) = sub_budget {
                if budget.priority <= min_priority
                    && budget.available() >= size
                    && (best_idx.is_none() || budget.priority < best_priority)
                {
                    best_idx = Some(i);
                    best_priority = budget.priority;
                }
            }
        }

        let idx = best_idx.ok_or_else(|| {
            memory_limit_exceeded_error(
                "No suitable sub-budget found with sufficient space and priority",
            )
        })?;

        // Try to allocate from the selected sub-budget
        if let Some(sub_budget) = &self.sub_budgets[idx] {
            sub_budget.try_allocate(size)?;

            // Create capability-guarded provider through the WRT factory
            let guard = CapabilityWrtFactory::create_provider::<N>(self.crate_id)?;

            Ok((guard, idx))
        } else {
            Err(Error::runtime_execution_error(
                "Sub-budget index became invalid during allocation",
            ))
        }
    }

    /// Deallocate from a specific sub-budget
    pub fn deallocate(&self, sub_budget_idx: usize, size: usize) -> Result<()> {
        if sub_budget_idx >= MAX_SUB_BUDGETS {
            return Err(Error::new(
                ErrorCategory::Capacity,
                codes::OUT_OF_BOUNDS_ERROR,
                "Sub-budget index out of bounds",
            ));
        }

        if let Some(sub_budget) = &self.sub_budgets[sub_budget_idx] {
            sub_budget.deallocate(size)
        } else {
            Err(Error::runtime_execution_error(
                "No sub-budget at specified index",
            ))
        }
    }

    /// Get statistics for all sub-budgets
    pub fn get_statistics(&self) -> HierarchicalStats {
        let mut stats = HierarchicalStats {
            total_budget:       self.total_budget,
            total_allocated:    0,
            sub_budget_count:   0,
            critical_allocated: 0,
            high_allocated:     0,
            normal_allocated:   0,
            low_allocated:      0,
        };

        for sub_budget in &self.sub_budgets {
            if let Some(budget) = sub_budget {
                let allocated = budget.current_allocation();
                stats.total_allocated += allocated;
                stats.sub_budget_count += 1;

                match budget.priority {
                    MemoryPriority::Critical => stats.critical_allocated += allocated,
                    MemoryPriority::High => stats.high_allocated += allocated,
                    MemoryPriority::Normal => stats.normal_allocated += allocated,
                    MemoryPriority::Low => stats.low_allocated += allocated,
                }
            }
        }

        stats
    }
}

/// Statistics for hierarchical budget system
#[derive(Debug, Clone)]
pub struct HierarchicalStats {
    pub total_budget:       usize,
    pub total_allocated:    usize,
    pub sub_budget_count:   usize,
    pub critical_allocated: usize,
    pub high_allocated:     usize,
    pub normal_allocated:   usize,
    pub low_allocated:      usize,
}

impl HierarchicalStats {
    /// Get utilization percentage
    pub fn utilization_percent(&self) -> f32 {
        if self.total_budget == 0 {
            0.0
        } else {
            (self.total_allocated as f32 / self.total_budget as f32) * 100.0
        }
    }

    /// Get available budget
    pub fn available(&self) -> usize {
        self.total_budget.saturating_sub(self.total_allocated)
    }
}

/// Hierarchical memory guard that tracks sub-budget
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct HierarchicalGuard<const N: usize> {
    guard:          crate::capabilities::CapabilityGuardedProvider<N>,
    sub_budget_idx: usize,
    budget:         *const HierarchicalBudget<16>, // Max 16 sub-budgets
}

/// Hierarchical memory guard that tracks sub-budget (no_std version)
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct HierarchicalGuard<const N: usize> {
    guard:          crate::safe_memory::NoStdProvider<N>,
    sub_budget_idx: usize,
    budget:         *const HierarchicalBudget<16>, // Max 16 sub-budgets
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const N: usize> HierarchicalGuard<N> {
    /// Create a new hierarchical guard
    pub fn new(
        guard: crate::capabilities::CapabilityGuardedProvider<N>,
        sub_budget_idx: usize,
        budget: *const HierarchicalBudget<16>,
    ) -> Self {
        Self {
            guard,
            sub_budget_idx,
            budget,
        }
    }

    /// Get the underlying memory guard
    pub fn guard(&self) -> &crate::capabilities::CapabilityGuardedProvider<N> {
        &self.guard
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<const N: usize> HierarchicalGuard<N> {
    /// Create a new hierarchical guard
    pub fn new(
        guard: crate::safe_memory::NoStdProvider<N>,
        sub_budget_idx: usize,
        budget: *const HierarchicalBudget<16>,
    ) -> Self {
        Self {
            guard,
            sub_budget_idx,
            budget,
        }
    }

    /// Get the underlying memory guard
    pub fn guard(&self) -> &crate::safe_memory::NoStdProvider<N> {
        &self.guard
    }
}

impl<const N: usize> Drop for HierarchicalGuard<N> {
    fn drop(&mut self) {
        // Return allocation to sub-budget
        #[allow(unsafe_code)] // Safe: pointer is valid during guard lifetime, null check performed
        unsafe {
            if !self.budget.is_null() {
                // Intentionally ignore errors in Drop to avoid panic
                let _ = (*self.budget).deallocate(self.sub_budget_idx, N);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_budget() {
        let sub_budget = SubBudget::new("test", 1024, MemoryPriority::Normal);

        assert_eq!(sub_budget.available(), 1024);

        sub_budget.try_allocate(512).unwrap();
        assert_eq!(sub_budget.current_allocation(), 512);
        assert_eq!(sub_budget.available(), 512);

        sub_budget.deallocate(256).unwrap();
        assert_eq!(sub_budget.current_allocation(), 256);
    }

    #[test]
    fn test_hierarchical_budget() {
        crate::memory_init::MemoryInitializer::initialize().unwrap();

        let mut budget = HierarchicalBudget::<4>::new(CrateId::Component, 4096);

        let idx1 = budget.add_sub_budget("critical", 1024, MemoryPriority::Critical).unwrap();
        let idx2 = budget.add_sub_budget("normal", 2048, MemoryPriority::Normal).unwrap();

        let stats = budget.get_statistics();
        assert_eq!(stats.sub_budget_count, 2);
        assert_eq!(stats.total_budget, 4096);
    }
}
