//! Enhanced requirements model with SCORE-inspired verification framework
//!
//! This module provides a comprehensive requirements traceability system
//! inspired by SCORE's approach to safety-critical system verification. It
//! links requirements to implementation code, tests, and documentation for full
//! accountability.

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

use crate::config::AsilLevel;

/// Unique identifier for a requirement
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequirementId(String);

impl RequirementId {
    /// Create a new requirement identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequirementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of requirement based on safety standards
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementType {
    /// Functional requirement (what the system must do)
    Functional,
    /// Performance requirement (timing, throughput, etc.)
    Performance,
    /// Safety requirement (ASIL-related)
    Safety,
    /// Security requirement (protection against attacks)
    Security,
    /// Reliability requirement (availability, fault tolerance)
    Reliability,
    /// Qualification requirement (certification, standards)
    Qualification,
    /// Platform requirement (hardware/OS specific)
    Platform,
    /// Memory requirement (allocation, constraints)
    Memory,
}

impl fmt::Display for RequirementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequirementType::Functional => write!(f, "Functional"),
            RequirementType::Performance => write!(f, "Performance"),
            RequirementType::Safety => write!(f, "Safety"),
            RequirementType::Security => write!(f, "Security"),
            RequirementType::Reliability => write!(f, "Reliability"),
            RequirementType::Qualification => write!(f, "Qualification"),
            RequirementType::Platform => write!(f, "Platform"),
            RequirementType::Memory => write!(f, "Memory"),
        }
    }
}

/// Verification method for a requirement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    /// Requirement verified through inspection/review
    Inspection,
    /// Requirement verified through analysis (static/dynamic)
    Analysis,
    /// Requirement verified through testing
    Test,
    /// Requirement verified through demonstration
    Demonstration,
    /// Requirement verified through simulation
    Simulation,
    /// Requirement verified through formal proof
    FormalProof,
}

impl fmt::Display for VerificationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationMethod::Inspection => write!(f, "Inspection"),
            VerificationMethod::Analysis => write!(f, "Analysis"),
            VerificationMethod::Test => write!(f, "Test"),
            VerificationMethod::Demonstration => write!(f, "Demonstration"),
            VerificationMethod::Simulation => write!(f, "Simulation"),
            VerificationMethod::FormalProof => write!(f, "Formal Proof"),
        }
    }
}

/// Current status of requirement verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Verification not started
    NotStarted,
    /// Verification in progress
    InProgress,
    /// Verification completed successfully
    Verified,
    /// Verification failed
    Failed(String),
    /// Verification not applicable
    NotApplicable,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationStatus::NotStarted => write!(f, "Not Started"),
            VerificationStatus::InProgress => write!(f, "In Progress"),
            VerificationStatus::Verified => write!(f, "Verified"),
            VerificationStatus::Failed(reason) => write!(f, "Failed: {}", reason),
            VerificationStatus::NotApplicable => write!(f, "Not Applicable"),
        }
    }
}

/// Coverage level for requirement testing
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageLevel {
    /// No coverage
    None,
    /// Basic coverage (happy path)
    Basic,
    /// Comprehensive coverage (edge cases)
    Comprehensive,
    /// Complete coverage (all paths, formal verification)
    Complete,
}

impl fmt::Display for CoverageLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverageLevel::None => write!(f, "None"),
            CoverageLevel::Basic => write!(f, "Basic"),
            CoverageLevel::Comprehensive => write!(f, "Comprehensive"),
            CoverageLevel::Complete => write!(f, "Complete"),
        }
    }
}

/// Safety requirement definition with full traceability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRequirement {
    /// Unique identifier
    pub id: RequirementId,
    /// Human-readable title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Requirement type classification
    pub req_type: RequirementType,
    /// Required ASIL level
    pub asil_level: AsilLevel,
    /// Verification method
    pub verification_method: VerificationMethod,
    /// Current verification status
    pub status: VerificationStatus,
    /// Coverage level achieved
    pub coverage: CoverageLevel,
    /// Parent requirement (if this is derived)
    pub parent: Option<RequirementId>,
    /// Source document/standard
    pub source: String,
    /// Implementation references
    pub implementations: Vec<String>,
    /// Test references
    pub tests: Vec<String>,
    /// Documentation references
    pub documentation: Vec<String>,
}

impl SafetyRequirement {
    /// Create a new safety requirement
    pub fn new(
        id: RequirementId,
        title: String,
        description: String,
        req_type: RequirementType,
        asil_level: AsilLevel,
    ) -> Self {
        Self {
            id,
            title,
            description,
            req_type,
            asil_level,
            verification_method: VerificationMethod::Test,
            status: VerificationStatus::NotStarted,
            coverage: CoverageLevel::None,
            parent: None,
            source: String::new(),
            implementations: Vec::new(),
            tests: Vec::new(),
            documentation: Vec::new(),
        }
    }

    /// Add implementation reference
    pub fn add_implementation(&mut self, implementation: String) {
        self.implementations.push(implementation);
    }

    /// Add test reference
    pub fn add_test(&mut self, test: String) {
        self.tests.push(test);
    }

    /// Add documentation reference
    pub fn add_documentation(&mut self, doc: String) {
        self.documentation.push(doc);
    }

    /// Set verification status
    pub fn set_status(&mut self, status: VerificationStatus) {
        self.status = status;
    }

    /// Set coverage level
    pub fn set_coverage(&mut self, coverage: CoverageLevel) {
        self.coverage = coverage;
    }

    /// Check if requirement is fully verified
    pub fn is_verified(&self) -> bool {
        matches!(self.status, VerificationStatus::Verified)
            && self.coverage >= CoverageLevel::Basic
            && !self.implementations.is_empty()
    }

    /// Check if requirement needs implementation
    pub fn needs_implementation(&self) -> bool {
        self.implementations.is_empty()
    }

    /// Check if requirement needs testing
    pub fn needs_testing(&self) -> bool {
        self.tests.is_empty() || self.coverage < CoverageLevel::Basic
    }

    /// Get compliance score (0.0 to 1.0)
    pub fn compliance_score(&self) -> f64 {
        let mut score = 0.0;
        let total_points = 4.0;

        // Implementation coverage
        if !self.implementations.is_empty() {
            score += 1.0;
        }

        // Test coverage
        match self.coverage {
            CoverageLevel::None => {},
            CoverageLevel::Basic => score += 0.5,
            CoverageLevel::Comprehensive => score += 0.8,
            CoverageLevel::Complete => score += 1.0,
        }

        // Verification status
        match &self.status {
            VerificationStatus::Verified => score += 1.0,
            VerificationStatus::InProgress => score += 0.3,
            _ => {},
        }

        // Documentation
        if !self.documentation.is_empty() {
            score += 1.0;
        }

        score / total_points
    }
}

/// Requirements registry for tracking all safety requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequirementRegistry {
    /// All registered requirements
    pub requirements: Vec<SafetyRequirement>,
}

impl RequirementRegistry {
    /// Create a new requirements registry
    pub fn new() -> Self {
        Self {
            requirements: Vec::new(),
        }
    }

    /// Add a requirement to the registry
    pub fn add_requirement(&mut self, requirement: SafetyRequirement) {
        self.requirements.push(requirement);
    }

    /// Get requirement by ID
    pub fn get_requirement(&self, id: &RequirementId) -> Option<&SafetyRequirement> {
        self.requirements.iter().find(|r| r.id == *id)
    }

    /// Get mutable requirement by ID
    pub fn get_requirement_mut(&mut self, id: &RequirementId) -> Option<&mut SafetyRequirement> {
        self.requirements.iter_mut().find(|r| r.id == *id)
    }

    /// Get all requirements for a specific ASIL level
    pub fn get_requirements_by_asil(&self, asil_level: AsilLevel) -> Vec<&SafetyRequirement> {
        self.requirements.iter().filter(|r| r.asil_level == asil_level).collect()
    }

    /// Get all requirements of a specific type
    pub fn get_requirements_by_type(&self, req_type: &RequirementType) -> Vec<&SafetyRequirement> {
        self.requirements.iter().filter(|r| r.req_type == *req_type).collect()
    }

    /// Get unverified requirements
    pub fn get_unverified_requirements(&self) -> Vec<&SafetyRequirement> {
        self.requirements.iter().filter(|r| !r.is_verified()).collect()
    }

    /// Get requirements needing implementation
    pub fn get_requirements_needing_implementation(&self) -> Vec<&SafetyRequirement> {
        self.requirements.iter().filter(|r| r.needs_implementation()).collect()
    }

    /// Get requirements needing testing
    pub fn get_requirements_needing_testing(&self) -> Vec<&SafetyRequirement> {
        self.requirements.iter().filter(|r| r.needs_testing()).collect()
    }

    /// Calculate overall compliance percentage
    pub fn overall_compliance(&self) -> f64 {
        if self.requirements.is_empty() {
            return 1.0; // 100% compliant if no requirements
        }

        let total_score: f64 = self.requirements.iter().map(|r| r.compliance_score()).sum();

        total_score / self.requirements.len() as f64
    }

    /// Calculate ASIL-specific compliance
    pub fn asil_compliance(&self, asil_level: AsilLevel) -> f64 {
        let asil_requirements = self.get_requirements_by_asil(asil_level);

        if asil_requirements.is_empty() {
            return 1.0;
        }

        let total_score: f64 = asil_requirements.iter().map(|r| r.compliance_score()).sum();

        total_score / asil_requirements.len() as f64
    }

    /// Generate compliance report
    pub fn generate_compliance_report(&self) -> ComplianceReport {
        let mut asil_compliance = HashMap::new();

        for asil in &[
            AsilLevel::QM,
            AsilLevel::A,
            AsilLevel::B,
            AsilLevel::C,
            AsilLevel::D,
        ] {
            asil_compliance.insert(*asil, self.asil_compliance(*asil));
        }

        ComplianceReport {
            total_requirements: self.requirements.len(),
            verified_requirements: self.requirements.iter().filter(|r| r.is_verified()).count(),
            overall_compliance: self.overall_compliance(),
            asil_compliance,
            unverified_count: self.get_unverified_requirements().len(),
            missing_implementation_count: self.get_requirements_needing_implementation().len(),
            missing_testing_count: self.get_requirements_needing_testing().len(),
        }
    }
}

/// Compliance report summarizing requirement verification status
#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Total number of requirements
    pub total_requirements: usize,
    /// Number of verified requirements
    pub verified_requirements: usize,
    /// Overall compliance percentage (0.0 to 1.0)
    pub overall_compliance: f64,
    /// ASIL-specific compliance percentages
    pub asil_compliance: HashMap<AsilLevel, f64>,
    /// Number of unverified requirements
    pub unverified_count: usize,
    /// Number of requirements missing implementation
    pub missing_implementation_count: usize,
    /// Number of requirements missing testing
    pub missing_testing_count: usize,
}

impl ComplianceReport {
    /// Check if the system meets minimum compliance threshold
    pub fn meets_compliance_threshold(&self, threshold: f64) -> bool {
        self.overall_compliance >= threshold
    }

    /// Get the lowest ASIL compliance level
    pub fn lowest_asil_compliance(&self) -> Option<(AsilLevel, f64)> {
        self.asil_compliance
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(asil, compliance)| (*asil, *compliance))
    }

    /// Format report as human-readable text
    pub fn format_human(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("📊 Requirements Compliance Report\n"));
        output.push_str(&format!("═══════════════════════════════\n\n"));

        output.push_str(&format!(
            "Overall Compliance: {:.1}%\n",
            self.overall_compliance * 100.0
        ));
        output.push_str(&format!(
            "Total Requirements: {}\n",
            self.total_requirements
        ));
        output.push_str(&format!(
            "Verified: {} ({:.1}%)\n",
            self.verified_requirements,
            (self.verified_requirements as f64 / self.total_requirements as f64) * 100.0
        ));

        output.push_str(&format!("\n📈 ASIL Compliance:\n"));
        output.push_str(&format!("──────────────────\n"));

        let mut asil_levels: Vec<_> = self.asil_compliance.iter().collect();
        asil_levels.sort_by_key(|(asil, _)| match asil {
            AsilLevel::QM => 0,
            AsilLevel::A => 1,
            AsilLevel::B => 2,
            AsilLevel::C => 3,
            AsilLevel::D => 4,
        });

        for (asil, compliance) in asil_levels {
            output.push_str(&format!("  {}: {:.1}%\n", asil, compliance * 100.0));
        }

        if self.unverified_count > 0 {
            output.push_str(&format!("\n⚠️  Issues Found:\n"));
            output.push_str(&format!("──────────────\n"));
            output.push_str(&format!("  Unverified: {}\n", self.unverified_count));
            output.push_str(&format!(
                "  Missing Implementation: {}\n",
                self.missing_implementation_count
            ));
            output.push_str(&format!(
                "  Missing Tests: {}\n",
                self.missing_testing_count
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirement_creation() {
        let req = SafetyRequirement::new(
            RequirementId::new("REQ_MEM_001"),
            "Memory Safety".to_string(),
            "All memory allocations must be bounded".to_string(),
            RequirementType::Memory,
            AsilLevel::C,
        );

        assert_eq!(req.id.as_str(), "REQ_MEM_001");
        assert_eq!(req.asil_level, AsilLevel::C);
        assert!(!req.is_verified());
        assert!(req.needs_implementation());
    }

    #[test]
    fn test_compliance_calculation() {
        let mut req = SafetyRequirement::new(
            RequirementId::new("REQ_TEST_001"),
            "Test Requirement".to_string(),
            "Test description".to_string(),
            RequirementType::Functional,
            AsilLevel::A,
        );

        // Initially no compliance
        assert_eq!(req.compliance_score(), 0.0);

        // Add implementation
        req.add_implementation("src/test.rs".to_string());
        assert!(req.compliance_score() > 0.0);

        // Add testing
        req.set_coverage(CoverageLevel::Comprehensive);
        // Score = (1.0 impl + 0.8 comprehensive) / 4.0 = 0.45
        assert!(req.compliance_score() > 0.4);

        // Mark as verified
        req.set_status(VerificationStatus::Verified);
        // Score = (1.0 impl + 0.8 comprehensive + 1.0 verified) / 4.0 = 0.7
        assert!(req.compliance_score() > 0.6);
    }
}
