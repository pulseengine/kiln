//! Diagnostic filtering and grouping functionality
//!
//! This module provides sophisticated filtering, sorting, and grouping
//! capabilities for diagnostic collections to help users focus on specific
//! issues.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    diagnostics::{Diagnostic, DiagnosticCollection, Severity},
    error::{BuildError, BuildResult},
};

/// Filter criteria for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticFilter {
    /// Filter by severity levels
    pub severities: Option<HashSet<Severity>>,
    /// Filter by diagnostic sources (tools)
    pub sources: Option<HashSet<String>>,
    /// Filter by file patterns (glob-style)
    pub file_patterns: Option<Vec<String>>,
    /// Filter by diagnostic codes
    pub codes: Option<HashSet<String>>,
    /// Filter by message content (regex)
    pub message_pattern: Option<String>,
    /// Exclude patterns (files to ignore)
    pub exclude_patterns: Option<Vec<String>>,
    /// Minimum number of diagnostics to include a file
    pub min_count: Option<usize>,
    /// Maximum number of diagnostics to include a file
    pub max_count: Option<usize>,
}

impl Default for DiagnosticFilter {
    fn default() -> Self {
        Self {
            severities: None,
            sources: None,
            file_patterns: None,
            codes: None,
            message_pattern: None,
            exclude_patterns: None,
            min_count: None,
            max_count: None,
        }
    }
}

/// Grouping options for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Group by file path
    File,
    /// Group by severity level
    Severity,
    /// Group by diagnostic source (tool)
    Source,
    /// Group by diagnostic code
    Code,
    /// No grouping (flat list)
    None,
}

/// Sorting options for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Sort by file path
    File,
    /// Sort by severity (errors first)
    Severity,
    /// Sort by line number within file
    Line,
    /// Sort by diagnostic source
    Source,
    /// Sort by diagnostic code
    Code,
    /// No specific sorting
    None,
}

/// Sorting direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending order
    Ascending,
    /// Descending order
    Descending,
}

/// Grouped diagnostics result
#[derive(Debug)]
pub struct GroupedDiagnostics {
    /// Groups with their diagnostics
    pub groups: HashMap<String, Vec<Diagnostic>>,
    /// Total number of diagnostics across all groups
    pub total_count: usize,
    /// Number of groups
    pub group_count: usize,
    /// Grouping method used
    pub grouped_by: GroupBy,
}

/// Filtering and grouping options
#[derive(Debug, Clone)]
pub struct FilterOptions {
    /// Filter criteria
    pub filter: DiagnosticFilter,
    /// Grouping method
    pub group_by: GroupBy,
    /// Sorting method
    pub sort_by: SortBy,
    /// Sorting direction
    pub sort_direction: SortDirection,
    /// Limit number of results
    pub limit: Option<usize>,
    /// Skip first N results
    pub offset: Option<usize>,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            filter: DiagnosticFilter::default(),
            group_by: GroupBy::None,
            sort_by: SortBy::File,
            sort_direction: SortDirection::Ascending,
            limit: None,
            offset: None,
        }
    }
}

/// Diagnostic filtering and grouping engine
pub struct DiagnosticProcessor {
    workspace_root: PathBuf,
}

impl DiagnosticProcessor {
    /// Create a new diagnostic processor
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Apply filtering and grouping to a diagnostic collection
    pub fn process(
        &self,
        collection: &DiagnosticCollection,
        options: &FilterOptions,
    ) -> BuildResult<GroupedDiagnostics> {
        // Apply filters
        let filtered_diagnostics = self.apply_filters(&collection.diagnostics, &options.filter)?;

        // Apply sorting
        let sorted_diagnostics = self.apply_sorting(
            filtered_diagnostics,
            options.sort_by,
            options.sort_direction,
        );

        // Apply pagination
        let paginated_diagnostics =
            self.apply_pagination(sorted_diagnostics, options.offset, options.limit);

        // Apply grouping
        let grouped = self.apply_grouping(paginated_diagnostics, options.group_by);

        Ok(grouped)
    }

    /// Apply filter criteria to diagnostics
    fn apply_filters(
        &self,
        diagnostics: &[Diagnostic],
        filter: &DiagnosticFilter,
    ) -> BuildResult<Vec<Diagnostic>> {
        let mut filtered = Vec::new();

        // Compile regex pattern if provided
        let message_regex = if let Some(pattern) = &filter.message_pattern {
            Some(
                Regex::new(pattern)
                    .map_err(|e| BuildError::Tool(format!("Invalid regex pattern: {}", e)))?,
            )
        } else {
            None
        };

        for diagnostic in diagnostics {
            // Filter by severity
            if let Some(severities) = &filter.severities {
                if !severities.contains(&diagnostic.severity) {
                    continue;
                }
            }

            // Filter by source
            if let Some(sources) = &filter.sources {
                if !sources.contains(&diagnostic.source) {
                    continue;
                }
            }

            // Filter by diagnostic code
            if let Some(codes) = &filter.codes {
                if let Some(code) = &diagnostic.code {
                    if !codes.contains(code) {
                        continue;
                    }
                } else {
                    continue; // No code, but codes filter is active
                }
            }

            // Filter by message pattern
            if let Some(regex) = &message_regex {
                if !regex.is_match(&diagnostic.message) {
                    continue;
                }
            }

            // Filter by file patterns
            if let Some(patterns) = &filter.file_patterns {
                if !self.matches_any_pattern(&diagnostic.file, patterns) {
                    continue;
                }
            }

            // Filter by exclude patterns
            if let Some(exclude_patterns) = &filter.exclude_patterns {
                if self.matches_any_pattern(&diagnostic.file, exclude_patterns) {
                    continue;
                }
            }

            filtered.push(diagnostic.clone());
        }

        // Apply count filters (group by file first)
        if filter.min_count.is_some() || filter.max_count.is_some() {
            filtered = self.apply_count_filters(filtered, filter.min_count, filter.max_count);
        }

        Ok(filtered)
    }

    /// Apply count-based filtering
    fn apply_count_filters(
        &self,
        diagnostics: Vec<Diagnostic>,
        min_count: Option<usize>,
        max_count: Option<usize>,
    ) -> Vec<Diagnostic> {
        // Group by file to count diagnostics per file
        let mut file_counts: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for diagnostic in diagnostics {
            file_counts
                .entry(diagnostic.file.clone())
                .or_insert_with(Vec::new)
                .push(diagnostic);
        }

        // Filter files based on count criteria
        let mut result = Vec::new();
        for (_, file_diagnostics) in file_counts {
            let count = file_diagnostics.len();

            if let Some(min) = min_count {
                if count < min {
                    continue;
                }
            }

            if let Some(max) = max_count {
                if count > max {
                    continue;
                }
            }

            result.extend(file_diagnostics);
        }

        result
    }

    /// Check if a file path matches any of the given patterns
    fn matches_any_pattern(&self, file_path: &str, patterns: &[String]) -> bool {
        for pattern in patterns {
            if self.matches_pattern(file_path, pattern) {
                return true;
            }
        }
        false
    }

    /// Check if a file path matches a glob-style pattern
    fn matches_pattern(&self, file_path: &str, pattern: &str) -> bool {
        // Simple glob-style matching
        if pattern == "*" {
            return true;
        }

        // Convert glob pattern to regex
        let regex_pattern = pattern.replace(".", r"\.").replace("*", ".*").replace("?", ".");

        if let Ok(regex) = Regex::new(&format!("^{}$", regex_pattern)) {
            return regex.is_match(file_path);
        }

        // Fallback to simple string matching
        file_path.contains(pattern)
    }

    /// Apply sorting to diagnostics
    fn apply_sorting(
        &self,
        mut diagnostics: Vec<Diagnostic>,
        sort_by: SortBy,
        direction: SortDirection,
    ) -> Vec<Diagnostic> {
        match sort_by {
            SortBy::File => {
                diagnostics.sort_by(|a, b| {
                    let cmp = a.file.cmp(&b.file);
                    match direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            },
            SortBy::Severity => {
                diagnostics.sort_by(|a, b| {
                    let order_a = severity_order(&a.severity);
                    let order_b = severity_order(&b.severity);
                    let cmp = order_a.cmp(&order_b);
                    match direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            },
            SortBy::Line => {
                diagnostics.sort_by(|a, b| {
                    let cmp = a
                        .file
                        .cmp(&b.file)
                        .then_with(|| a.range.start.line.cmp(&b.range.start.line))
                        .then_with(|| a.range.start.character.cmp(&b.range.start.character));
                    match direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            },
            SortBy::Source => {
                diagnostics.sort_by(|a, b| {
                    let cmp = a.source.cmp(&b.source);
                    match direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            },
            SortBy::Code => {
                diagnostics.sort_by(|a, b| {
                    let cmp = a.code.cmp(&b.code);
                    match direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            },
            SortBy::None => {
                // No sorting
            },
        }

        diagnostics
    }

    /// Apply pagination to diagnostics
    fn apply_pagination(
        &self,
        diagnostics: Vec<Diagnostic>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<Diagnostic> {
        let start = offset.unwrap_or(0);
        let end = if let Some(limit) = limit { start + limit } else { diagnostics.len() };

        diagnostics.into_iter().skip(start).take(end - start).collect()
    }

    /// Apply grouping to diagnostics
    fn apply_grouping(
        &self,
        diagnostics: Vec<Diagnostic>,
        group_by: GroupBy,
    ) -> GroupedDiagnostics {
        let total_count = diagnostics.len();
        let mut groups: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        match group_by {
            GroupBy::File => {
                for diagnostic in diagnostics {
                    groups.entry(diagnostic.file.clone()).or_insert_with(Vec::new).push(diagnostic);
                }
            },
            GroupBy::Severity => {
                for diagnostic in diagnostics {
                    let severity_key = format!("{:?}", diagnostic.severity);
                    groups.entry(severity_key).or_insert_with(Vec::new).push(diagnostic);
                }
            },
            GroupBy::Source => {
                for diagnostic in diagnostics {
                    groups
                        .entry(diagnostic.source.clone())
                        .or_insert_with(Vec::new)
                        .push(diagnostic);
                }
            },
            GroupBy::Code => {
                for diagnostic in diagnostics {
                    let code_key = diagnostic.code.clone().unwrap_or_else(|| "no-code".to_string());
                    groups.entry(code_key).or_insert_with(Vec::new).push(diagnostic);
                }
            },
            GroupBy::None => {
                groups.insert("all".to_string(), diagnostics);
            },
        }

        let group_count = groups.len();

        GroupedDiagnostics {
            groups,
            total_count,
            group_count,
            grouped_by: group_by,
        }
    }
}

/// Get severity order for sorting (errors first)
fn severity_order(severity: &Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Hint => 3,
    }
}

/// Builder for filter options
pub struct FilterOptionsBuilder {
    options: FilterOptions,
}

impl FilterOptionsBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            options: FilterOptions::default(),
        }
    }

    /// Filter by severities
    pub fn severities(mut self, severities: &[Severity]) -> Self {
        self.options.filter.severities = Some(severities.iter().cloned().collect());
        self
    }

    /// Filter by sources
    pub fn sources(mut self, sources: &[String]) -> Self {
        self.options.filter.sources = Some(sources.iter().cloned().collect());
        self
    }

    /// Filter by file patterns
    pub fn file_patterns(mut self, patterns: &[String]) -> Self {
        self.options.filter.file_patterns = Some(patterns.to_vec());
        self
    }

    /// Exclude file patterns
    pub fn exclude_patterns(mut self, patterns: &[String]) -> Self {
        self.options.filter.exclude_patterns = Some(patterns.to_vec());
        self
    }

    /// Filter by message pattern
    pub fn message_pattern(mut self, pattern: String) -> Self {
        self.options.filter.message_pattern = Some(pattern);
        self
    }

    /// Group by criterion
    pub fn group_by(mut self, group_by: GroupBy) -> Self {
        self.options.group_by = group_by;
        self
    }

    /// Sort by criterion
    pub fn sort_by(mut self, sort_by: SortBy, direction: SortDirection) -> Self {
        self.options.sort_by = sort_by;
        self.options.sort_direction = direction;
        self
    }

    /// Limit results
    pub fn limit(mut self, limit: usize) -> Self {
        self.options.limit = Some(limit);
        self
    }

    /// Skip results
    pub fn offset(mut self, offset: usize) -> Self {
        self.options.offset = Some(offset);
        self
    }

    /// Build the filter options
    pub fn build(self) -> FilterOptions {
        self.options
    }
}

impl Default for FilterOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::diagnostics::{Position, Range};

    fn create_test_diagnostic(
        file: &str,
        severity: Severity,
        source: &str,
        message: &str,
        code: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            file: file.to_string(),
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity,
            code: code.map(|s| s.to_string()),
            message: message.to_string(),
            source: source.to_string(),
            related_info: Vec::new(),
        }
    }

    #[test]
    fn test_severity_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let processor = DiagnosticProcessor::new(temp_dir.path().to_path_buf());

        let diagnostics = vec![
            create_test_diagnostic("file1.rs", Severity::Error, "rustc", "error message", None),
            create_test_diagnostic(
                "file2.rs",
                Severity::Warning,
                "clippy",
                "warning message",
                None,
            ),
            create_test_diagnostic("file3.rs", Severity::Info, "info", "info message", None),
        ];

        let mut collection =
            DiagnosticCollection::new(temp_dir.path().to_path_buf(), "test".to_string());
        for diag in diagnostics {
            collection.add_diagnostic(diag);
        }

        let options = FilterOptionsBuilder::new().severities(&[Severity::Error]).build();

        let result = processor.process(&collection, &options).unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.groups["all"][0].severity, Severity::Error);
    }

    #[test]
    fn test_source_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let processor = DiagnosticProcessor::new(temp_dir.path().to_path_buf());

        let diagnostics = vec![
            create_test_diagnostic("file1.rs", Severity::Error, "rustc", "error message", None),
            create_test_diagnostic(
                "file2.rs",
                Severity::Warning,
                "clippy",
                "warning message",
                None,
            ),
        ];

        let mut collection =
            DiagnosticCollection::new(temp_dir.path().to_path_buf(), "test".to_string());
        for diag in diagnostics {
            collection.add_diagnostic(diag);
        }

        let options = FilterOptionsBuilder::new().sources(&["clippy".to_string()]).build();

        let result = processor.process(&collection, &options).unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.groups["all"][0].source, "clippy");
    }

    #[test]
    fn test_file_grouping() {
        let temp_dir = TempDir::new().unwrap();
        let processor = DiagnosticProcessor::new(temp_dir.path().to_path_buf());

        let diagnostics = vec![
            create_test_diagnostic("file1.rs", Severity::Error, "rustc", "error 1", None),
            create_test_diagnostic("file1.rs", Severity::Warning, "clippy", "warning 1", None),
            create_test_diagnostic("file2.rs", Severity::Error, "rustc", "error 2", None),
        ];

        let mut collection =
            DiagnosticCollection::new(temp_dir.path().to_path_buf(), "test".to_string());
        for diag in diagnostics {
            collection.add_diagnostic(diag);
        }

        let options = FilterOptionsBuilder::new().group_by(GroupBy::File).build();

        let result = processor.process(&collection, &options).unwrap();
        assert_eq!(result.group_count, 2);
        assert_eq!(result.groups["file1.rs"].len(), 2);
        assert_eq!(result.groups["file2.rs"].len(), 1);
    }

    #[test]
    fn test_pattern_matching() {
        let temp_dir = TempDir::new().unwrap();
        let processor = DiagnosticProcessor::new(temp_dir.path().to_path_buf());

        assert!(processor.matches_pattern("src/main.rs", "*.rs"));
        assert!(processor.matches_pattern("src/lib.rs", "src/*"));
        assert!(!processor.matches_pattern("README.md", "*.rs"));
        assert!(processor.matches_pattern("any/file.txt", "*"));
    }
}
