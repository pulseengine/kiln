//! Streaming validation during WebAssembly parsing
//!
//! This module provides real-time validation capabilities that operate during
//! the parsing process, enabling early error detection and recovery.

#[cfg(not(feature = "std"))]
extern crate alloc;
// Import format! macro for no_std environments
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap as HashMap,
    string::{String, ToString},
    vec::Vec,
};
// Format macro is available through the prelude
#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::HashMap;

use wrt_error::{Error, ErrorCategory, Result, codes};

/// Validation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Information only
    Info,
    /// Warning that should be noted
    Warning,
    /// Error that prevents correct execution
    Error,
    /// Critical error that requires immediate attention
    Critical,
}

/// Validation issue discovered during parsing
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity level
    pub severity: ValidationSeverity,
    /// Location in the binary where issue was found
    pub offset: usize,
    /// Issue description
    pub message: String,
    /// Context information
    pub context: HashMap<String, String>,
    /// Suggested fix if available
    pub suggested_fix: Option<String>,
}

impl ValidationIssue {
    /// Create a new validation issue
    pub fn new(severity: ValidationSeverity, offset: usize, message: impl Into<String>) -> Self {
        Self {
            severity,
            offset,
            message: message.into(),
            context: HashMap::new(),
            suggested_fix: None,
        }
    }

    /// Add context information
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Add suggested fix
    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.suggested_fix = Some(fix.into());
        self
    }

    /// Check if this is an error or critical issue
    pub fn is_error(&self) -> bool {
        matches!(
            self.severity,
            ValidationSeverity::Error | ValidationSeverity::Critical
        )
    }
}

/// Streaming validator for WebAssembly binaries
#[derive(Debug)]
pub struct StreamingValidator {
    /// Issues found during validation
    issues: Vec<ValidationIssue>,
    /// Current parsing context
    context_stack: Vec<String>,
    /// Validation rules configuration
    config: ValidationConfig,
    /// Statistics
    stats: ValidationStats,
}

/// Validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum number of issues to collect before stopping
    pub max_issues: usize,
    /// Whether to continue parsing after critical errors
    pub continue_after_critical: bool,
    /// Whether to validate section ordering
    pub validate_section_order: bool,
    /// Whether to validate type consistency
    pub validate_types: bool,
    /// Whether to validate memory bounds
    pub validate_memory_bounds: bool,
    /// Maximum allowed function nesting depth
    pub max_nesting_depth: u32,
    /// Maximum allowed module size
    pub max_module_size: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_issues: 100,
            continue_after_critical: false,
            validate_section_order: true,
            validate_types: true,
            validate_memory_bounds: true,
            max_nesting_depth: 1024,
            max_module_size: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// Validation statistics
#[derive(Debug, Clone)]
pub struct ValidationStats {
    /// Total bytes validated
    pub bytes_validated: usize,
    /// Number of sections validated
    pub sections_validated: u32,
    /// Number of functions validated
    pub functions_validated: u32,
    /// Number of type definitions validated
    pub types_validated: u32,
    /// Validation start time (if std feature enabled)
    #[cfg(feature = "std")]
    pub start_time: std::time::Instant,
}

impl Default for ValidationStats {
    fn default() -> Self {
        Self {
            bytes_validated: 0,
            sections_validated: 0,
            functions_validated: 0,
            types_validated: 0,
            #[cfg(feature = "std")]
            start_time: std::time::Instant::now(),
        }
    }
}

impl StreamingValidator {
    /// Create a new streaming validator
    pub fn new() -> Result<Self> {
        Self::with_config(ValidationConfig::default())
    }

    /// Create a new streaming validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Result<Self> {
        Ok(Self {
            issues: Vec::new(),
            context_stack: Vec::new(),
            config,
            stats: ValidationStats {
                #[cfg(feature = "std")]
                start_time: std::time::Instant::now(),
                ..Default::default()
            },
        })
    }

    /// Enter a new validation context
    pub fn enter_context(&mut self, context: impl Into<String>) -> Result<()> {
        self.context_stack.push(context.into());
        Ok(())
    }

    /// Exit the current validation context
    pub fn exit_context(&mut self) -> Result<()> {
        if self.context_stack.pop().is_none() {
            return Err(Error::validation_error("Context stack underflow"));
        }
        Ok(())
    }

    /// Add a validation issue
    pub fn add_issue(&mut self, issue: ValidationIssue) -> Result<()> {
        if self.issues.len() >= self.config.max_issues {
            return Err(Error::validation_error("Too many validation issues"));
        }

        let should_abort =
            issue.severity == ValidationSeverity::Critical && !self.config.continue_after_critical;

        self.issues.push(issue);

        if should_abort {
            return Err(Error::validation_error("Critical validation error"));
        }

        Ok(())
    }

    /// Validate WASM magic number and version
    pub fn validate_header(&mut self, data: &[u8], offset: usize) -> Result<()> {
        self.enter_context("header")?;
        self.stats.bytes_validated += 8;

        if data.len() < 8 {
            self.add_issue(
                ValidationIssue::new(
                    ValidationSeverity::Critical,
                    offset,
                    "Insufficient data for WASM header",
                )
                .with_context("expected_size", "8")
                .with_context("actual_size", data.len().to_string()),
            )?;
            return Ok(());
        }

        // Check magic number
        if data[0..4] != [0x00, 0x61, 0x73, 0x6d] {
            self.add_issue(
                ValidationIssue::new(
                    ValidationSeverity::Critical,
                    offset,
                    "Invalid WASM magic number",
                )
                .with_context("expected", "00 61 73 6d")
                .with_context(
                    "actual",
                    format!(
                        "{:02x} {:02x} {:02x} {:02x}",
                        data[0], data[1], data[2], data[3]
                    ),
                )
                .with_fix("Ensure the file is a valid WebAssembly binary"),
            )?;
        }

        // Check version
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        match version {
            1 => {
                // Valid core module version
                self.add_issue(ValidationIssue::new(
                    ValidationSeverity::Info,
                    offset + 4,
                    "Core WebAssembly module (version 1)",
                ))?;
            },
            0x0a => {
                // Component model version
                self.add_issue(ValidationIssue::new(
                    ValidationSeverity::Info,
                    offset + 4,
                    "WebAssembly Component (version 0x0a)",
                ))?;
            },
            _ => {
                self.add_issue(
                    ValidationIssue::new(
                        ValidationSeverity::Warning,
                        offset + 4,
                        format!("Unknown WASM version: {}", version),
                    )
                    .with_context("version", version.to_string())
                    .with_fix("Consider updating to a supported version"),
                )?;
            },
        }

        self.exit_context()?;
        Ok(())
    }

    /// Validate section header
    pub fn validate_section_header(
        &mut self,
        section_id: u8,
        size: u32,
        offset: usize,
    ) -> Result<()> {
        self.enter_context(format!("section_{}", section_id))?;
        self.stats.sections_validated += 1;

        // Check for reasonable section size
        if size > self.config.max_module_size as u32 {
            self.add_issue(
                ValidationIssue::new(
                    ValidationSeverity::Error,
                    offset,
                    format!("Section too large: {} bytes", size),
                )
                .with_context("section_id", section_id.to_string())
                .with_context("max_size", self.config.max_module_size.to_string()),
            )?;
        }

        // Validate known section IDs
        match section_id {
            0 => self.add_issue(ValidationIssue::new(
                ValidationSeverity::Info,
                offset,
                "Custom section",
            ))?,
            1..=12 => {
                let section_name = match section_id {
                    1 => "Type",
                    2 => "Import",
                    3 => "Function",
                    4 => "Table",
                    5 => "Memory",
                    6 => "Global",
                    7 => "Export",
                    8 => "Start",
                    9 => "Element",
                    10 => "Code",
                    11 => "Data",
                    12 => "DataCount",
                    _ => unreachable!(),
                };
                self.add_issue(ValidationIssue::new(
                    ValidationSeverity::Info,
                    offset,
                    format!("{} section", section_name),
                ))?;
            },
            _ => {
                self.add_issue(
                    ValidationIssue::new(
                        ValidationSeverity::Warning,
                        offset,
                        format!("Unknown section ID: {}", section_id),
                    )
                    .with_context("section_id", section_id.to_string())
                    .with_fix("Consider using a custom section (ID 0) for non-standard data"),
                )?;
            },
        }

        self.exit_context()?;
        Ok(())
    }

    /// Validate LEB128 encoding
    pub fn validate_leb128(
        &mut self,
        data: &[u8],
        offset: usize,
        signed: bool,
    ) -> Result<(u64, usize)> {
        let mut result = 0u64;
        let mut shift = 0;
        let mut bytes_read = 0;

        for &byte in data {
            bytes_read += 1;

            if bytes_read > 10 {
                self.add_issue(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        offset,
                        "LEB128 encoding too long",
                    )
                    .with_context("max_bytes", "10")
                    .with_context("bytes_read", bytes_read.to_string()),
                )?;
                return Err(Error::parse_error("Invalid LEB128 encoding"));
            }

            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                // End of LEB128
                break;
            }
        }

        self.stats.bytes_validated += bytes_read;

        if signed && shift < 64 && (data[bytes_read - 1] & 0x40) != 0 {
            // Sign extend for signed LEB128
            result |= !0u64 << shift;
        }

        Ok((result, bytes_read))
    }

    /// Validate function signature
    pub fn validate_function_signature(
        &mut self,
        params: &[u8],
        results: &[u8],
        offset: usize,
    ) -> Result<()> {
        self.enter_context("function_signature")?;
        self.stats.functions_validated += 1;

        // Check parameter count
        if params.len() > 1000 {
            self.add_issue(
                ValidationIssue::new(
                    ValidationSeverity::Warning,
                    offset,
                    format!("Function has many parameters: {}", params.len()),
                )
                .with_context("param_count", params.len().to_string()),
            )?;
        }

        // Check result count (WASM 1.0 allows at most 1 result)
        if results.len() > 1 {
            self.add_issue(
                ValidationIssue::new(
                    ValidationSeverity::Warning,
                    offset,
                    format!("Function has multiple results: {}", results.len()),
                )
                .with_context("result_count", results.len().to_string())
                .with_fix("Multiple results require WASM multi-value proposal"),
            )?;
        }

        // Validate parameter types
        for (i, &param_type) in params.iter().enumerate() {
            if !self.is_valid_value_type(param_type) {
                self.add_issue(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        offset,
                        format!(
                            "Invalid parameter type at index {}: 0x{:02x}",
                            i, param_type
                        ),
                    )
                    .with_context("param_index", i.to_string())
                    .with_context("type_value", format!("0x{:02x}", param_type)),
                )?;
            }
        }

        // Validate result types
        for (i, &result_type) in results.iter().enumerate() {
            if !self.is_valid_value_type(result_type) {
                self.add_issue(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        offset,
                        format!("Invalid result type at index {}: 0x{:02x}", i, result_type),
                    )
                    .with_context("result_index", i.to_string())
                    .with_context("type_value", format!("0x{:02x}", result_type)),
                )?;
            }
        }

        self.exit_context()?;
        Ok(())
    }

    /// Check if a value type is valid
    fn is_valid_value_type(&self, type_byte: u8) -> bool {
        matches!(type_byte, 0x7C..=0x7F) // i32, i64, f32, f64
    }

    /// Get all validation issues
    pub fn get_issues(&self) -> &Vec<ValidationIssue> {
        &self.issues
    }

    /// Get validation statistics
    pub fn get_stats(&self) -> &ValidationStats {
        &self.stats
    }

    /// Check if validation passed (no errors or critical issues)
    pub fn validation_passed(&self) -> bool {
        !self.issues.iter().any(|issue| issue.is_error())
    }

    /// Generate validation report
    pub fn generate_report(&self) -> ValidationReport {
        let mut error_count = 0;
        let mut warning_count = 0;
        let mut info_count = 0;
        let mut critical_count = 0;

        for issue in &self.issues {
            match issue.severity {
                ValidationSeverity::Info => info_count += 1,
                ValidationSeverity::Warning => warning_count += 1,
                ValidationSeverity::Error => error_count += 1,
                ValidationSeverity::Critical => critical_count += 1,
            }
        }

        ValidationReport {
            total_issues: self.issues.len(),
            critical_count,
            error_count,
            warning_count,
            info_count,
            validation_passed: self.validation_passed(),
            stats: self.stats.clone(),
        }
    }
}

impl Default for StreamingValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create default validator")
    }
}

/// Validation report summary
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Total number of issues found
    pub total_issues: usize,
    /// Number of critical issues
    pub critical_count: usize,
    /// Number of errors
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
    /// Number of info messages
    pub info_count: usize,
    /// Whether validation passed overall
    pub validation_passed: bool,
    /// Validation statistics
    pub stats: ValidationStats,
}

impl ValidationReport {
    /// Check if there are any issues that need attention
    pub fn has_issues(&self) -> bool {
        self.total_issues > 0
    }

    /// Check if there are any blocking issues
    pub fn has_blocking_issues(&self) -> bool {
        self.critical_count > 0 || self.error_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = StreamingValidator::new().unwrap();
        assert_eq!(validator.issues.len(), 0);
        assert!(validator.validation_passed());
    }

    #[test]
    fn test_header_validation() {
        let mut validator = StreamingValidator::new().unwrap();

        // Valid WASM header
        let valid_header = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        validator.validate_header(&valid_header, 0).unwrap();

        let report = validator.generate_report();
        assert_eq!(report.info_count, 1); // Should have info about core module
        assert!(report.validation_passed);
    }

    #[test]
    fn test_invalid_magic() {
        let mut validator = StreamingValidator::new().unwrap();

        // Invalid magic number
        let invalid_header = [0xFF, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        validator.validate_header(&invalid_header, 0).unwrap();

        let report = validator.generate_report();
        assert!(report.critical_count > 0);
        assert!(!report.validation_passed);
    }

    #[test]
    fn test_leb128_validation() {
        let mut validator = StreamingValidator::new().unwrap();

        // Valid LEB128: 42 (0x2A)
        let leb_data = [0x2A];
        let (value, bytes_read) = validator.validate_leb128(&leb_data, 0, false).unwrap();
        assert_eq!(value, 42);
        assert_eq!(bytes_read, 1);
    }

    #[test]
    fn test_context_management() {
        let mut validator = StreamingValidator::new().unwrap();

        validator.enter_context("test").unwrap();
        validator.enter_context("nested").unwrap();
        validator.exit_context().unwrap();
        validator.exit_context().unwrap();

        // Should not panic or error
    }
}
