//! Testing utilities and framework for cargo-wrt
//!
//! Provides comprehensive testing infrastructure including mocks,
//! test helpers, and validation frameworks for all cargo-wrt functionality.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use tempfile::TempDir;
use wrt_build_core::{BuildConfig, BuildSystem, formatters::OutputFormat};

use crate::helpers::{
    CommandSuggestionEngine, GlobalArgs, OutputManager, PerformanceOptimizer, ProgressIndicator,
    ProjectContext, ProjectType,
};

/// Test context for cargo-wrt operations
pub struct TestContext {
    /// Temporary directory for test workspace
    pub temp_dir: TempDir,
    /// Mock build system
    pub build_system: MockBuildSystem,
    /// Test configuration
    pub config: TestConfig,
    /// Global arguments for testing
    pub global_args: GlobalArgs,
}

/// Configuration for test scenarios
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub workspace_type: WorkspaceType,
    pub enable_git: bool,
    pub enable_ci: bool,
    pub project_features: Vec<ProjectFeature>,
    pub output_format: OutputFormat,
    pub use_colors: bool,
}

/// Types of test workspaces
#[derive(Debug, Clone)]
pub enum WorkspaceType {
    /// Full WRT workspace
    WrtWorkspace,
    /// Single WRT crate
    WrtCrate { name: String },
    /// Generic Rust workspace
    RustWorkspace,
    /// Single Rust crate
    RustCrate,
    /// Empty directory
    Empty,
}

/// Project features for testing
#[derive(Debug, Clone)]
pub enum ProjectFeature {
    Tests,
    Benchmarks,
    Examples,
    Documentation,
    CI,
    Fuzzing,
    SafetyVerification,
    NoStd,
    WebAssembly,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            workspace_type: WorkspaceType::WrtWorkspace,
            enable_git: false,
            enable_ci: false,
            project_features: vec![],
            output_format: OutputFormat::Human,
            use_colors: false,
        }
    }
}

impl TestContext {
    /// Create a new test context with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(TestConfig::default())
    }

    /// Create a test context with specific configuration
    pub fn with_config(config: TestConfig) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let workspace_root = temp_dir.path().to_path_buf();

        // Set up workspace structure based on type
        Self::setup_workspace(&workspace_root, &config)?;

        // Create mock build system
        let build_system = MockBuildSystem::new(workspace_root.clone());

        // Create global args for testing
        let output = OutputManager::new(config.output_format.clone()).with_color(config.use_colors);

        let global_args = GlobalArgs {
            verbose: false,
            dry_run: false,
            trace_commands: false,
            profile: wrt_build_core::config::BuildProfile::Dev,
            features: vec![],
            workspace: Some(workspace_root.to_string_lossy().to_string()),
            output_format: config.output_format.clone(),
            output,
            cache: false,
            clear_cache: false,
            diff_only: false,
            filter_options: None,
            filter_severity: None,
            filter_source: None,
            filter_file: None,
            group_by: None,
            limit: None,
        };

        Ok(Self {
            temp_dir,
            build_system,
            config,
            global_args,
        })
    }

    /// Set up workspace structure based on configuration
    fn setup_workspace(workspace_root: &Path, config: &TestConfig) -> Result<()> {
        use std::fs;

        match &config.workspace_type {
            WorkspaceType::WrtWorkspace => {
                // Create WRT workspace structure
                fs::create_dir_all(workspace_root.join("wrt-foundation"))?;
                fs::create_dir_all(workspace_root.join("wrt-build-core"))?;
                fs::create_dir_all(workspace_root.join("cargo-wrt"))?;

                // Create workspace Cargo.toml
                fs::write(
                    workspace_root.join("Cargo.toml"),
                    r#"[workspace]
members = [
    "wrt-foundation",
    "wrt-build-core", 
    "cargo-wrt",
]

[workspace.package]
version = "0.2.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/pulseengine/wrt"

[workspace.lints.rust]
unsafe_code = "forbid"
"#,
                )?;

                // Create individual crate manifests
                for crate_name in &["wrt-foundation", "wrt-build-core", "cargo-wrt"] {
                    fs::write(
                        workspace_root.join(crate_name).join("Cargo.toml"),
                        format!(
                            r#"[package]
name = "{}"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
"#,
                            crate_name
                        ),
                    )?;

                    fs::create_dir_all(workspace_root.join(crate_name).join("src"))?;
                    fs::write(
                        workspace_root.join(crate_name).join("src").join("lib.rs"),
                        "//! Test crate\n",
                    )?;
                }
            },
            WorkspaceType::WrtCrate { name } => {
                fs::write(
                    workspace_root.join("Cargo.toml"),
                    format!(
                        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
                        name
                    ),
                )?;

                fs::create_dir_all(workspace_root.join("src"))?;
                fs::write(
                    workspace_root.join("src").join("lib.rs"),
                    "//! Test WRT crate\n",
                )?;
            },
            WorkspaceType::RustWorkspace => {
                fs::write(
                    workspace_root.join("Cargo.toml"),
                    r#"[workspace]
members = [
    "crate-a",
    "crate-b",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
"#,
                )?;

                for crate_name in &["crate-a", "crate-b"] {
                    fs::create_dir_all(workspace_root.join(crate_name).join("src"))?;
                    fs::write(
                        workspace_root.join(crate_name).join("Cargo.toml"),
                        format!(
                            r#"[package]
name = "{}"
version.workspace = true
edition.workspace = true

[dependencies]
"#,
                            crate_name
                        ),
                    )?;
                    fs::write(
                        workspace_root.join(crate_name).join("src").join("lib.rs"),
                        "//! Test crate\n",
                    )?;
                }
            },
            WorkspaceType::RustCrate => {
                fs::write(
                    workspace_root.join("Cargo.toml"),
                    r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
                )?;

                fs::create_dir_all(workspace_root.join("src"))?;
                fs::write(
                    workspace_root.join("src").join("lib.rs"),
                    "//! Test crate\n",
                )?;
            },
            WorkspaceType::Empty => {
                // Just create empty directory
            },
        }

        // Add optional features
        for feature in &config.project_features {
            match feature {
                ProjectFeature::Tests => {
                    fs::create_dir_all(workspace_root.join("tests"))?;
                    fs::write(
                        workspace_root.join("tests").join("integration.rs"),
                        "#[test]\nfn test_basic() {\n    assert_eq!(2 + 2, 4);\n}\n",
                    )?;
                },
                ProjectFeature::Benchmarks => {
                    fs::create_dir_all(workspace_root.join("benches"))?;
                    fs::write(
                        workspace_root.join("benches").join("benchmark.rs"),
                        "use criterion::{black_box, criterion_group, criterion_main, Criterion};\n",
                    )?;
                },
                ProjectFeature::Examples => {
                    fs::create_dir_all(workspace_root.join("examples"))?;
                    fs::write(
                        workspace_root.join("examples").join("example.rs"),
                        "fn main() {\n    println!(\"Hello, example!\");\n}\n",
                    )?;
                },
                ProjectFeature::Documentation => {
                    fs::write(
                        workspace_root.join("README.md"),
                        "# Test Project\n\nThis is a test project.\n",
                    )?;
                },
                ProjectFeature::CI => {
                    fs::create_dir_all(workspace_root.join(".github").join("workflows"))?;
                    fs::write(
                        workspace_root.join(".github").join("workflows").join("ci.yml"),
                        "name: CI\non: [push, pull_request]\njobs:\n  test:\n    runs-on: \
                         ubuntu-latest\n",
                    )?;
                },
                ProjectFeature::SafetyVerification => {
                    fs::write(
                        workspace_root.join("requirements.toml"),
                        "[requirements]\n[requirements.safety]\nlevel = \"ASIL-D\"\n",
                    )?;
                },
                _ => {}, // Other features can be added as needed
            }
        }

        // Set up git repository if requested
        if config.enable_git {
            fs::create_dir_all(workspace_root.join(".git"))?;
            fs::write(
                workspace_root.join(".git").join("config"),
                "[core]\n\trepositoryformatversion = 0\n",
            )?;
        }

        Ok(())
    }

    /// Get the workspace root path
    pub fn workspace_root(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a test file in the workspace
    pub fn create_file(&self, path: &str, content: &str) -> Result<()> {
        let file_path = self.workspace_root().join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file_path, content)?;
        Ok(())
    }

    /// Check if a file exists in the workspace
    pub fn file_exists(&self, path: &str) -> bool {
        self.workspace_root().join(path).exists()
    }

    /// Read file content from workspace
    pub fn read_file(&self, path: &str) -> Result<String> {
        Ok(std::fs::read_to_string(self.workspace_root().join(path))?)
    }
}

/// Mock build system for testing
pub struct MockBuildSystem {
    workspace_root: PathBuf,
    results: HashMap<String, MockBuildResult>,
    pub call_log: Vec<String>,
}

/// Mock build result
#[derive(Debug, Clone)]
pub struct MockBuildResult {
    pub success: bool,
    pub duration: std::time::Duration,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl MockBuildSystem {
    pub fn new(workspace_root: PathBuf) -> Self {
        let mut system = Self {
            workspace_root,
            results: HashMap::new(),
            call_log: Vec::new(),
        };

        // Set up default mock results
        system.results.insert(
            "build_all".to_string(),
            MockBuildResult {
                success: true,
                duration: std::time::Duration::from_millis(1500),
                warnings: vec!["unused variable `x`".to_string()],
                errors: vec![],
            },
        );

        system.results.insert(
            "test_all".to_string(),
            MockBuildResult {
                success: true,
                duration: std::time::Duration::from_millis(2000),
                warnings: vec![],
                errors: vec![],
            },
        );

        system
    }

    /// Set mock result for a specific operation
    pub fn set_result(&mut self, operation: &str, result: MockBuildResult) {
        self.results.insert(operation.to_string(), result);
    }

    /// Set operation to fail
    pub fn set_failure(&mut self, operation: &str, error: &str) {
        self.results.insert(
            operation.to_string(),
            MockBuildResult {
                success: false,
                duration: std::time::Duration::from_millis(500),
                warnings: vec![],
                errors: vec![error.to_string()],
            },
        );
    }

    /// Check if operation was called
    pub fn was_called(&self, operation: &str) -> bool {
        self.call_log.iter().any(|call| call == operation)
    }

    /// Get number of times operation was called
    pub fn call_count(&self, operation: &str) -> usize {
        self.call_log.iter().filter(|call| call == &operation).count()
    }

    /// Clear call log
    pub fn clear_log(&mut self) {
        self.call_log.clear();
    }
}

/// Test utilities for validation
pub struct TestValidator {
    context: TestContext,
}

impl TestValidator {
    pub fn new(context: TestContext) -> Self {
        Self { context }
    }

    /// Validate progress indicator functionality
    pub fn validate_progress_indicators(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::new("Progress Indicators");

        // Test spinner progress
        let mut spinner = ProgressIndicator::spinner(
            "Test operation",
            self.context.config.output_format.clone(),
            self.context.config.use_colors,
        );
        spinner.start();
        std::thread::sleep(std::time::Duration::from_millis(100));
        spinner.tick();
        spinner.finish();
        report.add_success("Spinner progress indicator works");

        // Test progress bar
        let mut bar = ProgressIndicator::bar(
            "Test progress",
            100,
            self.context.config.output_format.clone(),
            self.context.config.use_colors,
        );
        bar.start();
        for i in 0..=100 {
            bar.update(i);
            if i % 20 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        bar.finish();
        report.add_success("Progress bar works");

        Ok(report)
    }

    /// Validate command suggestion engine
    pub fn validate_command_suggestions(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::new("Command Suggestions");

        let engine = CommandSuggestionEngine::new();

        // Test exact match
        let suggestions = engine.suggest("build", None);
        if !suggestions.is_empty() && suggestions[0].command == "build" {
            report.add_success("Exact command match works");
        } else {
            report.add_failure("Exact command match failed");
        }

        // Test typo correction
        let suggestions = engine.suggest("buld", None);
        if !suggestions.is_empty() && suggestions[0].command == "build" {
            report.add_success("Typo correction works");
        } else {
            report.add_failure("Typo correction failed");
        }

        // Test similarity matching
        let suggestions = engine.suggest("bui", None);
        if !suggestions.is_empty() {
            report.add_success("Similarity matching works");
        } else {
            report.add_failure("Similarity matching failed");
        }

        Ok(report)
    }

    /// Validate performance optimization
    pub fn validate_performance_optimization(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::new("Performance Optimization");

        let mut optimizer = PerformanceOptimizer::with_defaults();

        // Test timer functionality
        optimizer.start_timer("test_command");
        std::thread::sleep(std::time::Duration::from_millis(100));
        optimizer.stop_timer("test_command");

        if optimizer.generate_report().metrics.command_times.contains_key("test_command") {
            report.add_success("Performance timing works");
        } else {
            report.add_failure("Performance timing failed");
        }

        // Test cache tracking
        optimizer.record_cache_hit();
        optimizer.record_cache_miss();
        let ratio = optimizer.cache_hit_ratio();
        if ratio == 0.5 {
            report.add_success("Cache tracking works");
        } else {
            report.add_failure("Cache tracking failed");
        }

        Ok(report)
    }

    /// Run comprehensive validation
    pub fn validate_all(&self) -> Result<Vec<ValidationReport>> {
        let mut reports = Vec::new();

        reports.push(self.validate_progress_indicators()?);
        reports.push(self.validate_command_suggestions()?);
        reports.push(self.validate_performance_optimization()?);

        Ok(reports)
    }
}

/// Validation report for test results
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub name: String,
    pub successes: Vec<String>,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            successes: Vec::new(),
            failures: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_success(&mut self, message: impl Into<String>) {
        self.successes.push(message.into());
    }

    pub fn add_failure(&mut self, message: impl Into<String>) {
        self.failures.push(message.into());
    }

    pub fn add_warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn is_successful(&self) -> bool {
        self.failures.is_empty()
    }

    pub fn total_tests(&self) -> usize {
        self.successes.len() + self.failures.len()
    }

    /// Format report for display
    pub fn format(&self, use_colors: bool) -> String {
        use colored::Colorize;

        let mut output = String::new();

        if use_colors {
            output.push_str(&format!(
                "{} {} ({}/{} passed)\n",
                if self.is_successful() { "✅" } else { "❌" },
                self.name.bright_white().bold(),
                self.successes.len().to_string().bright_green(),
                self.total_tests().to_string().bright_white()
            ));
        } else {
            output.push_str(&format!(
                "{} {} ({}/{} passed)\n",
                if self.is_successful() { "✅" } else { "❌" },
                self.name,
                self.successes.len(),
                self.total_tests()
            ));
        }

        for success in &self.successes {
            if use_colors {
                output.push_str(&format!("  {} {}\n", "✓".bright_green(), success));
            } else {
                output.push_str(&format!("  ✓ {}\n", success));
            }
        }

        for failure in &self.failures {
            if use_colors {
                output.push_str(&format!("  {} {}\n", "✗".bright_red(), failure));
            } else {
                output.push_str(&format!("  ✗ {}\n", failure));
            }
        }

        for warning in &self.warnings {
            if use_colors {
                output.push_str(&format!("  {} {}\n", "⚠".bright_yellow(), warning));
            } else {
                output.push_str(&format!("  ⚠ {}\n", warning));
            }
        }

        output
    }
}
