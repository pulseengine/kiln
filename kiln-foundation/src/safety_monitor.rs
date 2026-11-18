//! Runtime safety monitoring for production deployments
//!
//! This module provides real-time monitoring of safety-critical
//! properties during runtime execution, enabling proactive detection
//! of safety violations and system health tracking.
//!
//! # Safety Requirements
//! - SW-REQ-ID: ASIL-A-MON-001
//! - Provides runtime verification of safety properties
//! - Tracks memory allocation patterns and violations
//! - Monitors capability system integrity
//! - Enables production diagnostics

// Note: no_std is configured at the crate level

use crate::{CrateId, capabilities::CapabilityMask};

/// Safety monitor for runtime verification
///
/// Tracks safety-critical metrics during runtime execution
/// to ensure system remains within safe operating parameters.
#[derive(Debug)]
pub struct SafetyMonitor {
    /// Memory allocation tracking
    allocation_monitor: AllocationMonitor,
    /// Capability violation tracking
    capability_monitor: CapabilityMonitor,
    /// Error rate monitoring
    error_monitor: ErrorMonitor,
    /// Performance degradation detection
    performance_monitor: PerformanceMonitor,
}

/// Tracks memory allocation patterns and violations
#[derive(Debug, Default)]
struct AllocationMonitor {
    /// Total allocations
    total_allocations: u64,
    /// Failed allocations
    failed_allocations: u64,
    /// Budget violations
    budget_violations: u64,
    /// Largest allocation
    peak_allocation: usize,
    /// Current allocated bytes
    current_allocated: usize,
    /// Peak allocated bytes
    peak_allocated: usize,
}

/// Monitors capability system violations
#[derive(Debug, Default)]
struct CapabilityMonitor {
    /// Unauthorized access attempts
    access_violations: u64,
    /// Invalid capability uses
    invalid_uses: u64,
    /// Capability exhaustion events
    exhaustion_events: u64,
    /// Double-free attempts
    double_free_attempts: u64,
}

/// Tracks error rates and patterns
#[derive(Debug, Default)]
struct ErrorMonitor {
    /// Errors by severity level [Critical, High, Medium, Low]
    errors_by_level: [u64; 4],
    /// Recovery success rate
    recovery_successes: u64,
    /// Unrecoverable failures
    fatal_errors: u64,
    /// Error rate window (errors in last 1000 operations)
    recent_error_count: u32,
    /// Operations counter for rate calculation
    operation_count: u64,
}

/// Monitors performance degradation
#[derive(Debug, Default)]
struct PerformanceMonitor {
    /// Slow allocation events (>threshold)
    slow_allocations: u64,
    /// Memory pressure events
    memory_pressure_events: u64,
    /// Throughput degradation
    degradation_events: u64,
    /// Allocation time threshold in microseconds
    slow_threshold_us: u64,
}

/// Safety monitoring report
#[derive(Debug, Clone)]
pub struct SafetyReport {
    /// Total memory allocations attempted
    pub total_allocations: u64,
    /// Failed allocation attempts
    pub failed_allocations: u64,
    /// Budget violation count
    pub budget_violations: u64,
    /// Capability violation count
    pub capability_violations: u64,
    /// Fatal error count
    pub fatal_errors: u64,
    /// System health score (0-100)
    pub health_score: u8,
    /// Current memory usage
    pub current_memory_bytes: usize,
    /// Peak memory usage
    pub peak_memory_bytes: usize,
    /// Recent error rate (per 1000 operations)
    pub error_rate_per_1000: u32,
}

/// Safety violation types for reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    /// Memory budget exceeded
    BudgetExceeded,
    /// Capability access denied
    CapabilityViolation,
    /// Double-free detected
    DoubleFree,
    /// Memory corruption detected
    MemoryCorruption,
    /// Performance degradation
    PerformanceDegradation,
}

impl Default for SafetyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyMonitor {
    /// Create a new safety monitor
    pub const fn new() -> Self {
        Self {
            allocation_monitor: AllocationMonitor {
                total_allocations: 0,
                failed_allocations: 0,
                budget_violations: 0,
                peak_allocation: 0,
                current_allocated: 0,
                peak_allocated: 0,
            },
            capability_monitor: CapabilityMonitor {
                access_violations: 0,
                invalid_uses: 0,
                exhaustion_events: 0,
                double_free_attempts: 0,
            },
            error_monitor: ErrorMonitor {
                errors_by_level: [0; 4],
                recovery_successes: 0,
                fatal_errors: 0,
                recent_error_count: 0,
                operation_count: 0,
            },
            performance_monitor: PerformanceMonitor {
                slow_allocations: 0,
                memory_pressure_events: 0,
                degradation_events: 0,
                slow_threshold_us: 1000, // 1ms default
            },
        }
    }

    /// Record successful allocation
    pub fn record_allocation(&mut self, size: usize) {
        self.allocation_monitor.total_allocations += 1;
        self.allocation_monitor.current_allocated += size;

        if size > self.allocation_monitor.peak_allocation {
            self.allocation_monitor.peak_allocation = size;
        }

        if self.allocation_monitor.current_allocated > self.allocation_monitor.peak_allocated {
            self.allocation_monitor.peak_allocated = self.allocation_monitor.current_allocated;
        }

        self.increment_operations();
    }

    /// Record memory deallocation
    pub fn record_deallocation(&mut self, size: usize) {
        self.allocation_monitor.current_allocated =
            self.allocation_monitor.current_allocated.saturating_sub(size);
        self.increment_operations();
    }

    /// Record failed allocation
    pub fn record_allocation_failure(&mut self, size: usize) {
        self.allocation_monitor.failed_allocations += 1;
        self.error_monitor.errors_by_level[1] += 1; // High severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Record budget violation
    pub fn record_budget_violation(&mut self, crate_id: CrateId, requested: usize, budget: usize) {
        self.allocation_monitor.budget_violations += 1;
        self.error_monitor.errors_by_level[0] += 1; // Critical severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Record capability violation
    pub fn record_capability_violation(&mut self, crate_id: CrateId) {
        self.capability_monitor.access_violations += 1;
        self.error_monitor.errors_by_level[0] += 1; // Critical severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Record double-free attempt
    pub fn record_double_free(&mut self) {
        self.capability_monitor.double_free_attempts += 1;
        self.error_monitor.errors_by_level[0] += 1; // Critical severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Record slow allocation
    pub fn record_slow_allocation(&mut self, duration_us: u64) {
        if duration_us > self.performance_monitor.slow_threshold_us {
            self.performance_monitor.slow_allocations += 1;
        }
        self.increment_operations();
    }

    /// Record memory pressure event
    pub fn record_memory_pressure(&mut self) {
        self.performance_monitor.memory_pressure_events += 1;
        self.error_monitor.errors_by_level[2] += 1; // Medium severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Record successful error recovery
    pub fn record_recovery_success(&mut self) {
        self.error_monitor.recovery_successes += 1;
        self.increment_operations();
    }

    /// Record fatal error
    pub fn record_fatal_error(&mut self) {
        self.error_monitor.fatal_errors += 1;
        self.error_monitor.errors_by_level[0] += 1; // Critical severity
        self.update_error_rate();
        self.increment_operations();
    }

    /// Get safety report
    pub fn get_safety_report(&self) -> SafetyReport {
        SafetyReport {
            total_allocations: self.allocation_monitor.total_allocations,
            failed_allocations: self.allocation_monitor.failed_allocations,
            budget_violations: self.allocation_monitor.budget_violations,
            capability_violations: self.capability_monitor.access_violations,
            fatal_errors: self.error_monitor.fatal_errors,
            health_score: self.calculate_health_score(),
            current_memory_bytes: self.allocation_monitor.current_allocated,
            peak_memory_bytes: self.allocation_monitor.peak_allocated,
            error_rate_per_1000: self.error_monitor.recent_error_count,
        }
    }

    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.calculate_health_score() >= 80
    }

    /// Get critical violation count
    pub fn get_critical_violations(&self) -> u64 {
        self.allocation_monitor.budget_violations
            + self.capability_monitor.access_violations
            + self.capability_monitor.double_free_attempts
            + self.error_monitor.fatal_errors
    }

    /// Reset counters (for testing)
    #[cfg(test)]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Calculate system health score (0-100)
    fn calculate_health_score(&self) -> u8 {
        let total = self.allocation_monitor.total_allocations.max(1);

        // Calculate failure rates
        let failure_rate = (self.allocation_monitor.failed_allocations * 100) / total;
        let violation_rate = (self.allocation_monitor.budget_violations * 100) / total;
        let capability_rate = (self.capability_monitor.access_violations * 100) / total;

        // Start with perfect score
        let mut score = 100u8;

        // Deduct for failures (max 40 points)
        score = score.saturating_sub((failure_rate as u8).min(40));

        // Deduct for violations (max 30 points)
        score = score.saturating_sub((violation_rate as u8).min(30));

        // Deduct for capability violations (max 30 points)
        score = score.saturating_sub((capability_rate as u8).min(30));

        // Fatal errors immediately drop to critical
        if self.error_monitor.fatal_errors > 0 {
            score = score.min(50);
        }

        score
    }

    /// Update error rate calculation
    fn update_error_rate(&mut self) {
        self.error_monitor.recent_error_count =
            self.error_monitor.recent_error_count.saturating_add(1);

        // Reset rate counter every 1000 operations
        if self.error_monitor.operation_count % 1000 == 0 {
            self.error_monitor.recent_error_count = 0;
        }
    }

    /// Increment operation counter
    fn increment_operations(&mut self) {
        self.error_monitor.operation_count += 1;
    }
}

/// Global safety monitor instance
#[cfg(not(test))]
static mut SAFETY_MONITOR: SafetyMonitor = SafetyMonitor::new();

/// Lock guarding `SAFETY_MONITOR`: `true` while one caller holds it.
#[cfg(not(test))]
static MONITOR_LOCK: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Releases `MONITOR_LOCK` when dropped, so the lock is released on every
/// path out of the critical section, including unwinding.
#[cfg(not(test))]
struct MonitorLockGuard;

#[cfg(not(test))]
impl Drop for MonitorLockGuard {
    fn drop(&mut self) {
        MONITOR_LOCK.store(false, core::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
std::thread_local! {
    /// Unit tests run on one thread each and assert absolute counts, so each
    /// test thread owns its monitor instead of sharing the process-global one
    /// (which any concurrently running test that allocates would mutate).
    /// Every `with_safety_monitor` caller in the crate goes through this one
    /// function, so there is no other path to the global.
    static TEST_SAFETY_MONITOR: core::cell::RefCell<SafetyMonitor> =
        const { core::cell::RefCell::new(SafetyMonitor::new()) };
}

/// Get the safety monitor owned by the current test thread (unit tests only).
#[cfg(test)]
pub fn with_safety_monitor<F, R>(f: F) -> R
where
    F: FnOnce(&mut SafetyMonitor) -> R,
{
    TEST_SAFETY_MONITOR.with(|monitor| f(&mut monitor.borrow_mut()))
}

/// Run `f` with exclusive access to the global safety monitor.
///
/// Mutual exclusion is a spinlock on `MONITOR_LOCK`: a caller acquires it with
/// a compare-exchange from `false` to `true` (`Acquire`) and releases it with a
/// `Release` store, so the monitor writes of one holder are visible to the
/// next. The release happens in a guard's `Drop`, so a panic inside `f`
/// releases the lock too. `f` must not call `with_safety_monitor` itself: the
/// lock is not re-entrant and a nested call spins forever.
///
/// SW-REQ-ID: ASIL-A-MON-002 - Thread-safe monitor access
#[cfg(not(test))]
#[allow(unsafe_code)]
pub fn with_safety_monitor<F, R>(f: F) -> R
where
    F: FnOnce(&mut SafetyMonitor) -> R,
{
    use core::sync::atomic::Ordering;

    while MONITOR_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let _guard = MonitorLockGuard;

    // SAFETY: this thread set MONITOR_LOCK from false to true above and holds
    // it until `_guard` drops, so no other reference to SAFETY_MONITOR exists
    // while `f` runs.
    f(unsafe { &mut *core::ptr::addr_of_mut!(SAFETY_MONITOR) })
}

/// Production assertion that records safety violations
#[macro_export]
macro_rules! safety_assert {
    ($cond:expr, $violation:expr) => {
        if !$cond {
            $crate::safety_monitor::with_safety_monitor(|monitor| match $violation {
                $crate::safety_monitor::ViolationType::BudgetExceeded => {
                    monitor.record_budget_violation($crate::CrateId::Foundation, 0, 0);
                },
                $crate::safety_monitor::ViolationType::CapabilityViolation => {
                    monitor.record_capability_violation(0);
                },
                $crate::safety_monitor::ViolationType::DoubleFree => {
                    monitor.record_double_free();
                },
                _ => {
                    monitor.record_fatal_error();
                },
            });
            panic!("Safety assertion failed: {:?}", $violation);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use std::vec;

    #[test]
    fn test_health_score_calculation() {
        let mut monitor = SafetyMonitor::new();

        // Perfect health
        assert_eq!(monitor.calculate_health_score(), 100);

        // Record some successful operations
        for _ in 0..100 {
            monitor.record_allocation(1024);
        }
        assert_eq!(monitor.calculate_health_score(), 100);

        // Add some failures
        for _ in 0..10 {
            monitor.record_allocation_failure(1024);
        }
        // 10% failure rate should reduce score
        assert!(monitor.calculate_health_score() < 100);
        assert!(monitor.calculate_health_score() >= 60);

        // Add fatal error
        monitor.record_fatal_error();
        assert!(monitor.calculate_health_score() <= 50);
    }

    #[test]
    fn test_memory_tracking() {
        let mut monitor = SafetyMonitor::new();

        // Track allocations
        monitor.record_allocation(1024);
        monitor.record_allocation(2048);
        assert_eq!(monitor.allocation_monitor.current_allocated, 3072);
        assert_eq!(monitor.allocation_monitor.peak_allocated, 3072);

        // Track deallocation
        monitor.record_deallocation(1024);
        assert_eq!(monitor.allocation_monitor.current_allocated, 2048);
        assert_eq!(monitor.allocation_monitor.peak_allocated, 3072);
    }

    #[test]
    fn test_violation_tracking() {
        let mut monitor = SafetyMonitor::new();

        // Record various violations
        monitor.record_capability_violation(CrateId::Foundation);
        monitor.record_double_free();
        monitor.record_budget_violation(CrateId::Foundation, 8192, 4096);

        assert_eq!(monitor.get_critical_violations(), 3);

        let report = monitor.get_safety_report();
        assert_eq!(report.capability_violations, 1);
        assert_eq!(report.budget_violations, 1);
    }

    #[test]
    #[serial_test::serial]
    fn test_thread_safe_access() {
        // Reset global monitor to avoid pollution from other serial tests
        with_safety_monitor(|monitor| {
            monitor.reset();
        });

        with_safety_monitor(|monitor| {
            monitor.record_allocation(1024);
        });

        with_safety_monitor(|monitor| {
            let report = monitor.get_safety_report();
            assert_eq!(report.total_allocations, 1);
        });
    }
}
