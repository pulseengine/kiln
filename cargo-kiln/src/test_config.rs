//! Test configuration file support for cargo-wrt
//!
//! This module provides support for loading test configuration from TOML files,
//! allowing users to customize test execution without command-line arguments.

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use wrt_build_core::config::AsilLevel;

/// Main test configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrtTestConfig {
    /// Default ASIL level for testing
    #[serde(default = "default_asil_level")]
    pub default_asil: AsilLevel,

    /// Global test settings
    #[serde(default)]
    pub global: GlobalTestSettings,

    /// Per-ASIL level configuration
    #[serde(default)]
    pub asil: HashMap<AsilLevel, AsilTestConfig>,

    /// Package-specific test configurations
    #[serde(default)]
    pub packages: HashMap<String, PackageTestConfig>,
}

/// Global test settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTestSettings {
    /// Default number of test threads
    #[serde(default = "default_test_threads")]
    pub test_threads: usize,

    /// Default timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Whether to run tests in parallel by default
    #[serde(default = "default_parallel")]
    pub parallel: bool,

    /// Whether to include no_std tests by default
    #[serde(default)]
    pub include_no_std: bool,

    /// Default test filter patterns to exclude
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

/// ASIL-specific test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsilTestConfig {
    /// Required packages for this ASIL level
    #[serde(default)]
    pub required_packages: Vec<String>,

    /// Optional packages for this ASIL level
    #[serde(default)]
    pub optional_packages: Vec<String>,

    /// Test patterns to include
    #[serde(default)]
    pub include_patterns: Vec<String>,

    /// Test patterns to exclude
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Whether no_std tests are required
    #[serde(default)]
    pub require_no_std: bool,

    /// Minimum test coverage percentage
    #[serde(default)]
    pub min_coverage: Option<f64>,
}

/// Package-specific test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageTestConfig {
    /// Whether this package supports no_std
    #[serde(default = "default_true")]
    pub supports_no_std: bool,

    /// ASIL levels this package is relevant for
    #[serde(default)]
    pub asil_levels: Vec<AsilLevel>,

    /// Package-specific test arguments
    #[serde(default)]
    pub test_args: Vec<String>,

    /// Timeout override for this package
    #[serde(default)]
    pub timeout_seconds: Option<u64>,

    /// Whether to run tests in serial for this package
    #[serde(default)]
    pub serial: bool,
}

impl Default for GlobalTestSettings {
    fn default() -> Self {
        Self {
            test_threads: default_test_threads(),
            timeout_seconds: default_timeout(),
            parallel: default_parallel(),
            include_no_std: false,
            exclude_patterns: vec![],
        }
    }
}

impl Default for WrtTestConfig {
    fn default() -> Self {
        Self {
            default_asil: default_asil_level(),
            global: GlobalTestSettings::default(),
            asil: HashMap::new(),
            packages: HashMap::new(),
        }
    }
}

impl WrtTestConfig {
    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content =
            fs::read_to_string(path.as_ref()).context("Failed to read test configuration file")?;

        let config: WrtTestConfig =
            toml::from_str(&content).context("Failed to parse test configuration TOML")?;

        config.validate().context("Test configuration validation failed")?;

        Ok(config)
    }

    /// Load configuration with fallback to default
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::load_from_file(path).unwrap_or_default()
    }

    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content =
            toml::to_string_pretty(self).context("Failed to serialize test configuration")?;

        fs::write(path.as_ref(), content).context("Failed to write test configuration file")?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Check that all referenced packages exist in the workspace
        // This would require workspace introspection, so we'll keep it simple for now

        // Validate ASIL levels
        for (level, config) in &self.asil {
            if config.min_coverage.is_some() {
                let coverage = config.min_coverage.unwrap();
                if !(0.0..=100.0).contains(&coverage) {
                    anyhow::bail!(
                        "Invalid coverage percentage for ASIL-{}: {}",
                        level,
                        coverage
                    );
                }
            }
        }

        Ok(())
    }

    /// Get configuration for a specific ASIL level
    pub fn get_asil_config(&self, level: AsilLevel) -> AsilTestConfig {
        self.asil.get(&level).cloned().unwrap_or_else(|| {
            // Provide sensible defaults based on ASIL level
            match level {
                AsilLevel::QM => AsilTestConfig {
                    required_packages: vec![],
                    optional_packages: vec![],
                    include_patterns: vec![],
                    exclude_patterns: vec![],
                    require_no_std: false,
                    min_coverage: None,
                },
                AsilLevel::B => AsilTestConfig {
                    required_packages: vec![
                        "wrt-error".to_string(),
                        "wrt-foundation".to_string(),
                        "wrt-platform".to_string(),
                    ],
                    optional_packages: vec!["wrt-runtime".to_string()],
                    include_patterns: vec![],
                    exclude_patterns: vec!["*integration*".to_string()],
                    require_no_std: true,
                    min_coverage: Some(85.0),
                },
                AsilLevel::D => AsilTestConfig {
                    required_packages: vec![
                        "wrt-error".to_string(),
                        "wrt-foundation".to_string(),
                        "wrt-platform".to_string(),
                        "wrt-sync".to_string(),
                    ],
                    optional_packages: vec![],
                    include_patterns: vec!["*safety*".to_string(), "*verification*".to_string()],
                    exclude_patterns: vec!["*integration*".to_string(), "*example*".to_string()],
                    require_no_std: true,
                    min_coverage: Some(95.0),
                },
                _ => AsilTestConfig {
                    required_packages: vec!["wrt-error".to_string(), "wrt-foundation".to_string()],
                    optional_packages: vec![],
                    include_patterns: vec![],
                    exclude_patterns: vec![],
                    require_no_std: false,
                    min_coverage: Some(75.0),
                },
            }
        })
    }

    /// Get configuration for a specific package
    pub fn get_package_config(&self, package: &str) -> PackageTestConfig {
        self.packages.get(package).cloned().unwrap_or_else(|| {
            // Provide sensible defaults based on package name
            PackageTestConfig {
                supports_no_std: !package.contains("wrtd") && !package.contains("std"),
                asil_levels: match package {
                    p if p.starts_with("wrt-error") || p.starts_with("wrt-foundation") => {
                        vec![AsilLevel::QM, AsilLevel::B, AsilLevel::D]
                    },
                    p if p.starts_with("wrt-platform") || p.starts_with("wrt-sync") => {
                        vec![AsilLevel::B, AsilLevel::D]
                    },
                    _ => vec![AsilLevel::QM],
                },
                test_args: vec![],
                timeout_seconds: None,
                serial: false,
            }
        })
    }

    /// Generate an example configuration file
    pub fn example_config() -> Self {
        let mut config = Self::default();

        // Add example ASIL configurations
        config.asil.insert(
            AsilLevel::D,
            AsilTestConfig {
                required_packages: vec![
                    "wrt-error".to_string(),
                    "wrt-foundation".to_string(),
                    "wrt-platform".to_string(),
                ],
                optional_packages: vec!["wrt-sync".to_string()],
                include_patterns: vec!["*safety*".to_string()],
                exclude_patterns: vec!["*integration*".to_string()],
                require_no_std: true,
                min_coverage: Some(95.0),
            },
        );

        // Add example package configurations
        config.packages.insert(
            "wrt-foundation".to_string(),
            PackageTestConfig {
                supports_no_std: true,
                asil_levels: vec![AsilLevel::QM, AsilLevel::B, AsilLevel::D],
                test_args: vec!["--".to_string(), "--test-threads=1".to_string()],
                timeout_seconds: Some(300),
                serial: false,
            },
        );

        config
    }
}

// Default value functions for serde
fn default_asil_level() -> AsilLevel {
    AsilLevel::QM
}

fn default_test_threads() -> usize {
    num_cpus::get().max(1)
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}

fn default_parallel() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_default_config() {
        let config = WrtTestConfig::default();
        assert_eq!(config.default_asil, AsilLevel::QM);
        assert!(config.global.parallel);
        assert!(config.asil.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = WrtTestConfig::example_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();

        // Should be able to round-trip
        let parsed: WrtTestConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.default_asil, config.default_asil);
    }

    #[test]
    fn test_config_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.toml");

        let config = WrtTestConfig::example_config();
        config.save_to_file(&config_path).unwrap();

        let loaded_config = WrtTestConfig::load_from_file(&config_path).unwrap();
        assert_eq!(loaded_config.default_asil, config.default_asil);
    }

    #[test]
    fn test_asil_level_defaults() {
        let config = WrtTestConfig::default();

        let d_config = config.get_asil_config(AsilLevel::D);
        assert!(d_config.require_no_std);
        assert!(d_config.min_coverage.unwrap() >= 90.0);

        let qm_config = config.get_asil_config(AsilLevel::QM);
        assert!(!qm_config.require_no_std);
    }
}
