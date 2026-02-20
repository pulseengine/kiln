//! Error recovery and debugging infrastructure
//!
//! This module provides comprehensive error recovery mechanisms and detailed
//! debugging information for the WRT runtime system.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::HashMap;

use crate::{Error, ErrorCategory, Result};

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Error recovery strategy
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RecoveryStrategy {
    /// Abort execution immediately
    #[default]
    Abort,
    /// Skip the problematic operation and continue
    Skip,
    /// Use a default value and continue
    UseDefault,
    /// Retry with different parameters
    Retry {
        /// Maximum number of retry attempts allowed
        max_attempts: u32,
    },
    /// Log error and continue
    LogAndContinue,
}

/// Error context for debugging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Location where the error occurred
    pub location: String,
    /// Additional context information
    pub context: HashMap<String, String>,
    /// Stack trace if available
    pub stack_trace: Vec<String>,
    /// Recovery strategy to use
    pub recovery_strategy: RecoveryStrategy,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(location: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            context: HashMap::new(),
            stack_trace: Vec::new(),
            recovery_strategy: RecoveryStrategy::default(),
        }
    }

    /// Add context information
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Add stack frame
    #[must_use]
    pub fn with_stack_frame(mut self, frame: impl Into<String>) -> Self {
        self.stack_trace.push(frame.into());
        self
    }

    /// Set recovery strategy
    #[must_use]
    pub const fn with_recovery(mut self, strategy: RecoveryStrategy) -> Self {
        self.recovery_strategy = strategy;
        self
    }
}

/// Error recovery manager
#[derive(Debug)]
pub struct ErrorRecoveryManager {
    /// Global recovery strategies by error category
    strategies: HashMap<ErrorCategory, RecoveryStrategy>,
    /// Error history for pattern detection
    error_history: Vec<(Error, ErrorContext)>,
    /// Maximum error history size
    max_history: usize,
}

impl Default for ErrorRecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager
    #[must_use]
    pub fn new() -> Self {
        let mut strategies = HashMap::new();

        // Set default recovery strategies
        strategies.insert(ErrorCategory::Parse, RecoveryStrategy::Skip);
        strategies.insert(ErrorCategory::Type, RecoveryStrategy::LogAndContinue);
        strategies.insert(ErrorCategory::Runtime, RecoveryStrategy::Abort);
        strategies.insert(ErrorCategory::Memory, RecoveryStrategy::Abort);
        strategies.insert(ErrorCategory::Validation, RecoveryStrategy::UseDefault);

        Self {
            strategies,
            error_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Set recovery strategy for an error category
    pub fn set_strategy(&mut self, category: ErrorCategory, strategy: RecoveryStrategy) {
        self.strategies.insert(category, strategy);
    }

    /// Get recovery strategy for an error category
    #[must_use]
    pub fn get_strategy(&self, category: &ErrorCategory) -> RecoveryStrategy {
        self.strategies.get(category).cloned().unwrap_or_default()
    }

    /// Record an error with context
    pub fn record_error(&mut self, error: Error, context: ErrorContext) {
        self.error_history.push((error, context));

        // Limit history size
        if self.error_history.len() > self.max_history {
            self.error_history.remove(0);
        }
    }

    /// Analyze error patterns
    #[must_use]
    pub fn analyze_patterns(&self) -> ErrorPatternAnalysis {
        let mut category_counts = HashMap::new();
        let mut location_counts = HashMap::new();
        let mut recent_errors = Vec::new();

        for (error, context) in &self.error_history {
            // Count by category
            *category_counts.entry(error.category).or_insert(0) += 1;

            // Count by location
            *location_counts.entry(context.location.clone()).or_insert(0) += 1;

            // Collect recent errors (last 10)
            if recent_errors.len() < 10 {
                recent_errors.push((*error, context.clone()));
            }
        }

        ErrorPatternAnalysis {
            total_errors: self.error_history.len(),
            category_counts,
            location_counts,
            recent_errors,
        }
    }

    /// Attempt error recovery
    #[must_use]
    pub fn recover(&self, error: &Error, context: &ErrorContext) -> RecoveryResult {
        let strategy = match &context.recovery_strategy {
            RecoveryStrategy::Abort => &context.recovery_strategy,
            _ => self.strategies.get(&error.category).unwrap_or(&RecoveryStrategy::Abort),
        };

        match strategy {
            RecoveryStrategy::Abort => RecoveryResult::Abort,
            RecoveryStrategy::Skip => RecoveryResult::Skip,
            RecoveryStrategy::UseDefault => RecoveryResult::UseDefault,
            RecoveryStrategy::Retry { max_attempts } => RecoveryResult::Retry {
                attempts_left: *max_attempts,
            },
            RecoveryStrategy::LogAndContinue => {
                #[cfg(feature = "std")]
                {
                    use std::println;
                    println!("Warning: {} at {}", error.message, context.location);
                }
                RecoveryResult::Continue
            },
        }
    }
}

/// Result of error recovery attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryResult {
    /// Abort execution
    Abort,
    /// Skip the operation
    Skip,
    /// Use a default value
    UseDefault,
    /// Retry the operation
    Retry {
        /// Number of retry attempts remaining
        attempts_left: u32,
    },
    /// Continue execution
    Continue,
}

/// Error pattern analysis results
#[derive(Debug, Clone)]
pub struct ErrorPatternAnalysis {
    /// Total number of errors recorded
    pub total_errors: usize,
    /// Count of errors by category
    pub category_counts: HashMap<ErrorCategory, usize>,
    /// Count of errors by location
    pub location_counts: HashMap<String, usize>,
    /// Most recent errors
    pub recent_errors: Vec<(Error, ErrorContext)>,
}

impl ErrorPatternAnalysis {
    /// Get the most frequent error category
    #[must_use]
    pub fn most_frequent_category(&self) -> Option<ErrorCategory> {
        self.category_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(category, _)| *category)
    }

    /// Get the most problematic location
    #[must_use]
    pub fn most_problematic_location(&self) -> Option<String> {
        self.location_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(location, _)| location.clone())
    }

    /// Check if error rate is concerning
    #[must_use]
    pub fn is_error_rate_high(&self) -> bool {
        self.total_errors > 50 || self.category_counts.values().any(|&count| count > 10)
    }
}

/// Enhanced error with recovery context
#[derive(Debug, Clone)]
pub struct RecoverableError {
    /// The underlying error
    pub error: Error,
    /// Recovery context
    pub context: ErrorContext,
    /// Suggested recovery action
    pub recovery_suggestion: RecoveryResult,
}

impl RecoverableError {
    /// Create a new recoverable error
    #[must_use]
    pub fn new(error: Error, context: ErrorContext) -> Self {
        let manager = ErrorRecoveryManager::new();
        let recovery_suggestion = manager.recover(&error, &context);

        Self {
            error,
            context,
            recovery_suggestion,
        }
    }

    /// Convert to a standard Result
    ///
    /// # Errors
    /// Returns an error if the recovery result is `Abort` or `Retry`
    pub fn into_result<T>(self) -> Result<Option<T>> {
        match self.recovery_suggestion {
            RecoveryResult::Skip | RecoveryResult::Continue | RecoveryResult::UseDefault => {
                Ok(None)
            },
            RecoveryResult::Retry { .. } | RecoveryResult::Abort => Err(self.error),
        }
    }
}

/// Debugging utilities
pub struct DebugUtils;

impl DebugUtils {
    /// Format error with full debugging information
    #[must_use]
    pub fn format_detailed_error(error: &Error, context: &ErrorContext) -> String {
        use core::fmt::Write;

        let mut output = String::new();

        let _ = writeln!(output, "Error: {} (Code: {})", error.message, error.code);
        let _ = writeln!(output, "Category: {:?}", error.category);
        let _ = writeln!(output, "Location: {}", context.location);

        if !context.context.is_empty() {
            output.push_str("Context:\n");
            for (key, value) in &context.context {
                let _ = writeln!(output, "  {key}: {value}");
            }
        }

        if !context.stack_trace.is_empty() {
            output.push_str("Stack trace:\n");
            for (i, frame) in context.stack_trace.iter().enumerate() {
                let _ = writeln!(output, "  {i}: {frame}");
            }
        }

        let _ = writeln!(output, "Recovery strategy: {:?}", context.recovery_strategy);

        output
    }

    /// Create error context for a function
    #[must_use]
    pub fn function_context(function_name: &str, module: &str, line: u32) -> ErrorContext {
        ErrorContext::new(format!("{module}:{line} in {function_name}"))
            .with_context("function", function_name)
            .with_context("module", module)
            .with_context("line", format!("{line}"))
    }

    /// Create error context for WASM operations
    #[must_use]
    pub fn wasm_context(
        operation: &str,
        instruction_offset: usize,
        function_index: Option<u32>,
    ) -> ErrorContext {
        let mut ctx = ErrorContext::new(format!("WASM {operation} at offset {instruction_offset}"))
            .with_context("operation", operation)
            .with_context("offset", format!("{instruction_offset}"));

        if let Some(func_idx) = function_index {
            ctx = ctx.with_context("function_index", format!("{func_idx}"));
        }

        ctx
    }
}

/// Macro for creating error contexts with file/line information
#[macro_export]
macro_rules! error_context {
    ($location:expr) => {
        $crate::recovery::ErrorContext::new(format!("{}:{} in {}", file!(), line!(), $location))
            .with_context("file", file!())
            .with_context("line", format!("{}", line!()))
    };
    ($location:expr, $($key:expr => $value:expr),+ $(,)?) => {
        {
            let mut ctx = $crate::recovery::ErrorContext::new(format!("{}:{} in {}", file!(), line!(), $location))
                .with_context("file", file!())
                .with_context("line", format!("{}", line!()));
            $(
                ctx = ctx.with_context($key, $value);
            )+
            ctx
        }
    };
}

/// Macro for recoverable operations
#[macro_export]
macro_rules! recoverable {
    ($expr:expr, $context:expr) => {
        match $expr {
            Ok(value) => Ok(value),
            Err(error) => {
                let recoverable = $crate::recovery::RecoverableError::new(error, $context);
                match recoverable.recovery_suggestion {
                    $crate::recovery::RecoveryResult::Continue
                    | $crate::recovery::RecoveryResult::Skip => {
                        // Log and continue
                        #[cfg(feature = "std")]
                        eprintln!("Recovered from error: {}", recoverable.error.message);
                        return recoverable.into_result();
                    },
                    _ => Err(recoverable.error),
                }
            },
        }
    };
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;

    use super::*;
    use crate::codes;

    #[test]
    fn test_error_recovery_manager() {
        let mut manager = ErrorRecoveryManager::new();

        // Test setting and getting strategies
        manager.set_strategy(ErrorCategory::Parse, RecoveryStrategy::Skip);
        assert_eq!(
            manager.get_strategy(&ErrorCategory::Parse),
            RecoveryStrategy::Skip
        );

        // Test error recording
        let error = Error::new(ErrorCategory::Parse, codes::PARSE_ERROR, "Test error");
        let context = ErrorContext::new("test_location");
        manager.record_error(error, context);

        assert_eq!(manager.error_history.len(), 1);
    }

    #[test]
    fn test_error_context() {
        let context = ErrorContext::new("test_function")
            .with_context("param", "value")
            .with_stack_frame("frame1")
            .with_recovery(RecoveryStrategy::Skip);

        assert_eq!(context.location, "test_function");
        assert_eq!(context.context.get("param"), Some(&"value".to_string()));
        assert_eq!(context.stack_trace.len(), 1);
        assert_eq!(context.recovery_strategy, RecoveryStrategy::Skip);
    }

    #[test]
    fn test_pattern_analysis() {
        let mut manager = ErrorRecoveryManager::new();

        // Add multiple errors
        for i in 0..5 {
            let error = Error::new(ErrorCategory::Parse, codes::PARSE_ERROR, "Test error");
            let context = ErrorContext::new(format!("location_{}", i % 2));
            manager.record_error(error, context);
        }

        let analysis = manager.analyze_patterns();
        assert_eq!(analysis.total_errors, 5);
        assert_eq!(
            analysis.category_counts.get(&ErrorCategory::Parse),
            Some(&5)
        );
    }
}
