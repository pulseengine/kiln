//! Auto-fix functionality for common issues
//!
//! Provides automated fixes for common diagnostics and issues that can be
//! safely resolved without manual intervention.

use std::path::Path;

use anyhow::{Context, Result};
use wrt_build_core::diagnostics::{Diagnostic, DiagnosticCollection, Severity};

use super::OutputManager;

/// Auto-fix manager for applying automated corrections
pub struct AutoFixManager {
    output: OutputManager,
    dry_run: bool,
}

impl AutoFixManager {
    pub fn new(output: OutputManager, dry_run: bool) -> Self {
        Self { output, dry_run }
    }

    /// Apply auto-fixes to a collection of diagnostics
    #[must_use]
    pub fn apply_fixes(&self, diagnostics: &DiagnosticCollection) -> Result<AutoFixResult> {
        let mut result = AutoFixResult::default();

        for diagnostic in &diagnostics.diagnostics {
            if let Some(fix) = self.get_fix_for_diagnostic(diagnostic) {
                match self.apply_fix(&fix) {
                    Ok(()) => {
                        result.successful_fixes += 1;
                        self.output.success(&format!("Fixed: {}", fix.description));
                    },
                    Err(e) => {
                        result.failed_fixes += 1;
                        self.output.warning(&format!("Failed to fix {}: {}", fix.description, e));
                    },
                }
            }
        }

        Ok(result)
    }

    /// Get a fix for a specific diagnostic
    fn get_fix_for_diagnostic(&self, diagnostic: &Diagnostic) -> Option<AutoFix> {
        match diagnostic.code.as_deref() {
            // Formatting issues
            Some("rustfmt") => Some(AutoFix {
                description: format!("Format {}", diagnostic.file),
                fix_type: AutoFixType::Format,
                target: diagnostic.file.clone(),
            }),

            // Missing documentation
            Some("missing_docs") => Some(AutoFix {
                description: format!("Add documentation to {}", diagnostic.file),
                fix_type: AutoFixType::AddDocumentation,
                target: diagnostic.file.clone(),
            }),

            // Unused imports
            Some("unused_imports") => Some(AutoFix {
                description: format!("Remove unused imports in {}", diagnostic.file),
                fix_type: AutoFixType::RemoveUnusedImports,
                target: diagnostic.file.clone(),
            }),

            // Clippy suggestions with auto-fix
            Some(code)
                if code.starts_with("clippy::") && diagnostic.severity == Severity::Warning =>
            {
                Some(AutoFix {
                    description: format!("Apply clippy suggestion in {}", diagnostic.file),
                    fix_type: AutoFixType::ClippyFix(code.to_string()),
                    target: diagnostic.file.clone(),
                })
            },

            _ => None,
        }
    }

    /// Apply a specific fix
    fn apply_fix(&self, fix: &AutoFix) -> Result<()> {
        if self.dry_run {
            self.output.info(&format!("[DRY RUN] Would apply: {}", fix.description));
            return Ok(());
        }

        match &fix.fix_type {
            AutoFixType::Format => self.apply_format_fix(&fix.target),
            AutoFixType::AddDocumentation => self.apply_documentation_fix(&fix.target),
            AutoFixType::RemoveUnusedImports => self.apply_unused_imports_fix(&fix.target),
            AutoFixType::ClippyFix(suggestion) => self.apply_clippy_fix(&fix.target, suggestion),
        }
    }

    /// Apply rustfmt to a file
    fn apply_format_fix(&self, file: &str) -> Result<()> {
        let status = std::process::Command::new("rustfmt")
            .arg("--edition=2021")
            .arg(file)
            .status()
            .context("Failed to run rustfmt")?;

        if !status.success() {
            return Err(anyhow::anyhow!("rustfmt failed"));
        }

        Ok(())
    }

    /// Add basic documentation to a file
    fn apply_documentation_fix(&self, file: &str) -> Result<()> {
        // This is a simplified example - in reality, this would need to parse
        // the file and add appropriate documentation
        self.output.info(&format!(
            "Documentation fix for {} requires manual intervention",
            file
        ));
        Ok(())
    }

    /// Remove unused imports from a file
    fn apply_unused_imports_fix(&self, file: &str) -> Result<()> {
        // Run rustfix or cargo fix for unused imports
        let status = std::process::Command::new("cargo")
            .args(&["fix", "--allow-dirty", "--broken-code", "--", file])
            .status()
            .context("Failed to run cargo fix")?;

        if !status.success() {
            return Err(anyhow::anyhow!("cargo fix failed"));
        }

        Ok(())
    }

    /// Apply clippy suggestions
    fn apply_clippy_fix(&self, file: &str, suggestion: &str) -> Result<()> {
        // Run clippy with --fix
        let status = std::process::Command::new("cargo")
            .args(&["clippy", "--fix", "--allow-dirty", "--", "-W", suggestion])
            .status()
            .context("Failed to run cargo clippy --fix")?;

        if !status.success() {
            return Err(anyhow::anyhow!("cargo clippy --fix failed"));
        }

        Ok(())
    }
}

/// Types of auto-fixes available
#[derive(Debug, Clone)]
enum AutoFixType {
    Format,
    AddDocumentation,
    RemoveUnusedImports,
    ClippyFix(String),
}

/// Individual auto-fix description
#[derive(Debug, Clone)]
struct AutoFix {
    description: String,
    fix_type: AutoFixType,
    target: String,
}

/// Result of auto-fix operations
#[derive(Debug, Default)]
pub struct AutoFixResult {
    pub successful_fixes: usize,
    pub failed_fixes: usize,
}

impl AutoFixResult {
    pub fn has_fixes(&self) -> bool {
        self.successful_fixes > 0
    }

    pub fn all_successful(&self) -> bool {
        self.failed_fixes == 0
    }
}

/// Helper function to check if auto-fix is available for a command
pub fn supports_autofix(command: &str) -> bool {
    matches!(command, "check" | "build" | "test" | "clippy")
}

/// Apply common project-wide fixes
pub async fn apply_project_fixes(
    workspace_root: &Path,
    output: &OutputManager,
    dry_run: bool,
) -> Result<()> {
    output.header("Applying project-wide fixes");

    // Format all Rust files
    output.progress("Running rustfmt on all files...");
    if !dry_run {
        let status = std::process::Command::new("cargo")
            .arg("fmt")
            .current_dir(workspace_root)
            .status()
            .context("Failed to run cargo fmt")?;

        if status.success() {
            output.success("Code formatting completed");
        } else {
            output.warning("Some files could not be formatted");
        }
    } else {
        output.info("[DRY RUN] Would run: cargo fmt");
    }

    // Fix common clippy warnings
    output.progress("Applying clippy suggestions...");
    if !dry_run {
        let status = std::process::Command::new("cargo")
            .args(&["clippy", "--fix", "--allow-dirty", "--all-targets"])
            .current_dir(workspace_root)
            .status()
            .context("Failed to run cargo clippy --fix")?;

        if status.success() {
            output.success("Clippy fixes applied");
        } else {
            output.warning("Some clippy fixes could not be applied");
        }
    } else {
        output.info("[DRY RUN] Would run: cargo clippy --fix");
    }

    // Update dependencies
    output.progress("Checking for outdated dependencies...");
    if !dry_run {
        // Check if cargo-outdated is installed
        if which::which("cargo-outdated").is_ok() {
            let output_str = std::process::Command::new("cargo")
                .arg("outdated")
                .current_dir(workspace_root)
                .output()
                .context("Failed to run cargo outdated")?;

            if !output_str.stdout.is_empty() {
                output.info("Outdated dependencies found. Run 'cargo update' to update.");
            }
        }
    } else {
        output.info("[DRY RUN] Would check: cargo outdated");
    }

    Ok(())
}
