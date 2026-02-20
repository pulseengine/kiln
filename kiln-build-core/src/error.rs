//! Error types for the WRT build system

use std::fmt;

/// Result type alias for build operations
pub type BuildResult<T> = Result<T, BuildError>;

/// Comprehensive error type for build system operations
#[derive(Debug)]
pub enum BuildError {
    /// IO operation failed
    Io(std::io::Error),
    /// Configuration error
    Config(String),
    /// Build process failed
    Build(String),
    /// Test execution failed
    Test(String),
    /// Verification failed
    Verification(String),
    /// Tool not found or failed to execute
    Tool(String),
    /// Workspace or path related error
    Workspace(String),
    /// Generic error with context
    Other(anyhow::Error),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Io(err) => write!(f, "IO error: {}", err),
            BuildError::Config(msg) => write!(f, "Configuration error: {}", msg),
            BuildError::Build(msg) => write!(f, "Build error: {}", msg),
            BuildError::Test(msg) => write!(f, "Test error: {}", msg),
            BuildError::Verification(msg) => write!(f, "Verification error: {}", msg),
            BuildError::Tool(msg) => write!(f, "Tool error: {}", msg),
            BuildError::Workspace(msg) => write!(f, "Workspace error: {}", msg),
            BuildError::Other(err) => write!(f, "Error: {}", err),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::Io(err) => Some(err),
            BuildError::Other(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BuildError {
    fn from(err: std::io::Error) -> Self {
        BuildError::Io(err)
    }
}

impl From<anyhow::Error> for BuildError {
    fn from(err: anyhow::Error) -> Self {
        BuildError::Other(err)
    }
}
