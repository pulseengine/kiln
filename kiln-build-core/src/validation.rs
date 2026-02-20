//! Code validation utilities
//!
//! This module provides various code validation checks to ensure
//! code quality and organization standards.

use std::{
    fs,
    path::{Path, PathBuf},
};

use colored::Colorize;

use crate::error::{BuildError, BuildResult};

/// Validation results
#[derive(Debug)]
pub struct ValidationResults {
    /// Whether all validations passed
    pub success: bool,
    /// List of errors found
    pub errors: Vec<ValidationError>,
    /// List of warnings
    pub warnings: Vec<String>,
    /// Time taken for validation
    pub duration: std::time::Duration,
}

impl ValidationResults {
    /// Create new validation results
    pub fn new() -> Self {
        Self {
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            duration: std::time::Duration::default(),
        }
    }
}

/// Validation error details
#[derive(Debug)]
pub struct ValidationError {
    /// Error category
    pub category: String,
    /// File path where error occurred
    pub file: PathBuf,
    /// Error message
    pub message: String,
    /// Severity level
    pub severity: String,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(category: impl Into<String>, file: PathBuf, message: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            file,
            message: message.into(),
            severity: "error".to_string(),
        }
    }
}

/// Code validator
pub struct CodeValidator {
    workspace_root: PathBuf,
    verbose: bool,
}

impl CodeValidator {
    /// Create a new code validator
    pub fn new(workspace_root: PathBuf, verbose: bool) -> Self {
        Self {
            workspace_root,
            verbose,
        }
    }

    /// Check for test files in src/ directories
    pub fn check_no_test_files_in_src(&self) -> BuildResult<ValidationResults> {
        println!(
            "{} Checking for test files in src/ directories...",
            "🔍".bright_blue()
        );

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Walk through all Cargo.toml files to find crates
        let crates = self.find_workspace_crates()?;

        // Crates that legitimately contain test-related files as part of their implementation
        // (not unit tests, but testing infrastructure code)
        let excluded_crates = ["cargo-wrt", "wrt-build-core"];

        for crate_path in crates {
            // Skip excluded crates that legitimately contain test infrastructure
            if let Some(crate_name) = crate_path.file_name().and_then(|n| n.to_str()) {
                if excluded_crates.contains(&crate_name) {
                    continue;
                }
            }

            let src_dir = crate_path.join("src");
            if src_dir.exists() {
                self.check_directory_for_test_files(&src_dir, &mut errors)?;
            }
        }

        let success = errors.is_empty();

        if success {
            println!(
                "{} No test files found in src/ directories",
                "✅".bright_green()
            );
        } else {
            println!(
                "{} Found {} test files in src/ directories",
                "❌".bright_red(),
                errors.len()
            );
            for error in &errors {
                println!("  - {}", error.file.display());
            }
        }

        Ok(ValidationResults {
            success,
            errors,
            warnings,
            duration: std::time::Duration::default(),
        })
    }

    /// Check module documentation coverage
    pub fn check_module_documentation(&self) -> BuildResult<ValidationResults> {
        println!(
            "{} Checking module documentation coverage...",
            "📚".bright_blue()
        );

        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut total_modules = 0;
        let mut documented_modules = 0;

        // Walk through all Rust files
        let crates = self.find_workspace_crates()?;

        for crate_path in crates {
            let src_dir = crate_path.join("src");
            if src_dir.exists() {
                self.check_directory_documentation(
                    &src_dir,
                    &mut total_modules,
                    &mut documented_modules,
                    &mut warnings,
                )?;
            }
        }

        let coverage_percent = if total_modules > 0 {
            (documented_modules as f64 / total_modules as f64) * 100.0
        } else {
            100.0
        };

        println!(
            "{} Module documentation coverage: {:.1}%",
            "📊".bright_cyan(),
            coverage_percent
        );
        println!("  Documented: {}/{}", documented_modules, total_modules);

        if coverage_percent < 80.0 {
            errors.push(ValidationError::new(
                "test-coverage",
                self.workspace_root.clone(),
                format!(
                    "Test coverage {:.1}% is below 80% threshold",
                    coverage_percent
                ),
            ));
        }

        Ok(ValidationResults {
            success: errors.is_empty(),
            errors,
            warnings,
            duration: std::time::Duration::default(),
        })
    }

    /// Comprehensive documentation audit for all crates
    pub fn audit_crate_documentation(&self) -> BuildResult<ValidationResults> {
        println!("{} Auditing crate documentation...", "📚".bright_blue());

        let mut results = ValidationResults::new();
        let start = std::time::Instant::now();

        // Find all crate directories
        let mut crates = Vec::new();
        for entry in fs::read_dir(&self.workspace_root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let cargo_toml = path.join("Cargo.toml");
                if cargo_toml.exists() {
                    // Check if it's a crate (has [package] section)
                    let content = fs::read_to_string(&cargo_toml)?;
                    if content.contains("[package]") {
                        crates.push(path);
                    }
                }
            }
        }

        println!("    Found {} crates to audit", crates.len());

        let mut missing_readme = Vec::new();
        let mut missing_metadata = Vec::new();
        let mut poor_documentation = Vec::new();

        for crate_path in &crates {
            let crate_name = crate_path.file_name().unwrap().to_string_lossy();

            // Check README.md
            let readme_path = crate_path.join("README.md");
            if !readme_path.exists() {
                missing_readme.push(crate_name.to_string());
                results.errors.push(ValidationError::new(
                    "missing-readme",
                    readme_path.clone(),
                    "Missing README.md file",
                ));
            }

            // Check Cargo.toml metadata
            let cargo_toml_path = crate_path.join("Cargo.toml");
            let cargo_content = fs::read_to_string(&cargo_toml_path)?;

            let metadata_items = ["description", "documentation", "keywords", "categories"];
            let mut has_all_metadata = true;

            for item in &metadata_items {
                if !cargo_content.contains(&format!("{} =", item)) {
                    has_all_metadata = false;
                    results
                        .warnings
                        .push(format!("{}: Missing {} in Cargo.toml", crate_name, item));
                }
            }

            if !cargo_content.contains("[package.metadata.docs.rs]") {
                has_all_metadata = false;
                results.warnings.push(format!("{}: Missing docs.rs configuration", crate_name));
            }

            if !has_all_metadata {
                missing_metadata.push(crate_name.to_string());
            }

            // Check lib.rs documentation quality
            let lib_rs_path = crate_path.join("src/lib.rs");
            if lib_rs_path.exists() {
                let lib_content = fs::read_to_string(&lib_rs_path)?;
                let mut has_good_docs = true;

                // Check for crate-level documentation
                if !lib_content.lines().any(|line| line.starts_with("//! ")) {
                    has_good_docs = false;
                    results.errors.push(ValidationError::new(
                        "missing-docs",
                        lib_rs_path.clone(),
                        format!(
                            "Missing module-level documentation in {}",
                            lib_rs_path.display()
                        ),
                    ));
                } else {
                    // Check for examples in crate docs
                    let has_examples =
                        lib_content.contains("//! ```rust") || lib_content.contains("//! ```");
                    if !has_examples {
                        results.warnings.push(format!(
                            "{}: No examples in crate documentation",
                            crate_name
                        ));
                    }

                    // Check for features section
                    if !lib_content.contains("//! ## Features")
                        && !lib_content.contains("//! # Features")
                    {
                        results.warnings.push(format!(
                            "{}: No features section in documentation",
                            crate_name
                        ));
                    }
                }

                // Check for missing_docs lint
                if !lib_content.contains("#![warn(missing_docs)]")
                    && !lib_content.contains("#![deny(missing_docs)]")
                {
                    has_good_docs = false;
                    results.warnings.push(format!(
                        "{}: Missing #![warn(missing_docs)] lint",
                        crate_name
                    ));
                }

                if !has_good_docs {
                    poor_documentation.push(crate_name.to_string());
                }
            }
        }

        results.duration = start.elapsed();
        results.success = results.errors.is_empty();

        // Print summary
        println!("\n  {} Summary:", "📊".bright_cyan());
        println!("    Total crates: {}", crates.len());
        println!("    Crates missing README: {}", missing_readme.len());
        println!(
            "    Crates with incomplete metadata: {}",
            missing_metadata.len()
        );
        println!(
            "    Crates with poor documentation: {}",
            poor_documentation.len()
        );

        if !missing_readme.is_empty() {
            println!("\n    {} Crates missing README:", "❌".bright_red());
            for crate_name in &missing_readme {
                println!("      - {}", crate_name);
            }
        }

        if !missing_metadata.is_empty() {
            println!(
                "\n    {} Crates with incomplete metadata:",
                "⚠️".bright_yellow()
            );
            for crate_name in &missing_metadata {
                println!("      - {}", crate_name);
            }
        }

        if !poor_documentation.is_empty() {
            println!(
                "\n    {} Crates with poor documentation:",
                "⚠️".bright_yellow()
            );
            for crate_name in &poor_documentation {
                println!("      - {}", crate_name);
            }
        }

        if results.success {
            println!(
                "\n  {} All crates have proper documentation!",
                "✅".bright_green()
            );
        } else {
            println!("\n  {} Documentation audit found issues", "❌".bright_red());
        }

        Ok(results)
    }

    /// Find all workspace crates
    fn find_workspace_crates(&self) -> BuildResult<Vec<PathBuf>> {
        let mut crates = vec![self.workspace_root.clone()];

        // Read workspace Cargo.toml
        let workspace_toml = self.workspace_root.join("Cargo.toml");
        if let Ok(content) = fs::read_to_string(&workspace_toml) {
            // Simple parsing - look for workspace members
            if let Some(members_start) = content.find("members = [") {
                if let Some(members_end) = content[members_start..].find(']') {
                    let members_section = &content[members_start..members_start + members_end];
                    for line in members_section.lines() {
                        let line = line.trim();
                        if line.starts_with('"') && line.ends_with('"') {
                            let member = line.trim_matches('"').trim_matches(',');
                            let member_path = self.workspace_root.join(member);
                            if member_path.exists() {
                                crates.push(member_path);
                            }
                        }
                    }
                }
            }
        }

        Ok(crates)
    }

    /// Check a directory recursively for test files
    fn check_directory_for_test_files(
        &self,
        dir: &Path,
        errors: &mut Vec<ValidationError>,
    ) -> BuildResult<()> {
        for entry in fs::read_dir(dir).map_err(|e| {
            BuildError::Tool(format!("Failed to read directory {}: {}", dir.display(), e))
        })? {
            let entry = entry
                .map_err(|e| BuildError::Tool(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check for test files
                    if file_name.ends_with("_test.rs")
                        || file_name.ends_with("_tests.rs")
                        || file_name == "test.rs"
                        || file_name == "tests.rs"
                        || (file_name.contains("test")
                            && file_name.ends_with(".rs")
                            && !file_name.contains("test_utils"))
                    {
                        errors.push(ValidationError::new(
                            "security-vulnerability",
                            path.clone(),
                            format!("Security vulnerability found in {}", path.display()),
                        ));
                    }
                }
            } else if path.is_dir() {
                // Skip target and .git directories
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if dir_name != "target" && dir_name != ".git" {
                        self.check_directory_for_test_files(&path, errors)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check directory for module documentation
    fn check_directory_documentation(
        &self,
        dir: &Path,
        total_modules: &mut usize,
        documented_modules: &mut usize,
        warnings: &mut Vec<String>,
    ) -> BuildResult<()> {
        for entry in fs::read_dir(dir).map_err(|e| {
            BuildError::Tool(format!("Failed to read directory {}: {}", dir.display(), e))
        })? {
            let entry = entry
                .map_err(|e| BuildError::Tool(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.ends_with(".rs") && file_name != "lib.rs" && file_name != "main.rs"
                    {
                        *total_modules += 1;

                        // Check if file has module documentation
                        if let Ok(content) = fs::read_to_string(&path) {
                            let lines: Vec<&str> = content.lines().collect();
                            let mut has_module_doc = false;

                            // Look for //! at the start of the file
                            for line in lines.iter().take(10) {
                                let trimmed = line.trim();
                                if trimmed.starts_with("//!") {
                                    has_module_doc = true;
                                    break;
                                }
                                // Skip empty lines and attributes
                                if !trimmed.is_empty()
                                    && !trimmed.starts_with("#[")
                                    && !trimmed.starts_with("#!")
                                {
                                    break;
                                }
                            }

                            if has_module_doc {
                                *documented_modules += 1;
                            } else if self.verbose {
                                warnings.push(format!(
                                    "Missing module documentation: {}",
                                    path.display()
                                ));
                            }
                        }
                    }
                }
            } else if path.is_dir() {
                // Skip certain directories
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if dir_name != "target"
                        && dir_name != ".git"
                        && dir_name != "tests"
                        && dir_name != "benches"
                    {
                        // Check mod.rs in subdirectories
                        let mod_file = path.join("mod.rs");
                        if mod_file.exists() {
                            *total_modules += 1;

                            if let Ok(content) = fs::read_to_string(&mod_file) {
                                if content.trim_start().starts_with("//!") {
                                    *documented_modules += 1;
                                } else if self.verbose {
                                    warnings.push(format!(
                                        "Missing module documentation: {}",
                                        mod_file.display()
                                    ));
                                }
                            }
                        }

                        self.check_directory_documentation(
                            &path,
                            total_modules,
                            documented_modules,
                            warnings,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Run all validation checks
pub fn run_all_validations(workspace_root: &Path, verbose: bool) -> BuildResult<bool> {
    let validator = CodeValidator::new(workspace_root.to_path_buf(), verbose);

    println!("{} Running code validation checks...", "🔍".bright_blue());
    println!();

    let mut all_passed = true;

    // Check for test files in src/
    let test_files_result = validator.check_no_test_files_in_src()?;
    if !test_files_result.success {
        all_passed = false;
    }

    println!();

    // Check module documentation
    let doc_result = validator.check_module_documentation()?;
    if !doc_result.success {
        all_passed = false;
    }

    println!();

    // Audit crate documentation
    let audit_result = validator.audit_crate_documentation()?;
    if !audit_result.success {
        all_passed = false;
    }

    println!();

    if all_passed {
        println!("{} All validation checks passed!", "✅".bright_green());
    } else {
        println!("{} Some validation checks failed", "❌".bright_red());
    }

    Ok(all_passed)
}
