//! Build matrix verification with root cause analysis
//!
//! This module provides comprehensive build matrix verification to ensure
//! all required build configurations work correctly and identifies
//! architectural issues that could impact ASIL compliance.

use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use chrono::Local;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::error::{BuildError, BuildResult};

/// Build configuration for matrix testing
#[derive(Debug, Clone)]
pub struct BuildConfiguration {
    /// Configuration name
    pub name: String,
    /// Package to build
    pub package: String,
    /// Features to enable
    pub features: Vec<String>,
    /// ASIL level
    pub asil_level: AsilLevel,
}

/// ASIL safety levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsilLevel {
    /// Core functionality
    Core,
    /// ASIL-B safety level
    AsilB,
    /// ASIL-C safety level
    AsilC,
    /// ASIL-D safety level (highest)
    AsilD,
    /// Development mode
    Development,
    /// Server deployment
    Server,
    /// Component model
    Component,
}

impl std::fmt::Display for AsilLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsilLevel::Core => write!(f, "Core"),
            AsilLevel::AsilB => write!(f, "ASIL-B"),
            AsilLevel::AsilC => write!(f, "ASIL-C"),
            AsilLevel::AsilD => write!(f, "ASIL-D"),
            AsilLevel::Development => write!(f, "Development"),
            AsilLevel::Server => write!(f, "Server"),
            AsilLevel::Component => write!(f, "Component"),
        }
    }
}

/// Architectural issue types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ArchitecturalIssue {
    /// Cyclic package dependencies
    CyclicDependency,
    /// Duplicate definitions
    DuplicateDefinitions,
    /// Trait bound violations
    TraitBounds,
    /// Missing imports or modules
    MissingImports,
    /// Coherence violations
    Coherence,
    /// Memory allocation issues
    MemoryAllocation,
    /// Conflicting std/no_std requirements
    StdConflict,
    /// Unsafe code in ASIL configuration
    UnsafeInAsil,
}

impl ArchitecturalIssue {
    fn description(&self) -> &'static str {
        match self {
            ArchitecturalIssue::CyclicDependency => {
                "Cyclic dependencies violating ASIL modular design principles"
            },
            ArchitecturalIssue::DuplicateDefinitions => {
                "Duplicate definitions indicating poor modularity"
            },
            ArchitecturalIssue::TraitBounds => {
                "Trait bound violations suggesting improper abstractions"
            },
            ArchitecturalIssue::MissingImports => {
                "Missing imports/modules breaking deterministic compilation"
            },
            ArchitecturalIssue::Coherence => {
                "Coherence violations requiring architectural refactoring"
            },
            ArchitecturalIssue::MemoryAllocation => {
                "Memory allocation issues for no_std environments"
            },
            ArchitecturalIssue::StdConflict => "Conflicting std/no_std requirements",
            ArchitecturalIssue::UnsafeInAsil => "Unsafe code in ASIL-critical configurations",
        }
    }
}

/// Result of a single configuration test
#[derive(Debug, Serialize)]
pub struct ConfigurationResult {
    /// Name of the configuration
    pub name: String,
    /// Package being tested
    pub package: String,
    /// Features enabled for this configuration
    pub features: Vec<String>,
    /// ASIL level for this configuration
    pub asil_level: AsilLevel,
    /// Whether the build passed
    pub build_passed: bool,
    /// Whether tests passed
    pub test_passed: bool,
    /// Whether ASIL compliance checks passed
    pub asil_check_passed: Option<bool>,
    /// Architectural issues found
    pub architectural_issues: Vec<ArchitecturalIssue>,
    /// Error output if any
    pub error_output: Option<String>,
}

/// Overall verification results
#[derive(Debug, Serialize)]
pub struct VerificationResults {
    /// Results for each configuration tested
    pub configurations: Vec<ConfigurationResult>,
    /// Whether all configurations passed
    pub all_passed: bool,
    /// Unique architectural issues found across all configurations
    pub architectural_issues: HashSet<ArchitecturalIssue>,
    /// Timestamp of the verification run
    pub timestamp: String,
    /// Result of Kani formal verification if run
    pub kani_result: Option<bool>,
}

/// Build matrix verifier
pub struct MatrixVerifier {
    configurations: Vec<BuildConfiguration>,
    verbose: bool,
}

impl MatrixVerifier {
    /// Create a new matrix verifier with default configurations
    pub fn new(verbose: bool) -> Self {
        let configurations = Self::default_configurations();
        Self {
            configurations,
            verbose,
        }
    }

    /// Create with custom configurations
    pub fn with_configurations(configurations: Vec<BuildConfiguration>, verbose: bool) -> Self {
        Self {
            configurations,
            verbose,
        }
    }

    /// Get default build configurations
    fn default_configurations() -> Vec<BuildConfiguration> {
        vec![
            // WRT Library Configurations
            BuildConfiguration {
                name: "WRT no_std + alloc".to_string(),
                package: "wrt".to_string(),
                features: vec!["alloc".to_string()],
                asil_level: AsilLevel::Core,
            },
            BuildConfiguration {
                name: "WRT ASIL-D (no_std + alloc)".to_string(),
                package: "wrt".to_string(),
                features: vec!["alloc".to_string(), "safety-asil-d".to_string()],
                asil_level: AsilLevel::AsilD,
            },
            BuildConfiguration {
                name: "WRT ASIL-C (no_std + alloc)".to_string(),
                package: "wrt".to_string(),
                features: vec!["alloc".to_string(), "safety-asil-c".to_string()],
                asil_level: AsilLevel::AsilC,
            },
            BuildConfiguration {
                name: "WRT ASIL-B (no_std + alloc)".to_string(),
                package: "wrt".to_string(),
                features: vec!["alloc".to_string(), "safety-asil-b".to_string()],
                asil_level: AsilLevel::AsilB,
            },
            BuildConfiguration {
                name: "WRT Development (std)".to_string(),
                package: "wrt".to_string(),
                features: vec!["std".to_string()],
                asil_level: AsilLevel::Development,
            },
            BuildConfiguration {
                name: "WRT Development with Optimization".to_string(),
                package: "wrt".to_string(),
                features: vec!["std".to_string(), "optimize".to_string()],
                asil_level: AsilLevel::Development,
            },
            BuildConfiguration {
                name: "WRT Server".to_string(),
                package: "wrt".to_string(),
                features: vec![
                    "std".to_string(),
                    "optimize".to_string(),
                    "platform".to_string(),
                ],
                asil_level: AsilLevel::Server,
            },
            // WRTD Binary Configurations
            BuildConfiguration {
                name: "WRTD ASIL-D Runtime".to_string(),
                package: "wrtd".to_string(),
                features: vec![
                    "safety-asil-d".to_string(),
                    "wrt-execution".to_string(),
                    "enable-panic-handler".to_string(),
                ],
                asil_level: AsilLevel::AsilD,
            },
            BuildConfiguration {
                name: "WRTD ASIL-C Runtime".to_string(),
                package: "wrtd".to_string(),
                features: vec![
                    "safety-asil-c".to_string(),
                    "wrt-execution".to_string(),
                    "enable-panic-handler".to_string(),
                ],
                asil_level: AsilLevel::AsilC,
            },
            BuildConfiguration {
                name: "WRTD ASIL-B Runtime".to_string(),
                package: "wrtd".to_string(),
                features: vec![
                    "safety-asil-b".to_string(),
                    "wrt-execution".to_string(),
                    "asil-b-panic".to_string(),
                ],
                asil_level: AsilLevel::AsilB,
            },
            BuildConfiguration {
                name: "WRTD Development Runtime".to_string(),
                package: "wrtd".to_string(),
                features: vec![
                    "std".to_string(),
                    "wrt-execution".to_string(),
                    "dev-panic".to_string(),
                ],
                asil_level: AsilLevel::Development,
            },
            BuildConfiguration {
                name: "WRTD Server Runtime".to_string(),
                package: "wrtd".to_string(),
                features: vec!["std".to_string(), "wrt-execution".to_string()],
                asil_level: AsilLevel::Server,
            },
            // Component Model Tests
            BuildConfiguration {
                name: "Component Model Core".to_string(),
                package: "wrt-component".to_string(),
                features: vec![
                    "no_std".to_string(),
                    "alloc".to_string(),
                    "component-model-core".to_string(),
                ],
                asil_level: AsilLevel::Component,
            },
            BuildConfiguration {
                name: "Component Model Full".to_string(),
                package: "wrt-component".to_string(),
                features: vec!["std".to_string(), "component-model-all".to_string()],
                asil_level: AsilLevel::Component,
            },
        ]
    }

    /// Run the full verification matrix
    pub fn run_verification(&self) -> BuildResult<VerificationResults> {
        println!("{} Starting Build Matrix Verification", "🔍".bright_blue());
        println!();

        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut all_architectural_issues = HashSet::new();
        let mut all_passed = true;

        for config in &self.configurations {
            let result = self.test_configuration(config)?;

            if !result.build_passed || !result.test_passed {
                all_passed = false;
            }

            for issue in &result.architectural_issues {
                all_architectural_issues.insert(issue.clone());
            }

            results.push(result);
        }

        // Run Kani verification if available
        let kani_result = self.run_kani_verification();

        let verification_results = VerificationResults {
            configurations: results,
            all_passed,
            architectural_issues: all_architectural_issues,
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            kani_result,
        };

        let duration = start_time.elapsed();
        println!();
        println!(
            "{} Verification completed in {:.2}s",
            "✅".bright_green(),
            duration.as_secs_f64()
        );

        Ok(verification_results)
    }

    /// Test a single configuration
    fn test_configuration(&self, config: &BuildConfiguration) -> BuildResult<ConfigurationResult> {
        println!("{} Testing: {}", "📦".bright_blue(), config.name);

        // Clean build directory for accurate testing
        let _ = Command::new("cargo").arg("clean").arg("-p").arg(&config.package).output();

        // Build test
        print!("  Building... ");
        std::io::stdout().flush().unwrap();

        let mut build_cmd = Command::new("cargo");
        build_cmd.arg("build").arg("-p").arg(&config.package);

        if !config.features.is_empty() {
            build_cmd.arg("--features").arg(config.features.join(","));
        }

        let build_output = build_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute build: {}", e)))?;

        let build_passed = build_output.status.success();
        let mut architectural_issues = Vec::new();
        let mut error_output = None;

        if build_passed {
            println!("{}", "✓".bright_green());
        } else {
            println!("{}", "✗".bright_red());
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            error_output = Some(stderr.to_string());
            architectural_issues = self.analyze_failure(&config.name, &stderr, &config.features);
        }

        // Test execution
        print!("  Testing... ");
        std::io::stdout().flush().unwrap();

        let mut test_cmd = Command::new("cargo");
        test_cmd.arg("test").arg("-p").arg(&config.package);

        if !config.features.is_empty() {
            test_cmd.arg("--features").arg(config.features.join(","));
        }

        test_cmd.arg("--").arg("--nocapture");

        let test_output = test_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute tests: {}", e)))?;

        let test_passed = test_output.status.success();

        if test_passed {
            println!("{}", "✓".bright_green());
        } else {
            println!("{}", "✗".bright_red());
            if error_output.is_none() {
                let stderr = String::from_utf8_lossy(&test_output.stderr);
                error_output = Some(stderr.to_string());
                let test_issues = self.analyze_failure(
                    &format!("{} - Tests", config.name),
                    &stderr,
                    &config.features,
                );
                for issue in test_issues {
                    if !architectural_issues.contains(&issue) {
                        architectural_issues.push(issue);
                    }
                }
            }
        }

        // ASIL-specific checks
        let asil_check_passed = if matches!(config.asil_level, AsilLevel::AsilC | AsilLevel::AsilD)
        {
            print!("  ASIL compliance check... ");
            std::io::stdout().flush().unwrap();

            let check_result = self.check_asil_compliance(&config.package, &config.features);
            match check_result {
                Ok(has_unsafe) => {
                    if has_unsafe {
                        println!("{}", "⚠".bright_yellow());
                        architectural_issues.push(ArchitecturalIssue::UnsafeInAsil);
                        Some(false)
                    } else {
                        println!("{}", "✓".bright_green());
                        Some(true)
                    }
                },
                Err(_) => {
                    println!("{}", "?".bright_yellow());
                    None
                },
            }
        } else {
            None
        };

        Ok(ConfigurationResult {
            name: config.name.clone(),
            package: config.package.clone(),
            features: config.features.clone(),
            asil_level: config.asil_level.clone(),
            build_passed,
            test_passed,
            asil_check_passed,
            architectural_issues,
            error_output,
        })
    }

    /// Analyze build/test failures for architectural issues
    fn analyze_failure(
        &self,
        config_name: &str,
        error_output: &str,
        features: &[String],
    ) -> Vec<ArchitecturalIssue> {
        let mut issues = Vec::new();

        // Check for common architectural problems
        if error_output.contains("cyclic package dependency") {
            issues.push(ArchitecturalIssue::CyclicDependency);
        }

        if error_output.contains("multiple definitions")
            || error_output.contains("duplicate definitions")
        {
            issues.push(ArchitecturalIssue::DuplicateDefinitions);
        }

        if error_output.contains("trait bound") && error_output.contains("not satisfied")
            || error_output.contains("the trait") && error_output.contains("is not implemented")
        {
            issues.push(ArchitecturalIssue::TraitBounds);
        }

        if error_output.contains("cannot find") && error_output.contains("in scope")
            || error_output.contains("unresolved import")
        {
            issues.push(ArchitecturalIssue::MissingImports);
        }

        if error_output.contains("conflicting implementations")
            || error_output.contains("coherence")
        {
            issues.push(ArchitecturalIssue::Coherence);
        }

        if error_output.contains("memory allocation")
            || error_output.contains("alloc") && error_output.contains("not found")
        {
            issues.push(ArchitecturalIssue::MemoryAllocation);
        }

        // Feature interaction analysis
        let has_no_std = features.iter().any(|f| f.contains("no_std"));
        let has_std = features.iter().any(|f| f == "std");

        if has_no_std && has_std {
            issues.push(ArchitecturalIssue::StdConflict);
        }

        if self.verbose && !issues.is_empty() {
            println!(
                "  {} Architectural issues detected in {}: {:?}",
                "⚠".bright_yellow(),
                config_name,
                issues
            );
        }

        issues
    }

    /// Check ASIL compliance (looking for unsafe code)
    fn check_asil_compliance(&self, package: &str, features: &[String]) -> BuildResult<bool> {
        let mut cmd = Command::new("cargo");
        cmd.arg("check").arg("-p").arg(package).arg("--message-format=json");

        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run cargo check: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_unsafe = stdout.contains("unsafe");

        Ok(has_unsafe)
    }

    /// Run Kani formal verification if available
    fn run_kani_verification(&self) -> Option<bool> {
        // Check if cargo-kani is available
        let check_kani = Command::new("cargo").arg("kani").arg("--version").output();

        if check_kani.is_err() || !check_kani.unwrap().status.success() {
            return None;
        }

        println!();
        println!("{} Running Kani Verification", "🔬".bright_blue());

        let output = Command::new("cargo")
            .arg("kani")
            .arg("-p")
            .arg("wrt")
            .arg("--features")
            .arg("no_std,alloc,kani,safety-asil-d")
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    println!("{} Kani verification passed", "✓".bright_green());
                    Some(true)
                } else {
                    println!("{} Kani verification failed", "✗".bright_red());
                    Some(false)
                }
            },
            Err(_) => {
                println!("{} Kani verification error", "⚠".bright_yellow());
                None
            },
        }
    }

    /// Generate verification report
    pub fn generate_report(
        &self,
        results: &VerificationResults,
        output_dir: &Path,
    ) -> BuildResult<()> {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| BuildError::Tool(format!("Failed to create output directory: {}", e)))?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let report_path = output_dir.join(format!("BUILD_VERIFICATION_REPORT_{}.md", timestamp));
        let issues_path = output_dir.join(format!("ARCHITECTURAL_ISSUES_{}.md", timestamp));

        // Generate main report
        self.write_main_report(results, &report_path)?;

        // Generate architectural issues report if needed
        if !results.architectural_issues.is_empty() {
            self.write_issues_report(results, &issues_path)?;
        }

        println!();
        println!("Reports generated:");
        println!("  - {}", report_path.display());
        if !results.architectural_issues.is_empty() {
            println!("  - {}", issues_path.display());
        }

        Ok(())
    }

    /// Write main verification report
    fn write_main_report(&self, results: &VerificationResults, path: &Path) -> BuildResult<()> {
        let mut file = std::fs::File::create(path)
            .map_err(|e| BuildError::Tool(format!("Failed to create report file: {}", e)))?;

        writeln!(file, "# Build Matrix Verification Report")?;
        writeln!(file, "Date: {}", results.timestamp)?;
        writeln!(file)?;

        for config_result in &results.configurations {
            writeln!(file, "## Configuration: {}", config_result.name)?;
            writeln!(file, "- Package: {}", config_result.package)?;
            writeln!(file, "- Features: {}", config_result.features.join(", "))?;
            writeln!(file, "- ASIL Level: {}", config_result.asil_level)?;
            writeln!(file)?;

            writeln!(
                file,
                "{} Build: {}",
                if config_result.build_passed { "✅" } else { "❌" },
                if config_result.build_passed { "PASSED" } else { "FAILED" }
            )?;

            writeln!(
                file,
                "{} Tests: {}",
                if config_result.test_passed { "✅" } else { "❌" },
                if config_result.test_passed { "PASSED" } else { "FAILED" }
            )?;

            if let Some(asil_passed) = config_result.asil_check_passed {
                writeln!(
                    file,
                    "{} ASIL Check: {}",
                    if asil_passed { "✅" } else { "⚠️" },
                    if asil_passed { "No unsafe code" } else { "Unsafe code detected" }
                )?;
            }

            if !config_result.architectural_issues.is_empty() {
                writeln!(file, "⚠️ Architectural issues detected")?;
            }

            writeln!(file)?;
        }

        if let Some(kani_passed) = results.kani_result {
            writeln!(file, "## Kani Formal Verification")?;
            writeln!(
                file,
                "{} Kani: {}",
                if kani_passed { "✅" } else { "❌" },
                if kani_passed { "PASSED" } else { "FAILED" }
            )?;
            writeln!(file)?;
        }

        writeln!(file, "# Summary")?;
        writeln!(file)?;
        if results.all_passed {
            writeln!(file, "✅ **All configurations passed successfully**")?;
        } else {
            writeln!(file, "❌ **Some configurations failed**")?;
        }

        Ok(())
    }

    /// Write architectural issues report
    fn write_issues_report(&self, results: &VerificationResults, path: &Path) -> BuildResult<()> {
        let mut file = std::fs::File::create(path)
            .map_err(|e| BuildError::Tool(format!("Failed to create issues file: {}", e)))?;

        writeln!(file, "# Architectural Issues Analysis")?;
        writeln!(file, "Date: {}", results.timestamp)?;
        writeln!(file)?;

        // Detailed analysis for each failed configuration
        for config_result in &results.configurations {
            if !config_result.architectural_issues.is_empty() {
                writeln!(file, "## Analyzing failure for: {}", config_result.name)?;
                writeln!(file, "Features: {}", config_result.features.join(", "))?;
                writeln!(file)?;

                for issue in &config_result.architectural_issues {
                    match issue {
                        ArchitecturalIssue::CyclicDependency => {
                            writeln!(
                                file,
                                "### ⚠️ ARCHITECTURAL ISSUE: Cyclic Dependencies Detected"
                            )?;
                            writeln!(
                                file,
                                "This violates ASIL principles of modular design and clear \
                                 dependency hierarchy."
                            )?;
                        },
                        ArchitecturalIssue::DuplicateDefinitions => {
                            writeln!(file, "### ⚠️ ARCHITECTURAL ISSUE: Duplicate Definitions")?;
                            writeln!(
                                file,
                                "This indicates poor modularity and could lead to undefined \
                                 behavior in safety-critical systems."
                            )?;
                        },
                        ArchitecturalIssue::TraitBounds => {
                            writeln!(file, "### ⚠️ ARCHITECTURAL ISSUE: Trait Bound Violations")?;
                            writeln!(
                                file,
                                "Feature combinations are creating incompatible trait \
                                 requirements."
                            )?;
                            writeln!(
                                file,
                                "This suggests improper abstraction boundaries for ASIL \
                                 compliance."
                            )?;
                        },
                        ArchitecturalIssue::MissingImports => {
                            writeln!(file, "### ⚠️ ARCHITECTURAL ISSUE: Missing Imports/Modules")?;
                            writeln!(
                                file,
                                "Feature flags are not properly managing code visibility."
                            )?;
                            writeln!(
                                file,
                                "This violates ASIL requirement for deterministic compilation."
                            )?;
                        },
                        ArchitecturalIssue::Coherence => {
                            writeln!(file, "### ⚠️ ARCHITECTURAL ISSUE: Coherence Violations")?;
                            writeln!(
                                file,
                                "Multiple implementations conflict, indicating poor separation of \
                                 concerns."
                            )?;
                            writeln!(file, "ASIL-D requires single, unambiguous implementations.")?;
                        },
                        ArchitecturalIssue::MemoryAllocation => {
                            writeln!(
                                file,
                                "### ⚠️ ARCHITECTURAL ISSUE: Memory Allocation Problems"
                            )?;
                            writeln!(
                                file,
                                "Memory allocation strategy is not properly abstracted for no_std \
                                 environments."
                            )?;
                            writeln!(file, "Critical for ASIL-D compliance in embedded systems.")?;
                        },
                        ArchitecturalIssue::StdConflict => {
                            writeln!(file, "### ⚠️ ARCHITECTURAL ISSUE: std/no_std Conflict")?;
                            writeln!(file, "Conflicting standard library requirements detected.")?;
                        },
                        ArchitecturalIssue::UnsafeInAsil => {
                            writeln!(
                                file,
                                "### ⚠️ ARCHITECTURAL ISSUE: Unsafe Code in ASIL Configuration"
                            )?;
                            writeln!(
                                file,
                                "Unsafe code detected in safety-critical configuration."
                            )?;
                        },
                    }
                    writeln!(file)?;
                }

                if let Some(error_output) = &config_result.error_output {
                    writeln!(file, "### Raw Error Output")?;
                    writeln!(file, "```")?;
                    let lines: Vec<&str> = error_output.lines().take(100).collect();
                    for line in lines {
                        writeln!(file, "{}", line)?;
                    }
                    writeln!(file, "```")?;
                    writeln!(file)?;
                }
            }
        }

        // Summary
        writeln!(file, "# Architectural Issues Summary")?;
        writeln!(file)?;

        for issue in &results.architectural_issues {
            writeln!(file, "- {}", issue.description())?;
        }

        writeln!(file)?;
        writeln!(file, "## Recommended Actions")?;
        writeln!(file, "1. Review module boundaries and dependencies")?;
        writeln!(
            file,
            "2. Ensure feature flags properly isolate platform-specific code"
        )?;
        writeln!(
            file,
            "3. Verify all ASIL configurations can build without std library"
        )?;
        writeln!(
            file,
            "4. Remove or properly abstract unsafe code in safety-critical paths"
        )?;

        Ok(())
    }

    /// Print summary to console
    pub fn print_summary(&self, results: &VerificationResults) {
        println!();
        println!("{}", "=== Verification Summary ===".bright_blue());

        if results.all_passed {
            println!("{} All configurations passed!", "✅".bright_green());
        } else {
            println!("{} Some configurations failed!", "❌".bright_red());
        }

        if !results.architectural_issues.is_empty() {
            println!();
            println!("{}", "=== Architectural Issues Detected ===".bright_red());
            for issue in &results.architectural_issues {
                println!("- {}", issue.description());
            }
        }
    }
}

impl std::fmt::Display for ArchitecturalIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
