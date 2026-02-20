//! Simplified test framework integrated with cargo-wrt
//!
//! This module provides a lightweight test runner that integrates directly
//! with cargo-wrt, replacing the complex wrt-test-registry system with
//! a simpler approach that focuses on ASIL compliance testing.

use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use colored::Colorize;
use wrt_build_core::{
    config::{AsilLevel, BuildProfile},
    formatters::OutputFormat,
};

use super::{GlobalArgs, OutputManager};

/// Serialize Duration as seconds with decimal precision
fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_f64(duration.as_secs_f64())
}

/// Test suite configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// ASIL level to run tests for
    pub asil_level: AsilLevel,
    /// Test filter pattern
    pub filter: Option<String>,
    /// Run only no_std tests
    pub no_std_only: bool,
    /// Output format
    pub output_format: OutputFormat,
    /// Verbose output
    pub verbose: bool,
    /// Number of threads for parallel execution
    pub test_threads: Option<usize>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            asil_level: AsilLevel::QM,
            filter: None,
            no_std_only: false,
            output_format: OutputFormat::Human,
            verbose: false,
            test_threads: None,
        }
    }
}

/// Test result information
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test name
    pub name: String,
    /// Test package
    pub package: String,
    /// Success status
    pub success: bool,
    /// Execution duration
    pub duration: Duration,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether test was skipped
    pub skipped: bool,
    /// Skip reason
    pub skip_reason: Option<String>,
}

/// Test runner for simplified test execution
pub struct TestRunner {
    config: TestConfig,
    output: OutputManager,
}

impl TestRunner {
    /// Create new test runner
    pub fn new(config: TestConfig) -> Self {
        let output = OutputManager::new(config.output_format.clone())
            .with_color(!matches!(config.output_format, OutputFormat::Json));

        Self { config, output }
    }

    /// Run tests for the current ASIL configuration
    pub fn run(&self) -> Result<TestSummary> {
        let start = Instant::now();
        let mut results = Vec::new();

        // Get test packages based on ASIL level
        let packages = self.get_test_packages()?;

        self.output.info(&format!(
            "Running tests for ASIL-{} configuration ({} packages)",
            self.config.asil_level,
            packages.len()
        ));

        // Run tests for each package
        for package in packages {
            let result = self.run_package_tests(&package)?;
            results.push(result);
        }

        // Generate summary
        let summary = TestSummary::from_results(&results, start.elapsed());

        // Output results
        self.output_results(&summary)?;

        Ok(summary)
    }

    /// Get test packages for current ASIL level
    fn get_test_packages(&self) -> Result<Vec<String>> {
        // Core packages always tested
        let mut packages = vec!["wrt-error".to_string(), "wrt-foundation".to_string()];

        // Add ASIL-specific packages
        match self.config.asil_level {
            AsilLevel::QM => {
                // QM includes all packages
                packages.extend(
                    [
                        "wrt-decoder",
                        "wrt-runtime",
                        "wrt-component",
                        "wrt-wasi",
                        "wrtd",
                    ]
                    .iter()
                    .map(|s| s.to_string()),
                );
            },
            AsilLevel::B => {
                // ASIL-B excludes experimental features
                packages.extend(
                    ["wrt-decoder", "wrt-runtime", "wrt-platform"].iter().map(|s| s.to_string()),
                );
            },
            AsilLevel::D => {
                // ASIL-D only core safety-critical packages
                packages.extend(["wrt-platform", "wrt-sync"].iter().map(|s| s.to_string()));
            },
            _ => {}, // Other ASIL levels not used in simplified system
        }

        // Apply filter if provided
        if let Some(filter) = &self.config.filter {
            packages.retain(|p| p.contains(filter));
        }

        Ok(packages)
    }

    /// Run tests for a specific package
    fn run_package_tests(&self, package: &str) -> Result<TestResult> {
        let start = Instant::now();

        // Check if package should be skipped for no_std
        if self.config.no_std_only && self.requires_std(package) {
            return Ok(TestResult {
                name: format!("{} tests", package),
                package: package.to_string(),
                success: true,
                duration: Duration::default(),
                error: None,
                skipped: true,
                skip_reason: Some("Requires std".to_string()),
            });
        }

        // Build test command
        let mut cmd = Command::new("cargo");
        cmd.arg("test").arg("-p").arg(package).arg("--").arg("--quiet");

        // Add test threads if specified
        if let Some(threads) = self.config.test_threads {
            cmd.arg("--test-threads").arg(threads.to_string());
        }

        // Add no_std feature if needed
        if self.config.no_std_only {
            cmd.arg("--no-default-features");
        }

        // Add ASIL-specific features
        let asil_features = self.get_asil_features();
        if !asil_features.is_empty() {
            cmd.arg("--features").arg(asil_features.join(","));
        }

        // Execute tests
        if self.config.verbose {
            self.output.debug(&format!("Running: {:?}", cmd));
        }

        let output = cmd.output().context(format!("Failed to run tests for {}", package))?;

        let duration = start.elapsed();
        let success = output.status.success();

        let error = if !success {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        } else {
            None
        };

        Ok(TestResult {
            name: format!("{} tests", package),
            package: package.to_string(),
            success,
            duration,
            error,
            skipped: false,
            skip_reason: None,
        })
    }

    /// Check if package requires std
    fn requires_std(&self, package: &str) -> bool {
        matches!(package, "wrtd" | "wrt-wasi" | "cargo-wrt")
    }

    /// Get ASIL-specific features
    fn get_asil_features(&self) -> Vec<String> {
        match self.config.asil_level {
            AsilLevel::B => vec!["safety-asil-b".to_string()],
            AsilLevel::D => vec!["safety-asil-d".to_string()],
            _ => vec![],
        }
    }

    /// Output test results
    fn output_results(&self, summary: &TestSummary) -> Result<()> {
        match self.config.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(summary)?);
            },
            OutputFormat::Human => {
                self.output_human_results(summary);
            },
            _ => {
                self.output_human_results(summary);
            },
        }
        Ok(())
    }

    /// Output human-readable results
    fn output_human_results(&self, summary: &TestSummary) {
        println!("\n{}", "Test Results".bold());
        println!("{}", "=".repeat(50));

        // Summary stats
        println!(
            "\n{}: {} passed, {} failed, {} skipped in {:.2}s",
            if summary.all_passed() { "✅ Success".green() } else { "❌ Failed".red() },
            summary.passed.to_string().green(),
            summary.failed.to_string().red(),
            summary.skipped.to_string().yellow(),
            summary.duration.as_secs_f64()
        );

        // Failed tests details
        if summary.failed > 0 {
            println!("\n{}", "Failed Tests:".red().bold());
            for (package, error) in &summary.failures {
                println!("  {} {}", "❌".red(), package.red());
                if let Some(err) = error {
                    // Show first few lines of error
                    let lines: Vec<&str> = err.lines().take(3).collect();
                    for line in lines {
                        println!("     {}", line.dimmed());
                    }
                }
            }
        }

        // Skipped tests
        if summary.skipped > 0 {
            println!("\n{}", "Skipped Tests:".yellow().bold());
            for (package, reason) in &summary.skipped_tests {
                println!("  {} {} - {}", "⚠".yellow(), package, reason.dimmed());
            }
        }
    }
}

/// Test execution summary
#[derive(Debug, Clone, serde::Serialize)]
pub struct TestSummary {
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests  
    pub failed: usize,
    /// Number of skipped tests
    pub skipped: usize,
    /// Total execution time
    #[serde(serialize_with = "serialize_duration")]
    pub duration: Duration,
    /// Failed test details
    pub failures: HashMap<String, Option<String>>,
    /// Skipped test details
    pub skipped_tests: HashMap<String, String>,
}

impl TestSummary {
    /// Create summary from test results
    pub fn from_results(results: &[TestResult], duration: Duration) -> Self {
        let mut summary = Self {
            passed: 0,
            failed: 0,
            skipped: 0,
            duration,
            failures: HashMap::new(),
            skipped_tests: HashMap::new(),
        };

        for result in results {
            if result.skipped {
                summary.skipped += 1;
                summary.skipped_tests.insert(
                    result.package.clone(),
                    result.skip_reason.clone().unwrap_or_default(),
                );
            } else if result.success {
                summary.passed += 1;
            } else {
                summary.failed += 1;
                summary.failures.insert(result.package.clone(), result.error.clone());
            }
        }

        summary
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

/// Run tests with given configuration
pub fn run_tests(args: &GlobalArgs, config: TestConfig) -> Result<()> {
    let runner = TestRunner::new(config);
    let summary = runner.run()?;

    if !summary.all_passed() {
        anyhow::bail!("{} tests failed", summary.failed);
    }

    Ok(())
}

/// Run tests for specific ASIL level
pub fn run_asil_tests(args: &GlobalArgs, asil_level: AsilLevel) -> Result<()> {
    let config = TestConfig {
        asil_level,
        output_format: args.output_format.clone(),
        verbose: args.verbose,
        ..Default::default()
    };

    run_tests(args, config)
}

/// Run no_std compatibility tests
pub fn run_no_std_tests(args: &GlobalArgs) -> Result<()> {
    let config = TestConfig {
        no_std_only: true,
        output_format: args.output_format.clone(),
        verbose: args.verbose,
        ..Default::default()
    };

    run_tests(args, config)
}
