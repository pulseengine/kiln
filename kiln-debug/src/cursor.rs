// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! DWARF data cursor for zero-copy parsing

use wrt_error::{Error, Result};
use wrt_format::binary::{read_leb128_i32, read_leb128_i64, read_leb128_u32, read_leb128_u64};

/// DWARF parsing cursor
pub struct DwarfCursor<'a> {
    /// Data being parsed
    data: &'a [u8],
    /// Current position
    pos: usize,
}

impl<'a> DwarfCursor<'a> {
    /// Create a new cursor
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get remaining bytes
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Check if cursor is at end
    pub fn is_at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Read unsigned LEB128
    pub fn read_uleb128(&mut self) -> Result<u64> {
        let (value, consumed) = read_leb128_u64(self.data, self.pos)?;
        self.pos += consumed;
        Ok(value)
    }

    /// Read unsigned 32-bit LEB128
    pub fn read_uleb128_u32(&mut self) -> Result<u32> {
        let (value, consumed) = read_leb128_u32(self.data, self.pos)?;
        self.pos += consumed;
        Ok(value)
    }

    /// Read signed LEB128
    pub fn read_sleb128(&mut self) -> Result<i64> {
        let (value, consumed) = read_leb128_i64(self.data, self.pos)?;
        self.pos += consumed;
        Ok(value)
    }

    /// Read signed 32-bit LEB128
    pub fn read_sleb128_i32(&mut self) -> Result<i32> {
        let (value, consumed) = read_leb128_i32(self.data, self.pos)?;
        self.pos += consumed;
        Ok(value)
    }

    /// Read a single byte
    pub fn read_u8(&mut self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(Error::parse_error("Unexpected end of DWARF data"));
        }
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    /// Read a 16-bit value (little-endian)
    pub fn read_u16(&mut self) -> Result<u16> {
        if self.pos + 2 > self.data.len() {
            return Err(Error::parse_error("Unexpected end of DWARF data"));
        }
        let value = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(value)
    }

    /// Read a 32-bit value (little-endian)
    pub fn read_u32(&mut self) -> Result<u32> {
        if self.pos + 4 > self.data.len() {
            return Err(Error::parse_error("Unexpected end of DWARF data"));
        }
        let value = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(value)
    }

    /// Read a 64-bit value (little-endian)
    pub fn read_u64(&mut self) -> Result<u64> {
        if self.pos + 8 > self.data.len() {
            return Err(Error::parse_error("Unexpected end of DWARF data"));
        }
        let value = u64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(value)
    }

    /// Skip a number of bytes
    pub fn skip(&mut self, count: usize) -> Result<()> {
        if self.pos + count > self.data.len() {
            return Err(Error::parse_error("Skip beyond DWARF data bounds"));
        }
        self.pos += count;
        Ok(())
    }

    /// Read a slice of bytes
    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        if self.pos + count > self.data.len() {
            return Err(Error::parse_error("Read beyond DWARF data bounds"));
        }
        let slice = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    /// Peek at the next byte without advancing
    pub fn peek_u8(&self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(Error::parse_error("Peek beyond DWARF data bounds"));
        }
        Ok(self.data[self.pos])
    }

    /// Get remaining data as a slice
    pub fn remaining_slice(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    /// Advance the cursor position
    pub fn advance(&mut self, count: usize) -> Result<()> {
        self.skip(count)
    }
}
