//! Global argument propagation helper
//!
//! Provides a unified way to pass global CLI arguments to all subcommands
//! and helper functions, ensuring consistent behavior across the application.

use anyhow::Result;
use atty::{self, Stream};
use wrt_build_core::{
    config::BuildProfile,
    diagnostics::Severity,
    filtering::{FilterOptionsBuilder, GroupBy},
    formatters::OutputFormat,
};

use super::OutputManager;
use crate::{Cli, GroupByArg, OutputFormatArg, ProfileArg};

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

/// Global arguments that should be propagated to all commands
pub struct GlobalArgs {
    /// Enable verbose output
    pub verbose: bool,

    /// Show commands being executed without running them
    pub dry_run: bool,

    /// Trace all external commands being executed
    pub trace_commands: bool,

    /// Build profile to use
    pub profile: BuildProfile,

    /// Features to enable
    pub features: Vec<String>,

    /// Workspace root directory
    pub workspace: Option<String>,

    /// Output format for diagnostics and results
    pub output_format: OutputFormat,

    /// Output manager configured with format and color settings
    pub output: OutputManager,

    /// Enable diagnostic caching
    pub cache: bool,

    /// Clear diagnostic cache before running
    pub clear_cache: bool,

    /// Show only new/changed diagnostics
    pub diff_only: bool,

    /// Filter options builder (lazily created)
    filter_options: Option<FilterOptionsBuilder>,

    // Raw filter data for lazy initialization
    pub filter_severity: Option<Vec<String>>,
    pub filter_source: Option<Vec<String>>,
    pub filter_file: Option<Vec<String>>,
    pub group_by: Option<GroupBy>,
    pub limit: Option<usize>,
}

impl std::fmt::Debug for GlobalArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalArgs")
            .field("verbose", &self.verbose)
            .field("dry_run", &self.dry_run)
            .field("trace_commands", &self.trace_commands)
            .field("profile", &self.profile)
            .field("features", &self.features)
            .field("workspace", &self.workspace)
            .field("output_format", &self.output_format)
            .field("cache", &self.cache)
            .field("clear_cache", &self.clear_cache)
            .field("diff_only", &self.diff_only)
            .field("filter_options", &"<FilterOptionsBuilder>")
            .field("filter_severity", &self.filter_severity)
            .field("filter_source", &self.filter_source)
            .field("filter_file", &self.filter_file)
            .field("group_by", &self.group_by)
            .field("limit", &self.limit)
            .finish()
    }
}

impl Clone for GlobalArgs {
    fn clone(&self) -> Self {
        Self {
            verbose: self.verbose,
            dry_run: self.dry_run,
            trace_commands: self.trace_commands,
            profile: self.profile.clone(),
            features: self.features.clone(),
            workspace: self.workspace.clone(),
            output_format: self.output_format,
            output: self.output.clone(),
            cache: self.cache,
            clear_cache: self.clear_cache,
            diff_only: self.diff_only,
            filter_options: None, // Reset lazy field on clone
            filter_severity: self.filter_severity.clone(),
            filter_source: self.filter_source.clone(),
            filter_file: self.filter_file.clone(),
            group_by: self.group_by,
            limit: self.limit,
        }
    }
}

impl GlobalArgs {
    /// Create GlobalArgs from CLI struct
    #[must_use]
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let output_format: OutputFormat = cli.output.into();
        let use_colors = match output_format {
            OutputFormat::Human => atty::is(Stream::Stdout),
            OutputFormat::Json | OutputFormat::JsonLines => false,
        };
        let output = OutputManager::new(output_format.clone()).with_color(use_colors);

        let features = cli
            .features
            .as_ref()
            .map(|f| f.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        Ok(Self {
            verbose: cli.verbose,
            dry_run: cli.dry_run,
            trace_commands: cli.trace_commands,
            profile: cli.profile.into(),
            features,
            workspace: cli.workspace.clone(),
            output_format,
            output,
            cache: cli.cache,
            clear_cache: cli.clear_cache,
            diff_only: cli.diff_only,
            filter_options: None,
            filter_severity: cli.filter_severity.clone(),
            filter_source: cli.filter_source.clone(),
            filter_file: cli.filter_file.clone(),
            group_by: cli.group_by.map(Into::into),
            limit: cli.limit,
        })
    }

    /// Get filter options (creates them lazily if needed)
    #[must_use]
    pub fn build_filter_options(&mut self) -> Result<wrt_build_core::filtering::FilterOptions> {
        // Build filter options fresh each time
        let mut builder = FilterOptionsBuilder::new();

        // Apply severity filter
        if let Some(severity_strings) = &self.filter_severity {
            let severities = parse_severities(severity_strings)?;
            builder = builder.severities(&severities);
        }

        // Apply source filter
        if let Some(sources) = &self.filter_source {
            builder = builder.sources(sources);
        }

        // Apply file pattern filter
        if let Some(patterns) = &self.filter_file {
            builder = builder.file_patterns(patterns);
        }

        // Apply grouping
        if let Some(group_by) = self.group_by {
            builder = builder.group_by(group_by);
        }

        // Apply limit
        if let Some(limit) = self.limit {
            builder = builder.limit(limit);
        }

        // Default sorting
        builder = builder.sort_by(
            wrt_build_core::filtering::SortBy::File,
            wrt_build_core::filtering::SortDirection::Ascending,
        );

        Ok(builder.build())
    }

    /// Check if we should show progress/spinner
    pub fn show_progress(&self) -> bool {
        !self.dry_run && matches!(self.output_format, OutputFormat::Human)
    }

    /// Check if JSON mode is active
    pub fn is_json_mode(&self) -> bool {
        matches!(
            self.output_format,
            OutputFormat::Json | OutputFormat::JsonLines
        )
    }

    /// Get cache path for the workspace
    pub fn cache_path(&self) -> Option<std::path::PathBuf> {
        if self.cache || self.clear_cache || self.diff_only {
            let workspace_root =
                self.workspace.as_ref().map(std::path::PathBuf::from).unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                });

            Some(workspace_root.join("target").join("wrt-cache").join("diagnostics.json"))
        } else {
            None
        }
    }
}

/// Extension trait for commands that need global args
pub trait WithGlobalArgs {
    /// Execute with global arguments context
    fn with_global_args<F, R>(&self, args: &GlobalArgs, f: F) -> R
    where
        F: FnOnce(&GlobalArgs) -> R;
}
