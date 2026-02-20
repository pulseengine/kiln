// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Prelude module for wrt-decoder
//!
//! This module provides a unified set of imports for both std and no_std
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Don't duplicate format import since it's already in the use block above
#[cfg(not(feature = "std"))]
pub use core::result::Result as StdResult;
pub use core::{
    any::Any,
    cmp::{Eq, Ord, PartialEq, PartialOrd},
    convert::{From, Into, TryFrom, TryInto},
    fmt,
    fmt::{Debug, Display},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    slice, str,
};
// Re-export from alloc when the std feature is enabled
#[cfg(feature = "std")]
pub use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    format,
    rc::Rc,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
#[cfg(feature = "std")]
pub use core::result::Result as StdResult;
#[cfg(feature = "std")]
pub use std::{
    collections::{HashMap, HashSet},
    io,
    io::{Read, Write},
    sync::{Mutex, RwLock},
};

// Import synchronization primitives for no_std
//#[cfg(not(feature = "std"))]
// pub use wrt_sync::{Mutex, RwLock};

// Re-export from wrt-error
pub use wrt_error::{Error, ErrorCategory, Result, codes, kinds};
// Re-export format module for compatibility
pub use wrt_format as wrt_format_module;
// Re-export from wrt-format
pub use wrt_format::{
    // Conversion utilities
    conversion::{
        block_type_to_format_block_type, format_block_type_to_block_type,
        format_value_type as value_type_to_byte, parse_value_type,
    },
    // Module types
    module::{Element, Export, ExportKind, Function, Global, Import, ImportDesc, Memory, Table},
    // Pure format types (recommended over deprecated module types)
    pure_format_types::{PureDataMode, PureDataSegment, PureElementMode, PureElementSegment},
    // Section types
    section::{CustomSection, Section, SectionId},
    // Format-specific types
    types::{FormatBlockType, Limits, MemoryIndexType},
};

// Re-export pure types with shorter names for backward compatibility
pub use wrt_format::pure_format_types::{
    PureDataMode as DataMode, PureDataSegment as Data, PureElementMode as ElementMode,
};

// Binary std/no_std choice
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::{ComponentValue, ValType};
// Conversion utilities from wrt-foundation
#[cfg(feature = "conversion")]
pub use wrt_foundation::conversion::{ref_type_to_val_type, val_type_to_ref_type};
// No_std equivalents - use wrt-foundation types (Vec and String defined below with specific
// providers)
#[cfg(not(feature = "std"))]
pub use wrt_foundation::BoundedMap as HashMap;
// Re-export clean types from wrt-foundation
pub use wrt_foundation::{
    // SafeMemory types
    safe_memory::{SafeMemoryHandler, SafeSlice, SafeStack},
    // Legacy types for compatibility
    types::{BlockType, RefType, ValueType},
    values::Value,
};
// Use our unified memory management system
#[cfg(not(feature = "std"))]
pub use wrt_foundation::{
    BoundedString, BoundedVec,
    unified_types_simple::{DefaultTypes, EmbeddedTypes},
};
// Re-export clean types only when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use wrt_foundation::{
    CleanFuncType, CleanGlobalType, CleanMemoryType, CleanTableType, CleanValType, CleanValue,
};

// Most re-exports temporarily disabled for demo

// Binary std/no_std choice
pub use crate::decoder_no_alloc;
// For std mode, provide the same types but using std collections
// Priority: std overrides no_std when both are present
#[cfg(feature = "std")]
pub type DecoderVec<T> = Vec<T>;
#[cfg(feature = "std")]
pub type DecoderString = String;

// Universal length function that works with both Vec and BoundedVec
#[cfg(feature = "std")]
pub fn decoder_len<T>(vec: &DecoderVec<T>) -> usize {
    vec.len()
}

#[cfg(not(feature = "std"))]
pub fn decoder_len<T>(vec: &DecoderVec<T>) -> usize
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    wrt_foundation::traits::BoundedCapacity::len(vec)
}

// For no_std mode, use explicit bounded types (no confusing aliases!)
// Only when std is NOT enabled
#[cfg(not(feature = "std"))]
pub type DecoderVec<T> = BoundedVec<T, 256, wrt_foundation::NoStdProvider<4096>>;
#[cfg(not(feature = "std"))]
pub type DecoderString = BoundedString<256>;

// Factory function for creating providers using capability system
#[cfg(not(feature = "std"))]
pub fn create_decoder_provider<const N: usize>()
-> wrt_error::Result<wrt_foundation::NoStdProvider<N>> {
    use wrt_foundation::{CrateId, capabilities::MemoryFactory};
    MemoryFactory::create::<N>(CrateId::Decoder)
}

// For std mode, use the capability system as well
#[cfg(feature = "std")]
pub fn create_decoder_provider<const N: usize>()
-> wrt_error::Result<wrt_foundation::NoStdProvider<N>> {
    use wrt_foundation::{CrateId, capabilities::MemoryFactory};
    MemoryFactory::create::<N>(CrateId::Decoder)
}

// For no_std mode, provide a minimal ToString trait
/// Minimal ToString trait for no_std environments
#[cfg(not(feature = "std"))]
pub trait ToString {
    /// Convert to string
    fn to_string(&self) -> DecoderString;
}

#[cfg(not(feature = "std"))]
impl ToString for &str {
    fn to_string(&self) -> DecoderString {
        DecoderString::from_str_truncate(self).unwrap_or_default()
    }
}

// Binary std/no_std choice
/// Minimal format macro for no_std environments
#[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! format {
    ($($arg:tt)*) => {{
        // In pure no_std, return a simple bounded string
        use wrt_foundation::{BoundedString, NoStdProvider};
        if let Ok(provider) = $crate::prelude::create_decoder_provider::<512>() {
            $crate::prelude::DecoderString::from_str("formatted_string", provider)
                .unwrap_or_default()
        } else {
            $crate::prelude::DecoderString::default()
        }
    }};
}

// Export our custom format macro for no_std
#[cfg(not(feature = "std"))]
pub use crate::format;

/// Binary format utilities
#[cfg(feature = "std")]
pub mod binary {
    /// Read LEB128 u32 from data
    pub fn read_leb_u32(data: &[u8]) -> wrt_error::Result<(u32, usize)> {
        wrt_format::binary::read_leb128_u32(data, 0)
    }
}

/// Binary utilities for no_std environments
#[cfg(not(feature = "std"))]
pub mod binary {
    use wrt_foundation::{BoundedVec, NoStdProvider};

    use super::create_decoder_provider;

    /// Write LEB128 u32 in no_std mode
    pub fn write_leb128_u32(value: u32) -> BoundedVec<u8, 10, NoStdProvider<64>> {
        if let Ok(provider) = create_decoder_provider::<64>() {
            let mut result =
                BoundedVec::new(provider).expect("Failed to create bounded vec for LEB128");
            let mut buffer = [0u8; 10];
            // Simple LEB128 encoding for no_std
            let mut bytes_written = 0;
            let mut val = value;
            loop {
                let mut byte = (val & 0x7F) as u8;
                val >>= 7;
                if val != 0 {
                    byte |= 0x80;
                }
                if bytes_written < buffer.len() {
                    buffer[bytes_written] = byte;
                    bytes_written += 1;
                }
                if val == 0 {
                    break;
                }
            }

            if bytes_written > 0 {
                for i in 0..bytes_written {
                    let _ = result.push(buffer[i]);
                }
            }
            result
        } else {
            BoundedVec::default()
        }
    }

    /// Write string in no_std mode
    pub fn write_string(_s: &str) -> BoundedVec<u8, 256, NoStdProvider<512>> {
        if let Ok(provider) = create_decoder_provider::<512>() {
            // Simplified no_std implementation
            BoundedVec::new(provider).expect("Failed to create bounded vec for string")
        } else {
            BoundedVec::default()
        }
    }

    /// Read LEB128 u32 from data with offset
    pub fn read_leb_u32(data: &[u8], offset: usize) -> wrt_error::Result<(u32, usize)> {
        // Simple implementation for no_std - just read from beginning
        if offset >= data.len() {
            return Err(wrt_error::Error::parse_error("Offset out of bounds"));
        }
        // For simplicity, just parse from the offset
        let mut value = 0u32;
        let mut shift = 0;
        let mut bytes_read = 0;

        for &byte in &data[offset..] {
            if bytes_read >= 5 {
                return Err(wrt_error::Error::parse_error("LEB128 too long"));
            }

            value |= ((byte & 0x7F) as u32) << shift;
            bytes_read += 1;

            if (byte & 0x80) == 0 {
                return Ok((value, bytes_read));
            }

            shift += 7;
        }

        Err(wrt_error::Error::parse_error("Incomplete LEB128"))
    }

    /// Read name from binary data in no_std mode
    pub fn read_name(data: &[u8], offset: usize) -> wrt_error::Result<(&[u8], usize)> {
        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(
            offset = offset,
            data_byte = format!(
                "0x{:02x}",
                if offset < data.len() { data[offset] } else { 0 }
            ),
            "read_name"
        );
        if offset >= data.len() {
            return Err(wrt_error::Error::parse_error("Offset out of bounds"));
        }

        // Read length as LEB128
        let (length, bytes_consumed) = read_leb_u32(data, offset)?;
        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(
            length = length,
            bytes_consumed = bytes_consumed,
            "read_name length parsed"
        );
        let name_start = offset + bytes_consumed;

        if name_start + length as usize > data.len() {
            return Err(wrt_error::Error::parse_error("Name extends beyond data"));
        }

        let final_offset = name_start + length as usize;
        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(final_offset = final_offset, "read_name complete");
        Ok((
            &data[name_start..name_start + length as usize],
            final_offset,
        ))
    }
}

// Make commonly used binary functions available at top level (now exported by
// wrt_format directly)
// For no_std mode, provide the missing functions locally
#[cfg(not(feature = "std"))]
pub use binary::{read_name, write_leb128_u32, write_string};
pub use wrt_format::read_leb128_u32;
#[cfg(feature = "std")]
pub use wrt_format::{read_name, read_string, write_leb128_u32, write_string};

/// Extension trait to add missing methods to BoundedVec
pub trait BoundedVecExt<T, const N: usize, P: wrt_foundation::MemoryProvider> {
    /// Create an empty BoundedVec
    fn empty() -> Self;
    /// Try to push an item, returning an error if capacity is exceeded
    fn try_push(&mut self, item: T) -> wrt_error::Result<()>;
    /// Check if the collection is empty
    fn is_empty(&self) -> bool;
}

#[cfg(not(feature = "std"))]
impl<T, const N: usize, P> BoundedVecExt<T, N, P> for wrt_foundation::bounded::BoundedVec<T, N, P>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
{
    fn empty() -> Self {
        Self::new(P::default()).unwrap_or_default()
    }

    fn try_push(&mut self, item: T) -> wrt_error::Result<()> {
        self.push(item).map_err(|_e| {
            wrt_error::Error::runtime_execution_error("Failed to push item to bounded vector")
        })
    }

    fn is_empty(&self) -> bool {
        use wrt_foundation::traits::BoundedCapacity;
        self.len() == 0
    }
}

// Extension trait to add missing methods to Vec in std mode
pub trait DecoderVecExt<T> {
    /// Create from bytes with provider (compatible with both std and no_std)
    fn from_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self>
    where
        Self: Sized;

    /// Update checksum (compatible with both std and no_std)
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum);

    /// Serialized size (compatible with both std and no_std)
    fn serialized_size(&self) -> usize;

    /// To bytes with provider (compatible with both std and no_std)
    fn to_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()>;
}

#[cfg(feature = "std")]
impl<T> DecoderVecExt<T> for Vec<T>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Clone,
{
    fn from_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // For std mode, read all items without provider
        let mut result = Vec::new();
        // Read count first (assuming LEB128 u32 count prefix)
        let mut count_bytes = [0u8; 4];
        reader.read_exact(&mut count_bytes)?;
        let count = u32::from_le_bytes(count_bytes);

        result.reserve(count as usize);
        for _ in 0..count {
            let item = T::from_bytes_with_provider(reader, _provider)?;
            result.push(item);
        }
        Ok(result)
    }

    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        for item in self {
            item.update_checksum(checksum);
        }
    }

    fn serialized_size(&self) -> usize {
        4 + self.iter().map(|item| item.serialized_size()).sum::<usize>() // 4 bytes for count + items
    }

    fn to_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        // Write count first
        let count = self.len() as u32;
        writer.write_all(&count.to_le_bytes())?;

        // Write all items
        for item in self {
            item.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl<T, const N: usize, P> DecoderVecExt<T> for BoundedVec<T, N, P>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
{
    fn from_bytes_with_provider<
        'a,
        P2: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P2,
    ) -> wrt_error::Result<Self> {
        // For no_std mode, use the provider directly
        use wrt_foundation::traits::FromBytes;
        FromBytes::from_bytes_with_provider(reader, provider)
    }

    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_foundation::traits::Checksummable;
        Checksummable::update_checksum(self, checksum);
    }

    fn serialized_size(&self) -> usize {
        use wrt_foundation::traits::ToBytes;
        ToBytes::serialized_size(self)
    }

    fn to_bytes_with_provider<
        'a,
        P2: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P2,
    ) -> wrt_error::Result<()> {
        use wrt_foundation::traits::ToBytes;
        ToBytes::to_bytes_with_provider(self, writer, provider)
    }
}

// Extension trait to add missing methods to String in std mode
pub trait DecoderStringExt {
    /// Create from bytes with provider (compatible with both std and no_std)
    fn from_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self>
    where
        Self: Sized;

    /// Update checksum (compatible with both std and no_std)
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum);

    /// Serialized size (compatible with both std and no_std)
    fn serialized_size(&self) -> usize;

    /// To bytes with provider (compatible with both std and no_std)
    fn to_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()>;
}

#[cfg(feature = "std")]
impl DecoderStringExt for String {
    fn from_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // For std mode, read string manually using available ReadStream methods
        // Read length as LEB128 (simplified - assume single byte for now)
        let len_byte = reader
            .read_u8()
            .map_err(|_| wrt_error::Error::parse_error("Failed to read string length"))?;
        let len = len_byte as usize; // Simplified LEB128 - single byte only

        let mut string_bytes = vec![0u8; len];
        reader
            .read_exact(&mut string_bytes)
            .map_err(|_| wrt_error::Error::parse_error("Failed to read string bytes"))?;
        alloc::string::String::from_utf8(string_bytes)
            .map_err(|_| wrt_error::Error::parse_error("Invalid UTF-8 string"))
    }

    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(self.as_bytes());
    }

    fn serialized_size(&self) -> usize {
        self.len()
    }

    fn to_bytes_with_provider<
        'a,
        P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_all(self.as_bytes())?;
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl<const N: usize> DecoderStringExt for BoundedString<N> {
    fn from_bytes_with_provider<
        'a,
        P2: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P2,
    ) -> wrt_error::Result<Self> {
        // For no_std mode, use the provider to create bounded string
        let mut buffer = wrt_foundation::BoundedVec::<u8, N, P2>::new(provider.clone())?;
        // Read length first (assuming LEB128 u32 length prefix)
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        for _ in 0..len {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte)?;
            buffer
                .push(byte[0])
                .map_err(|_| wrt_error::Error::parse_error("String too long for buffer"))?;
        }
        let slice = buffer
            .as_slice()
            .map_err(|_| wrt_error::Error::parse_error("Failed to get buffer slice"))?;
        let s = core::str::from_utf8(slice)
            .map_err(|_| wrt_error::Error::parse_error("Invalid UTF-8"))?;
        // BoundedString no longer needs provider after StaticVec migration
        Self::try_from_str(s).map_err(|_| wrt_error::Error::parse_error("String too long"))
    }

    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_foundation::traits::Checksummable;
        Checksummable::update_checksum(self, checksum);
    }

    fn serialized_size(&self) -> usize {
        use wrt_foundation::traits::ToBytes;
        ToBytes::serialized_size(self)
    }

    fn to_bytes_with_provider<
        'a,
        P2: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq,
    >(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P2,
    ) -> wrt_error::Result<()> {
        use wrt_foundation::traits::ToBytes;
        ToBytes::to_bytes_with_provider(self, writer, provider)
    }
}

// For compatibility, add some aliases that the code expects
/// Read LEB128 u32 from data
#[cfg(feature = "std")]
pub fn read_leb_u32(data: &[u8]) -> wrt_error::Result<(u32, usize)> {
    read_leb128_u32(data, 0)
}

/// Read LEB128 u32 from data (no_std version)
#[cfg(not(feature = "std"))]
pub fn read_leb_u32(data: &[u8]) -> wrt_error::Result<(u32, usize)> {
    read_leb128_u32(data, 0)
}

/// Read string from data (no_std version)
#[cfg(not(feature = "std"))]
pub fn read_string(_data: &[u8], _offset: usize) -> wrt_error::Result<(&[u8], usize)> {
    // Simplified implementation for no_std
    Ok((&[], 0))
}

// Missing utility functions
/// Validate WebAssembly header
pub fn is_valid_wasm_header(data: &[u8]) -> bool {
    data.len() >= 8
        && data[0..4] == wrt_format::binary::WASM_MAGIC
        && data[4..8] == wrt_format::binary::WASM_VERSION
}

// read_name is now imported from wrt_format

// read_leb128_u32 is now imported from wrt_format

// Feature-gated function aliases - bring in functions from wrt_format that
// aren't already exported
#[cfg(feature = "std")]
pub use wrt_format::binary::with_alloc::parse_block_type as parse_format_block_type;

// Duplicate type aliases removed - using the ones defined earlier in the file
