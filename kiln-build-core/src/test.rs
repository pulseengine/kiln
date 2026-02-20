//! Test execution and management

use std::{path::Path, process::Command};

use colored::Colorize;

use crate::{
    build::BuildSystem,
    diagnostics::{Diagnostic, DiagnosticCollection, Range, Severity, ToolOutputParser},
    error::{BuildError, BuildResult},
    parsers::CargoOutputParser,
};

/// Test execution results
#[derive(Debug)]
pub struct TestResults {
    /// Whether all tests passed
    pub success: bool,
    /// Total number of tests run
    pub total_tests: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Test execution duration
    pub duration_ms: u64,
    /// Test output and failures
    pub output: String,
}

/// Test execution options
#[derive(Debug, Clone)]
pub struct TestOptions {
    /// Run only specific test filter
    pub filter: Option<String>,
    /// Include integration tests
    pub integration: bool,
    /// Include doc tests
    pub doc_tests: bool,
    /// Run tests with --nocapture
    pub nocapture: bool,
    /// Parallel test execution
    pub parallel: bool,
}

impl Default for TestOptions {
    fn default() -> Self {
        Self {
            filter: None,
            integration: true,
            doc_tests: true,
            nocapture: false,
            parallel: true,
        }
    }
}

impl BuildSystem {
    /// Run all tests in the workspace
    pub fn run_tests(&self) -> BuildResult<TestResults> {
        self.run_tests_with_options(&TestOptions::default())
    }

    /// Run tests with diagnostic output
    pub fn run_tests_with_diagnostics(
        &self,
        options: &TestOptions,
    ) -> BuildResult<DiagnosticCollection> {
        let start_time = std::time::Instant::now();
        let mut collection =
            DiagnosticCollection::new(self.workspace.root.clone(), "test".to_string());

        // Run tests with JSON output
        let mut cmd = Command::new("cargo");
        cmd.arg("test")
            .arg("--workspace")
            .arg("--message-format=json")
            .current_dir(&self.workspace.root);

        // Add test filter if provided
        if let Some(filter) = &options.filter {
            cmd.arg(filter);
        }

        // Add nocapture if requested
        if options.nocapture {
            cmd.arg("--").arg("--nocapture");
        }

        // Disable parallel execution if requested
        if !options.parallel {
            cmd.arg("--").arg("--test-threads=1");
        }

        // Add features if specified
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo test: {}", e)))?;

        // Parse cargo output for diagnostics
        let parser = CargoOutputParser::new(&self.workspace.root);
        match parser.parse_output(
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            &self.workspace.root,
        ) {
            Ok(diagnostics) => collection.add_diagnostics(diagnostics),
            Err(e) => {
                collection.add_diagnostic(Diagnostic::new(
                    "<test>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("Failed to parse test output: {}", e),
                    "cargo-wrt".to_string(),
                ));
            },
        }

        // Parse test results from output
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let mut test_passed = 0;
        let mut test_failed = 0;

        for line in stdout_str.lines() {
            if line.contains("test result:") {
                // Parse test result line like: "test result: ok. 25 passed; 0 failed; 0"
                // ignored; 0 measured; 0 filtered out"
                if line.contains(" passed);") {
                    if let Some(passed_str) =
                        line.split("ok. ").nth(1).and_then(|s| s.split(" passed);").next())
                    {
                        test_passed = passed_str.trim().parse::<usize>().unwrap_or(0);
                    }
                }
                if line.contains(" failed);") {
                    if let Some(failed_str) =
                        line.split(" passed); ").nth(1).and_then(|s| s.split(" failed);").next())
                    {
                        test_failed = failed_str.trim().parse::<usize>().unwrap_or(0);
                    }
                }
            }
        }

        // Add overall test status
        if !output.status.success() || test_failed > 0 {
            collection.add_diagnostic(Diagnostic::new(
                "<test>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                format!(
                    "Tests failed: {} passed, {} failed",
                    test_passed, test_failed
                ),
                "cargo-wrt".to_string(),
            ));
        } else {
            collection.add_diagnostic(Diagnostic::new(
                "<test>".to_string(),
                Range::entire_line(0),
                Severity::Info,
                format!("All tests passed: {} tests", test_passed),
                "cargo-wrt".to_string(),
            ));
        }

        let duration = start_time.elapsed();
        Ok(collection.finalize(duration.as_millis() as u64))
    }

    /// Run tests with specific options
    pub fn run_tests_with_options(&self, options: &TestOptions) -> BuildResult<TestResults> {
        println!("{} Running WRT test suite...", "🧪".bright_blue());

        let start_time = std::time::Instant::now();

        // Run unit tests
        let unit_results = self.run_unit_tests(options)?;

        // Run integration tests if requested
        let integration_results = if options.integration {
            Some(self.run_integration_tests(options)?)
        } else {
            None
        };

        // Run doc tests if requested
        let doc_results = if options.doc_tests { Some(self.run_doc_tests(options)?) } else { None };

        // Aggregate results
        let mut total_tests = unit_results.total_tests;
        let mut passed = unit_results.passed;
        let mut failed = unit_results.failed;
        let mut output = unit_results.output;

        if let Some(integration) = integration_results {
            total_tests += integration.total_tests;
            passed += integration.passed;
            failed += integration.failed;
            output.push_str(&integration.output);
        }

        if let Some(doc) = doc_results {
            total_tests += doc.total_tests;
            passed += doc.passed;
            failed += doc.failed;
            output.push_str(&doc.output);
        }

        let duration = start_time.elapsed();
        let success = failed == 0;

        if success {
            println!(
                "{} All tests passed! ({}/{} in {:.2}s)",
                "✅".bright_green(),
                passed,
                total_tests,
                duration.as_secs_f64()
            );
        } else {
            println!(
                "{} {} tests failed out of {} ({:.2}s)",
                "❌".bright_red(),
                failed,
                total_tests,
                duration.as_secs_f64()
            );
        }

        Ok(TestResults {
            success,
            total_tests,
            passed,
            failed,
            duration_ms: duration.as_millis() as u64,
            output,
        })
    }

    /// Run unit tests
    fn run_unit_tests(&self, options: &TestOptions) -> BuildResult<TestResults> {
        if self.config.verbose {
            println!("  {} Running unit tests...", "🔬".bright_cyan());
        }

        self.execute_cargo_test("test", options)
    }

    /// Run integration tests
    fn run_integration_tests(&self, options: &TestOptions) -> BuildResult<TestResults> {
        if self.config.verbose {
            println!("  {} Running integration tests...", "🔗".bright_cyan());
        }

        // Check if integration tests exist
        let integration_path = self.workspace.root.join("tests");
        if !integration_path.exists() {
            return Ok(TestResults {
                success: true,
                total_tests: 0,
                passed: 0,
                failed: 0,
                duration_ms: 0,
                output: "No integration tests found\n".to_string(),
            });
        }

        let mut test_options = options.clone();
        self.execute_cargo_test("test --test", &test_options)
    }

    /// Run documentation tests
    fn run_doc_tests(&self, options: &TestOptions) -> BuildResult<TestResults> {
        if self.config.verbose {
            println!("  {} Running documentation tests...", "📚".bright_cyan());
        }

        self.execute_cargo_test("test --doc", options)
    }

    /// Execute cargo test command with options
    fn execute_cargo_test(
        &self,
        test_type: &str,
        options: &TestOptions,
    ) -> BuildResult<TestResults> {
        let mut cmd = Command::new("cargo");

        // Split test_type into command and args
        let args: Vec<&str> = test_type.split_whitespace().collect();
        for arg in args {
            cmd.arg(arg);
        }

        cmd.arg("--workspace").current_dir(&self.workspace.root);

        // Add test filter if specified
        if let Some(filter) = &options.filter {
            cmd.arg(filter);
        }

        // Add nocapture if requested
        if options.nocapture {
            cmd.arg("--nocapture");
        }

        // Add features
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        // Execute test command
        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo test: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let full_output = format!("{}\n{}", stdout, stderr);

        // Parse test results (simplified parsing)
        let (total_tests, passed, failed) = self.parse_test_output(&stdout);

        Ok(TestResults {
            success: output.status.success(),
            total_tests,
            passed,
            failed,
            duration_ms: 0, // Would need to parse from output
            output: full_output,
        })
    }

    /// Parse test output to extract test counts
    fn parse_test_output(&self, output: &str) -> (usize, usize, usize) {
        // Simplified parsing - look for "test result:" line
        for line in output.lines() {
            if line.contains("test result:") {
                // Parse line like: "test result: ok. 42 passed; 0 failed; 0 ignored; 0"
                // measured; 0 filtered out"
                let parts: Vec<&str> = line.split("; ").collect();
                if parts.len() >= 2 {
                    let passed = parts[0]
                        .split_whitespace()
                        .find_map(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);

                    let failed = parts[1]
                        .split_whitespace()
                        .find_map(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);

                    return (passed + failed, passed, failed);
                }
            }
        }

        // Fallback: count test lines
        let test_lines = output
            .lines()
            .filter(|line| {
                let is_test = line.starts_with("test ");
                let is_ok = line.contains("... ok");
                let is_failed = line.contains("... FAILED");
                is_test && (is_ok || is_failed)
            })
            .count();

        let failed_count = output
            .lines()
            .filter(|line| {
                let is_test = line.starts_with("test ");
                let is_failed = line.contains("... FAILED");
                is_test && is_failed
            })
            .count();

        (test_lines, test_lines - failed_count, failed_count)
    }
}

impl TestResults {
    /// Check if all tests passed
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Get test summary string
    pub fn summary(&self) -> String {
        format!(
            "Tests: {} total, {} passed, {} failed",
            self.total_tests, self.passed, self.failed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_default() {
        let options = TestOptions::default();
        assert!(options.integration);
        assert!(options.doc_tests);
        assert!(!options.nocapture);
    }

    #[test]
    fn test_results_success() {
        let results = TestResults {
            success: true,
            total_tests: 10,
            passed: 10,
            failed: 0,
            duration_ms: 1000,
            output: "All tests passed".to_string(),
        };

        assert!(results.is_success());
        assert_eq!(results.summary(), "Tests: 10 total, 10 passed, 0 failed");
    }
}
