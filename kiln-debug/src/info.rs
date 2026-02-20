// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! DWARF .debug_info section parsing

#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_FILE_TABLE},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::bounded_debug_infra::{DEBUG_PROVIDER_SIZE, DebugProvider};

use crate::{
    abbrev::{AbbreviationTable, AttributeForm, attributes, tags},
    bounded_debug_infra,
    cursor::DwarfCursor,
    parameter::{BasicType, InlinedFunction, InlinedFunctions, Parameter, ParameterList},
    strings::{DebugString, StringTable},
};

/// DWARF compilation unit header
#[derive(Debug)]
pub struct CompilationUnitHeader {
    /// Total length of the compilation unit
    pub unit_length: u32,
    /// DWARF version
    pub version: u16,
    /// Offset into .debug_abbrev section
    pub abbrev_offset: u32,
    /// Size of addresses (4 or 8 bytes)
    pub address_size: u8,
}

/// Simple function information
#[derive(Debug, Clone)]
pub struct FunctionInfo<'a> {
    /// Function name (reference to string in .debug_str)
    pub name: Option<DebugString<'a>>,
    /// Low PC (start address)
    pub low_pc: u32,
    /// High PC (end address or size)
    pub high_pc: u32,
    /// Source file index
    pub file_index: u16,
    /// Source line number
    pub line: u32,
    /// Function parameters
    pub parameters: Option<ParameterList<'a>>,
    /// Return type
    pub return_type: BasicType,
    /// Is this function inlined?
    pub is_inline: bool,
}

/// DWARF debug info parser
pub struct DebugInfoParser<'a> {
    /// Reference to .debug_info data
    debug_info: &'a [u8],
    /// Reference to .debug_abbrev data
    debug_abbrev: &'a [u8],
    /// Reference to .debug_str data (optional)
    debug_str: Option<&'a [u8]>,
    /// Abbreviation table
    abbrev_table: AbbreviationTable,
    /// String table for name resolution
    string_table: Option<StringTable<'a>>,
    /// Function cache
    functions: BoundedVec<
        FunctionInfo<'a>,
        MAX_DWARF_FILE_TABLE,
        crate::bounded_debug_infra::DebugProvider,
    >,
    /// Inlined functions
    inlined_functions: InlinedFunctions<'a>,
    /// Current compilation unit index
    current_cu: u32,
}

impl<'a> DebugInfoParser<'a> {
    /// Create a new debug info parser
    pub fn new(
        debug_info: &'a [u8],
        debug_abbrev: &'a [u8],
        debug_str: Option<&'a [u8]>,
    ) -> Result<Self> {
        let string_table = debug_str.map(|data| StringTable::new(data));
        let provider = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;

        Ok(Self {
            debug_info,
            debug_abbrev,
            debug_str,
            abbrev_table: AbbreviationTable::new()?,
            string_table,
            functions: BoundedVec::new(provider)
                .map_err(|_| Error::memory_error("Failed to create functions vector"))?,
            inlined_functions: InlinedFunctions::new(),
            current_cu: 0,
        })
    }

    /// Parse the debug information
    pub fn parse(&mut self) -> Result<()> {
        let mut cursor = DwarfCursor::new(self.debug_info);

        while !cursor.is_at_end() {
            // Parse compilation unit header
            let header = self.parse_cu_header(&mut cursor)?;

            // Parse abbreviations for this CU
            let abbrev_data = &self.debug_abbrev[header.abbrev_offset as usize..];
            self.abbrev_table.parse(abbrev_data)?;

            // Parse DIEs in this compilation unit
            self.parse_dies(&mut cursor, &header)?;
        }

        Ok(())
    }

    /// Parse compilation unit header
    fn parse_cu_header(&self, cursor: &mut DwarfCursor) -> Result<CompilationUnitHeader> {
        let unit_length = cursor.read_u32()?;
        if unit_length == 0xffffffff {
            return Err(Error::parse_error("64-bit DWARF not supported"));
        }

        let version = cursor.read_u16()?;
        if version < 2 || version > 5 {
            return Err(Error::parse_error("Unsupported DWARF version"));
        }

        let abbrev_offset = cursor.read_u32()?;
        let address_size = cursor.read_u8()?;

        Ok(CompilationUnitHeader {
            unit_length,
            version,
            abbrev_offset,
            address_size,
        })
    }

    /// Parse DIEs (Debugging Information Entries)
    fn parse_dies(
        &mut self,
        cursor: &mut DwarfCursor,
        header: &CompilationUnitHeader,
    ) -> Result<()> {
        let end_offset = cursor.position() + header.unit_length as usize - 7; // Adjust for header

        while cursor.position() < end_offset {
            let abbrev_code = cursor.read_uleb128_u32()?;
            if abbrev_code == 0 {
                // Null entry
                continue;
            }

            let abbrev = self
                .abbrev_table
                .find(abbrev_code)
                .ok_or_else(|| Error::parse_error("Abbreviation not found"))?;

            // Handle specific tags we care about
            match abbrev.tag {
                tags::DW_TAG_SUBPROGRAM => {
                    self.parse_function(cursor, abbrev, header)?;
                },
                tags::DW_TAG_INLINED_SUBROUTINE => {
                    self.parse_inlined_function(cursor, abbrev, header)?;
                },
                tags::DW_TAG_FORMAL_PARAMETER => {
                    // Parameters are handled as children of functions
                    self.skip_die_attributes(cursor, abbrev, header)?;
                },
                tags::DW_TAG_COMPILE_UNIT => {
                    self.current_cu += 1;
                    self.skip_die_attributes(cursor, abbrev, header)?;
                },
                _ => {
                    // Skip other DIEs
                    self.skip_die_attributes(cursor, abbrev, header)?;
                },
            }

            // Skip children if any
            if abbrev.has_children {
                self.skip_children(cursor)?;
            }
        }

        Ok(())
    }

    /// Parse a function DIE
    fn parse_function(
        &mut self,
        cursor: &mut DwarfCursor,
        abbrev: &crate::abbrev::Abbreviation,
        header: &CompilationUnitHeader,
    ) -> Result<()> {
        let mut func = FunctionInfo {
            name: None,
            low_pc: 0,
            high_pc: 0,
            file_index: 0,
            line: 0,
            parameters: None,
            return_type: BasicType::Void,
            is_inline: false,
        };

        // Parse attributes
        for attr_spec in abbrev.attributes.iter() {
            match attr_spec.name {
                attributes::DW_AT_NAME => {
                    match attr_spec.form {
                        AttributeForm::Strp => {
                            let str_offset = cursor.read_u32()?;
                            if let Some(ref string_table) = self.string_table {
                                func.name = string_table.get_string(str_offset);
                            }
                        },
                        AttributeForm::String => {
                            // Inline string - read directly from debug_info
                            if let Ok(debug_str) = crate::strings::read_inline_string(cursor) {
                                func.name = Some(debug_str);
                            }
                        },
                        _ => {
                            self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                        },
                    }
                },
                attributes::DW_AT_LOW_PC => {
                    if header.address_size == 4 {
                        func.low_pc = cursor.read_u32()?;
                    } else {
                        cursor.skip(header.address_size as usize)?;
                    }
                },
                attributes::DW_AT_HIGH_PC => {
                    if header.address_size == 4 {
                        func.high_pc = cursor.read_u32()?;
                    } else {
                        cursor.skip(header.address_size as usize)?;
                    }
                },
                attributes::DW_AT_DECL_FILE => {
                    func.file_index = cursor.read_uleb128()? as u16;
                },
                attributes::DW_AT_DECL_LINE => {
                    func.line = cursor.read_uleb128()? as u32;
                },
                _ => {
                    self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                },
            }
        }

        // Parse children if this function has them (for parameters)
        if abbrev.has_children {
            let mut params = ParameterList::new();
            let mut param_position = 0u16;

            // Parse children until we hit a null entry
            loop {
                let child_abbrev_code = cursor.read_uleb128_u32()?;
                if child_abbrev_code == 0 {
                    break; // End of children
                }

                let child_abbrev = self
                    .abbrev_table
                    .find(child_abbrev_code)
                    .ok_or_else(|| Error::parse_error("Child abbreviation not found"))?;

                // Handle parameter DIEs
                if child_abbrev.tag == tags::DW_TAG_FORMAL_PARAMETER {
                    if let Some(param) =
                        self.parse_parameter(cursor, child_abbrev, header, param_position)?
                    {
                        params.add_parameter(param).ok();
                        param_position += 1;
                    }
                } else {
                    // Skip non-parameter children
                    self.skip_die_attributes(cursor, child_abbrev, header)?;
                    if child_abbrev.has_children {
                        self.skip_children(cursor)?;
                    }
                }
            }

            if params.count() > 0 {
                func.parameters = Some(params);
            }
        }

        // Store function if it has a valid address range
        if func.low_pc != 0 && func.high_pc != 0 {
            self.functions.push(func).ok(); // Ignore capacity errors
        }

        Ok(())
    }

    /// Skip DIE attributes we don't care about
    fn skip_die_attributes(
        &self,
        cursor: &mut DwarfCursor,
        abbrev: &crate::abbrev::Abbreviation,
        header: &CompilationUnitHeader,
    ) -> Result<()> {
        for attr_spec in abbrev.attributes.iter() {
            self.skip_attribute_value(cursor, &attr_spec.form, header)?;
        }
        Ok(())
    }

    /// Skip an attribute value based on its form
    fn skip_attribute_value(
        &self,
        cursor: &mut DwarfCursor,
        form: &AttributeForm,
        header: &CompilationUnitHeader,
    ) -> Result<()> {
        match form {
            AttributeForm::Addr => cursor.skip(header.address_size as usize)?,
            AttributeForm::Block1 => {
                let len = cursor.read_u8()? as usize;
                cursor.skip(len)?;
            },
            AttributeForm::Block2 => {
                let len = cursor.read_u16()? as usize;
                cursor.skip(len)?;
            },
            AttributeForm::Block4 => {
                let len = cursor.read_u32()? as usize;
                cursor.skip(len)?;
            },
            AttributeForm::Data1 | AttributeForm::Ref1 | AttributeForm::Flag => {
                cursor.skip(1)?;
            },
            AttributeForm::Data2 | AttributeForm::Ref2 => {
                cursor.skip(2)?;
            },
            AttributeForm::Data4
            | AttributeForm::Ref4
            | AttributeForm::Strp
            | AttributeForm::SecOffset => {
                cursor.skip(4)?;
            },
            AttributeForm::Data8 | AttributeForm::Ref8 | AttributeForm::RefSig8 => {
                cursor.skip(8)?;
            },
            AttributeForm::String => {
                // Skip null-terminated string
                while cursor.read_u8()? != 0 {}
            },
            AttributeForm::Block | AttributeForm::Exprloc => {
                let len = cursor.read_uleb128()? as usize;
                cursor.skip(len)?;
            },
            AttributeForm::Sdata | AttributeForm::Udata | AttributeForm::RefUdata => {
                cursor.read_uleb128()?;
            },
            AttributeForm::RefAddr => {
                cursor.skip(header.address_size as usize)?;
            },
            AttributeForm::FlagPresent => {
                // No data to skip
            },
            AttributeForm::Indirect => {
                let actual_form = cursor.read_uleb128()? as u16;
                let form = AttributeForm::from_u16(actual_form);
                self.skip_attribute_value(cursor, &form, header)?;
            },
            AttributeForm::Unknown(_) => {
                return Err(Error::parse_error("Unknown attribute form"));
            },
        }
        Ok(())
    }

    /// Skip children DIEs
    fn skip_children(&self, cursor: &mut DwarfCursor) -> Result<()> {
        let mut depth = 1;

        while depth > 0 {
            let abbrev_code = cursor.read_uleb128_u32()?;
            if abbrev_code == 0 {
                depth -= 1;
                continue;
            }

            let abbrev = self
                .abbrev_table
                .find(abbrev_code)
                .ok_or_else(|| Error::parse_error("Abbreviation not found while skipping"))?;

            // Skip attributes
            for attr_spec in abbrev.attributes.iter() {
                self.skip_attribute_value(
                    cursor,
                    &attr_spec.form,
                    &CompilationUnitHeader {
                        unit_length: 0,
                        version: 4,
                        abbrev_offset: 0,
                        address_size: 4, // Assume 32-bit for WebAssembly
                    },
                )?;
            }

            if abbrev.has_children {
                depth += 1;
            }
        }

        Ok(())
    }

    /// Find function containing the given PC
    pub fn find_function(&self, pc: u32) -> Option<&FunctionInfo<'a>> {
        self.functions.iter().find(|func| pc >= func.low_pc && pc < func.high_pc)
    }

    /// Get all parsed functions
    pub fn functions(&self) -> &[FunctionInfo<'a>] {
        self.functions.as_slice()
    }

    /// Parse a parameter DIE
    fn parse_parameter(
        &self,
        cursor: &mut DwarfCursor,
        abbrev: &crate::abbrev::Abbreviation,
        header: &CompilationUnitHeader,
        position: u16,
    ) -> Result<Option<Parameter<'a>>> {
        let mut param = Parameter {
            name: None,
            param_type: BasicType::Unknown,
            file_index: 0,
            line: 0,
            position,
            is_variadic: false,
        };

        // Parse attributes
        for attr_spec in abbrev.attributes.iter() {
            match attr_spec.name {
                attributes::DW_AT_NAME => match attr_spec.form {
                    AttributeForm::Strp => {
                        let str_offset = cursor.read_u32()?;
                        if let Some(ref string_table) = self.string_table {
                            param.name = string_table.get_string(str_offset);
                        }
                    },
                    AttributeForm::String => {
                        if let Ok(debug_str) = crate::strings::read_inline_string(cursor) {
                            param.name = Some(debug_str);
                        }
                    },
                    _ => {
                        self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                    },
                },
                attributes::DW_AT_TYPE => {
                    // For now, just mark as having a type
                    // Full type resolution would require following type references
                    self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                    param.param_type = BasicType::Unknown;
                },
                attributes::DW_AT_DECL_FILE => {
                    param.file_index = cursor.read_uleb128()? as u16;
                },
                attributes::DW_AT_DECL_LINE => {
                    param.line = cursor.read_uleb128()? as u32;
                },
                _ => {
                    self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                },
            }
        }

        Ok(Some(param))
    }

    /// Parse an inlined function DIE
    fn parse_inlined_function(
        &mut self,
        cursor: &mut DwarfCursor,
        abbrev: &crate::abbrev::Abbreviation,
        header: &CompilationUnitHeader,
    ) -> Result<()> {
        let mut inlined = InlinedFunction {
            name: None,
            abstract_origin: 0,
            low_pc: 0,
            high_pc: 0,
            call_file: 0,
            call_line: 0,
            call_column: 0,
            depth: 0,
        };

        // Parse attributes
        for attr_spec in abbrev.attributes.iter() {
            match attr_spec.name {
                attributes::DW_AT_ABSTRACT_ORIGIN => {
                    inlined.abstract_origin = cursor.read_u32()?;
                },
                attributes::DW_AT_LOW_PC => {
                    if header.address_size == 4 {
                        inlined.low_pc = cursor.read_u32()?;
                    } else {
                        cursor.skip(header.address_size as usize)?;
                    }
                },
                attributes::DW_AT_HIGH_PC => {
                    if header.address_size == 4 {
                        inlined.high_pc = cursor.read_u32()?;
                    } else {
                        cursor.skip(header.address_size as usize)?;
                    }
                },
                attributes::DW_AT_CALL_FILE => {
                    inlined.call_file = cursor.read_uleb128()? as u16;
                },
                attributes::DW_AT_CALL_LINE => {
                    inlined.call_line = cursor.read_uleb128()? as u32;
                },
                attributes::DW_AT_CALL_COLUMN => {
                    inlined.call_column = cursor.read_uleb128()? as u16;
                },
                _ => {
                    self.skip_attribute_value(cursor, &attr_spec.form, header)?;
                },
            }
        }

        // Store inlined function if it has valid addresses
        if inlined.low_pc != 0 && inlined.high_pc != 0 {
            self.inlined_functions.add(inlined).ok();
        }

        Ok(())
    }

    /// Get inlined functions at a specific PC
    pub fn find_inlined_at(&self, pc: u32) -> Vec<&InlinedFunction<'a>> {
        self.inlined_functions.find_at_pc(pc).collect()
    }

    /// Check if there are multiple compilation units
    pub fn has_multiple_cus(&self) -> bool {
        self.current_cu > 1
    }
}
