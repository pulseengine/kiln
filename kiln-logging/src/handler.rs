//! Log handler for the WebAssembly Runtime.
//!
//! This module provides types for handling logs from WebAssembly components.

#[cfg(feature = "std")]
use alloc::boxed::Box;

use wrt_host::{
    CallbackRegistry,
    CallbackType,
};

use crate::operation::LogOperation;

// Binary std/no_std choice
#[cfg(feature = "std")]
/// Function type for handling log operations
pub type LogHandler = Box<dyn Fn(LogOperation) + Send + Sync>;

#[cfg(feature = "std")]
/// Extension trait for CallbackRegistry to add logging-specific methods
pub trait LoggingExt {
    /// Register a log handler
    fn register_log_handler<F>(&mut self, handler: F)
    where
        F: Fn(LogOperation) + Send + Sync + 'static;

    /// Handle a log operation
    fn handle_log(&self, operation: LogOperation);

    /// Check if a log handler is registered
    fn has_log_handler(&self) -> bool;
}

// For pure no_std configuration
#[cfg(all(not(feature = "std"), not(feature = "std")))]
/// Function type for handling log operations (no dynamic dispatch in `no_std`)
pub type LogHandler = fn(LogOperation);

#[cfg(all(not(feature = "std"), not(feature = "std")))]
/// Extension trait for `CallbackRegistry` to add logging-specific methods
/// (`no_std`)
pub trait LoggingExt {
    /// Register a simple log handler function (`no_std` only supports function
    /// pointers)
    fn register_log_handler(&mut self, handler: LogHandler);

    /// Handle a log operation
    fn handle_log(&self, operation: LogOperation) -> wrt_error::Result<()>;

    /// Check if a log handler is registered
    fn has_log_handler(&self) -> bool;
}

// Binary std/no_std choice
#[cfg(feature = "std")]
impl LoggingExt for CallbackRegistry {
    fn register_log_handler<F>(&mut self, handler: F)
    where
        F: Fn(LogOperation) + Send + Sync + 'static,
    {
        self.register_callback(CallbackType::Logging, Box::new(handler) as LogHandler);
    }

    fn handle_log(&self, operation: LogOperation) {
        if let Some(handler) = self.get_callback::<LogHandler>(&CallbackType::Logging) {
            handler(operation);
        }
    }

    fn has_log_handler(&self) -> bool {
        self.get_callback::<LogHandler>(&CallbackType::Logging).is_some()
    }
}

// Implementation for pure no_std configuration
#[cfg(all(not(feature = "std"), not(feature = "std")))]
impl LoggingExt for CallbackRegistry {
    fn register_log_handler(&mut self, handler: LogHandler) {
        // In no_std mode, we can't store dynamic handlers
        // This is a limitation - only one handler per type can be stored
        let _ = handler; // Acknowledge the parameter
                         // Note: Actual registration would require a more
                         // complex design
    }

    fn handle_log(&self, operation: LogOperation) -> wrt_error::Result<()> {
        // In no_std mode, we can't dynamically dispatch to handlers
        let _ = operation; // Acknowledge the parameter
                           // Default no-op implementation for no_std
        Ok(())
    }

    fn has_log_handler(&self) -> bool {
        // In no_std mode, we can't track handlers dynamically
        false
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use std::sync::{
        Arc,
        Mutex,
    };

    use super::*;
    use crate::level::LogLevel;

    #[test]
    fn test_logging_extension() {
        let mut registry = CallbackRegistry::new();

        // Test without handler
        assert!(!registry.has_log_handler());

        // Logging without handler should not panic
        registry.handle_log(LogOperation::new(
            LogLevel::Info,
            "test message".to_string(),
        ));

        // Register handler
        let received = Arc::new(Mutex::new(Vec::new()));
        {
            let received = received.clone();
            registry.register_log_handler(move |log_op| {
                received.lock().unwrap().push((log_op.level, log_op.message));
            });
        }

        // Test with handler
        assert!(registry.has_log_handler());

        // Log some messages
        registry.handle_log(LogOperation::new(
            LogLevel::Info,
            "info message".to_string(),
        ));

        registry.handle_log(LogOperation::new(
            LogLevel::Error,
            "error message".to_string(),
        ));

        // Check received messages
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0], (LogLevel::Info, "info message".to_string()));
        assert_eq!(received[1], (LogLevel::Error, "error message".to_string()));
    }
}

// Test for no_std configuration
#[cfg(test)]
#[cfg(all(not(feature = "std"), not(feature = "std")))]
mod no_std_alloc_tests {
    use super::*;
    use crate::level::LogLevel;

    #[test]
    fn test_no_std_logging_extension() {
        let mut registry = CallbackRegistry::new();

        // Test without handler
        assert!(!registry.has_log_handler());

        // Logging without handler should not panic
        let _ = registry.handle_log(LogOperation::new(
            LogLevel::Info,
            "test message",
        ).unwrap());

        // Register a simple function handler
        fn simple_handler(_log_op: LogOperation) {
            // No-op handler for testing
        }

        registry.register_log_handler(simple_handler);

        // Test with handler (no_std mode doesn't actually store handlers)
        assert!(!registry.has_log_handler());

        // Log some messages (no_std mode is no-op)
        let _ = registry.handle_log(LogOperation::new(
            LogLevel::Info,
            "info message",
        ).unwrap());
        let _ = registry.handle_log(LogOperation::new(
            LogLevel::Error,
            "error message",
        ).unwrap());

        // In no_std mode, handlers are not actually registered or called
        // This test just verifies the API compiles and doesn't panic
    }
}
