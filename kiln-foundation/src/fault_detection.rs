//! Systematic Fault Detection for Memory Safety
//!
//! This module implements comprehensive fault detection mechanisms for memory
//! violations as required for ASIL-A compliance. It provides runtime monitoring
//! of memory operations and systematic detection of safety violations.

// Note: no_std is configured at the crate level

use core::sync::atomic::{
    AtomicBool,
    AtomicU32,
    AtomicUsize,
    Ordering,
};

use crate::{
    budget_aware_provider::CrateId,
    Error,
    Result,
};

/// Global fault detection state (safe for concurrent access)
pub struct FaultDetector {
    /// Whether fault detection is enabled
    enabled: AtomicBool,

    /// Count of detected memory violations
    memory_violations: AtomicU32,

    /// Count of detected budget violations
    budget_violations: AtomicU32,

    /// Count of detected bounds violations
    bounds_violations: AtomicU32,

    /// Count of detected capability violations
    capability_violations: AtomicU32,

    /// Current memory watermark (highest usage)
    memory_watermark: AtomicUsize,

    /// Fault response mode
    response_mode: FaultResponseMode,
}

/// How the system should respond to detected faults
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultResponseMode {
    /// Log and continue (development mode)
    LogOnly,

    /// Log and degrade gracefully (ASIL-A default)
    GracefulDegradation,

    /// Log and halt execution (highest safety)
    HaltOnFault,
}

/// Types of memory faults that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultType {
    /// Memory allocation exceeded budget
    BudgetExceeded { requested: usize, available: usize },

    /// Array/buffer bounds violation
    BoundsViolation { index: usize, limit: usize },

    /// Capability check failed
    CapabilityViolation { crate_id: CrateId },

    /// Memory corruption detected
    MemoryCorruption { address: usize },

    /// Double-free or use-after-free
    UseAfterFree { address: usize },

    /// Null pointer dereference
    NullPointer,

    /// Stack overflow detected
    StackOverflow,

    /// Alignment violation
    AlignmentViolation { address: usize, required: usize },
}

/// Fault detection context for a specific operation
pub struct FaultContext {
    /// The crate performing the operation
    pub crate_id: CrateId,

    /// Type of operation being performed
    pub operation: OperationType,

    /// Memory address involved (if applicable)
    pub address: Option<usize>,

    /// Size of operation (if applicable)
    pub size: Option<usize>,
}

/// Types of operations that can be monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Memory allocation
    Allocate,

    /// Memory deallocation
    Deallocate,

    /// Memory read access
    Read,

    /// Memory write access
    Write,

    /// Bounds check
    BoundsCheck,

    /// Capability verification
    CapabilityCheck,
}

impl FaultDetector {
    /// Create a new fault detector with specified response mode
    pub const fn new(response_mode: FaultResponseMode) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            memory_violations: AtomicU32::new(0),
            budget_violations: AtomicU32::new(0),
            bounds_violations: AtomicU32::new(0),
            capability_violations: AtomicU32::new(0),
            memory_watermark: AtomicUsize::new(0),
            response_mode,
        }
    }

    /// Enable or disable fault detection
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Check if fault detection is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Report a detected fault
    pub fn report_fault(&self, fault: FaultType, context: &FaultContext) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        // Increment appropriate counter
        match fault {
            FaultType::BudgetExceeded { .. } => {
                self.budget_violations.fetch_add(1, Ordering::Relaxed);
            },
            FaultType::BoundsViolation { .. } => {
                self.bounds_violations.fetch_add(1, Ordering::Relaxed);
            },
            FaultType::CapabilityViolation { .. } => {
                self.capability_violations.fetch_add(1, Ordering::Relaxed);
            },
            _ => {
                self.memory_violations.fetch_add(1, Ordering::Relaxed);
            },
        }

        // Log the fault (platform-specific implementation needed)
        self.log_fault(&fault, context);

        // Take action based on response mode
        match self.response_mode {
            FaultResponseMode::LogOnly => Ok(()),
            FaultResponseMode::GracefulDegradation => {
                self.handle_graceful_degradation(&fault, context)
            },
            FaultResponseMode::HaltOnFault => self.handle_halt_on_fault(&fault, context),
        }
    }

    /// Check memory bounds before access
    #[inline]
    pub fn check_bounds(&self, index: usize, limit: usize, context: &FaultContext) -> Result<()> {
        if index >= limit {
            let fault = FaultType::BoundsViolation { index, limit };
            self.report_fault(fault, context)?;
            Err(Error::memory_out_of_bounds("Bounds check failed"))
        } else {
            Ok(())
        }
    }

    /// Check memory budget before allocation
    #[inline]
    pub fn check_budget(
        &self,
        requested: usize,
        available: usize,
        context: &FaultContext,
    ) -> Result<()> {
        if requested > available {
            let fault = FaultType::BudgetExceeded {
                requested,
                available,
            };
            self.report_fault(fault, context)?;
            Err(Error::foundation_memory_provider_failed(
                "Memory budget exceeded",
            ))
        } else {
            // Update watermark if this would be a new high
            let current_usage = available.saturating_sub(requested);
            self.update_watermark(current_usage);
            Ok(())
        }
    }

    /// Check pointer alignment
    #[inline]
    pub fn check_alignment(
        &self,
        address: usize,
        required: usize,
        context: &FaultContext,
    ) -> Result<()> {
        if address % required != 0 {
            let fault = FaultType::AlignmentViolation { address, required };
            self.report_fault(fault, context)?;
            Err(Error::memory_error("Alignment violation"))
        } else {
            Ok(())
        }
    }

    /// Update memory usage watermark
    fn update_watermark(&self, usage: usize) {
        let mut current = self.memory_watermark.load(Ordering::Relaxed);
        loop {
            if usage <= current {
                break;
            }
            match self.memory_watermark.compare_exchange_weak(
                current,
                usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Log a fault (platform-specific implementation)
    fn log_fault(&self, fault: &FaultType, context: &FaultContext) {
        // In no_std, we can't use println!, so this would need platform-specific
        // logging. For now, we'll use a debug_assert to at least catch issues
        // during development - but not during tests since tests intentionally
        // trigger faults to verify fault handling.
        #[cfg(all(debug_assertions, not(test)))]
        {
            // In debug builds (non-test), we can at least trigger an assertion
            debug_assert!(
                false,
                "Fault detected: {:?} in {:?} for {:?}",
                fault, context.operation, context.crate_id
            );
        }

        // Suppress unused variable warnings when debug_assertions is off or in tests
        let _ = (fault, context);

        // In production, this would integrate with platform logging
        // For example, on Linux it might write to syslog, on embedded
        // systems it might write to a debug UART, etc.
    }

    /// Handle graceful degradation response
    fn handle_graceful_degradation(
        &self,
        fault: &FaultType,
        _context: &FaultContext,
    ) -> Result<()> {
        match fault {
            FaultType::BudgetExceeded { .. } => {
                // Could trigger garbage collection or resource cleanup
                Ok(())
            },
            FaultType::StackOverflow => {
                // Cannot recover from stack overflow safely
                Err(Error::foundation_verification_failed(
                    "Stack overflow detected",
                ))
            },
            _ => {
                // Most faults can be handled by returning error
                Ok(())
            },
        }
    }

    /// Handle halt-on-fault response
    fn handle_halt_on_fault(&self, _fault: &FaultType, _context: &FaultContext) -> Result<()> {
        // In highest safety mode, any fault triggers system halt
        // This would typically call platform-specific halt mechanism
        Err(Error::foundation_verification_failed(
            "System halted due to safety fault",
        ))
    }

    /// Get current fault statistics
    pub fn get_statistics(&self) -> FaultStatistics {
        FaultStatistics {
            memory_violations:     self.memory_violations.load(Ordering::Relaxed),
            budget_violations:     self.budget_violations.load(Ordering::Relaxed),
            bounds_violations:     self.bounds_violations.load(Ordering::Relaxed),
            capability_violations: self.capability_violations.load(Ordering::Relaxed),
            memory_watermark:      self.memory_watermark.load(Ordering::Relaxed),
        }
    }

    /// Reset fault counters (for testing/diagnostics)
    pub fn reset_counters(&self) {
        self.memory_violations.store(0, Ordering::Relaxed);
        self.budget_violations.store(0, Ordering::Relaxed);
        self.bounds_violations.store(0, Ordering::Relaxed);
        self.capability_violations.store(0, Ordering::Relaxed);
    }
}

/// Fault detection statistics
#[derive(Debug, Clone, Copy)]
pub struct FaultStatistics {
    /// Total memory violations detected
    pub memory_violations: u32,

    /// Total budget violations detected
    pub budget_violations: u32,

    /// Total bounds violations detected
    pub bounds_violations: u32,

    /// Total capability violations detected
    pub capability_violations: u32,

    /// Highest memory usage observed
    pub memory_watermark: usize,
}

/// Global fault detector instance
static FAULT_DETECTOR: FaultDetector = FaultDetector::new(FaultResponseMode::GracefulDegradation);

/// Get the global fault detector
pub fn fault_detector() -> &'static FaultDetector {
    &FAULT_DETECTOR
}

/// Convenience macro for bounds checking with fault detection
#[macro_export]
macro_rules! check_bounds {
    ($index:expr, $limit:expr, $crate_id:expr) => {{
        let context = $crate::fault_detection::FaultContext {
            crate_id:  $crate_id,
            operation: $crate::fault_detection::OperationType::BoundsCheck,
            address:   None,
            size:      Some($index),
        };
        $crate::fault_detection::fault_detector().check_bounds($index, $limit, &context)
    }};
}

/// Convenience macro for budget checking with fault detection
#[macro_export]
macro_rules! check_budget {
    ($requested:expr, $available:expr, $crate_id:expr) => {{
        let context = $crate::fault_detection::FaultContext {
            crate_id:  $crate_id,
            operation: $crate::fault_detection::OperationType::Allocate,
            address:   None,
            size:      Some($requested),
        };
        $crate::fault_detection::fault_detector().check_budget($requested, $available, &context)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_checking() {
        let detector = FaultDetector::new(FaultResponseMode::LogOnly);
        let context = FaultContext {
            crate_id:  CrateId::Foundation,
            operation: OperationType::BoundsCheck,
            address:   None,
            size:      Some(10),
        };

        // Valid bounds check
        assert!(detector.check_bounds(5, 10, &context).is_ok());

        // Invalid bounds check
        assert!(detector.check_bounds(10, 10, &context).is_err());
        assert!(detector.check_bounds(15, 10, &context).is_err());

        // Check statistics
        let stats = detector.get_statistics();
        assert_eq!(stats.bounds_violations, 2);
    }

    #[test]
    fn test_budget_checking() {
        let detector = FaultDetector::new(FaultResponseMode::LogOnly);
        let context = FaultContext {
            crate_id:  CrateId::Component,
            operation: OperationType::Allocate,
            address:   None,
            size:      Some(1024),
        };

        // Valid budget check
        assert!(detector.check_budget(512, 1024, &context).is_ok());

        // Invalid budget check
        assert!(detector.check_budget(2048, 1024, &context).is_err());

        // Check statistics
        let stats = detector.get_statistics();
        assert_eq!(stats.budget_violations, 1);
        assert!(stats.memory_watermark >= 512);
    }

    #[test]
    fn test_alignment_checking() {
        let detector = FaultDetector::new(FaultResponseMode::LogOnly);
        let context = FaultContext {
            crate_id:  CrateId::Runtime,
            operation: OperationType::Read,
            address:   Some(0x1000),
            size:      Some(4),
        };

        // Valid alignment
        assert!(detector.check_alignment(0x1000, 4, &context).is_ok());
        assert!(detector.check_alignment(0x1004, 4, &context).is_ok());

        // Invalid alignment
        assert!(detector.check_alignment(0x1001, 4, &context).is_err());
        assert!(detector.check_alignment(0x1002, 4, &context).is_err());

        // Check statistics
        let stats = detector.get_statistics();
        assert_eq!(stats.memory_violations, 2);
    }

    #[test]
    fn test_fault_detection_modes() {
        // LogOnly mode - should not return errors for non-fatal faults
        let detector = FaultDetector::new(FaultResponseMode::LogOnly);
        detector.set_enabled(true);

        let context = FaultContext {
            crate_id:  CrateId::Foundation,
            operation: OperationType::Allocate,
            address:   None,
            size:      Some(100),
        };

        let fault = FaultType::BudgetExceeded {
            requested: 200,
            available: 100,
        };

        // In LogOnly mode, report_fault should succeed
        assert!(detector.report_fault(fault, &context).is_ok());

        // Verify the fault was counted
        let stats = detector.get_statistics();
        assert_eq!(stats.budget_violations, 1);
    }
}
