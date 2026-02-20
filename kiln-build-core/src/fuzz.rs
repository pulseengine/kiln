//! Fuzzing support for WRT components
//!
//! Provides integration with cargo-fuzz for comprehensive fuzzing
//! of WebAssembly parsing, component models, and runtime operations.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use colored::Colorize;

use crate::{
    build::BuildSystem,
    error::{BuildError, BuildResult},
};

/// Fuzzing configuration options
#[derive(Debug, Clone)]
pub struct FuzzOptions {
    /// Duration to run each fuzzer (in seconds)
    pub duration: u64,
    /// Number of worker threads
    pub workers: usize,
    /// Maximum number of runs (overrides duration if set)
    pub runs: Option<u64>,
    /// Specific targets to run (if empty, runs all)
    pub targets: Vec<String>,
    /// Generate corpus coverage report
    pub coverage: bool,
}

impl Default for FuzzOptions {
    fn default() -> Self {
        Self {
            duration: 60,
            workers: 4,
            runs: None,
            targets: Vec::new(),
            coverage: false,
        }
    }
}

/// Fuzzing results
#[derive(Debug)]
pub struct FuzzResults {
    /// Whether fuzzing completed successfully
    pub success: bool,
    /// List of targets that were run
    pub targets_run: Vec<String>,
    /// List of targets that found crashes
    pub crashed_targets: Vec<String>,
    /// Total duration of fuzzing
    pub duration_ms: u64,
    /// Detailed fuzzing report
    pub report: String,
}

/// List available fuzzing targets implementation
pub fn list_fuzz_targets_impl(build_system: &BuildSystem) -> BuildResult<Vec<String>> {
    let mut targets = Vec::new();

    // Look for fuzz directories in workspace crates
    for crate_path in build_system.workspace.crate_paths() {
        let fuzz_dir = crate_path.join("fuzz");
        if fuzz_dir.exists() {
            if let Ok(fuzz_targets) = discover_fuzz_targets_impl(&fuzz_dir) {
                targets.extend(fuzz_targets);
            }
        }
    }

    Ok(targets)
}

/// Run fuzzing with options implementation
pub fn run_fuzz_with_options_impl(
    build_system: &BuildSystem,
    options: &FuzzOptions,
) -> BuildResult<FuzzResults> {
    println!("{} Starting fuzzing campaign...", "🐛".bright_blue());

    // Check if cargo-fuzz is available with helpful error message
    use crate::tools::ensure_tool_available;
    ensure_tool_available("cargo-fuzz", "fuzz")?;

    let start_time = std::time::Instant::now();
    let mut targets_run = Vec::new();
    let mut crashed_targets = Vec::new();
    let mut success = true;

    // Determine targets to run
    let all_targets = list_fuzz_targets_impl(build_system)?;
    let targets_to_run =
        if options.targets.is_empty() { all_targets } else { options.targets.clone() };

    if targets_to_run.is_empty() {
        return Err(BuildError::Tool(
            "No fuzzing targets found. Run 'cargo fuzz init' to set up fuzzing.".to_string(),
        ));
    }

    println!("Configuration:");
    println!("  Duration per target: {}s", options.duration);
    println!("  Workers: {}", options.workers);
    if let Some(runs) = options.runs {
        println!("  Runs: {}", runs);
    }
    println!("  Targets: {}", targets_to_run.len());
    println!();

    // Run each fuzzing target
    for target in &targets_to_run {
        println!("{} Running fuzzer: {}", "🎯".bright_yellow(), target);

        match run_single_fuzz_target_impl(build_system, target, options) {
            Ok(target_success) => {
                targets_run.push(target.clone());
                if !target_success {
                    crashed_targets.push(target.clone());
                    success = false;
                    println!("{} {} found crashes or failed", "⚠️".bright_red(), target);
                } else {
                    println!("{} {} completed successfully", "✅".bright_green(), target);
                }
            },
            Err(e) => {
                println!("{} {} failed to run: {}", "❌".bright_red(), target, e);
                targets_run.push(target.clone());
                crashed_targets.push(target.clone());
                success = false;
            },
        }
        println!();
    }

    let duration = start_time.elapsed();

    // Generate summary
    if success {
        println!(
            "{} Fuzzing campaign completed successfully!",
            "✅".bright_green()
        );
    } else {
        println!(
            "{} Fuzzing found issues in {} targets",
            "⚠️".bright_yellow(),
            crashed_targets.len()
        );
    }

    // Generate report
    let report = generate_fuzz_report_impl(&targets_run, &crashed_targets, duration)?;

    Ok(FuzzResults {
        success,
        targets_run,
        crashed_targets,
        duration_ms: duration.as_millis() as u64,
        report,
    })
}

/// Run a single fuzzing target implementation
fn run_single_fuzz_target_impl(
    build_system: &BuildSystem,
    target: &str,
    options: &FuzzOptions,
) -> BuildResult<bool> {
    // Find the crate containing this fuzz target
    let fuzz_dir = find_fuzz_target_dir_impl(build_system, target)?;

    let mut cmd = Command::new("cargo");
    cmd.arg("+nightly")
        .arg("fuzz")
        .arg("run")
        .arg(target)
        .arg("--")
        .arg(format!("-workers={}", options.workers))
        .current_dir(&fuzz_dir);

    // Add duration or runs limit
    if let Some(runs) = options.runs {
        cmd.arg(format!("-runs={}", runs));
    } else {
        cmd.arg(format!("-max_total_time={}", options.duration));
    }

    let output = cmd
        .output()
        .map_err(|e| BuildError::Tool(format!("Failed to run fuzzer {}: {}", target, e)))?;

    // Check if crashes were found
    let artifacts_dir = fuzz_dir.join("artifacts").join(target);
    let has_crashes = artifacts_dir.exists()
        && artifacts_dir.read_dir().map(|entries| entries.count() > 0).unwrap_or(false);

    Ok(output.status.success() && !has_crashes)
}

/// Check if cargo-fuzz is available implementation
fn is_cargo_fuzz_available_impl() -> BuildResult<bool> {
    use crate::tools::ensure_tool_available;

    match ensure_tool_available("cargo-fuzz", "fuzz") {
        Ok(()) => Ok(true),
        Err(_) => Ok(false), // Convert error to boolean for this function
    }
}

/// Discover fuzz targets in a fuzz directory implementation
fn discover_fuzz_targets_impl(fuzz_dir: &Path) -> BuildResult<Vec<String>> {
    let mut targets = Vec::new();

    let fuzz_targets_dir = fuzz_dir.join("fuzz_targets");
    if !fuzz_targets_dir.exists() {
        return Ok(targets);
    }

    let entries = std::fs::read_dir(&fuzz_targets_dir)
        .map_err(|e| BuildError::Tool(format!("Failed to read fuzz_targets directory: {}", e)))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| BuildError::Tool(format!("Failed to read directory entry: {}", e)))?;

        if let Some(file_name) = entry.file_name().to_str() {
            if file_name.ends_with(".rs") {
                let target_name = file_name.trim_end_matches(".rs");
                targets.push(target_name.to_string());
            }
        }
    }

    Ok(targets)
}

/// Find the directory containing a specific fuzz target implementation
fn find_fuzz_target_dir_impl(build_system: &BuildSystem, target: &str) -> BuildResult<PathBuf> {
    for crate_path in build_system.workspace.crate_paths() {
        let fuzz_dir = crate_path.join("fuzz");
        if fuzz_dir.exists() {
            let target_file = fuzz_dir.join("fuzz_targets").join(format!("{}.rs", target));
            if target_file.exists() {
                return Ok(fuzz_dir);
            }
        }
    }

    Err(BuildError::Tool(format!(
        "Fuzz target '{}' not found in any crate",
        target
    )))
}

/// Generate fuzzing report implementation
fn generate_fuzz_report_impl(
    targets_run: &[String],
    crashed_targets: &[String],
    duration: std::time::Duration,
) -> BuildResult<String> {
    let mut report = String::new();

    report.push_str("# Fuzzing Campaign Report\n\n");
    report.push_str(&format!(
        "**Generated:** {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    report.push_str(&format!("**Duration:** {:.2}s\n\n", duration.as_secs_f64()));

    // Summary
    report.push_str("## Summary\n\n");
    report.push_str(&format!("- **Targets Run:** {}\n", targets_run.len()));
    report.push_str(&format!(
        "- **Successful:** {}\n",
        targets_run.len() - crashed_targets.len()
    ));
    report.push_str(&format!(
        "- **Found Issues:** {}\n\n",
        crashed_targets.len()
    ));

    // Target details
    report.push_str("## Target Results\n\n");
    for target in targets_run {
        let status =
            if crashed_targets.contains(target) { "❌ ISSUES FOUND" } else { "✅ CLEAN" };
        report.push_str(&format!("- **{}:** {}\n", target, status));
    }

    if !crashed_targets.is_empty() {
        report.push_str("\n## Targets with Issues\n\n");
        for target in crashed_targets {
            report.push_str(&format!("### {}\n", target));
            report.push_str(
                "Crashes or timeouts detected. Check artifacts directory for details.\n\n",
            );
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_options() {
        let options = FuzzOptions::default();
        assert_eq!(options.duration, 60);
        assert_eq!(options.workers, 4);
        assert!(options.targets.is_empty());
    }

    #[test]
    fn test_fuzz_results() {
        let results = FuzzResults {
            success: true,
            targets_run: vec!["test_target".to_string()],
            crashed_targets: vec![],
            duration_ms: 1000,
            report: "Test report".to_string(),
        };

        assert!(results.success);
        assert_eq!(results.targets_run.len(), 1);
        assert!(results.crashed_targets.is_empty());
    }
}
