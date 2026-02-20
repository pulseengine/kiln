//! WebAssembly section handling.
//!
//! This module provides types and utilities for working with WebAssembly
//! sections.

// Import collection types
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    string::String,
    vec::Vec,
};

// Import the prelude for conditional imports
#[cfg(not(any(feature = "std")))]
use wrt_foundation::{
    traits::BoundedCapacity,
    MemoryProvider,
    NoStdProvider,
};

#[cfg(not(any(feature = "std")))]
use crate::WasmVec;

/// WebAssembly section ID constants
pub const CUSTOM_ID: u8 = 0;
pub const TYPE_ID: u8 = 1;
pub const IMPORT_ID: u8 = 2;
pub const FUNCTION_ID: u8 = 3;
pub const TABLE_ID: u8 = 4;
pub const MEMORY_ID: u8 = 5;
pub const GLOBAL_ID: u8 = 6;
pub const EXPORT_ID: u8 = 7;
pub const START_ID: u8 = 8;
pub const ELEMENT_ID: u8 = 9;
pub const CODE_ID: u8 = 10;
pub const DATA_ID: u8 = 11;
pub const DATA_COUNT_ID: u8 = 12;

/// WebAssembly section identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    /// Custom section
    Custom    = 0,
    /// Type section
    Type      = 1,
    /// Import section
    Import    = 2,
    /// Function section
    Function  = 3,
    /// Table section
    Table     = 4,
    /// Memory section
    Memory    = 5,
    /// Global section
    Global    = 6,
    /// Export section
    Export    = 7,
    /// Start section
    Start     = 8,
    /// Element section
    Element   = 9,
    /// Code section
    Code      = 10,
    /// Data section
    Data      = 11,
    /// Data count section
    DataCount = 12,
}

impl SectionId {
    /// Convert a u8 to a SectionId
    pub fn from_u8(id: u8) -> Option<Self> {
        match id {
            0 => Some(SectionId::Custom),
            1 => Some(SectionId::Type),
            2 => Some(SectionId::Import),
            3 => Some(SectionId::Function),
            4 => Some(SectionId::Table),
            5 => Some(SectionId::Memory),
            6 => Some(SectionId::Global),
            7 => Some(SectionId::Export),
            8 => Some(SectionId::Start),
            9 => Some(SectionId::Element),
            10 => Some(SectionId::Code),
            11 => Some(SectionId::Data),
            12 => Some(SectionId::DataCount),
            _ => None,
        }
    }
}

/// WebAssembly section
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub enum Section {
    /// Custom section
    Custom(CustomSection),
    /// Type section
    Type(Vec<u8>),
    /// Import section
    Import(Vec<u8>),
    /// Function section
    Function(Vec<u8>),
    /// Table section
    Table(Vec<u8>),
    /// Memory section
    Memory(Vec<u8>),
    /// Global section
    Global(Vec<u8>),
    /// Export section
    Export(Vec<u8>),
    /// Start section
    Start(Vec<u8>),
    /// Element section
    Element(Vec<u8>),
    /// Code section
    Code(Vec<u8>),
    /// Data section
    Data(Vec<u8>),
    /// Data count section
    DataCount(Vec<u8>),
}

/// WebAssembly section (clean architecture version)
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone)]
pub enum Section {
    /// Custom section
    Custom(CustomSection),
    /// Type section
    Type(alloc::vec::Vec<u8>),
    /// Import section
    Import(alloc::vec::Vec<u8>),
    /// Function section
    Function(alloc::vec::Vec<u8>),
    /// Table section
    Table(alloc::vec::Vec<u8>),
    /// Memory section
    Memory(alloc::vec::Vec<u8>),
    /// Global section
    Global(alloc::vec::Vec<u8>),
    /// Export section
    Export(alloc::vec::Vec<u8>),
    /// Start section
    Start(alloc::vec::Vec<u8>),
    /// Element section
    Element(alloc::vec::Vec<u8>),
    /// Code section
    Code(alloc::vec::Vec<u8>),
    /// Data section
    Data(alloc::vec::Vec<u8>),
    /// Data count section
    DataCount(alloc::vec::Vec<u8>),
}

/// WebAssembly custom section - Clean architecture version
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSection {
    /// Section name
    pub name: alloc::string::String,
    /// Section data
    pub data: alloc::vec::Vec<u8>,
}

#[cfg(not(feature = "std"))]
impl Default for CustomSection {
    fn default() -> Self {
        Self {
            name: alloc::string::String::new(),
            data: alloc::vec::Vec::new(),
        }
    }
}

// Implement Checksummable for CustomSection - clean version
#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for CustomSection {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(self.name.as_bytes());
        checksum.update_slice(&self.data);
    }
}

// Implement ToBytes for CustomSection - clean version
#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for CustomSection {
    fn serialized_size(&self) -> usize {
        self.name.len() + self.data.len() + 8 // 4 bytes each for lengths
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        stream: &mut wrt_foundation::traits::WriteStream,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write name length and name
        stream.write_all(&(self.name.len() as u32).to_le_bytes())?;
        stream.write_all(self.name.as_bytes())?;
        // Write data length and data
        stream.write_all(&(self.data.len() as u32).to_le_bytes())?;
        stream.write_all(&self.data)?;
        Ok(())
    }
}

// Implement FromBytes for CustomSection - no_std version
#[cfg(not(any(feature = "std")))]
impl wrt_foundation::traits::FromBytes for CustomSection {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read name length
        let name_len = reader.read_u32_le()?;
        let mut name_bytes = alloc::vec![0u8; name_len as usize];
        reader.read_exact(&mut name_bytes)?;
        let name = alloc::string::String::from_utf8(name_bytes)
            .map_err(|_| wrt_error::Error::parse_error("Invalid UTF-8 in custom section name"))?;

        // Read data length
        let data_len = reader.read_u32_le()?;
        let mut data = alloc::vec![0u8; data_len as usize];
        reader.read_exact(&mut data)?;

        Ok(Self { name, data })
    }
}

/// WebAssembly custom section - With Allocation
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct CustomSection {
    /// Section name
    pub name: String,
    /// Section data
    pub data: Vec<u8>,
}

#[cfg(feature = "std")]
impl Default for CustomSection {
    fn default() -> Self {
        Self {
            name: String::new(),
            data: Vec::new(),
        }
    }
}

#[cfg(feature = "std")]
impl CustomSection {
    /// Create a new custom section
    pub fn new(name: String, data: Vec<u8>) -> Self {
        Self { name, data }
    }

    /// Create a new custom section from raw bytes
    pub fn from_bytes(name: String, data: &[u8]) -> Self {
        Self {
            name,
            data: data.to_vec(),
        }
    }

    /// Serialize the custom section to binary
    #[cfg(feature = "std")]
    pub fn to_binary(&self) -> core::result::Result<Vec<u8>, wrt_error::Error> {
        let mut section_data = Vec::new();

        // Add name as encoded string (name length + name bytes)
        let name_len = self.name.len() as u32;
        let name_len_bytes = crate::binary::with_alloc::write_leb128_u32(name_len);
        section_data.extend_from_slice(&name_len_bytes);
        section_data.extend_from_slice(self.name.as_bytes());

        // Add the section data
        section_data.extend_from_slice(&self.data);

        Ok(section_data)
    }

    /// Get access to the section data as a safe slice
    #[cfg(feature = "std")]
    pub fn get_data(&self) -> core::result::Result<&[u8], wrt_error::Error> {
        Ok(&self.data)
    }
}

#[cfg(not(any(feature = "std")))]
impl CustomSection {
    /// Create a new custom section
    pub fn new(name: alloc::string::String, data: alloc::vec::Vec<u8>) -> Self {
        Self { name, data }
    }

    /// Create a new custom section from raw bytes
    pub fn from_bytes(name: &str, data: &[u8]) -> Self {
        Self {
            name: alloc::string::String::from(name),
            data: data.to_vec(),
        }
    }

    /// Get section data length
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Copy section data to a slice
    pub fn copy_data_to_slice(&self, dest: &mut [u8]) -> usize {
        let copy_len = core::cmp::min(dest.len(), self.data.len());
        dest[..copy_len].copy_from_slice(&self.data[..copy_len]);
        copy_len
    }
}

/// Component section types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentSectionType {
    /// Custom section - for metadata, debug info, etc.
    Custom       = 0,
    /// Core module section - contains WebAssembly modules
    CoreModule   = 1,
    /// Core instance section - contains core WebAssembly instances
    CoreInstance = 2,
    /// Core type section - contains core WebAssembly types
    CoreType     = 3,
    /// Component section - contains nested components
    Component    = 4,
    /// Component instance section - contains component instances
    Instance     = 5,
    /// Alias section - contains aliases
    Alias        = 6,
    /// Type section - contains component types
    Type         = 7,
    /// Canonical section - contains canonical function conversions
    Canon        = 8,
    /// Start section - contains the component start function
    Start        = 9,
    /// Import section - contains imports
    Import       = 10,
    /// Export section - contains exports
    Export       = 11,
    /// Value section - contains component values
    Value        = 12,
}

impl ComponentSectionType {
    /// Get the section ID for this section type
    pub fn id(&self) -> u8 {
        *self as u8
    }

    /// Parse a section ID into a ComponentSectionType
    pub fn from_u8(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Custom),
            1 => Some(Self::CoreModule),
            2 => Some(Self::CoreInstance),
            3 => Some(Self::CoreType),
            4 => Some(Self::Component),
            5 => Some(Self::Instance),
            6 => Some(Self::Alias),
            7 => Some(Self::Type),
            8 => Some(Self::Canon),
            9 => Some(Self::Start),
            10 => Some(Self::Import),
            11 => Some(Self::Export),
            12 => Some(Self::Value),
            _ => None,
        }
    }
}

/// Header for component sections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSectionHeader {
    /// Section type
    pub section_type: ComponentSectionType,
    /// Section size in bytes (excluding the header)
    pub size:         u32,
}

/// Parse a component section header from binary data
pub fn parse_component_section_header(
    bytes: &[u8],
    pos: usize,
) -> wrt_error::Result<(ComponentSectionHeader, usize)> {
    if pos >= bytes.len() {
        return Err(wrt_error::Error::validation_error(
            "Unexpected end of input",
        ));
    }

    let section_id = bytes[pos];
    let section_type = ComponentSectionType::from_u8(section_id)
        .ok_or_else(|| wrt_error::Error::validation_error("Invalid section ID"))?;

    let (size, new_pos) = crate::binary::read_leb128_u32(bytes, pos + 1)?;

    let header = ComponentSectionHeader { section_type, size };

    Ok((header, new_pos))
}

/// Write a component section header to binary
#[cfg(feature = "std")]
pub fn write_component_section_header(
    section_type: ComponentSectionType,
    content_size: u32,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(section_type.id());
    bytes.extend_from_slice(&crate::binary::with_alloc::write_leb128_u32(content_size));
    bytes
}

/// Format a component section with content
#[cfg(feature = "std")]
pub fn format_component_section<F>(section_type: ComponentSectionType, content_fn: F) -> Vec<u8>
where
    F: FnOnce() -> Vec<u8>,
{
    let content = content_fn();
    let content_size = content.len() as u32;

    let mut result = write_component_section_header(section_type, content_size);
    result.extend_from_slice(&content);

    result
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::{
        string::ToString,
        vec,
    };
    #[cfg(feature = "std")]
    use std::string::ToString;
    #[cfg(feature = "std")]
    use std::vec;

    use wrt_foundation::safe_memory::SafeSlice;

    use super::*;

    #[test]
    fn test_section_id_conversion() {
        assert_eq!(SectionId::from_u8(0), Some(SectionId::Custom));
        assert_eq!(SectionId::from_u8(1), Some(SectionId::Type));
        assert_eq!(SectionId::from_u8(2), Some(SectionId::Import));
        assert_eq!(SectionId::from_u8(3), Some(SectionId::Function));
        assert_eq!(SectionId::from_u8(4), Some(SectionId::Table));
        assert_eq!(SectionId::from_u8(5), Some(SectionId::Memory));
        assert_eq!(SectionId::from_u8(6), Some(SectionId::Global));
        assert_eq!(SectionId::from_u8(7), Some(SectionId::Export));
        assert_eq!(SectionId::from_u8(8), Some(SectionId::Start));
        assert_eq!(SectionId::from_u8(9), Some(SectionId::Element));
        assert_eq!(SectionId::from_u8(10), Some(SectionId::Code));
        assert_eq!(SectionId::from_u8(11), Some(SectionId::Data));
        assert_eq!(SectionId::from_u8(12), Some(SectionId::DataCount));
        assert_eq!(SectionId::from_u8(13), None);
    }

    #[test]
    fn test_component_section_type_conversion() {
        assert_eq!(
            ComponentSectionType::from_u8(0),
            Some(ComponentSectionType::Custom)
        );
        assert_eq!(
            ComponentSectionType::from_u8(1),
            Some(ComponentSectionType::CoreModule)
        );
        assert_eq!(
            ComponentSectionType::from_u8(2),
            Some(ComponentSectionType::CoreInstance)
        );
        assert_eq!(
            ComponentSectionType::from_u8(3),
            Some(ComponentSectionType::CoreType)
        );
        assert_eq!(
            ComponentSectionType::from_u8(4),
            Some(ComponentSectionType::Component)
        );
        assert_eq!(
            ComponentSectionType::from_u8(5),
            Some(ComponentSectionType::Instance)
        );
        assert_eq!(
            ComponentSectionType::from_u8(6),
            Some(ComponentSectionType::Alias)
        );
        assert_eq!(
            ComponentSectionType::from_u8(7),
            Some(ComponentSectionType::Type)
        );
        assert_eq!(
            ComponentSectionType::from_u8(8),
            Some(ComponentSectionType::Canon)
        );
        assert_eq!(
            ComponentSectionType::from_u8(9),
            Some(ComponentSectionType::Start)
        );
        assert_eq!(
            ComponentSectionType::from_u8(10),
            Some(ComponentSectionType::Import)
        );
        assert_eq!(
            ComponentSectionType::from_u8(11),
            Some(ComponentSectionType::Export)
        );
        assert_eq!(
            ComponentSectionType::from_u8(12),
            Some(ComponentSectionType::Value)
        );
        assert_eq!(ComponentSectionType::from_u8(13), None);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_custom_section_serialization() {
        let test_data = vec![1, 2, 3, 4];
        let section = CustomSection::new("test-section".to_string(), test_data.clone());

        // Serialize
        let binary = section.to_binary().unwrap();

        // Check that the binary data contains the section name
        let name_bytes = "test-section".as_bytes();
        let mut found_name = false;

        for i in 0..binary.len() - name_bytes.len() + 1 {
            if &binary[i..i + name_bytes.len()] == name_bytes {
                found_name = true;
                break;
            }
        }

        assert!(found_name, "Section name not found in binary");

        // Check that the binary data contains our test data
        let mut found_data = false;
        for i in 0..binary.len() - test_data.len() + 1 {
            if &binary[i..i + test_data.len()] == test_data {
                found_data = true;
                break;
            }
        }

        assert!(found_data, "Test data not found in binary");
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_custom_section_data_access() {
        let test_data = vec![1, 2, 3, 4];
        let section = CustomSection::new("test-section".to_string(), test_data);

        // Create a safe slice
        let safe_slice = SafeSlice::new(&section.data).unwrap();

        // Get the data
        let data = safe_slice.data().unwrap();

        // Check it matches our test data
        assert_eq!(data, &[1, 2, 3, 4]);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_component_section_header() {
        // Create a binary section header
        let header_bytes = write_component_section_header(ComponentSectionType::CoreModule, 42);

        // Check the header starts with the correct ID
        assert_eq!(header_bytes[0], ComponentSectionType::CoreModule as u8);

        // Parse the header
        let (header, _) = parse_component_section_header(&header_bytes, 0).unwrap();

        // Check the parsed values match what we wrote
        assert_eq!(header.section_type, ComponentSectionType::CoreModule);
        assert_eq!(header.size, 42);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_format_component_section() {
        // Create a section with some content
        let section_content = vec![1, 2, 3, 4, 5];
        let section_bytes =
            format_component_section(ComponentSectionType::CoreModule, || section_content.clone());

        // Parse the section header
        let (header, content_pos) = parse_component_section_header(&section_bytes, 0).unwrap();

        // Check the header values
        assert_eq!(header.section_type, ComponentSectionType::CoreModule);
        assert_eq!(header.size, 5); // Length of our content

        // Get the actual content bytes
        let content_slice = &section_bytes[content_pos..content_pos + header.size as usize];

        // Check individually to avoid ordering issues
        assert_eq!(content_slice.len(), section_content.len());
        for i in 0..section_content.len() {
            assert!(content_slice.contains(&section_content[i]));
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_invalid_component_section_id() {
        // Create an invalid section ID
        let mut header_bytes = Vec::new();
        header_bytes.push(255); // Invalid section ID
                                // Use a manual LEB128 encoding for 42
        header_bytes.push(42); // 42 fits in one byte for LEB128

        // Parse should fail
        let result = parse_component_section_header(&header_bytes, 0);
        assert!(result.is_err());
    }
}
