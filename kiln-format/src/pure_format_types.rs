//! Pure format representation types
//!
//! This module contains pure binary format representations that contain
//! only the structural information needed for parsing, without any
//! runtime initialization logic.

use crate::prelude::*;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    vec,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    vec,
    vec::Vec,
};

/// Pure data segment mode (format representation only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureDataMode {
    /// Active data segment reference (format-only, contains indices and
    /// expression data)
    Active {
        /// Memory index reference
        memory_index:    u32,
        /// Offset expression length (for parsing validation)
        offset_expr_len: u32,
    },
    /// Passive data segment (used with memory.init)
    Passive,
}

impl Default for PureDataMode {
    fn default() -> Self {
        Self::Passive
    }
}

/// Pure element segment mode (format representation only)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PureElementMode {
    /// Active element segment reference (format-only, contains indices and
    /// expression data)
    Active {
        /// Table index reference
        table_index:     u32,
        /// Offset expression length (for parsing validation)
        offset_expr_len: u32,
    },
    /// Passive element segment (used with table.init)
    Passive,
    /// Declared element segment (available for linking but not runtime init)
    Declared,
}

impl Default for PureElementMode {
    fn default() -> Self {
        Self::Passive
    }
}

/// Pure data segment (format representation only)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureDataSegment {
    /// Data mode (pure format representation)
    pub mode:              PureDataMode,
    /// Raw offset expression bytes (format-only, runtime interprets)
    pub offset_expr_bytes: Vec<u8>,
    /// Data bytes
    pub data_bytes:        Vec<u8>,
}

/// Pure element segment (format representation only)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureElementSegment {
    /// Element mode (pure format representation)
    pub mode:              PureElementMode,
    /// Element type (funcref, externref, etc.)
    pub element_type:      crate::types::RefType,
    /// Raw offset expression bytes (format-only, runtime interprets)
    pub offset_expr_bytes: Vec<u8>,
    /// Element initialization data (indices or expression bytes)
    pub init_data:         PureElementInit,
}

/// Pure element initialization data (format representation only)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PureElementInit {
    /// Function indices
    FunctionIndices(Vec<u32>),
    /// Raw expression bytes (runtime interprets)
    ExpressionBytes(Vec<Vec<u8>>),
}

impl Default for PureDataSegment {
    fn default() -> Self {
        Self {
            mode:              PureDataMode::default(),
            offset_expr_bytes: Vec::new(),
            data_bytes:        Vec::new(),
        }
    }
}

impl Default for PureElementSegment {
    fn default() -> Self {
        Self {
            mode:              PureElementMode::default(),
            element_type:      crate::types::RefType::Funcref,
            offset_expr_bytes: Vec::new(),
            init_data:         PureElementInit::FunctionIndices(Vec::new()),
        }
    }
}

impl Default for PureElementInit {
    fn default() -> Self {
        Self::FunctionIndices(Vec::new())
    }
}

impl PureDataSegment {
    /// Create new active data segment
    pub fn new_active(memory_index: u32, offset_expr: Vec<u8>, data: Vec<u8>) -> Self {
        Self {
            mode:              PureDataMode::Active {
                memory_index,
                offset_expr_len: offset_expr.len() as u32,
            },
            offset_expr_bytes: offset_expr,
            data_bytes:        data,
        }
    }

    /// Create new passive data segment
    pub fn new_passive(data: Vec<u8>) -> Self {
        Self {
            mode:              PureDataMode::Passive,
            offset_expr_bytes: Vec::new(),
            data_bytes:        data,
        }
    }

    /// Check if this is an active segment
    pub fn is_active(&self) -> bool {
        matches!(self.mode, PureDataMode::Active { .. })
    }

    /// Get memory index if active
    pub fn memory_index(&self) -> Option<u32> {
        match self.mode {
            PureDataMode::Active { memory_index, .. } => Some(memory_index),
            PureDataMode::Passive => None,
        }
    }
}

impl PureElementSegment {
    /// Create new active element segment
    pub fn new_active(
        table_index: u32,
        element_type: crate::types::RefType,
        offset_expr: Vec<u8>,
        init_data: PureElementInit,
    ) -> Self {
        Self {
            mode: PureElementMode::Active {
                table_index,
                offset_expr_len: offset_expr.len() as u32,
            },
            element_type,
            offset_expr_bytes: offset_expr,
            init_data,
        }
    }

    /// Create new passive element segment
    pub fn new_passive(element_type: crate::types::RefType, init_data: PureElementInit) -> Self {
        Self {
            mode: PureElementMode::Passive,
            element_type,
            offset_expr_bytes: Vec::new(),
            init_data,
        }
    }

    /// Create new declared element segment
    pub fn new_declared(element_type: crate::types::RefType, init_data: PureElementInit) -> Self {
        Self {
            mode: PureElementMode::Declared,
            element_type,
            offset_expr_bytes: Vec::new(),
            init_data,
        }
    }

    /// Check if this is an active segment
    pub fn is_active(&self) -> bool {
        matches!(self.mode, PureElementMode::Active { .. })
    }

    /// Get table index if active
    pub fn table_index(&self) -> Option<u32> {
        match self.mode {
            PureElementMode::Active { table_index, .. } => Some(table_index),
            _ => None,
        }
    }
}

// Trait implementations for PureDataMode
impl wrt_foundation::traits::Checksummable for PureDataMode {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            PureDataMode::Active {
                memory_index,
                offset_expr_len,
            } => {
                checksum.update_slice(&[0u8]); // Discriminant
                checksum.update_slice(&memory_index.to_le_bytes());
                checksum.update_slice(&offset_expr_len.to_le_bytes());
            },
            PureDataMode::Passive => {
                checksum.update_slice(&[1u8]); // Discriminant
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for PureDataMode {
    fn serialized_size(&self) -> usize {
        match self {
            PureDataMode::Active { .. } => 9, // 1 + 4 + 4
            PureDataMode::Passive => 1,
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            PureDataMode::Active {
                memory_index,
                offset_expr_len,
            } => {
                writer.write_all(&[0u8])?;
                writer.write_all(&memory_index.to_le_bytes())?;
                writer.write_all(&offset_expr_len.to_le_bytes())?;
            },
            PureDataMode::Passive => {
                writer.write_all(&[1u8])?;
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PureDataMode {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut disc_bytes = [0u8; 1];
        reader.read_exact(&mut disc_bytes)?;
        match disc_bytes[0] {
            0 => {
                let mut memory_index_bytes = [0u8; 4];
                reader.read_exact(&mut memory_index_bytes)?;
                let memory_index = u32::from_le_bytes(memory_index_bytes);

                let mut offset_expr_len_bytes = [0u8; 4];
                reader.read_exact(&mut offset_expr_len_bytes)?;
                let offset_expr_len = u32::from_le_bytes(offset_expr_len_bytes);

                Ok(PureDataMode::Active {
                    memory_index,
                    offset_expr_len,
                })
            },
            1 => Ok(PureDataMode::Passive),
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Invalid PureDataMode discriminant",
            )),
        }
    }
}

// Trait implementations for PureElementMode
impl wrt_foundation::traits::Checksummable for PureElementMode {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            PureElementMode::Active {
                table_index,
                offset_expr_len,
            } => {
                checksum.update_slice(&[0u8]); // Discriminant
                checksum.update_slice(&table_index.to_le_bytes());
                checksum.update_slice(&offset_expr_len.to_le_bytes());
            },
            PureElementMode::Passive => {
                checksum.update_slice(&[1u8]); // Discriminant
            },
            PureElementMode::Declared => {
                checksum.update_slice(&[2u8]); // Discriminant
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for PureElementMode {
    fn serialized_size(&self) -> usize {
        match self {
            PureElementMode::Active { .. } => 9, // 1 + 4 + 4
            PureElementMode::Passive | PureElementMode::Declared => 1,
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            PureElementMode::Active {
                table_index,
                offset_expr_len,
            } => {
                writer.write_all(&[0u8])?;
                writer.write_all(&table_index.to_le_bytes())?;
                writer.write_all(&offset_expr_len.to_le_bytes())?;
            },
            PureElementMode::Passive => {
                writer.write_all(&[1u8])?;
            },
            PureElementMode::Declared => {
                writer.write_all(&[2u8])?;
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PureElementMode {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut disc_bytes = [0u8; 1];
        reader.read_exact(&mut disc_bytes)?;
        match disc_bytes[0] {
            0 => {
                let mut table_index_bytes = [0u8; 4];
                reader.read_exact(&mut table_index_bytes)?;
                let table_index = u32::from_le_bytes(table_index_bytes);

                let mut offset_expr_len_bytes = [0u8; 4];
                reader.read_exact(&mut offset_expr_len_bytes)?;
                let offset_expr_len = u32::from_le_bytes(offset_expr_len_bytes);

                Ok(PureElementMode::Active {
                    table_index,
                    offset_expr_len,
                })
            },
            1 => Ok(PureElementMode::Passive),
            2 => Ok(PureElementMode::Declared),
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Invalid PureElementMode discriminant",
            )),
        }
    }
}

// Trait implementations for PureElementInit
impl wrt_foundation::traits::Checksummable for PureElementInit {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            PureElementInit::FunctionIndices(indices) => {
                checksum.update_slice(&[0u8]); // Discriminant
                checksum.update_slice(&(indices.len() as u32).to_le_bytes());
                for index in indices {
                    checksum.update_slice(&index.to_le_bytes());
                }
            },
            PureElementInit::ExpressionBytes(exprs) => {
                checksum.update_slice(&[1u8]); // Discriminant
                checksum.update_slice(&(exprs.len() as u32).to_le_bytes());
                for expr in exprs {
                    checksum.update_slice(&(expr.len() as u32).to_le_bytes());
                    checksum.update_slice(expr);
                }
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for PureElementInit {
    fn serialized_size(&self) -> usize {
        match self {
            PureElementInit::FunctionIndices(indices) => 5 + indices.len() * 4, // 1 + 4 + len * 4
            PureElementInit::ExpressionBytes(exprs) => {
                5 + exprs.iter().map(|e| 4 + e.len()).sum::<usize>() // 1 + 4 + sum(4 + len)
            },
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            PureElementInit::FunctionIndices(indices) => {
                writer.write_all(&[0u8])?;
                writer.write_all(&(indices.len() as u32).to_le_bytes())?;
                for index in indices {
                    writer.write_all(&index.to_le_bytes())?;
                }
            },
            PureElementInit::ExpressionBytes(exprs) => {
                writer.write_all(&[1u8])?;
                writer.write_all(&(exprs.len() as u32).to_le_bytes())?;
                for expr in exprs {
                    writer.write_all(&(expr.len() as u32).to_le_bytes())?;
                    writer.write_all(expr)?;
                }
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PureElementInit {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut disc_bytes = [0u8; 1];
        reader.read_exact(&mut disc_bytes)?;
        match disc_bytes[0] {
            0 => {
                let mut len_bytes = [0u8; 4];
                reader.read_exact(&mut len_bytes)?;
                let len = u32::from_le_bytes(len_bytes) as usize;

                let mut indices = Vec::with_capacity(len);
                for _ in 0..len {
                    let mut index_bytes = [0u8; 4];
                    reader.read_exact(&mut index_bytes)?;
                    indices.push(u32::from_le_bytes(index_bytes));
                }
                Ok(PureElementInit::FunctionIndices(indices))
            },
            1 => {
                let mut len_bytes = [0u8; 4];
                reader.read_exact(&mut len_bytes)?;
                let len = u32::from_le_bytes(len_bytes) as usize;

                let mut exprs = Vec::with_capacity(len);
                for _ in 0..len {
                    let mut expr_len_bytes = [0u8; 4];
                    reader.read_exact(&mut expr_len_bytes)?;
                    let expr_len = u32::from_le_bytes(expr_len_bytes) as usize;

                    let mut expr = vec![0u8; expr_len];
                    reader.read_exact(&mut expr)?;
                    exprs.push(expr);
                }
                Ok(PureElementInit::ExpressionBytes(exprs))
            },
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Invalid PureElementInit discriminant",
            )),
        }
    }
}

// Trait implementations for PureDataSegment
impl wrt_foundation::traits::Checksummable for PureDataSegment {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.mode.update_checksum(checksum);
        checksum.update_slice(&(self.offset_expr_bytes.len() as u32).to_le_bytes());
        checksum.update_slice(&self.offset_expr_bytes);
        checksum.update_slice(&(self.data_bytes.len() as u32).to_le_bytes());
        checksum.update_slice(&self.data_bytes);
    }
}

impl wrt_foundation::traits::ToBytes for PureDataSegment {
    fn serialized_size(&self) -> usize {
        self.mode.serialized_size() + 8 + self.offset_expr_bytes.len() + self.data_bytes.len()
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.mode.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&(self.offset_expr_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&self.offset_expr_bytes)?;
        writer.write_all(&(self.data_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&self.data_bytes)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PureDataSegment {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let mode = PureDataMode::from_bytes_with_provider(reader, provider)?;

        let mut offset_len_bytes = [0u8; 4];
        reader.read_exact(&mut offset_len_bytes)?;
        let offset_len = u32::from_le_bytes(offset_len_bytes) as usize;

        let mut offset_expr_bytes = vec![0u8; offset_len];
        reader.read_exact(&mut offset_expr_bytes)?;

        let mut data_len_bytes = [0u8; 4];
        reader.read_exact(&mut data_len_bytes)?;
        let data_len = u32::from_le_bytes(data_len_bytes) as usize;

        let mut data_bytes = vec![0u8; data_len];
        reader.read_exact(&mut data_bytes)?;

        Ok(PureDataSegment {
            mode,
            offset_expr_bytes,
            data_bytes,
        })
    }
}

// Trait implementations for PureElementSegment
impl wrt_foundation::traits::Checksummable for PureElementSegment {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.element_type.update_checksum(checksum);
        self.mode.update_checksum(checksum);
        checksum.update_slice(&(self.offset_expr_bytes.len() as u32).to_le_bytes());
        checksum.update_slice(&self.offset_expr_bytes);
        self.init_data.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for PureElementSegment {
    fn serialized_size(&self) -> usize {
        1 + self.mode.serialized_size()
            + 4
            + self.offset_expr_bytes.len()
            + self.init_data.serialized_size()
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let element_type_byte = match self.element_type {
            crate::types::RefType::Funcref => 0u8,
            crate::types::RefType::Externref => 1u8,
        };
        writer.write_all(&[element_type_byte])?;
        self.mode.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&(self.offset_expr_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&self.offset_expr_bytes)?;
        self.init_data.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PureElementSegment {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let mut element_type_bytes = [0u8; 1];
        reader.read_exact(&mut element_type_bytes)?;
        let element_type = match element_type_bytes[0] {
            0 => crate::types::RefType::Funcref,
            1 => crate::types::RefType::Externref,
            _ => {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Invalid element type",
                ))
            },
        };

        let mode = PureElementMode::from_bytes_with_provider(reader, provider)?;

        let mut offset_len_bytes = [0u8; 4];
        reader.read_exact(&mut offset_len_bytes)?;
        let offset_len = u32::from_le_bytes(offset_len_bytes) as usize;

        let mut offset_expr_bytes = vec![0u8; offset_len];
        reader.read_exact(&mut offset_expr_bytes)?;

        let init_data = PureElementInit::from_bytes_with_provider(reader, provider)?;

        Ok(PureElementSegment {
            element_type,
            mode,
            offset_expr_bytes,
            init_data,
        })
    }
}
