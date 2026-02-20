//! Smart defaults and context detection for cargo-wrt
//!
//! Automatically detects project context and suggests appropriate
//! default behaviors and command options.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Project context detected from the workspace
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    pub workspace_root: PathBuf,
    pub project_type: ProjectType,
    pub features: ProjectFeatures,
    pub git_context: Option<GitContext>,
    pub ci_context: Option<CiContext>,
    pub recommendations: Vec<Recommendation>,
}

/// Type of project detected
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    /// WRT workspace (full project)
    WrtWorkspace,
    /// Single WRT crate
    WrtCrate { name: String },
    /// Generic Rust workspace
    RustWorkspace,
    /// Single Rust crate
    RustCrate,
    /// Unknown/invalid
    Unknown,
}

/// Features detected in the project
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectFeatures {
    pub has_tests: bool,
    pub has_benchmarks: bool,
    pub has_examples: bool,
    pub has_docs: bool,
    pub has_ci: bool,
    pub has_fuzzing: bool,
    pub has_safety_verification: bool,
    pub no_std_support: bool,
    pub webassembly_targets: bool,
}

/// Git context information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitContext {
    pub current_branch: String,
    pub is_clean: bool,
    pub has_staged_changes: bool,
    pub has_unstaged_changes: bool,
    pub remote_url: Option<String>,
    pub is_github: bool,
}

/// CI/CD context information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiContext {
    pub provider: CiProvider,
    pub is_running_in_ci: bool,
    pub branch: Option<String>,
    pub pr_number: Option<u32>,
    pub build_number: Option<String>,
}

/// CI providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiProvider {
    GitHubActions,
    GitLab,
    Travis,
    CircleCI,
    Jenkins,
    Other(String),
}

/// Smart recommendation for user
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub title: String,
    pub description: String,
    pub command: Option<String>,
    pub priority: RecommendationPriority,
}

/// Recommendation categories
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendationCategory {
    Setup,
    Build,
    Test,
    Documentation,
    Safety,
    Performance,
    Maintenance,
}

/// Recommendation priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecommendationPriority {
    Critical,
    High,
    Medium,
    Low,
    Suggestion,
}

impl RecommendationPriority {
    pub fn emoji(&self) -> &'static str {
        match self {
            RecommendationPriority::Critical => "🚨",
            RecommendationPriority::High => "⚠️",
            RecommendationPriority::Medium => "💡",
            RecommendationPriority::Low => "ℹ️",
            RecommendationPriority::Suggestion => "💭",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            RecommendationPriority::Critical => "Critical",
            RecommendationPriority::High => "High",
            RecommendationPriority::Medium => "Medium",
            RecommendationPriority::Low => "Low",
            RecommendationPriority::Suggestion => "Suggestion",
        }
    }
}

/// Context detector for smart defaults
pub struct ContextDetector {
    workspace_root: PathBuf,
}

impl ContextDetector {
    /// Create a new context detector
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Detect project context
    pub fn detect(&self) -> Result<ProjectContext> {
        let project_type = self.detect_project_type()?;
        let features = self.detect_features()?;
        let git_context = self.detect_git_context().ok();
        let ci_context = self.detect_ci_context().ok();
        let recommendations =
            self.generate_recommendations(&project_type, &features, &git_context, &ci_context);

        Ok(ProjectContext {
            workspace_root: self.workspace_root.clone(),
            project_type,
            features,
            git_context,
            ci_context,
            recommendations,
        })
    }

    /// Detect the type of project
    fn detect_project_type(&self) -> Result<ProjectType> {
        // Check for WRT workspace
        if self.workspace_root.join("wrt-foundation").exists()
            && self.workspace_root.join("wrt-build-core").exists()
            && self.workspace_root.join("cargo-wrt").exists()
        {
            return Ok(ProjectType::WrtWorkspace);
        }

        // Check for single WRT crate
        if let Ok(manifest) = fs::read_to_string(self.workspace_root.join("Cargo.toml")) {
            if manifest.contains("wrt-") || manifest.contains("name = \"wrt") {
                if let Some(name) = self.extract_crate_name(&manifest) {
                    return Ok(ProjectType::WrtCrate { name });
                }
            }
        }

        // Check for Rust workspace
        if self.workspace_root.join("Cargo.toml").exists() {
            if let Ok(manifest) = fs::read_to_string(self.workspace_root.join("Cargo.toml")) {
                if manifest.contains("[workspace]") {
                    return Ok(ProjectType::RustWorkspace);
                } else {
                    return Ok(ProjectType::RustCrate);
                }
            }
        }

        Ok(ProjectType::Unknown)
    }

    /// Extract crate name from Cargo.toml
    fn extract_crate_name(&self, manifest: &str) -> Option<String> {
        for line in manifest.lines() {
            if let Some(name_part) = line.strip_prefix("name = ") {
                let name = name_part.trim_matches('"').trim_matches('\'');
                return Some(name.to_string());
            }
        }
        None
    }

    /// Detect project features
    fn detect_features(&self) -> Result<ProjectFeatures> {
        let mut features = ProjectFeatures::default();

        // Check for tests
        features.has_tests = self.workspace_root.join("tests").exists()
            || self.has_files_matching("**/tests/**/*.rs")
            || self.has_files_matching("**/*test*.rs");

        // Check for benchmarks
        features.has_benchmarks = self.workspace_root.join("benches").exists()
            || self.has_files_matching("**/benches/**/*.rs");

        // Check for examples
        features.has_examples = self.workspace_root.join("examples").exists();

        // Check for documentation
        features.has_docs = self.workspace_root.join("docs").exists()
            || self.workspace_root.join("README.md").exists();

        // Check for CI
        features.has_ci = self.workspace_root.join(".github").exists()
            || self.workspace_root.join(".gitlab-ci.yml").exists()
            || self.workspace_root.join(".travis.yml").exists();

        // Check for fuzzing
        features.has_fuzzing = self.workspace_root.join("fuzz").exists()
            || self.has_files_matching("**/fuzz_targets/**/*.rs");

        // Check for safety verification
        features.has_safety_verification = self.workspace_root.join("requirements.toml").exists()
            || self.has_files_matching("**/*safety*.rs")
            || self.has_files_matching("**/*verification*.rs");

        // Check for no_std support
        features.no_std_support = self.check_no_std_support();

        // Check for WebAssembly targets
        features.webassembly_targets = self.check_wasm_targets();

        Ok(features)
    }

    /// Check if files matching pattern exist
    fn has_files_matching(&self, _pattern: &str) -> bool {
        // Simplified implementation - in practice would use glob crate
        false
    }

    /// Check for no_std support
    fn check_no_std_support(&self) -> bool {
        if let Ok(manifest) = fs::read_to_string(self.workspace_root.join("Cargo.toml")) {
            manifest.contains("no_std") || manifest.contains("default-features = false")
        } else {
            false
        }
    }

    /// Check for WebAssembly target support
    fn check_wasm_targets(&self) -> bool {
        if let Ok(config) = fs::read_to_string(self.workspace_root.join(".cargo/config.toml")) {
            config.contains("wasm") || config.contains("wasi")
        } else {
            false
        }
    }

    /// Detect git context
    fn detect_git_context(&self) -> Result<GitContext> {
        // Simplified implementation - would use git2 crate in practice
        let git_dir = self.workspace_root.join(".git");
        if !git_dir.exists() {
            anyhow::bail!("Not a git repository");
        }

        Ok(GitContext {
            current_branch: "main".to_string(), // Would query actual branch
            is_clean: true,                     // Would check git status
            has_staged_changes: false,
            has_unstaged_changes: false,
            remote_url: None, // Would query git remote
            is_github: false,
        })
    }

    /// Detect CI context
    fn detect_ci_context(&self) -> Result<CiContext> {
        // Check environment variables for CI detection
        let is_ci = std::env::var("CI").is_ok();

        let provider = if std::env::var("GITHUB_ACTIONS").is_ok() {
            CiProvider::GitHubActions
        } else if std::env::var("GITLAB_CI").is_ok() {
            CiProvider::GitLab
        } else if std::env::var("TRAVIS").is_ok() {
            CiProvider::Travis
        } else if std::env::var("CIRCLECI").is_ok() {
            CiProvider::CircleCI
        } else if std::env::var("JENKINS_URL").is_ok() {
            CiProvider::Jenkins
        } else {
            CiProvider::Other("unknown".to_string())
        };

        Ok(CiContext {
            provider,
            is_running_in_ci: is_ci,
            branch: std::env::var("GITHUB_REF_NAME").ok(),
            pr_number: std::env::var("GITHUB_PR_NUMBER").ok().and_then(|s| s.parse().ok()),
            build_number: std::env::var("GITHUB_RUN_NUMBER").ok(),
        })
    }

    /// Generate smart recommendations
    fn generate_recommendations(
        &self,
        project_type: &ProjectType,
        features: &ProjectFeatures,
        git_context: &Option<GitContext>,
        ci_context: &Option<CiContext>,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        // Project setup recommendations
        match project_type {
            ProjectType::WrtWorkspace => {
                if !features.has_safety_verification {
                    recommendations.push(Recommendation {
                        category: RecommendationCategory::Setup,
                        title: "Initialize Safety Verification".to_string(),
                        description: "Set up safety verification framework for ASIL compliance"
                            .to_string(),
                        command: Some("cargo-wrt init --wrt-requirements".to_string()),
                        priority: RecommendationPriority::High,
                    });
                }
            },
            ProjectType::Unknown => {
                recommendations.push(Recommendation {
                    category: RecommendationCategory::Setup,
                    title: "Project Setup".to_string(),
                    description: "Initialize a new WRT project or navigate to existing project"
                        .to_string(),
                    command: Some("cargo-wrt init".to_string()),
                    priority: RecommendationPriority::Critical,
                });
            },
            _ => {},
        }

        // Test recommendations
        if !features.has_tests {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Test,
                title: "Add Tests".to_string(),
                description: "Create test suite to ensure code quality and reliability".to_string(),
                command: Some("cargo-wrt test --create-template".to_string()),
                priority: RecommendationPriority::Medium,
            });
        }

        // Documentation recommendations
        if !features.has_docs {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Documentation,
                title: "Generate Documentation".to_string(),
                description: "Create comprehensive API documentation".to_string(),
                command: Some("cargo-wrt docs --open".to_string()),
                priority: RecommendationPriority::Low,
            });
        }

        // CI/CD recommendations
        if git_context.is_some() && !features.has_ci {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Maintenance,
                title: "Set Up Continuous Integration".to_string(),
                description: "Add automated testing and verification workflows".to_string(),
                command: Some("cargo-wrt setup --ci".to_string()),
                priority: RecommendationPriority::Medium,
            });
        }

        // Performance recommendations
        if matches!(
            project_type,
            ProjectType::WrtWorkspace | ProjectType::WrtCrate { .. }
        ) {
            if !features.has_benchmarks {
                recommendations.push(Recommendation {
                    category: RecommendationCategory::Performance,
                    title: "Add Benchmarks".to_string(),
                    description: "Set up performance benchmarks to track runtime performance"
                        .to_string(),
                    command: Some("cargo-wrt benchmark --init".to_string()),
                    priority: RecommendationPriority::Suggestion,
                });
            }
        }

        // Sort by priority
        recommendations.sort_by_key(|r| r.priority.clone());
        recommendations
    }
}

/// Smart defaults provider
pub struct SmartDefaults {
    context: ProjectContext,
}

impl SmartDefaults {
    /// Create smart defaults from project context
    pub fn new(context: ProjectContext) -> Self {
        Self { context }
    }

    /// Get default command based on context
    pub fn suggest_default_command(&self) -> Option<String> {
        // If in CI, suggest verify
        if let Some(ci) = &self.context.ci_context {
            if ci.is_running_in_ci {
                return Some("verify --asil d".to_string());
            }
        }

        // If git has changes, suggest check first
        if let Some(git) = &self.context.git_context {
            if git.has_unstaged_changes || git.has_staged_changes {
                return Some("check".to_string());
            }
        }

        // Default based on project type
        match &self.context.project_type {
            ProjectType::WrtWorkspace => Some("build".to_string()),
            ProjectType::WrtCrate { .. } => Some("test".to_string()),
            ProjectType::Unknown => Some("help".to_string()),
            _ => Some("build".to_string()),
        }
    }

    /// Get recommended output format based on context
    pub fn suggest_output_format(&self) -> String {
        if let Some(ci) = &self.context.ci_context {
            if ci.is_running_in_ci {
                return "json".to_string();
            }
        }
        "human".to_string()
    }

    /// Get recommended cache settings
    pub fn suggest_cache_settings(&self) -> bool {
        // Enable caching for CI and large projects
        if let Some(ci) = &self.context.ci_context {
            if ci.is_running_in_ci {
                return true;
            }
        }

        matches!(self.context.project_type, ProjectType::WrtWorkspace)
    }
}
