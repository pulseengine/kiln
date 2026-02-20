//! KANI formal verification and report generation
//!
//! This module provides comprehensive KANI formal verification support for WRT,
//! including verification execution, report generation, and coverage analysis.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use chrono::Local;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::{
    config::AsilLevel,
    error::{BuildError, BuildResult},
};

/// KANI verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KaniConfig {
    /// ASIL profile for verification
    pub profile: AsilLevel,
    /// Specific package to verify (None = all packages)
    pub package: Option<String>,
    /// Specific harness to run (None = all harnesses)
    pub harness: Option<String>,
    /// Enable verbose output
    pub verbose: bool,
    /// Additional KANI arguments
    pub extra_args: Vec<String>,
}

impl Default for KaniConfig {
    fn default() -> Self {
        Self {
            profile: AsilLevel::C,
            package: None,
            harness: None,
            verbose: false,
            extra_args: Vec::new(),
        }
    }
}

/// KANI verification results for a single package
#[derive(Debug, Serialize)]
pub struct PackageVerificationResult {
    /// Package name
    pub package: String,
    /// Whether verification passed
    pub passed: bool,
    /// Total number of checks
    pub total_checks: usize,
    /// Number of passed checks
    pub passed_checks: usize,
    /// Verification duration in milliseconds
    pub duration_ms: u64,
    /// Failure messages
    pub failures: Vec<String>,
    /// Log file path
    pub log_file: PathBuf,
    /// Raw output
    pub output: String,
}

/// Complete KANI verification results
#[derive(Debug, Serialize)]
pub struct KaniVerificationResults {
    /// Timestamp of verification
    pub timestamp: String,
    /// ASIL profile used
    pub profile: AsilLevel,
    /// System information
    pub system_info: String,
    /// Results per package
    pub package_results: Vec<PackageVerificationResult>,
    /// Overall statistics
    pub total_packages: usize,
    /// Number of packages that passed
    pub passed_packages: usize,
    /// Success rate percentage
    pub success_rate: f64,
    /// Report file path
    pub report_file: PathBuf,
    /// Coverage report (if generated)
    pub coverage_report: Option<String>,
}

/// KANI verifier
pub struct KaniVerifier {
    workspace_root: PathBuf,
    config: KaniConfig,
    report_dir: PathBuf,
    timestamp: String,
}

impl KaniVerifier {
    /// Create a new KANI verifier
    pub fn new(workspace_root: PathBuf, config: KaniConfig) -> Self {
        let report_dir = workspace_root.join("target").join("kani-reports");
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

        Self {
            workspace_root,
            config,
            report_dir,
            timestamp,
        }
    }

    /// Run KANI verification
    pub fn run_verification(&self) -> BuildResult<KaniVerificationResults> {
        println!("{} WRT KANI Formal Verification", "🔍".bright_blue());
        println!("Profile: {:?}", self.config.profile);
        println!("Timestamp: {}", self.timestamp);
        println!();

        // Create report directory
        fs::create_dir_all(&self.report_dir)
            .map_err(|e| BuildError::Tool(format!("Failed to create report directory: {}", e)))?;

        let start_time = Instant::now();
        let mut package_results = Vec::new();

        if let Some(ref package) = self.config.package {
            // Verify specific package
            let result = self.run_kani_package(package)?;
            package_results.push(result);
        } else {
            // Verify all configured packages
            let packages = self.get_kani_packages()?;
            for package in packages {
                let result = self.run_kani_package(&package)?;
                package_results.push(result);
            }
        }

        let total_packages = package_results.len();
        let passed_packages = package_results.iter().filter(|r| r.passed).count();
        let success_rate = if total_packages > 0 {
            (passed_packages as f64 / total_packages as f64) * 100.0
        } else {
            0.0
        };

        // Generate main report
        let report_file = self.generate_report(
            &package_results,
            total_packages,
            passed_packages,
            success_rate,
        )?;

        // Generate coverage report for ASIL-D
        let coverage_report = if self.config.profile == AsilLevel::D {
            self.generate_coverage_report(&package_results).ok()
        } else {
            None
        };

        let duration = start_time.elapsed();
        println!();
        println!(
            "{} Verification completed in {:.2}s",
            "✅".bright_green(),
            duration.as_secs_f64()
        );
        println!("Report saved to: {}", report_file.display());

        if passed_packages == total_packages {
            println!("{} All packages passed verification!", "🎉".bright_green());
        } else {
            println!(
                "{} {}/{} packages failed verification",
                "⚠️".bright_yellow(),
                total_packages - passed_packages,
                total_packages
            );
        }

        Ok(KaniVerificationResults {
            timestamp: self.timestamp.clone(),
            profile: self.config.profile,
            system_info: self.get_system_info(),
            package_results,
            total_packages,
            passed_packages,
            success_rate,
            report_file,
            coverage_report,
        })
    }

    /// Run KANI on a specific package
    fn run_kani_package(&self, package: &str) -> BuildResult<PackageVerificationResult> {
        println!(
            "{} Verifying package: {}",
            "📦".bright_yellow(),
            package.bright_cyan()
        );

        let start_time = Instant::now();

        // Check if package is configured for KANI
        if !self.is_package_configured(package)? {
            return Err(BuildError::Tool(format!(
                "Package {} not configured for KANI",
                package
            )));
        }

        // Build KANI arguments
        let mut args = vec!["kani".to_string(), "-p".to_string(), package.to_string()];
        args.push("--tests".to_string());

        if let Some(ref harness) = self.config.harness {
            args.extend_from_slice(&["--harness".to_string(), harness.clone()]);
        }

        if self.config.verbose {
            args.push("--verbose".to_string());
        }

        // Add profile-specific arguments
        match self.config.profile {
            AsilLevel::D => {
                args.extend_from_slice(&[
                    "--enable-unstable".to_string(),
                    "--solver".to_string(),
                    "cadical".to_string(),
                ]);
            },
            AsilLevel::C | AsilLevel::B => {
                args.extend_from_slice(&["--solver".to_string(), "cadical".to_string()]);
            },
            _ => {
                args.extend_from_slice(&["--solver".to_string(), "minisat".to_string()]);
            },
        }

        // Add extra arguments
        args.extend_from_slice(&self.config.extra_args);

        println!("Running: cargo {}", args.join(" "));

        // Execute KANI
        let output = Command::new("cargo")
            .args(&args[1..]) // Skip "kani" since we're calling cargo directly
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run KANI: {}", e)))?;

        let output_string = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr).to_string();

        // Save output to log file
        let log_file = self.report_dir.join(format!("{}_{}.log", package, self.timestamp));
        fs::write(&log_file, &output_string)
            .map_err(|e| BuildError::Tool(format!("Failed to write log file: {}", e)))?;

        // Parse results
        let passed = output.status.success();
        let total_checks = self.count_checks(&output_string);
        let passed_checks = self.count_passed_checks(&output_string);
        let failures = if !passed { self.extract_failures(&output_string) } else { Vec::new() };

        let duration = start_time.elapsed();

        if passed {
            println!("  {} {} verification passed", "✓".bright_green(), package);
        } else {
            println!("  {} {} verification failed", "✗".bright_red(), package);
        }

        Ok(PackageVerificationResult {
            package: package.to_string(),
            passed,
            total_checks,
            passed_checks,
            duration_ms: duration.as_millis() as u64,
            failures,
            log_file,
            output: output_string,
        })
    }

    /// Get packages configured for KANI
    fn get_kani_packages(&self) -> BuildResult<Vec<String>> {
        let cargo_toml_path = self.workspace_root.join("Cargo.toml");
        let content = fs::read_to_string(&cargo_toml_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read Cargo.toml: {}", e)))?;

        let mut packages = Vec::new();
        let mut in_kani_section = false;

        for line in content.lines() {
            if line.contains("[[workspace.metadata.kani.package]]") {
                in_kani_section = true;
                continue;
            }

            if in_kani_section {
                if line.starts_with('[') && !line.contains("workspace.metadata.kani") {
                    in_kani_section = false;
                    continue;
                }

                if let Some(name_match) = line.strip_prefix("name = ") {
                    let name = name_match.trim_matches('"');
                    packages.push(name.to_string());
                    in_kani_section = false;
                }
            }
        }

        if packages.is_empty() {
            // Fallback: find packages with "kani" feature
            packages = vec![
                "wrt-foundation".to_string(),
                "wrt-component".to_string(),
                "wrt-runtime".to_string(),
                "wrt-host".to_string(),
            ];
        }

        Ok(packages)
    }

    /// Check if package is configured for KANI
    fn is_package_configured(&self, package: &str) -> BuildResult<bool> {
        let cargo_toml_path = self.workspace_root.join("Cargo.toml");
        let content = fs::read_to_string(&cargo_toml_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read Cargo.toml: {}", e)))?;

        Ok(content.contains(&format!("name = \"{}\"", package)))
    }

    /// Count total checks in KANI output
    fn count_checks(&self, output: &str) -> usize {
        output
            .lines()
            .filter(|line| line.contains("VERIFICATION:") && line.contains("CHECK"))
            .count()
    }

    /// Count passed checks in KANI output
    fn count_passed_checks(&self, output: &str) -> usize {
        output
            .lines()
            .filter(|line| line.contains("VERIFICATION:") && line.contains("SUCCESS"))
            .count()
    }

    /// Extract failure messages from KANI output
    fn extract_failures(&self, output: &str) -> Vec<String> {
        output
            .lines()
            .filter(|line| line.contains("VERIFICATION:") && line.contains("FAILURE"))
            .map(|line| line.to_string())
            .collect()
    }

    /// Generate markdown report
    fn generate_report(
        &self,
        package_results: &[PackageVerificationResult],
        total_packages: usize,
        passed_packages: usize,
        success_rate: f64,
    ) -> BuildResult<PathBuf> {
        let report_file =
            self.report_dir.join(format!("verification_report_{}.md", self.timestamp));

        let mut content = format!(
            r#"# WRT KANI Formal Verification Report

**Date**: {}  
**Profile**: {:?}  
**System**: {}  

## Summary

"#,
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            self.config.profile,
            self.get_system_info()
        );

        // Add package results
        for result in package_results {
            content.push_str(&format!(
                r#"### {}

**Status**: {}  
**Checks**: {}/{} passed  
**Duration**: {}ms  

"#,
                result.package,
                if result.passed { "✅ PASSED" } else { "❌ FAILED" },
                result.passed_checks,
                result.total_checks,
                result.duration_ms
            ));

            if !result.failures.is_empty() {
                content.push_str("**Failures**:\n");
                for failure in &result.failures {
                    content.push_str(&format!("- {}\n", failure));
                }
                content.push('\n');
            }
        }

        // Add overall results
        content.push_str(&format!(
            r#"## Overall Results

**Total Packages**: {}  
**Passed**: {}  
**Failed**: {}  
**Success Rate**: {:.1}%  
"#,
            total_packages,
            passed_packages,
            total_packages - passed_packages,
            success_rate
        ));

        fs::write(&report_file, content)
            .map_err(|e| BuildError::Tool(format!("Failed to write report: {}", e)))?;

        Ok(report_file)
    }

    /// Generate coverage report for ASIL-D
    fn generate_coverage_report(
        &self,
        package_results: &[PackageVerificationResult],
    ) -> BuildResult<String> {
        println!("{} Generating coverage report...", "📊".bright_blue());

        let coverage_file = self.report_dir.join(format!("coverage_{}.txt", self.timestamp));

        // Check if kani-cov is available
        let kani_cov_output = Command::new("kani-cov").arg("--version").output();

        if kani_cov_output.is_err() {
            println!(
                "  {} kani-cov not available, skipping coverage analysis",
                "⚠️".bright_yellow()
            );
            return Ok("Coverage analysis skipped - kani-cov not available".to_string());
        }

        // Collect log files
        let log_files: Vec<_> = package_results
            .iter()
            .map(|r| r.log_file.to_string_lossy().to_string())
            .collect();

        if log_files.is_empty() {
            return Ok("No log files available for coverage analysis".to_string());
        }

        // Run kani-cov
        let output = Command::new("kani-cov")
            .args(&log_files)
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run kani-cov: {}", e)))?;

        let coverage_content = String::from_utf8_lossy(&output.stdout).to_string();

        fs::write(&coverage_file, &coverage_content)
            .map_err(|e| BuildError::Tool(format!("Failed to write coverage report: {}", e)))?;

        println!(
            "  {} Coverage report saved to: {}",
            "✓".bright_green(),
            coverage_file.display()
        );

        Ok(coverage_content)
    }

    /// Get system information
    fn get_system_info(&self) -> String {
        let output = Command::new("uname")
            .arg("-a")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown system".to_string());

        output
    }

    /// Print verification summary
    pub fn print_summary(&self, results: &KaniVerificationResults) {
        println!();
        println!("{}", "=== Verification Summary ===".bright_blue());
        println!("Profile: {:?}", results.profile);
        println!("Total packages: {}", results.total_packages);
        println!("Passed: {}", results.passed_packages);
        println!(
            "Failed: {}",
            results.total_packages - results.passed_packages
        );
        println!("Success rate: {:.1}%", results.success_rate);

        if results.passed_packages == results.total_packages {
            println!("{} All verifications passed!", "🎉".bright_green());
        } else {
            println!("{} Some verifications failed", "⚠️".bright_yellow());
        }

        println!();
        println!("Report: {}", results.report_file.display());
        if let Some(ref coverage) = results.coverage_report {
            println!("Coverage analysis completed");
        }
    }
}

/// Check if KANI is available
pub fn is_kani_available() -> bool {
    use crate::tools::ToolManager;

    let manager = ToolManager::new();
    manager.check_tool("kani").available
}

/// Get KANI version information
pub fn get_kani_version() -> BuildResult<String> {
    let output = Command::new("kani")
        .arg("--version")
        .output()
        .map_err(|e| BuildError::Tool(format!("Failed to get KANI version: {}", e)))?;

    if !output.status.success() {
        return Err(BuildError::Tool("KANI not available".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string())
}
