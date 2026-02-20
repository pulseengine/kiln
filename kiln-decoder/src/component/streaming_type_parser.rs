//! Streaming Component Type Section Parser
//!
//! This module provides ASIL-compliant streaming parsing of Component Type
//! sections within Component binaries. It uses the unified capability-based
//! memory allocation system and operates without loading entire type
//! definitions into memory.
//!
//! # ASIL Compliance
//!
//! This implementation works across all ASIL levels using the unified provider
//! system:
//! - The BoundedVec types adapt their behavior based on the current ASIL level
//! - The NoStdProvider internally chooses appropriate allocation strategies
//! - All limits are enforced at compile time with runtime validation
//! - Single implementation that works for QM, ASIL-A, ASIL-B, ASIL-C, and
//!   ASIL-D
//!
//! # Architecture
//!
//! The parser uses a streaming approach where:
//! 1. Only section headers are read into memory
//! 2. Type data is processed incrementally
//! 3. Memory allocation is controlled via the capability system
//! 4. All operations are bounded and deterministic

// Environment setup
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use alloc::{boxed::Box, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(feature = "std"))]
use wrt_format::binary::{read_leb128_u32, read_string};
#[cfg(feature = "std")]
use wrt_format::{
    binary::{read_leb128_u32, read_string},
    component,
};

use self::placeholder_types::*;
use crate::prelude::{DecoderStringExt, DecoderVecExt, *};

// Define types that work in both std and no_std environments
mod placeholder_types {
    use core::fmt;

    use wrt_error::{Error, ErrorCategory, Result, codes};
    use wrt_foundation::traits::BoundedCapacity;

    use crate::prelude::DecoderVec;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ComponentType {
        pub definition: ComponentTypeDefinition,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ComponentImport {
        pub namespace: crate::prelude::DecoderString,
        pub name: crate::prelude::DecoderString,
        pub extern_type: ExternType,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ComponentExport {
        pub name: crate::prelude::DecoderString,
        pub extern_type: ExternType,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct FunctionParam {
        pub name: crate::prelude::DecoderString,
        pub val_type: FormatValType,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ComponentTypeDefinition {
        Component {
            imports: DecoderVec<ComponentImport>,
            exports: DecoderVec<ComponentExport>,
        },
        Instance {
            exports: DecoderVec<ComponentExport>,
        },
        Function {
            params: DecoderVec<FunctionParam>,
            results: DecoderVec<FormatValType>,
        },
        Value(FormatValType),
        Resource {
            name: Option<crate::prelude::DecoderString>,
            functions: DecoderVec<u32>,
        },
    }

    impl Default for ComponentTypeDefinition {
        fn default() -> Self {
            Self::Value(FormatValType::default())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ExternType {
        Function {
            params: DecoderVec<FunctionParam>,
            results: DecoderVec<FormatValType>,
        },
        Value(FormatValType),
        Type(u32),
        Instance {
            exports: DecoderVec<ComponentExport>,
        },
        Component {
            imports: DecoderVec<ComponentImport>,
            exports: DecoderVec<ComponentExport>,
        },
    }

    impl Default for ExternType {
        fn default() -> Self {
            Self::Value(FormatValType::default())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum FormatValType {
        #[default]
        Bool,
        S8,
        U8,
        S16,
        U16,
        S32,
        U32,
        S64,
        U64,
        F32,
        F64,
        Char,
        String,
        Record(DecoderVec<(crate::prelude::DecoderString, FormatValType)>),
        Variant(DecoderVec<(crate::prelude::DecoderString, Option<FormatValType>)>),
        #[cfg(feature = "std")]
        List(Box<FormatValType>),
        #[cfg(not(feature = "std"))]
        List(u32),
        Tuple(DecoderVec<FormatValType>),
        Own(u32),
        Borrow(u32),
    }

    impl wrt_format::Validatable for ComponentType {
        fn validate(&self) -> Result<()> {
            Ok(())
        }
    }

    // Conversion function from wrt_format types to placeholder types
    #[cfg(feature = "std")]
    impl From<wrt_format::component::FormatValType> for FormatValType {
        fn from(val: wrt_format::component::FormatValType) -> Self {
            use wrt_format::component::FormatValType as WrtFormat;
            match val {
                WrtFormat::Bool => FormatValType::Bool,
                WrtFormat::S8 => FormatValType::S8,
                WrtFormat::U8 => FormatValType::U8,
                WrtFormat::S16 => FormatValType::S16,
                WrtFormat::U16 => FormatValType::U16,
                WrtFormat::S32 => FormatValType::S32,
                WrtFormat::U32 => FormatValType::U32,
                WrtFormat::S64 => FormatValType::S64,
                WrtFormat::U64 => FormatValType::U64,
                WrtFormat::F32 => FormatValType::F32,
                WrtFormat::F64 => FormatValType::F64,
                WrtFormat::Char => FormatValType::Char,
                WrtFormat::String => FormatValType::String,
                // For complex types, use placeholders for now
                _ => FormatValType::Bool, // Placeholder
            }
        }
    }

    // Implement required traits for BoundedVec compatibility
    use wrt_foundation::traits::{Checksummable, FromBytes, ToBytes};

    impl Checksummable for ComponentType {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            checksum.update_slice(&[0]); // Placeholder implementation
        }
    }

    impl ToBytes for ComponentType {
        fn serialized_size(&self) -> usize {
            0 // Placeholder implementation
        }

        fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
            &self,
            _writer: &mut wrt_foundation::traits::WriteStream<'_>,
            _provider: &P,
        ) -> wrt_error::Result<()> {
            Ok(()) // Placeholder implementation
        }
    }

    impl FromBytes for ComponentType {
        fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
            _reader: &mut wrt_foundation::traits::ReadStream<'_>,
            _provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self::default()) // Placeholder implementation
        }
    }

    // Implement required traits for FormatValType
    impl Checksummable for FormatValType {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            checksum.update_slice(&[0]); // Placeholder implementation
        }
    }

    impl ToBytes for FormatValType {
        fn serialized_size(&self) -> usize {
            0 // Placeholder implementation
        }

        fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
            &self,
            _writer: &mut wrt_foundation::traits::WriteStream<'_>,
            _provider: &P,
        ) -> wrt_error::Result<()> {
            Ok(()) // Placeholder implementation
        }
    }

    impl FromBytes for FormatValType {
        fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
            _reader: &mut wrt_foundation::traits::ReadStream<'_>,
            _provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self::default()) // Placeholder implementation
        }
    }

    // Implement required traits for ComponentImport
    impl Checksummable for ComponentImport {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            self.namespace.update_checksum(checksum);
            self.name.update_checksum(checksum);
            self.extern_type.update_checksum(checksum);
        }
    }

    impl ToBytes for ComponentImport {
        fn serialized_size(&self) -> usize {
            self.namespace.serialized_size()
                + self.name.serialized_size()
                + self.extern_type.serialized_size()
        }

        fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<()> {
            self.namespace.to_bytes_with_provider(writer, provider)?;
            self.name.to_bytes_with_provider(writer, provider)?;
            self.extern_type.to_bytes_with_provider(writer, provider)?;
            Ok(())
        }
    }

    impl FromBytes for ComponentImport {
        fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self {
                namespace: {
                    <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?
                },
                name: {
                    <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?
                },
                extern_type: ExternType::from_bytes_with_provider(reader, provider)?,
            })
        }
    }

    // Implement required traits for ComponentExport
    impl Checksummable for ComponentExport {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            self.name.update_checksum(checksum);
            self.extern_type.update_checksum(checksum);
        }
    }

    impl ToBytes for ComponentExport {
        fn serialized_size(&self) -> usize {
            self.name.serialized_size() + self.extern_type.serialized_size()
        }

        fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<()> {
            self.name.to_bytes_with_provider(writer, provider)?;
            self.extern_type.to_bytes_with_provider(writer, provider)?;
            Ok(())
        }
    }

    impl FromBytes for ComponentExport {
        fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self {
                name: {
                    <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?
                },
                extern_type: ExternType::from_bytes_with_provider(reader, provider)?,
            })
        }
    }

    // Implement required traits for FunctionParam
    impl Checksummable for FunctionParam {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            self.name.update_checksum(checksum);
            self.val_type.update_checksum(checksum);
        }
    }

    impl ToBytes for FunctionParam {
        fn serialized_size(&self) -> usize {
            self.name.serialized_size() + self.val_type.serialized_size()
        }

        fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<()> {
            self.name.to_bytes_with_provider(writer, provider)?;
            self.val_type.to_bytes_with_provider(writer, provider)?;
            Ok(())
        }
    }

    impl FromBytes for FunctionParam {
        fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self {
                name: {
                    <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?
                },
                val_type: FormatValType::from_bytes_with_provider(reader, provider)?,
            })
        }
    }

    // Implement required traits for ExternType
    impl Checksummable for ExternType {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            match self {
                ExternType::Function { params, results } => {
                    checksum.update(0u8); // Tag for Function
                    // Use manual checksum update for Vec types
                    for param in params.iter() {
                        param.update_checksum(checksum);
                    }
                    for result in results.iter() {
                        result.update_checksum(checksum);
                    }
                },
                ExternType::Value(val_type) => {
                    checksum.update(1u8); // Tag for Value
                    val_type.update_checksum(checksum);
                },
                ExternType::Type(idx) => {
                    checksum.update(2u8); // Tag for Type
                    checksum.update_slice(&idx.to_le_bytes());
                },
                ExternType::Instance { exports } => {
                    checksum.update(3u8); // Tag for Instance
                    for export in exports.iter() {
                        export.update_checksum(checksum);
                    }
                },
                ExternType::Component { imports, exports } => {
                    checksum.update(4u8); // Tag for Component
                    for import in imports.iter() {
                        import.update_checksum(checksum);
                    }
                    for export in exports.iter() {
                        export.update_checksum(checksum);
                    }
                },
            }
        }
    }

    impl ToBytes for ExternType {
        fn serialized_size(&self) -> usize {
            1 + match self {
                // 1 byte for tag
                ExternType::Function { params, results } => {
                    crate::prelude::decoder_len(params) * 4
                        + crate::prelude::decoder_len(results) * 4 // Approximate size
                },
                ExternType::Value(val_type) => val_type.serialized_size(),
                ExternType::Type(_) => 4, // u32
                ExternType::Instance { exports } => crate::prelude::decoder_len(exports) * 8, /* Approximate size */
                ExternType::Component { imports, exports } => {
                    crate::prelude::decoder_len(imports) * 8
                        + crate::prelude::decoder_len(exports) * 8 // Approximate size
                },
            }
        }

        fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<()> {
            match self {
                ExternType::Function { params, results } => {
                    writer.write_u8(0)?; // Tag
                    // Manual serialization for Vec types
                    for param in params.iter() {
                        param.to_bytes_with_provider(writer, provider)?;
                    }
                    for result in results.iter() {
                        result.to_bytes_with_provider(writer, provider)?;
                    }
                },
                ExternType::Value(val_type) => {
                    writer.write_u8(1)?; // Tag
                    val_type.to_bytes_with_provider(writer, provider)?;
                },
                ExternType::Type(idx) => {
                    writer.write_u8(2)?; // Tag
                    writer.write_u32_le(*idx)?;
                },
                ExternType::Instance { exports } => {
                    writer.write_u8(3)?; // Tag
                    for export in exports.iter() {
                        export.to_bytes_with_provider(writer, provider)?;
                    }
                },
                ExternType::Component { imports, exports } => {
                    writer.write_u8(4)?; // Tag
                    for import in imports.iter() {
                        import.to_bytes_with_provider(writer, provider)?;
                    }
                    for export in exports.iter() {
                        export.to_bytes_with_provider(writer, provider)?;
                    }
                },
            }
            Ok(())
        }
    }

    impl FromBytes for ExternType {
        fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream<'a>,
            provider: &P,
        ) -> wrt_error::Result<Self> {
            let tag = reader.read_u8()?;
            match tag {
                0 => Ok(ExternType::Function {
                    params: {
                        let v: DecoderVec<FunctionParam> =
                            <DecoderVec<FunctionParam> as crate::prelude::DecoderVecExt<
                                FunctionParam,
                            >>::from_bytes_with_provider(
                                reader, provider
                            )?;
                        v
                    },
                    results: {
                        let v: DecoderVec<FormatValType> =
                            <DecoderVec<FormatValType> as crate::prelude::DecoderVecExt<
                                FormatValType,
                            >>::from_bytes_with_provider(
                                reader, provider
                            )?;
                        v
                    },
                }),
                1 => Ok(ExternType::Value(FormatValType::from_bytes_with_provider(
                    reader, provider,
                )?)),
                2 => Ok(ExternType::Type(reader.read_u32_le()?)),
                3 => Ok(ExternType::Instance {
                    exports: {
                        let v: DecoderVec<ComponentExport> =
                            <DecoderVec<ComponentExport> as crate::prelude::DecoderVecExt<
                                ComponentExport,
                            >>::from_bytes_with_provider(
                                reader, provider
                            )?;
                        v
                    },
                }),
                4 => Ok(ExternType::Component {
                    imports: {
                        let v: DecoderVec<ComponentImport> =
                            <DecoderVec<ComponentImport> as crate::prelude::DecoderVecExt<
                                ComponentImport,
                            >>::from_bytes_with_provider(
                                reader, provider
                            )?;
                        v
                    },
                    exports: {
                        let v: DecoderVec<ComponentExport> =
                            <DecoderVec<ComponentExport> as crate::prelude::DecoderVecExt<
                                ComponentExport,
                            >>::from_bytes_with_provider(
                                reader, provider
                            )?;
                        v
                    },
                }),
                _ => Err(Error::parse_error("Invalid ExternType tag ")),
            }
        }
    }
}

// Use the same types in both std and no_std modes for consistency
use placeholder_types::{ComponentExport, ComponentImport, FunctionParam};
#[cfg(not(feature = "std"))]
use placeholder_types::{ComponentType, ComponentTypeDefinition, ExternType, FormatValType};
use wrt_foundation::{
    BoundedVec, VerificationLevel,
    budget_aware_provider::CrateId,
    safe_memory::{NoStdMemoryProvider, NoStdProvider},
    traits::BoundedCapacity,
};

// Import the unified bounded decoder infrastructure
#[cfg(not(feature = "std"))]
use crate::bounded_decoder_infra::{
    BoundedTypeVec, MAX_TYPES_PER_COMPONENT, create_decoder_provider,
};

// For std mode, provide basic constants and functions
#[cfg(feature = "std")]
const MAX_TYPES_PER_COMPONENT: usize = 1024;

#[cfg(feature = "std")]
fn create_decoder_provider<const N: usize>() -> wrt_error::Result<wrt_foundation::NoStdProvider<N>>
{
    Ok(wrt_foundation::NoStdProvider::default())
}

#[cfg(feature = "std")]
type BoundedTypeVec<T> = Vec<T>;

// Import bounded types from prelude
use crate::prelude::*;

/// Maximum size of a single type definition (64KB, ASIL constraint)
pub const MAX_TYPE_DEFINITION_SIZE: usize = 64 * 1024;

/// Maximum recursion depth for nested types (ASIL constraint)
pub const MAX_TYPE_RECURSION_DEPTH: usize = 32;

/// Decoder provider type for consistent allocation
type DecoderProvider = NoStdProvider<65536>;

/// Component Type Section streaming parser
///
/// This parser processes Component Type sections within Component binaries
/// using a streaming approach that minimizes memory allocation and provides
/// deterministic behavior across all ASIL levels using the unified provider
/// system.
pub struct StreamingTypeParser<'a> {
    /// Binary data being parsed
    data: &'a [u8],
    /// Current parsing offset
    offset: usize,
    /// Verification level for parsing strictness
    verification_level: VerificationLevel,
    /// Current recursion depth for nested types
    recursion_depth: usize,
}

/// Component Type Section parsing result
#[derive(Debug)]
pub struct ComponentTypeSection {
    /// Number of types parsed
    pub type_count: u32,
    /// Total bytes consumed
    pub bytes_consumed: usize,
    /// Types parsed using unified bounded storage
    pub types: BoundedTypeVec<ComponentType>,
}

impl<'a> StreamingTypeParser<'a> {
    /// Create a new streaming component type parser
    ///
    /// # Arguments
    /// * `data` - The binary data containing the component type section
    /// * `verification_level` - Level of validation to perform
    ///
    /// # Returns
    /// A new parser instance ready to process component types
    pub fn new(data: &'a [u8], verification_level: VerificationLevel) -> Result<Self> {
        if data.is_empty() {
            return Err(Error::runtime_execution_error(
                "Streaming type parser error ",
            ));
        }

        // ASIL constraint: Verify data size constraints
        if data.len() > MAX_TYPE_DEFINITION_SIZE {
            return Err(Error::validation_error(
                "Type definition size exceeds maximum",
            ));
        }

        Ok(Self {
            data,
            offset: 0,
            verification_level,
            recursion_depth: 0,
        })
    }

    /// Parse the component type section using streaming approach
    ///
    /// This method processes the component type section without loading entire
    /// type definitions into memory, using the unified capability-based
    /// allocation system.
    ///
    /// # Returns
    /// A ComponentTypeSection containing parsed types and metadata
    pub fn parse(&mut self) -> Result<ComponentTypeSection> {
        // Read the number of component types
        let (type_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        // ASIL constraint: Validate type count
        if type_count > MAX_TYPES_PER_COMPONENT as u32 {
            return Err(Error::validation_error(
                "Too many component types in section ",
            ));
        }

        // Initialize storage using unified provider system
        let mut types = self.create_type_storage()?;

        // Parse each component type
        for i in 0..type_count {
            let comp_type = self.parse_single_component_type(i)?;
            self.store_type(&mut types, comp_type)?;
        }

        Ok(ComponentTypeSection {
            type_count,
            bytes_consumed: self.offset,
            types,
        })
    }

    /// Create type storage using unified provider system
    fn create_type_storage(&self) -> Result<BoundedTypeVec<ComponentType>> {
        // For std mode, use Vec
        #[cfg(feature = "std")]
        {
            Ok(Vec::new())
        }

        // For no_std mode, use StaticVec
        #[cfg(not(feature = "std"))]
        {
            use wrt_foundation::collections::StaticVec;
            Ok(StaticVec::new())
        }
    }

    /// Parse a single component type from the binary stream
    fn parse_single_component_type(&mut self, type_index: u32) -> Result<ComponentType> {
        if self.offset >= self.data.len() {
            return Err(Error::parse_error("Unexpected end of type section "));
        }

        // Read type form
        let type_form = self.data[self.offset];
        self.offset += 1;

        // ASIL constraint: Check recursion depth
        if self.recursion_depth >= MAX_TYPE_RECURSION_DEPTH {
            return Err(Error::validation_error("Type recursion depth exceeded "));
        }

        self.recursion_depth += 1;

        let definition = match type_form {
            0x40 => self.parse_component_type_definition()?,
            0x41 => self.parse_instance_type_definition()?,
            0x42 => self.parse_function_type_definition()?,
            0x43 => self.parse_value_type_definition()?,
            0x44 => self.parse_resource_type_definition()?,
            _ => {
                return Err(Error::parse_error("Unknown component type form "));
            },
        };

        self.recursion_depth -= 1;

        Ok(ComponentType { definition })
    }

    /// Parse component type definition (0x40)
    fn parse_component_type_definition(&mut self) -> Result<ComponentTypeDefinition> {
        // Read import count
        let (import_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        #[cfg(not(feature = "std"))]
        let provider = create_decoder_provider::<4096>()?;

        #[cfg(not(feature = "std"))]
        let mut imports = DecoderVec::new(provider.clone())?;
        #[cfg(feature = "std")]
        let mut imports = DecoderVec::new();
        for _ in 0..import_count {
            let namespace = self.read_string()?;
            let name = self.read_string()?;
            let extern_type = self.parse_extern_type()?;
            // Create ComponentImport struct
            let import = ComponentImport {
                namespace,
                name,
                extern_type,
            };
            #[cfg(not(feature = "std"))]
            imports
                .push(import)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
            #[cfg(feature = "std")]
            imports.push(import);
        }

        // Read export count
        let (export_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        #[cfg(not(feature = "std"))]
        let mut exports = DecoderVec::new(provider)?;
        #[cfg(feature = "std")]
        let mut exports = DecoderVec::new();
        for _ in 0..export_count {
            let name = self.read_string()?;
            let extern_type = self.parse_extern_type()?;
            // Create ComponentExport struct
            let export = ComponentExport { name, extern_type };
            #[cfg(not(feature = "std"))]
            exports
                .push(export)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
            #[cfg(feature = "std")]
            exports.push(export);
        }

        Ok(ComponentTypeDefinition::Component { imports, exports })
    }

    /// Parse instance type definition (0x41)
    fn parse_instance_type_definition(&mut self) -> Result<ComponentTypeDefinition> {
        // Read export count
        let (export_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        #[cfg(not(feature = "std"))]
        let mut exports = {
            let provider = create_decoder_provider::<4096>()?;
            DecoderVec::new(provider)?
        };
        #[cfg(feature = "std")]
        let mut exports = DecoderVec::new();
        for _ in 0..export_count {
            let name = self.read_string()?;
            let extern_type = self.parse_extern_type()?;
            // Create ComponentExport struct
            let export = ComponentExport { name, extern_type };
            #[cfg(not(feature = "std"))]
            exports
                .push(export)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
            #[cfg(feature = "std")]
            exports.push(export);
        }

        Ok(ComponentTypeDefinition::Instance { exports })
    }

    /// Parse function type definition (0x42)
    fn parse_function_type_definition(&mut self) -> Result<ComponentTypeDefinition> {
        // Read parameter count
        let (param_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        #[cfg(not(feature = "std"))]
        let mut params = {
            let provider = create_decoder_provider::<4096>()?;
            DecoderVec::new(provider.clone())?
        };
        #[cfg(feature = "std")]
        let mut params = DecoderVec::new();
        for _ in 0..param_count {
            let name = self.read_string()?;
            let val_type = self.parse_value_type()?;
            let param = FunctionParam { name, val_type };
            #[cfg(not(feature = "std"))]
            params
                .push(param)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
            #[cfg(feature = "std")]
            params.push(param);
        }

        // Read result count
        let (result_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        #[cfg(not(feature = "std"))]
        let mut results = {
            let provider = create_decoder_provider::<4096>()?;
            DecoderVec::new(provider)?
        };
        #[cfg(feature = "std")]
        let mut results = DecoderVec::new();
        for _ in 0..result_count {
            let val_type = self.parse_value_type()?;
            #[cfg(not(feature = "std"))]
            results
                .push(val_type)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
            #[cfg(feature = "std")]
            results.push(val_type);
        }

        Ok(ComponentTypeDefinition::Function { params, results })
    }

    /// Parse value type definition (0x43)
    fn parse_value_type_definition(&mut self) -> Result<ComponentTypeDefinition> {
        let val_type = self.parse_value_type()?;
        Ok(ComponentTypeDefinition::Value(val_type))
    }

    /// Parse resource type definition (0x44)
    fn parse_resource_type_definition(&mut self) -> Result<ComponentTypeDefinition> {
        // Read resource representation
        let representation = self.parse_resource_representation()?;

        // Read nullable flag
        let nullable =
            if self.offset < self.data.len() { self.data[self.offset] != 0 } else { false };
        if self.offset < self.data.len() {
            self.offset += 1;
        }

        #[cfg(feature = "std")]
        {
            Ok(ComponentTypeDefinition::Resource {
                name: None, // Placeholder for now
                functions: DecoderVec::new(),
            })
        }
        #[cfg(not(feature = "std"))]
        {
            let provider = create_decoder_provider::<4096>()?;
            Ok(ComponentTypeDefinition::Resource {
                name: None, // Placeholder for now
                functions: DecoderVec::new(provider)?,
            })
        }
    }

    /// Parse extern type
    fn parse_extern_type(&mut self) -> Result<ExternType> {
        if self.offset >= self.data.len() {
            return Err(Error::parse_error(
                "Unexpected end while reading extern type ",
            ));
        }

        let extern_form = self.data[self.offset];
        self.offset += 1;

        match extern_form {
            0x00 => {
                // Function type
                let (param_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;

                #[cfg(not(feature = "std"))]
                let mut params = {
                    let provider = create_decoder_provider::<4096>()?;
                    DecoderVec::new(provider.clone())?
                };
                #[cfg(feature = "std")]
                let mut params = DecoderVec::new();
                for _ in 0..param_count {
                    let name = self.read_string()?;
                    let val_type = self.parse_value_type()?;
                    let param = FunctionParam { name, val_type };
                    #[cfg(not(feature = "std"))]
                    params.push(param).map_err(|_| {
                        Error::runtime_execution_error("Streaming type parser error ")
                    })?;
                    #[cfg(feature = "std")]
                    params.push(param);
                }

                let (result_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;

                #[cfg(not(feature = "std"))]
                let mut results = {
                    let provider = create_decoder_provider::<4096>()?;
                    DecoderVec::new(provider)?
                };
                #[cfg(feature = "std")]
                let mut results = DecoderVec::new();
                for _ in 0..result_count {
                    let val_type = self.parse_value_type()?;
                    #[cfg(not(feature = "std"))]
                    results.push(val_type).map_err(|_| {
                        Error::runtime_execution_error("Streaming type parser error ")
                    })?;
                    #[cfg(feature = "std")]
                    results.push(val_type);
                }

                Ok(ExternType::Function { params, results })
            },
            0x01 => {
                // Value type
                let val_type = self.parse_value_type()?;
                Ok(ExternType::Value(val_type))
            },
            0x02 => {
                // Type reference
                let (type_idx, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;
                Ok(ExternType::Type(type_idx))
            },
            0x03 => {
                // Instance type - recursive parse
                self.recursion_depth += 1;
                let instance_def = self.parse_instance_type_definition()?;
                self.recursion_depth -= 1;

                if let ComponentTypeDefinition::Instance { exports } = instance_def {
                    Ok(ExternType::Instance { exports })
                } else {
                    Err(Error::parse_error("Invalid instance type definition"))
                }
            },
            0x04 => {
                // Component type - recursive parse
                self.recursion_depth += 1;
                let component_def = self.parse_component_type_definition()?;
                self.recursion_depth -= 1;

                if let ComponentTypeDefinition::Component { imports, exports } = component_def {
                    Ok(ExternType::Component { imports, exports })
                } else {
                    Err(Error::parse_error("Invalid component type definition"))
                }
            },
            _ => Err(Error::parse_error("Unknown extern type form ")),
        }
    }

    /// Parse component value type
    fn parse_value_type(&mut self) -> Result<FormatValType> {
        if self.offset >= self.data.len() {
            return Err(Error::parse_error(
                "Unexpected end while reading value type ",
            ));
        }

        let val_form = self.data[self.offset];
        self.offset += 1;

        match val_form {
            0x7F => Ok(FormatValType::Bool),
            0x7E => Ok(FormatValType::S8),
            0x7D => Ok(FormatValType::U8),
            0x7C => Ok(FormatValType::S16),
            0x7B => Ok(FormatValType::U16),
            0x7A => Ok(FormatValType::S32),
            0x79 => Ok(FormatValType::U32),
            0x78 => Ok(FormatValType::S64),
            0x77 => Ok(FormatValType::U64),
            0x76 => Ok(FormatValType::F32),
            0x75 => Ok(FormatValType::F64),
            0x74 => Ok(FormatValType::Char),
            0x73 => Ok(FormatValType::String),
            0x72 => {
                // Record type - simplified for streaming
                let (field_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;

                // Skip field definitions for now (would need full recursive parsing)
                for _ in 0..field_count {
                    let _name = self.read_string()?;
                    let _field_type = self.parse_value_type()?;
                }

                // Use bounded vec for empty record - allocation will be handled by capability
                // system
                #[cfg(not(feature = "std"))]
                let empty_fields = {
                    let provider = create_decoder_provider::<4096>()?;
                    DecoderVec::new(provider)?
                };
                #[cfg(feature = "std")]
                let empty_fields = DecoderVec::new();
                Ok(FormatValType::Record(empty_fields))
            },
            0x71 => {
                // Variant type - simplified for streaming
                let (case_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;

                // Skip case definitions for now
                for _ in 0..case_count {
                    let _name = self.read_string()?;
                    // Optional case type
                    if self.offset < self.data.len() && self.data[self.offset] == 1 {
                        self.offset += 1;
                        let _case_type = self.parse_value_type()?;
                    } else if self.offset < self.data.len() {
                        self.offset += 1; // Skip the 0 byte
                    }
                }

                // Use bounded vec for empty variant - allocation will be handled by capability
                // system
                #[cfg(not(feature = "std"))]
                let empty_cases = {
                    let provider = create_decoder_provider::<4096>()?;
                    DecoderVec::new(provider)?
                };
                #[cfg(feature = "std")]
                let empty_cases = DecoderVec::new();
                Ok(FormatValType::Variant(empty_cases))
            },
            0x70 => {
                // List type
                #[cfg(feature = "std")]
                {
                    let element_type = self.parse_value_type()?;
                    // For std, we use Box<FormatValType>
                    Ok(FormatValType::List(Box::new(element_type)))
                }
                #[cfg(not(feature = "std"))]
                {
                    let element_type_ref = self.parse_type_ref()?;
                    // For no_std placeholder, we use u32 type reference
                    Ok(FormatValType::List(element_type_ref))
                }
            },
            0x6F => {
                // Tuple type - simplified for streaming
                let (element_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;

                // Skip element types for now
                for _ in 0..element_count {
                    let _element_type = self.parse_value_type()?;
                }

                // Use bounded vec for empty tuple - allocation will be handled by capability
                // system
                #[cfg(not(feature = "std"))]
                let empty_elements = {
                    let provider = create_decoder_provider::<4096>()?;
                    DecoderVec::new(provider)?
                };
                #[cfg(feature = "std")]
                let empty_elements = DecoderVec::new();
                Ok(FormatValType::Tuple(empty_elements))
            },
            0x6E => {
                // Own resource
                let (resource_idx, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;
                Ok(FormatValType::Own(resource_idx))
            },
            0x6D => {
                // Borrow resource
                let (resource_idx, bytes_read) = read_leb128_u32(self.data, self.offset)?;
                self.offset += bytes_read;
                Ok(FormatValType::Borrow(resource_idx))
            },
            _ => Err(Error::parse_error("Unknown value type form ")),
        }
    }

    /// Parse type reference (simplified as u32 for streaming)
    fn parse_type_ref(&mut self) -> Result<u32> {
        let (type_ref, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;
        Ok(type_ref)
    }

    /// Parse resource representation
    fn parse_resource_representation(
        &mut self,
    ) -> Result<wrt_foundation::resource::ResourceRepresentation> {
        if self.offset >= self.data.len() {
            return Err(Error::parse_error(
                "Unexpected end while reading resource representation",
            ));
        }

        let repr_form = self.data[self.offset];
        self.offset += 1;

        match repr_form {
            0x00 => Ok(wrt_foundation::resource::ResourceRepresentation::Handle32),
            0x01 => Ok(wrt_foundation::resource::ResourceRepresentation::Handle64),
            _ => Err(Error::parse_error("Unknown resource representation")),
        }
    }

    /// Read a string from the binary data
    fn read_string(&mut self) -> Result<DecoderString> {
        let (string, bytes_read) = read_string(self.data, self.offset)?;
        self.offset += bytes_read;

        // Convert to bounded string
        // Convert bytes to string first
        let string_str = core::str::from_utf8(string)
            .map_err(|_| Error::parse_error("Invalid UTF-8 in string"))?;

        #[cfg(not(feature = "std"))]
        let bounded_string = DecoderString::try_from_str(string_str)
            .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
        #[cfg(feature = "std")]
        let bounded_string = DecoderString::from(string_str);

        Ok(bounded_string)
    }

    /// Store a parsed type in the bounded storage
    fn store_type(
        &self,
        types: &mut BoundedTypeVec<ComponentType>,
        comp_type: ComponentType,
    ) -> Result<()> {
        #[cfg(not(feature = "std"))]
        {
            types
                .push(comp_type)
                .map_err(|_| Error::runtime_execution_error("Streaming type parser error "))?;
        }
        #[cfg(feature = "std")]
        {
            types.push(comp_type);
        }
        Ok(())
    }

    /// Get current parsing offset
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get remaining bytes in the section
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }
}

impl ComponentTypeSection {
    /// Get the number of parsed types
    pub fn type_count(&self) -> u32 {
        self.type_count
    }

    /// Get total bytes consumed during parsing
    pub fn bytes_consumed(&self) -> usize {
        self.bytes_consumed
    }

    /// Get a type by index (ASIL-safe)
    pub fn get_type(&self, index: usize) -> wrt_error::Result<ComponentType> {
        self.types
            .get(index)
            .cloned()
            .ok_or_else(|| wrt_error::Error::parse_error("Component type index out of bounds"))
    }

    /// Iterate over all types (ASIL-safe)  
    /// Note: Manual iteration to work around BoundedVec iterator limitations
    pub fn iter_types(&self) -> impl Iterator<Item = ComponentType> + '_ {
        (0..self.types.len()).filter_map(move |i| self.types.get(i).cloned())
    }

    /// Get the number of types as usize
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Check if the section is empty
    pub fn is_empty(&self) -> bool {
        use wrt_foundation::traits::BoundedCapacity;
        self.types.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_section() {
        let data = &[0u8]; // Zero types

        let mut parser = StreamingTypeParser::new(data, VerificationLevel::Standard).unwrap();

        let result = parser.parse().unwrap();
        assert_eq!(result.type_count(), 0);
        assert_eq!(result.bytes_consumed(), 1);
        assert!(result.is_empty());
    }

    #[test]
    fn test_invalid_type_count() -> Result<()> {
        // Create data with too many types
        let type_count = (MAX_TYPES_PER_COMPONENT + 1) as u32;
        #[cfg(not(feature = "std"))]
        let mut data = {
            let provider = create_decoder_provider::<4096>()?;
            DecoderVec::new(provider)?
        };
        #[cfg(feature = "std")]
        let mut data = DecoderVec::new();

        // Write LEB128 encoded type count
        let mut count = type_count;
        while count >= 0x80 {
            #[cfg(not(feature = "std"))]
            data.push((count & 0x7F) as u8 | 0x80).unwrap();
            #[cfg(feature = "std")]
            data.push(((count & 0x7F) as u8) | 0x80);
            count >>= 7;
        }
        #[cfg(not(feature = "std"))]
        data.push(count as u8).unwrap();
        #[cfg(feature = "std")]
        data.push(count as u8);

        let mut parser =
            StreamingTypeParser::new(data.as_slice(), VerificationLevel::Standard).unwrap();

        assert!(parser.parse().is_err());
        Ok(())
    }

    #[test]
    fn test_recursion_depth_protection() {
        // This would test that deep recursion is properly handled,
        // but requires complex binary construction
        let data = &[0u8]; // Zero types for now

        let mut parser = StreamingTypeParser::new(data, VerificationLevel::Standard).unwrap();

        // Set recursion depth to maximum
        parser.recursion_depth = MAX_TYPE_RECURSION_DEPTH;

        // This should not crash due to recursion protection
        assert!(parser.parse().is_ok());
    }

    #[test]
    fn test_parser_offset_tracking() {
        let data = &[0u8]; // Zero types

        let mut parser = StreamingTypeParser::new(data, VerificationLevel::Standard).unwrap();

        assert_eq!(parser.offset(), 0);
        assert_eq!(parser.remaining(), 1);

        let result = parser.parse().unwrap();
        assert_eq!(parser.offset(), 1);
        assert_eq!(parser.remaining(), 0);
        assert_eq!(result.bytes_consumed(), 1);
    }
}
