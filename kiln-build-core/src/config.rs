//! Configuration management for the WRT build system

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BuildError, BuildResult};

/// Build configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Whether to enable verbose output
    pub verbose: bool,
    /// Number of parallel jobs (-1 for auto)
    pub jobs: i32,
    /// Build profile (dev, release, test)
    pub profile: BuildProfile,
    /// Target architecture filter
    pub target_filter: Vec<String>,
    /// Feature flags to enable
    pub features: Vec<String>,
    /// Whether to run clippy checks
    pub clippy: bool,
    /// Whether to run format checks
    pub format_check: bool,
    /// Show commands without executing them
    pub dry_run: bool,
    /// Trace all external commands being executed
    pub trace_commands: bool,
}

/// Build profiles available
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildProfile {
    /// Development build (fast compile, debug info)
    Dev,
    /// Release build (optimized, no debug info)
    Release,
    /// Test build (for testing purposes)
    Test,
}

impl Default for BuildProfile {
    fn default() -> Self {
        BuildProfile::Dev
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            jobs: -1, // Auto-detect
            profile: BuildProfile::default(),
            target_filter: vec![],
            features: vec![],
            clippy: true,
            format_check: true,
            dry_run: false,
            trace_commands: false,
        }
    }
}

/// Workspace configuration and metadata
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Root directory of the workspace
    pub root: PathBuf,
    /// List of member crates
    pub members: Vec<String>,
    /// Workspace-level dependencies
    pub dependencies: Vec<String>,
    /// ASIL level for safety verification
    pub asil_level: AsilLevel,
}

/// ASIL (Automotive Safety Integrity Level) configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash, Serialize, Deserialize)]
pub enum AsilLevel {
    /// Quality Management (no specific safety requirements)
    QM,
    /// ASIL-A (lowest safety integrity level)
    A,
    /// ASIL-B
    B,
    /// ASIL-C
    C,
    /// ASIL-D (highest safety integrity level)
    D,
}

impl Default for AsilLevel {
    fn default() -> Self {
        AsilLevel::QM
    }
}

impl std::fmt::Display for AsilLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsilLevel::QM => write!(f, "QM"),
            AsilLevel::A => write!(f, "ASIL-A"),
            AsilLevel::B => write!(f, "ASIL-B"),
            AsilLevel::C => write!(f, "ASIL-C"),
            AsilLevel::D => write!(f, "ASIL-D"),
        }
    }
}

impl WorkspaceConfig {
    /// Load workspace configuration from Cargo.toml
    pub fn load(workspace_root: &std::path::Path) -> BuildResult<Self> {
        let cargo_toml = workspace_root.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(BuildError::Workspace(
                "Cargo.toml not found in workspace root".to_string(),
            ));
        }

        let content = std::fs::read_to_string(&cargo_toml)
            .map_err(|e| BuildError::Workspace(format!("Failed to read Cargo.toml: {}", e)))?;

        // Parse workspace members (simplified - real implementation would use toml
        // crate)
        let members = Self::parse_workspace_members(&content)?;

        Ok(Self {
            root: workspace_root.to_path_buf(),
            members,
            dependencies: vec![], // TODO: Parse from Cargo.toml
            asil_level: AsilLevel::default(),
        })
    }

    /// Parse workspace members from Cargo.toml content
    fn parse_workspace_members(content: &str) -> BuildResult<Vec<String>> {
        let mut members = Vec::new();
        let mut in_workspace = false;
        let mut in_members = false;

        for line in content.lines() {
            let line = line.trim();

            if line == "[workspace]" {
                in_workspace = true;
                continue;
            }

            if in_workspace && line.starts_with('[') && line != "[workspace]" {
                in_workspace = false;
                in_members = false;
                continue;
            }

            if in_workspace && line == "members = [" {
                in_members = true;
                continue;
            }

            if in_members && line == "]" {
                in_members = false;
                continue;
            }

            if in_members && !line.is_empty() {
                let member = line.trim_matches(|c| c == '"' || c == ',' || c == ' ');
                if !member.starts_with('#') && !member.is_empty() {
                    members.push(member.to_string());
                }
            }
        }

        Ok(members)
    }

    /// Get all buildable crate paths
    pub fn crate_paths(&self) -> Vec<PathBuf> {
        self.members
            .iter()
            .map(|member| self.root.join(member))
            .filter(|path| path.exists())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_build_config() {
        let config = BuildConfig::default();
        assert!(!config.verbose);
        assert_eq!(config.jobs, -1);
        assert!(matches!(config.profile, BuildProfile::Dev));
    }

    #[test]
    fn test_workspace_member_parsing() {
        let content = r#"
[workspace]
members = [
    "wrt",
    "wrt-runtime",
    # "commented-out",
    "wrt-component",
]
        "#;

        let members = WorkspaceConfig::parse_workspace_members(content).unwrap();
        assert_eq!(members, vec!["wrt", "wrt-runtime", "wrt-component"]);
    }
}
