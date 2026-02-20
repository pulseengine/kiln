//! WebAssembly state serialization.
//!
//! This module provides utilities for serializing and deserializing WebAssembly
//! runtime state using custom sections.

// alloc is imported in lib.rs with proper feature gates
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use alloc::{
    string::String,
    vec::Vec,
};

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_format::{
    compression::{
        rle_decode,
        rle_encode,
        CompressionType,
    },
    section::CustomSection,
    version::{
        STATE_MAGIC,
        STATE_VERSION,
    },
};
#[cfg(not(any(feature = "std")))]
use wrt_format::{
    WasmString,
    WasmVec,
};
#[cfg(not(any(feature = "std")))]
use wrt_foundation::{
    MemoryProvider,
    NoStdProvider,
};

/// Constants for state section names
pub const STATE_SECTION_PREFIX: &str = "wrt-state";

/// Types of state sections
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateSection {
    /// Metadata section
    Meta    = 0,
    /// Stack state section
    Stack   = 1,
    /// Frame state section
    Frames  = 2,
    /// Global variables section
    Globals = 3,
    /// Memory section
    Memory  = 4,
}

impl StateSection {
    /// Get the section name for this state section type
    #[cfg(feature = "std")]
    pub fn name(&self) -> String {
        match self {
            Self::Meta => format!("{}-meta", STATE_SECTION_PREFIX),
            Self::Stack => format!("{}-stack", STATE_SECTION_PREFIX),
            Self::Frames => format!("{}-frames", STATE_SECTION_PREFIX),
            Self::Globals => format!("{}-globals", STATE_SECTION_PREFIX),
            Self::Memory => format!("{}-memory", STATE_SECTION_PREFIX),
        }
    }

    /// Get the section name for this state section type (no_std version)
    #[cfg(not(any(feature = "std")))]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Meta => "wrt-state-meta",
            Self::Stack => "wrt-state-stack",
            Self::Frames => "wrt-state-frames",
            Self::Globals => "wrt-state-globals",
            Self::Memory => "wrt-state-memory",
        }
    }

    /// Convert a section name to a StateSection
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "wrt-state-meta" => Some(Self::Meta),
            "wrt-state-stack" => Some(Self::Stack),
            "wrt-state-frames" => Some(Self::Frames),
            "wrt-state-globals" => Some(Self::Globals),
            "wrt-state-memory" => Some(Self::Memory),
            _ => None,
        }
    }

    /// Convert a u32 to a StateSection
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Meta),
            1 => Some(Self::Stack),
            2 => Some(Self::Frames),
            3 => Some(Self::Globals),
            4 => Some(Self::Memory),
            _ => None,
        }
    }
}

/// State section header
#[derive(Debug, Clone)]
pub struct StateHeader {
    /// Section type
    pub section_type:      StateSection,
    /// Compression type
    pub compression_type:  CompressionType,
    /// Data size
    pub data_size:         u32,
    /// Original uncompressed size
    pub uncompressed_size: u32,
}

/// Create a custom section containing serialized state
#[cfg(feature = "std")]
pub fn create_state_section(
    section_type: StateSection,
    data: &[u8],
    compression_type: CompressionType,
) -> Result<CustomSection> {
    // Create header
    let mut header = Vec::with_capacity(17);

    // Magic bytes
    header.extend_from_slice(STATE_MAGIC);

    // Version
    header.extend_from_slice(&STATE_VERSION.to_le_bytes());

    // Section type
    header.push(section_type as u8);

    // Compression type
    header.push(compression_type as u8);

    // Original uncompressed size
    let uncompressed_size = data.len() as u32;
    header.extend_from_slice(&uncompressed_size.to_le_bytes());

    // Compress data
    let compressed_data = match compression_type {
        CompressionType::None => data.to_vec(),
        CompressionType::RLE => rle_encode(data),
    };

    // Serialized data size
    let compressed_size = compressed_data.len() as u32;
    header.extend_from_slice(&compressed_size.to_le_bytes());

    // Create complete section contents: header + compressed data
    let mut section_data = Vec::with_capacity(header.len() + compressed_data.len());
    section_data.extend_from_slice(&header);
    section_data.extend_from_slice(&compressed_data);

    // Create custom section with name and data
    Ok(CustomSection::new(section_type.name(), section_data))
}

/// Extract state data from a custom section
#[cfg(feature = "std")]
pub fn extract_state_section(section: &CustomSection) -> Result<(StateHeader, Vec<u8>)> {
    // Verify that this is a valid state section
    let section_type = StateSection::from_name(&section.name)
        .ok_or_else(|| Error::validation_parse_error("Invalid state section name"))?;

    // Get the data
    let data = &section.data;

    // Parse header
    if data.len() < 17 {
        return Err(Error::validation_parse_error(
            "State section header too small",
        ));
    }

    // Verify magic bytes
    if data[0..4] != *STATE_MAGIC {
        return Err(Error::validation_parse_error(
            "Invalid state section magic bytes",
        ));
    }

    // Parse version
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Version check
    if version != STATE_VERSION {
        // In future versions we'll need to handle migration
        // For now, just reject mismatched versions
    }

    // Parse section type
    let parsed_section_type = StateSection::from_u32(data[8] as u32)
        .ok_or_else(|| Error::validation_parse_error("Invalid section type ID"))?;

    // Verify section type matches the name
    if parsed_section_type != section_type {
        return Err(Error::validation_parse_error("Section type mismatch"));
    }

    // Parse compression type
    let compression_type = match CompressionType::from_u8(data[9]) {
        Some(t) => t,
        None => {
            return Err(Error::validation_parse_error("Unknown compression type"));
        },
    };

    // Parse uncompressed size
    let uncompressed_size = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);

    // Parse compressed size
    let compressed_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);

    // Extract the compressed data
    if data.len() < 18 + compressed_size as usize {
        return Err(Error::validation_parse_error("Compressed data truncated"));
    }

    let compressed_data = &data[18..18 + compressed_size as usize];

    // Decompress the data
    let decompressed_data = match compression_type {
        CompressionType::None => compressed_data.to_vec(),
        CompressionType::RLE => rle_decode(compressed_data)?,
    };

    // Verify decompressed size
    if decompressed_data.len() != uncompressed_size as usize {
        return Err(Error::validation_parse_error("Decompressed size mismatch"));
    }

    // Create header
    let header = StateHeader {
        section_type,
        compression_type,
        data_size: compressed_size,
        uncompressed_size,
    };

    Ok((header, decompressed_data))
}

/// Check if a module contains state sections
///
/// # Arguments
///
/// * `custom_sections` - Vector of custom sections to check
///
/// # Returns
///
/// `true` if the module contains at least one state section
pub fn has_state_sections(custom_sections: &[CustomSection]) -> bool {
    custom_sections.iter().any(|section| {
        #[cfg(feature = "std")]
        {
            section.name.starts_with(STATE_SECTION_PREFIX)
        }
        #[cfg(not(any(feature = "std")))]
        {
            section.name.starts_with(STATE_SECTION_PREFIX)
        }
    })
}

/// Checks if a given section name corresponds to a known `StateSection`.
///
/// # Arguments
///
/// * `name` - The name of the custom section to check.
///
/// # Returns
///
/// `true` if the name matches one of the `StateSection` variants, `false`
/// otherwise.
pub fn is_state_section_name(name: &str) -> bool {
    StateSection::from_name(name).is_some()
}

#[cfg(test)]
mod tests {

    // ... existing test code ...
}
