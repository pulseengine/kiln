//! Configuration management utilities
//!
//! Provides functions for loading configuration files, merging global
//! arguments, and handling default values consistently across cargo-wrt
//! commands.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::validation::validate_file_path;
use crate::Cli;

/// Standard configuration structure for cargo-wrt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoWrtConfig {
    /// Default output format preference
    pub output_format: Option<String>,

    /// Default ASIL level for verification
    pub default_asil: Option<String>,

    /// Enable caching by default
    pub enable_cache: Option<bool>,

    /// Default requirements file path
    pub requirements_file: Option<String>,

    /// Default tool versions to use
    pub tool_versions: Option<toml::Value>,

    /// Custom diagnostic filters
    pub diagnostic_filters: Option<DiagnosticFilters>,

    /// Browser command for opening results
    pub browser_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticFilters {
    pub default_severity: Option<Vec<String>>,
    pub default_sources: Option<Vec<String>>,
    pub default_file_patterns: Option<Vec<String>>,
}

impl Default for CargoWrtConfig {
    fn default() -> Self {
        Self {
            output_format: Some("human".to_string()),
            default_asil: Some("qm".to_string()),
            enable_cache: Some(true),
            requirements_file: Some("requirements.toml".to_string()),
            tool_versions: None,
            diagnostic_filters: None,
            browser_command: None,
        }
    }
}

/// Load configuration from file if it exists
pub fn load_config_file(workspace_root: &Path) -> Result<CargoWrtConfig> {
    let config_paths = [
        workspace_root.join(".cargo-wrt.toml"),
        workspace_root.join("cargo-wrt.toml"),
        workspace_root.join(".config").join("cargo-wrt.toml"),
    ];

    for config_path in &config_paths {
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path).with_context(|| {
                format!("Failed to read config file: {}", config_path.display())
            })?;

            let config: CargoWrtConfig = toml::from_str(&content).with_context(|| {
                format!("Failed to parse config file: {}", config_path.display())
            })?;

            return Ok(config);
        }
    }

    // Return default configuration if no file found
    Ok(CargoWrtConfig::default())
}

/// Merge global CLI arguments with configuration file settings
pub fn merge_global_args(cli: &Cli, config: &CargoWrtConfig) -> MergedConfig {
    MergedConfig {
        cache_enabled: cli.cache || config.enable_cache.unwrap_or(false),
        clear_cache: cli.clear_cache,
        diff_only: cli.diff_only,
        filter_severity: cli
            .filter_severity
            .clone()
            .or_else(|| config.diagnostic_filters.as_ref()?.default_severity.clone()),
        filter_source: cli
            .filter_source
            .clone()
            .or_else(|| config.diagnostic_filters.as_ref()?.default_sources.clone()),
        filter_file: cli
            .filter_file
            .clone()
            .or_else(|| config.diagnostic_filters.as_ref()?.default_file_patterns.clone()),
        group_by: cli.group_by.map(Into::into),
        limit: cli.limit,
        verbose: cli.verbose,
        dry_run: cli.dry_run,
        workspace: cli.workspace.clone(),
    }
}

/// Merged configuration after combining CLI args and config file
#[derive(Debug, Clone)]
pub struct MergedConfig {
    pub cache_enabled: bool,
    pub clear_cache: bool,
    pub diff_only: bool,
    pub filter_severity: Option<Vec<String>>,
    pub filter_source: Option<Vec<String>>,
    pub filter_file: Option<Vec<String>>,
    pub group_by: Option<wrt_build_core::filtering::GroupBy>,
    pub limit: Option<usize>,
    pub verbose: bool,
    pub dry_run: bool,
    pub workspace: Option<String>,
}

/// Load JSON configuration files (for test results, coverage data, etc.)
pub fn load_json_config<T>(path: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = validate_file_path(path)?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read JSON file: {}", path.display()))?;

    let config: T = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file: {}", path.display()))?;

    Ok(config)
}

/// Save JSON configuration files
pub fn save_json_config<T>(data: &T, path: &str, force: bool) -> Result<()>
where
    T: Serialize,
{
    let path = super::validation::prepare_output_path(path, force)?;
    let content = serde_json::to_string_pretty(data).context("Failed to serialize data to JSON")?;

    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write JSON file: {}", path.display()))?;

    Ok(())
}

/// Initialize a sample configuration file
pub fn init_config_file(workspace_root: &Path, force: bool) -> Result<PathBuf> {
    let config_path = workspace_root.join(".cargo-wrt.toml");

    if config_path.exists() && !force {
        return Err(anyhow::anyhow!(
            "Configuration file already exists: {}. Use --force to overwrite",
            config_path.display()
        ));
    }

    let default_config = CargoWrtConfig::default();
    let content = toml::to_string_pretty(&default_config)
        .context("Failed to serialize default configuration")?;

    std::fs::write(&config_path, content)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    Ok(config_path)
}

/// Get the browser command from config or system default
pub fn get_browser_command(config: &CargoWrtConfig) -> Option<String> {
    if let Some(ref browser) = config.browser_command {
        return Some(browser.clone());
    }

    // Try to detect system browser
    #[cfg(target_os = "macos")]
    {
        return Some("open".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        return Some("start".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        // Try common browser commands
        for browser in &["xdg-open", "firefox", "chrome", "chromium"] {
            if which::which(browser).is_ok() {
                return Some(browser.to_string());
            }
        }
        return None;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Open a file or URL in the system browser
pub fn open_in_browser(path_or_url: &str, config: &CargoWrtConfig) -> Result<()> {
    let browser_cmd = get_browser_command(config)
        .ok_or_else(|| anyhow::anyhow!("No browser command available"))?;

    std::process::Command::new(&browser_cmd)
        .arg(path_or_url)
        .spawn()
        .with_context(|| format!("Failed to open {} with {}", path_or_url, browser_cmd))?;

    Ok(())
}

/// Helper to create and open HTML reports
pub fn create_and_open_html_report(
    report_content: &str,
    report_name: &str,
    config: &CargoWrtConfig,
    auto_open: bool,
    output: &super::OutputManager,
) -> Result<PathBuf> {
    // Create temp directory if it doesn't exist
    let temp_dir = std::env::temp_dir().join("cargo-wrt-reports");
    std::fs::create_dir_all(&temp_dir)
        .context("Failed to create temp directory for HTML reports")?;

    // Generate unique filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.html", report_name, timestamp);
    let report_path = temp_dir.join(filename);

    // Write HTML content
    std::fs::write(&report_path, report_content)
        .with_context(|| format!("Failed to write HTML report: {}", report_path.display()))?;

    output.success(&format!("HTML report generated: {}", report_path.display()));

    if auto_open {
        if let Err(e) = open_in_browser(&report_path.to_string_lossy(), config) {
            output.warning(&format!("Failed to open browser: {}", e));
            output.info(&format!("You can manually open: {}", report_path.display()));
        } else {
            output.info("Opening report in browser...");
        }
    }

    Ok(report_path)
}
