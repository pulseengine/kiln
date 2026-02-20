// WRT - wrt-error
// Module: ASIL Safety Level Support
// SW-REQ-ID: REQ_SAFETY_ASIL_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! ASIL (Automotive Safety Integrity Level) support for error handling
//!
//! This module provides ASIL-specific functionality for safety-critical
//! error handling as per ISO 26262 standard.

use crate::{Error, ErrorCategory};

/// ASIL (Automotive Safety Integrity Level) as defined by ISO 26262
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AsilLevel {
    /// Quality Management (no safety requirements)
    QM = 0,
    /// ASIL-A (lowest safety integrity level)
    AsilA = 1,
    /// ASIL-B (medium-low safety integrity level, ≥90% SPFM)
    AsilB = 2,
    /// ASIL-C (medium-high safety integrity level, ≥97% SPFM)
    AsilC = 3,
    /// ASIL-D (highest safety integrity level, ≥99% SPFM)
    AsilD = 4,
}

impl AsilLevel {
    /// Get the current compile-time ASIL level
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(feature = "asil-d")]
        {
            Self::AsilD
        }
        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Self::AsilC
        }
        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            Self::AsilB
        }
        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            Self::AsilA
        }
        #[cfg(not(any(
            feature = "asil-a",
            feature = "asil-b",
            feature = "asil-c",
            feature = "asil-d"
        )))]
        {
            Self::QM
        }
    }

    /// Check if current level meets minimum requirement
    #[must_use]
    pub const fn meets_requirement(required: Self) -> bool {
        (Self::current() as u8) >= (required as u8)
    }

    /// Get human-readable name
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::QM => "QM",
            Self::AsilA => "ASIL-A",
            Self::AsilB => "ASIL-B",
            Self::AsilC => "ASIL-C",
            Self::AsilD => "ASIL-D",
        }
    }
}

/// Safety monitor for ASIL-C and above
#[cfg(any(feature = "asil-c", feature = "asil-d"))]
pub struct SafetyMonitor {
    error_count: core::sync::atomic::AtomicU32,
    last_error_code: core::sync::atomic::AtomicU16,
}

#[cfg(any(feature = "asil-c", feature = "asil-d"))]
impl Default for SafetyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(feature = "asil-c", feature = "asil-d"))]
impl SafetyMonitor {
    /// Create a new safety monitor
    #[must_use]
    pub const fn new() -> Self {
        Self {
            error_count: core::sync::atomic::AtomicU32::new(0),
            last_error_code: core::sync::atomic::AtomicU16::new(0),
        }
    }

    /// Record an error occurrence
    pub fn record_error(&self, error: &Error) {
        use core::sync::atomic::Ordering;

        self.error_count.fetch_add(1, Ordering::SeqCst);
        self.last_error_code.store(error.code, Ordering::SeqCst);

        // ASIL-D: Check for error storm indicating systematic failure
        #[cfg(feature = "asil-d")]
        {
            let count = self.error_count.load(Ordering::SeqCst);
            if count > 100 {
                // In a real system, this would trigger safe state transition
                // For now, we just create a critical error
                let _ = Error::safety_violation("Error storm detected - systematic failure");
            }
        }
    }

    /// Get total error count
    pub fn error_count(&self) -> u32 {
        use core::sync::atomic::Ordering;
        self.error_count.load(Ordering::SeqCst)
    }

    /// Reset monitor state
    pub fn reset(&self) {
        use core::sync::atomic::Ordering;
        self.error_count.store(0, Ordering::SeqCst);
        self.last_error_code.store(0, Ordering::SeqCst);
    }
}

/// Error context with ASIL metadata
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
#[derive(Debug, Clone)]
pub struct AsilErrorContext {
    /// The error itself
    pub error: Error,
    /// ASIL level when error occurred
    pub asil_level: AsilLevel,
    /// Timestamp (if available)
    pub timestamp: Option<u64>,
    /// Source module ID
    pub module_id: Option<u32>,
}

#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
impl AsilErrorContext {
    /// Create a new ASIL error context
    #[must_use]
    pub const fn new(error: Error) -> Self {
        Self {
            error,
            asil_level: AsilLevel::current(),
            timestamp: None,
            module_id: None,
        }
    }

    /// Add timestamp to context
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Add module ID to context
    #[must_use]
    pub const fn with_module_id(mut self, module_id: u32) -> Self {
        self.module_id = Some(module_id);
        self
    }

    /// Check if this error requires immediate action
    #[must_use]
    pub const fn requires_immediate_action(&self) -> bool {
        matches!(
            self.error.category,
            ErrorCategory::Safety | ErrorCategory::Memory | ErrorCategory::RuntimeTrap
        )
    }
}

/// Validate error consistency for ASIL-D
#[cfg(feature = "asil-d")]
#[must_use]
pub const fn validate_error_consistency(error: &Error) -> bool {
    // Check error code is in valid range for category
    let valid_code = match error.category {
        ErrorCategory::Core => error.code >= 1000 && error.code < 2000,
        ErrorCategory::Component => error.code >= 2000 && error.code < 3000,
        ErrorCategory::Resource => error.code >= 3000 && error.code < 4000,
        ErrorCategory::Memory => error.code >= 4000 && error.code < 5000,
        ErrorCategory::Validation => error.code >= 5000 && error.code < 6000,
        ErrorCategory::Type => error.code >= 6000 && error.code < 7000,
        ErrorCategory::Runtime | ErrorCategory::Safety => error.code >= 7000 && error.code < 8000,
        ErrorCategory::System => error.code >= 8000 && error.code < 9000,
        _ => false,
    };

    // Check message is not empty
    valid_code && !error.message.is_empty()
}

/// Create an error with ASIL level validation
///
/// # Errors
///
/// Returns an error if:
/// - Current ASIL level doesn't meet the required level
/// - Error consistency validation fails (ASIL-D only)
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
#[must_use = "ASIL errors must be handled to maintain safety compliance"]
#[allow(clippy::missing_const_for_fn)] // Contains control flow not supported in const fn
pub fn create_asil_error(
    category: ErrorCategory,
    code: u16,
    message: &'static str,
    required_level: AsilLevel,
) -> Result<Error, Error> {
    // Check if current ASIL level meets requirement
    if !AsilLevel::meets_requirement(required_level) {
        return Err(Error::safety_violation("ASIL level requirement not met"));
    }

    // Create error
    let error = Error::new(category, code, message);

    // ASIL-D: Validate error consistency
    #[cfg(feature = "asil-d")]
    {
        if !validate_error_consistency(&error) {
            return Err(Error::verification_failed(
                "Error consistency validation failed",
            ));
        }
    }

    Ok(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asil_level_ordering() {
        assert!(AsilLevel::QM < AsilLevel::AsilA);
        assert!(AsilLevel::AsilA < AsilLevel::AsilB);
        assert!(AsilLevel::AsilB < AsilLevel::AsilC);
        assert!(AsilLevel::AsilC < AsilLevel::AsilD);
    }

    #[test]
    fn test_asil_level_names() {
        assert_eq!(AsilLevel::QM.name(), "QM");
        assert_eq!(AsilLevel::AsilA.name(), "ASIL-A");
        assert_eq!(AsilLevel::AsilB.name(), "ASIL-B");
        assert_eq!(AsilLevel::AsilC.name(), "ASIL-C");
        assert_eq!(AsilLevel::AsilD.name(), "ASIL-D");
    }

    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    #[test]
    fn test_safety_monitor() {
        let monitor = SafetyMonitor::new();
        assert_eq!(monitor.error_count(), 0);

        let error = Error::new(ErrorCategory::Memory, 4000, "Test error");
        monitor.record_error(&error);
        assert_eq!(monitor.error_count(), 1);

        monitor.reset();
        assert_eq!(monitor.error_count(), 0);
    }

    #[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
    #[test]
    fn test_asil_error_context() {
        let error = Error::new(ErrorCategory::Safety, 7000, "Safety error");
        let context = AsilErrorContext::new(error).with_timestamp(12345).with_module_id(42);

        assert_eq!(context.timestamp, Some(12345));
        assert_eq!(context.module_id, Some(42));
        assert!(context.requires_immediate_action());
    }

    #[cfg(feature = "asil-d")]
    #[test]
    fn test_error_consistency_validation() {
        // Valid error
        let valid_error = Error::new(ErrorCategory::Memory, 4500, "Memory error");
        assert!(validate_error_consistency(&valid_error));

        // Invalid error (code out of range)
        let invalid_error = Error::new(ErrorCategory::Memory, 1500, "Memory error");
        assert!(!validate_error_consistency(&invalid_error));

        // Invalid error (empty message)
        let empty_msg_error = Error::new(ErrorCategory::Memory, 4500, "");
        assert!(!validate_error_consistency(&empty_msg_error));
    }
}
