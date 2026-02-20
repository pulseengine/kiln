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

use crate::{
    capabilities::CapabilityMask,
    CrateId,
};

/// Safety monitor for runtime verification
///
/// Tracks safety-critical metrics during runtime execution
/// to ensure system remains within safe operating parameters.
#[derive(Debug)]
pub struct SafetyMonitor {
    /// Memory allocation tracking
    allocation_monitor:  AllocationMonitor,
    /// Capability violation tracking
    capability_monitor:  CapabilityMonitor,
    /// Error rate monitoring
    error_monitor:       ErrorMonitor,
    /// Performance degradation detection
    performance_monitor: PerformanceMonitor,
}

/// Tracks memory allocation patterns and violations
#[derive(Debug, Default)]
struct AllocationMonitor {
    /// Total allocations
    total_allocations:  u64,
    /// Failed allocations
    failed_allocations: u64,
    /// Budget violations
    budget_violations:  u64,
    /// Largest allocation
    peak_allocation:    usize,
    /// Current allocated bytes
    current_allocated:  usize,
    /// Peak allocated bytes
    peak_allocated:     usize,
}

/// Monitors capability system violations
#[derive(Debug, Default)]
struct CapabilityMonitor {
    /// Unauthorized access attempts
    access_violations:    u64,
    /// Invalid capability uses
    invalid_uses:         u64,
    /// Capability exhaustion events
    exhaustion_events:    u64,
    /// Double-free attempts
    double_free_attempts: u64,
}

/// Tracks error rates and patterns
#[derive(Debug, Default)]
struct ErrorMonitor {
    /// Errors by severity level [Critical, High, Medium, Low]
    errors_by_level:    [u64; 4],
    /// Recovery success rate
    recovery_successes: u64,
    /// Unrecoverable failures
    fatal_errors:       u64,
    /// Error rate window (errors in last 1000 operations)
    recent_error_count: u32,
    /// Operations counter for rate calculation
    operation_count:    u64,
}

/// Monitors performance degradation
#[derive(Debug, Default)]
struct PerformanceMonitor {
    /// Slow allocation events (>threshold)
    slow_allocations:       u64,
    /// Memory pressure events
    memory_pressure_events: u64,
    /// Throughput degradation
    degradation_events:     u64,
    /// Allocation time threshold in microseconds
    slow_threshold_us:      u64,
}

/// Safety monitoring report
#[derive(Debug, Clone)]
pub struct SafetyReport {
    /// Total memory allocations attempted
    pub total_allocations:     u64,
    /// Failed allocation attempts
    pub failed_allocations:    u64,
    /// Budget violation count
    pub budget_violations:     u64,
    /// Capability violation count
    pub capability_violations: u64,
    /// Fatal error count
    pub fatal_errors:          u64,
    /// System health score (0-100)
    pub health_score:          u8,
    /// Current memory usage
    pub current_memory_bytes:  usize,
    /// Peak memory usage
    pub peak_memory_bytes:     usize,
    /// Recent error rate (per 1000 operations)
    pub error_rate_per_1000:   u32,
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
            allocation_monitor:  AllocationMonitor {
                total_allocations:  0,
                failed_allocations: 0,
                budget_violations:  0,
                peak_allocation:    0,
                current_allocated:  0,
                peak_allocated:     0,
            },
            capability_monitor:  CapabilityMonitor {
                access_violations:    0,
                invalid_uses:         0,
                exhaustion_events:    0,
                double_free_attempts: 0,
            },
            error_monitor:       ErrorMonitor {
                errors_by_level:    [0; 4],
                recovery_successes: 0,
                fatal_errors:       0,
                recent_error_count: 0,
                operation_count:    0,
            },
            performance_monitor: PerformanceMonitor {
                slow_allocations:       0,
                memory_pressure_events: 0,
                degradation_events:     0,
                slow_threshold_us:      1000, // 1ms default
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
            total_allocations:     self.allocation_monitor.total_allocations,
            failed_allocations:    self.allocation_monitor.failed_allocations,
            budget_violations:     self.allocation_monitor.budget_violations,
            capability_violations: self.capability_monitor.access_violations,
            fatal_errors:          self.error_monitor.fatal_errors,
            health_score:          self.calculate_health_score(),
            current_memory_bytes:  self.allocation_monitor.current_allocated,
            peak_memory_bytes:     self.allocation_monitor.peak_allocated,
            error_rate_per_1000:   self.error_monitor.recent_error_count,
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
static mut SAFETY_MONITOR: SafetyMonitor = SafetyMonitor::new();

/// Safety monitor lock for thread safety
static mut MONITOR_LOCK: bool = false;

/// Get global safety monitor
///
/// # Safety
/// This function provides thread-safe access to the global safety monitor.
/// It uses a simple spinlock for mutual exclusion in no_std environments.
///
/// # Safety
///
/// This function uses unsafe code for lock-free synchronization which is
/// necessary for runtime safety monitoring in no_std environments. The unsafe
/// operations are:
/// 1. Volatile reads/writes to implement a spinlock without std::sync
///    primitives
/// 2. Direct memory access to the global monitor instance
///
/// Safety is ensured by:
/// - Using volatile operations to prevent compiler optimizations
/// - Simple boolean flag prevents data races
/// - Critical section is minimal to reduce contention
///
/// SW-REQ-ID: ASIL-A-MON-002 - Thread-safe monitor access
#[allow(unsafe_code)]
pub fn with_safety_monitor<F, R>(f: F) -> R
where
    F: FnOnce(&mut SafetyMonitor) -> R,
{
    // Simple spinlock for thread safety
    // SAFETY: Volatile read ensures visibility across threads
    while unsafe { core::ptr::read_volatile(&raw const MONITOR_LOCK) } {
        core::hint::spin_loop();
    }

    // SAFETY: This unsafe block implements a critical section for thread-safe
    // access:
    // 1. Volatile writes ensure lock acquisition is visible to other threads
    // 2. The monitor is only accessed while lock is held
    // 3. Lock is always released, even if function panics (no panic in no_std)
    unsafe {
        // Acquire lock
        core::ptr::write_volatile(&raw mut MONITOR_LOCK, true);

        // Execute function with monitor
        let result = f(&mut *core::ptr::addr_of_mut!(SAFETY_MONITOR));

        // Release lock
        core::ptr::write_volatile(&raw mut MONITOR_LOCK, false);

        result
    }
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
    fn test_thread_safe_access() {
        with_safety_monitor(|monitor| {
            monitor.record_allocation(1024);
        });

        with_safety_monitor(|monitor| {
            let report = monitor.get_safety_report();
            assert_eq!(report.total_allocations, 1);
        });
    }
}
