//! Tool version management and configuration
//!
//! This module manages specific versions of external tools required by the
//! build system, ensuring reproducible builds and proper dependency tracking.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Tool version requirement specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolVersion {
    /// Required version (exact, minimum, or range)
    pub version: String,
    /// Version requirement type
    pub requirement_type: VersionRequirement,
    /// Installation command with specific version
    pub install_command: String,
    /// How to check the installed version
    pub version_check_args: Vec<String>,
    /// Expected output pattern to extract version
    pub version_pattern: Option<String>,
    /// Whether this tool is required for basic functionality
    #[serde(default)]
    pub required: bool,
    /// Which cargo-wrt commands need this tool
    #[serde(default)]
    pub used_by: Vec<String>,
    /// Tool description
    #[serde(default)]
    pub description: String,
    /// Target-specific configurations
    #[serde(default)]
    pub target_specific: HashMap<String, TargetToolConfig>,
}

/// Target-specific tool configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetToolConfig {
    /// Override version for this target
    pub version: Option<String>,
    /// Override installation command for this target
    pub install_command: Option<String>,
    /// Override version check args for this target
    pub version_check_args: Option<Vec<String>>,
    /// Target-specific requirements or constraints
    pub constraints: Vec<String>,
    /// Whether this target is supported
    pub supported: bool,
}

/// Version requirement types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VersionRequirement {
    /// Exact version required
    Exact,
    /// Minimum version required
    Minimum,
    /// Compatible version (semver compatible)
    Compatible,
    /// Any version acceptable
    Any,
}

/// Tool versions configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersionConfig {
    /// Map of tool name to version specification
    pub tools: HashMap<String, ToolVersion>,
    /// Configuration metadata
    pub metadata: VersionConfigMetadata,
    /// Rust toolchain configuration (read from rust-toolchain.toml)
    pub rust_toolchain: Option<RustToolchainConfig>,
}

/// TOML file structure for tool versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersionsToml {
    /// Configuration metadata
    pub metadata: VersionConfigMetadata,
    /// Tool specifications
    pub tools: HashMap<String, ToolVersionToml>,
}

/// Rust toolchain configuration from rust-toolchain.toml
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustToolchainConfig {
    /// Toolchain channel (e.g., "stable", "nightly")
    pub channel: String,
    /// Specific version if pinned
    pub version: Option<String>,
    /// Components to include
    #[serde(default)]
    pub components: Vec<String>,
    /// Targets to include
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Tool version as stored in TOML file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersionToml {
    /// Required version (exact, minimum, or range)
    pub version: String,
    /// Version requirement type
    pub requirement_type: String,
    /// Installation command with specific version
    pub install_command: String,
    /// How to check the installed version
    pub version_check_args: Vec<String>,
    /// Expected output pattern to extract version
    pub version_pattern: String,
    /// Whether this tool is required for basic functionality
    #[serde(default)]
    pub required: bool,
    /// Which cargo-wrt commands need this tool
    #[serde(default)]
    pub used_by: Vec<String>,
    /// Tool description
    #[serde(default)]
    pub description: String,
    /// Target-specific configurations
    #[serde(default)]
    pub target_specific: HashMap<String, TargetToolConfigToml>,
}

/// Target-specific tool configuration in TOML format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetToolConfigToml {
    /// Override version for this target
    pub version: Option<String>,
    /// Override installation command for this target
    pub install_command: Option<String>,
    /// Override version check args for this target
    pub version_check_args: Option<Vec<String>>,
    /// Target-specific requirements or constraints
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Whether this target is supported
    #[serde(default = "default_true")]
    pub supported: bool,
}

fn default_true() -> bool {
    true
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConfigMetadata {
    /// Configuration format version
    pub config_version: String,
    /// Last updated timestamp
    pub last_updated: String,
    /// Description
    pub description: String,
}

impl Default for ToolVersionConfig {
    fn default() -> Self {
        // Try to load from file first, fall back to hardcoded defaults
        Self::load_from_workspace().unwrap_or_else(|_| Self::create_fallback_config())
    }
}

impl ToolVersionConfig {
    /// Load configuration from file or use default workspace detection
    pub fn load_from_workspace() -> Result<Self, crate::error::BuildError> {
        // Try to find workspace root and load tool-versions.toml
        match crate::detect_workspace_root() {
            Ok(workspace_root) => {
                let config_path = workspace_root.join("tool-versions.toml");
                let mut config = if config_path.exists() {
                    Self::load_from_file(&config_path)?
                } else {
                    // File doesn't exist, use fallback defaults
                    Self::create_fallback_config()
                };

                // Load rust-toolchain.toml if it exists
                let rust_toolchain_path = workspace_root.join("rust-toolchain.toml");
                if rust_toolchain_path.exists() {
                    config.rust_toolchain = Some(Self::load_rust_toolchain(&rust_toolchain_path)?);
                }

                Ok(config)
            },
            Err(_) => {
                // Can't find workspace, use fallback defaults
                Ok(Self::create_fallback_config())
            },
        }
    }

    /// Load configuration from a specific file
    pub fn load_from_file(config_path: &std::path::Path) -> Result<Self, crate::error::BuildError> {
        use crate::error::BuildError;

        let content = std::fs::read_to_string(config_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read tool-versions.toml: {}", e)))?;

        let toml_config: ToolVersionsToml = toml::from_str(&content)
            .map_err(|e| BuildError::Tool(format!("Failed to parse tool-versions.toml: {}", e)))?;

        // Convert from TOML format to internal format
        let mut tools = HashMap::new();
        for (name, toml_tool) in toml_config.tools {
            let requirement_type = match toml_tool.requirement_type.as_str() {
                "Exact" => VersionRequirement::Exact,
                "Minimum" => VersionRequirement::Minimum,
                "Compatible" => VersionRequirement::Compatible,
                "Any" => VersionRequirement::Any,
                _ => VersionRequirement::Minimum, // Default fallback
            };

            // Convert target-specific configurations
            let mut target_specific = HashMap::new();
            for (target, target_config) in toml_tool.target_specific {
                target_specific.insert(
                    target,
                    TargetToolConfig {
                        version: target_config.version,
                        install_command: target_config.install_command,
                        version_check_args: target_config.version_check_args,
                        constraints: target_config.constraints,
                        supported: target_config.supported,
                    },
                );
            }

            tools.insert(
                name,
                ToolVersion {
                    version: toml_tool.version,
                    requirement_type,
                    install_command: toml_tool.install_command,
                    version_check_args: toml_tool.version_check_args,
                    version_pattern: if toml_tool.version_pattern.is_empty() {
                        None
                    } else {
                        Some(toml_tool.version_pattern)
                    },
                    required: toml_tool.required,
                    used_by: toml_tool.used_by,
                    description: toml_tool.description,
                    target_specific,
                },
            );
        }

        Ok(ToolVersionConfig {
            tools,
            metadata: toml_config.metadata,
            rust_toolchain: None, // Will be loaded separately if available
        })
    }

    /// Load rust-toolchain.toml configuration
    pub fn load_rust_toolchain(
        toolchain_path: &std::path::Path,
    ) -> Result<RustToolchainConfig, crate::error::BuildError> {
        use crate::error::BuildError;

        let content = std::fs::read_to_string(toolchain_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read rust-toolchain.toml: {}", e)))?;

        // Parse the [toolchain] section
        #[derive(Deserialize)]
        struct RustToolchainToml {
            toolchain: RustToolchainSection,
        }

        #[derive(Deserialize)]
        struct RustToolchainSection {
            channel: String,
            version: Option<String>,
            #[serde(default)]
            components: Vec<String>,
            #[serde(default)]
            targets: Vec<String>,
        }

        let toml_config: RustToolchainToml = toml::from_str(&content)
            .map_err(|e| BuildError::Tool(format!("Failed to parse rust-toolchain.toml: {}", e)))?;

        Ok(RustToolchainConfig {
            channel: toml_config.toolchain.channel,
            version: toml_config.toolchain.version,
            components: toml_config.toolchain.components,
            targets: toml_config.toolchain.targets,
        })
    }

    /// Create fallback configuration when file is not available  
    pub fn create_fallback_config() -> Self {
        let mut tools = HashMap::new();

        // Kani formal verification
        tools.insert(
            "kani".to_string(),
            ToolVersion {
                version: "0.63.0".to_string(),
                requirement_type: VersionRequirement::Exact,
                install_command: "cargo install --locked --version 0.63.0 kani-verifier && \
                                     cargo kani setup"
                    .to_string(),
                version_check_args: vec!["--version".to_string()],
                version_pattern: Some(r"kani (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["kani-verify".to_string(), "verify".to_string()],
                description: "CBMC-based formal verification for Rust".to_string(),
                target_specific: HashMap::new(),
            },
        );

        // Cargo-fuzz fuzzing tool
        tools.insert(
            "cargo-fuzz".to_string(),
            ToolVersion {
                version: "0.12.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "cargo install --locked --version 0.12.0 cargo-fuzz".to_string(),
                version_check_args: vec!["fuzz".to_string(), "--version".to_string()],
                version_pattern: Some(r"cargo-fuzz (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["fuzz".to_string()],
                description: "Coverage-guided fuzzing for Rust".to_string(),
                target_specific: HashMap::new(),
            },
        );

        // Rust toolchain components
        tools.insert(
            "clippy".to_string(),
            ToolVersion {
                version: "1.86.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "rustup component add clippy".to_string(),
                version_check_args: vec!["clippy".to_string(), "--version".to_string()],
                version_pattern: Some(r"clippy (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["check".to_string(), "ci".to_string()],
                description: "Rust linter for code quality checks".to_string(),
                target_specific: HashMap::new(),
            },
        );

        tools.insert(
            "rustfmt".to_string(),
            ToolVersion {
                version: "1.86.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "rustup component add rustfmt".to_string(),
                version_check_args: vec!["fmt".to_string(), "--version".to_string()],
                version_pattern: Some(r"rustfmt (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["check".to_string(), "ci".to_string()],
                description: "Rust code formatter".to_string(),
                target_specific: HashMap::new(),
            },
        );

        // Git version control
        tools.insert(
            "git".to_string(),
            ToolVersion {
                version: "2.30.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "Please install Git from https://git-scm.com/".to_string(),
                version_check_args: vec!["--version".to_string()],
                version_pattern: Some(r"git version (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["setup".to_string()],
                description: "Distributed version control".to_string(),
                target_specific: HashMap::new(),
            },
        );

        // LLVM tools for coverage
        tools.insert(
            "llvm-cov".to_string(),
            ToolVersion {
                version: "1.75.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "rustup component add llvm-tools-preview".to_string(),
                version_check_args: vec!["--version".to_string()],
                version_pattern: Some(r"llvm-cov (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["coverage".to_string()],
                description: "LLVM coverage analysis tools".to_string(),
                target_specific: HashMap::new(),
            },
        );

        // Documentation tools
        tools.insert(
            "python3".to_string(),
            ToolVersion {
                version: "3.8.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "Install Python from https://python.org or via package manager"
                    .to_string(),
                version_check_args: vec!["--version".to_string()],
                version_pattern: Some(r"Python (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["docs".to_string()],
                description: "Python interpreter for Sphinx documentation generation".to_string(),
                target_specific: HashMap::new(),
            },
        );

        tools.insert(
            "python-venv".to_string(),
            ToolVersion {
                version: "3.8.0".to_string(),
                requirement_type: VersionRequirement::Minimum,
                install_command: "Included with Python 3.8+ - install Python if missing"
                    .to_string(),
                version_check_args: vec![
                    "-m".to_string(),
                    "venv".to_string(),
                    "--help".to_string(),
                ],
                version_pattern: Some(r"Python (\d+\.\d+\.\d+)".to_string()),
                required: false,
                used_by: vec!["docs".to_string()],
                description: "Python virtual environment support for isolated \
                                     documentation dependencies"
                    .to_string(),
                target_specific: HashMap::new(),
            },
        );

        Self {
            tools,
            metadata: VersionConfigMetadata {
                config_version: "1.0.0".to_string(),
                last_updated: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                description: "WRT build system tool version requirements (fallback)".to_string(),
            },
            rust_toolchain: None, // Will be loaded from rust-toolchain.toml if available
        }
    }

    /// Load configuration from file or use default
    pub fn load_or_default() -> Self {
        Self::default() // This now loads from file or fallback
    }

    /// Get version requirement for a tool
    pub fn get_tool_version(&self, tool_name: &str) -> Option<&ToolVersion> {
        self.tools.get(tool_name)
    }

    /// Get version requirement for a tool with target-specific override
    pub fn get_tool_version_for_target(
        &self,
        tool_name: &str,
        target: Option<&str>,
    ) -> Option<ToolVersion> {
        let base_tool = self.tools.get(tool_name)?.clone();

        // If no target specified or no target-specific config, return base tool
        let target = match target {
            Some(t) => t,
            None => return Some(base_tool),
        };

        // Check for target-specific configuration
        if let Some(target_config) = base_tool.target_specific.get(target).cloned() {
            if !target_config.supported {
                return None; // Target not supported
            }

            let mut tool = base_tool;

            // Override with target-specific settings
            if let Some(version) = &target_config.version {
                tool.version = version.clone();
            }
            if let Some(install_cmd) = &target_config.install_command {
                tool.install_command = install_cmd.clone();
            }
            if let Some(check_args) = &target_config.version_check_args {
                tool.version_check_args = check_args.clone();
            }

            Some(tool)
        } else {
            Some(base_tool)
        }
    }

    /// Check if a target is supported for a tool
    pub fn is_target_supported(&self, tool_name: &str, target: &str) -> bool {
        if let Some(tool) = self.tools.get(tool_name) {
            if let Some(target_config) = tool.target_specific.get(target) {
                target_config.supported
            } else {
                true // No specific config means generally supported
            }
        } else {
            false
        }
    }

    /// Get the effective Rust toolchain version from rust-toolchain.toml or
    /// fallback
    pub fn get_rust_toolchain_version(&self) -> String {
        if let Some(toolchain) = &self.rust_toolchain {
            if let Some(version) = &toolchain.version {
                version.clone()
            } else {
                // Use channel as version (e.g., "stable")
                toolchain.channel.clone()
            }
        } else {
            "1.86.0".to_string() // Fallback version
        }
    }

    /// Get all targets specified in rust-toolchain.toml
    pub fn get_rust_targets(&self) -> Vec<String> {
        if let Some(toolchain) = &self.rust_toolchain {
            toolchain.targets.clone()
        } else {
            vec![] // No targets specified
        }
    }

    /// Check if rustup target is installed
    pub fn check_rustup_target_installed(&self, target: &str) -> bool {
        use std::process::Command;

        let output = Command::new("rustup").args(["target", "list", "--installed"]).output();

        match output {
            Ok(output) if output.status.success() => {
                let installed_targets = String::from_utf8_lossy(&output.stdout);
                installed_targets.lines().any(|line| line.trim() == target)
            },
            _ => false,
        }
    }

    /// Check if a tool version satisfies requirements
    pub fn check_version_compatibility(
        &self,
        tool_name: &str,
        installed_version: &str,
    ) -> Option<VersionComparison> {
        let tool_version = self.get_tool_version(tool_name)?;

        match tool_version.requirement_type {
            VersionRequirement::Any => Some(VersionComparison::Satisfies),
            VersionRequirement::Exact => {
                if installed_version == tool_version.version {
                    Some(VersionComparison::Satisfies)
                } else {
                    Some(VersionComparison::Mismatch {
                        installed: installed_version.to_string(),
                        required: tool_version.version.clone(),
                    })
                }
            },
            VersionRequirement::Minimum | VersionRequirement::Compatible => {
                match compare_versions(installed_version, &tool_version.version) {
                    std::cmp::Ordering::Greater => Some(VersionComparison::Newer {
                        installed: installed_version.to_string(),
                        required: tool_version.version.clone(),
                    }),
                    std::cmp::Ordering::Equal => Some(VersionComparison::Satisfies),
                    std::cmp::Ordering::Less => Some(VersionComparison::TooOld {
                        installed: installed_version.to_string(),
                        required: tool_version.version.clone(),
                    }),
                }
            },
        }
    }

    /// Generate installation command for a tool
    pub fn get_install_command(&self, tool_name: &str) -> Option<&str> {
        self.get_tool_version(tool_name).map(|v| v.install_command.as_str())
    }

    /// Get list of all managed tools
    pub fn get_managed_tools(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }

    /// Save configuration to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Load configuration from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

/// Version comparison result
#[derive(Debug, PartialEq, Eq)]
pub enum VersionComparison {
    /// Installed version meets requirements
    Satisfies,
    /// Installed version is older than required
    TooOld {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
    /// Installed version is newer than required (warning)
    Newer {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
    /// Exact version mismatch
    Mismatch {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
}

/// Simple semantic version comparison
/// Returns Ordering::Less if v1 < v2, Equal if v1 == v2, Greater if v1 > v2
fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .take(3) // Major.Minor.Patch
            .map(|s| s.parse::<u32>().unwrap_or(0))
            .collect()
    };

    let v1_parts = parse_version(v1);
    let v2_parts = parse_version(v2);

    // Pad to ensure same length
    let max_len = v1_parts.len().max(v2_parts.len());
    let v1_padded: Vec<u32> =
        v1_parts.into_iter().chain(std::iter::repeat(0)).take(max_len).collect();
    let v2_padded: Vec<u32> =
        v2_parts.into_iter().chain(std::iter::repeat(0)).take(max_len).collect();

    v1_padded.cmp(&v2_padded)
}

/// Extract version from command output using regex
pub fn extract_version_from_output(output: &str, pattern: &str) -> Option<String> {
    use regex::Regex;

    let regex = Regex::new(pattern).ok()?;
    let captures = regex.captures(output)?;
    captures.get(1).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("1.0.1", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.0.0", "1.0.1"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("2.0.0", "1.9.9"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.9.9", "2.0.0"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_version_extraction() {
        let output = "kani 0.63.0";
        let pattern = r"kani (\d+\.\d+\.\d+)";
        assert_eq!(
            extract_version_from_output(output, pattern),
            Some("0.63.0".to_string())
        );

        let output = "git version 2.39.5 (Apple Git-154)";
        let pattern = r"git version (\d+\.\d+\.\d+)";
        assert_eq!(
            extract_version_from_output(output, pattern),
            Some("2.39.5".to_string())
        );
    }

    #[test]
    fn test_version_compatibility_check() {
        let config = ToolVersionConfig::default();

        // Test exact version requirement
        if let Some(kani_version) = config.get_tool_version("kani") {
            let comparison = config.check_version_compatibility("kani", "0.63.0");
            assert_eq!(comparison, Some(VersionComparison::Satisfies));

            let comparison = config.check_version_compatibility("kani", "0.62.0");
            assert!(matches!(
                comparison,
                Some(VersionComparison::Mismatch { .. })
            ));
        }

        // Test minimum version requirement
        let comparison = config.check_version_compatibility("cargo-fuzz", "0.12.1");
        assert!(matches!(comparison, Some(VersionComparison::Newer { .. })));

        let comparison = config.check_version_compatibility("cargo-fuzz", "0.11.0");
        assert!(matches!(comparison, Some(VersionComparison::TooOld { .. })));
    }

    #[test]
    fn test_config_serialization() {
        let config = ToolVersionConfig::default();
        let toml_str = config.to_toml().expect("Should serialize to TOML");
        let loaded_config =
            ToolVersionConfig::from_toml(&toml_str).expect("Should deserialize from TOML");

        assert_eq!(config.tools.len(), loaded_config.tools.len());
        assert_eq!(
            config.metadata.config_version,
            loaded_config.metadata.config_version
        );
    }
}
