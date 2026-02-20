//! Output formatters for diagnostic collections
//!
//! This module provides different output formats for diagnostic collections,
//! including human-readable output (default) and structured JSON formats
//! compatible with LSP and CI/CD systems.

use std::{collections::HashMap, fmt};

use colored::{ColoredString, Colorize};
use serde_json;

use crate::diagnostics::{Diagnostic, DiagnosticCollection, Severity};

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable format with colors (default)
    Human,
    /// JSON format for LSP/tooling integration
    Json,
    /// JSON Lines format for streaming/incremental updates
    JsonLines,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonLines => write!(f, "jsonlines"),
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(OutputFormat::Human),
            "json" => Ok(OutputFormat::Json),
            "jsonlines" | "json-lines" => Ok(OutputFormat::JsonLines),
            _ => Err(format!(
                "Invalid output format '{}'. Valid options: human, json, jsonlines",
                s
            )),
        }
    }
}

/// Trait for formatting diagnostic collections
pub trait OutputFormatter {
    /// Format a complete diagnostic collection
    fn format_collection(&self, collection: &DiagnosticCollection) -> String;

    /// Format individual diagnostics (for streaming)
    fn format_diagnostics(&self, diagnostics: &[Diagnostic]) -> String;

    /// Format just the summary
    fn format_summary(&self, collection: &DiagnosticCollection) -> String;
}

/// Human-readable formatter with colors and grouping
pub struct HumanFormatter {
    /// Whether to use colors in output
    use_colors: bool,
    /// Whether to show related information
    show_related: bool,
    /// Group diagnostics by file
    group_by_file: bool,
}

impl HumanFormatter {
    /// Create a new human formatter
    pub fn new() -> Self {
        Self {
            use_colors: true,
            show_related: true,
            group_by_file: true,
        }
    }

    /// Create formatter without colors (for piped output)
    pub fn no_colors() -> Self {
        Self {
            use_colors: false,
            show_related: true,
            group_by_file: true,
        }
    }

    /// Set whether to use colors
    pub fn with_colors(mut self, use_colors: bool) -> Self {
        self.use_colors = use_colors;
        self
    }

    /// Set whether to show related information
    pub fn with_related_info(mut self, show_related: bool) -> Self {
        self.show_related = show_related;
        self
    }

    /// Set whether to group diagnostics by file
    pub fn with_file_grouping(mut self, group_by_file: bool) -> Self {
        self.group_by_file = group_by_file;
        self
    }

    /// Colorize text based on severity
    fn colorize_severity(&self, text: &str, severity: Severity) -> ColoredString {
        if !self.use_colors {
            return text.normal();
        }

        match severity {
            Severity::Error => text.bright_red(),
            Severity::Warning => text.bright_yellow(),
            Severity::Info => text.bright_blue(),
            Severity::Hint => text.bright_cyan(),
        }
    }

    /// Get severity icon
    fn severity_icon(&self, severity: Severity) -> &'static str {
        if !self.use_colors {
            match severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
                Severity::Hint => "hint",
            }
        } else {
            match severity {
                Severity::Error => "❌",
                Severity::Warning => "⚠️",
                Severity::Info => "ℹ️",
                Severity::Hint => "💡",
            }
        }
    }

    /// Format a single diagnostic
    fn format_diagnostic(&self, diagnostic: &Diagnostic) -> String {
        let mut output = String::new();

        // Format: severity: message [code] (source)
        let severity_text = format!("{}", diagnostic.severity);
        let colored_severity = self.colorize_severity(&severity_text, diagnostic.severity);

        output.push_str(&format!(
            "{} {}: {} ",
            self.severity_icon(diagnostic.severity),
            colored_severity,
            diagnostic.message
        ));

        // Add error code if present
        if let Some(code) = &diagnostic.code {
            if self.use_colors {
                output.push_str(&format!("[{}] ", code.bright_white()));
            } else {
                output.push_str(&format!("[{}] ", code));
            }
        }

        // Add source in parentheses
        if self.use_colors {
            output.push_str(&format!("({})", diagnostic.source.dimmed()));
        } else {
            output.push_str(&format!("({})", diagnostic.source));
        }

        output.push('\n');

        // Add location information
        let location = format!(
            "  --> {}:{}:{}",
            diagnostic.file,
            diagnostic.range.start.line + 1, // Convert to 1-indexed
            diagnostic.range.start.character + 1
        );

        if self.use_colors {
            output.push_str(&format!("{}\n", location.dimmed()));
        } else {
            output.push_str(&format!("{}\n", location));
        }

        // Add related information if enabled
        if self.show_related && !diagnostic.related_info.is_empty() {
            for related in &diagnostic.related_info {
                let related_location = format!(
                    "  note: {}: {}:{}:{}",
                    related.message,
                    related.file,
                    related.range.start.line + 1,
                    related.range.start.character + 1
                );

                if self.use_colors {
                    output.push_str(&format!("{}\n", related_location.dimmed()));
                } else {
                    output.push_str(&format!("{}\n", related_location));
                }
            }
        }

        output
    }
}

impl Default for HumanFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormatter for HumanFormatter {
    fn format_collection(&self, collection: &DiagnosticCollection) -> String {
        let mut output = String::new();

        if collection.diagnostics.is_empty() {
            if self.use_colors {
                output.push_str(&format!("{} No issues found\n", "✅".bright_green()));
            } else {
                output.push_str("No issues found\n");
            }
            return output;
        }

        if self.group_by_file {
            // Group diagnostics by file
            let grouped = collection.group_by_file();
            let mut files: Vec<_> = grouped.keys().collect();
            files.sort();

            for file in files {
                let diagnostics = grouped.get(file).unwrap();

                // File header
                if self.use_colors {
                    output.push_str(&format!("\n{}\n", file.bright_white().underline()));
                } else {
                    output.push_str(&format!("\n{}\n", file));
                }

                for diagnostic in diagnostics {
                    output.push_str(&self.format_diagnostic(diagnostic));
                }
            }
        } else {
            // Sequential output
            for diagnostic in &collection.diagnostics {
                output.push_str(&self.format_diagnostic(diagnostic));
            }
        }

        // Add summary
        output.push('\n');
        output.push_str(&self.format_summary(collection));

        output
    }

    fn format_diagnostics(&self, diagnostics: &[Diagnostic]) -> String {
        let mut output = String::new();
        for diagnostic in diagnostics {
            output.push_str(&self.format_diagnostic(diagnostic));
        }
        output
    }

    fn format_summary(&self, collection: &DiagnosticCollection) -> String {
        let summary = &collection.summary;
        let mut output = String::new();

        if summary.total == 0 {
            if self.use_colors {
                output.push_str(&format!("{} No issues found", "✅".bright_green()));
            } else {
                output.push_str("No issues found");
            }
        } else {
            let parts = vec![
                if summary.errors > 0 {
                    Some(if self.use_colors {
                        format!("{} errors", summary.errors.to_string().bright_red())
                    } else {
                        format!("{} errors", summary.errors)
                    })
                } else {
                    None
                },
                if summary.warnings > 0 {
                    Some(if self.use_colors {
                        format!("{} warnings", summary.warnings.to_string().bright_yellow())
                    } else {
                        format!("{} warnings", summary.warnings)
                    })
                } else {
                    None
                },
                if summary.infos > 0 { Some(format!("{} infos", summary.infos)) } else { None },
                if summary.hints > 0 { Some(format!("{} hints", summary.hints)) } else { None },
            ];

            let parts: Vec<String> = parts.into_iter().flatten().collect();
            let summary_text = parts.join(", ");

            let icon = if summary.errors > 0 {
                if self.use_colors { "❌" } else { "error" }
            } else if summary.warnings > 0 {
                if self.use_colors { "⚠️" } else { "warning" }
            } else {
                if self.use_colors { "ℹ️" } else { "info" }
            };

            output.push_str(&format!(
                "{} {} in {:.2}s",
                icon,
                summary_text,
                summary.duration_ms as f64 / 1000.0
            ));
        }

        output
    }
}

/// JSON formatter for LSP and tooling integration
pub struct JsonFormatter {
    /// Whether to use pretty formatting
    pretty: bool,
}

impl JsonFormatter {
    /// Create a new JSON formatter
    pub fn new() -> Self {
        Self { pretty: false }
    }

    /// Create a pretty-printing JSON formatter
    pub fn pretty() -> Self {
        Self { pretty: true }
    }

    /// Set pretty printing
    pub fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormatter for JsonFormatter {
    fn format_collection(&self, collection: &DiagnosticCollection) -> String {
        if self.pretty {
            serde_json::to_string_pretty(collection).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(collection).unwrap_or_else(|_| "{}".to_string())
        }
    }

    fn format_diagnostics(&self, diagnostics: &[Diagnostic]) -> String {
        if self.pretty {
            serde_json::to_string_pretty(diagnostics).unwrap_or_else(|_| "[]".to_string())
        } else {
            serde_json::to_string(diagnostics).unwrap_or_else(|_| "[]".to_string())
        }
    }

    fn format_summary(&self, collection: &DiagnosticCollection) -> String {
        if self.pretty {
            serde_json::to_string_pretty(&collection.summary).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(&collection.summary).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

/// JSON Lines formatter for streaming output
pub struct JsonLinesFormatter;

impl JsonLinesFormatter {
    /// Create a new JSON Lines formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonLinesFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormatter for JsonLinesFormatter {
    fn format_collection(&self, collection: &DiagnosticCollection) -> String {
        let mut output = String::new();

        // Output each diagnostic as a separate JSON line
        for diagnostic in &collection.diagnostics {
            if let Ok(json) = serde_json::to_string(diagnostic) {
                output.push_str(&json);
                output.push('\n');
            }
        }

        // Output summary as final line
        if let Ok(json) = serde_json::to_string(&collection.summary) {
            output.push_str(&json);
            output.push('\n');
        }

        output
    }

    fn format_diagnostics(&self, diagnostics: &[Diagnostic]) -> String {
        let mut output = String::new();
        for diagnostic in diagnostics {
            if let Ok(json) = serde_json::to_string(diagnostic) {
                output.push_str(&json);
                output.push('\n');
            }
        }
        output
    }

    fn format_summary(&self, collection: &DiagnosticCollection) -> String {
        serde_json::to_string(&collection.summary)
            .map(|json| format!("{}\n", json))
            .unwrap_or_else(|_| "{}\n".to_string())
    }
}

/// Factory for creating output formatters
pub struct FormatterFactory;

impl FormatterFactory {
    /// Create a formatter for the given format
    pub fn create(format: OutputFormat) -> Box<dyn OutputFormatter> {
        match format {
            OutputFormat::Human => Box::new(HumanFormatter::new()),
            OutputFormat::Json => Box::new(JsonFormatter::new()),
            OutputFormat::JsonLines => Box::new(JsonLinesFormatter::new()),
        }
    }

    /// Create a formatter with options
    pub fn create_with_options(
        format: OutputFormat,
        pretty: bool,
        use_colors: bool,
    ) -> Box<dyn OutputFormatter> {
        match format {
            OutputFormat::Human => Box::new(HumanFormatter::new().with_colors(use_colors)),
            OutputFormat::Json => Box::new(JsonFormatter::new().with_pretty(pretty)),
            OutputFormat::JsonLines => Box::new(JsonLinesFormatter::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::diagnostics::{Position, Range};

    fn create_test_collection() -> DiagnosticCollection {
        let mut collection =
            DiagnosticCollection::new(PathBuf::from("/workspace"), "test".to_string());

        collection.add_diagnostic(
            Diagnostic::new(
                "src/main.rs".to_string(),
                Range::single_line(10, 5, 15),
                Severity::Error,
                "undefined variable 'x'".to_string(),
                "rustc".to_string(),
            )
            .with_code("E0425".to_string()),
        );

        collection.add_diagnostic(
            Diagnostic::new(
                "src/lib.rs".to_string(),
                Range::single_line(5, 0, 10),
                Severity::Warning,
                "unused import".to_string(),
                "rustc".to_string(),
            )
            .with_code("W0612".to_string()),
        );

        collection.finalize(1500)
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!(
            "human".parse::<OutputFormat>().unwrap(),
            OutputFormat::Human
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "jsonlines".parse::<OutputFormat>().unwrap(),
            OutputFormat::JsonLines
        );
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_human_formatter() {
        let collection = create_test_collection();
        let formatter = HumanFormatter::no_colors();
        let output = formatter.format_collection(&collection);

        assert!(output.contains("undefined variable 'x'"));
        assert!(output.contains("unused import"));
        assert!(output.contains("src/main.rs:11:6")); // 1-indexed
        assert!(output.contains("1 errors, 1 warnings"));
    }

    #[test]
    fn test_json_formatter() {
        let collection = create_test_collection();
        let formatter = JsonFormatter::new();
        let output = formatter.format_collection(&collection);

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
        assert!(parsed["diagnostics"].is_array());
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_json_lines_formatter() {
        let collection = create_test_collection();
        let formatter = JsonLinesFormatter::new();
        let output = formatter.format_collection(&collection);

        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 3); // 2 diagnostics + 1 summary

        // Each line should be valid JSON
        for line in lines {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }

    #[test]
    fn test_formatter_factory() {
        let human_formatter = FormatterFactory::create(OutputFormat::Human);
        let json_formatter = FormatterFactory::create(OutputFormat::Json);
        let jsonlines_formatter = FormatterFactory::create(OutputFormat::JsonLines);

        let collection = create_test_collection();

        // Should not panic and should produce different outputs
        let human_output = human_formatter.format_collection(&collection);
        let json_output = json_formatter.format_collection(&collection);
        let jsonlines_output = jsonlines_formatter.format_collection(&collection);

        assert_ne!(human_output, json_output);
        assert_ne!(json_output, jsonlines_output);
        assert_ne!(human_output, jsonlines_output);
    }
}
