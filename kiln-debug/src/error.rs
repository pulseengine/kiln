/// Error types for DWARF debug information parsing
use wrt_error::Error;

/// Debug-specific error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugError {
    /// Invalid or corrupted DWARF data
    InvalidData,
    /// Unexpected end of data
    UnexpectedEof,
    /// Unsupported DWARF version
    UnsupportedVersion(u16),
    /// Invalid abbreviation code
    InvalidAbbreviation(u64),
    /// String table access error
    StringError,
}

/// Result type for debug operations
#[allow(dead_code)]
pub type DebugResult<T> = Result<T, DebugError>;

impl From<wrt_error::Error> for DebugError {
    fn from(_err: wrt_error::Error) -> Self {
        // For simplicity, map all WRT errors to InvalidData
        // A more sophisticated implementation could map specific error categories
        DebugError::InvalidData
    }
}

impl From<DebugError> for wrt_error::Error {
    fn from(err: DebugError) -> Self {
        match err {
            DebugError::InvalidData => Error::parse_error("Invalid DWARF data"),
            DebugError::UnexpectedEof => Error::parse_error("Unexpected end of DWARF data"),
            DebugError::UnsupportedVersion(_version) => {
                Error::validation_unsupported_feature("Unsupported DWARF version")
            },
            DebugError::InvalidAbbreviation(_code) => {
                Error::parse_error("Invalid abbreviation code")
            },
            DebugError::StringError => Error::parse_error("String table access error"),
        }
    }
}
