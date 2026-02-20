//! Tests for error conversion and `FromError` implementations

use wrt_error::{
    Error, ErrorCategory, codes,
    kinds::{ComponentError, ParseError, ResourceError, RuntimeError, ValidationError},
};

#[test]
fn test_error_from_validation_error() {
    let validation_error = ValidationError("test validation error");
    let error: Error = Error::from(validation_error);

    // Using ErrorSource trait methods
    assert_eq!(error.category, ErrorCategory::Validation);
    assert_eq!(error.code, codes::VALIDATION_ERROR);
}

#[test]
fn test_error_from_parse_error() {
    let parse_error = ParseError("test parse error");
    let error: Error = Error::from(parse_error);

    assert_eq!(error.category, ErrorCategory::Runtime);
    assert_eq!(error.code, codes::PARSE_ERROR);
}

#[test]
fn test_error_categories() {
    // Test different error categories individually
    let validation_error = ValidationError("test");
    let error: Error = Error::from(validation_error);
    assert_eq!(error.category, ErrorCategory::Validation);

    let parse_error = ParseError("test");
    let error: Error = Error::from(parse_error);
    assert_eq!(error.category, ErrorCategory::Runtime);

    // ExecutionError only has FromError, not From implementation
    // So we skip it for now

    let component_error = ComponentError("test");
    let error: Error = Error::from(component_error);
    assert_eq!(error.category, ErrorCategory::Component);

    let resource_error = ResourceError("test");
    let error: Error = Error::from(resource_error);
    assert_eq!(error.category, ErrorCategory::Resource);

    let runtime_error = RuntimeError("test");
    let error: Error = Error::from(runtime_error);
    assert_eq!(error.category, ErrorCategory::Runtime);
}

#[test]
fn test_error_codes_consistency() {
    // Test that specific errors map to correct codes
    let validation_error = ValidationError("test");
    let error: Error = Error::from(validation_error);

    assert_eq!(error.code, codes::VALIDATION_ERROR);
    assert!(
        error.code >= 5000 && error.code < 6000,
        "Validation errors should be in 5000-5999 range"
    );
}

#[test]
fn test_error_messages() {
    let parse_error = ParseError("custom parse message");
    let error: Error = Error::from(parse_error);

    assert!(!error.message.is_empty());
    // Message should contain some indication of the error type
    let message = error.message.to_lowercase();
    assert!(message.contains("parse") || message.contains("error"));
}

#[test]
fn test_specific_error_instances() {
    // Test specific error instances that we know have From implementations
    let validation_error = ValidationError("test instance");
    let error: Error = Error::from(validation_error);

    assert_eq!(error.code, codes::VALIDATION_ERROR);
    assert_eq!(error.category, ErrorCategory::Validation);
}

#[test]
fn test_runtime_vs_validation_categories() {
    let validation_error = ValidationError("validation issue");
    let runtime_error = RuntimeError("runtime issue");

    let val_error: Error = Error::from(validation_error);
    let run_error: Error = Error::from(runtime_error);

    assert_eq!(val_error.category, ErrorCategory::Validation);
    assert_eq!(run_error.category, ErrorCategory::Runtime);
    assert_ne!(val_error.category, run_error.category);
}

#[test]
fn test_error_code_ranges() {
    // Test that error codes fall within expected ranges
    let validation_error = ValidationError("test");
    let error: Error = Error::from(validation_error);
    assert!(
        error.code >= 5000 && error.code < 6000,
        "Validation error code {} not in expected range 5000-5999",
        error.code
    );

    let runtime_error = RuntimeError("test");
    let error: Error = Error::from(runtime_error);
    assert!(
        error.code >= 7000 && error.code < 8000,
        "Runtime error code {} not in expected range 7000-7999",
        error.code
    );

    let component_error = ComponentError("test");
    let error: Error = Error::from(component_error);
    assert!(
        error.code >= 2000 && error.code < 3000,
        "Component error code {} not in expected range 2000-2999",
        error.code
    );
}

#[test]
fn test_error_debug_output() {
    let error = ValidationError("test validation");
    let debug_output = format!("{error:?}");
    assert!(!debug_output.is_empty());
    // Should contain some meaningful information
    assert!(debug_output.len() > 10);
}
