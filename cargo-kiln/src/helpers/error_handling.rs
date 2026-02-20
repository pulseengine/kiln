//! Standardized error handling for cargo-wrt commands
//!
//! Provides consistent error formatting, categorization, and reporting
//! across all cargo-wrt commands.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json;
use wrt_build_core::formatters::OutputFormat;

use super::OutputManager;

/// Error categories for consistent error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Build system errors (compilation, linking, etc.)
    Build,
    /// Configuration errors (invalid settings, missing files, etc.)
    Configuration,
    /// I/O errors (file not found, permission denied, etc.)
    Io,
    /// Network errors (download failures, connection issues, etc.)
    Network,
    /// Validation errors (invalid input, constraint violations, etc.)
    Validation,
    /// Tool errors (missing dependencies, version mismatches, etc.)
    Tool,
    /// Internal errors (bugs, assertion failures, etc.)
    Internal,
}

impl ErrorCategory {
    /// Get the emoji representation for the error category
    pub fn emoji(&self) -> &'static str {
        match self {
            ErrorCategory::Build => "🔨",
            ErrorCategory::Configuration => "⚙️",
            ErrorCategory::Io => "📁",
            ErrorCategory::Network => "🌐",
            ErrorCategory::Validation => "✅",
            ErrorCategory::Tool => "🔧",
            ErrorCategory::Internal => "🐛",
        }
    }

    /// Get the color for the error category
    pub fn color(&self) -> colored::Color {
        match self {
            ErrorCategory::Build => colored::Color::Red,
            ErrorCategory::Configuration => colored::Color::Yellow,
            ErrorCategory::Io => colored::Color::Magenta,
            ErrorCategory::Network => colored::Color::Blue,
            ErrorCategory::Validation => colored::Color::Cyan,
            ErrorCategory::Tool => colored::Color::Green,
            ErrorCategory::Internal => colored::Color::BrightRed,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            ErrorCategory::Build => "Build Error",
            ErrorCategory::Configuration => "Configuration Error",
            ErrorCategory::Io => "I/O Error",
            ErrorCategory::Network => "Network Error",
            ErrorCategory::Validation => "Validation Error",
            ErrorCategory::Tool => "Tool Error",
            ErrorCategory::Internal => "Internal Error",
        }
    }
}

/// Categorized error with context
#[derive(Debug)]
pub struct CategorizedError {
    pub category: ErrorCategory,
    pub message: String,
    pub context: Vec<String>,
    pub suggestions: Vec<String>,
}

impl CategorizedError {
    /// Create a new categorized error
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            context: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Add context to the error
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context.push(context.into());
        self
    }

    /// Add a suggestion for fixing the error
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Format the error for human-readable output
    #[must_use]
    pub fn format_human(&self, use_colors: bool) -> String {
        let mut output = String::new();

        // Header with emoji and category
        if use_colors {
            output.push_str(&format!(
                "{} {}\n",
                self.category.emoji(),
                self.category.name().color(self.category.color()).bold()
            ));
        } else {
            output.push_str(&format!(
                "{} {}\n",
                self.category.emoji(),
                self.category.name()
            ));
        }

        // Main message
        if use_colors {
            output.push_str(&format!("  {}\n", self.message.bright_white()));
        } else {
            output.push_str(&format!("  {}\n", self.message));
        }

        // Context information
        if !self.context.is_empty() {
            output.push_str("\n");
            if use_colors {
                output.push_str(&format!("  {}\n", "Context:".bright_blue().bold()));
            } else {
                output.push_str("  Context:\n");
            }
            for ctx in &self.context {
                output.push_str(&format!("    • {}\n", ctx));
            }
        }

        // Suggestions
        if !self.suggestions.is_empty() {
            output.push_str("\n");
            if use_colors {
                output.push_str(&format!("  {}\n", "Suggestions:".bright_green().bold()));
            } else {
                output.push_str("  Suggestions:\n");
            }
            for suggestion in &self.suggestions {
                if use_colors {
                    output.push_str(&format!("    💡 {}\n", suggestion.green()));
                } else {
                    output.push_str(&format!("    💡 {}\n", suggestion));
                }
            }
        }

        output
    }

    /// Format the error for JSON output
    #[must_use]
    pub fn format_json(&self) -> serde_json::Value {
        serde_json::json!({
            "category": format!("{:?}", self.category),
            "category_name": self.category.name(),
            "message": self.message,
            "context": self.context,
            "suggestions": self.suggestions,
            "emoji": self.category.emoji()
        })
    }
}

impl std::fmt::Display for CategorizedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.category.name(), self.message)
    }
}

impl std::error::Error for CategorizedError {}

/// Extension trait for Result types to provide categorized error handling
pub trait ErrorContext<T> {
    /// Add categorized context to an error
    fn with_category(self, category: ErrorCategory, message: impl Into<String>) -> Result<T>;

    /// Add a build error context
    fn build_context(self, message: impl Into<String>) -> Result<T>;

    /// Add a configuration error context
    fn config_context(self, message: impl Into<String>) -> Result<T>;

    /// Add an I/O error context
    fn io_context(self, message: impl Into<String>) -> Result<T>;

    /// Add a validation error context
    fn validation_context(self, message: impl Into<String>) -> Result<T>;

    /// Add a tool error context
    fn tool_context(self, message: impl Into<String>) -> Result<T>;
}

impl<T> ErrorContext<T> for Result<T> {
    fn with_category(self, category: ErrorCategory, message: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let categorized = CategorizedError::new(category, message.into());
            anyhow::Error::new(categorized).context(e)
        })
    }

    fn build_context(self, message: impl Into<String>) -> Result<T> {
        self.with_category(ErrorCategory::Build, message)
    }

    fn config_context(self, message: impl Into<String>) -> Result<T> {
        self.with_category(ErrorCategory::Configuration, message)
    }

    fn io_context(self, message: impl Into<String>) -> Result<T> {
        self.with_category(ErrorCategory::Io, message)
    }

    fn validation_context(self, message: impl Into<String>) -> Result<T> {
        self.with_category(ErrorCategory::Validation, message)
    }

    fn tool_context(self, message: impl Into<String>) -> Result<T> {
        self.with_category(ErrorCategory::Tool, message)
    }
}

/// Error handling utilities
pub struct ErrorHandler<'a> {
    output: &'a OutputManager,
}

impl<'a> ErrorHandler<'a> {
    pub fn new(output: &'a OutputManager) -> Self {
        Self { output }
    }

    /// Handle and format error for output
    #[must_use]
    pub fn handle_error(&self, error: &anyhow::Error) -> String {
        // Try to downcast to CategorizedError
        if let Some(categorized) = error.downcast_ref::<CategorizedError>() {
            match self.output.format() {
                OutputFormat::Human => categorized.format_human(self.output.is_colored()),
                OutputFormat::Json | OutputFormat::JsonLines => {
                    serde_json::to_string_pretty(&categorized.format_json())
                        .unwrap_or_else(|_| format!("Error: {}", categorized.message))
                },
            }
        } else {
            // Fallback for non-categorized errors
            match self.output.format() {
                OutputFormat::Human => {
                    if self.output.is_colored() {
                        format!("{} {}", "❌".bright_red(), error.to_string().bright_white())
                    } else {
                        format!("❌ {}", error)
                    }
                },
                OutputFormat::Json | OutputFormat::JsonLines => {
                    serde_json::to_string_pretty(&serde_json::json!({
                        "category": "unknown",
                        "message": error.to_string(),
                        "context": [],
                        "suggestions": []
                    }))
                    .unwrap_or_else(|_| format!("Error: {}", error))
                },
            }
        }
    }
}

/// Create common build errors
pub mod build_errors {
    use super::*;

    pub fn compilation_failed(details: impl Into<String>) -> CategorizedError {
        CategorizedError::new(ErrorCategory::Build, "Compilation failed")
            .with_context(details)
            .with_suggestion("Check the compilation output for specific errors")
            .with_suggestion("Run 'cargo-wrt check' for detailed diagnostics")
    }

    pub fn test_failed(test_name: impl Into<String>) -> CategorizedError {
        CategorizedError::new(
            ErrorCategory::Validation,
            format!("Test failed: {}", test_name.into()),
        )
        .with_suggestion("Check test logs for detailed failure information")
    }

    pub fn dependency_missing(dep_name: impl Into<String>) -> CategorizedError {
        CategorizedError::new(
            ErrorCategory::Configuration,
            format!("Missing dependency: {}", dep_name.into()),
        )
        .with_suggestion("Install the missing dependency")
        .with_suggestion("Run 'cargo-wrt setup --check' to verify all dependencies")
    }
}

/// Create common configuration errors
pub mod config_errors {
    use super::*;

    pub fn invalid_config_file(path: impl Into<String>) -> CategorizedError {
        CategorizedError::new(ErrorCategory::Configuration, "Invalid configuration file")
            .with_context(format!("File: {}", path.into()))
            .with_suggestion("Check the configuration file syntax")
            .with_suggestion("Refer to the documentation for valid configuration options")
    }

    pub fn missing_config_file(path: impl Into<String>) -> CategorizedError {
        CategorizedError::new(ErrorCategory::Configuration, "Configuration file not found")
            .with_context(format!("Expected: {}", path.into()))
            .with_suggestion("Create the configuration file")
            .with_suggestion("Run 'cargo-wrt init' to generate a default configuration")
    }
}
