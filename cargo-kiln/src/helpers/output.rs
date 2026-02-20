//! Standardized output formatting utilities
//!
//! Provides consistent output formatting across all cargo-wrt commands,
//! supporting human-readable, JSON, and JSON Lines formats.

use std::fmt::Display;

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use wrt_build_core::{
    diagnostics::DiagnosticCollection,
    formatters::{FormatterFactory, OutputFormat},
};

/// Standard result formatting for any serializable + displayable type
#[must_use]
pub fn output_result<T>(result: &T, format: &OutputFormat) -> Result<()>
where
    T: Serialize + Display,
{
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(result)?);
        },
        OutputFormat::JsonLines => {
            println!("{}", serde_json::to_string(result)?);
        },
        OutputFormat::Human => {
            println!("{}", result);
        },
    }
    Ok(())
}

/// Specialized output for diagnostic collections
#[must_use]
pub fn output_diagnostics(diagnostics: DiagnosticCollection, format: &OutputFormat) -> Result<()> {
    let formatter = FormatterFactory::create_with_options(format.clone(), false, true);
    print!("{}", formatter.format_collection(&diagnostics));
    Ok(())
}

/// Format results with optional success/failure coloring for human output
pub fn format_result<T>(
    result: &T,
    format: &OutputFormat,
    is_success: bool,
    success_message: Option<&str>,
    failure_message: Option<&str>,
) -> Result<()>
where
    T: Serialize + Display,
{
    match format {
        OutputFormat::Json | OutputFormat::JsonLines => output_result(result, format),
        OutputFormat::Human => {
            if is_success {
                if let Some(msg) = success_message {
                    println!("✅ {}", msg);
                }
            } else if let Some(msg) = failure_message {
                println!("❌ {}", msg);
            }
            println!("{}", result);
            Ok(())
        },
    }
}

/// Check if output format supports colors
pub fn supports_colors(format: &OutputFormat) -> bool {
    matches!(format, OutputFormat::Human) && atty::is(atty::Stream::Stdout)
}

/// Create a simple JSON response for operations without complex data
#[derive(Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl SimpleResponse {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            details: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl Display for SimpleResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Centralized output manager for consistent formatting across commands
#[derive(Clone, Debug)]
pub struct OutputManager {
    format: OutputFormat,
    colored: bool,
}

impl OutputManager {
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            colored: atty::is(atty::Stream::Stdout),
        }
    }

    pub fn with_color(mut self, colored: bool) -> Self {
        self.colored = colored;
        self
    }

    /// Output a structured result
    pub fn output_result<T>(&self, result: &T) -> Result<()>
    where
        T: Serialize + Display,
    {
        output_result(result, &self.format)
    }

    /// Output an error message with consistent formatting
    pub fn error(&self, message: &str) {
        if self.is_json_mode() {
            let response = SimpleResponse::failure(message);
            let _ = self.output_result(&response);
        } else {
            let prefix = if self.colored { "❌".bright_red() } else { "❌".normal() };
            eprintln!("{} {}", prefix, message);
        }
    }

    /// Output a success message with consistent formatting
    pub fn success(&self, message: &str) {
        if self.is_json_mode() {
            let response = SimpleResponse::success(message);
            let _ = self.output_result(&response);
        } else {
            let prefix = if self.colored { "✅".bright_green() } else { "✅".normal() };
            println!("{} {}", prefix, message);
        }
    }

    /// Output a warning message with consistent formatting
    pub fn warning(&self, message: &str) {
        if !self.is_json_mode() {
            let prefix = if self.colored { "⚠️".bright_yellow() } else { "⚠️".normal() };
            println!("{} {}", prefix, message);
        }
    }

    /// Output an info message with consistent formatting
    pub fn info(&self, message: &str) {
        if !self.is_json_mode() {
            let prefix = if self.colored { "ℹ️".bright_blue() } else { "ℹ️".normal() };
            println!("{} {}", prefix, message);
        }
    }

    /// Output a progress message with consistent formatting
    pub fn progress(&self, message: &str) {
        if !self.is_json_mode() {
            let prefix = if self.colored { "🔨".bright_blue() } else { "🔨".normal() };
            println!("{} {}", prefix, message);
        }
    }

    /// Output a section header with consistent formatting
    pub fn header(&self, title: &str) {
        if !self.is_json_mode() {
            if self.colored {
                println!("{}", title.bright_cyan().bold());
            } else {
                println!("{}", title);
            }
        }
    }

    /// Output a subheader with consistent formatting
    pub fn subheader(&self, title: &str) {
        if !self.is_json_mode() {
            if self.colored {
                println!("  {}", title.cyan());
            } else {
                println!("  {}", title);
            }
        }
    }

    /// Output plain text (respects JSON mode)
    pub fn text(&self, message: &str) {
        if !self.is_json_mode() {
            println!("{}", message);
        }
    }

    /// Output indented text with consistent formatting
    pub fn indent(&self, message: &str) {
        if !self.is_json_mode() {
            println!("  {}", message);
        }
    }

    /// Check if we're in JSON output mode
    pub fn is_json_mode(&self) -> bool {
        matches!(self.format, OutputFormat::Json | OutputFormat::JsonLines)
    }

    /// Get the current output format
    pub fn format(&self) -> &OutputFormat {
        &self.format
    }

    /// Check if colored output is enabled
    pub fn is_colored(&self) -> bool {
        self.colored
    }

    /// Output a debug message with consistent formatting
    pub fn debug(&self, message: &str) {
        if !self.is_json_mode() {
            let prefix = if self.colored { "🐛".bright_magenta() } else { "🐛".normal() };
            println!("{} {}", prefix, message);
        }
    }
}
