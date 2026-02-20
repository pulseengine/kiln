//! WebAssembly type definitions.
//!
//! This module provides type definitions for WebAssembly types.
//! Most core types are re-exported from wrt-foundation.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_error::Result;
// Import types from wrt-foundation
pub use wrt_foundation::{
    BlockType,
    FuncType,
    RefType,
    ValueType,
};

/// WebAssembly memory index type (standard or 64-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryIndexType {
    /// Standard WebAssembly 1.0 memory (i32 addressing)
    /// Limited to 4GiB (65536 pages × 64KiB)
    I32,
    /// Memory64 extension (i64 addressing)
    /// Allows for memories larger than 4GiB
    I64,
}

/// WebAssembly limits
///
/// Limits represent the minimum and optional maximum sizes for
/// memories and tables as defined in the WebAssembly Core Specification.
///
/// For memories, limits are specified in units of pages (64KiB each).
/// For tables, limits are specified in number of elements.
///
/// The WebAssembly 1.0 specification has the following constraints:
/// - For memories, the maximum number of pages is 65536 (4GiB)
/// - Shared memories must have a maximum size specified
/// - The maximum size must be greater than or equal to the minimum size
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Limits {
    /// Minimum size (pages for memory, elements for table)
    pub min:      u64,
    /// Maximum size (optional, required for shared memories)
    pub max:      Option<u64>,
    /// Shared memory flag, used for memory types
    /// When true, memory can be shared between threads and requires max to be
    /// set
    pub shared:   bool,
    /// Whether this limit is for a 64-bit memory
    pub memory64: bool,
}

/// Parser-specific block type for binary format
#[derive(Debug, Clone, PartialEq)]
pub enum FormatBlockType {
    /// No return value (void)
    Empty,
    /// Single return value
    ValueType(ValueType),
    /// Function type reference
    TypeIndex(u32),
    /// Function type (used for complex block types)
    #[cfg(any(feature = "std", feature = "alloc"))]
    FuncType(wrt_foundation::CleanFuncType),
}

impl From<FormatBlockType> for BlockType {
    fn from(bt: FormatBlockType) -> Self {
        match bt {
            FormatBlockType::Empty => BlockType::Value(None),
            FormatBlockType::ValueType(vt) => BlockType::Value(Some(vt)),
            FormatBlockType::TypeIndex(idx) => BlockType::FuncType(idx),
            #[cfg(any(feature = "std", feature = "alloc"))]
            FormatBlockType::FuncType(_func_type) => BlockType::FuncType(0), /* TODO: proper type
                                                                              * index mapping with clean types */
        }
    }
}

/// Parse a value type byte to a ValueType enum using the conversion module
pub fn parse_value_type(byte: u8) -> Result<ValueType> {
    crate::conversion::parse_value_type(byte)
}

/// Convert a ValueType to its binary representation using the conversion module
pub fn value_type_to_byte(value_type: ValueType) -> u8 {
    crate::conversion::format_value_type(value_type)
}

/// Type for a global variable in the binary format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormatGlobalType {
    pub value_type: ValueType, // This is wrt_foundation::ValueType re-exported in this file
    pub mutable:    bool,
}

impl wrt_foundation::traits::Checksummable for FormatGlobalType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.value_type.update_checksum(checksum);
        checksum.update(if self.mutable { 1 } else { 0 });
    }
}

impl wrt_foundation::traits::ToBytes for FormatGlobalType {
    fn serialized_size(&self) -> usize {
        self.value_type.serialized_size() + 1
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        stream: &mut wrt_foundation::traits::WriteStream,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.value_type.to_bytes_with_provider(stream, provider)?;
        stream.write_u8(if self.mutable { 1 } else { 0 })?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for FormatGlobalType {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        stream: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let value_type = ValueType::from_bytes_with_provider(stream, provider)?;
        let mutable_byte = stream.read_u8()?;
        let mutable = match mutable_byte {
            0 => false,
            1 => true,
            _ => {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Invalid mutable byte value: expected 0 or 1",
                ))
            },
        };
        Ok(FormatGlobalType {
            value_type,
            mutable,
        })
    }
}

/// Represents the core WebAssembly specification version of a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CoreWasmVersion {
    /// WebAssembly Core Specification 2.0
    #[default]
    V2_0, // Assumes 0x01 0x00 0x00 0x00 version bytes
    /// WebAssembly Core Specification 3.0 (Draft)
    V3_0, /* Assumes 0x03 0x00 0x00 0x00 version bytes (hypothetical)
           * Potentially an Unknown or Other variant if needed */
}

impl CoreWasmVersion {
    /// Returns the raw version bytes for the Wasm module header.
    pub fn to_bytes(self) -> [u8; 4] {
        match self {
            CoreWasmVersion::V2_0 => [0x01, 0x00, 0x00, 0x00],
            CoreWasmVersion::V3_0 => [0x03, 0x00, 0x00, 0x00], // Hypothetical
        }
    }

    /// Attempts to create a CoreWasmVersion from module header version bytes.
    /// Returns None if the version bytes are not recognized.
    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        match bytes {
            [0x01, 0x00, 0x00, 0x00] => Some(CoreWasmVersion::V2_0),
            [0x03, 0x00, 0x00, 0x00] => Some(CoreWasmVersion::V3_0), // Hypothetical
            _ => None,
        }
    }
}

// Serialization helpers for Limits
impl Limits {
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        #[cfg(feature = "std")]
        {
            let mut bytes = Vec::new();
            // Encode min
            bytes.extend(&self.min.to_le_bytes());
            // Encode max (1 byte for Some/None, then 8 bytes if Some)
            if let Some(max) = self.max {
                bytes.push(1);
                bytes.extend(&max.to_le_bytes());
            } else {
                bytes.push(0);
            }
            // Encode shared flag
            bytes.push(self.shared as u8);
            // Encode memory64 flag
            bytes.push(self.memory64 as u8);
            Ok(bytes)
        }
        #[cfg(not(any(feature = "std")))]
        {
            use wrt_foundation::BoundedVec;
            let mut bytes = BoundedVec::<u8, 32, wrt_foundation::NoStdProvider<256>>::new(
                wrt_foundation::NoStdProvider::default(),
            )
            .map_err(|_| {
                wrt_error::Error::runtime_execution_error(
                    "Failed to allocate BoundedVec for limits encoding",
                )
            })?;
            // Encode min
            for &b in self.min.to_le_bytes().iter() {
                bytes.push(b).map_err(|_| {
                    wrt_error::Error::new(
                        wrt_error::ErrorCategory::Memory,
                        wrt_error::codes::MEMORY_ERROR,
                        "Failed to push min limit bytes",
                    )
                })?;
            }
            // Encode max
            if let Some(max) = self.max {
                bytes.push(1).map_err(|_| {
                    wrt_error::Error::runtime_execution_error("Failed to push max limit flag")
                })?;
                for &b in max.to_le_bytes().iter() {
                    bytes.push(b).map_err(|_| {
                        wrt_error::Error::new(
                            wrt_error::ErrorCategory::Memory,
                            wrt_error::codes::MEMORY_ERROR,
                            "Failed to push max limit bytes",
                        )
                    })?;
                }
            } else {
                bytes.push(0).map_err(|_| {
                    wrt_error::Error::runtime_execution_error("Failed to push no-max-limit flag")
                })?;
            }
            // Encode flags
            bytes.push(self.shared as u8).map_err(|_| {
                wrt_error::Error::new(
                    wrt_error::ErrorCategory::Memory,
                    wrt_error::codes::MEMORY_ERROR,
                    "Failed to push limit flag",
                )
            })?;
            bytes.push(self.memory64 as u8).map_err(|_| {
                wrt_error::Error::runtime_execution_error("Failed to push memory64 flag")
            })?;
            Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Runtime,
                wrt_error::codes::UNSUPPORTED_OPERATION,
                "Limits encoding not supported",
            ))
        }
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 10 {
            // 8 for min + 1 for max flag + 1 for shared
            return Err(wrt_error::Error::runtime_execution_error(
                "Insufficient bytes for limit deserialization: minimum 10 bytes required",
            ));
        }

        let min = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);

        let mut offset = 8;
        let max = if bytes[offset] != 0 {
            offset += 1;
            if bytes.len() < offset + 8 {
                return Err(wrt_error::Error::new(
                    wrt_error::ErrorCategory::Validation,
                    wrt_error::codes::PARSE_ERROR,
                    "Insufficient bytes for max limit value deserialization",
                ));
            }
            let max_val = u64::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            offset += 8;
            Some(max_val)
        } else {
            offset += 1;
            None
        };

        if bytes.len() < offset + 2 {
            return Err(wrt_error::Error::runtime_execution_error(
                "Insufficient bytes for flags",
            ));
        }

        let shared = bytes[offset] != 0;
        let memory64 = bytes[offset + 1] != 0;

        Ok(Self {
            min,
            max,
            shared,
            memory64,
        })
    }
}

// Implement Checksummable trait for Limits
impl wrt_foundation::traits::Checksummable for Limits {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.min.to_le_bytes());
        if let Some(max) = self.max {
            checksum.update_slice(&[1]);
            checksum.update_slice(&max.to_le_bytes());
        } else {
            checksum.update_slice(&[0]);
        }
        checksum.update_slice(&[self.shared as u8]);
        checksum.update_slice(&[self.memory64 as u8]);
    }
}

// Implement FromBytes trait for Limits
impl wrt_foundation::traits::FromBytes for Limits {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read the minimum size (8 bytes)
        let min = reader.read_u64_le()?;

        // Read max flag (1 byte)
        let has_max = reader.read_u8()? != 0;
        let max = if has_max { Some(reader.read_u64_le()?) } else { None };

        // Read flags (2 bytes)
        let shared = reader.read_u8()? != 0;
        let memory64 = reader.read_u8()? != 0;

        Ok(Self {
            min,
            max,
            shared,
            memory64,
        })
    }
}

// Implement ToBytes trait for Limits
impl wrt_foundation::traits::ToBytes for Limits {
    fn serialized_size(&self) -> usize {
        8 + if self.max.is_some() { 1 + 8 } else { 1 } + 2 // min + max_flag +
                                                           // max + shared +
                                                           // memory64
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write the minimum size (8 bytes)
        writer.write_all(&self.min.to_le_bytes())?;

        // Write max flag and value
        if let Some(max) = self.max {
            writer.write_u8(1)?;
            writer.write_all(&max.to_le_bytes())?;
        } else {
            writer.write_u8(0)?;
        }

        // Write flags
        writer.write_u8(self.shared as u8)?;
        writer.write_u8(self.memory64 as u8)?;

        Ok(())
    }
}
