//! Unified diagnostic system for LSP-compatible error reporting
//!
//! This module provides a standardized way to collect, format, and output
//! diagnostics from all WRT build operations. The diagnostic format is
//! compatible with the Language Server Protocol (LSP) specification.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// LSP-compatible diagnostic severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Error that prevents successful completion
    Error,
    /// Warning that should be addressed but doesn't prevent completion
    Warning,
    /// Informational message
    Info,
    /// Hint or suggestion for improvement
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

/// Position within a file (0-indexed, LSP format)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Line number (0-indexed)
    pub line: u32,
    /// Character offset (0-indexed, UTF-16 code units)
    pub character: u32,
}

impl Position {
    /// Create a new position
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    /// Create position from 1-indexed line number (character is 0-indexed)
    pub fn from_line_1_indexed(line: u32, character: u32) -> Self {
        Self {
            line: line.saturating_sub(1),
            character,
        }
    }

    /// Create position from 1-indexed line and column numbers (converts both to
    /// 0-indexed)
    pub fn from_line_col_1_indexed(line: u32, column: u32) -> Self {
        Self {
            line: line.saturating_sub(1),
            character: column.saturating_sub(1),
        }
    }
}

/// Range within a file (LSP format)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Start position (inclusive)
    pub start: Position,
    /// End position (exclusive)
    pub end: Position,
}

impl Range {
    /// Create a new range
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a range for a single line
    pub fn single_line(line: u32, start_char: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(line, start_char),
            end: Position::new(line, end_char),
        }
    }

    /// Create a range from 1-indexed line numbers (common in compiler output)
    pub fn from_line_1_indexed(line: u32, start_char: u32, end_char: u32) -> Self {
        Self {
            start: Position::from_line_1_indexed(line, start_char),
            end: Position::from_line_1_indexed(line, end_char),
        }
    }

    /// Create a range spanning an entire line
    pub fn entire_line(line: u32) -> Self {
        Self {
            start: Position::new(line, 0),
            end: Position::new(line, u32::MAX),
        }
    }

    /// Create a range from 1-indexed line and column numbers (converts both to
    /// 0-indexed)
    pub fn from_line_col_1_indexed(start_line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start: Position::from_line_col_1_indexed(start_line, start_col),
            end: Position::from_line_col_1_indexed(start_line, end_col),
        }
    }
}

/// Related diagnostic information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedInfo {
    /// File path relative to workspace root
    pub file: String,
    /// Range within the file
    pub range: Range,
    /// Related message
    pub message: String,
}

impl RelatedInfo {
    /// Create new related information
    pub fn new(file: String, range: Range, message: String) -> Self {
        Self {
            file,
            range,
            message,
        }
    }
}

/// Individual diagnostic item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// File path relative to workspace root
    pub file: String,
    /// Range within the file
    pub range: Range,
    /// Severity level
    pub severity: Severity,
    /// Optional error/warning code
    pub code: Option<String>,
    /// Human-readable message
    pub message: String,
    /// Source tool that generated this diagnostic (e.g., "cargo", "kani")
    pub source: String,
    /// Related information (e.g., where a symbol is defined)
    pub related_info: Vec<RelatedInfo>,
}

impl Diagnostic {
    /// Create a new diagnostic
    pub fn new(
        file: String,
        range: Range,
        severity: Severity,
        message: String,
        source: String,
    ) -> Self {
        Self {
            file,
            range,
            severity,
            code: None,
            message,
            source,
            related_info: Vec::new(),
        }
    }

    /// Set error code
    pub fn with_code(mut self, code: String) -> Self {
        self.code = Some(code);
        self
    }

    /// Add related information
    pub fn with_related_info(mut self, related: RelatedInfo) -> Self {
        self.related_info.push(related);
        self
    }

    /// Add multiple related information items
    pub fn with_related_infos(mut self, related: Vec<RelatedInfo>) -> Self {
        self.related_info.extend(related);
        self
    }
}

/// Summary statistics for a diagnostic collection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    /// Total number of diagnostics
    pub total: usize,
    /// Number of errors
    pub errors: usize,
    /// Number of warnings
    pub warnings: usize,
    /// Number of info messages
    pub infos: usize,
    /// Number of hints
    pub hints: usize,
    /// Number of files with diagnostics
    pub files_with_diagnostics: usize,
    /// Duration of operation in milliseconds
    pub duration_ms: u64,
}

impl DiagnosticSummary {
    /// Create summary from diagnostic collection
    pub fn from_diagnostics(diagnostics: &[Diagnostic], duration_ms: u64) -> Self {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        let mut hints = 0;
        let mut files = std::collections::HashSet::new();

        for diagnostic in diagnostics {
            match diagnostic.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
                Severity::Hint => hints += 1,
            }
            files.insert(&diagnostic.file);
        }

        Self {
            total: diagnostics.len(),
            errors,
            warnings,
            infos,
            hints,
            files_with_diagnostics: files.len(),
            duration_ms,
        }
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Check if operation was successful (no errors)
    pub fn is_success(&self) -> bool {
        !self.has_errors()
    }
}

/// Collection of diagnostics with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCollection {
    /// Version of diagnostic format
    pub version: String,
    /// Timestamp when diagnostics were generated
    pub timestamp: String,
    /// Workspace root path
    pub workspace_root: String,
    /// Command that generated these diagnostics
    pub command: String,
    /// Individual diagnostics
    pub diagnostics: Vec<Diagnostic>,
    /// Summary statistics
    pub summary: DiagnosticSummary,
}

impl DiagnosticCollection {
    /// Create a new diagnostic collection
    pub fn new(workspace_root: PathBuf, command: String) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let workspace_root_str = workspace_root.to_string_lossy().to_string();

        Self {
            version: "1.0".to_string(),
            timestamp,
            workspace_root: workspace_root_str,
            command,
            diagnostics: Vec::new(),
            summary: DiagnosticSummary {
                total: 0,
                errors: 0,
                warnings: 0,
                infos: 0,
                hints: 0,
                files_with_diagnostics: 0,
                duration_ms: 0,
            },
        }
    }

    /// Add a diagnostic to the collection
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Add multiple diagnostics
    pub fn add_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    /// Finalize the collection with timing information
    pub fn finalize(mut self, duration_ms: u64) -> Self {
        self.summary = DiagnosticSummary::from_diagnostics(&self.diagnostics, duration_ms);
        self
    }

    /// Get diagnostics by severity
    pub fn by_severity(&self, severity: Severity) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == severity).collect()
    }

    /// Get diagnostics by file
    pub fn by_file(&self, file: &str) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.file == file).collect()
    }

    /// Get diagnostics by source tool
    pub fn by_source(&self, source: &str) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.source == source).collect()
    }

    /// Group diagnostics by file
    pub fn group_by_file(&self) -> HashMap<String, Vec<&Diagnostic>> {
        let mut groups = HashMap::new();
        for diagnostic in &self.diagnostics {
            groups.entry(diagnostic.file.clone()).or_insert_with(Vec::new).push(diagnostic);
        }
        groups
    }

    /// Check if collection has any errors
    pub fn has_errors(&self) -> bool {
        self.summary.has_errors()
    }

    /// Check if operation was successful
    pub fn is_success(&self) -> bool {
        self.summary.is_success()
    }
}

/// Trait for parsing external tool output into diagnostics
pub trait ToolOutputParser {
    /// Parse tool output into diagnostics
    fn parse_output(
        &self,
        stdout: &str,
        stderr: &str,
        working_dir: &Path,
    ) -> crate::error::BuildResult<Vec<Diagnostic>>;

    /// Get the name of the tool this parser handles
    fn tool_name(&self) -> &'static str;

    /// Get the source identifier for diagnostics
    fn source_name(&self) -> &'static str {
        self.tool_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_creation() {
        let pos = Position::new(5, 10);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.character, 10);

        let pos_1_indexed = Position::from_line_1_indexed(6, 10);
        assert_eq!(pos_1_indexed.line, 5);
        assert_eq!(pos_1_indexed.character, 10);
    }

    #[test]
    fn test_range_creation() {
        let range = Range::single_line(5, 0, 10);
        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.character, 10);
    }

    #[test]
    fn test_diagnostic_creation() {
        let diagnostic = Diagnostic::new(
            "src/main.rs".to_string(),
            Range::single_line(10, 0, 5),
            Severity::Error,
            "undefined variable".to_string(),
            "rustc".to_string(),
        )
        .with_code("E0425".to_string());

        assert_eq!(diagnostic.file, "src/main.rs");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code, Some("E0425".to_string()));
        assert_eq!(diagnostic.source, "rustc");
    }

    #[test]
    fn test_diagnostic_collection() {
        let mut collection =
            DiagnosticCollection::new(PathBuf::from("/workspace"), "build".to_string());

        let diagnostic1 = Diagnostic::new(
            "src/main.rs".to_string(),
            Range::single_line(10, 0, 5),
            Severity::Error,
            "error message".to_string(),
            "rustc".to_string(),
        );

        let diagnostic2 = Diagnostic::new(
            "src/lib.rs".to_string(),
            Range::single_line(5, 0, 10),
            Severity::Warning,
            "warning message".to_string(),
            "clippy".to_string(),
        );

        collection.add_diagnostic(diagnostic1);
        collection.add_diagnostic(diagnostic2);

        let collection = collection.finalize(1000);

        assert_eq!(collection.summary.total, 2);
        assert_eq!(collection.summary.errors, 1);
        assert_eq!(collection.summary.warnings, 1);
        assert_eq!(collection.summary.files_with_diagnostics, 2);
        assert!(collection.has_errors());
        assert!(!collection.is_success());
    }

    #[test]
    fn test_diagnostic_summary() {
        let diagnostics = vec![
            Diagnostic::new(
                "file1.rs".to_string(),
                Range::single_line(1, 0, 5),
                Severity::Error,
                "error".to_string(),
                "tool".to_string(),
            ),
            Diagnostic::new(
                "file1.rs".to_string(),
                Range::single_line(2, 0, 5),
                Severity::Warning,
                "warning".to_string(),
                "tool".to_string(),
            ),
            Diagnostic::new(
                "file2.rs".to_string(),
                Range::single_line(1, 0, 5),
                Severity::Info,
                "info".to_string(),
                "tool".to_string(),
            ),
        ];

        let summary = DiagnosticSummary::from_diagnostics(&diagnostics, 500);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.warnings, 1);
        assert_eq!(summary.infos, 1);
        assert_eq!(summary.hints, 0);
        assert_eq!(summary.files_with_diagnostics, 2);
        assert_eq!(summary.duration_ms, 500);
        assert!(summary.has_errors());
        assert!(!summary.is_success());
    }
}
