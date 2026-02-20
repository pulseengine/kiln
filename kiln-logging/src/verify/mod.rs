//! Verification module for wrt-logging using Kani.
//!
//! This module contains verification harnesses for the wrt-logging crate.
//! It is only included when the `kani` feature is enabled.

use super::*;

#[cfg(kani)]
#[kani::proof]
fn verify_log_level() {
    // Verify that log levels can be converted to/from strings
    assert_eq!(LogLevel::from_string_or_default("info"), LogLevel::Info);
    assert_eq!(LogLevel::from_string_or_default("debug"), LogLevel::Debug);
    assert_eq!(LogLevel::from_string_or_default("warning"), LogLevel::Warn);
    assert_eq!(LogLevel::from_string_or_default("error"), LogLevel::Error);
    assert_eq!(LogLevel::from_string_or_default("invalid"), LogLevel::Info); // Default

    // Test string representation
    assert_eq!(LogLevel::Info.as_str(), "info");
    assert_eq!(LogLevel::Debug.as_str(), "debug");
    assert_eq!(LogLevel::Warn.as_str(), "warn");
    assert_eq!(LogLevel::Error.as_str(), "error");
}

#[cfg(kani)]
#[kani::proof]
fn verify_log_operation() {
    // Create a log operation
    let op = LogOperation::new(LogLevel::Info, "test message".to_string());

    // Verify fields
    assert_eq!(op.level, LogLevel::Info);
    assert_eq!(op.message, "test message");
    assert!(op.component_id.is_none());

    // Create with component ID
    let op_with_id = LogOperation::with_component(LogLevel::Debug, "test message", "component-1");

    // Verify fields
    assert_eq!(op_with_id.level, LogLevel::Debug);
    assert_eq!(op_with_id.message, "test message");
    assert_eq!(op_with_id.component_id, Some("component-1".to_string()));
}

#[cfg(kani)]
#[kani::proof]
fn verify_logging_ext() {
    #[cfg(feature = "std")]
    {
        use wrt_host::CallbackRegistry;

        use crate::handler::LoggingExt;

        // Create a registry
        let mut registry = CallbackRegistry::new();

        // Add handler (using LoggingExt trait)
        let registry_with_handler = {
            let mut r = CallbackRegistry::new();
            r.register_log_handler(|_| {});
            r
        };

        // Verify that we can log
        registry_with_handler.handle_log(LogOperation::new(
            LogLevel::Info,
            "test message".to_string(),
        ));
    }
}
