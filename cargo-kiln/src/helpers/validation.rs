//! Input validation utilities
//!
//! Provides standardized validation functions for common input types
//! used across cargo-wrt commands with consistent error handling.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use wrt_build_core::config::AsilLevel;

/// Standard error type for validation failures
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: String },

    #[error("Invalid file extension: expected {expected}, got {actual}")]
    InvalidExtension { expected: String, actual: String },

    #[error("Invalid ASIL level: {level}")]
    InvalidAsilLevel { level: String },

    #[error("Invalid format: {format}")]
    InvalidFormat { format: String },

    #[error("Path is not valid UTF-8: {path}")]
    InvalidUtf8Path { path: String },
}

/// Standard error wrapper for consistent error handling
#[derive(Debug, thiserror::Error)]
#[error("{operation} failed: {source}")]
pub struct StandardError {
    operation: String,
    #[source]
    source: anyhow::Error,
}

impl StandardError {
    pub fn new(operation: impl Into<String>, source: anyhow::Error) -> Self {
        Self {
            operation: operation.into(),
            source,
        }
    }
}

/// Validate a file path exists and is readable
pub fn validate_file_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(ValidationError::FileNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    if !path.is_file() {
        return Err(ValidationError::DirectoryNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    Ok(path.to_path_buf())
}

/// Validate a directory path exists
pub fn validate_directory_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(ValidationError::DirectoryNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    if !path.is_dir() {
        return Err(ValidationError::FileNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    Ok(path.to_path_buf())
}

/// Validate file has expected extension
pub fn validate_file_extension(path: impl AsRef<Path>, expected_ext: &str) -> Result<PathBuf> {
    let path = validate_file_path(path)?;

    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            if ext_str.eq_ignore_ascii_case(expected_ext) {
                return Ok(path);
            }
        }
    }

    Err(ValidationError::InvalidExtension {
        expected: expected_ext.to_string(),
        actual: path.extension().and_then(|ext| ext.to_str()).unwrap_or("none").to_string(),
    }
    .into())
}

/// Validate ASIL level string
pub fn validate_asil_level(level: &str) -> Result<AsilLevel> {
    match level.to_uppercase().as_str() {
        "QM" => Ok(AsilLevel::QM),
        "A" => Ok(AsilLevel::A),
        "B" => Ok(AsilLevel::B),
        "C" => Ok(AsilLevel::C),
        "D" => Ok(AsilLevel::D),
        _ => Err(ValidationError::InvalidAsilLevel {
            level: level.to_string(),
        }
        .into()),
    }
}

/// Validate output format string
pub fn validate_output_format(format: &str) -> Result<()> {
    match format.to_lowercase().as_str() {
        "human" | "json" | "html" | "markdown" => Ok(()),
        _ => Err(ValidationError::InvalidFormat {
            format: format.to_string(),
        }
        .into()),
    }
}

/// Create a path that doesn't exist yet (useful for output files)
pub fn prepare_output_path(path: impl AsRef<Path>, force: bool) -> Result<PathBuf> {
    let path = path.as_ref();

    if path.exists() && !force {
        return Err(anyhow::anyhow!(
            "Output file already exists: {}. Use --force to overwrite",
            path.display()
        ));
    }

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    Ok(path.to_path_buf())
}

/// Validate and canonicalize a workspace path
pub fn validate_workspace_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = validate_directory_path(path)?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;
    Ok(canonical)
}

/// Check if path contains valid UTF-8
pub fn ensure_utf8_path(path: impl AsRef<Path>) -> Result<String> {
    path.as_ref()
        .to_str()
        .ok_or_else(|| ValidationError::InvalidUtf8Path {
            path: path.as_ref().display().to_string(),
        })
        .map(|s| s.to_string())
        .map_err(Into::into)
}
