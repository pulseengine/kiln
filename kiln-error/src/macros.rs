// WRT - wrt-error
// Module: ASIL-aware Error Macros
// SW-REQ-ID: REQ_SAFETY_ASIL_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Macros for ASIL-aware error handling
//!
//! These macros provide compile-time and runtime safety checks
//! based on the configured ASIL level.

/// Create an error with ASIL level validation
///
/// # Examples
///
/// ```ignore
/// use wrt_error::{asil_error, ErrorCategory, codes};
///
/// // Create a safety-critical error (requires ASIL-B or higher)
/// let error = asil_error!(
///     ErrorCategory::Safety,
///     codes::SAFETY_VIOLATION,
///     "Critical safety violation",
///     "asil-b"
/// ;
/// ```
#[macro_export]
macro_rules! asil_error {
    ($category:expr, $code:expr, $message:expr,"asil-d") => {{
        #[cfg(not(feature = "asil-d"))]
        compile_error!("This error requires ASIL-D safety level");

        #[cfg(feature = "asil-d")]
        {
            let error = $crate::Error::new($category, $code, $message);
            // Validate at runtime for ASIL-D
            if !error.validate_integrity() {
                // ASIL-D: Enter safe state instead of panic
                loop {
                    core::hint::spin_loop();
                }
            }
            error
        }
    }};
    ($category:expr, $code:expr, $message:expr,"asil-c") => {{
        #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
        compile_error!("This error requires ASIL-C safety level or higher");

        #[cfg(any(feature = "asil-c", feature = "asil-d"))]
        $crate::Error::new($category, $code, $message)
    }};
    ($category:expr, $code:expr, $message:expr,"asil-b") => {{
        #[cfg(not(any(feature = "asil-b", feature = "asil-c", feature = "asil-d")))]
        compile_error!("This error requires ASIL-B safety level or higher");

        #[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
        $crate::Error::new($category, $code, $message)
    }};
    ($category:expr, $code:expr, $message:expr) => {
        $crate::Error::new($category, $code, $message)
    };
}

/// Record an error with safety monitoring (ASIL-C and above)
///
/// # Examples
///
/// ```ignore
/// use wrt_error::{monitor_error, Error, ErrorCategory, codes};
///
/// let monitor = SafetyMonitor::new();
/// let error = Error::new(ErrorCategory::Memory, codes::MEMORY_OUT_OF_BOUNDS, "Out of bounds");
/// monitor_error!(monitor, error);
/// ```
#[cfg(any(feature = "asil-c", feature = "asil-d"))]
#[macro_export]
macro_rules! monitor_error {
    ($monitor:expr, $error:expr) => {{
        $monitor.record_error(&$error);

        // ASIL-D: Check if immediate safe state is required
        #[cfg(feature = "asil-d")]
        {
            if $error.requires_safe_state() {
                // In a real system, this would trigger safe state transition
                // For now, we just log it
                $error
            } else {
                $error
            }
        }

        #[cfg(not(feature = "asil-d"))]
        $error
    }};
}

/// Assert with ASIL-level appropriate behavior
///
/// - QM/ASIL-A: Standard debug assertion
/// - ASIL-B: Always checks, returns error
/// - ASIL-C/D: Always checks, panics on failure
///
/// # Examples
///
/// ```ignore
/// use wrt_error::{asil_assert, Error, ErrorCategory, codes};
///
/// fn validate_index(index: usize, max: usize) -> Result<(), Error> {
///     asil_assert!(
///         index < max,
///         ErrorCategory::Validation,
///         codes::OUT_OF_BOUNDS_ERROR,
///         "Index out of bounds"
///     )
/// }
/// ```
#[macro_export]
macro_rules! asil_assert {
    ($condition:expr, $category:expr, $code:expr, $message:expr) => {{
        #[cfg(feature = "asil-d")]
        {
            if !$condition {
                // ASIL-D: Enter safe state instead of panic
                loop {
                    core::hint::spin_loop();
                }
            }
            Ok(())
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            if !$condition {
                // ASIL-C: Enter safe state for critical assertions
                loop {
                    core::hint::spin_loop();
                }
            }
            Ok(())
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            if !$condition {
                // ASIL-B: Return error for recoverable assertions
                Err($crate::Error::new($category, $code, $message))
            } else {
                Ok(())
            }
        }

        #[cfg(not(any(feature = "asil-b", feature = "asil-c", feature = "asil-d")))]
        {
            // QM/ASIL-A: Debug assertion only
            debug_assert!($condition, "{}", $message);
            Ok(())
        }
    }};
}

/// Create an error with compile-time ASIL validation
///
/// This macro ensures that certain error types can only be created
/// when the appropriate ASIL level is configured.
///
/// # Examples
///
/// ```ignore
/// use wrt_error::{safety_error, codes};
///
/// // This will only compile with ASIL-C or higher
/// let error = safety_error!(
///     codes::MEMORY_CORRUPTION_DETECTED,
///     "Critical memory corruption detected"
/// ;
/// ```
#[macro_export]
macro_rules! safety_error {
    ($code:expr, $message:expr) => {{
        #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
        compile_error!("Safety errors require ASIL-C or higher");

        #[cfg(any(feature = "asil-c", feature = "asil-d"))]
        $crate::Error::new($crate::ErrorCategory::Safety, $code, $message)
    }};
}

/// Log an error with ASIL-appropriate detail level
///
/// - QM: Basic error logging
/// - ASIL-B: Include error category
/// - ASIL-C: Include error code and category
/// - ASIL-D: Full error details with integrity check
#[macro_export]
macro_rules! asil_log_error {
    ($error:expr) => {{
        #[cfg(feature = "asil-d")]
        {
            // ASIL-D: Full logging with integrity check
            let integrity = if $error.validate_integrity() { "VALID" } else { "INVALID" };
            format!(
                "[{}][E{:04X}][{}][{}] {}",
                format!("{:?}", $error.category),
                $error.code,
                $error.asil_level(),
                integrity,
                $error.message
            )
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            // ASIL-C: Detailed logging
            format!(
                "[{:?}][E{:04X}][{}] {}",
                $error.category,
                $error.code,
                $error.asil_level(),
                $error.message
            )
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            // ASIL-B: Category and message
            format!("[{:?}] {}", $error.category, $error.message)
        }

        #[cfg(not(any(feature = "asil-b", feature = "asil-c", feature = "asil-d")))]
        {
            // QM: Basic message only
            format!("{}", $error.message)
        }
    }};
}
