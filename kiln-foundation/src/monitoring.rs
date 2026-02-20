//! Memory System Monitoring and Telemetry
//!
//! This module provides comprehensive monitoring and telemetry for the memory
//! management system, enabling A+ grade observability.
//!
//! SW-REQ-ID: REQ_MONITOR_001 - System observability

use core::sync::atomic::{
    AtomicU64,
    AtomicUsize,
    Ordering,
};

use crate::{
    budget_aware_provider::CrateId,
    memory_coordinator::CrateIdentifier,
};

/// Global monitoring statistics
pub struct MemoryMonitor {
    /// Total allocations across all crates
    pub total_allocations:         AtomicU64,
    /// Total deallocations across all crates
    pub total_deallocations:       AtomicU64,
    /// Peak memory usage
    pub peak_usage:                AtomicUsize,
    /// Current total allocated bytes
    pub current_usage:             AtomicUsize,
    /// Number of allocation failures
    pub allocation_failures:       AtomicU64,
    /// Number of budget overruns prevented
    pub budget_overruns_prevented: AtomicU64,
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryMonitor {
    /// Create a new memory monitor
    pub const fn new() -> Self {
        Self {
            total_allocations:         AtomicU64::new(0),
            total_deallocations:       AtomicU64::new(0),
            peak_usage:                AtomicUsize::new(0),
            current_usage:             AtomicUsize::new(0),
            allocation_failures:       AtomicU64::new(0),
            budget_overruns_prevented: AtomicU64::new(0),
        }
    }

    /// Record a successful allocation
    pub fn record_allocation(&self, size: usize) {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        let new_usage = self.current_usage.fetch_add(size, Ordering::Relaxed) + size;

        // Update peak if necessary
        let mut current_peak = self.peak_usage.load(Ordering::Relaxed);
        while new_usage > current_peak {
            match self.peak_usage.compare_exchange_weak(
                current_peak,
                new_usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_peak = x,
            }
        }
    }

    /// Record a deallocation
    pub fn record_deallocation(&self, size: usize) {
        self.total_deallocations.fetch_add(1, Ordering::Relaxed);
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
    }

    /// Record an allocation failure
    pub fn record_allocation_failure(&self) {
        self.allocation_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a budget overrun prevention
    pub fn record_budget_overrun_prevented(&self) {
        self.budget_overruns_prevented.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current statistics snapshot
    pub fn get_statistics(&self) -> MemoryStatistics {
        MemoryStatistics {
            total_allocations:         self.total_allocations.load(Ordering::Relaxed),
            total_deallocations:       self.total_deallocations.load(Ordering::Relaxed),
            peak_usage:                self.peak_usage.load(Ordering::Relaxed),
            current_usage:             self.current_usage.load(Ordering::Relaxed),
            allocation_failures:       self.allocation_failures.load(Ordering::Relaxed),
            budget_overruns_prevented: self.budget_overruns_prevented.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters (useful for testing)
    pub fn reset(&self) {
        self.total_allocations.store(0, Ordering::Relaxed);
        self.total_deallocations.store(0, Ordering::Relaxed);
        self.peak_usage.store(0, Ordering::Relaxed);
        self.current_usage.store(0, Ordering::Relaxed);
        self.allocation_failures.store(0, Ordering::Relaxed);
        self.budget_overruns_prevented.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of memory statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStatistics {
    pub total_allocations:         u64,
    pub total_deallocations:       u64,
    pub peak_usage:                usize,
    pub current_usage:             usize,
    pub allocation_failures:       u64,
    pub budget_overruns_prevented: u64,
}

impl MemoryStatistics {
    /// Calculate allocation success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_allocations == 0 {
            return 1.0;
        }
        let successful = self.total_allocations - self.allocation_failures;
        successful as f64 / self.total_allocations as f64
    }

    /// Check if there are any memory leaks
    pub fn has_leaks(&self) -> bool {
        self.total_allocations > self.total_deallocations && self.current_usage > 0
    }

    /// Get memory efficiency (prevents overruns)
    pub fn efficiency_score(&self) -> f64 {
        if self.total_allocations == 0 {
            return 1.0;
        }
        1.0 - (self.budget_overruns_prevented as f64 / self.total_allocations as f64)
    }
}

/// Global memory monitor instance
pub static MEMORY_MONITOR: MemoryMonitor = MemoryMonitor::new();

/// Per-crate monitoring
pub struct CrateMonitor {
    crate_id:      CrateId,
    allocations:   AtomicU64,
    current_usage: AtomicUsize,
    peak_usage:    AtomicUsize,
}

impl CrateMonitor {
    pub const fn new(crate_id: CrateId) -> Self {
        Self {
            crate_id,
            allocations: AtomicU64::new(0),
            current_usage: AtomicUsize::new(0),
            peak_usage: AtomicUsize::new(0),
        }
    }

    pub fn record_allocation(&self, size: usize) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        let new_usage = self.current_usage.fetch_add(size, Ordering::Relaxed) + size;

        // Update peak
        let mut current_peak = self.peak_usage.load(Ordering::Relaxed);
        while new_usage > current_peak {
            match self.peak_usage.compare_exchange_weak(
                current_peak,
                new_usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_peak = x,
            }
        }

        // Also record in global monitor
        MEMORY_MONITOR.record_allocation(size);
    }

    pub fn record_deallocation(&self, size: usize) {
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
        MEMORY_MONITOR.record_deallocation(size);
    }

    pub fn get_statistics(&self) -> CrateStatistics {
        CrateStatistics {
            crate_id:      self.crate_id,
            allocations:   self.allocations.load(Ordering::Relaxed),
            current_usage: self.current_usage.load(Ordering::Relaxed),
            peak_usage:    self.peak_usage.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CrateStatistics {
    pub crate_id:      CrateId,
    pub allocations:   u64,
    pub current_usage: usize,
    pub peak_usage:    usize,
}

/// Debug tracking for development
#[cfg(debug_assertions)]
pub fn debug_track_allocation(crate_id: CrateId, size: usize, purpose: &str) {
    // In debug mode, we can add detailed tracking
    MEMORY_MONITOR.record_allocation(size);

    #[cfg(feature = "std")]
    {
        use std::{
            collections::HashMap,
            sync::{
                Mutex,
                OnceLock,
            },
        };

        static DEBUG_TRACKER: OnceLock<Mutex<HashMap<String, (usize, usize)>>> = OnceLock::new();

        let tracker = DEBUG_TRACKER.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut map) = tracker.lock() {
            let key = format!("{}:{}", crate_id.name(), purpose);
            let (count, total) = map.entry(key).or_insert((0, 0));
            *count += 1;
            *total += size;
        }
    }
}

/// Get comprehensive system report
pub fn get_system_report() -> SystemReport {
    let global_stats = MEMORY_MONITOR.get_statistics();

    SystemReport {
        global_statistics: global_stats,
        system_health:     calculate_system_health(&global_stats),
    }
}

#[derive(Debug, Clone)]
pub struct SystemReport {
    pub global_statistics: MemoryStatistics,
    pub system_health:     SystemHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemHealth {
    Excellent, // >95% success rate, no leaks
    Good,      // >90% success rate, minimal leaks
    Warning,   // >80% success rate, some issues
    Critical,  // <80% success rate or major leaks
}

fn calculate_system_health(stats: &MemoryStatistics) -> SystemHealth {
    let success_rate = stats.success_rate();
    let has_leaks = stats.has_leaks();

    if success_rate > 0.95 && !has_leaks {
        SystemHealth::Excellent
    } else if success_rate > 0.90 && stats.current_usage < stats.peak_usage / 2 {
        SystemHealth::Good
    } else if success_rate > 0.80 {
        SystemHealth::Warning
    } else {
        SystemHealth::Critical
    }
}

/// Convenience functions for monitoring
pub mod convenience {
    use super::*;

    /// Get global memory statistics
    pub fn global_stats() -> MemoryStatistics {
        MEMORY_MONITOR.get_statistics()
    }

    /// Check if system is healthy
    pub fn is_healthy() -> bool {
        matches!(
            calculate_system_health(&global_stats()),
            SystemHealth::Excellent | SystemHealth::Good
        )
    }

    /// Get success rate percentage
    pub fn success_rate_percent() -> f64 {
        global_stats().success_rate() * 100.0
    }

    /// Get current memory usage in KB
    pub fn current_usage_kb() -> f64 {
        global_stats().current_usage as f64 / 1024.0
    }

    /// Get peak memory usage in KB
    pub fn peak_usage_kb() -> f64 {
        global_stats().peak_usage as f64 / 1024.0
    }
}
