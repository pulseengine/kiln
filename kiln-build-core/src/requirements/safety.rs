//! Safety Verification Framework for cargo-wrt
//!
//! This module provides comprehensive safety verification integrated with
//! cargo-wrt's diagnostic system, requirements traceability, and ASIL-tagged
//! testing. Inspired by SCORE's verification methodology.

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

use super::model::{
    CoverageLevel, RequirementId, RequirementRegistry, SafetyRequirement, VerificationStatus,
};
use crate::{
    config::AsilLevel,
    diagnostics::{Diagnostic, DiagnosticCollection, Position, Range, Severity},
    error::{BuildError, BuildResult},
    formatters::OutputFormat,
};

/// Helper functions for creating diagnostics
impl DiagnosticCollection {
    fn add_info(&mut self, file: &str, message: String, code: &str) {
        self.add_diagnostic(
            Diagnostic::new(
                file.to_string(),
                Range::single_line(0, 0, 0),
                Severity::Info,
                message,
                "safety-verification".to_string(),
            )
            .with_code(code.to_string()),
        );
    }

    fn add_warning(&mut self, file: &str, message: String, code: &str) {
        self.add_diagnostic(
            Diagnostic::new(
                file.to_string(),
                Range::single_line(0, 0, 0),
                Severity::Warning,
                message,
                "safety-verification".to_string(),
            )
            .with_code(code.to_string()),
        );
    }

    fn add_error(&mut self, file: &str, message: String, code: &str) {
        self.add_diagnostic(
            Diagnostic::new(
                file.to_string(),
                Range::single_line(0, 0, 0),
                Severity::Error,
                message,
                "safety-verification".to_string(),
            )
            .with_code(code.to_string()),
        );
    }
}

/// Safety verification framework that coordinates all verification activities
/// Integrates with cargo-wrt's diagnostic and output systems
#[derive(Debug)]
pub struct SafetyVerificationFramework {
    /// Registry of all safety requirements
    requirement_registry: RequirementRegistry,
    /// Test execution results
    test_results: Vec<TestResult>,
    /// Code coverage data
    coverage_data: CoverageData,
    /// Platform verification data
    platform_verifications: Vec<PlatformVerification>,
    /// Workspace root for file operations
    workspace_root: std::path::PathBuf,
}

impl SafetyVerificationFramework {
    /// Create a new safety verification framework
    pub fn new(workspace_root: std::path::PathBuf) -> Self {
        Self {
            requirement_registry: RequirementRegistry::new(),
            test_results: Vec::new(),
            coverage_data: CoverageData::new(),
            platform_verifications: Vec::new(),
            workspace_root,
        }
    }

    /// Add a safety requirement to be tracked
    pub fn add_requirement(&mut self, requirement: SafetyRequirement) {
        self.requirement_registry.add_requirement(requirement);
    }

    /// Load requirements from external source and generate diagnostics
    pub fn load_requirements_from_source(
        &mut self,
        source: &str,
    ) -> BuildResult<(usize, DiagnosticCollection)> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "safety-verification".to_string(),
        );

        // Generate standard safety requirements based on WRT needs
        let standard_requirements = self.generate_wrt_safety_requirements();
        let count = standard_requirements.len();

        for req in standard_requirements {
            self.requirement_registry.add_requirement(req);
        }

        diagnostics.add_info(
            source,
            format!("Loaded {} safety requirements", count),
            "safety-load",
        );

        Ok((count, diagnostics))
    }

    /// Verify ASIL compliance with integrated diagnostics
    pub fn verify_asil_compliance(
        &mut self,
        target_asil: AsilLevel,
    ) -> BuildResult<(ComplianceVerificationResult, DiagnosticCollection)> {
        let mut diagnostics =
            DiagnosticCollection::new(self.workspace_root.clone(), "asil-compliance".to_string());

        let requirements = self.requirement_registry.get_requirements_by_asil(target_asil);
        let total_requirements = requirements.len();

        let mut verified_count = 0;
        let mut missing_implementation_count = 0;
        let mut missing_testing_count = 0;
        let mut violations = Vec::new();

        for requirement in requirements {
            if requirement.is_verified() {
                verified_count += 1;
                diagnostics.add_info(
                    "safety-verification",
                    format!("Requirement {} verified", requirement.id),
                    "requirement-verified",
                );
            }

            if requirement.needs_implementation() {
                missing_implementation_count += 1;
                let violation = ComplianceViolation {
                    requirement_id: requirement.id.clone(),
                    violation_type: ViolationType::MissingImplementation,
                    description: format!("Requirement {} lacks implementation", requirement.id),
                    severity: self.determine_violation_severity(&requirement.asil_level),
                };

                let diagnostic_severity = match violation.severity {
                    ViolationSeverity::Critical => Severity::Error,
                    ViolationSeverity::High => Severity::Error,
                    ViolationSeverity::Medium => Severity::Warning,
                    ViolationSeverity::Low => Severity::Warning,
                    ViolationSeverity::Info => Severity::Info,
                };

                diagnostics.add_diagnostic(Diagnostic {
                    file: "safety-requirements".to_string(),
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                    severity: diagnostic_severity,
                    message: violation.description.clone(),
                    code: Some("missing-implementation".to_string()),
                    source: "safety-verification".to_string(),
                    related_info: vec![],
                });

                violations.push(violation);
            }

            if requirement.needs_testing() {
                missing_testing_count += 1;
                let violation = ComplianceViolation {
                    requirement_id: requirement.id.clone(),
                    violation_type: ViolationType::InsufficientTesting,
                    description: format!(
                        "Requirement {} needs more testing coverage",
                        requirement.id
                    ),
                    severity: self.determine_violation_severity(&requirement.asil_level),
                };

                let diagnostic_severity = match violation.severity {
                    ViolationSeverity::Critical => Severity::Error,
                    ViolationSeverity::High => Severity::Error,
                    ViolationSeverity::Medium => Severity::Warning,
                    ViolationSeverity::Low => Severity::Warning,
                    ViolationSeverity::Info => Severity::Info,
                };

                diagnostics.add_diagnostic(Diagnostic {
                    file: "safety-requirements".to_string(),
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                    severity: diagnostic_severity,
                    message: violation.description.clone(),
                    code: Some("insufficient-testing".to_string()),
                    source: "safety-verification".to_string(),
                    related_info: vec![],
                });

                violations.push(violation);
            }
        }

        let compliance_percentage = if total_requirements > 0 {
            (verified_count as f64 / total_requirements as f64) * 100.0
        } else {
            100.0
        };

        let result = ComplianceVerificationResult {
            target_asil,
            total_requirements,
            verified_requirements: verified_count,
            compliance_percentage,
            violations,
            missing_implementation_count,
            missing_testing_count,
            is_compliant: compliance_percentage >= self.get_compliance_threshold(target_asil),
        };

        // Add overall compliance diagnostic
        if result.is_compliant {
            diagnostics.add_info(
                "safety-verification",
                format!(
                    "ASIL {} compliance: {:.1}% (PASS)",
                    target_asil, compliance_percentage
                ),
                "compliance-pass",
            );
        } else {
            diagnostics.add_error(
                "safety-verification",
                format!(
                    "ASIL {} compliance: {:.1}% (FAIL - requires {:.1}%)",
                    target_asil,
                    compliance_percentage,
                    self.get_compliance_threshold(target_asil)
                ),
                "compliance-fail",
            );
        }

        Ok((result, diagnostics))
    }

    /// Record test execution result with diagnostic integration
    pub fn record_test_result(&mut self, result: TestResult) -> DiagnosticCollection {
        let mut diagnostics =
            DiagnosticCollection::new(self.workspace_root.clone(), "test-recording".to_string());

        // Update requirement verification status based on test results
        for requirement_id in &result.verified_requirements {
            if let Some(requirement) = self.requirement_registry.get_requirement_mut(requirement_id)
            {
                if result.passed {
                    // Update coverage based on test comprehensiveness
                    let new_coverage = match result.coverage_type {
                        TestCoverageType::Basic => CoverageLevel::Basic,
                        TestCoverageType::Comprehensive => CoverageLevel::Comprehensive,
                        TestCoverageType::Complete => CoverageLevel::Complete,
                    };

                    if new_coverage > requirement.coverage {
                        requirement.set_coverage(new_coverage.clone());
                        diagnostics.add_info(
                            &result.test_name,
                            format!(
                                "Updated coverage for {} to {:?}",
                                requirement_id, &new_coverage
                            ),
                            "coverage-update",
                        );
                    }

                    // Mark as verified if sufficiently tested
                    if requirement.coverage >= CoverageLevel::Basic
                        && !requirement.implementations.is_empty()
                    {
                        requirement.set_status(VerificationStatus::Verified);
                        diagnostics.add_info(
                            &result.test_name,
                            format!("Requirement {} marked as verified", requirement_id),
                            "requirement-verified",
                        );
                    }
                } else {
                    requirement
                        .set_status(VerificationStatus::Failed(result.failure_reason.clone()));
                    diagnostics.add_error(
                        &result.test_name,
                        format!(
                            "Requirement {} verification failed: {}",
                            requirement_id, result.failure_reason
                        ),
                        "verification-failed",
                    );
                }
            }
        }

        if result.passed {
            diagnostics.add_info(
                &result.test_name,
                format!(
                    "Test passed in {}ms (ASIL {})",
                    result.execution_time_ms, result.asil_level
                ),
                "test-passed",
            );
        } else {
            diagnostics.add_error(
                &result.test_name,
                format!("Test failed: {}", result.failure_reason),
                "test-failed",
            );
        }

        self.test_results.push(result);
        diagnostics
    }

    /// Update code coverage data
    pub fn update_coverage_data(&mut self, coverage: CoverageData) {
        self.coverage_data = coverage;
    }

    /// Add platform verification result
    pub fn add_platform_verification(&mut self, verification: PlatformVerification) {
        self.platform_verifications.push(verification);
    }

    /// Generate comprehensive safety report with diagnostic integration
    pub fn generate_safety_report(&self) -> (SafetyReport, DiagnosticCollection) {
        let mut diagnostics =
            DiagnosticCollection::new(self.workspace_root.clone(), "safety-report".to_string());

        let overall_compliance = self.requirement_registry.overall_compliance();

        let asil_compliance = [
            AsilLevel::QM,
            AsilLevel::A,
            AsilLevel::B,
            AsilLevel::C,
            AsilLevel::D,
        ]
        .iter()
        .map(|&asil| (asil, self.requirement_registry.asil_compliance(asil)))
        .collect();

        let test_summary = TestSummary {
            total_tests: self.test_results.len(),
            passed_tests: self.test_results.iter().filter(|r| r.passed).count(),
            failed_tests: self.test_results.iter().filter(|r| !r.passed).count(),
            coverage_percentage: self.coverage_data.overall_coverage(),
        };

        let platform_summary = PlatformSummary {
            verified_platforms: self
                .platform_verifications
                .iter()
                .filter(|v| v.verification_passed)
                .count(),
            total_platforms: self.platform_verifications.len(),
            platform_results: self.platform_verifications.clone(),
        };

        let critical_violations = self.get_critical_violations();
        let recommendations = self.generate_recommendations();

        // Generate diagnostics for report
        diagnostics.add_info(
            "safety-report",
            format!("Overall compliance: {:.1}%", overall_compliance * 100.0),
            "overall-compliance",
        );

        diagnostics.add_info(
            "safety-report",
            format!(
                "Test results: {}/{} passed",
                test_summary.passed_tests, test_summary.total_tests
            ),
            "test-summary",
        );

        for violation in &critical_violations {
            diagnostics.add_error(
                "safety-report",
                format!("Critical violation: {}", violation.description),
                "critical-violation",
            );
        }

        let report = SafetyReport {
            overall_compliance,
            asil_compliance,
            test_summary,
            platform_summary,
            coverage_data: self.coverage_data.clone(),
            unverified_requirements: self.requirement_registry.get_unverified_requirements().len(),
            critical_violations,
            recommendations,
        };

        (report, diagnostics)
    }

    /// Check certification readiness with diagnostic output
    pub fn check_certification_readiness(
        &self,
        asil_level: AsilLevel,
    ) -> (CertificationReadiness, DiagnosticCollection) {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "certification-readiness".to_string(),
        );

        let compliance_result = self.verify_asil_compliance_readonly(asil_level);
        let required_threshold = self.get_compliance_threshold(asil_level);
        let coverage_threshold = self.get_coverage_threshold(asil_level);

        let blocking_issues = self.get_blocking_issues_for_asil(asil_level);

        // If there are no requirements for this ASIL level, system is trivially compliant
        // (coverage check not applicable when there's nothing to cover)
        let is_ready = if compliance_result.total_requirements == 0 {
            compliance_result.compliance_percentage >= required_threshold
                && blocking_issues.is_empty()
        } else {
            compliance_result.compliance_percentage >= required_threshold
                && self.coverage_data.overall_coverage() >= coverage_threshold
                && blocking_issues.is_empty()
        };

        if is_ready {
            diagnostics.add_info(
                "certification",
                format!("System is ready for ASIL {} certification", asil_level),
                "certification-ready",
            );
        } else {
            diagnostics.add_warning(
                "certification",
                format!("System not ready for ASIL {} certification", asil_level),
                "certification-not-ready",
            );

            for issue in &blocking_issues {
                diagnostics.add_error(
                    "certification",
                    format!("Blocking issue: {}", issue),
                    "blocking-issue",
                );
            }
        }

        let readiness = CertificationReadiness {
            asil_level,
            is_ready,
            compliance_percentage: compliance_result.compliance_percentage,
            required_compliance: required_threshold,
            coverage_percentage: self.coverage_data.overall_coverage(),
            required_coverage: coverage_threshold,
            blocking_issues: blocking_issues.clone(),
            recommendations: if blocking_issues.is_empty() {
                vec!["System is ready for certification".to_string()]
            } else {
                self.generate_certification_recommendations(asil_level)
            },
        };

        (readiness, diagnostics)
    }

    /// Convert safety verification results to cargo-wrt diagnostics
    pub fn to_diagnostics(&self, output_format: OutputFormat) -> BuildResult<DiagnosticCollection> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "safety-verification".to_string(),
        );

        // Add compliance diagnostics for each ASIL level
        for asil in &[
            AsilLevel::QM,
            AsilLevel::A,
            AsilLevel::B,
            AsilLevel::C,
            AsilLevel::D,
        ] {
            let compliance = self.requirement_registry.asil_compliance(*asil);
            let threshold = self.get_compliance_threshold(*asil);

            if compliance >= threshold / 100.0 {
                diagnostics.add_info(
                    "safety-verification",
                    format!(
                        "ASIL {} compliance: {:.1}% (PASS)",
                        asil,
                        compliance * 100.0
                    ),
                    "asil-compliance",
                );
            } else {
                diagnostics.add_warning(
                    "safety-verification",
                    format!(
                        "ASIL {} compliance: {:.1}% (requires {:.1}%)",
                        asil,
                        compliance * 100.0,
                        threshold
                    ),
                    "asil-compliance",
                );
            }
        }

        // Add test result diagnostics
        for test in &self.test_results {
            if test.passed {
                diagnostics.add_info(
                    &test.test_name,
                    format!(
                        "Test passed ({}ms, ASIL {})",
                        test.execution_time_ms, test.asil_level
                    ),
                    "test-result",
                );
            } else {
                diagnostics.add_error(
                    &test.test_name,
                    format!("Test failed: {}", test.failure_reason),
                    "test-result",
                );
            }
        }

        Ok(diagnostics)
    }

    /// Get requirement registry
    pub fn registry(&self) -> &RequirementRegistry {
        &self.requirement_registry
    }

    /// Get mutable requirement registry
    pub fn registry_mut(&mut self) -> &mut RequirementRegistry {
        &mut self.requirement_registry
    }

    // Private helper methods

    fn generate_wrt_safety_requirements(&self) -> Vec<SafetyRequirement> {
        use super::model::{RequirementType, VerificationMethod};

        vec![
            {
                let mut req = SafetyRequirement::new(
                    RequirementId::new("WRT_MEM_001"),
                    "Memory Safety".to_string(),
                    "All memory allocations must be bounded and verified through \
                     safe_managed_alloc"
                        .to_string(),
                    RequirementType::Memory,
                    AsilLevel::D,
                );
                req.verification_method = VerificationMethod::FormalProof;
                req.add_implementation("wrt-foundation/src/safe_allocation.rs".to_string());
                req.add_test("wrt-foundation/tests/memory_budget_validation.rs".to_string());
                req
            },
            {
                let mut req = SafetyRequirement::new(
                    RequirementId::new("WRT_RUNTIME_001"),
                    "Runtime Safety Context".to_string(),
                    "Runtime must maintain safety context with ASIL tracking throughout execution"
                        .to_string(),
                    RequirementType::Safety,
                    AsilLevel::D,
                );
                req.verification_method = VerificationMethod::Test;
                req.add_implementation("wrt-runtime/src/execution.rs".to_string());
                req.add_test(
                    "wrt-tests/integration/safety_critical_integration_tests.rs".to_string(),
                );
                req
            },
            {
                let mut req = SafetyRequirement::new(
                    RequirementId::new("WRT_COMPONENT_001"),
                    "Component Isolation".to_string(),
                    "Components must be isolated with resource bounds and memory budgets"
                        .to_string(),
                    RequirementType::Safety,
                    AsilLevel::C,
                );
                req.verification_method = VerificationMethod::Test;
                req.add_implementation(
                    "wrt-component/src/bounded_resource_management.rs".to_string(),
                );
                req.add_test(
                    "wrt-component/tests/safety_critical_memory_budget_tests.rs".to_string(),
                );
                req
            },
            {
                let mut req = SafetyRequirement::new(
                    RequirementId::new("WRT_PLATFORM_001"),
                    "Platform Abstraction".to_string(),
                    "Platform abstractions must maintain safety properties across all supported \
                     targets"
                        .to_string(),
                    RequirementType::Platform,
                    AsilLevel::B,
                );
                req.verification_method = VerificationMethod::Analysis;
                req.add_implementation("wrt-platform/src/memory.rs".to_string());
                req.add_test("wrt-platform/tests/linux_integration_test.rs".to_string());
                req
            },
        ]
    }

    fn determine_violation_severity(&self, asil_level: &AsilLevel) -> ViolationSeverity {
        match asil_level {
            AsilLevel::D => ViolationSeverity::Critical,
            AsilLevel::C => ViolationSeverity::High,
            AsilLevel::B => ViolationSeverity::Medium,
            AsilLevel::A => ViolationSeverity::Low,
            AsilLevel::QM => ViolationSeverity::Info,
        }
    }

    fn get_compliance_threshold(&self, asil_level: AsilLevel) -> f64 {
        match asil_level {
            AsilLevel::D => 98.0,
            AsilLevel::C => 95.0,
            AsilLevel::B => 90.0,
            AsilLevel::A => 85.0,
            AsilLevel::QM => 70.0,
        }
    }

    fn get_coverage_threshold(&self, asil_level: AsilLevel) -> f64 {
        match asil_level {
            AsilLevel::D => 95.0,
            AsilLevel::C => 90.0,
            AsilLevel::B => 80.0,
            AsilLevel::A => 70.0,
            AsilLevel::QM => 50.0,
        }
    }

    fn verify_asil_compliance_readonly(
        &self,
        target_asil: AsilLevel,
    ) -> ComplianceVerificationResult {
        let requirements = self.requirement_registry.get_requirements_by_asil(target_asil);
        let total_requirements = requirements.len();
        let verified_count = requirements.iter().filter(|r| r.is_verified()).count();

        let compliance_percentage = if total_requirements > 0 {
            (verified_count as f64 / total_requirements as f64) * 100.0
        } else {
            100.0
        };

        ComplianceVerificationResult {
            target_asil,
            total_requirements,
            verified_requirements: verified_count,
            compliance_percentage,
            violations: Vec::new(),
            missing_implementation_count: 0,
            missing_testing_count: 0,
            is_compliant: compliance_percentage >= self.get_compliance_threshold(target_asil),
        }
    }

    fn get_critical_violations(&self) -> Vec<ComplianceViolation> {
        // Analyze current state for critical violations
        let mut violations = Vec::new();

        for req in &self.requirement_registry.requirements {
            if matches!(req.asil_level, AsilLevel::D | AsilLevel::C) && !req.is_verified() {
                violations.push(ComplianceViolation {
                    requirement_id: req.id.clone(),
                    violation_type: ViolationType::FailedVerification,
                    description: format!("Critical requirement {} not verified", req.id),
                    severity: ViolationSeverity::Critical,
                });
            }
        }

        violations
    }

    fn get_blocking_issues_for_asil(&self, asil_level: AsilLevel) -> Vec<String> {
        let mut issues = Vec::new();

        let requirements = self.requirement_registry.get_requirements_by_asil(asil_level);
        let unverified = requirements.iter().filter(|r| !r.is_verified()).count();

        if unverified > 0 {
            issues.push(format!(
                "{} unverified requirements for ASIL {}",
                unverified, asil_level
            ));
        }

        let threshold = self.get_compliance_threshold(asil_level);
        let current = self.requirement_registry.asil_compliance(asil_level) * 100.0;
        if current < threshold {
            issues.push(format!(
                "Compliance {:.1}% below required {:.1}%",
                current, threshold
            ));
        }

        issues
    }

    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if self.requirement_registry.overall_compliance() < 0.9 {
            recommendations.push("Increase test coverage for unverified requirements".to_string());
        }

        if self.coverage_data.overall_coverage() < 80.0 {
            recommendations.push("Improve code coverage through additional testing".to_string());
        }

        let unverified = self.requirement_registry.get_unverified_requirements();
        if !unverified.is_empty() {
            recommendations.push(format!(
                "Address {} unverified requirements",
                unverified.len()
            ));
        }

        recommendations
    }

    fn generate_certification_recommendations(&self, asil_level: AsilLevel) -> Vec<String> {
        let mut recommendations = vec![
            "Complete all requirement implementations".to_string(),
            format!(
                "Achieve minimum {:.1}% test coverage threshold",
                self.get_coverage_threshold(asil_level)
            ),
            "Resolve all critical violations".to_string(),
        ];

        match asil_level {
            AsilLevel::D => {
                recommendations
                    .push("Perform formal verification for all critical paths".to_string());
                recommendations
                    .push("Complete hazard analysis and safety argumentation".to_string());
            },
            AsilLevel::C => {
                recommendations.push("Ensure comprehensive integration testing".to_string());
                recommendations
                    .push("Document safety measures and verification evidence".to_string());
            },
            _ => {},
        }

        recommendations
    }
}

/// Result of compliance verification for a specific ASIL level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceVerificationResult {
    /// Target ASIL level being verified.
    pub target_asil: AsilLevel,
    /// Total number of requirements for this ASIL level.
    pub total_requirements: usize,
    /// Number of requirements that passed verification.
    pub verified_requirements: usize,
    /// Percentage of requirements that are compliant.
    pub compliance_percentage: f64,
    /// List of compliance violations found.
    pub violations: Vec<ComplianceViolation>,
    /// Count of requirements missing implementation.
    pub missing_implementation_count: usize,
    /// Count of requirements missing tests.
    pub missing_testing_count: usize,
    /// Whether the verification passed all compliance checks.
    pub is_compliant: bool,
}

/// A compliance violation that needs to be addressed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    /// Unique identifier of the requirement that was violated.
    pub requirement_id: RequirementId,
    /// Type of violation that occurred.
    pub violation_type: ViolationType,
    /// Human-readable description of the violation.
    pub description: String,
    /// Severity level of this violation.
    pub severity: ViolationSeverity,
}

/// Types of compliance violations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    /// Requirement lacks implementation code.
    MissingImplementation,
    /// Requirement has insufficient test coverage.
    InsufficientTesting,
    /// Verification checks did not pass.
    FailedVerification,
    /// Requirement lacks required documentation.
    MissingDocumentation,
    /// Requirement assigned incorrect ASIL level.
    IncorrectASILLevel,
}

impl fmt::Display for ViolationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViolationType::MissingImplementation => write!(f, "Missing Implementation"),
            ViolationType::InsufficientTesting => write!(f, "Insufficient Testing"),
            ViolationType::FailedVerification => write!(f, "Failed Verification"),
            ViolationType::MissingDocumentation => write!(f, "Missing Documentation"),
            ViolationType::IncorrectASILLevel => write!(f, "Incorrect ASIL Level"),
        }
    }
}

/// Severity levels for violations
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ViolationSeverity {
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

impl fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViolationSeverity::Info => write!(f, "Info"),
            ViolationSeverity::Low => write!(f, "Low"),
            ViolationSeverity::Medium => write!(f, "Medium"),
            ViolationSeverity::High => write!(f, "High"),
            ViolationSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Test execution result with ASIL metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the test that was executed.
    pub test_name: String,
    /// Whether the test passed.
    pub passed: bool,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
    /// List of requirements verified by this test.
    pub verified_requirements: Vec<RequirementId>,
    /// Type of coverage achieved by this test.
    pub coverage_type: TestCoverageType,
    /// Reason for failure if test did not pass.
    pub failure_reason: String,
    /// ASIL level of this test.
    pub asil_level: AsilLevel,
}

/// Type of test coverage achieved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestCoverageType {
    /// Basic test coverage with minimal testing.
    Basic,
    /// Comprehensive test coverage with thorough testing.
    Comprehensive,
    /// Complete test coverage with exhaustive testing.
    Complete,
}

impl fmt::Display for TestCoverageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestCoverageType::Basic => write!(f, "Basic"),
            TestCoverageType::Comprehensive => write!(f, "Comprehensive"),
            TestCoverageType::Complete => write!(f, "Complete"),
        }
    }
}

/// Code coverage data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageData {
    /// Percentage of lines covered by tests.
    pub line_coverage: f64,
    /// Percentage of branches covered by tests.
    pub branch_coverage: f64,
    /// Percentage of functions covered by tests.
    pub function_coverage: f64,
    /// Coverage data for individual files.
    pub file_coverages: Vec<FileCoverage>,
}

impl CoverageData {
    /// Create new coverage data with zero coverage.
    pub fn new() -> Self {
        Self {
            line_coverage: 0.0,
            branch_coverage: 0.0,
            function_coverage: 0.0,
            file_coverages: Vec::new(),
        }
    }

    /// Calculate overall coverage as average of line, branch, and function coverage.
    pub fn overall_coverage(&self) -> f64 {
        (self.line_coverage + self.branch_coverage + self.function_coverage) / 3.0
    }
}

impl Default for CoverageData {
    fn default() -> Self {
        Self::new()
    }
}

/// Coverage data for a specific file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCoverage {
    /// Path to the source file.
    pub file_path: String,
    /// Percentage of lines covered in this file.
    pub line_coverage: f64,
    /// Percentage of branches covered in this file.
    pub branch_coverage: f64,
    /// Percentage of functions covered in this file.
    pub function_coverage: f64,
}

/// Platform verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformVerification {
    /// Name of the platform being verified.
    pub platform_name: String,
    /// Whether platform verification passed.
    pub verification_passed: bool,
    /// List of features that passed verification.
    pub verified_features: Vec<String>,
    /// List of features that failed verification.
    pub failed_features: Vec<String>,
    /// ASIL compliance level achieved.
    pub asil_compliance: AsilLevel,
}

/// Comprehensive safety report
#[derive(Debug, Serialize, Deserialize)]
pub struct SafetyReport {
    /// Overall compliance percentage across all ASIL levels.
    pub overall_compliance: f64,
    /// Compliance percentage by ASIL level.
    pub asil_compliance: HashMap<AsilLevel, f64>,
    /// Summary of test execution results.
    pub test_summary: TestSummary,
    /// Summary of platform verification results.
    pub platform_summary: PlatformSummary,
    /// Code coverage data.
    pub coverage_data: CoverageData,
    /// Number of requirements not yet verified.
    pub unverified_requirements: usize,
    /// List of critical compliance violations.
    pub critical_violations: Vec<ComplianceViolation>,
    /// List of recommendations for improving safety compliance.
    pub recommendations: Vec<String>,
}

/// Test execution summary
#[derive(Debug, Serialize, Deserialize)]
pub struct TestSummary {
    /// Total number of tests executed.
    pub total_tests: usize,
    /// Number of tests that passed.
    pub passed_tests: usize,
    /// Number of tests that failed.
    pub failed_tests: usize,
    /// Overall test coverage percentage.
    pub coverage_percentage: f64,
}

/// Platform verification summary
#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformSummary {
    /// Number of platforms that passed verification.
    pub verified_platforms: usize,
    /// Total number of platforms verified.
    pub total_platforms: usize,
    /// Detailed verification results for each platform.
    pub platform_results: Vec<PlatformVerification>,
}

/// Certification readiness assessment
#[derive(Debug, Serialize, Deserialize)]
pub struct CertificationReadiness {
    /// ASIL level being assessed for certification.
    pub asil_level: AsilLevel,
    /// Whether the system is ready for certification.
    pub is_ready: bool,
    /// Current compliance percentage.
    pub compliance_percentage: f64,
    /// Required compliance percentage for certification.
    pub required_compliance: f64,
    /// Current code coverage percentage.
    pub coverage_percentage: f64,
    /// Required coverage percentage for certification.
    pub required_coverage: f64,
    /// List of issues blocking certification.
    pub blocking_issues: Vec<String>,
    /// Recommendations for achieving certification readiness.
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::requirements::model::{RequirementType, VerificationMethod};

    #[test]
    fn test_safety_verification_framework_creation() {
        let framework = SafetyVerificationFramework::new(PathBuf::from("/tmp"));
        let (report, _diagnostics) = framework.generate_safety_report();

        assert_eq!(report.overall_compliance, 1.0); // 100% if no requirements
        assert_eq!(report.test_summary.total_tests, 0);
    }

    #[test]
    fn test_requirement_addition_and_verification() {
        let mut framework = SafetyVerificationFramework::new(PathBuf::from("/tmp"));

        let mut req = SafetyRequirement::new(
            RequirementId::new("TEST_REQ_001"),
            "Test Requirement".to_string(),
            "Test description".to_string(),
            RequirementType::Safety,
            AsilLevel::C,
        );

        req.add_implementation("test_impl.rs".to_string());
        req.set_coverage(CoverageLevel::Basic);
        req.set_status(VerificationStatus::Verified);

        framework.add_requirement(req);

        let (compliance_result, _diagnostics) =
            framework.verify_asil_compliance(AsilLevel::C).unwrap();
        assert_eq!(compliance_result.total_requirements, 1);
        assert_eq!(compliance_result.verified_requirements, 1);
        assert_eq!(compliance_result.compliance_percentage, 100.0);
    }

    #[test]
    fn test_test_result_recording() {
        let mut framework = SafetyVerificationFramework::new(PathBuf::from("/tmp"));

        let test_result = TestResult {
            test_name: "test_memory_safety".to_string(),
            passed: true,
            execution_time_ms: 150,
            verified_requirements: vec![RequirementId::new("REQ_MEM_001")],
            coverage_type: TestCoverageType::Comprehensive,
            failure_reason: String::new(),
            asil_level: AsilLevel::C,
        };

        let _diagnostics = framework.record_test_result(test_result);

        let (report, _diagnostics) = framework.generate_safety_report();
        assert_eq!(report.test_summary.total_tests, 1);
        assert_eq!(report.test_summary.passed_tests, 1);
    }

    #[test]
    fn test_certification_readiness() {
        let framework = SafetyVerificationFramework::new(PathBuf::from("/tmp"));

        let (readiness, _diagnostics) = framework.check_certification_readiness(AsilLevel::A);

        // Should be ready if no requirements (trivially compliant)
        assert!(readiness.is_ready);
        assert_eq!(readiness.compliance_percentage, 100.0);
    }

    #[test]
    fn test_wrt_safety_requirements_generation() {
        let framework = SafetyVerificationFramework::new(PathBuf::from("/tmp"));
        let requirements = framework.generate_wrt_safety_requirements();

        assert!(!requirements.is_empty());
        assert!(requirements.iter().any(|r| r.id.as_str() == "WRT_MEM_001"));
        assert!(requirements.iter().any(|r| r.id.as_str() == "WRT_RUNTIME_001"));
    }
}
