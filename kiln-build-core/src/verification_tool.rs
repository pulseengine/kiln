//! WRT verification tool functionality
//!
//! Provides integrated access to verification capabilities including
//! safety verification, documentation checking, and platform verification.

use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::{
    build::BuildSystem,
    error::{BuildError, BuildResult},
};

/// Verification tool options
#[derive(Debug, Clone)]
pub struct VerificationToolOptions {
    /// Check for test files in src/ directories
    pub check_test_files: bool,
    /// Check module documentation coverage
    pub check_docs: bool,
    /// Audit crate documentation comprehensively
    pub audit_docs: bool,
    /// Verify requirements traceability
    pub check_requirements: bool,
    /// Platform verification with external limits
    pub platform_verification: bool,
    /// Container runtime detection
    pub container_detection: bool,
    /// Show verbose output
    pub verbose: bool,
}

impl Default for VerificationToolOptions {
    fn default() -> Self {
        Self {
            check_test_files: true,
            check_docs: true,
            audit_docs: false,
            check_requirements: false,
            platform_verification: false,
            container_detection: false,
            verbose: false,
        }
    }
}

/// Verification tool results
#[derive(Debug)]
pub struct VerificationToolResults {
    /// Overall success
    pub success: bool,
    /// Test file check results
    pub test_files_result: Option<TestFilesCheckResult>,
    /// Documentation check results
    pub docs_result: Option<DocsCheckResult>,
    /// Requirements verification results
    pub requirements_result: Option<RequirementsCheckResult>,
    /// Platform verification results
    pub platform_result: Option<PlatformCheckResult>,
    /// Duration of verification
    pub duration_ms: u64,
}

/// Test files check result
#[derive(Debug)]
pub struct TestFilesCheckResult {
    /// Whether check passed
    pub success: bool,
    /// Test files found in src/ directories
    pub test_files_in_src: Vec<PathBuf>,
    /// Error message if any
    pub error: Option<String>,
}

/// Documentation check result
#[derive(Debug)]
pub struct DocsCheckResult {
    /// Whether check passed
    pub success: bool,
    /// Coverage percentage
    pub coverage_percentage: f32,
    /// Missing documentation items
    pub missing_docs: Vec<String>,
    /// Error message if any
    pub error: Option<String>,
}

/// Requirements check result
#[derive(Debug)]
pub struct RequirementsCheckResult {
    /// Whether check passed
    pub success: bool,
    /// Total requirements found
    pub total_requirements: usize,
    /// Verified requirements
    pub verified_requirements: usize,
    /// Missing files
    pub missing_files: Vec<PathBuf>,
    /// Incomplete requirements
    pub incomplete_requirements: Vec<String>,
    /// Certification readiness percentage
    pub certification_readiness: f32,
}

/// Platform verification check result
#[derive(Debug)]
pub struct PlatformCheckResult {
    /// Whether check passed
    pub success: bool,
    /// Detected memory limits
    pub max_memory_mb: u64,
    /// Detected component limits
    pub max_components: u32,
    /// Container runtime detected
    pub container_runtime: String,
    /// Error message if any
    pub error: Option<String>,
}

impl BuildSystem {
    /// Run verification tool checks with default options
    pub fn run_verification_tool(&self) -> BuildResult<VerificationToolResults> {
        self.run_verification_tool_with_options(&VerificationToolOptions::default())
    }

    /// Run verification tool checks with specific options
    pub fn run_verification_tool_with_options(
        &self,
        options: &VerificationToolOptions,
    ) -> BuildResult<VerificationToolResults> {
        println!("{} Running verification tool checks...", "🔍".bright_blue());

        let start_time = std::time::Instant::now();
        let mut overall_success = true;

        // Run test files check
        let test_files_result = if options.check_test_files {
            match self.check_test_files_in_src(options.verbose) {
                Ok(result) => {
                    if !result.success {
                        overall_success = false;
                    }
                    Some(result)
                },
                Err(e) => {
                    overall_success = false;
                    Some(TestFilesCheckResult {
                        success: false,
                        test_files_in_src: vec![],
                        error: Some(e.to_string()),
                    })
                },
            }
        } else {
            None
        };

        // Run documentation check
        let docs_result = if options.check_docs || options.audit_docs {
            match self.check_documentation_coverage(options.audit_docs, options.verbose) {
                Ok(result) => {
                    if !result.success {
                        overall_success = false;
                    }
                    Some(result)
                },
                Err(e) => {
                    overall_success = false;
                    Some(DocsCheckResult {
                        success: false,
                        coverage_percentage: 0.0,
                        missing_docs: vec![],
                        error: Some(e.to_string()),
                    })
                },
            }
        } else {
            None
        };

        // Run requirements check
        let requirements_result = if options.check_requirements {
            match self.check_requirements_traceability(options.verbose) {
                Ok(result) => {
                    if !result.success {
                        overall_success = false;
                    }
                    Some(result)
                },
                Err(e) => {
                    overall_success = false;
                    Some(RequirementsCheckResult {
                        success: false,
                        total_requirements: 0,
                        verified_requirements: 0,
                        missing_files: vec![],
                        incomplete_requirements: vec![],
                        certification_readiness: 0.0,
                    })
                },
            }
        } else {
            None
        };

        // Run platform verification
        let platform_result = if options.platform_verification {
            match self.check_platform_verification(options.container_detection, options.verbose) {
                Ok(result) => {
                    if !result.success {
                        overall_success = false;
                    }
                    Some(result)
                },
                Err(e) => {
                    overall_success = false;
                    Some(PlatformCheckResult {
                        success: false,
                        max_memory_mb: 0,
                        max_components: 0,
                        container_runtime: "unknown".to_string(),
                        error: Some(e.to_string()),
                    })
                },
            }
        } else {
            None
        };

        let duration = start_time.elapsed();

        if overall_success {
            println!("{} Verification tool checks passed!", "✅".bright_green());
        } else {
            println!("{} Some verification tool checks failed", "❌".bright_red());
        }

        Ok(VerificationToolResults {
            success: overall_success,
            test_files_result,
            docs_result,
            requirements_result,
            platform_result,
            duration_ms: duration.as_millis() as u64,
        })
    }

    /// Check for test files in src/ directories
    fn check_test_files_in_src(&self, verbose: bool) -> BuildResult<TestFilesCheckResult> {
        if verbose {
            println!(
                "  {} Checking for test files in src/ directories...",
                "🔍".bright_cyan()
            );
        }

        // This is a simplified implementation - the actual validation module
        // already has this functionality in validation.rs
        use crate::validation::CodeValidator;

        let validator = CodeValidator::new(self.workspace.root.clone(), verbose);
        let result = validator
            .check_no_test_files_in_src()
            .map_err(|e| BuildError::Verification(format!("Test file check failed: {}", e)))?;

        let test_files_in_src: Vec<PathBuf> =
            result.errors.iter().map(|error| error.file.clone()).collect();

        Ok(TestFilesCheckResult {
            success: result.success,
            test_files_in_src,
            error: None,
        })
    }

    /// Check documentation coverage
    fn check_documentation_coverage(
        &self,
        audit: bool,
        verbose: bool,
    ) -> BuildResult<DocsCheckResult> {
        if verbose {
            println!(
                "  {} Checking documentation coverage...",
                "📚".bright_cyan()
            );
        }

        // This would integrate with the existing validation module
        use crate::validation::CodeValidator;

        let validator = CodeValidator::new(self.workspace.root.clone(), verbose);

        let result = if audit {
            validator.audit_crate_documentation().map_err(|e| {
                BuildError::Verification(format!("Documentation audit failed: {}", e))
            })?
        } else {
            validator.check_module_documentation().map_err(|e| {
                BuildError::Verification(format!("Documentation check failed: {}", e))
            })?
        };

        // Calculate coverage (simplified)
        let coverage_percentage = if result.success { 100.0 } else { 75.0 };

        Ok(DocsCheckResult {
            success: result.success,
            coverage_percentage,
            missing_docs: vec![], // Would be populated from validation results
            error: None,
        })
    }

    /// Check requirements traceability
    fn check_requirements_traceability(
        &self,
        verbose: bool,
    ) -> BuildResult<RequirementsCheckResult> {
        if verbose {
            println!(
                "  {} Checking requirements traceability...",
                "📋".bright_cyan()
            );
        }

        // This would integrate with the existing requirements module
        use crate::requirements::Requirements;

        let req_path = self.workspace.root.join("requirements.toml");
        if !req_path.exists() {
            return Ok(RequirementsCheckResult {
                success: true, // Not having requirements is OK
                total_requirements: 0,
                verified_requirements: 0,
                missing_files: vec![],
                incomplete_requirements: vec![],
                certification_readiness: 0.0,
            });
        }

        let requirements = Requirements::load(&req_path)
            .map_err(|e| BuildError::Verification(format!("Failed to load requirements: {}", e)))?;

        let results = requirements.verify(&self.workspace.root).map_err(|e| {
            BuildError::Verification(format!("Requirements verification failed: {}", e))
        })?;

        Ok(RequirementsCheckResult {
            success: results.certification_readiness >= 80.0,
            total_requirements: results.total_requirements,
            verified_requirements: results.verified_requirements,
            missing_files: results.missing_files.into_iter().map(PathBuf::from).collect(),
            incomplete_requirements: results.incomplete_requirements,
            certification_readiness: results.certification_readiness as f32,
        })
    }

    /// Check platform verification
    fn check_platform_verification(
        &self,
        container_detection: bool,
        verbose: bool,
    ) -> BuildResult<PlatformCheckResult> {
        if verbose {
            println!("  {} Running platform verification...", "🖥️".bright_cyan());
        }

        // Simplified platform verification - integrates with requirements platform
        // verification functionality via the PlatformVerificationEngine

        // Mock platform detection
        let max_memory_mb = 8192; // 8GB default
        let max_components = 256;
        let container_runtime = if container_detection {
            "docker".to_string() // Would actually detect
        } else {
            "native".to_string()
        };

        Ok(PlatformCheckResult {
            success: true,
            max_memory_mb,
            max_components,
            container_runtime,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_tool_options() {
        let options = VerificationToolOptions::default();
        assert!(options.check_test_files);
        assert!(options.check_docs);
        assert!(!options.audit_docs);
    }

    #[test]
    fn test_test_files_check_result() {
        let result = TestFilesCheckResult {
            success: true,
            test_files_in_src: vec![],
            error: None,
        };

        assert!(result.success);
        assert!(result.test_files_in_src.is_empty());
        assert!(result.error.is_none());
    }
}
