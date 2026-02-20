// WRT - wrt-foundation
// Module: WebAssembly Section Definitions
// SW-REQ-ID: REQ_018
// SW-REQ-ID: REQ_WASM_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Module representing WASM module sections with built-in
//! safety verification built in.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::SafeSlice;

/// WebAssembly section ID values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionId {
    /// Custom section (0)
    Custom    = 0,
    /// Type section (1)
    Type      = 1,
    /// Import section (2)
    Import    = 2,
    /// Function section (3)
    Function  = 3,
    /// Table section (4)
    Table     = 4,
    /// Memory section (5)
    Memory    = 5,
    /// Global section (6)
    Global    = 6,
    /// Export section (7)
    Export    = 7,
    /// Start section (8)
    Start     = 8,
    /// Element section (9)
    Element   = 9,
    /// Code section (10)
    Code      = 10,
    /// Data section (11)
    Data      = 11,
    /// Data count section (12)
    DataCount = 12,
}

impl SectionId {
    /// Convert from the WebAssembly binary format value
    pub fn from_binary(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Custom),
            1 => Ok(Self::Type),
            2 => Ok(Self::Import),
            3 => Ok(Self::Function),
            4 => Ok(Self::Table),
            5 => Ok(Self::Memory),
            6 => Ok(Self::Global),
            7 => Ok(Self::Export),
            8 => Ok(Self::Start),
            9 => Ok(Self::Element),
            10 => Ok(Self::Code),
            11 => Ok(Self::Data),
            12 => Ok(Self::DataCount),
            _ => Err(Error::validation_parse_error("Invalid section id")),
        }
    }
}

/// WebAssembly section type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    /// Custom section (0)
    Custom,
    /// Type section (1)
    Type,
    /// Import section (2)
    Import,
    /// Function section (3)
    Function,
    /// Table section (4)
    Table,
    /// Memory section (5)
    Memory,
    /// Global section (6)
    Global,
    /// Export section (7)
    Export,
    /// Start section (8)
    Start,
    /// Element section (9)
    Element,
    /// Code section (10)
    Code,
    /// Data section (11)
    Data,
    /// Data count section (12)
    DataCount,
}

impl From<SectionId> for SectionType {
    fn from(id: SectionId) -> Self {
        match id {
            SectionId::Custom => Self::Custom,
            SectionId::Type => Self::Type,
            SectionId::Import => Self::Import,
            SectionId::Function => Self::Function,
            SectionId::Table => Self::Table,
            SectionId::Memory => Self::Memory,
            SectionId::Global => Self::Global,
            SectionId::Export => Self::Export,
            SectionId::Start => Self::Start,
            SectionId::Element => Self::Element,
            SectionId::Code => Self::Code,
            SectionId::Data => Self::Data,
            SectionId::DataCount => Self::DataCount,
        }
    }
}

impl From<SectionType> for SectionId {
    fn from(ty: SectionType) -> Self {
        match ty {
            SectionType::Custom => Self::Custom,
            SectionType::Type => Self::Type,
            SectionType::Import => Self::Import,
            SectionType::Function => Self::Function,
            SectionType::Table => Self::Table,
            SectionType::Memory => Self::Memory,
            SectionType::Global => Self::Global,
            SectionType::Export => Self::Export,
            SectionType::Start => Self::Start,
            SectionType::Element => Self::Element,
            SectionType::Code => Self::Code,
            SectionType::Data => Self::Data,
            SectionType::DataCount => Self::DataCount,
        }
    }
}

/// WebAssembly section with safety verification
#[derive(Clone)]
pub struct Section<'a> {
    /// Section id
    pub id:     SectionId,
    /// Section data
    pub data:   SafeSlice<'a>,
    /// Size of the section in bytes
    pub size:   u32,
    /// Offset of the section in the module
    pub offset: u32,
}

impl<'a> Section<'a> {
    /// Create a new section
    #[must_use]
    pub fn new(id: SectionId, data: SafeSlice<'a>, size: u32, offset: u32) -> Self {
        Self {
            id,
            data,
            size,
            offset,
        }
    }

    /// Verify the section's integrity
    pub fn verify(&self) -> Result<()> {
        // Verify data integrity
        self.data.verify_integrity()?;

        // Verify size matches data length
        if self.size as usize != self.data.len() {
            return Err(Error::validation_error("Section size mismatch"));
        }
        Ok(())
    }
}
