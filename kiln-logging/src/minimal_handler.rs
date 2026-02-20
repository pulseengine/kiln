//! Minimal logging handler for pure no_std environments.
//!
//! This module provides a minimal implementation of logging functionality
//! that works in pure no_std environments without allocation.

use crate::level::LogLevel;

/// Minimal log message for pure `no_std` environments.
///
/// This struct is used as a simplified version of `LogOperation` when
/// Binary `std/no_std` choice
/// static message.
#[derive(Debug, Clone, Copy)]
pub struct MinimalLogMessage {
    /// Log level
    pub level:   LogLevel,
    /// Static log message
    pub message: &'static str,
}

impl MinimalLogMessage {
    /// Create a new minimal log message with static lifetime
    #[must_use]
    pub const fn new(level: LogLevel, message: &'static str) -> Self {
        Self { level, message }
    }
}

/// Minimal log handler for pure `no_std` environments.
///
/// This trait provides a simplified logging interface that doesn't
/// Binary `std/no_std` choice
pub trait MinimalLogHandler {
    /// Handle a minimal log message
    ///
    /// # Errors
    ///
    /// Returns an error if the log message cannot be processed
    fn handle_minimal_log(&self, level: LogLevel, message: &'static str) -> crate::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_log_message() {
        let msg = MinimalLogMessage::new(LogLevel::Info, "test message");
        assert_eq!(msg.level, LogLevel::Info);
        assert_eq!(msg.message, "test message");

        // Test copying
        let msg2 = msg;
        assert_eq!(msg2.level, LogLevel::Info);
        assert_eq!(msg2.message, "test message");
    }
}
