// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! DWARF abbreviation table parsing

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_ABBREV_CACHE},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::bounded_debug_infra::{DEBUG_PROVIDER_SIZE, DebugProvider};

use crate::{bounded_debug_infra, cursor::DwarfCursor};
/// DWARF attribute form constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeForm {
    Addr,
    Block2,
    Block4,
    Data2,
    Data4,
    Data8,
    String,
    Block,
    Block1,
    Data1,
    Flag,
    Sdata,
    Strp,
    Udata,
    RefAddr,
    Ref1,
    Ref2,
    Ref4,
    Ref8,
    RefUdata,
    Indirect,
    SecOffset,
    Exprloc,
    FlagPresent,
    RefSig8,
    Unknown(u16),
}

impl AttributeForm {
    /// Parse an attribute form from its encoded value
    pub fn from_u16(value: u16) -> Self {
        match value {
            0x01 => Self::Addr,
            0x03 => Self::Block2,
            0x04 => Self::Block4,
            0x05 => Self::Data2,
            0x06 => Self::Data4,
            0x07 => Self::Data8,
            0x08 => Self::String,
            0x09 => Self::Block,
            0x0a => Self::Block1,
            0x0b => Self::Data1,
            0x0c => Self::Flag,
            0x0d => Self::Sdata,
            0x0e => Self::Strp,
            0x0f => Self::Udata,
            0x10 => Self::RefAddr,
            0x11 => Self::Ref1,
            0x12 => Self::Ref2,
            0x13 => Self::Ref4,
            0x14 => Self::Ref8,
            0x15 => Self::RefUdata,
            0x16 => Self::Indirect,
            0x17 => Self::SecOffset,
            0x18 => Self::Exprloc,
            0x19 => Self::FlagPresent,
            0x20 => Self::RefSig8,
            _ => Self::Unknown(value),
        }
    }
}

/// DWARF attribute specification
#[derive(Debug, Clone, Copy)]
pub struct AttributeSpec {
    /// Attribute name/type
    pub name: u16,
    /// Attribute form
    pub form: AttributeForm,
}

/// DWARF abbreviation entry
#[derive(Debug, Clone)]
pub struct Abbreviation {
    /// Abbreviation code
    pub code: u32,
    /// DIE tag
    pub tag: u16,
    /// Has children flag
    pub has_children: bool,
    /// Attribute specifications
    pub attributes: BoundedVec<AttributeSpec, 32, crate::bounded_debug_infra::DebugProvider>,
}

/// DWARF abbreviation table
pub struct AbbreviationTable {
    /// Cached abbreviations
    entries:
        BoundedVec<Abbreviation, MAX_DWARF_ABBREV_CACHE, crate::bounded_debug_infra::DebugProvider>,
}

impl AbbreviationTable {
    /// Create a new abbreviation table
    pub fn new() -> Result<Self> {
        let provider = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
        Ok(Self {
            entries: BoundedVec::new(provider)
                .map_err(|_| Error::memory_error("Failed to create abbreviation entries"))?,
        })
    }

    /// Parse abbreviations from data
    pub fn parse(&mut self, data: &[u8]) -> Result<()> {
        let mut cursor = DwarfCursor::new(data);

        self.entries.clear();

        while !cursor.is_at_end() {
            // Read abbreviation code
            let code = cursor.read_uleb128_u32()?;
            if code == 0 {
                // Null entry marks end of abbreviations
                break;
            }

            // Read tag
            let tag = cursor.read_uleb128()? as u16;

            // Read has_children flag
            let has_children = cursor.read_u8()? != 0;

            // Read attributes
            let attr_provider = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
            let mut attributes = BoundedVec::new(attr_provider)
                .map_err(|_| Error::memory_error("Failed to create attributes vector"))?;

            loop {
                let name = cursor.read_uleb128()? as u16;
                let form = cursor.read_uleb128()? as u16;

                if name == 0 && form == 0 {
                    // Null attribute marks end of attributes
                    break;
                }

                let attr_spec = AttributeSpec {
                    name,
                    form: AttributeForm::from_u16(form),
                };

                attributes
                    .push(attr_spec)
                    .map_err(|_| Error::capacity_exceeded("Too many attributes in abbreviation"))?;
            }

            let abbrev = Abbreviation {
                code,
                tag,
                has_children,
                attributes,
            };

            self.entries
                .push(abbrev)
                .map_err(|_| Error::capacity_exceeded("Abbreviation cache full"))?;
        }

        Ok(())
    }

    /// Find an abbreviation by code
    pub fn find(&self, code: u32) -> Option<&Abbreviation> {
        self.entries.iter().find(|abbrev| abbrev.code == code)
    }

    /// Get the number of cached abbreviations
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Common DWARF tags
pub mod tags {
    pub const DW_TAG_COMPILE_UNIT: u16 = 0x11;
    pub const DW_TAG_SUBPROGRAM: u16 = 0x2e;
    pub const DW_TAG_VARIABLE: u16 = 0x34;
    pub const DW_TAG_FORMAL_PARAMETER: u16 = 0x05;
    pub const DW_TAG_LEXICAL_BLOCK: u16 = 0x0b;
    pub const DW_TAG_INLINED_SUBROUTINE: u16 = 0x1d;
    pub const DW_TAG_BASE_TYPE: u16 = 0x24;
    pub const DW_TAG_TYPEDEF: u16 = 0x16;
    pub const DW_TAG_STRUCTURE_TYPE: u16 = 0x13;
    pub const DW_TAG_UNION_TYPE: u16 = 0x17;
    pub const DW_TAG_ENUMERATION_TYPE: u16 = 0x04;
}

/// Common DWARF attributes
pub mod attributes {
    pub const DW_AT_NAME: u16 = 0x03;
    pub const DW_AT_STMT_LIST: u16 = 0x10;
    pub const DW_AT_LOW_PC: u16 = 0x11;
    pub const DW_AT_HIGH_PC: u16 = 0x12;
    pub const DW_AT_LANGUAGE: u16 = 0x13;
    pub const DW_AT_COMP_DIR: u16 = 0x1b;
    pub const DW_AT_TYPE: u16 = 0x49;
    pub const DW_AT_LOCATION: u16 = 0x02;
    pub const DW_AT_BYTE_SIZE: u16 = 0x0b;
    pub const DW_AT_DECL_FILE: u16 = 0x3a;
    pub const DW_AT_DECL_LINE: u16 = 0x3b;
    pub const DW_AT_LINKAGE_NAME: u16 = 0x6e;
    pub const DW_AT_ABSTRACT_ORIGIN: u16 = 0x31;
    pub const DW_AT_CALL_FILE: u16 = 0x58;
    pub const DW_AT_CALL_LINE: u16 = 0x59;
    pub const DW_AT_CALL_COLUMN: u16 = 0x57;
}
