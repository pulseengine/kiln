//! Output parsers for external tools
//!
//! This module provides parsers for converting output from external tools
//! (cargo, clippy, kani, etc.) into unified diagnostic format.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, Position, Range, RelatedInfo, Severity, ToolOutputParser};
// Re-export parser types
// Note: These are defined below in this module
use crate::error::BuildResult;

/// Cargo compiler message format (from cargo --message-format=json)
#[derive(Debug, Deserialize)]
pub struct CargoMessage {
    /// Message type (e.g., "compiler-message", "compiler-artifact")
    pub reason: String,
    /// Package ID
    pub package_id: Option<String>,
    /// Target information
    pub target: Option<Target>,
    /// Compiler message details
    pub message: Option<CompilerMessage>,
}

/// Target information from cargo
#[derive(Debug, Deserialize)]
pub struct Target {
    /// Target name
    pub name: String,
    /// Target kind (e.g., "bin", "lib")
    pub kind: Vec<String>,
    /// Source root path
    pub src_path: String,
}

/// Compiler message from cargo
#[derive(Debug, Deserialize)]
pub struct CompilerMessage {
    /// Message text
    pub message: String,
    /// Message level (error, warning, etc.)
    pub level: String,
    /// Error/warning code
    pub code: Option<ErrorCode>,
    /// Source spans showing where the issue is
    pub spans: Vec<Span>,
    /// Child messages (notes, helps, etc.)
    pub children: Vec<CompilerMessage>,
    /// Rendered message (formatted for display)
    pub rendered: Option<String>,
}

/// Error or warning code
#[derive(Debug, Deserialize)]
pub struct ErrorCode {
    /// Code string (e.g., "E0425")
    pub code: String,
    /// Explanation text
    pub explanation: Option<String>,
}

/// Source span showing location of issue
#[derive(Debug, Deserialize)]
pub struct Span {
    /// File name
    pub file_name: String,
    /// Byte start position
    pub byte_start: u32,
    /// Byte end position  
    pub byte_end: u32,
    /// Line start (1-indexed)
    pub line_start: u32,
    /// Line end (1-indexed)
    pub line_end: u32,
    /// Column start (1-indexed)
    pub column_start: u32,
    /// Column end (1-indexed)
    pub column_end: u32,
    /// Whether this is the primary span
    pub is_primary: bool,
    /// Span text
    pub text: Vec<SpanText>,
    /// Label for this span
    pub label: Option<String>,
    /// Suggested replacement
    pub suggested_replacement: Option<String>,
    /// Suggestion applicability
    pub suggestion_applicability: Option<String>,
    /// Expansion information (for macros)
    pub expansion: Option<Box<Expansion>>,
}

/// Text content of a span
#[derive(Debug, Deserialize)]
pub struct SpanText {
    /// Text content
    pub text: String,
    /// Highlight start (1-indexed)
    pub highlight_start: u32,
    /// Highlight end (1-indexed)
    pub highlight_end: u32,
}

/// Macro expansion information
#[derive(Debug, Deserialize)]
pub struct Expansion {
    /// Span of the expansion
    pub span: Span,
    /// Macro name
    pub macro_decl_name: String,
    /// Definition site
    pub def_site_span: Option<Span>,
}

/// Parser for cargo build output
pub struct CargoOutputParser {
    workspace_root: String,
}

impl CargoOutputParser {
    /// Create a new cargo output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Convert cargo message level to diagnostic severity
    fn level_to_severity(level: &str) -> Severity {
        match level {
            "error" | "failure-note" => Severity::Error,
            "warning" => Severity::Warning,
            "note" | "help" => Severity::Info,
            _ => Severity::Info,
        }
    }

    /// Convert span to range
    fn span_to_range(span: &Span) -> Range {
        // Cargo uses 1-indexed positions, LSP uses 0-indexed
        Range::new(
            Position::from_line_col_1_indexed(span.line_start, span.column_start),
            Position::from_line_col_1_indexed(span.line_end, span.column_end),
        )
    }

    /// Make file path relative to workspace root
    fn make_relative_path(&self, absolute_path: &str) -> String {
        if let Ok(path) = std::path::Path::new(absolute_path).strip_prefix(&self.workspace_root) {
            path.to_string_lossy().to_string()
        } else {
            absolute_path.to_string()
        }
    }

    /// Convert compiler message to diagnostic
    fn message_to_diagnostic(&self, message: &CompilerMessage, primary_span: &Span) -> Diagnostic {
        let file = self.make_relative_path(&primary_span.file_name);
        let range = Self::span_to_range(primary_span);
        let severity = Self::level_to_severity(&message.level);

        let mut diagnostic = Diagnostic::new(
            file,
            range,
            severity,
            message.message.clone(),
            "rustc".to_string(),
        );

        // Add error code if present
        if let Some(code) = &message.code {
            diagnostic = diagnostic.with_code(code.code.clone());
        }

        // Add related information from child messages and secondary spans
        let mut related_info = Vec::new();

        // Add secondary spans as related info
        for span in &message.spans {
            if !span.is_primary && !span.file_name.is_empty() {
                let related_file = self.make_relative_path(&span.file_name);
                let related_range = Self::span_to_range(span);
                let related_message = span.label.as_deref().unwrap_or("related").to_string();

                related_info.push(RelatedInfo::new(
                    related_file,
                    related_range,
                    related_message,
                ));
            }
        }

        // Add child messages as related info
        for child in &message.children {
            if let Some(child_span) = child.spans.first() {
                let related_file = self.make_relative_path(&child_span.file_name);
                let related_range = Self::span_to_range(child_span);

                related_info.push(RelatedInfo::new(
                    related_file,
                    related_range,
                    child.message.clone(),
                ));
            }
        }

        diagnostic.with_related_infos(related_info)
    }
}

impl ToolOutputParser for CargoOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Parse JSON messages from stdout
        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as cargo JSON message
            if let Ok(cargo_msg) = serde_json::from_str::<CargoMessage>(line) {
                match cargo_msg.reason.as_str() {
                    "compiler-message" => {
                        if let Some(message) = cargo_msg.message {
                            // Handle messages with primary span
                            if let Some(primary_span) = message.spans.iter().find(|s| s.is_primary)
                            {
                                let diagnostic = self.message_to_diagnostic(&message, primary_span);
                                diagnostics.push(diagnostic);
                            } else if !message.spans.is_empty() {
                                // No primary span, use first span
                                let diagnostic =
                                    self.message_to_diagnostic(&message, &message.spans[0]);
                                diagnostics.push(diagnostic);
                            } else {
                                // No spans at all, create a generic diagnostic
                                diagnostics.push(
                                    Diagnostic::new(
                                        "<unknown>".to_string(),
                                        Range::entire_line(0),
                                        Self::level_to_severity(&message.level),
                                        message.message.clone(),
                                        "cargo".to_string(),
                                    )
                                    .with_code(message.code.map(|c| c.code).unwrap_or_default()),
                                );
                            }
                        }
                    },
                    "build-finished" => {
                        // Build finished message, check if it failed
                        // Try to parse the message field directly as JSON
                        if let Ok(msg_value) = serde_json::from_str::<serde_json::Value>(line) {
                            if let Ok(finished) = serde_json::from_value::<BuildFinished>(msg_value)
                            {
                                if !finished.success {
                                    diagnostics.push(Diagnostic::new(
                                        "<build>".to_string(),
                                        Range::entire_line(0),
                                        Severity::Error,
                                        "Build failed".to_string(),
                                        "cargo".to_string(),
                                    ));
                                }
                            }
                        }
                    },
                    _ => {}, // Ignore other message types for now
                }
            }
        }

        // Also parse stderr for non-JSON error messages
        if !stderr.is_empty() && diagnostics.is_empty() {
            // Fallback to generic parsing if no JSON messages found
            let generic_parser =
                GenericOutputParser::new("cargo".to_string(), &self.workspace_root);
            let stderr_diagnostics = generic_parser.parse_error_patterns(stderr);
            diagnostics.extend(stderr_diagnostics);
        }

        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "cargo"
    }
}

/// Build finished message from cargo
#[derive(Debug, Deserialize)]
struct BuildFinished {
    /// Whether the build succeeded
    success: bool,
}

/// Generic parser for tools that don't have structured output
pub struct GenericOutputParser {
    tool_name: String,
    workspace_root: String,
}

impl GenericOutputParser {
    /// Create a new generic parser
    pub fn new<P: AsRef<Path>>(tool_name: String, workspace_root: P) -> Self {
        Self {
            tool_name,
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse generic error patterns from stderr
    fn parse_error_patterns(&self, stderr: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for line in stderr.lines() {
            // Look for common error patterns
            if line.contains("error:") || line.contains("ERROR:") {
                // Try to extract file and line information
                // Pattern: "file.rs:line:col: error: message"
                if let Some(diagnostic) = self.parse_error_line(line, Severity::Error) {
                    diagnostics.push(diagnostic);
                }
            } else if line.contains("warning:") || line.contains("WARNING:") {
                if let Some(diagnostic) = self.parse_error_line(line, Severity::Warning) {
                    diagnostics.push(diagnostic);
                }
            }
        }

        diagnostics
    }

    /// Parse a single error line
    fn parse_error_line(&self, line: &str, severity: Severity) -> Option<Diagnostic> {
        // Simple regex-like parsing for common patterns
        // This is a fallback for tools without structured output

        // Pattern 1: "file:line:col: level: message"
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 4 {
            if let (Ok(line_num), Ok(col_num)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>())
            {
                let file = parts[0].to_string();
                let message = parts[3..].join(":").trim().to_string();

                return Some(Diagnostic::new(
                    file,
                    Range::from_line_1_indexed(line_num, col_num, col_num + 1),
                    severity,
                    message,
                    self.tool_name.clone(),
                ));
            }
        }

        // Pattern 2: Just create a generic diagnostic for the whole file
        if line.contains(&self.tool_name) {
            return Some(Diagnostic::new(
                "<unknown>".to_string(),
                Range::entire_line(0),
                severity,
                line.to_string(),
                self.tool_name.clone(),
            ));
        }

        None
    }
}

impl ToolOutputParser for GenericOutputParser {
    fn parse_output(
        &self,
        _stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        Ok(self.parse_error_patterns(stderr))
    }

    fn tool_name(&self) -> &'static str {
        "generic"
    }

    fn source_name(&self) -> &'static str {
        // For generic parser, we can't return &self.tool_name due to lifetime issues
        // In practice, this would be fixed by using a different approach
        "tool"
    }
}

/// Parser for Miri undefined behavior detection output
pub struct MiriOutputParser {
    workspace_root: String,
}

impl MiriOutputParser {
    /// Create a new Miri output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse Miri-specific output patterns
    fn parse_miri_output(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut current_error: Option<(String, String, Option<String>)> = None;
        let mut in_backtrace = false;
        let mut backtrace_items = Vec::new();

        for line in output.lines() {
            // Detect start of Miri error
            if line.starts_with("error: ") {
                // Save previous error if any
                if let Some((msg, file, code)) = current_error.take() {
                    diagnostics.push(self.create_miri_diagnostic(
                        msg,
                        file,
                        code,
                        &backtrace_items,
                    ));
                    backtrace_items.clear();
                }

                let error_msg = line.strip_prefix("error: ").unwrap_or(line).to_string();
                current_error = Some((error_msg, String::new(), None));
                in_backtrace = false;
            }
            // Parse error code like "error[E0080]:"
            else if line.contains("error[") && line.contains("]:") {
                if let Some(code_match) = line.split('[').nth(1).and_then(|s| s.split(']').next()) {
                    if let Some((_, _, ref mut code)) = current_error {
                        *code = Some(code_match.to_string());
                    }
                }
            }
            // Parse file location " --> src/main.rs:10:5"
            else if line.trim_start().starts_with("--> ") {
                if let Some(location) = line.trim_start().strip_prefix("--> ") {
                    if let Some((_, ref mut file, _)) = current_error {
                        *file = location.to_string();
                    }
                }
            }
            // Detect undefined behavior
            else if line.contains("undefined behavior:") || line.contains("Undefined Behavior:") {
                let ub_msg = line.trim().to_string();
                if let Some((ref mut msg, _, _)) = current_error {
                    msg.push_str(&format!(" - {}", ub_msg));
                }
            }
            // Parse backtrace
            else if line.trim() == "backtrace:" {
                in_backtrace = true;
            } else if in_backtrace && line.trim_start().starts_with("at ") {
                if let Some(location) = line.trim_start().strip_prefix("at ") {
                    backtrace_items.push(location.to_string());
                }
            }
            // Memory access errors
            else if line.contains("memory access") || line.contains("accessing memory") {
                if let Some((ref mut msg, _, _)) = current_error {
                    msg.push_str(&format!(" - {}", line.trim()));
                }
            }
        }

        // Don't forget the last error
        if let Some((msg, file, code)) = current_error {
            diagnostics.push(self.create_miri_diagnostic(msg, file, code, &backtrace_items));
        }

        // If no specific errors found but miri failed, create a generic error
        if diagnostics.is_empty() && output.contains("error:") {
            diagnostics.push(Diagnostic::new(
                "<miri>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                "Miri detected undefined behavior".to_string(),
                "miri".to_string(),
            ));
        }

        diagnostics
    }

    /// Create a diagnostic from parsed Miri error information
    fn create_miri_diagnostic(
        &self,
        message: String,
        file_location: String,
        code: Option<String>,
        backtrace: &[String],
    ) -> Diagnostic {
        // Parse file location "src/main.rs:10:5"
        let (file, range) = if !file_location.is_empty() {
            let parts: Vec<&str> = file_location.split(':').collect();
            if parts.len() >= 2 {
                let file = self.make_relative_path(parts[0]);
                let line = parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                let col = parts.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                (file, Range::from_line_1_indexed(line, col, col + 1))
            } else {
                (file_location, Range::entire_line(0))
            }
        } else {
            ("<miri>".to_string(), Range::entire_line(0))
        };

        let mut diagnostic =
            Diagnostic::new(file, range, Severity::Error, message, "miri".to_string());

        if let Some(code) = code {
            diagnostic = diagnostic.with_code(code);
        }

        // Add backtrace as related information
        let mut related_info = Vec::new();
        for (i, location) in backtrace.iter().enumerate() {
            let parts: Vec<&str> = location.split(':').collect();
            if parts.len() >= 2 {
                let bt_file = self.make_relative_path(parts[0]);
                let bt_line = parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                let bt_col = parts.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);

                related_info.push(RelatedInfo::new(
                    bt_file,
                    Range::from_line_1_indexed(bt_line, bt_col, bt_col + 1),
                    format!("backtrace frame {}", i + 1),
                ));
            }
        }

        if !related_info.is_empty() {
            diagnostic = diagnostic.with_related_infos(related_info);
        }

        diagnostic
    }

    /// Make file path relative to workspace root
    fn make_relative_path(&self, absolute_path: &str) -> String {
        if let Ok(path) = std::path::Path::new(absolute_path).strip_prefix(&self.workspace_root) {
            path.to_string_lossy().to_string()
        } else {
            absolute_path.to_string()
        }
    }
}

impl ToolOutputParser for MiriOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Miri outputs to stderr
        diagnostics.extend(self.parse_miri_output(stderr));

        // Also check stdout for any additional information
        diagnostics.extend(self.parse_miri_output(stdout));

        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "miri"
    }
}

/// Parser for Kani verification output
pub struct KaniOutputParser {
    workspace_root: String,
}

impl KaniOutputParser {
    /// Create a new Kani output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse Kani-specific output patterns
    fn parse_kani_output(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for line in output.lines() {
            // Look for Kani verification failures
            if line.contains("VERIFICATION:- FAILED") || line.contains("assertion failed") {
                // Create a diagnostic for the verification failure
                diagnostics.push(Diagnostic::new(
                    "<kani>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    line.to_string(),
                    "kani".to_string(),
                ));
            } else if line.contains("VERIFICATION:- SUCCESSFUL") {
                diagnostics.push(Diagnostic::new(
                    "<kani>".to_string(),
                    Range::entire_line(0),
                    Severity::Info,
                    "Verification successful".to_string(),
                    "kani".to_string(),
                ));
            }
        }

        diagnostics
    }
}

impl ToolOutputParser for KaniOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = self.parse_kani_output(stdout);
        diagnostics.extend(self.parse_kani_output(stderr));
        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "kani"
    }
}

/// Parser for cargo-audit security vulnerability reports
pub struct CargoAuditOutputParser {
    workspace_root: String,
}

impl CargoAuditOutputParser {
    /// Create a new cargo-audit output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse cargo-audit JSON output
    fn parse_audit_json(&self, json_str: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Try to parse as JSON
        if let Ok(audit_report) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Parse vulnerabilities array
            if let Some(vulnerabilities) = audit_report["vulnerabilities"]["list"].as_array() {
                for vuln in vulnerabilities {
                    let advisory = &vuln["advisory"];
                    let package = &vuln["package"];

                    let id = advisory["id"].as_str().unwrap_or("UNKNOWN");
                    let title = advisory["title"].as_str().unwrap_or("Security vulnerability");
                    let description = advisory["description"].as_str().unwrap_or("");
                    let severity = advisory["severity"].as_str().unwrap_or("unknown");
                    let package_name = package["name"].as_str().unwrap_or("unknown");
                    let package_version = package["version"].as_str().unwrap_or("unknown");

                    let message = format!(
                        "{}: {} in {} v{}\n{}",
                        id, title, package_name, package_version, description
                    );

                    let severity_level = match severity.to_lowercase().as_str() {
                        "critical" | "high" => Severity::Error,
                        "medium" => Severity::Warning,
                        _ => Severity::Info,
                    };

                    diagnostics.push(
                        Diagnostic::new(
                            "Cargo.lock".to_string(),
                            Range::entire_line(0),
                            severity_level,
                            message,
                            "cargo-audit".to_string(),
                        )
                        .with_code(id.to_string()),
                    );
                }
            }

            // Parse warnings if any
            if let Some(warnings) = audit_report["warnings"].as_array() {
                for warning in warnings {
                    if let Some(msg) = warning["message"].as_str() {
                        diagnostics.push(Diagnostic::new(
                            "<audit>".to_string(),
                            Range::entire_line(0),
                            Severity::Warning,
                            msg.to_string(),
                            "cargo-audit".to_string(),
                        ));
                    }
                }
            }
        }

        diagnostics
    }

    /// Parse cargo-audit text output (fallback)
    fn parse_audit_text(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut current_vuln: Option<(String, String, Severity)> = None;
        let mut details = String::new();

        for line in output.lines() {
            // Parse vulnerability header like "RUSTSEC-2021-0139: ansi_term is
            // Unmaintained"
            if line.starts_with("RUSTSEC-") || line.starts_with("CVE-") {
                // Save previous vulnerability if any
                if let Some((id, title, severity)) = current_vuln.take() {
                    diagnostics.push(
                        Diagnostic::new(
                            "Cargo.lock".to_string(),
                            Range::entire_line(0),
                            severity,
                            format!("{}: {}\n{}", id, title, details.trim()),
                            "cargo-audit".to_string(),
                        )
                        .with_code(id),
                    );
                    details.clear();
                }

                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let id = parts[0].trim().to_string();
                    let title = parts[1].trim().to_string();
                    current_vuln = Some((id, title, Severity::Warning));
                }
            }
            // Parse severity
            else if line.trim().starts_with("Severity:") {
                if let Some((_, _, ref mut sev)) = current_vuln {
                    let severity_str = line.trim().strip_prefix("Severity:").unwrap_or("").trim();
                    *sev = match severity_str.to_lowercase().as_str() {
                        "critical" | "high" => Severity::Error,
                        "medium" => Severity::Warning,
                        _ => Severity::Info,
                    };
                }
            }
            // Collect details
            else if current_vuln.is_some() && !line.trim().is_empty() {
                details.push_str(line);
                details.push('\n');
            }
        }

        // Don't forget the last vulnerability
        if let Some((id, title, severity)) = current_vuln {
            diagnostics.push(
                Diagnostic::new(
                    "Cargo.lock".to_string(),
                    Range::entire_line(0),
                    severity,
                    format!("{}: {}\n{}", id, title, details.trim()),
                    "cargo-audit".to_string(),
                )
                .with_code(id),
            );
        }

        // Check for summary line
        if output.contains("vulnerabilities found") {
            let vuln_count = output
                .lines()
                .find(|line| line.contains("vulnerabilities found"))
                .and_then(|line| line.split_whitespace().next())
                .and_then(|num| num.parse::<usize>().ok())
                .unwrap_or(0);

            if vuln_count > 0 {
                diagnostics.push(Diagnostic::new(
                    "<audit>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!(
                        "{} security vulnerabilities found in dependencies",
                        vuln_count
                    ),
                    "cargo-audit".to_string(),
                ));
            }
        }

        diagnostics
    }
}

impl ToolOutputParser for CargoAuditOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Try JSON parsing first (if --format json was used)
        if stdout.trim().starts_with('{') {
            diagnostics.extend(self.parse_audit_json(stdout));
        } else {
            // Fall back to text parsing
            diagnostics.extend(self.parse_audit_text(stdout));
        }

        // Check stderr for errors
        if !stderr.is_empty() && diagnostics.is_empty() {
            if stderr.contains("not found") || stderr.contains("not installed") {
                diagnostics.push(Diagnostic::new(
                    "<audit>".to_string(),
                    Range::entire_line(0),
                    Severity::Info,
                    "cargo-audit not available. Install with: cargo install cargo-audit"
                        .to_string(),
                    "cargo-audit".to_string(),
                ));
            } else {
                diagnostics.push(Diagnostic::new(
                    "<audit>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("cargo-audit error: {}", stderr.trim()),
                    "cargo-audit".to_string(),
                ));
            }
        }

        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "cargo-audit"
    }
}

/// Parser for rustdoc documentation generation errors
pub struct RustdocOutputParser {
    workspace_root: String,
}

impl RustdocOutputParser {
    /// Create a new rustdoc output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse rustdoc output
    fn parse_rustdoc_output(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut current_file = String::new();
        let mut in_error = false;

        for line in output.lines() {
            // Parse error locations like "error: unresolved link to `NonExistent`"
            if line.trim_start().starts_with("error:") || line.trim_start().starts_with("warning:")
            {
                in_error = true;
                let is_error = line.contains("error:");
                let message = line
                    .trim_start()
                    .strip_prefix("error:")
                    .or_else(|| line.trim_start().strip_prefix("warning:"))
                    .unwrap_or(line)
                    .trim()
                    .to_string();

                let severity = if is_error { Severity::Error } else { Severity::Warning };

                // Create diagnostic without file info (will be updated if found)
                diagnostics.push(Diagnostic::new(
                    if current_file.is_empty() {
                        "<rustdoc>".to_string()
                    } else {
                        current_file.clone()
                    },
                    Range::entire_line(0),
                    severity,
                    message,
                    "rustdoc".to_string(),
                ));
            }
            // Parse file location " --> src/lib.rs:10:5"
            else if line.trim_start().starts_with("--> ") && in_error {
                if let Some(location) = line.trim_start().strip_prefix("--> ") {
                    let parts: Vec<&str> = location.split(':').collect();
                    if parts.len() >= 2 {
                        let file = self.make_relative_path(parts[0]);
                        let line_num =
                            parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                        let col = parts.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);

                        // Update the last diagnostic with file info
                        if let Some(last_diag) = diagnostics.last_mut() {
                            *last_diag = Diagnostic::new(
                                file.clone(),
                                Range::from_line_1_indexed(line_num, col, col + 1),
                                last_diag.severity.clone(),
                                last_diag.message.clone(),
                                "rustdoc".to_string(),
                            );
                            current_file = file;
                        }
                    }
                }
                in_error = false;
            }
            // Parse documentation test failures
            else if line.contains("FAILED") && line.contains("doc-tests") {
                diagnostics.push(Diagnostic::new(
                    "<doc-tests>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    line.trim().to_string(),
                    "rustdoc".to_string(),
                ));
            }
            // Parse specific doc comment issues
            else if line.contains("missing code example") {
                in_error = false;
                diagnostics.push(
                    Diagnostic::new(
                        if current_file.is_empty() {
                            "<rustdoc>".to_string()
                        } else {
                            current_file.clone()
                        },
                        Range::entire_line(0),
                        Severity::Warning,
                        "Missing code example in documentation".to_string(),
                        "rustdoc".to_string(),
                    )
                    .with_code("DOC001".to_string()),
                );
            }
            // Parse broken links
            else if line.contains("unresolved link") || line.contains("broken link") {
                let link_match = line.split('`').nth(1).unwrap_or("unknown");

                diagnostics.push(
                    Diagnostic::new(
                        if current_file.is_empty() {
                            "<rustdoc>".to_string()
                        } else {
                            current_file.clone()
                        },
                        Range::entire_line(0),
                        Severity::Warning,
                        format!("Broken documentation link: `{}`", link_match),
                        "rustdoc".to_string(),
                    )
                    .with_code("DOC002".to_string()),
                );
            }
        }

        // Check for common rustdoc issues in stderr
        if output.contains("could not document") {
            diagnostics.push(Diagnostic::new(
                "<rustdoc>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                "Documentation generation failed".to_string(),
                "rustdoc".to_string(),
            ));
        }

        diagnostics
    }

    /// Make file path relative to workspace root
    fn make_relative_path(&self, absolute_path: &str) -> String {
        if let Ok(path) = std::path::Path::new(absolute_path).strip_prefix(&self.workspace_root) {
            path.to_string_lossy().to_string()
        } else {
            absolute_path.to_string()
        }
    }
}

impl ToolOutputParser for RustdocOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Parse both stdout and stderr as rustdoc outputs to both
        diagnostics.extend(self.parse_rustdoc_output(stdout));
        diagnostics.extend(self.parse_rustdoc_output(stderr));

        // If no specific diagnostics but rustdoc failed, add generic error
        if diagnostics.is_empty() && stderr.contains("aborting due to") {
            diagnostics.push(Diagnostic::new(
                "<rustdoc>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                "Documentation generation failed with errors".to_string(),
                "rustdoc".to_string(),
            ));
        }

        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "rustdoc"
    }
}

/// Parser for cargo-tarpaulin coverage analysis output
pub struct TarpaulinOutputParser {
    workspace_root: String,
}

impl TarpaulinOutputParser {
    /// Create a new tarpaulin output parser
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Parse tarpaulin JSON output
    fn parse_tarpaulin_json(&self, json_str: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if let Ok(coverage_report) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Parse overall coverage
            if let Some(coverage_pct) = coverage_report["coverage_percent"].as_f64() {
                let severity = if coverage_pct < 50.0 {
                    Severity::Error
                } else if coverage_pct < 80.0 {
                    Severity::Warning
                } else {
                    Severity::Info
                };

                diagnostics.push(
                    Diagnostic::new(
                        "<coverage>".to_string(),
                        Range::entire_line(0),
                        severity,
                        format!("Overall test coverage: {:.2}%", coverage_pct),
                        "tarpaulin".to_string(),
                    )
                    .with_code("COV001".to_string()),
                );
            }

            // Parse per-file coverage
            if let Some(files) = coverage_report["files"].as_object() {
                for (file_path, file_data) in files {
                    let relative_path = self.make_relative_path(file_path);

                    if let Some(file_coverage) = file_data["coverage_percent"].as_f64() {
                        if file_coverage < 80.0 {
                            let severity = if file_coverage < 50.0 {
                                Severity::Warning
                            } else {
                                Severity::Info
                            };

                            diagnostics.push(
                                Diagnostic::new(
                                    relative_path.clone(),
                                    Range::entire_line(0),
                                    severity,
                                    format!("File coverage: {:.2}%", file_coverage),
                                    "tarpaulin".to_string(),
                                )
                                .with_code("COV002".to_string()),
                            );
                        }
                    }

                    // Parse uncovered lines
                    if let Some(uncovered_lines) = file_data["uncovered_lines"].as_array() {
                        for line_value in uncovered_lines {
                            if let Some(line_num) = line_value.as_u64() {
                                diagnostics.push(
                                    Diagnostic::new(
                                        relative_path.clone(),
                                        Range::from_line_1_indexed(line_num as u32, 1, 1),
                                        Severity::Info,
                                        "Line not covered by tests".to_string(),
                                        "tarpaulin".to_string(),
                                    )
                                    .with_code("COV003".to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }

        diagnostics
    }

    /// Parse tarpaulin text output
    fn parse_tarpaulin_text(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut current_file = String::new();

        for line in output.lines() {
            // Parse overall coverage line like "Coverage Results: 85.50%"
            if line.contains("Coverage Results:") {
                if let Some(pct_str) = line.split(':').nth(1) {
                    let coverage_pct =
                        pct_str.trim().trim_end_matches('%').parse::<f64>().unwrap_or(0.0);

                    let severity = if coverage_pct < 50.0 {
                        Severity::Error
                    } else if coverage_pct < 80.0 {
                        Severity::Warning
                    } else {
                        Severity::Info
                    };

                    diagnostics.push(
                        Diagnostic::new(
                            "<coverage>".to_string(),
                            Range::entire_line(0),
                            severity,
                            format!("Overall test coverage: {:.2}%", coverage_pct),
                            "tarpaulin".to_string(),
                        )
                        .with_code("COV001".to_string()),
                    );
                }
            }
            // Parse file coverage like "src/main.rs: 75.00%"
            else if line.contains(".rs:") && line.contains('%') {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 {
                    current_file = self.make_relative_path(parts[0].trim());

                    if let Some(pct_str) = parts[1].trim().strip_suffix('%') {
                        let file_coverage = pct_str.parse::<f64>().unwrap_or(0.0);

                        if file_coverage < 80.0 {
                            let severity = if file_coverage < 50.0 {
                                Severity::Warning
                            } else {
                                Severity::Info
                            };

                            diagnostics.push(
                                Diagnostic::new(
                                    current_file.clone(),
                                    Range::entire_line(0),
                                    severity,
                                    format!("File coverage: {:.2}%", file_coverage),
                                    "tarpaulin".to_string(),
                                )
                                .with_code("COV002".to_string()),
                            );
                        }
                    }
                }
            }
            // Parse uncovered lines info
            else if line.contains("Uncovered Lines:") && !current_file.is_empty() {
                if let Some(lines_str) = line.split(':').nth(1) {
                    // Parse line ranges like "10-15, 20, 25-30"
                    for range_str in lines_str.split(',') {
                        let range_str = range_str.trim();
                        if range_str.contains('-') {
                            // Line range
                            let parts: Vec<&str> = range_str.split('-').collect();
                            if parts.len() == 2 {
                                if let (Ok(start), Ok(end)) =
                                    (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                                {
                                    for line_num in start..=end {
                                        diagnostics.push(
                                            Diagnostic::new(
                                                current_file.clone(),
                                                Range::from_line_1_indexed(line_num, 1, 1),
                                                Severity::Info,
                                                "Line not covered by tests".to_string(),
                                                "tarpaulin".to_string(),
                                            )
                                            .with_code("COV003".to_string()),
                                        );
                                    }
                                }
                            }
                        } else {
                            // Single line
                            if let Ok(line_num) = range_str.parse::<u32>() {
                                diagnostics.push(
                                    Diagnostic::new(
                                        current_file.clone(),
                                        Range::from_line_1_indexed(line_num, 1, 1),
                                        Severity::Info,
                                        "Line not covered by tests".to_string(),
                                        "tarpaulin".to_string(),
                                    )
                                    .with_code("COV003".to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }

        diagnostics
    }

    /// Make file path relative to workspace root
    fn make_relative_path(&self, absolute_path: &str) -> String {
        if let Ok(path) = std::path::Path::new(absolute_path).strip_prefix(&self.workspace_root) {
            path.to_string_lossy().to_string()
        } else {
            absolute_path.to_string()
        }
    }
}

impl ToolOutputParser for TarpaulinOutputParser {
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        _working_dir: &Path,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Try JSON parsing first
        if stdout.trim().starts_with('{') {
            diagnostics.extend(self.parse_tarpaulin_json(stdout));
        } else {
            // Fall back to text parsing
            diagnostics.extend(self.parse_tarpaulin_text(stdout));
        }

        // Check for errors
        if !stderr.is_empty() && diagnostics.is_empty() {
            if stderr.contains("not found") || stderr.contains("not installed") {
                diagnostics.push(Diagnostic::new(
                    "<coverage>".to_string(),
                    Range::entire_line(0),
                    Severity::Info,
                    "cargo-tarpaulin not available. Install with: cargo install cargo-tarpaulin"
                        .to_string(),
                    "tarpaulin".to_string(),
                ));
            } else if stderr.contains("error:") {
                diagnostics.push(Diagnostic::new(
                    "<coverage>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    "Coverage analysis failed".to_string(),
                    "tarpaulin".to_string(),
                ));
            }
        }

        Ok(diagnostics)
    }

    fn tool_name(&self) -> &'static str {
        "tarpaulin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_message_parsing() {
        let json_message = r#"{"reason":"compiler-message","package_id":"test-package","message":{"message":"cannot find value `x` in this scope","code":{"code":"E0425","explanation":null},"level":"error","spans":[{"file_name":"/workspace/src/main.rs","byte_start":100,"byte_end":101,"line_start":10,"line_end":10,"column_start":5,"column_end":6,"is_primary":true,"text":[],"label":"not found in this scope","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}}"#;

        let parser = CargoOutputParser::new("/workspace");
        let diagnostics = parser.parse_output(json_message, "", Path::new("/workspace")).unwrap();

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];

        assert_eq!(diagnostic.file, "src/main.rs");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code, Some("E0425".to_string()));
        assert_eq!(diagnostic.message, "cannot find value `x` in this scope");
        assert_eq!(diagnostic.source, "rustc");

        // Check position conversion (1-indexed to 0-indexed)
        assert_eq!(diagnostic.range.start.line, 9);
        assert_eq!(diagnostic.range.start.character, 4);
    }

    #[test]
    fn test_generic_parser() {
        let parser = GenericOutputParser::new("test-tool".to_string(), "/workspace");
        let stderr = "file.rs:10:5: error: something went wrong";

        let diagnostics = parser.parse_output("", stderr, Path::new("/workspace")).unwrap();

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];

        assert_eq!(diagnostic.file, "file.rs");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.range.start.line, 9); // 1-indexed to 0-indexed
        assert_eq!(diagnostic.source, "test-tool");
    }

    #[test]
    fn test_kani_parser() {
        let parser = KaniOutputParser::new("/workspace");
        let output = "VERIFICATION:- FAILED\nassertion failed: x > 0";

        let diagnostics = parser.parse_output(output, "", Path::new("/workspace")).unwrap();

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert_eq!(diagnostics[0].source, "kani");
    }

    #[test]
    fn test_miri_parser() {
        let parser = MiriOutputParser::new("/workspace");
        let stderr = r#"error: Undefined Behavior: accessing memory with invalid pointer
  --> /workspace/src/main.rs:10:5
   |
10 |     *ptr = 42;
   |     ^^^^^^^^^ accessing memory with invalid pointer
   |
   = help: this is a dangling pointer
backtrace:
   at /workspace/src/main.rs:10:5
   at /workspace/src/lib.rs:20:10"#;

        let diagnostics = parser.parse_output("", stderr, Path::new("/workspace")).unwrap();

        assert!(!diagnostics.is_empty());
        let diag = &diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.source, "miri");
        assert!(diag.message.contains("Undefined Behavior"));
        assert_eq!(diag.file, "src/main.rs");
        assert_eq!(diag.range.start.line, 9); // 0-indexed
        assert!(!diag.related_info.is_empty());
    }

    #[test]
    fn test_cargo_audit_json_parser() {
        let parser = CargoAuditOutputParser::new("/workspace");
        let json_output = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {
                            "id": "RUSTSEC-2021-0139",
                            "title": "ansi_term is Unmaintained",
                            "description": "The ansi_term crate is unmaintained.",
                            "severity": "medium"
                        },
                        "package": {
                            "name": "ansi_term",
                            "version": "0.12.1"
                        }
                    }
                ]
            }
        }"#;

        let diagnostics = parser.parse_output(json_output, "", Path::new("/workspace")).unwrap();

        assert_eq!(diagnostics.len(), 1);
        let diag = &diagnostics[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.source, "cargo-audit");
        assert!(diag.message.contains("RUSTSEC-2021-0139"));
        assert!(diag.message.contains("ansi_term"));
    }

    #[test]
    fn test_cargo_audit_text_parser() {
        let parser = CargoAuditOutputParser::new("/workspace");
        let text_output = r#"RUSTSEC-2021-0139: ansi_term is Unmaintained
    Severity: Medium
    The ansi_term crate is unmaintained.
    
2 vulnerabilities found"#;

        let diagnostics = parser.parse_output(text_output, "", Path::new("/workspace")).unwrap();

        assert!(diagnostics.len() >= 2);
        assert!(diagnostics.iter().any(|d| d.code == Some("RUSTSEC-2021-0139".to_string())));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("2 security vulnerabilities found"))
        );
    }

    #[test]
    fn test_rustdoc_parser() {
        let parser = RustdocOutputParser::new("/workspace");
        let output = r#"error: unresolved link to `NonExistent`
  --> /workspace/src/lib.rs:10:5
   |
10 | /// See [`NonExistent`] for more info
   |          ^^^^^^^^^^^^^ no item named `NonExistent` in scope
   |
warning: missing code example in documentation
  --> /workspace/src/lib.rs:20:1"#;

        let diagnostics = parser.parse_output(output, "", Path::new("/workspace")).unwrap();

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert_eq!(diagnostics[0].file, "src/lib.rs");

        // Find the warning diagnostic
        let warning = diagnostics.iter().find(|d| d.severity == Severity::Warning);
        assert!(warning.is_some());
        assert!(warning.unwrap().message.contains("missing code example"));
    }

    #[test]
    fn test_tarpaulin_text_parser() {
        let parser = TarpaulinOutputParser::new("/workspace");
        let output = r#"Coverage Results: 75.50%
src/main.rs: 85.00%
src/lib.rs: 45.00%
  Uncovered Lines: 10-15, 20, 25-30"#;

        let diagnostics = parser.parse_output(output, "", Path::new("/workspace")).unwrap();

        // Should have overall coverage warning, file warning, and uncovered lines
        assert!(diagnostics.len() > 3);
        assert!(diagnostics.iter().any(|d| d.message.contains("Overall test coverage: 75.50%")));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.file == "src/lib.rs" && d.severity == Severity::Warning)
        );
        assert!(diagnostics.iter().any(|d| d.code == Some("COV003".to_string())));
    }
}
