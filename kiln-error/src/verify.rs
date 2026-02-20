//! Formal verification for the error handling system using Kani.
//!
//! This module contains proofs that verify core properties of the error
//! handling system. These proofs only run with Kani and are isolated from
//! normal compilation and testing.

// Only compile Kani verification code when documentation is being generated
// or when explicitly running cargo kani. This prevents interference with
// coverage testing.
#[cfg(any(doc, kani))]
/// Kani verification proofs for error handling.
pub mod kani_verification {
    use core::fmt::{self, Debug, Display};
    #[cfg(feature = "std")]
    use std::format;

    // Use crate::Error directly, remove ResultExt if it was here.
    use crate::{Error, ErrorCategory, ErrorSource, Result, codes}; // Added Error, codes

    // A simple test error for verification
    #[derive(Debug, Clone, Copy)] // Made it Copy since it's just a &'static str
    struct VerifyError(&'static str);

    impl Display for VerifyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "VerifyError: {}", self.0)
        }
    }

    impl ErrorSource for VerifyError {
        // Removed: #[cfg(feature = "std")] fn source(&self) -> Option<&(dyn ErrorSource
        // + 'static)> This was for a different ErrorSource trait definition.
        // The current crate::ErrorSource does not have a source() method by default.

        fn code(&self) -> u16 {
            // Use a fixed code for verification errors
            codes::UNKNOWN // Using a generic code, or define a specific one
        }

        fn message(&self) -> &'static str {
            // Ensure &'static str
            self.0
        }

        fn category(&self) -> ErrorCategory {
            // Use validation category for verification errors
            ErrorCategory::Validation
        }
    }

    // Implement From<VerifyError> for crate::Error
    impl From<VerifyError> for Error {
        fn from(ve: VerifyError) -> Self {
            Error::new(ve.category(), ve.code(), ve.message())
        }
    }

    /// Verify that creating and displaying an error works correctly
    #[cfg(feature = "std")] // Retaining for format! usage
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_error_creation_and_display() {
        // Renamed
        let verify_err = VerifyError("verification test");
        let error = Error::from(verify_err);

        // Direct field assertions
        assert_eq!(error.category, ErrorCategory::Validation);
        assert_eq!(error.code, codes::UNKNOWN); // Or the specific code used
        assert_eq!(error.message, "verification test");

        // Binary std/no_std choice
        let error_str = format!("{}", error);
        // Example: "[Validation][E270F] verification test" if UNKNOWN is 9999 (0x270F)
        // For now, check for essential parts.
        assert!(error_str.contains("[Validation]")); // Check category part
        assert!(error_str.contains(&format!("[E{:04X}]", codes::UNKNOWN))); // Check code part
        assert!(error_str.contains(" verification test")); // Check message part
    }

    // Removed verify_error_context and verify_multiple_contexts as ResultExt is
    // gone.

    /// Verify that factory methods create the correct error types
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_factory_methods() {
        // Test a few existing factory methods from errors.rs
        let core_err = Error::core_error("test core error");
        assert_eq!(core_err.category, ErrorCategory::Core);
        assert_eq!(core_err.code, codes::EXECUTION_ERROR);
        assert_eq!(core_err.message, "test core error");

        let type_err = Error::type_error("test type error");
        assert_eq!(type_err.category, ErrorCategory::Type);
        assert_eq!(type_err.code, codes::TYPE_MISMATCH_ERROR);
        assert_eq!(type_err.message, "test type error");

        // Test conversion from a 'kind' error
        let kind_validation_err = crate::kinds::validation_error("kind validation");
        let error_from_kind = Error::from(kind_validation_err);
        assert_eq!(error_from_kind.category, ErrorCategory::Validation);
        // The From<kinds::ValidationError> for Error impl uses codes::VALIDATION_ERROR
        assert_eq!(error_from_kind.code, codes::VALIDATION_ERROR);
        assert_eq!(error_from_kind.message, "kind validation");
    }

    /// Verify that Error::from works correctly for our VerifyError
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_error_from_verify_error() {
        // Renamed for clarity
        let original_error = VerifyError("source error");
        let error = Error::from(original_error);

        assert_eq!(error.category, ErrorCategory::Validation);
        assert_eq!(error.code, codes::UNKNOWN);
        assert_eq!(error.message, "source error");
    }

    /// Verify that Result works correctly with different error types
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_result_type() {
        // Test with wrt_error::Error
        let result1: Result<i32> = Ok(42); // Result<T> is core::result::Result<T, crate::Error>
        assert_eq!(result1.unwrap(), 42);

        let sample_error = Error::runtime_error("sample runtime error for result");
        let result2: Result<i32> = Err(sample_error);
        assert!(result2.is_err());
        if let Err(e) = result2 {
            assert_eq!(e.category, ErrorCategory::Runtime);
            assert_eq!(e.message, "sample runtime error for result");
        }

        // Test with a custom error type (VerifyError)
        // This uses core::result::Result<T, VerifyError>, not crate::Result
        let result3: core::result::Result<i32, VerifyError> = Ok(42);
        assert_eq!(result3.unwrap(), 42);

        let verify_err_instance = VerifyError("custom error for result");
        let result4: core::result::Result<i32, VerifyError> = Err(verify_err_instance);
        assert!(result4.is_err());
        if let Err(ve) = result4 {
            assert_eq!(ve.0, "custom error for result");
        }
    }
}

// Expose the verification module in docs but not for normal compilation
#[cfg(any(doc, kani))]
pub use kani_verification::*;
