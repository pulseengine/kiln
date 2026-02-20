// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! DWARF line number program implementation

use wrt_error::{Error, Result};

use crate::cursor::DwarfCursor;

/// Minimal line number information
#[derive(Clone, Copy, Debug)]
pub struct LineInfo {
    /// Index into file table
    pub file_index: u16,
    /// Source line number
    pub line: u32,
    /// Source column number
    pub column: u16,
    /// Is a statement boundary
    pub is_stmt: bool,
    /// Marks end of a sequence
    pub end_sequence: bool,
}

impl LineInfo {
    /// Format as "filename:line:column" for display
    /// Uses the provided file table to resolve the file index
    pub fn format_location<'a>(
        &'a self,
        file_table: &'a crate::FileTable<'a>,
    ) -> LocationDisplay<'a> {
        LocationDisplay {
            line_info: self,
            file_table,
        }
    }
}

/// Helper for displaying line information with resolved file paths
pub struct LocationDisplay<'a> {
    line_info: &'a LineInfo,
    file_table: &'a crate::FileTable<'a>,
}

impl<'a> LocationDisplay<'a> {
    /// Write the location in "file:line:column" format
    pub fn display<F>(&self, mut writer: F) -> core::result::Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> core::result::Result<(), core::fmt::Error>,
    {
        // Get the file path
        if let Some(file_path) = self.file_table.get_full_path(self.line_info.file_index) {
            file_path.display(&mut writer)?;
        } else {
            writer("<unknown>")?;
        }

        // Add line number
        writer(":")?;
        // Binary std/no_std choice
        let mut buf = [0u8; 10];
        let s = format_u32(self.line_info.line, &mut buf);
        writer(s)?;

        // Add column if present
        if self.line_info.column > 0 {
            writer(":")?;
            let s = format_u32(self.line_info.column as u32, &mut buf);
            writer(s)?;
        }

        Ok(())
    }
}

// Binary std/no_std choice
fn format_u32(mut n: u32, buf: &mut [u8]) -> &str {
    if n == 0 {
        return "0";
    }

    let mut i = buf.len();
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

/// DWARF line number program opcodes
mod opcodes {
    pub const DW_LNS_COPY: u8 = 1;
    pub const DW_LNS_ADVANCE_PC: u8 = 2;
    pub const DW_LNS_ADVANCE_LINE: u8 = 3;
    pub const DW_LNS_SET_FILE: u8 = 4;
    pub const DW_LNS_SET_COLUMN: u8 = 5;
    pub const DW_LNS_NEGATE_STMT: u8 = 6;
    pub const DW_LNS_SET_BASIC_BLOCK: u8 = 7;
    pub const DW_LNS_CONST_ADD_PC: u8 = 8;
    pub const DW_LNS_FIXED_ADVANCE_PC: u8 = 9;
    pub const DW_LNS_SET_PROLOGUE_END: u8 = 10;
    pub const DW_LNS_SET_EPILOGUE_BEGIN: u8 = 11;
    pub const DW_LNS_SET_ISA: u8 = 12;

    pub const DW_LNE_END_SEQUENCE: u8 = 1;
    pub const DW_LNE_SET_ADDRESS: u8 = 2;
    #[allow(dead_code)]
    pub const DW_LNE_DEFINE_FILE: u8 = 3;
    #[allow(dead_code)]
    pub const DW_LNE_SET_DISCRIMINATOR: u8 = 4;
}

/// Line number program state machine
pub struct LineNumberState {
    // Standard registers
    address: u32,
    file: u16,
    line: u32,
    column: u16,
    is_stmt: bool,
    basic_block: bool,
    end_sequence: bool,
    prologue_end: bool,
    epilogue_begin: bool,
    isa: u32,
    discriminator: u32,

    // Header configuration
    minimum_instruction_length: u8,
    maximum_ops_per_instruction: u8,
    default_is_stmt: bool,
    line_base: i8,
    line_range: u8,
    opcode_base: u8,

    // Standard opcode lengths (we'll store a few)
    standard_opcode_lengths: [u8; 12],
}

impl Default for LineNumberState {
    fn default() -> Self {
        Self::new()
    }
}

impl LineNumberState {
    /// Create a new line number state machine
    pub fn new() -> Self {
        Self {
            address: 0,
            file: 1,
            line: 1,
            column: 0,
            is_stmt: false,
            basic_block: false,
            end_sequence: false,
            prologue_end: false,
            epilogue_begin: false,
            isa: 0,
            discriminator: 0,
            minimum_instruction_length: 1,
            maximum_ops_per_instruction: 1,
            default_is_stmt: true,
            line_base: -5,
            line_range: 14,
            opcode_base: 13,
            standard_opcode_lengths: [0; 12],
        }
    }

    /// Reset the state machine (except header fields)
    fn reset(&mut self) {
        self.address = 0;
        self.file = 1;
        self.line = 1;
        self.column = 0;
        self.is_stmt = self.default_is_stmt;
        self.basic_block = false;
        self.end_sequence = false;
        self.prologue_end = false;
        self.epilogue_begin = false;
        self.isa = 0;
        self.discriminator = 0;
    }

    /// Parse the line number program header
    fn parse_header(&mut self, cursor: &mut DwarfCursor) -> Result<()> {
        // Read unit length (32-bit for now, skip 64-bit DWARF)
        let unit_length = cursor.read_u32()?;
        if unit_length == 0xffffffff {
            return Err(Error::parse_error("64-bit DWARF not supported"));
        }

        // Read version
        let version = cursor.read_u16()?;
        if !(2..=5).contains(&version) {
            return Err(Error::parse_error("Unsupported DWARF line version"));
        }

        // Read header length
        let header_length = cursor.read_u32()?;
        let header_start = cursor.position();

        // Read header fields
        self.minimum_instruction_length = cursor.read_u8()?;
        if version >= 4 {
            self.maximum_ops_per_instruction = cursor.read_u8()?;
        }
        self.default_is_stmt = cursor.read_u8()? != 0;
        self.line_base = cursor.read_u8()? as i8;
        self.line_range = cursor.read_u8()?;
        self.opcode_base = cursor.read_u8()?;

        // Read standard opcode lengths
        for i in 0..self.opcode_base.saturating_sub(1).min(12) as usize {
            self.standard_opcode_lengths[i] = cursor.read_u8()?;
        }

        // Skip the rest of the header (directories, files, etc.)
        let header_consumed = cursor.position() - header_start;
        if header_consumed < header_length as usize {
            cursor.skip(header_length as usize - header_consumed)?;
        }

        Ok(())
    }

    /// Execute a special opcode
    fn execute_special_opcode(&mut self, opcode: u8) -> Result<()> {
        let adjusted_opcode = opcode - self.opcode_base;
        let address_increment =
            (adjusted_opcode / self.line_range) as u32 * self.minimum_instruction_length as u32;
        let line_increment = self.line_base + (adjusted_opcode % self.line_range) as i8;

        self.address += address_increment;
        self.line = (self.line as i32 + line_increment as i32) as u32;
        self.basic_block = false;
        self.prologue_end = false;
        self.epilogue_begin = false;
        self.discriminator = 0;

        Ok(())
    }

    /// Execute a standard opcode
    fn execute_standard_opcode(&mut self, opcode: u8, cursor: &mut DwarfCursor) -> Result<()> {
        use opcodes::*;

        match opcode {
            DW_LNS_COPY => {
                self.discriminator = 0;
                self.basic_block = false;
                self.prologue_end = false;
                self.epilogue_begin = false;
            },
            DW_LNS_ADVANCE_PC => {
                let advance = cursor.read_uleb128()? as u32;
                self.address += advance * self.minimum_instruction_length as u32;
            },
            DW_LNS_ADVANCE_LINE => {
                let advance = cursor.read_sleb128()? as i32;
                self.line = (self.line as i32 + advance) as u32;
            },
            DW_LNS_SET_FILE => {
                self.file = cursor.read_uleb128()? as u16;
            },
            DW_LNS_SET_COLUMN => {
                self.column = cursor.read_uleb128()? as u16;
            },
            DW_LNS_NEGATE_STMT => {
                self.is_stmt = !self.is_stmt;
            },
            DW_LNS_SET_BASIC_BLOCK => {
                self.basic_block = true;
            },
            DW_LNS_CONST_ADD_PC => {
                let adjusted_opcode = 255 - self.opcode_base;
                let address_increment = (adjusted_opcode / self.line_range) as u32
                    * self.minimum_instruction_length as u32;
                self.address += address_increment;
            },
            DW_LNS_FIXED_ADVANCE_PC => {
                let advance = cursor.read_u16()? as u32;
                self.address += advance;
            },
            DW_LNS_SET_PROLOGUE_END => {
                self.prologue_end = true;
            },
            DW_LNS_SET_EPILOGUE_BEGIN => {
                self.epilogue_begin = true;
            },
            DW_LNS_SET_ISA => {
                self.isa = cursor.read_uleb128()? as u32;
            },
            _ => {
                // Unknown opcode, skip its arguments
                if opcode > 0 && opcode < self.opcode_base {
                    let arg_count = self
                        .standard_opcode_lengths
                        .get((opcode - 1) as usize)
                        .copied()
                        .unwrap_or(0);
                    for _ in 0..arg_count {
                        cursor.read_uleb128()?;
                    }
                }
            },
        }

        Ok(())
    }

    /// Binary std/no_std choice
    pub fn find_line_for_pc(
        &mut self,
        debug_line_data: &[u8],
        target_pc: u32,
    ) -> Result<Option<LineInfo>> {
        let mut cursor = DwarfCursor::new(debug_line_data);

        // Parse header
        self.parse_header(&mut cursor)?;

        // Reset state machine
        self.reset();

        let mut last_line_info = None;

        // Execute line number program
        while !cursor.is_at_end() {
            let opcode = cursor.read_u8()?;

            match opcode {
                0 => {
                    // Extended opcode
                    let length = cursor.read_uleb128()?;
                    if length == 0 {
                        continue;
                    }

                    let extended_opcode = cursor.read_u8()?;
                    let remaining = length - 1;

                    match extended_opcode {
                        opcodes::DW_LNE_END_SEQUENCE => {
                            self.end_sequence = true;
                        },
                        opcodes::DW_LNE_SET_ADDRESS => {
                            // Assume 4-byte addresses for WebAssembly
                            if remaining >= 4 {
                                self.address = cursor.read_u32()?;
                            } else {
                                cursor.skip(remaining as usize)?;
                            }
                        },
                        _ => {
                            // Skip unknown extended opcodes
                            cursor.skip(remaining as usize)?;
                        },
                    }
                },
                1..=12 => {
                    // Standard opcodes
                    self.execute_standard_opcode(opcode, &mut cursor)?;
                },
                _ => {
                    // Special opcode
                    self.execute_special_opcode(opcode)?;
                },
            }

            // Check if we've found the target
            if self.address <= target_pc && !self.end_sequence {
                last_line_info = Some(LineInfo {
                    file_index: self.file,
                    line: self.line,
                    column: self.column,
                    is_stmt: self.is_stmt,
                    end_sequence: self.end_sequence,
                });
            } else if self.address > target_pc {
                // We've passed the target
                break;
            }

            if self.end_sequence {
                self.reset();
            }
        }

        Ok(last_line_info)
    }
}
