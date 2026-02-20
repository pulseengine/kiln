//! Requirements traceability and safety verification
//!
//! This module implements SCORE-inspired safety verification with:
//! - Requirements file parsing and validation
//! - File traceability checking
//! - ASIL compliance verification
//! - Certification readiness assessment
//! - Enhanced safety requirement modeling

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use colored::Colorize;
use serde::{Deserialize, Serialize};

use self::model::{
    CoverageLevel, RequirementId, RequirementRegistry, RequirementType, SafetyRequirement,
    VerificationMethod, VerificationStatus,
};
use super::model;
use crate::{
    config::AsilLevel,
    error::{BuildError, BuildResult},
};

/// Requirements file structure
#[derive(Debug, Deserialize)]
pub struct Requirements {
    /// Requirements metadata
    pub metadata: RequirementsMetadata,
    /// List of requirements
    pub requirement: Vec<Requirement>,
}

/// Requirements metadata
#[derive(Debug, Deserialize)]
pub struct RequirementsMetadata {
    /// Project name
    pub project: String,
    /// Project version
    pub version: String,
    /// ASIL level for the project
    pub asil_level: String,
    /// Verification method used
    pub verification_method: String,
}

/// Individual requirement definition
#[derive(Debug, Deserialize, Clone)]
pub struct Requirement {
    /// Unique requirement ID
    pub id: String,
    /// Requirement name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// ASIL level for this requirement
    pub asil_level: String,
    /// Requirement category
    pub category: String,
    /// Verification method for this requirement
    pub verification_method: String,
    /// Source files implementing this requirement
    pub source_files: Vec<String>,
    /// Test files verifying this requirement
    pub test_files: Vec<String>,
    /// Documentation files for this requirement
    pub documentation_files: Vec<String>,
    /// Current implementation status
    pub status: String,
    /// Target platforms for this requirement
    pub platform: Vec<String>,
}

/// Requirements verification results
#[derive(Debug, Serialize)]
pub struct RequirementsVerificationResult {
    /// Total number of requirements
    pub total_requirements: usize,
    /// Number of verified requirements
    pub verified_requirements: usize,
    /// List of missing files
    pub missing_files: Vec<String>,
    /// List of incomplete requirements
    pub incomplete_requirements: Vec<String>,
    /// Certification readiness percentage
    pub certification_readiness: f64,
}

impl Requirements {
    /// Load requirements from TOML file
    pub fn load(path: &Path) -> BuildResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            BuildError::Workspace(format!("Failed to read requirements file: {}", e))
        })?;

        toml::from_str(&content)
            .map_err(|e| BuildError::Verification(format!("Failed to parse requirements: {}", e)))
    }

    /// Convert to enhanced requirement registry
    pub fn to_registry(&self) -> RequirementRegistry {
        let mut registry = RequirementRegistry::new();

        for req in &self.requirement {
            let safety_req = req.to_safety_requirement();
            registry.add_requirement(safety_req);
        }

        registry
    }

    /// Initialize a sample requirements file
    pub fn init_sample(path: &Path) -> BuildResult<()> {
        let sample = r#"[metadata]
project = "WRT - WebAssembly Runtime"
version = "0.2.0"
asil_level = "ASIL-D"
verification_method = "SCORE-inspired"

[[requirement]]
id = "REQ-SAFETY-001"
name = "Memory Safety"
description = "All memory operations shall be bounds-checked and verified"
asil_level = "ASIL-D"
category = "Safety"
verification_method = "Static Analysis + Formal Verification"
source_files = ["wrt-runtime/src/memory.rs", "wrt-foundation/src/bounded.rs"]
test_files = ["wrt-runtime/src/memory_test.rs"]
documentation_files = ["docs/source/safety/memory_safety.rst"]
status = "Implemented"
platform = ["all"]

[[requirement]]
id = "REQ-SAFETY-002"
name = "No Panic in Production"
description = "Production code shall not contain panic! macros"
asil_level = "ASIL-D"
category = "Safety"
verification_method = "Static Analysis"
source_files = ["wrt-*/src/**/*.rs"]
test_files = ["xtask/src/check_panics.rs"]
documentation_files = ["docs/source/safety/panic_handling.rst"]
status = "Implemented"
platform = ["all"]
"#;

        std::fs::write(path, sample).map_err(|e| {
            BuildError::Verification(format!("Failed to write requirements file: {}", e))
        })?;

        println!(
            "{} Created sample requirements.toml at {}",
            "✅".bright_green(),
            path.display()
        );
        Ok(())
    }

    /// Verify all requirements
    pub fn verify(&self, workspace_root: &Path) -> BuildResult<RequirementsVerificationResult> {
        let mut missing_files = Vec::new();
        let mut incomplete_requirements = Vec::new();
        let mut verified_count = 0;

        for req in &self.requirement {
            let mut req_complete = true;

            // Check source files
            for file in &req.source_files {
                let path = workspace_root.join(file);
                if !path.exists() && !file.contains("*") {
                    missing_files.push(format!("{} (source for {})", file, req.id));
                    req_complete = false;
                }
            }

            // Check test files
            for file in &req.test_files {
                let path = workspace_root.join(file);
                if !path.exists() && !file.contains("*") {
                    missing_files.push(format!("{} (test for {})", file, req.id));
                    req_complete = false;
                }
            }

            // Check documentation files
            for file in &req.documentation_files {
                let path = workspace_root.join(file);
                if !path.exists() && !file.contains("*") {
                    missing_files.push(format!("{} (doc for {})", file, req.id));
                    req_complete = false;
                }
            }

            if req_complete {
                verified_count += 1;
            } else {
                incomplete_requirements.push(req.id.clone());
            }
        }

        let certification_readiness =
            (verified_count as f64 / self.requirement.len() as f64) * 100.0;

        Ok(RequirementsVerificationResult {
            total_requirements: self.requirement.len(),
            verified_requirements: verified_count,
            missing_files,
            incomplete_requirements,
            certification_readiness,
        })
    }

    /// Generate requirements traceability matrix
    pub fn generate_traceability_matrix(&self) -> String {
        let mut matrix = String::new();

        matrix.push_str("# Requirements Traceability Matrix\n\n");
        matrix.push_str("| ID | Name | ASIL | Status | Sources | Tests | Docs |\n");
        matrix.push_str("|-------|------|------|--------|---------|-------|------|\n");

        for req in &self.requirement {
            matrix.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                req.id,
                req.name,
                req.asil_level,
                req.status,
                req.source_files.len(),
                req.test_files.len(),
                req.documentation_files.len()
            ));
        }

        matrix
    }
}

impl Requirement {
    /// Convert simple requirement to enhanced SafetyRequirement
    pub fn to_safety_requirement(&self) -> SafetyRequirement {
        let mut req = SafetyRequirement::new(
            RequirementId::new(&self.id),
            self.name.clone(),
            self.description.clone(),
            self.parse_requirement_type(),
            self.parse_asil_level(),
        );

        // Set verification method
        req.verification_method = self.parse_verification_method();

        // Add source files as implementations
        for src in &self.source_files {
            req.add_implementation(src.clone());
        }

        // Add test files
        for test in &self.test_files {
            req.add_test(test.clone());
        }

        // Add documentation
        for doc in &self.documentation_files {
            req.add_documentation(doc.clone());
        }

        // Set status based on simple status field
        req.status = match self.status.to_lowercase().as_str() {
            "implemented" | "verified" | "complete" => VerificationStatus::Verified,
            "in_progress" | "partial" => VerificationStatus::InProgress,
            "not_started" | "pending" => VerificationStatus::NotStarted,
            _ => VerificationStatus::NotStarted,
        };

        // Estimate coverage level based on test files
        req.coverage = if self.test_files.is_empty() {
            CoverageLevel::None
        } else if self.test_files.len() == 1 {
            CoverageLevel::Basic
        } else {
            CoverageLevel::Comprehensive
        };

        req
    }

    /// Parse requirement type from category string
    fn parse_requirement_type(&self) -> RequirementType {
        match self.category.to_lowercase().as_str() {
            "functional" => RequirementType::Functional,
            "performance" => RequirementType::Performance,
            "safety" => RequirementType::Safety,
            "security" => RequirementType::Security,
            "reliability" => RequirementType::Reliability,
            "qualification" => RequirementType::Qualification,
            "platform" => RequirementType::Platform,
            "memory" => RequirementType::Memory,
            _ => RequirementType::Functional,
        }
    }

    /// Parse ASIL level from string
    fn parse_asil_level(&self) -> AsilLevel {
        match self.asil_level.to_uppercase().as_str() {
            "QM" => AsilLevel::QM,
            "ASIL-A" | "ASIL_A" | "A" => AsilLevel::A,
            "ASIL-B" | "ASIL_B" | "B" => AsilLevel::B,
            "ASIL-C" | "ASIL_C" | "C" => AsilLevel::C,
            "ASIL-D" | "ASIL_D" | "D" => AsilLevel::D,
            _ => AsilLevel::QM,
        }
    }

    /// Parse verification method from string
    fn parse_verification_method(&self) -> VerificationMethod {
        let method = self.verification_method.to_lowercase();
        if method.contains("inspection") || method.contains("review") {
            VerificationMethod::Inspection
        } else if method.contains("analysis") {
            VerificationMethod::Analysis
        } else if method.contains("test") {
            VerificationMethod::Test
        } else if method.contains("demonstration") || method.contains("demo") {
            VerificationMethod::Demonstration
        } else if method.contains("simulation") || method.contains("sim") {
            VerificationMethod::Simulation
        } else if method.contains("formal") || method.contains("proof") {
            VerificationMethod::FormalProof
        } else {
            VerificationMethod::Test
        }
    }
}

/// Enhanced requirements verification using SCORE methodology
pub struct EnhancedRequirementsVerifier {
    /// Path to workspace root
    workspace_root: PathBuf,
    /// Requirements registry
    registry: RequirementRegistry,
}

impl EnhancedRequirementsVerifier {
    /// Create a new enhanced verifier
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            registry: RequirementRegistry::new(),
        }
    }

    /// Load requirements from file and convert to enhanced model
    pub fn load_requirements(&mut self, path: &Path) -> BuildResult<()> {
        let requirements = Requirements::load(path)?;
        self.registry = requirements.to_registry();
        Ok(())
    }

    /// Verify all requirements with enhanced checking
    pub fn verify_all(&mut self) -> BuildResult<()> {
        // First do file-based verification
        self.verify_file_references()?;

        // Then update status based on verification results
        self.update_verification_status();

        Ok(())
    }

    /// Verify file references for all requirements
    fn verify_file_references(&self) -> BuildResult<()> {
        for req in &self.registry.requirements {
            // Check implementation files
            for impl_file in &req.implementations {
                let path = self.workspace_root.join(impl_file);
                if !path.exists() && !impl_file.contains("*") {
                    println!(
                        "{} Missing implementation file for {}: {}",
                        "⚠️ ".yellow(),
                        req.id,
                        impl_file
                    );
                }
            }

            // Check test files
            for test_file in &req.tests {
                let path = self.workspace_root.join(test_file);
                if !path.exists() && !test_file.contains("*") {
                    println!(
                        "{} Missing test file for {}: {}",
                        "⚠️ ".yellow(),
                        req.id,
                        test_file
                    );
                }
            }

            // Check documentation files
            for doc_file in &req.documentation {
                let path = self.workspace_root.join(doc_file);
                if !path.exists() && !doc_file.contains("*") {
                    println!(
                        "{} Missing documentation file for {}: {}",
                        "⚠️ ".yellow(),
                        req.id,
                        doc_file
                    );
                }
            }
        }

        Ok(())
    }

    /// Update verification status based on file existence
    fn update_verification_status(&mut self) {
        for req in &mut self.registry.requirements {
            let mut all_files_exist = true;

            // Check all referenced files
            for file in req
                .implementations
                .iter()
                .chain(req.tests.iter())
                .chain(req.documentation.iter())
            {
                let path = self.workspace_root.join(file);
                if !path.exists() && !file.contains("*") {
                    all_files_exist = false;
                    break;
                }
            }

            // Update status if needed
            if matches!(req.status, VerificationStatus::Verified) && !all_files_exist {
                req.status = VerificationStatus::Failed("Missing referenced files".to_string());
            }
        }
    }

    /// Get the requirement registry
    pub fn registry(&self) -> &RequirementRegistry {
        &self.registry
    }

    /// Get mutable requirement registry
    pub fn registry_mut(&mut self) -> &mut RequirementRegistry {
        &mut self.registry
    }
}
