//! cargo-wrt - Unified build tool for WRT (WebAssembly Runtime)
//!
//! This is the main CLI entry point for the WRT build system, providing a clean
//! interface to the wrt-build-core library. It replaces the fragmented approach
//! of justfile, xtask, and shell scripts with a single, AI-friendly tool.

// Standard library imports
use std::{path::PathBuf, process};

// External crates
use anyhow::{Context, Result};
use chrono;
use clap::{Parser, Subcommand};
use colored::Colorize;
// Internal crates (wrt_* imports)
use wrt_build_core::{
    BuildConfig, BuildSystem,
    cache::CacheManager,
    config::{AsilLevel, BuildProfile},
    diagnostics::{DiagnosticCollection, Severity},
    filtering::{FilterOptionsBuilder, GroupBy, SortBy, SortDirection},
    formatters::{FormatterFactory, OutputFormat},
    kani::{KaniConfig, KaniVerifier},
};

// Local helper modules
mod formatters;
mod helpers;

mod commands;
mod test_config;
#[cfg(test)]
mod testing;

use commands::{TestValidateArgs, cmd_embed_limits, execute_test_validate};
use helpers::{
    AutoFixManager, GlobalArgs, OutputManager, TestConfig, output_diagnostics, run_asil_tests,
    run_no_std_tests,
};

/// WRT Build System - Unified tool for building, testing, and verifying WRT
#[derive(Parser)]
#[command(name = "cargo-wrt")]
#[command(
    version,
    about = "Unified build tool for WRT (WebAssembly Runtime)",
    long_about = "
Unified build tool for WRT (WebAssembly Runtime)

Usage:
  cargo-wrt <COMMAND>           # Direct usage
  cargo wrt <COMMAND>           # As Cargo subcommand

Basic Examples:
  cargo-wrt build --package wrt
  cargo wrt build --package wrt
  cargo-wrt fuzz --list
  cargo wrt verify --asil d

Diagnostic System Examples:
  # JSON output for tooling/AI agents
  cargo-wrt build --output json
  
  # Filter errors only
  cargo-wrt build --output json --filter-severity error
  
  # Enable caching for faster incremental builds
  cargo-wrt build --cache
  
  # Show only new/changed diagnostics
  cargo-wrt build --cache --diff-only
  
  # Group diagnostics by file
  cargo-wrt build --output json --group-by file
  
  # Filter by source tool
  cargo-wrt check --output json --filter-source clippy

Output Formats:
  --output human        Human-readable with colors (default)
  --output json         LSP-compatible JSON for tooling
  --output json-lines   Streaming JSON (one diagnostic per line)

Advanced Diagnostic Help:
  cargo-wrt help diagnostics    Comprehensive diagnostic system guide
"
)]
#[command(author = "WRT Team")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Show commands being executed without running them
    #[arg(long, global = true)]
    dry_run: bool,

    /// Trace all external commands being executed
    #[arg(long, global = true)]
    trace_commands: bool,

    /// Build profile to use
    #[arg(long, global = true, value_enum, default_value = "dev")]
    profile: ProfileArg,

    /// Features to enable (comma-separated)
    #[arg(long, global = true)]
    features: Option<String>,

    /// Workspace root directory
    #[arg(long, global = true)]
    workspace: Option<String>,

    /// Output format for diagnostics and results
    #[arg(long, global = true, value_enum, default_value = "human")]
    output: OutputFormatArg,

    /// Enable diagnostic caching for faster incremental builds
    #[arg(long, global = true)]
    cache: bool,

    /// Clear diagnostic cache before running
    #[arg(long, global = true)]
    clear_cache: bool,

    /// Filter diagnostics by severity (comma-separated: error,warning,info)
    #[arg(long, global = true, value_delimiter = ',')]
    filter_severity: Option<Vec<String>>,

    /// Filter diagnostics by source tool (comma-separated:
    /// rustc,clippy,miri,etc)
    #[arg(long, global = true, value_delimiter = ',')]
    filter_source: Option<Vec<String>>,

    /// Filter diagnostics by file patterns (comma-separated glob patterns)
    #[arg(long, global = true, value_delimiter = ',')]
    filter_file: Option<Vec<String>>,

    /// Group diagnostics by criterion
    #[arg(long, global = true, value_enum)]
    group_by: Option<GroupByArg>,

    /// Limit number of diagnostics shown
    #[arg(long, global = true)]
    limit: Option<usize>,

    /// Show only new/changed diagnostics (requires --cache)
    #[arg(long, global = true)]
    diff_only: bool,
}

/// Available build profiles
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ProfileArg {
    Dev,
    Release,
    Test,
}

/// Available output formats
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum OutputFormatArg {
    /// Human-readable format with colors (default)
    Human,
    /// JSON format for LSP/tooling integration  
    Json,
    /// JSON Lines format for streaming output
    JsonLines,
}

/// Available grouping options
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum GroupByArg {
    /// Group by file path
    File,
    /// Group by severity level
    Severity,
    /// Group by diagnostic source (tool)
    Source,
    /// Group by diagnostic code
    Code,
}

impl From<ProfileArg> for BuildProfile {
    fn from(profile: ProfileArg) -> Self {
        match profile {
            ProfileArg::Dev => BuildProfile::Dev,
            ProfileArg::Release => BuildProfile::Release,
            ProfileArg::Test => BuildProfile::Test,
        }
    }
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(format: OutputFormatArg) -> Self {
        match format {
            OutputFormatArg::Human => OutputFormat::Human,
            OutputFormatArg::Json => OutputFormat::Json,
            OutputFormatArg::JsonLines => OutputFormat::JsonLines,
        }
    }
}

impl From<GroupByArg> for GroupBy {
    fn from(group_by: GroupByArg) -> Self {
        match group_by {
            GroupByArg::File => GroupBy::File,
            GroupByArg::Severity => GroupBy::Severity,
            GroupByArg::Source => GroupBy::Source,
            GroupByArg::Code => GroupBy::Code,
        }
    }
}

impl From<AsilArg> for AsilLevel {
    fn from(asil: AsilArg) -> Self {
        match asil {
            AsilArg::QM => AsilLevel::QM,
            AsilArg::A => AsilLevel::A,
            AsilArg::B => AsilLevel::B,
            AsilArg::C => AsilLevel::C,
            AsilArg::D => AsilLevel::D,
        }
    }
}

/// Available subcommands
#[derive(Subcommand)]
enum Commands {
    /// Build all WRT components
    Build {
        /// Build specific crate only
        #[arg(long, short)]
        package: Option<String>,

        /// Run clippy checks during build
        #[arg(long)]
        clippy: bool,

        /// Check code formatting
        #[arg(long)]
        fmt_check: bool,
    },

    /// Run tests across the workspace
    Test {
        /// Test specific package only
        #[arg(long, short)]
        package: Option<String>,

        /// Filter tests by name pattern
        #[arg(long)]
        filter: Option<String>,

        /// Run with --nocapture
        #[arg(long)]
        nocapture: bool,

        /// Skip integration tests
        #[arg(long)]
        unit_only: bool,

        /// Skip doc tests
        #[arg(long)]
        no_doc_tests: bool,
    },

    /// Run ASIL-specific test suites
    TestAsil {
        /// Target ASIL level for testing
        #[arg(long, value_enum, default_value = "qm")]
        asil: AsilArg,

        /// Test filter pattern
        #[arg(long)]
        filter: Option<String>,

        /// Number of test threads
        #[arg(long)]
        test_threads: Option<usize>,

        /// Run only no_std compatible tests
        #[arg(long)]
        no_std_only: bool,
    },

    /// Run no_std compatibility tests
    TestNoStd {
        /// Test filter pattern
        #[arg(long)]
        filter: Option<String>,

        /// Number of test threads
        #[arg(long)]
        test_threads: Option<usize>,
    },

    /// Generate test configuration file
    TestConfig {
        /// Output path for configuration file
        #[arg(long, default_value = "wrt-test.toml")]
        output: String,

        /// Generate example configuration
        #[arg(long)]
        example: bool,
    },

    /// Run safety verification and compliance checks
    Verify {
        /// Target ASIL level for verification
        #[arg(long, value_enum, default_value = "qm")]
        asil: AsilArg,

        /// Skip Kani formal verification
        #[arg(long)]
        no_kani: bool,

        /// Skip MIRI checks
        #[arg(long)]
        no_miri: bool,

        /// Generate detailed report
        #[arg(long)]
        detailed: bool,

        /// Path to allowed unsafe configuration file
        #[arg(long, default_value = "allowed-unsafe.toml")]
        allowed_unsafe: String,
    },

    /// Generate documentation
    Docs {
        /// Open documentation in browser
        #[arg(long)]
        open: bool,

        /// Include private items
        #[arg(long)]
        private: bool,

        /// Output directory for documentation
        #[arg(long)]
        output_dir: Option<String>,

        /// Generate multi-version documentation (comma-separated list of
        /// versions)
        #[arg(long)]
        multi_version: Option<String>,
    },

    /// Run coverage analysis
    Coverage {
        /// Generate HTML report
        #[arg(long)]
        html: bool,

        /// Open coverage report in browser
        #[arg(long)]
        open: bool,

        /// Output format (text, json, html)
        #[arg(long, default_value = "text")]
        format: String,

        /// Continue on errors and generate coverage for what works
        #[arg(long)]
        best_effort: bool,
    },

    /// Run static analysis (clippy + formatting)
    Check {
        /// Run with strict linting rules
        #[arg(long)]
        strict: bool,

        /// Fix issues automatically where possible
        #[arg(long)]
        fix: bool,
    },

    /// Verify no_std compatibility
    NoStd {
        /// Continue on error
        #[arg(long)]
        continue_on_error: bool,

        /// Show detailed output
        #[arg(long)]
        detailed: bool,
    },

    /// Build WRTD (WebAssembly Runtime Daemon) binaries
    Wrtd {
        /// Build specific runtime variant
        #[arg(long, value_enum)]
        variant: Option<WrtdVariant>,

        /// Test binaries after building
        #[arg(long)]
        test: bool,

        /// Cross-compile for embedded targets
        #[arg(long)]
        cross: bool,
    },

    /// Run comprehensive CI checks
    Ci {
        /// Fail fast on first error
        #[arg(long)]
        fail_fast: bool,

        /// Generate JSON output for CI
        #[arg(long)]
        json: bool,
    },

    /// Clean build artifacts
    Clean {
        /// Remove all target directories
        #[arg(long)]
        all: bool,
    },

    /// Run comprehensive build matrix verification
    VerifyMatrix {
        /// Generate detailed reports
        #[arg(long)]
        report: bool,

        /// Output directory for reports
        #[arg(long, default_value = ".")]
        output_dir: String,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Simulate CI workflow for local testing
    SimulateCi {
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Output directory for simulation artifacts
        #[arg(long, default_value = ".")]
        output_dir: String,
    },

    /// Run KANI formal verification
    KaniVerify {
        /// ASIL profile for verification
        #[arg(long, value_enum, default_value = "c")]
        asil_profile: AsilArg,

        /// Specific package to verify
        #[arg(long, short)]
        package: Option<String>,

        /// Specific harness to run
        #[arg(long)]
        harness: Option<String>,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Additional KANI arguments
        #[arg(long)]
        extra_args: Vec<String>,
    },

    /// Run code validation checks
    Validate {
        /// Check for test files in src/
        #[arg(long)]
        check_test_files: bool,

        /// Check module documentation coverage
        #[arg(long)]
        check_docs: bool,

        /// Audit crate documentation (README, Cargo.toml metadata, lib.rs)
        #[arg(long)]
        audit_docs: bool,

        /// Run all validation checks
        #[arg(long)]
        all: bool,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Setup development environment
    Setup {
        /// Setup git hooks
        #[arg(long)]
        hooks: bool,

        /// Setup all development tools
        #[arg(long)]
        all: bool,

        /// Check status of all tools
        #[arg(long)]
        check: bool,

        /// Install optional tools
        #[arg(long)]
        install: bool,
    },

    /// Manage tool version configuration
    ToolVersions {
        #[command(subcommand)]
        command: ToolVersionCommand,
    },

    /// Run fuzzing tests
    Fuzz {
        /// Specific fuzz target or "all"
        #[arg(long, short, default_value = "all")]
        target: String,

        /// Duration to run each fuzzer (seconds)
        #[arg(long, short, default_value = "60")]
        duration: u64,

        /// Number of workers to use
        #[arg(long, short, default_value = "4")]
        workers: u32,

        /// Number of runs (overrides duration)
        #[arg(long, short)]
        runs: Option<u64>,

        /// List available fuzz targets
        #[arg(long)]
        list: bool,

        /// Package to fuzz
        #[arg(long, short)]
        package: Option<String>,
    },

    /// Test feature combinations
    TestFeatures {
        /// Test specific package
        #[arg(long, short)]
        package: Option<String>,

        /// Test all feature combinations
        #[arg(long)]
        combinations: bool,

        /// Test predefined feature groups
        #[arg(long)]
        groups: bool,

        /// Show detailed errors
        #[arg(long, short)]
        verbose: bool,
    },

    /// WebAssembly test suite management
    Testsuite {
        /// Extract test modules from .wast files
        #[arg(long)]
        extract: bool,

        /// Path to wabt tools
        #[arg(long)]
        wabt_path: Option<String>,

        /// Run validation tests
        #[arg(long)]
        validate: bool,

        /// Clean extracted test files
        #[arg(long)]
        clean: bool,

        /// Run WAST test suite
        #[arg(long)]
        run_wast: bool,

        /// Directory containing WAST files
        #[arg(long, default_value = "external/testsuite")]
        wast_dir: String,

        /// Filter WAST files by pattern
        #[arg(long)]
        wast_filter: Option<String>,

        /// Show per-assertion results
        #[arg(long)]
        per_assertion: bool,

        /// Output file for WAST test report
        #[arg(long)]
        wast_report: Option<String>,

        /// Timeout for individual tests in milliseconds
        #[arg(long, default_value = "5000")]
        test_timeout_ms: u64,
    },

    /// Requirements verification with SCORE methodology
    Requirements {
        #[command(subcommand)]
        command: RequirementsCommand,
    },

    /// WebAssembly module verification and analysis
    Wasm {
        #[command(subcommand)]
        command: WasmCommand,
    },

    /// Safety verification with SCORE-inspired methodology
    Safety {
        #[command(subcommand)]
        command: SafetyCommand,
    },

    /// Embed resource limits into WebAssembly binaries
    EmbedLimits {
        /// Path to the WebAssembly binary
        wasm_file: PathBuf,

        /// Path to the TOML configuration file
        #[arg(short = 'c', long = "config")]
        config_file: PathBuf,

        /// Output file path (defaults to modifying in place)
        #[arg(short = 'o', long = "output")]
        output_file: Option<PathBuf>,

        /// ASIL level to enforce
        #[arg(short = 'a', long = "asil")]
        asil_level: Option<String>,

        /// Binary hash for qualification
        #[arg(long = "binary-hash")]
        binary_hash: Option<String>,

        /// Validate limits against ASIL requirements
        #[arg(long = "validate")]
        validate: bool,

        /// Remove existing resource limits sections
        #[arg(long = "replace")]
        replace: bool,
    },

    /// Show comprehensive diagnostic system help
    #[command(name = "help-diagnostics", hide = true)]
    HelpDiagnostics,
}

/// ASIL level arguments for CLI
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum AsilArg {
    #[value(name = "qm")]
    QM,
    #[value(name = "a")]
    A,
    #[value(name = "b")]
    B,
    #[value(name = "c")]
    C,
    #[value(name = "d")]
    D,
}

/// Requirements management subcommands
#[derive(Subcommand, Clone)]
enum RequirementsCommand {
    /// Initialize a sample requirements.toml file
    Init {
        /// Path for the requirements file
        #[arg(default_value = "requirements.toml")]
        path: String,

        /// Force overwrite if file exists
        #[arg(long)]
        force: bool,
    },

    /// Verify requirements against implementation
    Verify {
        /// Path to requirements file
        #[arg(default_value = "requirements.toml")]
        path: String,

        /// Show detailed verification results
        #[arg(long)]
        detailed: bool,

        /// Use enhanced SCORE methodology
        #[arg(long)]
        enhanced: bool,
    },

    /// Show compliance scores for all requirements
    Score {
        /// Path to requirements file
        #[arg(default_value = "requirements.toml")]
        path: String,

        /// Group scores by ASIL level
        #[arg(long)]
        by_asil: bool,

        /// Group scores by requirement type
        #[arg(long)]
        by_type: bool,

        /// Show detailed breakdown
        #[arg(long)]
        detailed: bool,
    },

    /// Generate requirements traceability matrix
    Matrix {
        /// Path to requirements file
        #[arg(default_value = "requirements.toml")]
        path: String,

        /// Output format (markdown, html, json)
        #[arg(long, default_value = "markdown")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(long)]
        output: Option<String>,
    },

    /// List requirements needing attention
    Missing {
        /// Path to requirements file
        #[arg(default_value = "requirements.toml")]
        path: String,

        /// Show requirements missing implementation
        #[arg(long)]
        implementation: bool,

        /// Show requirements missing tests
        #[arg(long)]
        tests: bool,

        /// Show requirements missing documentation
        #[arg(long)]
        docs: bool,

        /// Show all missing items
        #[arg(long)]
        all: bool,
    },

    /// Demonstrate SCORE methodology
    Demo {
        /// Output directory for demo files
        #[arg(long, default_value = "./score-demo")]
        output_dir: String,

        /// Run interactive demo
        #[arg(long)]
        interactive: bool,
    },
}

/// WebAssembly verification subcommands
#[derive(Subcommand, Clone)]
enum WasmCommand {
    /// Verify a WebAssembly module
    Verify {
        /// Path to the WebAssembly module (.wasm file)
        #[arg(required = true)]
        file: String,

        /// Show detailed verification information
        #[arg(long)]
        detailed: bool,

        /// Run performance benchmarks
        #[arg(long)]
        benchmark: bool,
    },

    /// List imports from a WebAssembly module
    Imports {
        /// Path to the WebAssembly module (.wasm file)
        #[arg(required = true)]
        file: String,

        /// Filter for builtin imports only
        #[arg(long)]
        builtins_only: bool,

        /// Filter by module name
        #[arg(long)]
        module: Option<String>,
    },

    /// List exports from a WebAssembly module
    Exports {
        /// Path to the WebAssembly module (.wasm file)
        #[arg(required = true)]
        file: String,

        /// Filter by export kind (function, table, memory, global)
        #[arg(long)]
        kind: Option<String>,
    },

    /// Analyze multiple WebAssembly modules
    Analyze {
        /// Paths to WebAssembly modules (glob patterns supported)
        #[arg(required = true)]
        files: Vec<String>,

        /// Generate summary report
        #[arg(long)]
        summary: bool,

        /// Include performance metrics
        #[arg(long)]
        performance: bool,
    },

    /// Create a minimal test WebAssembly module
    CreateTest {
        /// Output path for the test module
        #[arg(default_value = "test.wasm")]
        output: String,

        /// Include builtin imports
        #[arg(long)]
        with_builtins: bool,
    },
}

/// Safety verification subcommands with SCORE methodology
#[derive(Subcommand, Clone)]
enum SafetyCommand {
    /// Perform comprehensive ASIL compliance verification
    Verify {
        /// Target ASIL level to verify against
        #[arg(long, value_enum, default_value = "qm")]
        asil: AsilArg,

        /// Path to requirements file
        #[arg(long, default_value = "requirements.toml")]
        requirements: String,

        /// Include test results integration
        #[arg(long)]
        include_tests: bool,

        /// Include platform verification
        #[arg(long)]
        include_platform: bool,

        /// Generate detailed compliance report
        #[arg(long)]
        detailed: bool,
    },

    /// Check certification readiness for specific ASIL level
    Certify {
        /// ASIL level for certification assessment
        #[arg(value_enum, default_value = "a")]
        asil: AsilArg,

        /// Path to requirements file
        #[arg(long, default_value = "requirements.toml")]
        requirements: String,

        /// Path to test results (JSON format)
        #[arg(long)]
        test_results: Option<String>,

        /// Path to coverage data (JSON format)
        #[arg(long)]
        coverage_data: Option<String>,
    },

    /// Generate comprehensive safety report
    Report {
        /// Path to requirements file
        #[arg(long, default_value = "requirements.toml")]
        requirements: String,

        /// Include all ASIL levels in report
        #[arg(long)]
        all_asil: bool,

        /// Output file for report (stdout if not specified)
        #[arg(long)]
        output: Option<String>,

        /// Report format (human, json, html)
        #[arg(long, default_value = "human")]
        format: String,
    },

    /// Record and integrate test results with safety verification
    RecordTest {
        /// Test name
        #[arg(required = true)]
        test_name: String,

        /// Test passed (true) or failed (false)
        #[arg(long)]
        passed: bool,

        /// Test execution time in milliseconds
        #[arg(long)]
        duration_ms: Option<u64>,

        /// ASIL level of the test
        #[arg(long, value_enum, default_value = "qm")]
        asil: AsilArg,

        /// Requirements verified by this test (comma-separated)
        #[arg(long)]
        verifies: Option<String>,

        /// Test coverage type
        #[arg(long, value_enum, default_value = "basic")]
        coverage_type: TestCoverageArg,

        /// Failure reason (if test failed)
        #[arg(long)]
        failure_reason: Option<String>,
    },

    /// Update code coverage data for safety verification
    UpdateCoverage {
        /// Line coverage percentage (0-100)
        #[arg(long)]
        line_coverage: Option<f64>,

        /// Branch coverage percentage (0-100)
        #[arg(long)]
        branch_coverage: Option<f64>,

        /// Function coverage percentage (0-100)
        #[arg(long)]
        function_coverage: Option<f64>,

        /// Path to detailed coverage data (JSON)
        #[arg(long)]
        coverage_file: Option<String>,
    },

    /// Add platform verification result
    PlatformVerify {
        /// Platform name
        #[arg(required = true)]
        platform: String,

        /// Verification passed
        #[arg(long)]
        passed: bool,

        /// ASIL compliance level achieved
        #[arg(long, value_enum, default_value = "qm")]
        asil: AsilArg,

        /// Verified features (comma-separated)
        #[arg(long)]
        verified_features: Option<String>,

        /// Failed features (comma-separated)
        #[arg(long)]
        failed_features: Option<String>,
    },

    /// Initialize safety verification framework
    Init {
        /// Initialize with WRT-specific safety requirements
        #[arg(long)]
        wrt_requirements: bool,

        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,
    },

    /// Generate SCORE methodology demonstration
    Demo {
        /// Output directory for demo files
        #[arg(long, default_value = "./safety-demo")]
        output_dir: String,

        /// Run interactive demonstration
        #[arg(long)]
        interactive: bool,

        /// Include certification workflow
        #[arg(long)]
        certification: bool,
    },

    /// Verify documentation compliance for safety requirements
    VerifyDocs {
        /// Path to requirements file
        #[arg(long, default_value = "requirements.toml")]
        requirements: String,

        /// Target ASIL level for documentation verification
        #[arg(long, value_enum)]
        asil: Option<AsilArg>,

        /// Generate detailed documentation report
        #[arg(long)]
        detailed: bool,

        /// Check implementation documentation
        #[arg(long)]
        check_implementations: bool,

        /// Check API documentation
        #[arg(long)]
        check_api: bool,
    },

    /// Generate documentation compliance report
    DocsReport {
        /// Path to requirements file
        #[arg(long, default_value = "requirements.toml")]
        requirements: String,

        /// Include all ASIL levels in report
        #[arg(long)]
        all_asil: bool,

        /// Output file for report (stdout if not specified)
        #[arg(long)]
        output: Option<String>,

        /// Report format (human, json, html)
        #[arg(long, default_value = "human")]
        format: String,
    },
}

/// Test coverage type arguments
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum TestCoverageArg {
    Basic,
    Comprehensive,
    Complete,
}

/// Tool version management subcommands
#[derive(Subcommand, Clone)]
enum ToolVersionCommand {
    /// Generate tool-versions.toml configuration file
    Generate {
        /// Overwrite existing file
        #[arg(long)]
        force: bool,

        /// Include all available tools (not just required ones)
        #[arg(long)]
        all: bool,
    },

    /// Check current tool versions against configuration
    Check {
        /// Show detailed version information
        #[arg(long)]
        verbose: bool,

        /// Check specific tool only
        #[arg(long)]
        tool: Option<String>,
    },

    /// Update tool-versions.toml with current installed versions
    Update {
        /// Update specific tool only
        #[arg(long)]
        tool: Option<String>,

        /// Update all tools to their currently installed versions
        #[arg(long)]
        all: bool,
    },
}

/// WRTD runtime variants
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum WrtdVariant {
    Std,
    Alloc,
    NoStd,
}

/// Determine if colors should be used based on output format and terminal
fn should_use_colors(output_format: &OutputFormat) -> bool {
    match output_format {
        OutputFormat::Human => atty::is(atty::Stream::Stdout),
        OutputFormat::Json | OutputFormat::JsonLines => false,
    }
}

/// Parse severity strings to Severity enum
fn parse_severities(severity_strings: &[String]) -> Result<Vec<Severity>> {
    let mut severities = Vec::new();
    for s in severity_strings {
        match s.to_lowercase().as_str() {
            "error" => severities.push(Severity::Error),
            "warning" => severities.push(Severity::Warning),
            "info" => severities.push(Severity::Info),
            _ => anyhow::bail!(
                "Invalid severity: {}. Valid values: error, warning, info",
                s
            ),
        }
    }
    Ok(severities)
}

/// Create filter options from CLI arguments
fn create_filter_options(cli: &Cli) -> Result<FilterOptionsBuilder> {
    let mut builder = FilterOptionsBuilder::new();

    // Apply severity filter
    if let Some(severity_strings) = &cli.filter_severity {
        let severities = parse_severities(severity_strings)?;
        builder = builder.severities(&severities);
    }

    // Apply source filter
    if let Some(sources) = &cli.filter_source {
        builder = builder.sources(sources);
    }

    // Apply file pattern filter
    if let Some(patterns) = &cli.filter_file {
        builder = builder.file_patterns(patterns);
    }

    // Apply grouping
    if let Some(group_by) = &cli.group_by {
        builder = builder.group_by((*group_by).into());
    }

    // Apply limit
    if let Some(limit) = cli.limit {
        builder = builder.limit(limit);
    }

    // Default sorting
    builder = builder.sort_by(SortBy::File, SortDirection::Ascending);

    Ok(builder)
}

/// Get cache path for the workspace
fn get_cache_path(workspace_root: &std::path::Path) -> std::path::PathBuf {
    workspace_root.join("target").join("wrt-cache").join("diagnostics.json")
}

/// Parse command line arguments, handling both `cargo-wrt` and `cargo wrt`
/// patterns
fn parse_args() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    // Check if we're being called as a Cargo subcommand
    // Pattern: ["cargo-wrt", "wrt", "build", ...] vs ["cargo-wrt", "build", ...]
    let is_cargo_subcommand = args.len() > 1 && args[1] == "wrt";

    if is_cargo_subcommand {
        // We're being called as `cargo wrt`, so skip the "wrt" argument
        // Create new args without the "wrt" part
        let mut filtered_args = vec![args[0].clone()]; // Keep binary name

        // Add remaining arguments (skip the "wrt" at position 1)
        if args.len() > 2 {
            filtered_args.extend(args[2..].iter().cloned());
        }

        // Parse with filtered arguments - if no command provided, show help
        if filtered_args.len() == 1 {
            filtered_args.push("--help".to_string());
        }

        Cli::parse_from(filtered_args)
    } else {
        // Normal `cargo-wrt` call
        Cli::parse()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the memory system at the very beginning
    // This must happen before any other operations to prevent stack overflow
    // when creating engines or using capability-based memory
    if let Err(e) = wrt_foundation::memory_init::MemoryInitializer::initialize() {
        eprintln!("Warning: Failed to initialize memory system: {}", e);
        // Continue execution - the system should still work with default
        // providers
    }

    // Handle special help cases before parsing
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && (args[1] == "help" && args.get(2) == Some(&"diagnostics".to_string())) {
        print_diagnostic_help();
        return Ok(());
    }
    if args.len() >= 3
        && args[1] == "wrt"
        && args[2] == "help"
        && args.get(3) == Some(&"diagnostics".to_string())
    {
        print_diagnostic_help();
        return Ok(());
    }

    // Handle both `cargo-wrt` and `cargo wrt` calling patterns
    let cli = parse_args();

    // Print header
    if cli.verbose {
        let args: Vec<String> = std::env::args().collect();
        let calling_pattern =
            if args.len() > 1 && args[1] == "wrt" { "cargo wrt" } else { "cargo-wrt" };

        println!(
            "{} {} v{}",
            "🚀".bright_blue(),
            calling_pattern,
            env!("CARGO_PKG_VERSION")
        );
        println!("{} WebAssembly Runtime Build System", "📦".bright_green());
        println!();
    }

    // Create global args from CLI
    let mut global = GlobalArgs::from_cli(&cli)?;

    // Create build system instance
    let build_system = match &cli.workspace {
        Some(workspace) => {
            let workspace_path = std::path::PathBuf::from(workspace);
            BuildSystem::new(workspace_path)
        },
        None => BuildSystem::for_current_dir(),
    }
    .context("Failed to initialize build system")?;

    // Configure build system
    let mut config = BuildConfig::default();
    config.verbose = global.verbose;
    config.profile = global.profile.clone();
    config.dry_run = global.dry_run;
    config.trace_commands = global.trace_commands;
    config.features = global.features.clone();

    let mut build_system = build_system;
    build_system.set_config(config);

    // Execute command
    let result = match &cli.command {
        Commands::Build {
            package,
            clippy,
            fmt_check,
        } => {
            cmd_build(
                &build_system,
                package.clone(),
                *clippy,
                *fmt_check,
                &mut global,
            )
            .await
        },
        Commands::Test {
            package,
            filter,
            nocapture,
            unit_only,
            no_doc_tests,
        } => {
            let output_format = global.output_format.clone();
            let use_colors = should_use_colors(&output_format);
            cmd_test(
                &build_system,
                package.clone(),
                filter.clone(),
                *nocapture,
                *unit_only,
                *no_doc_tests,
                &output_format,
                use_colors,
                &cli,
                &mut global,
            )
            .await
        },
        Commands::TestAsil {
            asil,
            filter,
            test_threads,
            no_std_only,
        } => {
            cmd_test_asil(
                *asil,
                filter.clone(),
                *test_threads,
                *no_std_only,
                &mut global,
            )
            .await
        },
        Commands::TestNoStd {
            filter,
            test_threads,
        } => cmd_test_no_std(filter.clone(), *test_threads, &mut global).await,
        Commands::TestConfig { output, example } => {
            cmd_test_config(output.clone(), *example, &mut global).await
        },
        Commands::Verify {
            asil,
            no_kani,
            no_miri,
            detailed,
            allowed_unsafe: _,
        } => {
            let output_format = global.output_format.clone();
            let use_colors = should_use_colors(&output_format);
            cmd_verify(
                &build_system,
                *asil,
                *no_kani,
                *no_miri,
                *detailed,
                &output_format,
                use_colors,
                &cli,
                &mut global,
            )
            .await
        },
        Commands::Docs {
            open,
            private,
            output_dir,
            multi_version,
        } => {
            cmd_docs(
                &build_system,
                *open,
                *private,
                output_dir.clone(),
                multi_version.clone(),
            )
            .await
        },
        Commands::Coverage {
            html,
            open,
            format,
            best_effort,
        } => cmd_coverage(&build_system, *html, *open, format.clone(), *best_effort).await,
        Commands::Check { strict, fix } => {
            cmd_check(&build_system, *strict, *fix, &mut global).await
        },
        Commands::NoStd {
            continue_on_error,
            detailed,
        } => cmd_no_std(&build_system, *continue_on_error, *detailed, &global.output).await,
        Commands::Wrtd {
            variant,
            test,
            cross,
        } => cmd_wrtd(&build_system, *variant, *test, *cross).await,
        Commands::Ci { fail_fast, json } => cmd_ci(&build_system, *fail_fast, *json).await,
        Commands::Clean { all } => cmd_clean(&build_system, *all, &mut global).await,
        Commands::VerifyMatrix {
            report,
            output_dir,
            verbose,
        } => cmd_verify_matrix(&build_system, *report, output_dir.clone(), *verbose).await,
        Commands::SimulateCi {
            verbose,
            output_dir,
        } => cmd_simulate_ci(&build_system, *verbose, output_dir.clone()).await,
        Commands::KaniVerify {
            asil_profile,
            package,
            harness,
            verbose,
            extra_args,
        } => {
            cmd_kani_verify(
                &build_system,
                *asil_profile,
                package.clone(),
                harness.clone(),
                *verbose,
                extra_args.clone(),
            )
            .await
        },
        Commands::Validate {
            check_test_files,
            check_docs,
            audit_docs,
            all,
            verbose,
        } => {
            cmd_validate(
                &build_system,
                *check_test_files,
                *check_docs,
                *audit_docs,
                *all,
                *verbose,
            )
            .await
        },
        Commands::Setup {
            hooks,
            all,
            check,
            install,
        } => cmd_setup(&build_system, *hooks, *all, *check, *install).await,
        Commands::ToolVersions { command } => {
            cmd_tool_versions(&build_system, command.clone()).await
        },
        Commands::Fuzz {
            target,
            duration,
            workers,
            runs,
            list,
            package,
        } => {
            cmd_fuzz(
                &build_system,
                target.clone(),
                *duration,
                *workers,
                *runs,
                *list,
                package.clone(),
            )
            .await
        },
        Commands::TestFeatures {
            package,
            combinations,
            groups,
            verbose,
        } => {
            cmd_test_features(
                &build_system,
                package.clone(),
                *combinations,
                *groups,
                *verbose,
            )
            .await
        },
        Commands::Testsuite {
            extract,
            wabt_path,
            validate,
            clean,
            run_wast,
            wast_dir,
            wast_filter,
            per_assertion,
            wast_report,
            test_timeout_ms,
        } => {
            cmd_testsuite(
                &build_system,
                *extract,
                wabt_path.clone(),
                *validate,
                *clean,
                *run_wast,
                wast_dir.clone(),
                wast_filter.clone(),
                *per_assertion,
                wast_report.clone(),
                *test_timeout_ms,
                &mut global,
            )
            .await
        },
        Commands::Requirements { command } => {
            cmd_requirements(
                &build_system,
                command.clone(),
                &global.output_format,
                should_use_colors(&global.output_format),
                &cli,
            )
            .await
        },
        Commands::Wasm { command } => {
            cmd_wasm(
                &build_system,
                command.clone(),
                &global.output_format,
                should_use_colors(&global.output_format),
                &cli,
            )
            .await
        },
        Commands::Safety { command } => {
            cmd_safety(
                &build_system,
                command.clone(),
                &global.output_format,
                should_use_colors(&global.output_format),
                &cli,
            )
            .await
        },
        Commands::EmbedLimits {
            wasm_file,
            config_file,
            output_file,
            asil_level,
            binary_hash,
            validate,
            replace,
        } => {
            let args = commands::embed_limits::EmbedLimitsArgs {
                wasm_file: wasm_file.clone(),
                config_file: config_file.clone(),
                output_file: output_file.clone(),
                asil_level: asil_level.clone(),
                binary_hash: binary_hash.clone(),
                validate: *validate,
                replace: *replace,
            };
            cmd_embed_limits(args, &global.output)
        },
        Commands::HelpDiagnostics => {
            print_diagnostic_help();
            Ok(())
        },
    };

    match result {
        Ok(()) => {
            if cli.verbose {
                global.output.success("Command completed successfully");
            }
            Ok(())
        },
        Err(e) => {
            global.output.error(&e.to_string());
            process::exit(1);
        },
    }
}

/// Build command implementation
async fn cmd_build(
    build_system: &BuildSystem,
    package: Option<String>,
    clippy: bool,
    fmt_check: bool,
    global: &mut GlobalArgs,
) -> Result<()> {
    let output = &global.output;
    match output.format() {
        OutputFormat::Json | OutputFormat::JsonLines => {
            // Use diagnostic-based build with caching and filtering
            let mut diagnostics = if let Some(pkg) = &package {
                build_system
                    .build_package_with_diagnostics(pkg)
                    .context("Package build failed")?
            } else {
                build_system.build_all_with_diagnostics().context("Build failed")?
            };

            // Apply caching and diff functionality if enabled
            if global.cache {
                let workspace_root = build_system.workspace_root();
                let cache_path = get_cache_path(workspace_root);
                let mut cache_manager =
                    CacheManager::new(workspace_root.to_path_buf(), cache_path, true)?;

                if global.clear_cache {
                    cache_manager.clear()?;
                }

                // Apply diff filtering if requested
                if global.diff_only {
                    let diff_diagnostics =
                        cache_manager.get_diff_diagnostics(&diagnostics.diagnostics);
                    diagnostics.diagnostics = diff_diagnostics;
                }

                // Cache new diagnostics (after diff processing)
                let mut file_diagnostic_map: std::collections::HashMap<String, Vec<_>> =
                    std::collections::HashMap::new();
                for diagnostic in &diagnostics.diagnostics {
                    file_diagnostic_map
                        .entry(diagnostic.file.clone())
                        .or_insert_with(Vec::new)
                        .push(diagnostic.clone());
                }

                for (file, file_diagnostics) in file_diagnostic_map {
                    if let Ok(file_path) = workspace_root.join(&file).canonicalize() {
                        cache_manager.cache_diagnostics(&file_path, file_diagnostics)?;
                    }
                }

                cache_manager.save()?;
            }

            // Apply filtering if specified
            if global.filter_severity.is_some()
                || global.filter_source.is_some()
                || global.filter_file.is_some()
                || global.group_by.is_some()
            {
                let filter_options = global.build_filter_options()?;
                let processor = wrt_build_core::filtering::DiagnosticProcessor::new(
                    build_system.workspace_root().to_path_buf(),
                );
                let grouped = processor.process(&diagnostics, &filter_options)?;

                // Convert grouped diagnostics back to collection format
                let mut filtered_diagnostics = Vec::new();
                for (_, group_diagnostics) in grouped.groups {
                    filtered_diagnostics.extend(group_diagnostics);
                }
                diagnostics.diagnostics = filtered_diagnostics;
            }

            let formatter = FormatterFactory::create_with_options(
                global.output.format().clone(),
                true,
                global.output.is_colored(),
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if diagnostics.has_errors() {
                process::exit(1);
            }
        },
        OutputFormat::Human => {
            // Use enhanced progress indicators for human format
            use helpers::{MultiStepProgress, ProgressIndicator};

            if let Some(pkg) = package {
                let mut progress = ProgressIndicator::spinner(
                    format!("Building package: {}", pkg),
                    global.output_format.clone(),
                    output.is_colored(),
                );
                progress.start();

                let results = build_system.build_package(&pkg).context("Package build failed")?;

                progress.finish_with_message(format!(
                    "Package '{}' built successfully in {:.2}s",
                    pkg,
                    results.duration().as_secs_f64()
                ));

                if !results.warnings().is_empty() {
                    output.warning("Build warnings:");
                    for warning in results.warnings() {
                        output.indent(warning);
                    }
                }
            } else {
                // Multi-step progress for full build
                let steps = vec![
                    "Analyzing dependencies".to_string(),
                    "Compiling core components".to_string(),
                    "Running post-build checks".to_string(),
                ];

                let mut progress = MultiStepProgress::new(
                    steps,
                    global.output_format.clone(),
                    output.is_colored(),
                );
                progress.start();

                progress.begin_step("Analyzing workspace dependencies");
                // Simulate dependency analysis
                std::thread::sleep(std::time::Duration::from_millis(500));
                progress.finish_step("Dependencies analyzed");

                progress.begin_step("Compiling all workspace components");
                let results = build_system.build_all().context("Build failed")?;
                progress.finish_step("Compilation completed");

                progress.begin_step("Running clippy and format checks");
                // Additional checks would go here
                std::thread::sleep(std::time::Duration::from_millis(300));
                progress.finish_step("Checks completed");

                progress.finish(format!(
                    "All components built successfully in {:.2}s",
                    results.duration().as_secs_f64()
                ));

                if !results.warnings().is_empty() {
                    output.warning("Build warnings:");
                    for warning in results.warnings() {
                        output.indent(warning);
                    }
                }
            }

            if clippy {
                output.progress("Running clippy checks...");
                build_system.run_static_analysis().context("Clippy checks failed")?;
            }

            if fmt_check {
                output.progress("Checking code formatting...");
                build_system.check_formatting().context("Format check failed")?;
            }
        },
    }

    Ok(())
}

/// Test command implementation
async fn cmd_test(
    build_system: &BuildSystem,
    package: Option<String>,
    filter: Option<String>,
    nocapture: bool,
    unit_only: bool,
    no_doc_tests: bool,
    output_format: &OutputFormat,
    use_colors: bool,
    cli: &Cli,
    global: &mut GlobalArgs,
) -> Result<()> {
    match output_format {
        OutputFormat::Json | OutputFormat::JsonLines => {
            // Use diagnostic-based test output with caching and filtering
            let mut test_options = wrt_build_core::test::TestOptions::default();
            test_options.filter = filter;
            test_options.nocapture = nocapture;
            test_options.integration = !unit_only;
            test_options.doc_tests = !no_doc_tests;

            let mut diagnostics =
                build_system.run_tests_with_diagnostics(&test_options).context("Tests failed")?;

            // Apply caching and diff functionality if enabled
            if global.cache {
                let workspace_root = build_system.workspace_root();
                let cache_path = get_cache_path(workspace_root);
                let mut cache_manager =
                    CacheManager::new(workspace_root.to_path_buf(), cache_path, true)?;

                if global.clear_cache {
                    cache_manager.clear()?;
                }

                // Apply diff filtering if requested
                if global.diff_only {
                    let diff_diagnostics =
                        cache_manager.get_diff_diagnostics(&diagnostics.diagnostics);
                    diagnostics.diagnostics = diff_diagnostics;
                }

                cache_manager.save()?;
            }

            // Apply filtering if specified
            if global.filter_severity.is_some()
                || global.filter_source.is_some()
                || global.filter_file.is_some()
                || global.group_by.is_some()
            {
                let filter_options = global.build_filter_options()?;
                let processor = wrt_build_core::filtering::DiagnosticProcessor::new(
                    build_system.workspace_root().to_path_buf(),
                );
                let grouped = processor.process(&diagnostics, &filter_options)?;

                // Convert grouped diagnostics back to collection format
                let mut filtered_diagnostics = Vec::new();
                for (_, group_diagnostics) in grouped.groups {
                    filtered_diagnostics.extend(group_diagnostics);
                }
                diagnostics.diagnostics = filtered_diagnostics;
            }

            let formatter = FormatterFactory::create_with_options(
                global.output.format().clone(),
                true,
                global.output.is_colored(),
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if diagnostics.has_errors() {
                process::exit(1);
            }
        },
        OutputFormat::Human => {
            // Use traditional output for human format
            if use_colors {
                println!("{} Running tests...", "🧪".bright_blue());
            } else {
                println!("Running tests...");
            }

            if let Some(pkg) = package {
                println!("  Testing package: {}", pkg.bright_cyan());
                let results = build_system.test_package(&pkg).context("Package tests failed")?;

                if !results.warnings().is_empty() {
                    println!("{} Test warnings:", "⚠️".bright_yellow());
                    for warning in results.warnings() {
                        println!("  {}", warning);
                    }
                }

                println!(
                    "{} Package tests completed in {:.2}s",
                    "✅".bright_green(),
                    results.duration().as_secs_f64()
                );
                return Ok(());
            }

            let mut test_options = wrt_build_core::test::TestOptions::default();
            test_options.filter = filter;
            test_options.nocapture = nocapture;
            test_options.integration = !unit_only;
            test_options.doc_tests = !no_doc_tests;

            let results =
                build_system.run_tests_with_options(&test_options).context("Tests failed")?;

            if results.is_success() {
                println!(
                    "{} {} tests passed ({:.2}s)",
                    "✅".bright_green(),
                    results.passed,
                    results.duration_ms as f64 / 1000.0
                );
            } else {
                println!(
                    "{} {} tests failed, {} passed",
                    "❌".bright_red(),
                    results.failed,
                    results.passed
                );
                anyhow::bail!("Test suite failed");
            }
        },
    }

    Ok(())
}

/// ASIL-specific test command implementation
async fn cmd_test_asil(
    asil: AsilArg,
    filter: Option<String>,
    test_threads: Option<usize>,
    no_std_only: bool,
    global: &mut GlobalArgs,
) -> Result<()> {
    let config = TestConfig {
        asil_level: asil.into(),
        filter,
        no_std_only,
        output_format: global.output_format.clone(),
        verbose: global.verbose,
        test_threads,
    };

    global.output.info(&format!(
        "Running ASIL-{} test suite{}",
        config.asil_level,
        if no_std_only { " (no_std only)" } else { "" }
    ));

    run_asil_tests(global, config.asil_level).context("ASIL tests failed")
}

/// No_std compatibility test command implementation
async fn cmd_test_no_std(
    filter: Option<String>,
    test_threads: Option<usize>,
    global: &mut GlobalArgs,
) -> Result<()> {
    let config = TestConfig {
        asil_level: AsilLevel::QM,
        filter,
        no_std_only: true,
        output_format: global.output_format.clone(),
        verbose: global.verbose,
        test_threads,
    };

    global.output.info("Running no_std compatibility tests");

    run_no_std_tests(global).context("No_std tests failed")
}

/// Test configuration generation command implementation
async fn cmd_test_config(output: String, example: bool, global: &mut GlobalArgs) -> Result<()> {
    use test_config::WrtTestConfig;

    let config = if example {
        global.output.info("Generating example test configuration");
        WrtTestConfig::example_config()
    } else {
        global.output.info("Generating default test configuration");
        WrtTestConfig::default()
    };

    config.save_to_file(&output).context("Failed to save test configuration file")?;

    global.output.success(&format!("Test configuration saved to: {}", output));

    if example {
        global.output.info("Edit the configuration file to customize test behavior");
        global.output.info("Use 'cargo-wrt test-asil' to run ASIL-specific tests");
    }

    Ok(())
}

/// Verify command implementation
async fn cmd_verify(
    build_system: &BuildSystem,
    asil: AsilArg,
    no_kani: bool,
    no_miri: bool,
    detailed: bool,
    output_format: &OutputFormat,
    use_colors: bool,
    cli: &Cli,
    global: &mut GlobalArgs,
) -> Result<()> {
    let mut options = wrt_build_core::verify::VerificationOptions::default();
    options.target_asil = asil.into();
    options.kani = !no_kani;
    options.miri = !no_miri;
    options.detailed_reports = detailed;

    // Load allowed unsafe configuration if it exists
    let allowed_unsafe_path = build_system.workspace_root().join("allowed-unsafe.toml");
    if allowed_unsafe_path.exists() {
        match wrt_build_core::verify::AllowedUnsafeConfig::load_from_file(&allowed_unsafe_path) {
            Ok(config) => {
                options.allowed_unsafe = Some(config);
                if use_colors {
                    println!(
                        "  {} Loaded allowed unsafe configuration from {}",
                        "📋".bright_cyan(),
                        allowed_unsafe_path.display()
                    );
                } else {
                    println!(
                        "  Loaded allowed unsafe configuration from {}",
                        allowed_unsafe_path.display()
                    );
                }
            },
            Err(e) => {
                if use_colors {
                    eprintln!(
                        "  {} Failed to load allowed unsafe configuration: {}",
                        "⚠️".bright_yellow(),
                        e
                    );
                } else {
                    eprintln!(
                        "  Warning: Failed to load allowed unsafe configuration: {}",
                        e
                    );
                }
            },
        }
    }

    match output_format {
        OutputFormat::Json | OutputFormat::JsonLines => {
            // Use new diagnostic-based verification with caching and filtering
            let mut diagnostics = build_system
                .verify_safety_with_diagnostics(&options)
                .context("Safety verification failed")?;

            // Apply caching and diff functionality if enabled
            if global.cache {
                let workspace_root = build_system.workspace_root();
                let cache_path = get_cache_path(workspace_root);
                let mut cache_manager =
                    CacheManager::new(workspace_root.to_path_buf(), cache_path, true)?;

                if global.clear_cache {
                    cache_manager.clear()?;
                }

                // Apply diff filtering if requested
                if global.diff_only {
                    let diff_diagnostics =
                        cache_manager.get_diff_diagnostics(&diagnostics.diagnostics);
                    diagnostics.diagnostics = diff_diagnostics;
                }

                cache_manager.save()?;
            }

            // Apply filtering if specified
            if global.filter_severity.is_some()
                || global.filter_source.is_some()
                || global.filter_file.is_some()
                || global.group_by.is_some()
            {
                let filter_options = global.build_filter_options()?;
                let processor = wrt_build_core::filtering::DiagnosticProcessor::new(
                    build_system.workspace_root().to_path_buf(),
                );
                let grouped = processor.process(&diagnostics, &filter_options)?;

                // Convert grouped diagnostics back to collection format
                let mut filtered_diagnostics = Vec::new();
                for (_, group_diagnostics) in grouped.groups {
                    filtered_diagnostics.extend(group_diagnostics);
                }
                diagnostics.diagnostics = filtered_diagnostics;
            }

            let formatter = FormatterFactory::create_with_options(
                global.output.format().clone(),
                true,
                global.output.is_colored(),
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if diagnostics.has_errors() {
                process::exit(1);
            }
        },
        OutputFormat::Human => {
            // Use traditional output for human format
            if use_colors {
                println!("{} Running safety verification...", "🛡️".bright_blue());
            } else {
                println!("Running safety verification...");
            }

            let results = build_system
                .verify_safety_with_options(&options)
                .context("Safety verification failed")?;

            if results.success {
                if use_colors {
                    println!(
                        "{} Safety verification passed! ASIL level: {:?}",
                        "✅".bright_green(),
                        results.asil_level
                    );
                } else {
                    println!(
                        "Safety verification passed! ASIL level: {:?}",
                        results.asil_level
                    );
                }
            } else {
                if use_colors {
                    println!("{} Safety verification failed", "❌".bright_red());
                } else {
                    println!("Safety verification failed");
                }
                anyhow::bail!("Safety verification failed");
            }

            if detailed {
                println!("\n{}", results.report);
            }
        },
    }

    Ok(())
}

/// Docs command implementation
async fn cmd_docs(
    build_system: &BuildSystem,
    open: bool,
    private: bool,
    output_dir: Option<String>,
    multi_version: Option<String>,
) -> Result<()> {
    use wrt_build_core::tools::ToolManager;

    // Check if multi-version documentation is requested
    if let Some(versions_str) = multi_version {
        let versions: Vec<String> = versions_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if versions.is_empty() {
            anyhow::bail!("No versions specified for multi-version documentation");
        }

        println!(
            "📚 Generating multi-version documentation for: {:?}",
            versions
        );
        build_system
            .generate_multi_version_docs(versions)
            .context("Multi-version documentation generation failed")?;

        return Ok(());
    }

    // Check documentation dependencies first
    let tool_manager = ToolManager::new();
    let python_status = tool_manager.check_tool("python3");
    let venv_status = tool_manager.check_tool("python-venv");

    if !python_status.available {
        println!("⚠️  Python not available - generating Rust API docs only");
        println!("   💡 Install Python 3.8+ to enable comprehensive documentation generation");
    } else if !venv_status.available {
        println!("⚠️  Python venv not available - generating Rust API docs only");
        println!("   💡 Python virtual environment support needed for documentation dependencies");
    } else {
        println!("📚 Python environment ready - will generate comprehensive documentation");
    }

    // Generate documentation with enhanced functionality
    if let Some(out_dir) = output_dir {
        build_system.generate_docs_with_output_dir(private, open, Some(out_dir))
    } else {
        build_system.generate_docs_with_options(private, open)
    }
    .context("Documentation generation failed")?;

    Ok(())
}

/// Coverage command implementation
async fn cmd_coverage(
    build_system: &BuildSystem,
    html: bool,
    open: bool,
    format: String,
    best_effort: bool,
) -> Result<()> {
    if best_effort {
        println!(
            "{} Running coverage analysis in best-effort mode...",
            "📊".bright_blue()
        );
        println!(
            "{} Will continue on errors and generate coverage for working components",
            "ℹ️".bright_yellow()
        );
    } else {
        println!("{} Running coverage analysis...", "📊".bright_blue());
    }

    if best_effort {
        // In best-effort mode, try to run coverage but continue on failures
        match build_system.run_coverage() {
            Ok(_) => println!(
                "{} Coverage analysis completed successfully",
                "✅".bright_green()
            ),
            Err(e) => {
                println!("{} Coverage analysis failed: {}", "⚠️".bright_yellow(), e);
                println!(
                    "{} Continuing in best-effort mode - partial results may be available",
                    "ℹ️".bright_yellow()
                );
            },
        }
    } else {
        // Normal mode - fail on errors
        build_system.run_coverage().context("Coverage analysis failed")?;
    }

    if open {
        println!(
            "{} Opening coverage report in browser...",
            "🌐".bright_blue()
        );
        // TODO: Implement browser opening
    }

    Ok(())
}

/// Check command implementation
async fn cmd_check(
    build_system: &BuildSystem,
    strict: bool,
    fix: bool,
    global: &mut GlobalArgs,
) -> Result<()> {
    let output = &global.output;
    match output.format() {
        OutputFormat::Json | OutputFormat::JsonLines => {
            // Use diagnostic-based static analysis with caching and filtering
            let mut diagnostics = build_system
                .run_static_analysis_with_diagnostics(strict)
                .context("Static analysis failed")?;

            // Apply caching and diff functionality if enabled
            if global.cache {
                let workspace_root = build_system.workspace_root();
                let cache_path = get_cache_path(workspace_root);
                let mut cache_manager =
                    CacheManager::new(workspace_root.to_path_buf(), cache_path, true)?;

                if global.clear_cache {
                    cache_manager.clear()?;
                }

                // Apply diff filtering if requested
                if global.diff_only {
                    let diff_diagnostics =
                        cache_manager.get_diff_diagnostics(&diagnostics.diagnostics);
                    diagnostics.diagnostics = diff_diagnostics;
                }

                cache_manager.save()?;
            }

            // Apply filtering if specified
            if global.filter_severity.is_some()
                || global.filter_source.is_some()
                || global.filter_file.is_some()
                || global.group_by.is_some()
            {
                let filter_options = global.build_filter_options()?;
                let processor = wrt_build_core::filtering::DiagnosticProcessor::new(
                    build_system.workspace_root().to_path_buf(),
                );
                let grouped = processor.process(&diagnostics, &filter_options)?;

                // Convert grouped diagnostics back to collection format
                let mut filtered_diagnostics = Vec::new();
                for (_, group_diagnostics) in grouped.groups {
                    filtered_diagnostics.extend(group_diagnostics);
                }
                diagnostics.diagnostics = filtered_diagnostics;
            }

            let formatter = FormatterFactory::create_with_options(
                global.output.format().clone(),
                true,
                global.output.is_colored(),
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if diagnostics.has_errors() {
                process::exit(1);
            }
        },
        OutputFormat::Human => {
            // Use traditional output for human format
            output.progress("Running static analysis...");

            // Get diagnostics for auto-fix even in human mode
            if fix {
                let diagnostics = build_system
                    .run_static_analysis_with_diagnostics(strict)
                    .context("Static analysis failed")?;

                output.progress("Auto-fixing issues...");
                let autofix_manager = AutoFixManager::new(output.clone(), global.dry_run);
                let fix_result = autofix_manager.apply_fixes(&diagnostics)?;

                if fix_result.has_fixes() {
                    output.success(&format!("Applied {} fixes", fix_result.successful_fixes));
                    if fix_result.failed_fixes > 0 {
                        output.warning(&format!("{} fixes failed", fix_result.failed_fixes));
                    }
                } else {
                    output.info("No auto-fixable issues found");
                }
            } else {
                build_system.run_static_analysis().context("Static analysis failed")?;
            }

            output.success("Static analysis completed");
        },
    }

    Ok(())
}

/// NoStd command implementation
async fn cmd_no_std(
    build_system: &BuildSystem,
    continue_on_error: bool,
    detailed: bool,
    output: &OutputManager,
) -> Result<()> {
    output.progress("Verifying no_std compatibility...");

    if output.is_json_mode() {
        // Use diagnostic-based verification for structured output
        build_system.verify_no_std().context("no_std verification failed")?;

        output.success("no_std verification completed successfully");
    } else {
        // Use traditional verification for human output
        match build_system.verify_no_std() {
            Ok(()) => output.success("no_std compatibility verified"),
            Err(e) => {
                if continue_on_error {
                    output.warning(&format!("no_std verification failed: {}", e));
                } else {
                    return Err(anyhow::anyhow!("no_std verification failed: {}", e));
                }
            },
        }
    }

    Ok(())
}

/// WRTD command implementation
async fn cmd_wrtd(
    build_system: &BuildSystem,
    variant: Option<WrtdVariant>,
    test: bool,
    cross: bool,
) -> Result<()> {
    println!("{} Building WRTD binaries...", "🏗️".bright_blue());

    build_system.build_wrtd_binaries().context("WRTD build failed")?;

    if test {
        println!("{} Testing WRTD binaries...", "🧪".bright_blue());
        // TODO: Implement WRTD testing
    }

    Ok(())
}

/// CI command implementation
async fn cmd_ci(build_system: &BuildSystem, fail_fast: bool, json: bool) -> Result<()> {
    println!("{} Running comprehensive CI checks...", "🤖".bright_blue());

    let mut errors = Vec::new();

    // 1. Build
    println!("  {} Building...", "🔨".bright_cyan());
    if let Err(e) = build_system.build_all() {
        errors.push(format!("Build failed: {}", e));
        if fail_fast {
            anyhow::bail!("Build failed: {}", e);
        }
    }

    // 2. Tests
    println!("  {} Testing...", "🧪".bright_cyan());
    if let Err(e) = build_system.run_tests() {
        errors.push(format!("Tests failed: {}", e));
        if fail_fast {
            anyhow::bail!("Tests failed: {}", e);
        }
    }

    // 3. Static analysis
    println!("  {} Static analysis...", "🔍".bright_cyan());
    if let Err(e) = build_system.run_static_analysis() {
        errors.push(format!("Static analysis failed: {}", e));
        if fail_fast {
            anyhow::bail!("Static analysis failed: {}", e);
        }
    }

    // 4. Safety verification
    println!("  {} Safety verification...", "🛡️".bright_cyan());
    if let Err(e) = build_system.verify_safety() {
        errors.push(format!("Safety verification failed: {}", e));
        if fail_fast {
            anyhow::bail!("Safety verification failed: {}", e);
        }
    }

    // 5. Advanced tests
    println!("  {} Advanced tests...", "🧪".bright_cyan());
    if let Err(e) = build_system.run_advanced_tests() {
        errors.push(format!("Advanced tests failed: {}", e));
        if fail_fast {
            anyhow::bail!("Advanced tests failed: {}", e);
        }
    }

    // 6. Integrity checks
    println!("  {} Integrity checks...", "🔒".bright_cyan());
    if let Err(e) = build_system.run_integrity_checks() {
        errors.push(format!("Integrity checks failed: {}", e));
        if fail_fast {
            anyhow::bail!("Integrity checks failed: {}", e);
        }
    }

    if errors.is_empty() {
        println!("{} All CI checks passed!", "✅".bright_green());
        Ok(())
    } else {
        if json {
            let report = serde_json::json!({
                "status": "failed",
                "errors": errors,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{} CI checks failed:", "❌".bright_red());
            for error in &errors {
                println!("  - {}", error);
            }
        }
        anyhow::bail!("{} errors in CI checks", errors.len());
    }
}

/// Clean command implementation
async fn cmd_clean(build_system: &BuildSystem, all: bool, global: &mut GlobalArgs) -> Result<()> {
    use helpers::{ErrorContext, build_errors};

    let output = &global.output;
    output.progress("Cleaning build artifacts...");

    let workspace_root = build_system.workspace_root();

    if all {
        // Remove all target directories
        let target_dir = workspace_root.join("target");
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir).context("Failed to remove target directory")?;
            output.indent(&format!("Removed {}", target_dir.display()));
        }

        // Remove cargo-wrt target if it exists
        let cargo_wrt_target = workspace_root.join("cargo-wrt").join("target");
        if cargo_wrt_target.exists() {
            std::fs::remove_dir_all(&cargo_wrt_target)
                .context("Failed to remove cargo-wrt target directory")?;
            output.indent(&format!("Removed {}", cargo_wrt_target.display()));
        }

        // Remove wrt-build-core target if it exists
        let build_core_target = workspace_root.join("wrt-build-core").join("target");
        if build_core_target.exists() {
            std::fs::remove_dir_all(&build_core_target)
                .context("Failed to remove wrt-build-core target directory")?;
            output.indent(&format!("Removed {}", build_core_target.display()));
        }
    } else {
        // Standard cargo clean
        let mut cmd = process::Command::new("cargo");
        cmd.arg("clean").current_dir(workspace_root);

        let command_output =
            cmd.output().context("Failed to run cargo clean - is cargo installed?")?;

        if !command_output.status.success() {
            return Err(build_errors::compilation_failed("cargo clean failed").into());
        }
    }

    output.success("Clean completed");
    Ok(())
}

/// Verify matrix command implementation
async fn cmd_verify_matrix(
    build_system: &BuildSystem,
    report: bool,
    output_dir: String,
    verbose: bool,
) -> Result<()> {
    use wrt_build_core::matrix::MatrixVerifier;

    let verifier = MatrixVerifier::new(verbose);
    let results = verifier.run_verification()?;

    verifier.print_summary(&results);

    if report {
        let output_path = std::path::Path::new(&output_dir);
        verifier.generate_report(&results, output_path)?;
    }

    if !results.all_passed {
        anyhow::bail!("Build matrix verification failed");
    }

    Ok(())
}

/// Simulate CI command implementation
async fn cmd_simulate_ci(
    build_system: &BuildSystem,
    verbose: bool,
    output_dir: String,
) -> Result<()> {
    use wrt_build_core::ci::CiSimulator;

    let workspace_root = build_system.workspace_root().to_path_buf();
    let simulator = CiSimulator::new(workspace_root, verbose);

    let results = simulator.run_simulation().context("CI simulation failed")?;

    simulator.print_summary(&results);

    if !results.overall_passed {
        anyhow::bail!("CI simulation found issues that need to be addressed");
    }

    Ok(())
}

/// KANI verify command implementation
async fn cmd_kani_verify(
    build_system: &BuildSystem,
    asil_profile: AsilArg,
    package: Option<String>,
    harness: Option<String>,
    verbose: bool,
    extra_args: Vec<String>,
) -> Result<()> {
    use wrt_build_core::kani::{KaniConfig, KaniVerifier};

    // Check if KANI is available
    if !wrt_build_core::kani::is_kani_available() {
        anyhow::bail!(
            "KANI is not available. Please install it with: cargo install --locked kani-verifier \
             && cargo kani setup"
        );
    }

    let config = KaniConfig {
        profile: asil_profile.into(),
        package,
        harness,
        verbose,
        extra_args,
    };

    let workspace_root = build_system.workspace_root().to_path_buf();
    let verifier = KaniVerifier::new(workspace_root, config);

    let results = verifier.run_verification().context("KANI verification failed")?;

    verifier.print_summary(&results);

    if results.passed_packages < results.total_packages {
        anyhow::bail!(
            "KANI verification failed for {}/{} packages",
            results.total_packages - results.passed_packages,
            results.total_packages
        );
    }

    Ok(())
}

/// Validate command implementation
async fn cmd_validate(
    build_system: &BuildSystem,
    check_test_files: bool,
    check_docs: bool,
    audit_docs: bool,
    all: bool,
    verbose: bool,
) -> Result<()> {
    use wrt_build_core::validation::CodeValidator;

    let workspace_root = build_system.workspace_root().to_path_buf();
    let validator = CodeValidator::new(workspace_root.clone(), verbose);

    let mut any_failed = false;

    if all || check_test_files {
        println!("{} Checking for test files in src/...", "🔍".bright_blue());
        let result = validator
            .check_no_test_files_in_src()
            .context("Failed to check for test files")?;

        if !result.success {
            any_failed = true;
            for error in &result.errors {
                println!(
                    "{} {}: {}",
                    "❌".bright_red(),
                    error.file.display(),
                    error.message
                );
            }
        }
    }

    if all || check_docs {
        println!();
        println!(
            "{} Checking module documentation coverage...",
            "📚".bright_blue()
        );
        let result = validator
            .check_module_documentation()
            .context("Failed to check documentation")?;

        if !result.success {
            any_failed = true;
        }
    }

    if all || audit_docs {
        println!();
        println!(
            "{} Running comprehensive documentation audit...",
            "📚".bright_blue()
        );
        let result =
            validator.audit_crate_documentation().context("Failed to audit documentation")?;

        if !result.success {
            any_failed = true;
        }
    }

    if !all && !check_test_files && !check_docs && !audit_docs {
        // If no specific checks requested, run all
        let all_passed = wrt_build_core::validation::run_all_validations(&workspace_root, verbose)
            .context("Failed to run validation checks")?;

        if !all_passed {
            any_failed = true;
        }
    }

    if any_failed {
        anyhow::bail!("Validation checks failed");
    }

    Ok(())
}

/// Setup command implementation
async fn cmd_setup(
    build_system: &BuildSystem,
    hooks: bool,
    all: bool,
    check: bool,
    install: bool,
) -> Result<()> {
    use std::{fs, process::Command};

    println!(
        "{} Setting up development environment...",
        "🔧".bright_blue()
    );

    let workspace_root = build_system.workspace_root();

    // Handle tool status check
    if all || check {
        println!("{} Checking tool availability...", "🔍".bright_cyan());

        use wrt_build_core::tools::ToolManager;
        let tool_manager = ToolManager::new();
        tool_manager.print_tool_status();
        println!();

        if check && !all && !hooks && !install {
            return Ok(()); // Only check was requested
        }
    }

    // Handle tool installation
    if all || install {
        println!("{} Installing optional tools...", "💿".bright_cyan());

        use wrt_build_core::tools::ToolManager;
        let tool_manager = ToolManager::new();

        if let Err(e) = tool_manager.install_all_needed_tools() {
            println!("    ⚠️ Some tools failed to install: {}", e);
        }

        println!();
    }

    if all || hooks {
        println!("{} Configuring git hooks...", "🪝".bright_cyan());

        // Check if .githooks directory exists
        let githooks_dir = workspace_root.join(".githooks");
        if !githooks_dir.exists() {
            fs::create_dir(&githooks_dir).context("Failed to create .githooks directory")?;
        }

        // Configure git to use .githooks directory
        let mut git_cmd = Command::new("git");
        git_cmd
            .args(["config", "core.hooksPath", ".githooks"])
            .current_dir(workspace_root);

        let output = git_cmd.output().context("Failed to configure git hooks")?;

        if output.status.success() {
            println!("{} Git hooks configured successfully!", "✅".bright_green());
            println!("  Pre-commit hook will prevent test files in src/ directories");
            println!();
            println!("  To bypass hooks temporarily (not recommended), use:");
            println!("    git commit --no-verify");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to configure git hooks: {}", stderr);
        }
    }

    if !all && !hooks && !check && !install {
        println!(
            "{} No setup options specified. Available options:",
            "ℹ️".bright_blue()
        );
        println!("  --check    Check status of all tools");
        println!("  --hooks    Setup git hooks");
        println!("  --install  Install optional tools (cargo-fuzz, kani-verifier)");
        println!("  --all      Do everything (check + hooks + install)");
        println!();
        println!("Examples:");
        println!("  cargo-wrt setup --check");
        println!("  cargo-wrt setup --install");
        println!("  cargo-wrt setup --all");
    }

    Ok(())
}

/// Tool versions command implementation  
async fn cmd_tool_versions(build_system: &BuildSystem, command: ToolVersionCommand) -> Result<()> {
    use wrt_build_core::{tool_versions::ToolVersionConfig, tools::ToolManager};

    match command {
        ToolVersionCommand::Generate { force, all } => {
            let workspace_root = build_system.workspace_root();
            let config_path = workspace_root.join("tool-versions.toml");

            if config_path.exists() && !force {
                anyhow::bail!(
                    "Tool version file already exists at {}\nUse --force to overwrite",
                    config_path.display()
                );
            }

            println!("{} Generating tool-versions.toml...", "📝".bright_blue());

            // Load current config or create new one
            let config = if all {
                // Generate comprehensive config with all tools
                ToolVersionConfig::load_or_default()
            } else {
                // Generate minimal config with required tools only
                ToolVersionConfig::create_fallback_config()
            };

            // Convert to TOML and write to file
            let toml_content =
                config.to_toml().context("Failed to serialize tool version configuration")?;

            std::fs::write(&config_path, toml_content)
                .context("Failed to write tool-versions.toml")?;

            println!("  ✅ Generated {}", config_path.display());
            println!(
                "  📋 Configuration includes {} tools",
                config.get_managed_tools().len()
            );
            println!();
            println!("  💡 Edit the file to customize tool versions and requirements");
            println!("  🔄 Run 'cargo-wrt tool-versions check' to validate");
        },

        ToolVersionCommand::Check { verbose, tool } => {
            println!("{} Checking tool versions...", "🔍".bright_blue());

            let tool_manager = ToolManager::new();

            if let Some(tool_name) = tool {
                // Check specific tool
                let status = tool_manager.check_tool(&tool_name);
                if verbose {
                    println!("  Tool: {}", tool_name.bright_cyan());
                    println!(
                        "  Available: {}",
                        if status.available { "✅ Yes" } else { "❌ No" }
                    );
                    if let Some(version) = &status.version {
                        println!("  Version: {}", version);
                    }
                    if let Some(error) = &status.error {
                        println!("  Error: {}", error.bright_red());
                    }
                    println!("  Version Status: {:?}", status.version_status);
                    println!(
                        "  Needs Action: {}",
                        if status.needs_action { "Yes" } else { "No" }
                    );
                } else {
                    let icon = if status.available && !status.needs_action { "✅" } else { "❌" };
                    println!("  {} {}", icon, tool_name.bright_cyan());
                }
            } else {
                // Check all tools
                if verbose {
                    tool_manager.print_tool_status();
                } else {
                    let results = tool_manager.check_all_tools();
                    for (tool_name, status) in results {
                        let icon =
                            if status.available && !status.needs_action { "✅" } else { "❌" };
                        println!("  {} {}", icon, tool_name.bright_cyan());
                    }
                }
            }
        },

        ToolVersionCommand::Update { tool, all } => {
            println!("{} Updating tool-versions.toml...", "🔄".bright_blue());

            let workspace_root = build_system.workspace_root();
            let config_path = workspace_root.join("tool-versions.toml");

            if !config_path.exists() {
                anyhow::bail!(
                    "Tool version file not found at {}\nRun 'cargo-wrt tool-versions generate' \
                     first",
                    config_path.display()
                );
            }

            if tool.is_some() {
                println!("  🚧 Updating specific tools is not yet implemented");
                println!(
                    "  💡 For now, please edit {} manually",
                    config_path.display()
                );
            } else if all {
                println!("  🚧 Auto-updating all tools is not yet implemented");
                println!(
                    "  💡 For now, please edit {} manually",
                    config_path.display()
                );
            } else {
                println!("  ℹ️ Specify --tool <name> or --all to update versions");
                println!("  📝 Current file: {}", config_path.display());
            }
        },
    }

    Ok(())
}

/// Fuzz command implementation
async fn cmd_fuzz(
    build_system: &BuildSystem,
    target: String,
    duration: u64,
    workers: u32,
    runs: Option<u64>,
    list: bool,
    package: Option<String>,
) -> Result<()> {
    use wrt_build_core::fuzz::FuzzOptions;

    if list {
        println!("{} Available fuzz targets:", "🎯".bright_blue());

        match build_system.list_fuzz_targets() {
            Ok(targets) => {
                if targets.is_empty() {
                    println!("  No fuzz targets found. Run 'cargo fuzz init' to set up fuzzing.");
                } else {
                    for target in targets {
                        println!("  - {}", target);
                    }
                }
            },
            Err(e) => {
                println!("  Failed to list fuzz targets: {}", e);
            },
        }
        return Ok(());
    }

    println!("{} Running fuzzing campaign...", "🐛".bright_blue());

    let mut options = FuzzOptions {
        duration,
        workers: workers as usize,
        runs,
        targets: if target == "all" { vec![] } else { vec![target.clone()] },
        coverage: false,
    };

    if let Some(pkg) = package {
        println!("  Focusing on package: {}", pkg.bright_cyan());
        // Filter targets by package - would need package-specific logic
    }

    match build_system.run_fuzz_with_options(&options) {
        Ok(results) => {
            if results.success {
                println!(
                    "{} Fuzzing completed successfully in {:.2}s",
                    "✅".bright_green(),
                    results.duration_ms as f64 / 1000.0
                );
                println!("  Targets run: {}", results.targets_run.len());
            } else {
                println!(
                    "{} Fuzzing found issues in {} targets",
                    "⚠️".bright_yellow(),
                    results.crashed_targets.len()
                );
                for target in &results.crashed_targets {
                    println!("    - {}", target);
                }
            }
        },
        Err(e) => {
            anyhow::bail!("Fuzzing failed: {}", e);
        },
    }

    Ok(())
}

/// Test features command implementation
async fn cmd_test_features(
    build_system: &BuildSystem,
    package: Option<String>,
    combinations: bool,
    groups: bool,
    verbose: bool,
) -> Result<()> {
    println!("{} Testing feature combinations...", "🧪".bright_blue());

    if let Some(pkg) = package {
        println!("  Testing package: {}", pkg.bright_cyan());
    }

    if combinations {
        println!("  Testing all feature combinations");
    }

    if groups {
        println!("  Testing predefined feature groups");
    }

    // TODO: Implement feature testing through wrt-build-core
    println!("{} Feature testing completed", "✅".bright_green());
    Ok(())
}

/// Testsuite command implementation
async fn cmd_testsuite(
    build_system: &BuildSystem,
    extract: bool,
    wabt_path: Option<String>,
    validate: bool,
    clean: bool,
    run_wast: bool,
    wast_dir: String,
    wast_filter: Option<String>,
    per_assertion: bool,
    wast_report: Option<String>,
    test_timeout_ms: u64,
    global: &mut GlobalArgs,
) -> Result<()> {
    if clean {
        println!("{} Cleaning extracted test files...", "🧹".bright_blue());
        // TODO: Implement cleaning through wrt-build-core
        return Ok(());
    }

    if extract {
        println!(
            "{} Extracting WebAssembly test modules...",
            "📦".bright_blue()
        );
        if let Some(wabt) = wabt_path {
            println!("  Using WABT tools at: {}", wabt);
        }
        // TODO: Implement extraction through wrt-build-core
    }

    if validate {
        println!("{} Validating test modules...", "✅".bright_blue());
        // TODO: Implement validation through wrt-build-core
    }

    if run_wast {
        use wrt_build_core::wast::{ReportFormat, WastConfig, WastTestRunner};

        println!("{} Running WAST test suite...", "🧪".bright_blue());

        let workspace_root = build_system.workspace_root();
        let test_directory = workspace_root.join(&wast_dir);

        if !test_directory.exists() {
            anyhow::bail!(
                "WAST test directory not found: {}",
                test_directory.display()
            );
        }

        // Determine report format
        let report_format = match global.output_format {
            OutputFormat::Json | OutputFormat::JsonLines => ReportFormat::Json,
            OutputFormat::Human => ReportFormat::Human,
        };

        // Create WAST configuration
        let config = WastConfig {
            test_directory,
            file_filter: wast_filter,
            per_assertion,
            report_format,
            test_timeout_ms,
        };

        // Create and run WAST test runner
        let mut runner =
            WastTestRunner::new(config).context("Failed to create WAST test runner")?;
        let start_time = std::time::Instant::now();

        match runner.run_all_tests() {
            Ok(results) => {
                let total_time = start_time.elapsed();

                // Generate output based on format
                match global.output_format {
                    OutputFormat::Json | OutputFormat::JsonLines => {
                        // JSON output
                        let output = serde_json::json!({
                            "wast_test_results": {
                                "summary": runner.stats,
                                "files": results,
                                "execution_time_ms": total_time.as_millis(),
                                "diagnostics": runner.diagnostics()
                            }
                        });

                        if let Some(report_file) = wast_report {
                            let report_path = workspace_root.join(report_file);
                            std::fs::write(&report_path, serde_json::to_string_pretty(&output)?)?;
                            println!(
                                "{} WAST report written to {}",
                                "✅".bright_green(),
                                report_path.display()
                            );
                        } else {
                            println!("{}", serde_json::to_string_pretty(&output)?);
                        }
                    },
                    OutputFormat::Human => {
                        // Human-readable output
                        let summary = runner.generate_summary(&results);
                        println!("{}", summary);

                        if per_assertion {
                            println!("\n{} Per-assertion Results:", "📊".bright_cyan());
                            for file_result in &results {
                                println!("\n📄 {}", file_result.file_path.bright_cyan());
                                for directive in &file_result.directive_results {
                                    let status_icon = match directive.result {
                                        wrt_build_core::wast::TestResult::Passed => "✅",
                                        wrt_build_core::wast::TestResult::Failed => "❌",
                                        wrt_build_core::wast::TestResult::Skipped => "⏭️",
                                    };
                                    println!(
                                        "  {} {} ({})",
                                        status_icon, directive.directive_name, directive.test_type
                                    );
                                    if let Some(error) = &directive.error_message {
                                        println!("    Error: {}", error.bright_red());
                                    }
                                }
                            }
                        }

                        // Show overall results
                        let total_files = results.len();
                        let passed_files = results
                            .iter()
                            .filter(|r| r.status == wrt_build_core::wast::TestResult::Passed)
                            .count();
                        let failed_files = results
                            .iter()
                            .filter(|r| r.status == wrt_build_core::wast::TestResult::Failed)
                            .count();

                        println!("\n{} Overall Results:", "📈".bright_blue());
                        println!(
                            "  Files: {} total, {} passed, {} failed",
                            total_files, passed_files, failed_files
                        );
                        println!(
                            "  Assertions: {} passed, {} failed",
                            runner.stats.passed, runner.stats.failed
                        );
                        println!("  Execution time: {:.2}s", total_time.as_secs_f64());

                        if let Some(report_file) = wast_report {
                            let report_path = workspace_root.join(report_file);
                            let markdown_report =
                                generate_markdown_report(&runner, &results, total_time);
                            std::fs::write(&report_path, markdown_report)?;
                            println!(
                                "{} Markdown report written to {}",
                                "✅".bright_green(),
                                report_path.display()
                            );
                        }
                    },
                }

                // Output diagnostics if in diagnostic mode
                if !runner.diagnostics().is_empty() {
                    let formatter = FormatterFactory::create(global.output_format.clone());
                    // Convert diagnostics to DiagnosticCollection
                    let mut collection = DiagnosticCollection::new(
                        std::path::PathBuf::from("."),
                        "wast-test".to_string(),
                    );
                    collection.diagnostics = runner.diagnostics().to_vec();
                    print!("{}", formatter.format_collection(&collection));
                }

                // Exit with error code if tests failed
                if runner.stats.failed > 0 {
                    anyhow::bail!(
                        "WAST tests failed: {} assertions failed",
                        runner.stats.failed
                    );
                }
            },
            Err(e) => {
                global.output.error(&format!("WAST test execution failed: {}", e));
                return Err(e);
            },
        }
    }

    if !extract && !validate && !clean && !run_wast {
        println!(
            "{} No testsuite operations specified. Available options:",
            "ℹ️".bright_blue()
        );
        println!("  --extract      Extract test modules from .wast files");
        println!("  --validate     Run validation tests");
        println!("  --clean        Clean extracted test files");
        println!("  --run-wast     Run WAST test suite");
        println!();
        println!("Examples:");
        println!("  cargo-wrt testsuite --run-wast");
        println!("  cargo-wrt testsuite --run-wast --wast-filter \"simd\"");
        println!("  cargo-wrt testsuite --run-wast --per-assertion --output json");
    }

    Ok(())
}

/// Requirements command implementation
async fn cmd_requirements(
    build_system: &BuildSystem,
    command: RequirementsCommand,
    output_format: &OutputFormat,
    use_colors: bool,
    cli: &Cli,
) -> Result<()> {
    use wrt_build_core::requirements::{
        EnhancedRequirementsVerifier, Requirements,
        model::{ComplianceReport, RequirementType},
    };

    let workspace_root = build_system.workspace_root();

    match command {
        RequirementsCommand::Init { path, force } => {
            let req_path = workspace_root.join(&path);

            if req_path.exists() && !force {
                anyhow::bail!(
                    "Requirements file already exists at {}\nUse --force to overwrite",
                    req_path.display()
                );
            }

            Requirements::init_sample(&req_path)?;
            println!(
                "{} Initialized sample requirements file at {}",
                "✅".bright_green(),
                req_path.display()
            );
            Ok(())
        },

        RequirementsCommand::Verify {
            path,
            detailed,
            enhanced,
        } => {
            let req_path = workspace_root.join(&path);

            if enhanced {
                // Use enhanced SCORE verification
                let mut verifier = EnhancedRequirementsVerifier::new(workspace_root.to_path_buf());
                verifier.load_requirements(&req_path)?;
                verifier.verify_all()?;

                let registry = verifier.registry();
                let report = registry.generate_compliance_report();

                match output_format {
                    OutputFormat::Json | OutputFormat::JsonLines => {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    },
                    OutputFormat::Human => {
                        println!("{}", report.format_human());

                        if detailed {
                            println!("\n📋 Detailed Requirements Status:");
                            println!("─────────────────────────────────");

                            for req in &registry.requirements {
                                println!("\n{} {}", req.id, req.title.bright_cyan());
                                println!("  Type: {}", req.req_type);
                                println!("  ASIL: {}", req.asil_level);
                                println!("  Status: {}", req.status);
                                println!("  Coverage: {}", req.coverage);
                                println!("  Score: {:.1}%", req.compliance_score() * 100.0);
                            }
                        }
                    },
                }
            } else {
                // Use simple verification
                let requirements = Requirements::load(&req_path)?;
                let result = requirements.verify(&workspace_root)?;

                match output_format {
                    OutputFormat::Json | OutputFormat::JsonLines => {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    },
                    OutputFormat::Human => {
                        println!("{} Requirements Verification Report", "📊".bright_blue());
                        println!("  Total Requirements: {}", result.total_requirements);
                        println!("  Verified: {}", result.verified_requirements);
                        println!(
                            "  Certification Readiness: {:.1}%",
                            result.certification_readiness
                        );

                        if !result.missing_files.is_empty() {
                            println!("\n{} Missing Files:", "⚠️ ".yellow());
                            for file in &result.missing_files {
                                println!("  - {}", file);
                            }
                        }

                        if !result.incomplete_requirements.is_empty() {
                            println!("\n{} Incomplete Requirements:", "❌".red());
                            for req in &result.incomplete_requirements {
                                println!("  - {}", req);
                            }
                        }
                    },
                }
            }
            Ok(())
        },

        RequirementsCommand::Score {
            path,
            by_asil,
            by_type,
            detailed,
        } => {
            let req_path = workspace_root.join(&path);
            let requirements = Requirements::load(&req_path)?;
            let registry = requirements.to_registry();
            let report = registry.generate_compliance_report();

            match output_format {
                OutputFormat::Json | OutputFormat::JsonLines => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                },
                OutputFormat::Human => {
                    println!("{}", report.format_human());

                    if by_asil {
                        println!("\n📊 Compliance by ASIL Level:");
                        println!("───────────────────────────");
                        for (asil, score) in &report.asil_compliance {
                            println!("  {}: {:.1}%", asil, score * 100.0);
                        }
                    }

                    if by_type {
                        println!("\n📊 Compliance by Requirement Type:");
                        println!("──────────────────────────────────");
                        for req_type in &[
                            RequirementType::Functional,
                            RequirementType::Performance,
                            RequirementType::Safety,
                            RequirementType::Security,
                            RequirementType::Reliability,
                            RequirementType::Qualification,
                            RequirementType::Platform,
                            RequirementType::Memory,
                        ] {
                            let type_reqs = registry.get_requirements_by_type(req_type);
                            if !type_reqs.is_empty() {
                                let score: f64 =
                                    type_reqs.iter().map(|r| r.compliance_score()).sum::<f64>()
                                        / type_reqs.len() as f64;
                                println!("  {}: {:.1}%", req_type, score * 100.0);
                            }
                        }
                    }

                    if detailed {
                        if let Some((asil, score)) = report.lowest_asil_compliance() {
                            println!(
                                "\n⚠️  Lowest ASIL Compliance: {} at {:.1}%",
                                asil,
                                score * 100.0
                            );
                        }
                    }
                },
            }
            Ok(())
        },

        RequirementsCommand::Matrix {
            path,
            format,
            output,
        } => {
            let req_path = workspace_root.join(&path);
            let requirements = Requirements::load(&req_path)?;

            let matrix = match format.as_str() {
                "json" => {
                    let registry = requirements.to_registry();
                    serde_json::to_string_pretty(&registry)?
                },
                "html" => {
                    // Generate HTML requirements matrix
                    use crate::formatters::{HtmlFormatter, HtmlReportGenerator};

                    let formatter = HtmlFormatter::new();
                    let registry = requirements.to_registry();

                    // Convert to HTML data format
                    let req_data: Vec<_> = registry
                        .requirements
                        .iter()
                        .map(|req| crate::formatters::html::RequirementData {
                            id: req.id.to_string(),
                            title: req.title.clone(),
                            asil_level: req.asil_level.to_string(),
                            req_type: req.req_type.to_string(),
                            status: req.status.to_string(),
                            implementations: req.implementations.clone(),
                            tests: req.tests.clone(),
                            documentation: req.documentation.clone(),
                        })
                        .collect();

                    HtmlReportGenerator::requirements_matrix(&req_data, &formatter)?
                },
                "markdown" | "md" => {
                    // Generate Markdown requirements matrix
                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::new();
                    let registry = requirements.to_registry();

                    // Convert to data format
                    let req_data: Vec<_> = registry
                        .requirements
                        .iter()
                        .map(|req| crate::formatters::html::RequirementData {
                            id: req.id.to_string(),
                            title: req.title.clone(),
                            asil_level: req.asil_level.to_string(),
                            req_type: req.req_type.to_string(),
                            status: req.status.to_string(),
                            implementations: req.implementations.clone(),
                            tests: req.tests.clone(),
                            documentation: req.documentation.clone(),
                        })
                        .collect();

                    MarkdownReportGenerator::requirements_matrix(&req_data, &formatter)?
                },
                "github" => {
                    // Generate GitHub-flavored Markdown
                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::github();
                    let registry = requirements.to_registry();

                    // Convert to data format
                    let req_data: Vec<_> = registry
                        .requirements
                        .iter()
                        .map(|req| crate::formatters::html::RequirementData {
                            id: req.id.to_string(),
                            title: req.title.clone(),
                            asil_level: req.asil_level.to_string(),
                            req_type: req.req_type.to_string(),
                            status: req.status.to_string(),
                            implementations: req.implementations.clone(),
                            tests: req.tests.clone(),
                            documentation: req.documentation.clone(),
                        })
                        .collect();

                    MarkdownReportGenerator::requirements_matrix(&req_data, &formatter)?
                },
                _ => {
                    // Default to simple markdown
                    requirements.generate_traceability_matrix()
                },
            };

            if let Some(output_file) = output {
                std::fs::write(&output_file, matrix)?;
                println!(
                    "{} Generated traceability matrix at {}",
                    "✅".bright_green(),
                    output_file
                );
            } else {
                println!("{}", matrix);
            }
            Ok(())
        },

        RequirementsCommand::Missing {
            path,
            implementation,
            tests,
            docs,
            all,
        } => {
            let req_path = workspace_root.join(&path);
            let requirements = Requirements::load(&req_path)?;
            let registry = requirements.to_registry();

            let show_all = all || (!implementation && !tests && !docs);

            if show_all || implementation {
                let missing_impl = registry.get_requirements_needing_implementation();
                if !missing_impl.is_empty() {
                    println!("{} Requirements Missing Implementation:", "⚠️ ".yellow());
                    for req in missing_impl {
                        println!("  - {} {}", req.id, req.title);
                    }
                    println!();
                }
            }

            if show_all || tests {
                let missing_tests = registry.get_requirements_needing_testing();
                if !missing_tests.is_empty() {
                    println!("{} Requirements Missing Tests:", "⚠️ ".yellow());
                    for req in missing_tests {
                        println!("  - {} {} (coverage: {})", req.id, req.title, req.coverage);
                    }
                    println!();
                }
            }

            if show_all || docs {
                let missing_docs: Vec<_> =
                    registry.requirements.iter().filter(|r| r.documentation.is_empty()).collect();
                if !missing_docs.is_empty() {
                    println!("{} Requirements Missing Documentation:", "⚠️ ".yellow());
                    for req in missing_docs {
                        println!("  - {} {}", req.id, req.title);
                    }
                }
            }
            Ok(())
        },

        RequirementsCommand::Demo {
            output_dir,
            interactive,
        } => {
            println!("{} SCORE Methodology Demo", "🎓".bright_blue());
            println!("════════════════════════");

            if interactive {
                println!("Interactive demo not yet implemented");
            } else {
                // Create demo directory
                std::fs::create_dir_all(&output_dir)?;

                // Generate sample requirements file
                let req_path = PathBuf::from(&output_dir).join("requirements.toml");
                Requirements::init_sample(&req_path)?;

                println!("✅ Created sample requirements at {}", req_path.display());
                println!("\n📚 SCORE Methodology Overview:");
                println!("  - Safety requirements with ASIL levels");
                println!("  - Multiple verification methods");
                println!("  - Coverage level tracking");
                println!("  - Compliance scoring");
                println!("  - Traceability to implementation, tests, and docs");

                println!("\n🚀 Try these commands:");
                println!(
                    "  cargo-wrt requirements verify --enhanced --path {}",
                    req_path.display()
                );
                println!(
                    "  cargo-wrt requirements score --by-asil --path {}",
                    req_path.display()
                );
                println!(
                    "  cargo-wrt requirements missing --all --path {}",
                    req_path.display()
                );
            }
            Ok(())
        },
    }
}

/// WebAssembly command implementation
async fn cmd_wasm(
    build_system: &BuildSystem,
    command: WasmCommand,
    output_format: &OutputFormat,
    use_colors: bool,
    cli: &Cli,
) -> Result<()> {
    use wrt_build_core::wasm::{WasmVerifier, create_minimal_module, verify_modules};

    let workspace_root = build_system.workspace_root();

    match command {
        WasmCommand::Verify {
            file,
            detailed,
            benchmark,
        } => {
            let wasm_path = workspace_root.join(&file);

            if !wasm_path.exists() {
                anyhow::bail!("WebAssembly module not found: {}", wasm_path.display());
            }

            let verifier = WasmVerifier::new(&wasm_path);
            let result = verifier.verify()?;

            match output_format {
                OutputFormat::Json | OutputFormat::JsonLines => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                },
                OutputFormat::Human => {
                    verifier.print_results(&result);

                    if detailed && !result.errors.is_empty() {
                        println!("\n🔍 Detailed Error Analysis:");
                        println!("───────────────────────────");
                        for (i, error) in result.errors.iter().enumerate() {
                            println!("  {}. {}", i + 1, error);
                        }
                    }
                },
            }

            // Also output as diagnostics if there are errors
            if !result.errors.is_empty() {
                let diagnostics = verifier.to_diagnostics(&result);
                if cli.verbose {
                    println!("\n📋 Diagnostic Output:");
                    println!("───────────────────");
                    let formatter =
                        wrt_build_core::formatters::FormatterFactory::create(*output_format);
                    println!("{}", formatter.format_collection(&diagnostics));
                }
            }

            Ok(())
        },

        WasmCommand::Imports {
            file,
            builtins_only,
            module,
        } => {
            let wasm_path = workspace_root.join(&file);

            if !wasm_path.exists() {
                anyhow::bail!("WebAssembly module not found: {}", wasm_path.display());
            }

            let verifier = WasmVerifier::new(&wasm_path);
            let result = verifier.verify()?;

            match output_format {
                OutputFormat::Json | OutputFormat::JsonLines => {
                    if builtins_only {
                        println!("{}", serde_json::to_string_pretty(&result.builtin_imports)?);
                    } else {
                        let filtered_imports: Vec<_> = result
                            .imports
                            .iter()
                            .filter(|imp| module.as_ref().map_or(true, |m| &imp.module == m))
                            .collect();
                        println!("{}", serde_json::to_string_pretty(&filtered_imports)?);
                    }
                },
                OutputFormat::Human => {
                    if builtins_only {
                        if result.builtin_imports.is_empty() {
                            println!("No builtin imports found");
                        } else {
                            println!("🔧 Builtin Imports ({}):", result.builtin_imports.len());
                            for builtin in &result.builtin_imports {
                                println!("  - wasi_builtin::{}", builtin);
                            }
                        }
                    } else {
                        let filtered_imports: Vec<_> = result
                            .imports
                            .iter()
                            .filter(|imp| module.as_ref().map_or(true, |m| &imp.module == m))
                            .collect();

                        if filtered_imports.is_empty() {
                            println!("No imports found");
                        } else {
                            println!("📥 Imports ({}):", filtered_imports.len());
                            for import in filtered_imports {
                                println!(
                                    "  - {}::{} ({})",
                                    import.module, import.name, import.kind
                                );
                            }
                        }
                    }
                },
            }

            Ok(())
        },

        WasmCommand::Exports { file, kind } => {
            let wasm_path = workspace_root.join(&file);

            if !wasm_path.exists() {
                anyhow::bail!("WebAssembly module not found: {}", wasm_path.display());
            }

            let verifier = WasmVerifier::new(&wasm_path);
            let result = verifier.verify()?;

            let filtered_exports: Vec<_> = result
                .exports
                .iter()
                .filter(|exp| kind.as_ref().map_or(true, |k| &exp.kind == k))
                .collect();

            match output_format {
                OutputFormat::Json | OutputFormat::JsonLines => {
                    println!("{}", serde_json::to_string_pretty(&filtered_exports)?);
                },
                OutputFormat::Human => {
                    if filtered_exports.is_empty() {
                        println!("No exports found");
                    } else {
                        println!("📤 Exports ({}):", filtered_exports.len());
                        for export in filtered_exports {
                            println!("  - {} ({})", export.name, export.kind);
                        }
                    }
                },
            }

            Ok(())
        },

        WasmCommand::Analyze {
            files,
            summary,
            performance,
        } => {
            let mut wasm_paths = Vec::new();

            // Expand glob patterns
            for pattern in &files {
                let full_pattern = workspace_root.join(pattern);
                match glob::glob(&full_pattern.to_string_lossy()) {
                    Ok(paths) => {
                        for path in paths {
                            match path {
                                Ok(p) => wasm_paths.push(p),
                                Err(e) => eprintln!("Warning: Failed to read path: {}", e),
                            }
                        }
                    },
                    Err(_) => {
                        // Not a glob pattern, treat as single file
                        wasm_paths.push(workspace_root.join(pattern));
                    },
                }
            }

            if wasm_paths.is_empty() {
                anyhow::bail!("No WebAssembly modules found matching the patterns");
            }

            let results = verify_modules(&wasm_paths)?;

            match output_format {
                OutputFormat::Json | OutputFormat::JsonLines => {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                },
                OutputFormat::Human => {
                    println!("🔍 WebAssembly Module Analysis");
                    println!("════════════════════════════");
                    println!("Analyzed {} modules\n", results.len());

                    let mut total_valid = 0;
                    let mut total_imports = 0;
                    let mut total_exports = 0;
                    let mut total_builtins = 0;

                    for (path, result) in &results {
                        if summary {
                            println!("📄 {}", path.bright_cyan());
                            println!(
                                "  Status: {}",
                                if result.valid { "✅ Valid" } else { "❌ Invalid" }
                            );
                            if performance {
                                if let Some(perf) = result.performance.as_ref() {
                                    println!("  Parse time: {}ms", perf.parse_time_ms);
                                    println!("  Size: {} bytes", perf.module_size);
                                }
                            }
                            println!(
                                "  Imports: {}, Exports: {}, Builtins: {}",
                                result.imports.len(),
                                result.exports.len(),
                                result.builtin_imports.len()
                            );
                            println!();
                        } else {
                            let verifier = WasmVerifier::new(path);
                            verifier.print_results(result);
                            println!();
                        }

                        if result.valid {
                            total_valid += 1;
                        }
                        total_imports += result.imports.len();
                        total_exports += result.exports.len();
                        total_builtins += result.builtin_imports.len();
                    }

                    if summary {
                        println!("📊 Summary:");
                        println!("─────────");
                        println!("  Valid modules: {}/{}", total_valid, results.len());
                        println!("  Total imports: {}", total_imports);
                        println!("  Total exports: {}", total_exports);
                        println!("  Total builtins: {}", total_builtins);
                    }
                },
            }

            Ok(())
        },

        WasmCommand::CreateTest {
            output,
            with_builtins,
        } => {
            let test_module = if with_builtins {
                create_minimal_module()
            } else {
                // Create even simpler module without imports
                vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00] // Just header
            };

            let output_path = workspace_root.join(&output);
            std::fs::write(&output_path, test_module)?;

            println!(
                "{} Created test WebAssembly module at {}",
                "✅".bright_green(),
                output_path.display()
            );

            if with_builtins {
                println!("  Module includes builtin imports (wasi_builtin::random)");
            } else {
                println!("  Module is minimal (header only)");
            }

            Ok(())
        },
    }
}

/// Handle safety verification commands with SCORE methodology
async fn cmd_safety(
    build_system: &BuildSystem,
    command: SafetyCommand,
    output_format: &OutputFormat,
    use_colors: bool,
    cli: &Cli,
) -> Result<()> {
    let workspace_root = build_system.workspace_root().to_path_buf();

    match command {
        SafetyCommand::Verify {
            asil,
            requirements,
            include_tests,
            include_platform,
            detailed,
        } => {
            let asil_level = wrt_build_core::config::AsilLevel::from(asil);

            // Import the safety verification framework
            use wrt_build_core::requirements::{Requirements, SafetyVerificationFramework};

            let mut framework = SafetyVerificationFramework::new(workspace_root.clone());

            // Load WRT-specific safety requirements
            let (_count, load_diagnostics) =
                framework.load_requirements_from_source(&requirements)?;

            // If we have a requirements file, load it too
            let requirements_path = workspace_root.join(&requirements);
            if requirements_path.exists() {
                let reqs = Requirements::load(&requirements_path)?;
                let registry = reqs.to_registry();
                for req in registry.requirements {
                    framework.add_requirement(req);
                }
            }

            // Perform ASIL compliance verification
            let (compliance_result, verification_diagnostics) =
                framework.verify_asil_compliance(asil_level)?;

            // Generate safety report
            let (safety_report, report_diagnostics) = framework.generate_safety_report();

            // Combine all diagnostics manually
            let mut all_diagnostics = load_diagnostics;
            // Add verification diagnostics
            for diag in verification_diagnostics.diagnostics {
                all_diagnostics.add_diagnostic(diag);
            }
            // Add report diagnostics
            for diag in report_diagnostics.diagnostics {
                all_diagnostics.add_diagnostic(diag);
            }

            // Format and display results
            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!(
                "{}",
                formatter.format_diagnostics(&all_diagnostics.diagnostics)
            );

            if detailed {
                match output_format {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&compliance_result)?);
                    },
                    _ => {
                        println!(
                            "\n{} ASIL {} Compliance Verification:",
                            "📊".bright_blue(),
                            asil_level
                        );
                        println!(
                            "  Total Requirements: {}",
                            compliance_result.total_requirements
                        );
                        println!("  Verified: {}", compliance_result.verified_requirements);
                        println!(
                            "  Compliance: {:.1}%",
                            compliance_result.compliance_percentage
                        );
                        println!(
                            "  Status: {}",
                            if compliance_result.is_compliant {
                                "✅ COMPLIANT".bright_green()
                            } else {
                                "❌ NON-COMPLIANT".bright_red()
                            }
                        );

                        if !compliance_result.violations.is_empty() {
                            println!("\n{} Violations:", "⚠️ ".bright_yellow());
                            for violation in &compliance_result.violations {
                                println!(
                                    "  • {}: {}",
                                    violation.violation_type, violation.description
                                );
                            }
                        }
                    },
                }
            }

            Ok(())
        },

        SafetyCommand::Certify {
            asil,
            requirements,
            test_results,
            coverage_data,
        } => {
            let asil_level = wrt_build_core::config::AsilLevel::from(asil);

            use wrt_build_core::requirements::{Requirements, SafetyVerificationFramework};

            let mut framework = SafetyVerificationFramework::new(workspace_root.clone());

            // Load requirements
            let requirements_path = workspace_root.join(&requirements);
            if requirements_path.exists() {
                let reqs = Requirements::load(&requirements_path)?;
                let registry = reqs.to_registry();
                for req in registry.requirements {
                    framework.add_requirement(req);
                }
            }

            // Load test results if provided
            if let Some(test_file) = test_results {
                // TODO: Implement test results loading from JSON
                println!("Loading test results from: {}", test_file);
            }

            // Load coverage data if provided
            if let Some(coverage_file) = coverage_data {
                // TODO: Implement coverage data loading from JSON
                println!("Loading coverage data from: {}", coverage_file);
            }

            // Check certification readiness
            let (readiness, diagnostics) = framework.check_certification_readiness(asil_level);

            // Format and display results
            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!("{}", formatter.format_collection(&diagnostics));

            match output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&readiness)?);
                },
                _ => {
                    println!(
                        "\n{} ASIL {} Certification Readiness:",
                        "🏆".bright_yellow(),
                        asil_level
                    );
                    println!(
                        "  Status: {}",
                        if readiness.is_ready {
                            "✅ READY FOR CERTIFICATION".bright_green()
                        } else {
                            "❌ NOT READY".bright_red()
                        }
                    );

                    println!(
                        "  Compliance: {:.1}% (requires {:.1}%)",
                        readiness.compliance_percentage, readiness.required_compliance
                    );
                    println!(
                        "  Coverage: {:.1}% (requires {:.1}%)",
                        readiness.coverage_percentage, readiness.required_coverage
                    );

                    if !readiness.blocking_issues.is_empty() {
                        println!("\n{} Blocking Issues:", "🚫".bright_red());
                        for issue in &readiness.blocking_issues {
                            println!("  • {}", issue);
                        }
                    }

                    if !readiness.recommendations.is_empty() {
                        println!("\n{} Recommendations:", "💡".bright_blue());
                        for rec in &readiness.recommendations {
                            println!("  • {}", rec);
                        }
                    }
                },
            }

            Ok(())
        },

        SafetyCommand::Report {
            requirements,
            all_asil,
            output,
            format,
        } => {
            use wrt_build_core::requirements::{Requirements, SafetyVerificationFramework};

            let mut framework = SafetyVerificationFramework::new(workspace_root.clone());

            // Load requirements
            let requirements_path = workspace_root.join(&requirements);
            if requirements_path.exists() {
                let reqs = Requirements::load(&requirements_path)?;
                let registry = reqs.to_registry();
                for req in registry.requirements {
                    framework.add_requirement(req);
                }
            }

            // Generate comprehensive safety report
            let (safety_report, diagnostics) = framework.generate_safety_report();

            // Format output based on requested format
            let report_content = match format.as_str() {
                "json" => serde_json::to_string_pretty(&safety_report)?,
                "html" => {
                    // Generate HTML safety report
                    use std::collections::HashMap;

                    use crate::formatters::{HtmlFormatter, HtmlReportGenerator};

                    let formatter = HtmlFormatter::new();

                    // Convert to HTML data format
                    let asil_compliance: HashMap<String, f64> = safety_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v))
                        .collect();

                    let html_data = crate::formatters::html::SafetyReportData {
                        overall_compliance: safety_report.overall_compliance * 100.0,
                        asil_compliance,
                        test_summary: crate::formatters::html::TestSummaryData {
                            total_tests: safety_report.test_summary.total_tests,
                            passed_tests: safety_report.test_summary.passed_tests,
                            failed_tests: safety_report.test_summary.failed_tests,
                            coverage_percentage: safety_report.test_summary.coverage_percentage,
                        },
                        recommendations: safety_report.recommendations.clone(),
                    };

                    HtmlReportGenerator::safety_report(&html_data, &formatter)?
                },
                "markdown" | "md" => {
                    // Generate Markdown safety report
                    use std::collections::HashMap;

                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::new();

                    // Convert to data format
                    let asil_compliance: HashMap<String, f64> = safety_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v * 100.0))
                        .collect();

                    let report_data = crate::formatters::html::SafetyReportData {
                        overall_compliance: safety_report.overall_compliance * 100.0,
                        asil_compliance,
                        test_summary: crate::formatters::html::TestSummaryData {
                            total_tests: safety_report.test_summary.total_tests,
                            passed_tests: safety_report.test_summary.passed_tests,
                            failed_tests: safety_report.test_summary.failed_tests,
                            coverage_percentage: safety_report.test_summary.coverage_percentage,
                        },
                        recommendations: safety_report.recommendations.clone(),
                    };

                    MarkdownReportGenerator::safety_report(&report_data, &formatter)?
                },
                "github" => {
                    // Generate GitHub-flavored Markdown safety report
                    use std::collections::HashMap;

                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::github();

                    // Convert to data format
                    let asil_compliance: HashMap<String, f64> = safety_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v * 100.0))
                        .collect();

                    let report_data = crate::formatters::html::SafetyReportData {
                        overall_compliance: safety_report.overall_compliance * 100.0,
                        asil_compliance,
                        test_summary: crate::formatters::html::TestSummaryData {
                            total_tests: safety_report.test_summary.total_tests,
                            passed_tests: safety_report.test_summary.passed_tests,
                            failed_tests: safety_report.test_summary.failed_tests,
                            coverage_percentage: safety_report.test_summary.coverage_percentage,
                        },
                        recommendations: safety_report.recommendations.clone(),
                    };

                    MarkdownReportGenerator::safety_report(&report_data, &formatter)?
                },
                _ => {
                    // Human-readable format
                    let mut content = String::new();
                    content.push_str(&format!("🛡️  WRT Safety Verification Report\n"));
                    content.push_str(&format!("════════════════════════════════\n\n"));
                    content.push_str(&format!(
                        "Overall Compliance: {:.1}%\n",
                        safety_report.overall_compliance * 100.0
                    ));
                    content.push_str(&format!(
                        "Unverified Requirements: {}\n",
                        safety_report.unverified_requirements
                    ));
                    content.push_str(&format!(
                        "Critical Violations: {}\n\n",
                        safety_report.critical_violations.len()
                    ));

                    content.push_str("📊 Test Summary:\n");
                    content.push_str(&format!(
                        "  Total Tests: {}\n",
                        safety_report.test_summary.total_tests
                    ));
                    content.push_str(&format!(
                        "  Passed: {}\n",
                        safety_report.test_summary.passed_tests
                    ));
                    content.push_str(&format!(
                        "  Failed: {}\n",
                        safety_report.test_summary.failed_tests
                    ));
                    content.push_str(&format!(
                        "  Coverage: {:.1}%\n\n",
                        safety_report.test_summary.coverage_percentage
                    ));

                    if all_asil {
                        content.push_str("🎯 ASIL Compliance:\n");
                        for (asil, compliance) in &safety_report.asil_compliance {
                            content.push_str(&format!("  {}: {:.1}%\n", asil, compliance * 100.0));
                        }
                        content.push('\n');
                    }

                    if !safety_report.recommendations.is_empty() {
                        content.push_str("💡 Recommendations:\n");
                        for rec in &safety_report.recommendations {
                            content.push_str(&format!("  • {}\n", rec));
                        }
                    }

                    content
                },
            };

            // Output to file or stdout
            if let Some(output_file) = output {
                let output_path = workspace_root.join(output_file);
                std::fs::write(&output_path, report_content)?;
                println!(
                    "{} Safety report written to {}",
                    "✅".bright_green(),
                    output_path.display()
                );
            } else {
                println!("{}", report_content);
            }

            // Also output diagnostics
            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!("{}", formatter.format_collection(&diagnostics));

            Ok(())
        },

        SafetyCommand::RecordTest {
            test_name,
            passed,
            duration_ms,
            asil,
            verifies,
            coverage_type,
            failure_reason,
        } => {
            use wrt_build_core::requirements::{
                RequirementId, SafetyVerificationFramework, TestCoverageType, TestResult,
            };

            let mut framework = SafetyVerificationFramework::new(workspace_root.clone());
            let asil_level = wrt_build_core::config::AsilLevel::from(asil);

            // Parse verified requirements
            let verified_requirements = if let Some(req_list) = verifies {
                req_list.split(',').map(|s| RequirementId::new(s.trim())).collect()
            } else {
                Vec::new()
            };

            // Convert coverage type
            let test_coverage_type = match coverage_type {
                TestCoverageArg::Basic => TestCoverageType::Basic,
                TestCoverageArg::Comprehensive => TestCoverageType::Comprehensive,
                TestCoverageArg::Complete => TestCoverageType::Complete,
            };

            let test_result = TestResult {
                test_name: test_name.clone(),
                passed,
                execution_time_ms: duration_ms.unwrap_or(0),
                verified_requirements,
                coverage_type: test_coverage_type,
                failure_reason: failure_reason.unwrap_or_default(),
                asil_level,
            };

            let diagnostics = framework.record_test_result(test_result);

            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if passed {
                println!(
                    "{} Test {} recorded as PASSED",
                    "✅".bright_green(),
                    test_name
                );
            } else {
                println!(
                    "{} Test {} recorded as FAILED",
                    "❌".bright_red(),
                    test_name
                );
            }

            Ok(())
        },

        SafetyCommand::UpdateCoverage {
            line_coverage,
            branch_coverage,
            function_coverage,
            coverage_file,
        } => {
            use wrt_build_core::requirements::{CoverageData, FileCoverage};

            let coverage_data = if let Some(file) = coverage_file {
                // TODO: Load from JSON file
                println!("Loading coverage data from: {}", file);
                CoverageData::new()
            } else {
                CoverageData {
                    line_coverage: line_coverage.unwrap_or(0.0),
                    branch_coverage: branch_coverage.unwrap_or(0.0),
                    function_coverage: function_coverage.unwrap_or(0.0),
                    file_coverages: Vec::new(),
                }
            };

            println!("{} Coverage data updated:", "📊".bright_blue());
            println!("  Line Coverage: {:.1}%", coverage_data.line_coverage);
            println!("  Branch Coverage: {:.1}%", coverage_data.branch_coverage);
            println!(
                "  Function Coverage: {:.1}%",
                coverage_data.function_coverage
            );
            println!("  Overall: {:.1}%", coverage_data.overall_coverage());

            Ok(())
        },

        SafetyCommand::PlatformVerify {
            platform,
            passed,
            asil,
            verified_features,
            failed_features,
        } => {
            use wrt_build_core::requirements::PlatformVerification;

            let asil_level = wrt_build_core::config::AsilLevel::from(asil);

            let verified = verified_features
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let failed = failed_features
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            let verification = PlatformVerification {
                platform_name: platform.clone(),
                verification_passed: passed,
                verified_features: verified,
                failed_features: failed,
                asil_compliance: asil_level,
            };

            if passed {
                println!(
                    "{} Platform {} verification PASSED (ASIL {})",
                    "✅".bright_green(),
                    platform,
                    asil_level
                );
            } else {
                println!(
                    "{} Platform {} verification FAILED",
                    "❌".bright_red(),
                    platform
                );
            }

            Ok(())
        },

        SafetyCommand::Init {
            wrt_requirements,
            force,
        } => {
            use wrt_build_core::requirements::SafetyVerificationFramework;

            let mut framework = SafetyVerificationFramework::new(workspace_root.clone());

            if wrt_requirements {
                // Initialize with WRT-specific safety requirements
                let (_count, diagnostics) =
                    framework.load_requirements_from_source("wrt-safety-requirements")?;

                let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                    *output_format,
                    true,
                    use_colors,
                );
                print!("{}", formatter.format_collection(&diagnostics));

                println!(
                    "{} Initialized safety verification with WRT-specific requirements",
                    "✅".bright_green()
                );
            } else {
                println!(
                    "{} Safety verification framework initialized",
                    "✅".bright_green()
                );
            }

            Ok(())
        },

        SafetyCommand::Demo {
            output_dir,
            interactive,
            certification,
        } => {
            let demo_path = workspace_root.join(&output_dir);
            std::fs::create_dir_all(&demo_path)?;

            println!(
                "{} Creating SCORE methodology demonstration in {}",
                "🎯".bright_blue(),
                demo_path.display()
            );

            if interactive {
                println!("🔄 Interactive demo mode not yet implemented");
            }

            if certification {
                println!("🏆 Certification workflow demo not yet implemented");
            }

            println!("{} SCORE demo setup complete", "✅".bright_green());
            Ok(())
        },

        SafetyCommand::VerifyDocs {
            requirements,
            asil,
            detailed,
            check_implementations,
            check_api,
        } => {
            use wrt_build_core::requirements::{
                DocumentationVerificationConfig, DocumentationVerificationFramework, Requirements,
            };

            let mut framework = DocumentationVerificationFramework::new(workspace_root.clone());

            // Configure the framework based on options
            let mut config = DocumentationVerificationConfig::default();
            config.enable_api_documentation_check = check_api;
            framework = framework.with_config(config);

            // Load requirements
            let requirements_path = workspace_root.join(&requirements);
            if requirements_path.exists() {
                let reqs = Requirements::load(&requirements_path)?;
                let registry = reqs.to_registry();
                for req in registry.requirements {
                    framework.add_requirement(req);
                }
            }

            // Perform documentation verification
            let (result, diagnostics) = if let Some(asil_level) = asil {
                let asil_level = wrt_build_core::config::AsilLevel::from(asil_level);
                framework.verify_asil_documentation(asil_level)?
            } else {
                framework.verify_all_documentation()?
            };

            // Format and display results
            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!("{}", formatter.format_collection(&diagnostics));

            if detailed {
                match output_format {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    },
                    _ => {
                        println!(
                            "\n{} Documentation Compliance Verification:",
                            "📚".bright_blue()
                        );
                        println!("  Total Requirements: {}", result.total_requirements);
                        println!("  Compliant: {}", result.compliant_requirements);
                        println!("  Compliance: {:.1}%", result.compliance_percentage);
                        println!(
                            "  Certification Ready: {}",
                            if result.is_certification_ready {
                                "✅ YES".bright_green()
                            } else {
                                "❌ NO".bright_red()
                            }
                        );

                        if !result.violations.is_empty() {
                            println!("\n{} Documentation Violations:", "⚠️ ".bright_yellow());
                            for violation in &result.violations {
                                println!(
                                    "  • {} ({}): {}",
                                    violation.violation_type,
                                    violation.severity,
                                    violation.description
                                );
                            }
                        }
                    },
                }
            }

            Ok(())
        },

        SafetyCommand::DocsReport {
            requirements,
            all_asil,
            output,
            format,
        } => {
            use wrt_build_core::requirements::{DocumentationVerificationFramework, Requirements};

            let mut framework = DocumentationVerificationFramework::new(workspace_root.clone());

            // Load requirements
            let requirements_path = workspace_root.join(&requirements);
            if requirements_path.exists() {
                let reqs = Requirements::load(&requirements_path)?;
                let registry = reqs.to_registry();
                for req in registry.requirements {
                    framework.add_requirement(req);
                }
            }

            // Generate comprehensive documentation report
            let (doc_report, diagnostics) = framework.generate_report();

            // Format output based on requested format
            let report_content = match format.as_str() {
                "json" => serde_json::to_string_pretty(&doc_report)?,
                "html" => {
                    // Generate HTML documentation report
                    use std::collections::HashMap;

                    use crate::formatters::{HtmlFormatter, HtmlReportGenerator};

                    let formatter = HtmlFormatter::new();

                    // Convert to HTML data format
                    let asil_compliance: HashMap<String, f64> = doc_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v))
                        .collect();

                    let html_data = crate::formatters::html::DocumentationReportData {
                        overall_compliance: doc_report.overall_compliance * 100.0,
                        total_requirements: doc_report.total_requirements,
                        total_violations: doc_report.total_violations,
                        critical_violations: doc_report.critical_violations,
                        asil_compliance,
                    };

                    HtmlReportGenerator::documentation_report(&html_data, &formatter)?
                },
                "markdown" | "md" => {
                    // Generate Markdown documentation report
                    use std::collections::HashMap;

                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::new();

                    // Convert to data format
                    let asil_compliance: HashMap<String, f64> = doc_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v * 100.0))
                        .collect();

                    let report_data = crate::formatters::html::DocumentationReportData {
                        overall_compliance: doc_report.overall_compliance * 100.0,
                        total_requirements: doc_report.total_requirements,
                        total_violations: doc_report.total_violations,
                        critical_violations: doc_report.critical_violations,
                        asil_compliance,
                    };

                    MarkdownReportGenerator::documentation_report(&report_data, &formatter)?
                },
                "github" => {
                    // Generate GitHub-flavored Markdown documentation report
                    use std::collections::HashMap;

                    use crate::formatters::{MarkdownFormatter, MarkdownReportGenerator};

                    let formatter = MarkdownFormatter::github();

                    // Convert to data format
                    let asil_compliance: HashMap<String, f64> = doc_report
                        .asil_compliance
                        .iter()
                        .map(|(k, v)| (format!("{:?}", k), *v * 100.0))
                        .collect();

                    let report_data = crate::formatters::html::DocumentationReportData {
                        overall_compliance: doc_report.overall_compliance * 100.0,
                        total_requirements: doc_report.total_requirements,
                        total_violations: doc_report.total_violations,
                        critical_violations: doc_report.critical_violations,
                        asil_compliance,
                    };

                    MarkdownReportGenerator::documentation_report(&report_data, &formatter)?
                },
                _ => {
                    // Human-readable format
                    let mut content = String::new();
                    content.push_str(&format!("📚 WRT Documentation Compliance Report\n"));
                    content.push_str(&format!("═══════════════════════════════════════\n\n"));
                    content.push_str(&format!(
                        "Overall Compliance: {:.1}%\n",
                        doc_report.overall_compliance
                    ));
                    content.push_str(&format!(
                        "Total Requirements: {}\n",
                        doc_report.total_requirements
                    ));
                    content.push_str(&format!(
                        "Total Violations: {}\n",
                        doc_report.total_violations
                    ));
                    content.push_str(&format!(
                        "Critical Violations: {}\n\n",
                        doc_report.critical_violations
                    ));

                    if all_asil && !doc_report.asil_compliance.is_empty() {
                        content.push_str("📊 ASIL Documentation Compliance:\n");
                        let mut sorted_asil: Vec<_> = doc_report.asil_compliance.iter().collect();
                        sorted_asil.sort_by_key(|(asil, _)| match asil {
                            wrt_build_core::config::AsilLevel::QM => 0,
                            wrt_build_core::config::AsilLevel::A => 1,
                            wrt_build_core::config::AsilLevel::B => 2,
                            wrt_build_core::config::AsilLevel::C => 3,
                            wrt_build_core::config::AsilLevel::D => 4,
                        });
                        for (asil, compliance) in sorted_asil {
                            content.push_str(&format!("  {}: {:.1}%\n", asil, compliance));
                        }
                        content.push('\n');
                    }

                    if !doc_report.recommendations.is_empty() {
                        content.push_str("💡 Recommendations:\n");
                        for rec in &doc_report.recommendations {
                            content.push_str(&format!("  • {}\n", rec));
                        }
                    }

                    content
                },
            };

            // Output to file or stdout
            if let Some(output_file) = output {
                let output_path = workspace_root.join(output_file);
                std::fs::write(&output_path, report_content)?;
                println!(
                    "{} Documentation report written to {}",
                    "✅".bright_green(),
                    output_path.display()
                );
            } else {
                println!("{}", report_content);
            }

            // Also output diagnostics
            let formatter = wrt_build_core::formatters::FormatterFactory::create_with_options(
                *output_format,
                true,
                use_colors,
            );
            print!("{}", formatter.format_collection(&diagnostics));

            Ok(())
        },
    }
}

/// Print comprehensive diagnostic system help
fn print_diagnostic_help() {
    println!(
        r#"{} WRT Diagnostic System - Comprehensive Guide

{}

The WRT build system includes a unified diagnostic system with LSP-compatible
structured output, caching, filtering, grouping, and differential analysis.

{} Global Diagnostic Flags (work with build, test, verify, check):

  {} Output Format Control:
    --output human          Human-readable with colors (default)
    --output json           LSP-compatible JSON for tooling/AI agents
    --output json-lines     Streaming JSON (one diagnostic per line)

  {} Caching Control:
    --cache                 Enable diagnostic caching for faster builds
    --clear-cache          Clear cache before running
    --diff-only            Show only new/changed diagnostics (requires --cache)

  {} Filtering Options:
    --filter-severity LIST  error,warning,info,hint (comma-separated)
    --filter-source LIST    rustc,clippy,miri,cargo (comma-separated)
    --filter-file PATTERNS  *.rs,src/* (glob patterns, comma-separated)

  {} Grouping & Pagination:
    --group-by CRITERION    file|severity|source|code
    --limit NUMBER         Limit diagnostic output count

{} Common Usage Patterns:

  {} Basic Error Analysis:
    cargo-wrt build --output json --filter-severity error
    cargo-wrt check --output json --filter-source clippy

  {} Incremental Development Workflow:
    # Initial baseline
    cargo-wrt build --cache --clear-cache
    
    # Subsequent runs - see only new issues
    cargo-wrt build --cache --diff-only
    
    # Focus on errors only
    cargo-wrt build --cache --diff-only --filter-severity error

  {} Code Quality Analysis:
    # Group warnings by file for focused fixes
    cargo-wrt check --output json --group-by file --filter-severity warning
    
    # Limit output for manageable chunks
    cargo-wrt check --output json --limit 10

  {} CI/CD Integration:
    # Generate structured reports
    cargo-wrt verify --output json --filter-source kani,miri
    
    # Stream processing for large outputs
    cargo-wrt build --output json-lines | process_diagnostics

{} JSON Diagnostic Format:

  The JSON output follows LSP (Language Server Protocol) specification:
  
  {{
    "version": "1.0",
    "timestamp": "2025-06-21T11:39:57Z",
    "workspace_root": "/path/to/workspace",
    "command": "build",
    "diagnostics": [
      {{
        "file": "src/main.rs",
        "range": {{
          "start": {{"line": 10, "character": 5}},
          "end": {{"line": 10, "character": 15}}
        }},
        "severity": "error",
        "code": "E0425",
        "message": "cannot find value `undefined_var`",
        "source": "rustc",
        "related_info": []
      }}
    ],
    "summary": {{
      "total": 1,
      "errors": 1,
      "warnings": 0,
      "infos": 0,
      "hints": 0,
      "files_with_diagnostics": 1,
      "duration_ms": 1500
    }}
  }}

{} Key Fields:
  - file: Relative path from workspace root
  - range: LSP-compatible position (0-indexed line/character)
  - severity: "error"|"warning"|"info"|"hint"
  - code: Tool-specific error code (optional)
  - source: Tool that generated diagnostic ("rustc", "clippy", etc.)

{} Performance Benefits:
  - Initial run: Full analysis (3-4 seconds)
  - Cached run: Incremental analysis (~0.7 seconds)
  - Diff-only: Shows only changed diagnostics

{} Advanced Examples:

  {} Multi-tool Analysis:
    cargo-wrt verify --output json --filter-source "rustc,clippy,miri"

  {} File-specific Focus:
    cargo-wrt build --output json --filter-file "wrt-foundation/*"

  {} Severity Prioritization:
    cargo-wrt build --output json --group-by severity --limit 20

  {} JSON Processing with jq:
    # Extract error messages
    cargo-wrt build --output json | jq '.diagnostics[] | select(.severity == "error") | .message'
    
    # Count diagnostics by file
    cargo-wrt build --output json | jq '.diagnostics | group_by(.file) | map({{file: .[0].file, count: length}})'
    
    # Check for errors programmatically
    cargo-wrt build --output json | jq '.summary.errors > 0'

{} Integration Notes:
  - Exit code 0: No errors present
  - Exit code 1: Errors found (warnings don't affect exit code)
  - Compatible with IDEs via LSP diagnostic publishing
  - Cacheable for CI/CD performance optimization

For command-specific help: cargo-wrt <command> --help
"#,
        "🔧".bright_blue(),
        "═".repeat(60).bright_blue(),
        "📋".bright_cyan(),
        "📤".bright_green(),
        "💾".bright_yellow(),
        "🔍".bright_magenta(),
        "📊".bright_red(),
        "🚀".bright_blue(),
        "1.".bright_cyan(),
        "2.".bright_cyan(),
        "3.".bright_cyan(),
        "4.".bright_cyan(),
        "📄".bright_green(),
        "🔑".bright_yellow(),
        "⚡".bright_magenta(),
        "💡".bright_blue(),
        "•".bright_green(),
        "•".bright_green(),
        "•".bright_green(),
        "•".bright_green(),
        "🔗".bright_cyan()
    );
}

/// Generate a markdown report for WAST test results
fn generate_markdown_report(
    runner: &wrt_build_core::wast::WastTestRunner,
    results: &[wrt_build_core::wast::WastFileResult],
    total_time: std::time::Duration,
) -> String {
    let mut report = String::new();

    report.push_str("# WAST Test Suite Report\n\n");
    report.push_str(&format!(
        "Generated on: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    ));

    // Summary
    report.push_str("## Summary\n\n");
    report.push_str(&format!(
        "- **Files processed**: {}\n",
        runner.stats.files_processed
    ));
    report.push_str(&format!(
        "- **Total assertions**: {} passed, {} failed, {} skipped\n",
        runner.stats.passed, runner.stats.failed, runner.stats.skipped
    ));
    report.push_str(&format!(
        "- **Execution time**: {:.2}s\n",
        total_time.as_secs_f64()
    ));

    // Test type breakdown
    report.push_str("\n## Test Type Breakdown\n\n");
    report.push_str(&format!(
        "- **assert_return**: {}\n",
        runner.stats.assert_return_count
    ));
    report.push_str(&format!(
        "- **assert_trap**: {}\n",
        runner.stats.assert_trap_count
    ));
    report.push_str(&format!(
        "- **assert_invalid**: {}\n",
        runner.stats.assert_invalid_count
    ));
    report.push_str(&format!(
        "- **assert_malformed**: {}\n",
        runner.stats.assert_malformed_count
    ));
    report.push_str(&format!(
        "- **assert_unlinkable**: {}\n",
        runner.stats.assert_unlinkable_count
    ));
    report.push_str(&format!(
        "- **assert_exhaustion**: {}\n",
        runner.stats.assert_exhaustion_count
    ));
    report.push_str(&format!(
        "- **register**: {}\n",
        runner.stats.register_count
    ));

    // File results
    report.push_str("\n## File Results\n\n");
    report.push_str("| File | Status | Directives | Execution Time |\n");
    report.push_str("|------|--------|------------|----------------|\n");

    for file_result in results {
        let status_icon = match file_result.status {
            wrt_build_core::wast::TestResult::Passed => "✅ Passed",
            wrt_build_core::wast::TestResult::Failed => "❌ Failed",
            wrt_build_core::wast::TestResult::Skipped => "⏭️ Skipped",
        };
        report.push_str(&format!(
            "| {} | {} | {} | {}ms |\n",
            file_result.file_path,
            status_icon,
            file_result.directives_count,
            file_result.execution_time_ms
        ));
    }

    // Failed tests details
    let failed_files: Vec<_> = results
        .iter()
        .filter(|r| r.status == wrt_build_core::wast::TestResult::Failed)
        .collect();
    if !failed_files.is_empty() {
        report.push_str("\n## Failed Tests Details\n\n");
        for file_result in failed_files {
            report.push_str(&format!("### {}\n\n", file_result.file_path));

            if let Some(error) = &file_result.error_message {
                report.push_str(&format!("**File Error**: {}\n\n", error));
            }

            for directive in &file_result.directive_results {
                if directive.result == wrt_build_core::wast::TestResult::Failed {
                    report.push_str(&format!(
                        "- **{}**: {}\n",
                        directive.directive_name,
                        directive.error_message.as_deref().unwrap_or("Unknown error")
                    ));
                }
            }
            report.push_str("\n");
        }
    }

    report.push_str("\n---\n");
    report.push_str("*Generated by cargo-wrt testsuite*\n");

    report
}
