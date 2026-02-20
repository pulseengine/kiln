//! Log level definitions for the WebAssembly Runtime.
//!
//! This module provides types for representing log levels in component logging.

use core::str::FromStr;

/// Log levels for WebAssembly component logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    /// Trace-level messages (detailed debugging information)
    Trace,
    /// Debug-level messages (useful for developers)
    Debug,
    /// Informational messages (general runtime information)
    #[default]
    Info,
    /// Warning messages (potential issues)
    Warn,
    /// Error messages (recoverable errors)
    Error,
    /// Critical error messages (severe issues)
    Critical,
}

/// Custom error for parsing log levels
#[derive(Debug)]
pub struct ParseLogLevelError {
    /// Static error message
    pub message: &'static str,
}

#[cfg(feature = "std")]
impl core::fmt::Display for ParseLogLevelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid log level: {}", self.message)
    }
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for ParseLogLevelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid log level: {}", self.message)
    }
}

#[cfg(feature = "std")]
impl core::error::Error for ParseLogLevelError {}

#[cfg(feature = "std")]
impl FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" | "err" => Ok(Self::Error),
            "critical" | "fatal" => Ok(Self::Critical),
            _ => Err(ParseLogLevelError {
                message: "Invalid log level",
            }),
        }
    }
}

#[cfg(not(feature = "std"))]
impl FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        // Manual case-insensitive matching for no_std
        if s.eq_ignore_ascii_case("trace") {
            Ok(Self::Trace)
        } else if s.eq_ignore_ascii_case("debug") {
            Ok(Self::Debug)
        } else if s.eq_ignore_ascii_case("info") {
            Ok(Self::Info)
        } else if s.eq_ignore_ascii_case("warn") || s.eq_ignore_ascii_case("warning") {
            Ok(Self::Warn)
        } else if s.eq_ignore_ascii_case("error") || s.eq_ignore_ascii_case("err") {
            Ok(Self::Error)
        } else if s.eq_ignore_ascii_case("critical") || s.eq_ignore_ascii_case("fatal") {
            Ok(Self::Critical)
        } else {
            Err(ParseLogLevelError {
                message: "Invalid log level",
            })
        }
    }
}

impl LogLevel {
    /// Creates a `LogLevel` from a string, defaulting to Info for invalid
    /// levels
    #[must_use]
    pub fn from_string_or_default(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::Info)
    }

    /// Convert `LogLevel` to a string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        // Test valid log levels
        assert_eq!("trace".parse::<LogLevel>().unwrap(), LogLevel::Trace);
        assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("warning".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("error".parse::<LogLevel>().unwrap(), LogLevel::Error);
        assert_eq!("critical".parse::<LogLevel>().unwrap(), LogLevel::Critical);

        // Test case insensitivity
        assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("Warning".parse::<LogLevel>().unwrap(), LogLevel::Warn);

        // Test invalid log levels
        assert!("invalid".parse::<LogLevel>().is_err());
        assert!("".parse::<LogLevel>().is_err());

        // Test error message
        let err = "invalid".parse::<LogLevel>().unwrap_err();
        assert_eq!(err.message, "Invalid log level");
    }

    #[test]
    fn test_log_level_from_string_or_default() {
        assert_eq!(LogLevel::from_string_or_default("trace"), LogLevel::Trace);
        assert_eq!(LogLevel::from_string_or_default("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_string_or_default("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_string_or_default("warn"), LogLevel::Warn);
        assert_eq!(LogLevel::from_string_or_default("error"), LogLevel::Error);
        assert_eq!(
            LogLevel::from_string_or_default("critical"),
            LogLevel::Critical
        );

        // Test invalid defaults to Info
        assert_eq!(LogLevel::from_string_or_default("invalid"), LogLevel::Info);
        assert_eq!(LogLevel::from_string_or_default(""), LogLevel::Info);
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Trace.as_str(), "trace");
        assert_eq!(LogLevel::Debug.as_str(), "debug");
        assert_eq!(LogLevel::Info.as_str(), "info");
        assert_eq!(LogLevel::Warn.as_str(), "warn");
        assert_eq!(LogLevel::Error.as_str(), "error");
        assert_eq!(LogLevel::Critical.as_str(), "critical");
    }
}
