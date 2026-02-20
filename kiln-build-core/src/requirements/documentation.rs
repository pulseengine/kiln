//! Documentation Verification Framework for cargo-wrt
//!
//! This module provides comprehensive documentation verification for
//! safety-critical systems, ensuring that all requirements, implementations,
//! and tests are properly documented according to ASIL standards. Integrated
//! with cargo-wrt's diagnostic system.

use std::{collections::HashMap, fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{
    RequirementId, RequirementRegistry, RequirementType, SafetyRequirement, VerificationMethod,
};
use crate::{
    config::AsilLevel,
    diagnostics::{Diagnostic, DiagnosticCollection, Position, Range, Severity},
    error::{BuildError, BuildResult},
    formatters::OutputFormat,
};

/// Documentation verification framework that ensures proper documentation
/// coverage for safety-critical requirements with cargo-wrt integration
#[derive(Debug)]
pub struct DocumentationVerificationFramework {
    /// Registry of requirements to verify documentation for
    requirement_registry: RequirementRegistry,
    /// Documentation analysis results
    documentation_analysis: Vec<DocumentationAnalysis>,
    /// Configuration for verification standards
    verification_config: DocumentationVerificationConfig,
    /// Workspace root for file operations
    workspace_root: PathBuf,
}

impl DocumentationVerificationFramework {
    /// Create a new documentation verification framework
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            requirement_registry: RequirementRegistry::new(),
            documentation_analysis: Vec::new(),
            verification_config: DocumentationVerificationConfig::default(),
            workspace_root,
        }
    }

    /// Set the verification configuration
    pub fn with_config(mut self, config: DocumentationVerificationConfig) -> Self {
        self.verification_config = config;
        self
    }

    /// Add a requirement to be verified for documentation
    pub fn add_requirement(&mut self, requirement: SafetyRequirement) {
        self.requirement_registry.add_requirement(requirement);
    }

    /// Verify documentation for all requirements with diagnostic integration
    pub fn verify_all_documentation(
        &mut self,
    ) -> BuildResult<(DocumentationVerificationResult, DiagnosticCollection)> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "documentation-verification".to_string(),
        );

        let requirements = self.requirement_registry.requirements.clone();
        let mut violations = Vec::new();
        let mut compliant_requirements = 0;

        for requirement in &requirements {
            let analysis = self.analyze_requirement_documentation(requirement, &mut diagnostics);

            if analysis.is_compliant() {
                compliant_requirements += 1;
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "documentation-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!("Requirement {} documentation is compliant", requirement.id),
                        "documentation-verification".to_string(),
                    )
                    .with_code("doc-compliant".to_string()),
                );
            } else {
                // Collect violations and convert to diagnostics
                for violation in &analysis.violations {
                    violations.push(violation.clone());

                    let severity = match violation.severity {
                        DocumentationViolationSeverity::Critical => Severity::Error,
                        DocumentationViolationSeverity::High => Severity::Error,
                        DocumentationViolationSeverity::Medium => Severity::Warning,
                        DocumentationViolationSeverity::Low => Severity::Warning,
                        DocumentationViolationSeverity::Info => Severity::Info,
                    };

                    diagnostics.add_diagnostic(
                        Diagnostic::new(
                            "documentation-verification".to_string(),
                            Range::single_line(0, 0, 0),
                            severity,
                            format!("{}: {}", violation.violation_type, violation.description),
                            "documentation-verification".to_string(),
                        )
                        .with_code(format!("doc-{:?}", violation.violation_type).to_lowercase()),
                    );
                }
            }

            self.documentation_analysis.push(analysis);
        }

        let total_requirements = requirements.len();
        let compliance_percentage = if total_requirements > 0 {
            (compliant_requirements as f64 / total_requirements as f64) * 100.0
        } else {
            100.0
        };

        let result = DocumentationVerificationResult {
            total_requirements,
            compliant_requirements,
            compliance_percentage,
            violations,
            analysis_results: self.documentation_analysis.clone(),
            is_certification_ready: self.is_certification_ready(compliance_percentage),
        };

        // Add summary diagnostic
        if result.is_certification_ready {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "documentation-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Info,
                    format!(
                        "Documentation compliance: {:.1}% (READY FOR CERTIFICATION)",
                        compliance_percentage
                    ),
                    "documentation-verification".to_string(),
                )
                .with_code("doc-certification-ready".to_string()),
            );
        } else {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "documentation-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Warning,
                    format!(
                        "Documentation compliance: {:.1}% (NOT READY)",
                        compliance_percentage
                    ),
                    "documentation-verification".to_string(),
                )
                .with_code("doc-certification-not-ready".to_string()),
            );
        }

        Ok((result, diagnostics))
    }

    /// Verify documentation for a specific ASIL level
    pub fn verify_asil_documentation(
        &mut self,
        asil_level: AsilLevel,
    ) -> BuildResult<(DocumentationVerificationResult, DiagnosticCollection)> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            format!("documentation-verification-{}", asil_level),
        );

        let requirements = self.requirement_registry.get_requirements_by_asil(asil_level);
        let mut violations = Vec::new();
        let mut compliant_requirements = 0;

        for requirement in &requirements {
            let analysis = self.analyze_requirement_documentation(requirement, &mut diagnostics);

            if analysis.is_compliant() {
                compliant_requirements += 1;
            } else {
                for violation in &analysis.violations {
                    violations.push(violation.clone());
                }
            }
        }

        let total_requirements = requirements.len();
        let compliance_percentage = if total_requirements > 0 {
            (compliant_requirements as f64 / total_requirements as f64) * 100.0
        } else {
            100.0
        };

        let result = DocumentationVerificationResult {
            total_requirements,
            compliant_requirements,
            compliance_percentage,
            violations,
            analysis_results: self.documentation_analysis.clone(),
            is_certification_ready: self
                .is_certification_ready_for_asil(compliance_percentage, asil_level),
        };

        diagnostics.add_diagnostic(
            Diagnostic::new(
                "documentation-verification".to_string(),
                Range::single_line(0, 0, 0),
                if result.is_certification_ready { Severity::Info } else { Severity::Warning },
                format!(
                    "ASIL {} documentation compliance: {:.1}%",
                    asil_level, compliance_percentage
                ),
                "documentation-verification".to_string(),
            )
            .with_code("asil-doc-compliance".to_string()),
        );

        Ok((result, diagnostics))
    }

    /// Generate comprehensive documentation report with diagnostics
    pub fn generate_report(&self) -> (DocumentationReport, DiagnosticCollection) {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "documentation-report".to_string(),
        );

        let overall_compliance = if !self.documentation_analysis.is_empty() {
            self.documentation_analysis.iter().map(|a| a.compliance_score).sum::<f64>()
                / self.documentation_analysis.len() as f64
        } else {
            100.0
        };

        let total_violations = self.documentation_analysis.iter().map(|a| a.violations.len()).sum();

        let critical_violations = self
            .documentation_analysis
            .iter()
            .flat_map(|a| &a.violations)
            .filter(|v| v.severity == DocumentationViolationSeverity::Critical)
            .count();

        let report = DocumentationReport {
            overall_compliance,
            total_requirements: self.documentation_analysis.len(),
            total_violations,
            critical_violations,
            asil_compliance: self.calculate_asil_compliance(),
            recommendations: self.generate_recommendations(),
            analysis_summary: self.documentation_analysis.clone(),
        };

        // Generate diagnostics for the report
        diagnostics.add_diagnostic(
            Diagnostic::new(
                "documentation-report".to_string(),
                Range::single_line(0, 0, 0),
                Severity::Info,
                format!(
                    "Overall documentation compliance: {:.1}%",
                    overall_compliance
                ),
                "documentation-verification".to_string(),
            )
            .with_code("doc-overall-compliance".to_string()),
        );

        if critical_violations > 0 {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "documentation-report".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Error,
                    format!(
                        "{} critical documentation violations found",
                        critical_violations
                    ),
                    "documentation-verification".to_string(),
                )
                .with_code("doc-critical-violations".to_string()),
            );
        }

        (report, diagnostics)
    }

    /// Convert documentation verification to cargo-wrt diagnostics
    pub fn to_diagnostics(&self, output_format: OutputFormat) -> BuildResult<DiagnosticCollection> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "documentation-verification".to_string(),
        );

        // Add compliance diagnostics for each requirement
        for analysis in &self.documentation_analysis {
            if analysis.is_compliant() {
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "documentation-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!(
                            "Requirement {} documentation compliant ({:.1}%)",
                            analysis.requirement_id, analysis.compliance_score
                        ),
                        "documentation-verification".to_string(),
                    )
                    .with_code("doc-compliant".to_string()),
                );
            } else {
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "documentation-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Warning,
                        format!(
                            "Requirement {} documentation non-compliant ({:.1}%, {} violations)",
                            analysis.requirement_id,
                            analysis.compliance_score,
                            analysis.violations.len()
                        ),
                        "documentation-verification".to_string(),
                    )
                    .with_code("doc-non-compliant".to_string()),
                );
            }
        }

        Ok(diagnostics)
    }

    /// Get the requirement registry
    pub fn registry(&self) -> &RequirementRegistry {
        &self.requirement_registry
    }

    /// Get mutable requirement registry
    pub fn registry_mut(&mut self) -> &mut RequirementRegistry {
        &mut self.requirement_registry
    }

    // Private helper methods

    /// Analyze documentation for a single requirement
    fn analyze_requirement_documentation(
        &self,
        requirement: &SafetyRequirement,
        diagnostics: &mut DiagnosticCollection,
    ) -> DocumentationAnalysis {
        let mut violations = Vec::new();
        let required_standards = self.get_documentation_standards_for_asil(requirement.asil_level);

        // Check requirement documentation completeness
        if requirement.description.trim().is_empty() {
            let violation = DocumentationViolation {
                requirement_id: requirement.id.clone(),
                violation_type: DocumentationViolationType::MissingDescription,
                severity: self.get_violation_severity(
                    requirement.asil_level,
                    DocumentationViolationType::MissingDescription,
                ),
                description: "Requirement lacks detailed description".to_string(),
                location: DocumentationLocation::Requirement,
            };
            violations.push(violation);
        }

        // Check if description meets ASIL standards
        if requirement.description.len() < required_standards.min_description_length {
            let violation = DocumentationViolation {
                requirement_id: requirement.id.clone(),
                violation_type: DocumentationViolationType::InsufficientDetail,
                severity: self.get_violation_severity(
                    requirement.asil_level,
                    DocumentationViolationType::InsufficientDetail,
                ),
                description: format!(
                    "Description too brief ({}/<{} chars)",
                    requirement.description.len(),
                    required_standards.min_description_length
                ),
                location: DocumentationLocation::Requirement,
            };
            violations.push(violation);
        }

        // Check implementation documentation
        if requirement.implementations.is_empty() {
            let violation = DocumentationViolation {
                requirement_id: requirement.id.clone(),
                violation_type: DocumentationViolationType::MissingImplementation,
                severity: self.get_violation_severity(
                    requirement.asil_level,
                    DocumentationViolationType::MissingImplementation,
                ),
                description: "No implementation references found".to_string(),
                location: DocumentationLocation::Implementation,
            };
            violations.push(violation);
        } else {
            // Verify implementation documentation exists
            for impl_ref in &requirement.implementations {
                if !self.verify_implementation_documented(impl_ref) {
                    let violation = DocumentationViolation {
                        requirement_id: requirement.id.clone(),
                        violation_type: DocumentationViolationType::UndocumentedImplementation,
                        severity: self.get_violation_severity(
                            requirement.asil_level,
                            DocumentationViolationType::UndocumentedImplementation,
                        ),
                        description: format!("Implementation '{}' lacks documentation", impl_ref),
                        location: DocumentationLocation::Implementation,
                    };
                    violations.push(violation);
                }
            }
        }

        // Check test documentation
        if requirement.tests.is_empty() {
            let violation = DocumentationViolation {
                requirement_id: requirement.id.clone(),
                violation_type: DocumentationViolationType::MissingTestDocumentation,
                severity: self.get_violation_severity(
                    requirement.asil_level,
                    DocumentationViolationType::MissingTestDocumentation,
                ),
                description: "No test documentation found".to_string(),
                location: DocumentationLocation::Test,
            };
            violations.push(violation);
        }

        // Check verification documentation
        if required_standards.requires_verification_document && requirement.documentation.is_empty()
        {
            let violation = DocumentationViolation {
                requirement_id: requirement.id.clone(),
                violation_type: DocumentationViolationType::MissingVerificationDocument,
                severity: self.get_violation_severity(
                    requirement.asil_level,
                    DocumentationViolationType::MissingVerificationDocument,
                ),
                description: "Missing verification documentation".to_string(),
                location: DocumentationLocation::Verification,
            };
            violations.push(violation);
        }

        let compliance_score = self.calculate_compliance_score(&violations, &required_standards);

        DocumentationAnalysis {
            requirement_id: requirement.id.clone(),
            asil_level: requirement.asil_level,
            violations,
            compliance_score,
            required_standards,
            analyzed_locations: vec![
                DocumentationLocation::Requirement,
                DocumentationLocation::Implementation,
                DocumentationLocation::Test,
                DocumentationLocation::Verification,
            ],
        }
    }

    /// Get documentation standards for a specific ASIL level
    fn get_documentation_standards_for_asil(
        &self,
        asil_level: AsilLevel,
    ) -> DocumentationStandards {
        match asil_level {
            AsilLevel::QM => DocumentationStandards {
                min_description_length: 50,
                requires_implementation_docs: false,
                requires_test_docs: false,
                requires_verification_document: false,
                max_allowed_violations: 10,
                required_compliance_score: 50.0,
            },
            AsilLevel::A => DocumentationStandards {
                min_description_length: 100,
                requires_implementation_docs: true,
                requires_test_docs: false,
                requires_verification_document: false,
                max_allowed_violations: 5,
                required_compliance_score: 70.0,
            },
            AsilLevel::B => DocumentationStandards {
                min_description_length: 150,
                requires_implementation_docs: true,
                requires_test_docs: true,
                requires_verification_document: false,
                max_allowed_violations: 3,
                required_compliance_score: 80.0,
            },
            AsilLevel::C => DocumentationStandards {
                min_description_length: 200,
                requires_implementation_docs: true,
                requires_test_docs: true,
                requires_verification_document: true,
                max_allowed_violations: 1,
                required_compliance_score: 90.0,
            },
            AsilLevel::D => DocumentationStandards {
                min_description_length: 300,
                requires_implementation_docs: true,
                requires_test_docs: true,
                requires_verification_document: true,
                max_allowed_violations: 0,
                required_compliance_score: 95.0,
            },
        }
    }

    /// Verify that an implementation has proper documentation
    fn verify_implementation_documented(&self, implementation_ref: &str) -> bool {
        // Check if the implementation file exists and has documentation
        let impl_path = self.workspace_root.join(implementation_ref);

        if !impl_path.exists() {
            return false;
        }

        // In a real implementation, this would:
        // - Check for rustdoc comments
        // - Verify API documentation completeness
        // - Check for examples and usage documentation
        // - Validate cross-references to requirements

        // For now, check if file exists and has some basic content
        if let Ok(content) = std::fs::read_to_string(&impl_path) {
            // Look for documentation markers
            content.contains("///")
                || content.contains("//!")
                || content.contains("/*!")
                || content.contains("*/")
        } else {
            false
        }
    }

    /// Calculate compliance score for a requirement
    fn calculate_compliance_score(
        &self,
        violations: &[DocumentationViolation],
        _standards: &DocumentationStandards,
    ) -> f64 {
        if violations.is_empty() {
            return 100.0;
        }

        let total_penalty: f64 =
            violations.iter().map(|v| self.get_violation_penalty(&v.severity)).sum();
        let max_possible_penalty = 100.0; // Maximum penalty possible

        ((max_possible_penalty - total_penalty) / max_possible_penalty * 100.0).max(0.0)
    }

    /// Get penalty points for a violation severity
    fn get_violation_penalty(&self, severity: &DocumentationViolationSeverity) -> f64 {
        match severity {
            DocumentationViolationSeverity::Info => 5.0,
            DocumentationViolationSeverity::Low => 10.0,
            DocumentationViolationSeverity::Medium => 20.0,
            DocumentationViolationSeverity::High => 40.0,
            DocumentationViolationSeverity::Critical => 80.0,
        }
    }

    /// Get violation severity based on ASIL level and violation type
    fn get_violation_severity(
        &self,
        asil_level: AsilLevel,
        violation_type: DocumentationViolationType,
    ) -> DocumentationViolationSeverity {
        match (asil_level, violation_type) {
            (AsilLevel::D, DocumentationViolationType::MissingDescription) => {
                DocumentationViolationSeverity::Critical
            },
            (AsilLevel::D, _) => DocumentationViolationSeverity::High,
            (AsilLevel::C, DocumentationViolationType::MissingDescription) => {
                DocumentationViolationSeverity::High
            },
            (AsilLevel::C, _) => DocumentationViolationSeverity::Medium,
            (AsilLevel::B, _) => DocumentationViolationSeverity::Medium,
            (AsilLevel::A, _) => DocumentationViolationSeverity::Low,
            (AsilLevel::QM, _) => DocumentationViolationSeverity::Info,
        }
    }

    /// Check if system is ready for certification based on documentation
    fn is_certification_ready(&self, compliance_percentage: f64) -> bool {
        compliance_percentage >= self.verification_config.min_certification_compliance
    }

    /// Check if system is ready for ASIL-specific certification
    fn is_certification_ready_for_asil(
        &self,
        compliance_percentage: f64,
        asil_level: AsilLevel,
    ) -> bool {
        let required_threshold = match asil_level {
            AsilLevel::D => 95.0,
            AsilLevel::C => 90.0,
            AsilLevel::B => 85.0,
            AsilLevel::A => 80.0,
            AsilLevel::QM => 70.0,
        };

        compliance_percentage >= required_threshold
    }

    /// Calculate compliance per ASIL level
    fn calculate_asil_compliance(&self) -> HashMap<AsilLevel, f64> {
        let mut asil_compliance = HashMap::new();

        for asil_level in [
            AsilLevel::QM,
            AsilLevel::A,
            AsilLevel::B,
            AsilLevel::C,
            AsilLevel::D,
        ] {
            let asil_analyses: Vec<_> = self
                .documentation_analysis
                .iter()
                .filter(|a| a.asil_level == asil_level)
                .collect();

            if !asil_analyses.is_empty() {
                let compliance = asil_analyses.iter().map(|a| a.compliance_score).sum::<f64>()
                    / asil_analyses.len() as f64;
                asil_compliance.insert(asil_level, compliance);
            }
        }

        asil_compliance
    }

    /// Generate recommendations for improving documentation
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        let critical_violations = self
            .documentation_analysis
            .iter()
            .flat_map(|a| &a.violations)
            .filter(|v| v.severity == DocumentationViolationSeverity::Critical)
            .count();

        if critical_violations > 0 {
            recommendations.push(format!(
                "Address {} critical documentation violations immediately",
                critical_violations
            ));
        }

        let missing_descriptions = self
            .documentation_analysis
            .iter()
            .flat_map(|a| &a.violations)
            .filter(|v| v.violation_type == DocumentationViolationType::MissingDescription)
            .count();

        if missing_descriptions > 0 {
            recommendations.push(format!(
                "Add detailed descriptions for {} requirements",
                missing_descriptions
            ));
        }

        let undocumented_implementations = self
            .documentation_analysis
            .iter()
            .flat_map(|a| &a.violations)
            .filter(|v| v.violation_type == DocumentationViolationType::UndocumentedImplementation)
            .count();

        if undocumented_implementations > 0 {
            recommendations.push(format!(
                "Add documentation for {} implementations",
                undocumented_implementations
            ));
        }

        recommendations
    }
}

impl Default for DocumentationVerificationFramework {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

/// Configuration for documentation verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationVerificationConfig {
    /// Minimum compliance percentage required for certification.
    pub min_certification_compliance: f64,
    /// Enable validation of cross-references between documents.
    pub enable_cross_reference_validation: bool,
    /// Enable checking of API documentation completeness.
    pub enable_api_documentation_check: bool,
    /// Enable validation of code examples in documentation.
    pub enable_example_validation: bool,
}

impl Default for DocumentationVerificationConfig {
    fn default() -> Self {
        Self {
            min_certification_compliance: 85.0,
            enable_cross_reference_validation: true,
            enable_api_documentation_check: true,
            enable_example_validation: false,
        }
    }
}

/// Documentation standards for a specific ASIL level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationStandards {
    /// Minimum character length for requirement descriptions.
    pub min_description_length: usize,
    /// Whether implementation documentation is required.
    pub requires_implementation_docs: bool,
    /// Whether test documentation is required.
    pub requires_test_docs: bool,
    /// Whether verification documentation is required.
    pub requires_verification_document: bool,
    /// Maximum number of violations allowed.
    pub max_allowed_violations: usize,
    /// Required compliance score to pass.
    pub required_compliance_score: f64,
}

/// Result of documentation verification
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationVerificationResult {
    /// Total number of requirements analyzed.
    pub total_requirements: usize,
    /// Number of requirements with compliant documentation.
    pub compliant_requirements: usize,
    /// Overall compliance percentage.
    pub compliance_percentage: f64,
    /// List of documentation violations found.
    pub violations: Vec<DocumentationViolation>,
    /// Detailed analysis results for each requirement.
    pub analysis_results: Vec<DocumentationAnalysis>,
    /// Whether the documentation is ready for certification.
    pub is_certification_ready: bool,
}

/// Analysis of documentation for a single requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationAnalysis {
    /// Unique identifier of the requirement being analyzed.
    pub requirement_id: RequirementId,
    /// ASIL level of the requirement.
    pub asil_level: AsilLevel,
    /// List of documentation violations found.
    pub violations: Vec<DocumentationViolation>,
    /// Compliance score for this requirement.
    pub compliance_score: f64,
    /// Documentation standards required for this ASIL level.
    pub required_standards: DocumentationStandards,
    /// Locations where documentation was analyzed.
    pub analyzed_locations: Vec<DocumentationLocation>,
}

impl DocumentationAnalysis {
    /// Check if this requirement's documentation is compliant
    pub fn is_compliant(&self) -> bool {
        self.compliance_score >= self.required_standards.required_compliance_score
            && self.violations.len() <= self.required_standards.max_allowed_violations
    }
}

/// A documentation violation that needs to be addressed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationViolation {
    /// Unique identifier of the requirement with the violation.
    pub requirement_id: RequirementId,
    /// Type of documentation violation.
    pub violation_type: DocumentationViolationType,
    /// Severity level of the violation.
    pub severity: DocumentationViolationSeverity,
    /// Human-readable description of the violation.
    pub description: String,
    /// Location where the violation was found.
    pub location: DocumentationLocation,
}

/// Types of documentation violations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentationViolationType {
    /// Requirement lacks a description.
    MissingDescription,
    /// Description lacks sufficient detail.
    InsufficientDetail,
    /// Implementation code is missing.
    MissingImplementation,
    /// Implementation exists but is not documented.
    UndocumentedImplementation,
    /// Test documentation is missing.
    MissingTestDocumentation,
    /// Verification document is missing.
    MissingVerificationDocument,
    /// Cross-references between documents are inconsistent.
    InconsistentCrossReferences,
    /// Code examples are missing.
    MissingExamples,
    /// Documentation is outdated.
    OutdatedDocumentation,
}

impl fmt::Display for DocumentationViolationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentationViolationType::MissingDescription => write!(f, "Missing Description"),
            DocumentationViolationType::InsufficientDetail => write!(f, "Insufficient Detail"),
            DocumentationViolationType::MissingImplementation => {
                write!(f, "Missing Implementation")
            },
            DocumentationViolationType::UndocumentedImplementation => {
                write!(f, "Undocumented Implementation")
            },
            DocumentationViolationType::MissingTestDocumentation => {
                write!(f, "Missing Test Documentation")
            },
            DocumentationViolationType::MissingVerificationDocument => {
                write!(f, "Missing Verification Document")
            },
            DocumentationViolationType::InconsistentCrossReferences => {
                write!(f, "Inconsistent Cross References")
            },
            DocumentationViolationType::MissingExamples => write!(f, "Missing Examples"),
            DocumentationViolationType::OutdatedDocumentation => {
                write!(f, "Outdated Documentation")
            },
        }
    }
}

/// Severity levels for documentation violations
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DocumentationViolationSeverity {
    /// Informational severity.
    Info,
    /// Low severity violation.
    Low,
    /// Medium severity violation.
    Medium,
    /// High severity violation.
    High,
    /// Critical severity violation requiring immediate attention.
    Critical,
}

impl fmt::Display for DocumentationViolationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentationViolationSeverity::Info => write!(f, "Info"),
            DocumentationViolationSeverity::Low => write!(f, "Low"),
            DocumentationViolationSeverity::Medium => write!(f, "Medium"),
            DocumentationViolationSeverity::High => write!(f, "High"),
            DocumentationViolationSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Location where documentation issue was found
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentationLocation {
    /// Issue found in requirement documentation.
    Requirement,
    /// Issue found in implementation documentation.
    Implementation,
    /// Issue found in test documentation.
    Test,
    /// Issue found in verification documentation.
    Verification,
    /// Issue found in API documentation.
    Api,
    /// Issue found in example documentation.
    Example,
}

impl fmt::Display for DocumentationLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentationLocation::Requirement => write!(f, "Requirement"),
            DocumentationLocation::Implementation => write!(f, "Implementation"),
            DocumentationLocation::Test => write!(f, "Test"),
            DocumentationLocation::Verification => write!(f, "Verification"),
            DocumentationLocation::Api => write!(f, "API"),
            DocumentationLocation::Example => write!(f, "Example"),
        }
    }
}

/// Comprehensive documentation report
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationReport {
    /// Overall compliance percentage across all requirements.
    pub overall_compliance: f64,
    /// Total number of requirements analyzed.
    pub total_requirements: usize,
    /// Total number of violations found.
    pub total_violations: usize,
    /// Number of critical violations found.
    pub critical_violations: usize,
    /// Compliance percentage by ASIL level.
    pub asil_compliance: HashMap<AsilLevel, f64>,
    /// List of recommendations for improving documentation.
    pub recommendations: Vec<String>,
    /// Summary of analysis for each requirement.
    pub analysis_summary: Vec<DocumentationAnalysis>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_documentation_verification_framework_creation() {
        let mut framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));
        let (result, _diagnostics) = framework.verify_all_documentation().unwrap();

        assert_eq!(result.total_requirements, 0);
        assert_eq!(result.compliance_percentage, 100.0);
        assert!(result.is_certification_ready);
    }

    #[test]
    fn test_requirement_documentation_analysis() {
        let mut framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));

        let requirement = SafetyRequirement::new(
            RequirementId::new("DOC_TEST_001"),
            "Test Requirement".to_string(),
            "A".to_string(), // Very short description - should trigger violation
            RequirementType::Safety,
            AsilLevel::C,
        );

        framework.add_requirement(requirement);

        let (result, _diagnostics) = framework.verify_all_documentation().unwrap();

        assert_eq!(result.total_requirements, 1);
        assert_eq!(result.compliant_requirements, 0);
        assert!(!result.violations.is_empty());
        assert!(!result.is_certification_ready);
    }

    #[test]
    fn test_asil_specific_documentation_standards() {
        let framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));

        let qm_standards = framework.get_documentation_standards_for_asil(AsilLevel::QM);
        let asil_d_standards = framework.get_documentation_standards_for_asil(AsilLevel::D);

        assert!(asil_d_standards.min_description_length > qm_standards.min_description_length);
        assert!(asil_d_standards.requires_verification_document);
        assert!(!qm_standards.requires_verification_document);
        assert!(
            asil_d_standards.required_compliance_score > qm_standards.required_compliance_score
        );
    }

    #[test]
    fn test_compliant_requirement_documentation() {
        let mut framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));

        let mut requirement = SafetyRequirement::new(
            RequirementId::new("DOC_TEST_002"),
            "Well Documented Requirement".to_string(),
            "This is a comprehensive description of a safety requirement that provides detailed \
             information about the expected behavior, constraints, and verification criteria for \
             the implementation."
                .to_string(),
            RequirementType::Safety,
            AsilLevel::A,
        );

        requirement.add_implementation("well_documented_impl.rs".to_string());
        requirement.add_test("comprehensive_test.rs".to_string());

        framework.add_requirement(requirement);

        let (result, _diagnostics) = framework.verify_all_documentation().unwrap();

        assert_eq!(result.total_requirements, 1);
        assert_eq!(result.compliant_requirements, 1);
        assert_eq!(result.compliance_percentage, 100.0);
        assert!(result.is_certification_ready);
    }

    #[test]
    fn test_documentation_report_generation() {
        let mut framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));
        let (report, _diagnostics) = framework.generate_report();

        assert_eq!(report.overall_compliance, 100.0);
        assert_eq!(report.total_requirements, 0);
        assert_eq!(report.total_violations, 0);
        assert_eq!(report.critical_violations, 0);
    }

    #[test]
    fn test_violation_severity_mapping() {
        let framework = DocumentationVerificationFramework::new(PathBuf::from("/tmp"));

        let asil_d_missing_desc = framework
            .get_violation_severity(AsilLevel::D, DocumentationViolationType::MissingDescription);

        let qm_missing_desc = framework.get_violation_severity(
            AsilLevel::QM,
            DocumentationViolationType::MissingDescription,
        );

        assert_eq!(
            asil_d_missing_desc,
            DocumentationViolationSeverity::Critical
        );
        assert_eq!(qm_missing_desc, DocumentationViolationSeverity::Info);
        assert!(asil_d_missing_desc > qm_missing_desc);
    }
}
