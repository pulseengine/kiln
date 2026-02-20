//! Diagnostic integration utilities
//!
//! Provides standardized patterns for integrating diagnostic collection,
//! caching, and filtering across all cargo-wrt commands.

use anyhow::Result;
use wrt_build_core::{
    BuildSystem,
    cache::CacheManager,
    diagnostics::{DiagnosticCollection, Severity},
    filtering::FilterOptionsBuilder,
    formatters::OutputFormat,
};

use super::output::{OutputManager, output_diagnostics};
use crate::Cli;

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

/// Trait for commands that support diagnostic integration
pub trait DiagnosticCommand {
    /// Execute the command and return diagnostics
    fn execute_with_diagnostics(&self, build_system: &BuildSystem) -> Result<DiagnosticCollection>;

    /// Whether this command supports caching
    fn supports_caching(&self) -> bool {
        true
    }

    /// Whether this command supports filtering
    fn supports_filtering(&self) -> bool {
        true
    }

    /// Get the cache key for this command
    fn cache_key(&self) -> String {
        "default".to_string()
    }
}

/// Standard wrapper for executing commands with full diagnostic integration
pub async fn with_diagnostic_integration<F, T>(
    command_name: &str,
    build_system: &BuildSystem,
    cli: &Cli,
    output: &OutputManager,
    executor: F,
) -> Result<T>
where
    F: Fn() -> Result<(T, DiagnosticCollection)>,
{
    let (result, mut diagnostics) = executor()?;

    // Apply caching if enabled
    if cli.cache {
        let workspace_root = build_system.workspace_root();
        let cache_path = get_cache_path(workspace_root);
        let mut cache_manager = CacheManager::new(workspace_root.to_path_buf(), cache_path, true)?;

        if cli.clear_cache {
            cache_manager.clear()?;
        }

        if cli.diff_only {
            let diff_diagnostics = cache_manager.get_diff_diagnostics(&diagnostics.diagnostics);
            diagnostics.diagnostics = diff_diagnostics;
        }
        cache_manager.save()?;
    }

    // Apply filtering if requested
    if cli.filter_severity.is_some() || cli.filter_source.is_some() || cli.filter_file.is_some() {
        let mut filter_builder = FilterOptionsBuilder::new();

        if let Some(ref severity_strings) = cli.filter_severity {
            let severities = parse_severities(severity_strings)?;
            filter_builder = filter_builder.severities(&severities);
        }

        if let Some(ref sources) = cli.filter_source {
            filter_builder = filter_builder.sources(sources);
        }

        if let Some(ref patterns) = cli.filter_file {
            filter_builder = filter_builder.file_patterns(patterns);
        }

        let filter_options = filter_builder.build();
        // TODO: Implement filtering manually if needed
    }

    // TODO: Apply grouping and sorting if requested
    // These methods don't exist yet on DiagnosticCollection

    // Output diagnostics
    output_diagnostics(diagnostics, output.format())?;

    Ok(result)
}

/// Get cache path for diagnostics
fn get_cache_path(workspace_root: &std::path::Path) -> std::path::PathBuf {
    workspace_root.join("target").join("cargo-wrt-cache")
}

/// Simple wrapper for commands that don't yet support full diagnostics
pub async fn with_simple_output<T>(
    result: T,
    output: &OutputManager,
    success_message: Option<&str>,
) -> Result<T>
where
    T: serde::Serialize + std::fmt::Display + Clone,
{
    if let Some(msg) = success_message {
        output.success(msg);
    }
    output.output_result(&result)?;
    Ok(result)
}

/// Enhanced diagnostic helper with better integration
pub struct DiagnosticHelper {
    output: OutputManager,
    cli: Cli,
}

impl DiagnosticHelper {
    pub fn new(output: OutputManager, cli: Cli) -> Self {
        Self { output, cli }
    }

    /// Execute a command with full diagnostic support
    pub async fn execute<F, T>(&self, build_system: &BuildSystem, executor: F) -> Result<T>
    where
        F: Fn() -> Result<(T, DiagnosticCollection)>,
    {
        with_diagnostic_integration("command", build_system, &self.cli, &self.output, executor)
            .await
    }

    /// Execute a simple command without diagnostics
    pub async fn execute_simple<T>(&self, result: T, success_message: Option<&str>) -> Result<T>
    where
        T: serde::Serialize + std::fmt::Display + Clone,
    {
        with_simple_output(result, &self.output, success_message).await
    }
}
