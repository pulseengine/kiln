//! Native text search functionality to replace external grep dependency
//!
//! Provides fast, native Rust-based text searching capabilities for source code
//! analysis without relying on external tools like grep.

use std::{
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;
use walkdir::WalkDir;

use crate::error::{BuildError, BuildResult};

/// Text search options
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// File patterns to include (e.g., "*.rs")
    pub include_patterns: Vec<String>,
    /// File patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Whether to search recursively
    pub recursive: bool,
    /// Case sensitive search
    pub case_sensitive: bool,
    /// Include line numbers in results
    pub include_line_numbers: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            include_patterns: vec!["*.rs".to_string()],
            exclude_patterns: Vec::new(),
            recursive: true,
            case_sensitive: true,
            include_line_numbers: true,
        }
    }
}

/// Search result for a single match
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// File path where match was found
    pub file_path: PathBuf,
    /// Line number (1-indexed)
    pub line_number: usize,
    /// Line content
    pub line_content: String,
    /// Whether this line appears to be a comment
    pub is_comment: bool,
    /// Whether this line appears to be in a test context
    pub is_test_context: bool,
}

/// Text searcher
pub struct TextSearcher {
    options: SearchOptions,
}

impl TextSearcher {
    /// Create a new text searcher with default options
    pub fn new() -> Self {
        Self {
            options: SearchOptions::default(),
        }
    }

    /// Create a new text searcher with custom options
    pub fn with_options(options: SearchOptions) -> Self {
        Self { options }
    }

    /// Search for a pattern in the given directory
    pub fn search(&self, pattern: &str, search_dir: &Path) -> BuildResult<Vec<SearchMatch>> {
        let regex = if self.options.case_sensitive {
            Regex::new(pattern)
        } else {
            Regex::new(&format!("(?i){}", pattern))
        }
        .map_err(|e| BuildError::Tool(format!("Invalid regex pattern '{}': {}", pattern, e)))?;

        let mut matches = Vec::new();

        // Walk directory structure
        let walker = if self.options.recursive {
            WalkDir::new(search_dir)
        } else {
            WalkDir::new(search_dir).max_depth(1)
        };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // Skip directories
            if !path.is_file() {
                continue;
            }

            // Check if file matches include patterns
            if !self.should_include_file(path) {
                continue;
            }

            // Search within the file
            if let Ok(file_matches) = self.search_file(&regex, path) {
                matches.extend(file_matches);
            }
        }

        Ok(matches)
    }

    /// Search for unsafe code usage
    pub fn search_unsafe_code(&self, search_dir: &Path) -> BuildResult<Vec<SearchMatch>> {
        // Pattern to match unsafe blocks and functions
        let pattern = r"\bunsafe\b";
        let matches = self.search(pattern, search_dir)?;

        // Filter out comments and focus on actual unsafe code
        let mut filtered_matches = Vec::new();
        let mut i = 0;

        while i < matches.len() {
            let m = &matches[i];

            // Skip if it's a comment or not actual unsafe code
            if m.is_comment || !m.line_content.trim().contains("unsafe") {
                i += 1;
                continue;
            }

            // Check if this unsafe block has proper safety documentation
            let has_safety_comment = self.has_safety_documentation(&matches, i)?;
            let has_allow_attribute = self.has_allow_unsafe_attribute(&matches, i)?;

            // Only include as a violation if it lacks both safety documentation and allow
            // attribute
            if !has_safety_comment && !has_allow_attribute {
                filtered_matches.push(m.clone());
            }

            i += 1;
        }

        Ok(filtered_matches)
    }

    /// Search for panic! macro usage
    pub fn search_panic_usage(&self, search_dir: &Path) -> BuildResult<Vec<SearchMatch>> {
        let pattern = r"panic!";
        let matches = self.search(pattern, search_dir)?;

        // Filter out comments and test code
        let filtered_matches: Vec<_> =
            matches.into_iter().filter(|m| !m.is_comment && !m.is_test_context).collect();

        Ok(filtered_matches)
    }

    /// Check if an unsafe block has a SAFETY comment
    fn has_safety_documentation(
        &self,
        matches: &[SearchMatch],
        unsafe_index: usize,
    ) -> BuildResult<bool> {
        let unsafe_match = &matches[unsafe_index];

        // Look backwards up to 5 lines for a SAFETY comment
        for j in 1..=5 {
            if unsafe_index >= j {
                let prev_match_idx = unsafe_index - j;
                if prev_match_idx < matches.len() {
                    let prev_match = &matches[prev_match_idx];
                    // Check if same file and nearby line
                    if prev_match.file_path == unsafe_match.file_path
                        && unsafe_match.line_number.saturating_sub(prev_match.line_number) <= 5
                    {
                        // Check for SAFETY comment patterns
                        let line = prev_match.line_content.trim();
                        if line.contains("SAFETY:")
                            || line.contains("Safety:")
                            || line.contains("# Safety")
                            || line.contains("## Safety")
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        // Also check for re-reading the file to look at exact previous lines
        // This handles cases where we might not have all lines in our matches
        if let Ok(content) = fs::read_to_string(&unsafe_match.file_path) {
            let lines: Vec<&str> = content.lines().collect();
            let unsafe_line_idx = unsafe_match.line_number.saturating_sub(1);

            // Look up to 5 lines before the unsafe block
            for offset in 1..=5 {
                if unsafe_line_idx >= offset {
                    let prev_line_idx = unsafe_line_idx - offset;
                    if prev_line_idx < lines.len() {
                        let line = lines[prev_line_idx].trim();
                        if line.contains("SAFETY:")
                            || line.contains("Safety:")
                            || line.contains("# Safety")
                            || line.contains("## Safety")
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Check if an unsafe block has #[allow(unsafe_code)] attribute
    fn has_allow_unsafe_attribute(
        &self,
        matches: &[SearchMatch],
        unsafe_index: usize,
    ) -> BuildResult<bool> {
        let unsafe_match = &matches[unsafe_index];

        // Look for #[allow(unsafe_code)] in the same line or previous lines
        if unsafe_match.line_content.contains("#[allow(unsafe_code)]") {
            return Ok(true);
        }

        // Check previous lines for the attribute
        if let Ok(content) = fs::read_to_string(&unsafe_match.file_path) {
            let lines: Vec<&str> = content.lines().collect();
            let unsafe_line_idx = unsafe_match.line_number.saturating_sub(1);

            // Look up to 3 lines before for the attribute
            for offset in 1..=3 {
                if unsafe_line_idx >= offset {
                    let prev_line_idx = unsafe_line_idx - offset;
                    if prev_line_idx < lines.len() {
                        let line = lines[prev_line_idx].trim();
                        if line.contains("#[allow(unsafe_code)]") {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Search for .unwrap() usage
    pub fn search_unwrap_usage(&self, search_dir: &Path) -> BuildResult<Vec<SearchMatch>> {
        let pattern = r"\.unwrap\(\)";
        let matches = self.search(pattern, search_dir)?;

        // Filter out comments and test code
        let filtered_matches: Vec<_> =
            matches.into_iter().filter(|m| !m.is_comment && !m.is_test_context).collect();

        Ok(filtered_matches)
    }

    /// Check if a file should be included based on patterns
    fn should_include_file(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check exclude patterns first
        for exclude_pattern in &self.options.exclude_patterns {
            if self.matches_glob_pattern(file_name, exclude_pattern) {
                return false;
            }
        }

        // Check include patterns
        if self.options.include_patterns.is_empty() {
            return true;
        }

        for include_pattern in &self.options.include_patterns {
            if self.matches_glob_pattern(file_name, include_pattern) {
                return true;
            }
        }

        false
    }

    /// Simple glob pattern matching for file names
    fn matches_glob_pattern(&self, file_name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with("*.") {
            let extension = &pattern[2..];
            return file_name.ends_with(extension);
        }

        if pattern.contains('*') {
            // More complex glob patterns would need a proper glob library
            // For now, handle simple cases
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return file_name.starts_with(parts[0]) && file_name.ends_with(parts[1]);
            }
        }

        file_name == pattern
    }

    /// Search within a single file
    fn search_file(&self, regex: &Regex, file_path: &Path) -> BuildResult<Vec<SearchMatch>> {
        let content = fs::read_to_string(file_path).map_err(|e| {
            BuildError::Tool(format!(
                "Failed to read file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let mut matches = Vec::new();
        let mut in_test_module = false;
        let mut brace_depth = 0;

        for (line_number, line) in content.lines().enumerate() {
            let line_number = line_number + 1; // Convert to 1-indexed

            // Track if we're in a test module
            if line.contains("#[cfg(test)]") || line.contains("mod tests") {
                in_test_module = true;
                brace_depth = 0;
            }

            // Track brace depth to know when we exit test modules
            brace_depth += line.chars().filter(|&c| c == '{').count() as i32;
            brace_depth -= line.chars().filter(|&c| c == '}').count() as i32;

            if in_test_module && brace_depth <= 0 {
                in_test_module = false;
            }

            // Check if line matches pattern
            if regex.is_match(line) {
                matches.push(SearchMatch {
                    file_path: file_path.to_path_buf(),
                    line_number,
                    line_content: line.to_string(),
                    is_comment: self.is_comment_line(line),
                    is_test_context: in_test_module || self.is_test_function(line),
                });
            }
        }

        Ok(matches)
    }

    /// Check if a line is a comment
    fn is_comment_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*")
    }

    /// Check if a line is in a test function context
    fn is_test_function(&self, line: &str) -> bool {
        line.contains("#[test]") || line.contains("#[cfg(test)]")
    }
}

/// Count matches from search results
pub fn count_matches(matches: &[SearchMatch]) -> usize {
    matches.len()
}

/// Count matches excluding comments and test code
pub fn count_production_matches(matches: &[SearchMatch]) -> usize {
    matches.iter().filter(|m| !m.is_comment && !m.is_test_context).count()
}

/// Format search results for display
pub fn format_matches(matches: &[SearchMatch], max_display: Option<usize>) -> String {
    let mut output = String::new();

    let display_count = max_display.unwrap_or(matches.len()).min(matches.len());

    for (i, search_match) in matches.iter().take(display_count).enumerate() {
        output.push_str(&format!(
            "{}:{}:{}\n",
            search_match.file_path.display(),
            search_match.line_number,
            search_match.line_content.trim()
        ));

        if i >= 9 && matches.len() > 10 {
            output.push_str(&format!("... and {} more matches\n", matches.len() - 10));
            break;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();
        assert!(options.recursive);
        assert!(options.case_sensitive);
        assert_eq!(options.include_patterns, vec!["*.rs"]);
    }

    #[test]
    fn test_glob_pattern_matching() {
        let searcher = TextSearcher::new();

        assert!(searcher.matches_glob_pattern("test.rs", "*.rs"));
        assert!(searcher.matches_glob_pattern("main.rs", "*.rs"));
        assert!(!searcher.matches_glob_pattern("test.txt", "*.rs"));
        assert!(searcher.matches_glob_pattern("anything", "*"));
    }

    #[test]
    fn test_comment_detection() {
        let searcher = TextSearcher::new();

        assert!(searcher.is_comment_line("// This is a comment"));
        assert!(searcher.is_comment_line("    // Indented comment"));
        assert!(searcher.is_comment_line("/* Block comment */"));
        assert!(searcher.is_comment_line("    * Documentation"));
        assert!(!searcher.is_comment_line("let x = 5; // Not a comment line"));
        assert!(!searcher.is_comment_line("fn test() {}"));
    }

    #[test]
    fn test_search_in_temp_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.rs");

        fs::write(
            &file_path,
            r#"
fn main() {
    unsafe {
        println!("Hello");
    }
    // unsafe comment
    panic!("error");
    let x = some_option.unwrap();
}
"#,
        )?;

        let searcher = TextSearcher::new();

        // Test unsafe search
        let unsafe_matches = searcher.search_unsafe_code(temp_dir.path())?;
        assert_eq!(count_production_matches(&unsafe_matches), 1); // Only the actual unsafe block

        // Test panic search
        let panic_matches = searcher.search_panic_usage(temp_dir.path())?;
        assert_eq!(count_production_matches(&panic_matches), 1);

        // Test unwrap search
        let unwrap_matches = searcher.search_unwrap_usage(temp_dir.path())?;
        assert_eq!(count_production_matches(&unwrap_matches), 1);

        Ok(())
    }
}
