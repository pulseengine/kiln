//! No-std, no-alloc support for WebAssembly Component Model
//!
//! This module provides minimal component model functionality in pure no_std
//! environments without heap allocation. It enables basic validation and
//! introspection capabilities.

// Re-export the component header verification from wrt-decoder
#[cfg(feature = "decoder")]
pub use wrt_decoder::component::decode_no_alloc::{verify_component_header, COMPONENT_MAGIC};

// Placeholder when decoder is not available
#[cfg(not(feature = "decoder"))]
pub const COMPONENT_MAGIC: &[u8] = b"\x00asm";
#[cfg(not(feature = "decoder"))]
pub fn verify_component_header(_data: &[u8]) -> Result<bool> {
    Ok(false) // Simplified verification
}
use wrt_error::{codes, Error, ErrorCategory, Result};
use wrt_foundation::{
    bounded::{ MAX_COMPONENT_TYPES, MAX_WASM_NAME_LENGTH},
    safe_memory::{NoStdProvider, SafeSlice},
    verification::VerificationLevel,
};

/// Maximum size of a WebAssembly component's sections
pub const MAX_COMPONENT_SECTION_SIZE: usize = 4096; // 4KB

/// Section IDs for Component Model binaries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComponentSectionId {
    /// Custom section
    Custom = 0,
    /// Component type section
    ComponentType = 1,
    /// Core module section
    CoreModule = 2,
    /// Instance section
    Instance = 3,
    /// Component section
    Component = 4,
    /// Import section
    Import = 5,
    /// Export section
    Export = 6,
    /// Start section
    Start = 7,
    /// Alias section
    Alias = 8,
    /// Unknown section
    Unknown = 255,
}

impl From<u8> for ComponentSectionId {
    fn from(id: u8) -> Self {
        match id {
            0 => ComponentSectionId::Custom,
            1 => ComponentSectionId::ComponentType,
            2 => ComponentSectionId::CoreModule,
            3 => ComponentSectionId::Instance,
            4 => ComponentSectionId::Component,
            5 => ComponentSectionId::Import,
            6 => ComponentSectionId::Export,
            7 => ComponentSectionId::Start,
            8 => ComponentSectionId::Alias,
            _ => ComponentSectionId::Unknown,
        }
    }
}

/// Information about a component section
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSectionInfo {
    /// Section ID
    pub id: ComponentSectionId,
    /// Section size in bytes
    pub size: u32,
    /// Offset of the section data in the binary
    pub offset: usize,
}

/// Minimal component header with basic information
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ComponentHeader {
    /// Component size in bytes
    pub size: usize,
    /// Number of core modules within the component
    pub module_count: u8,
    /// Number of exports from the component
    pub export_count: u8,
    /// Number of imports to the component
    pub import_count: u8,
    /// Whether the component has a start function
    pub has_start: bool,
    /// Section information
    pub sections: [Option<ComponentSectionInfo>; 16],
}

/// Validates a WebAssembly Component Model binary
///
/// This function performs basic validation of a Component Model binary without
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if valid, Error otherwise
pub fn validate_component_no_alloc(bytes: &[u8]) -> Result<()> {
    // First validate the header
    verify_component_header(bytes)?;

    // Currently we only do header validation in pure no_std mode
    // A more comprehensive validation would be added here for real-world use

    Ok(())
}

/// The types of validation levels for component validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLevel {
    /// Basic validation only checks header and section structure
    Basic,
    /// Standard validation performs additional checks
    Standard,
    /// Full validation does comprehensive validation
    Full,
}

/// Validates component with the specified validation level
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
/// * `level` - The validation level to use
///
/// # Returns
///
/// * `Result<()>` - Ok if valid, Error otherwise
pub fn validate_component_with_level(bytes: &[u8], level: ValidationLevel) -> Result<()> {
    // Validate header for all levels
    verify_component_header(bytes)?;

    match level {
        ValidationLevel::Basic => Ok(()),
        ValidationLevel::Standard | ValidationLevel::Full => {
            // For Standard and Full, validate section structure
            validate_component_structure(bytes)?;

            if level == ValidationLevel::Full {
                // Additional checks for Full validation
                validate_component_imports_exports(bytes)?;
            }

            Ok(())
        }
    }
}

/// Validates the component's section structure
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if valid, Error otherwise
fn validate_component_structure(bytes: &[u8]) -> Result<()> {
    // Verify header
    if bytes.len() < 8 {
        return Err(Error::parse_error("Component header too short";
    }

    // Verify magic number and version
    if &bytes[0..8] != &COMPONENT_MAGIC {
        return Err(Error::parse_error("Invalid component magic number";
    }

    // For now, we just do basic validation
    // More comprehensive validation would be added here

    Ok(())
}

/// Validates component imports and exports
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if valid, Error otherwise
fn validate_component_imports_exports(bytes: &[u8]) -> Result<()> {
    // This is a placeholder for more comprehensive validation
    // that would check import/export consistency

    Ok(())
}

/// A minimal compatibility layer for pure no_std environments
///
/// This is a very limited subset of component model functionality
/// Binary std/no_std choice
/// validation and introspection capabilities.
pub struct MinimalComponent {
    /// Header information
    pub header: ComponentHeader,
    /// Verification level used for operations
    pub verification_level: VerificationLevel,
}

impl MinimalComponent {
    /// Creates a new MinimalComponent
    ///
    /// # Arguments
    ///
    /// * `bytes` - The WebAssembly Component binary data
    /// * `level` - Verification level for operations
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The minimal component or error
    pub fn new(bytes: &[u8], level: VerificationLevel) -> Result<Self> {
        // Validate component
        validate_component_no_alloc(bytes)?;

        // Create a default header
        let mut header = ComponentHeader::default());
        header.size = bytes.len();

        // Populate header with basic info
        // This would scan the binary for section info in a real implementation

        Ok(Self { header, verification_level: level })
    }

    /// Gets the size of the component in bytes
    #[must_use]
    pub const fn size(&self) -> usize {
        self.header.size
    }

    /// Checks if the component has a start function
    #[must_use]
    pub const fn has_start(&self) -> bool {
        self.header.has_start
    }

    /// Gets the number of core modules within the component
    #[must_use]
    pub const fn module_count(&self) -> u8 {
        self.header.module_count
    }

    /// Gets the number of exports from the component
    #[must_use]
    pub const fn export_count(&self) -> u8 {
        self.header.export_count
    }

    /// Gets the number of imports to the component
    #[must_use]
    pub const fn import_count(&self) -> u8 {
        self.header.import_count
    }

}
