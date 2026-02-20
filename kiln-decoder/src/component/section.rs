// WRT - wrt-decoder
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component Model section definitions
//!
//! This module provides type definitions for WebAssembly Component Model
//! sections and common structures used in component binary parsing.

use wrt_foundation::{
    MemoryProvider, NoStdProvider,
    bounded::{BoundedString, MAX_WASM_NAME_LENGTH},
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

/// Binary std/no_std choice
///
/// A simplified version of the wrt-foundation component::Export for
/// use in memory-constrained environments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentExport {
    /// Export name
    pub name: BoundedString<MAX_WASM_NAME_LENGTH>,
    /// Export type index
    pub type_index: u32,
    /// Export kind
    pub kind: u8,
}

/// Binary std/no_std choice
///
/// A simplified version of the wrt-foundation component::Import for
/// use in memory-constrained environments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentImport {
    /// Import name
    pub name: BoundedString<MAX_WASM_NAME_LENGTH>,
    /// Import type index
    pub type_index: u32,
}

/// Represents a Component section
///
/// This provides a common structure for all component sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSection {
    /// Section ID
    pub id: u8,
    /// Section size
    pub size: u32,
    /// Section payload offset
    pub offset: usize,
}

/// Component value types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentValueType {
    /// Primitive types
    Primitive = 0,
    /// Composite types
    Composite = 1,
    /// Resource types
    Resource = 2,
}

impl From<u8> for ComponentValueType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Primitive,
            1 => Self::Composite,
            2 => Self::Resource,
            _ => Self::Primitive, // Default to primitive for unknown values
        }
    }
}

/// Binary std/no_std choice
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentInstance {
    /// Instance type
    pub type_index: u32,
}

/// Binary std/no_std choice
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentType {
    /// The type form byte
    pub form: u8,
}

// Implement required traits for ComponentExport
impl Checksummable for ComponentExport {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum);
        self.type_index.update_checksum(checksum);
        self.kind.update_checksum(checksum);
    }
}

impl ToBytes for ComponentExport {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.type_index.to_bytes_with_provider(writer, provider)?;
        self.kind.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ComponentExport {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            name: BoundedString::from_bytes_with_provider(reader, provider)?,
            type_index: u32::from_bytes_with_provider(reader, provider)?,
            kind: u8::from_bytes_with_provider(reader, provider)?,
        })
    }
}

// Implement required traits for ComponentImport
impl Checksummable for ComponentImport {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum);
        self.type_index.update_checksum(checksum);
    }
}

impl ToBytes for ComponentImport {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.type_index.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ComponentImport {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            name: BoundedString::from_bytes_with_provider(reader, provider)?,
            type_index: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

// Implement required traits for ComponentSection
impl Checksummable for ComponentSection {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.id.update_checksum(checksum);
        self.size.update_checksum(checksum);
        (self.offset as u32).update_checksum(checksum);
    }
}

impl ToBytes for ComponentSection {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.id.to_bytes_with_provider(writer, provider)?;
        self.size.to_bytes_with_provider(writer, provider)?;
        (self.offset as u32).to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ComponentSection {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            id: u8::from_bytes_with_provider(reader, provider)?,
            size: u32::from_bytes_with_provider(reader, provider)?,
            offset: u32::from_bytes_with_provider(reader, provider)? as usize,
        })
    }
}

// Implement required traits for ComponentType
impl Checksummable for ComponentType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.form.update_checksum(checksum);
    }
}

impl ToBytes for ComponentType {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.form.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ComponentType {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            form: u8::from_bytes_with_provider(reader, provider)?,
        })
    }
}

// Implement required traits for ComponentInstance
impl Checksummable for ComponentInstance {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.type_index.update_checksum(checksum);
    }
}

impl ToBytes for ComponentInstance {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.type_index.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ComponentInstance {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            type_index: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

// Implement required traits for ComponentValueType
impl Default for ComponentValueType {
    fn default() -> Self {
        Self::Primitive
    }
}

impl Checksummable for ComponentValueType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        (*self as u8).update_checksum(checksum);
    }
}

impl ToBytes for ComponentValueType {
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        (*self as u8).to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ComponentValueType {
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = u8::from_bytes_with_provider(reader, provider)?;
        Ok(Self::from(byte))
    }
}
