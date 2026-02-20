//! Tests for ASIL-specific error handling features

#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
use wrt_error::{AsilErrorContext, AsilLevel};
use wrt_error::{Error, ErrorCategory, codes};

#[test]
fn test_basic_error_creation() {
    let error = Error::memory_out_of_bounds("Memory bounds check failed");
    assert_eq!(error.code, codes::MEMORY_OUT_OF_BOUNDS);
    assert_eq!(error.category, ErrorCategory::Memory);
}

#[cfg(any(
    feature = "asil-a",
    feature = "asil-b",
    feature = "asil-c",
    feature = "asil-d"
))]
#[test]
fn test_asil_level_detection() {
    let error = Error::safety_violation("Safety violation");
    let asil_level = error.asil_level();

    // Safety errors should be ASIL-D
    assert_eq!(asil_level, "ASIL-D");

    // Memory errors should be ASIL-C
    let mem_error = Error::memory_out_of_bounds("Memory allocation failed");
    assert_eq!(mem_error.asil_level(), "ASIL-C");

    // Validation errors should be ASIL-B
    let val_error = Error::validation_error("Validation error");
    assert_eq!(val_error.asil_level(), "ASIL-B");
}

#[cfg(any(feature = "asil-c", feature = "asil-d"))]
#[test]
fn test_safe_state_requirements() {
    // Safety errors require safe state
    let safety_error = Error::safety_violation("Critical error");
    assert!(safety_error.requires_safe_state());

    // Memory errors require safe state
    let memory_error = Error::memory_out_of_bounds("Memory allocation failed");
    assert!(memory_error.requires_safe_state());

    // Runtime trap errors require safe state
    let trap_error = Error::new(
        ErrorCategory::RuntimeTrap,
        codes::RUNTIME_TRAP_ERROR,
        "Runtime trap detected",
    );
    assert!(trap_error.requires_safe_state());

    // Component errors do not require immediate safe state
    let comp_error = Error::new(
        ErrorCategory::Component,
        codes::COMPONENT_INSTANTIATION_ERROR,
        "Component runtime error",
    );
    assert!(!comp_error.requires_safe_state());
}

#[cfg(feature = "std")]
#[test]
fn test_error_integrity_validation() {
    // Valid error
    let valid_error = Error::memory_out_of_bounds("Valid memory error");
    #[cfg(feature = "asil-d")]
    assert!(valid_error.validate_integrity());
    #[cfg(not(feature = "asil-d"))]
    let _ = valid_error;

    // Error with valid code for category
    let type_error = Error::type_mismatch_error("Type validation failed");
    #[cfg(feature = "asil-d")]
    assert!(type_error.validate_integrity());
    #[cfg(not(feature = "asil-d"))]
    let _ = type_error;

    // Note: We can't easily test invalid errors in const fn context
}

#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
#[test]
fn test_asil_error_context() {
    let error = Error::safety_violation("Test error");
    let context = AsilErrorContext::new(error).with_timestamp(123_456_789).with_module_id(42);

    assert_eq!(context.error.code, error.code);
    assert_eq!(context.timestamp, Some(123_456_789));
    assert_eq!(context.module_id, Some(42));
    assert!(context.requires_immediate_action());
}

#[cfg(any(feature = "asil-c", feature = "asil-d"))]
#[test]
fn test_safety_monitor() {
    use wrt_error::SafetyMonitor;

    let monitor = SafetyMonitor::new();
    assert_eq!(monitor.error_count(), 0);

    // Record some errors
    let error1 = Error::memory_out_of_bounds("Memory error");
    let error2 = Error::safety_violation("Safety error");

    monitor.record_error(&error1);
    monitor.record_error(&error2);

    assert_eq!(monitor.error_count(), 2);

    // Reset monitor
    monitor.reset();
    assert_eq!(monitor.error_count(), 0);
}

#[test]
fn test_error_display_format() {
    let error = Error::memory_out_of_bounds("Test error");
    let display = format!("{error}");

    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    {
        // ASIL-C/D includes ASIL level in display
        assert!(display.contains("[Memory]"));
        assert!(display.contains("[E0FA0]")); // MEMORY_OUT_OF_BOUNDS = 4000 = 0x0FA0
        assert!(display.contains("[ASIL-C]")); // Memory errors are ASIL-C
        assert!(display.contains("Test error"));
    }

    #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
    {
        // Standard format without ASIL level
        assert!(display.contains("[Memory]"));
        assert!(display.contains("[E0FA0]")); // MEMORY_OUT_OF_BOUNDS = 4000 = 0x0FA0
        assert!(display.contains("Test error"));
        assert!(!display.contains("ASIL"));
    }
}

#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
#[test]
fn test_current_asil_level() {
    let current = AsilLevel::current();

    #[cfg(feature = "asil-d")]
    assert_eq!(current, AsilLevel::AsilD);

    #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
    assert_eq!(current, AsilLevel::AsilC);

    #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
    assert_eq!(current, AsilLevel::AsilB);

    // Test requirement checking
    assert!(AsilLevel::meets_requirement(AsilLevel::QM));

    #[cfg(feature = "asil-d")]
    {
        assert!(AsilLevel::meets_requirement(AsilLevel::AsilD));
        assert!(AsilLevel::meets_requirement(AsilLevel::AsilC));
        assert!(AsilLevel::meets_requirement(AsilLevel::AsilB));
    }
}
