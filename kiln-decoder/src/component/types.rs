// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Import core traits
use core::default::Default;

// Re-export the main component types from wrt-format for convenience
#[cfg(feature = "std")]
pub use wrt_format::component::{
    Component, ComponentType, CoreExternType, CoreInstance, CoreType, Export, ExternType, Import,
    Instance, Start, ValType,
};

// No_std bounded alternatives following functional safety guidelines
#[cfg(not(feature = "std"))]
mod no_std_types {
    use wrt_foundation::{
        BoundedMap, BoundedString, BoundedVec, NoStdProvider, budget_aware_provider::CrateId,
        safe_managed_alloc,
    };

    use super::*;

    /// Component string type for no_std environments
    pub type ComponentString = BoundedString<256>;

    /// Component vector type for no_std environments
    pub type ComponentVec<T> = BoundedVec<T, 128, NoStdProvider<4096>>;

    /// No_std Component with bounded allocation limits
    ///
    /// # Safety Requirements
    /// - All collections have compile-time bounds
    /// - No heap allocation or dynamic memory
    /// - Graceful degradation when limits exceeded
    #[derive(Debug, Clone)]
    pub struct Component {
        pub magic: [u8; 4],
        pub version: [u8; 4],
        pub exports: BoundedVec<Export, 128, NoStdProvider<4096>>,
        pub imports: BoundedVec<Import, 128, NoStdProvider<4096>>,
    }

    impl Component {
        pub fn new() -> wrt_error::Result<Self> {
            let exports_provider = safe_managed_alloc!(4096, CrateId::Decoder)?;
            let imports_provider = safe_managed_alloc!(4096, CrateId::Decoder)?;
            Ok(Self {
                magic: *b"\0asm",
                version: [0x0a, 0x00, 0x01, 0x00], // Component format
                exports: BoundedVec::new(exports_provider)?,
                imports: BoundedVec::new(imports_provider)?,
            })
        }
    }

    /// Simplified component type for no_std environments
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ComponentType {
        #[default]
        Module,
        Component,
        Instance,
        Function,
        Value,
        Type,
    }

    /// Core extern type enumeration
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CoreExternType {
        Function,
        Table,
        Memory,
        Global,
    }

    /// Core instance reference
    #[derive(Debug, Clone)]
    pub struct CoreInstance {
        pub id: u32,
        pub exports: ComponentVec<ComponentString>,
    }

    /// Core type definitions
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CoreType {
        Function,
        Module,
    }

    /// Export definition
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Export {
        pub name: ComponentString,
        pub kind: ComponentType,
        pub index: u32,
    }

    /// External type reference
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExternType {
        Function,
        Table,
        Memory,
        Global,
        Type,
    }

    /// Import definition
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Import {
        pub module: ComponentString,
        pub name: ComponentString,
        pub kind: ComponentType,
    }

    /// Instance reference
    #[derive(Debug, Clone)]
    pub struct Instance {
        pub id: u32,
        pub exports: ComponentVec<Export>,
    }

    /// Start function reference
    #[derive(Debug, Clone, Copy)]
    pub struct Start {
        pub function_index: u32,
    }

    /// Value type enumeration for no_std
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ValType {
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
    }

    // Implement required traits for Export
    impl wrt_foundation::traits::ToBytes for Export {
        fn serialized_size(&self) -> usize {
            // ComponentString size + enum + u32
            wrt_foundation::traits::ToBytes::serialized_size(&self.name) + 1 + 4
        }

        fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream,
            provider: &PStream,
        ) -> wrt_error::Result<()> {
            wrt_foundation::traits::ToBytes::to_bytes_with_provider(&self.name, writer, provider)?;
            writer.write_u8(self.kind as u8)?;
            writer.write_u32_le(self.index)?;
            Ok(())
        }
    }

    impl wrt_foundation::traits::FromBytes for Export {
        fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream,
            provider: &PStream,
        ) -> wrt_error::Result<Self> {
            let name = <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?;
            let kind_byte = reader.read_u8()?;
            let kind = match kind_byte {
                0 => ComponentType::Module,
                1 => ComponentType::Component,
                2 => ComponentType::Instance,
                3 => ComponentType::Function,
                4 => ComponentType::Value,
                5 => ComponentType::Type,
                _ => ComponentType::Module, // Default fallback
            };
            let index = reader.read_u32_le()?;
            Ok(Export { name, kind, index })
        }
    }

    impl wrt_foundation::traits::Checksummable for Export {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            wrt_foundation::traits::Checksummable::update_checksum(&self.name, checksum);
            checksum.update_slice(&[self.kind as u8]);
            checksum.update_slice(&self.index.to_le_bytes());
        }
    }

    // Implement required traits for Import
    impl wrt_foundation::traits::ToBytes for Import {
        fn serialized_size(&self) -> usize {
            // Two ComponentString sizes + enum
            wrt_foundation::traits::ToBytes::serialized_size(&self.module)
                + wrt_foundation::traits::ToBytes::serialized_size(&self.name)
                + 1
        }

        fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream,
            provider: &PStream,
        ) -> wrt_error::Result<()> {
            wrt_foundation::traits::ToBytes::to_bytes_with_provider(
                &self.module,
                writer,
                provider,
            )?;
            wrt_foundation::traits::ToBytes::to_bytes_with_provider(&self.name, writer, provider)?;
            writer.write_u8(self.kind as u8)?;
            Ok(())
        }
    }

    impl wrt_foundation::traits::FromBytes for Import {
        fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
            reader: &mut wrt_foundation::traits::ReadStream,
            provider: &PStream,
        ) -> wrt_error::Result<Self> {
            let module = <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?;
            let name = <crate::prelude::DecoderString as crate::prelude::DecoderStringExt>::from_bytes_with_provider(reader, provider)?;
            let kind_byte = reader.read_u8()?;
            let kind = match kind_byte {
                0 => ComponentType::Module,
                1 => ComponentType::Component,
                2 => ComponentType::Instance,
                3 => ComponentType::Function,
                4 => ComponentType::Value,
                5 => ComponentType::Type,
                _ => ComponentType::Module, // Default fallback
            };
            Ok(Import { module, name, kind })
        }
    }

    impl wrt_foundation::traits::Checksummable for Import {
        fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
            use wrt_foundation::traits::Checksummable;
            wrt_foundation::traits::Checksummable::update_checksum(&self.module, checksum);
            wrt_foundation::traits::Checksummable::update_checksum(&self.name, checksum);
            checksum.update_slice(&[self.kind as u8]);
        }
    }
}

#[cfg(not(feature = "std"))]
pub use no_std_types::*;

use crate::prelude::*;

/// Trait for component analysis capabilities
pub trait ComponentAnalyzer {
    /// Create a summary of a component's structure
    fn analyze(&self) -> wrt_error::Result<crate::component::analysis::ComponentSummary>;

    /// Get embedded modules from a component
    #[cfg(feature = "std")]
    fn get_embedded_modules(&self) -> Vec<Vec<u8>>;

    /// Get embedded modules from a component (no_std bounded version)
    #[cfg(not(feature = "std"))]
    fn get_embedded_modules(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<
            BoundedVec<u8, 1024, wrt_foundation::safe_memory::NoStdProvider<65536>>,
            16,
            wrt_foundation::safe_memory::NoStdProvider<8192>,
        >,
    >;

    /// Check if a component has a specific export
    fn has_export(&self, name: &str) -> bool;

    /// Get information about exports
    #[cfg(feature = "std")]
    fn get_export_info(&self) -> Vec<ExportInfo>;

    /// Get information about exports (no_std bounded version)
    #[cfg(not(feature = "std"))]
    fn get_export_info(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<ExportInfo, 64, wrt_foundation::safe_memory::NoStdProvider<8192>>,
    >;

    /// Get information about imports
    #[cfg(feature = "std")]
    fn get_import_info(&self) -> Vec<ImportInfo>;

    /// Get information about imports (no_std bounded version)
    #[cfg(not(feature = "std"))]
    fn get_import_info(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<ImportInfo, 64, wrt_foundation::safe_memory::NoStdProvider<8192>>,
    >;
}

/// Export information
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExportInfo {
    /// Export name
    #[cfg(feature = "std")]
    pub name: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub name: crate::prelude::DecoderString,

    /// Type of export (function, memory, etc.)
    #[cfg(feature = "std")]
    pub kind: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub kind: crate::prelude::DecoderString,

    /// Type information (as string)
    #[cfg(feature = "std")]
    pub type_info: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub type_info: crate::prelude::DecoderString,
}

impl ExportInfo {
    /// Create a new ExportInfo with managed memory allocation
    pub fn new() -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        return Ok(Self {
            name: alloc::string::String::new(),
            kind: alloc::string::String::new(),
            type_info: alloc::string::String::new(),
        });

        #[cfg(not(feature = "std"))]
        {
            Ok(Self {
                name: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
                kind: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
                type_info: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
            })
        }
    }
}

/// Import information
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportInfo {
    /// Import module
    #[cfg(feature = "std")]
    pub module: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub module: crate::prelude::DecoderString,

    /// Import name
    #[cfg(feature = "std")]
    pub name: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub name: crate::prelude::DecoderString,

    /// Type of import (function, memory, etc.)
    #[cfg(feature = "std")]
    pub kind: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub kind: crate::prelude::DecoderString,

    /// Type information (as string)
    #[cfg(feature = "std")]
    pub type_info: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub type_info: crate::prelude::DecoderString,
}

impl ImportInfo {
    /// Create a new ImportInfo with managed memory allocation
    pub fn new() -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        return Ok(Self {
            module: alloc::string::String::new(),
            name: alloc::string::String::new(),
            kind: alloc::string::String::new(),
            type_info: alloc::string::String::new(),
        });

        #[cfg(not(feature = "std"))]
        {
            Ok(Self {
                module: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
                name: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
                kind: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
                type_info: crate::prelude::DecoderString::try_from_str("")
                    .map_err(|_| wrt_error::Error::parse_error("Failed to create string"))?,
            })
        }
    }
}

/// Component binary metadata
#[derive(Debug, Clone)]
pub struct ComponentMetadata {
    /// Component name or identifier
    #[cfg(feature = "std")]
    pub name: alloc::string::String,
    #[cfg(not(feature = "std"))]
    pub name: crate::prelude::DecoderString,

    /// Component version (if available)
    #[cfg(feature = "std")]
    pub version: Option<alloc::string::String>,
    #[cfg(not(feature = "std"))]
    pub version: Option<crate::prelude::DecoderString>,

    /// Custom sections contained in the component
    #[cfg(feature = "std")]
    pub custom_sections: alloc::vec::Vec<alloc::string::String>,
    #[cfg(not(feature = "std"))]
    pub custom_sections: crate::prelude::DecoderVec<crate::prelude::DecoderString>,
}

/// Module information within a component
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Module index
    pub idx: u32,
    /// Module size in bytes
    pub size: usize,
    /// Module function count
    pub function_count: usize,
    /// Module memory count
    pub memory_count: usize,
    /// Module table count
    pub table_count: usize,
    /// Module global count
    pub global_count: usize,
}

/// Implementation of ComponentAnalyzer for Component
#[cfg(feature = "std")]
impl ComponentAnalyzer for Component {
    fn analyze(&self) -> wrt_error::Result<crate::component::analysis::ComponentSummary> {
        // Create a basic summary directly from the component
        Ok(crate::component::analysis::ComponentSummary {
            name: String::new(), /* Keep as std String for now as this is used
                                  * in analysis */
            core_modules_count: self.modules.len() as u32,
            core_instances_count: self.core_instances.len() as u32,
            imports_count: self.imports.len() as u32,
            exports_count: self.exports.len() as u32,
            aliases_count: self.aliases.len() as u32,
            module_info: Vec::new(), // Keep as std Vec for analysis compatibility
            export_info: Vec::new(),
            import_info: Vec::new(),
        })
    }

    #[cfg(feature = "std")]
    fn get_embedded_modules(&self) -> Vec<Vec<u8>> {
        // This will be implemented in the analysis module
        Vec::new()
    }

    fn has_export(&self, name: &str) -> bool {
        self.exports.iter().any(|export| export.name.name == name)
    }

    #[cfg(feature = "std")]
    fn get_export_info(&self) -> Vec<ExportInfo> {
        // This will be implemented in the analysis module
        Vec::new()
    }

    #[cfg(feature = "std")]
    fn get_import_info(&self) -> Vec<ImportInfo> {
        // This will be implemented in the analysis module
        Vec::new()
    }
}

#[cfg(not(feature = "std"))]
impl ComponentAnalyzer for Component {
    fn analyze(&self) -> wrt_error::Result<crate::component::analysis::ComponentSummary> {
        // Create a basic summary directly from the component (simplified for no_std)
        Ok(crate::component::analysis::ComponentSummary {
            name: "",
            core_modules_count: 0,   // No modules field in no_std Component
            core_instances_count: 0, // No core_instances field in no_std Component
            imports_count: wrt_foundation::traits::BoundedCapacity::len(&self.imports) as u32,
            exports_count: wrt_foundation::traits::BoundedCapacity::len(&self.exports) as u32,
            aliases_count: 0, // No aliases field in no_std Component
            module_info: wrt_foundation::BoundedVec::new(wrt_foundation::safe_managed_alloc!(
                4096,
                wrt_foundation::budget_aware_provider::CrateId::Decoder
            )?)?,
            export_info: (),
            import_info: (),
        })
    }

    #[cfg(not(feature = "std"))]
    fn get_embedded_modules(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<
            BoundedVec<u8, 1024, wrt_foundation::safe_memory::NoStdProvider<65536>>,
            16,
            wrt_foundation::safe_memory::NoStdProvider<8192>,
        >,
    > {
        // This will be implemented in the analysis module
        use wrt_foundation::safe_memory::NoStdProvider;
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        BoundedVec::new(provider).map_err(|_| {
            wrt_error::helpers::memory_allocation_failed_error(
                "Failed to create embedded modules vector",
            )
        })
    }

    fn has_export(&self, _name: &str) -> bool {
        // Simplified for no_std - export checking not supported
        false
    }

    #[cfg(not(feature = "std"))]
    fn get_export_info(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<ExportInfo, 64, wrt_foundation::safe_memory::NoStdProvider<8192>>,
    > {
        // This will be implemented in the analysis module
        use wrt_foundation::safe_memory::NoStdProvider;
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        BoundedVec::new(provider).map_err(|_| {
            wrt_error::helpers::memory_allocation_failed_error(
                "Failed to create export info vector",
            )
        })
    }

    #[cfg(not(feature = "std"))]
    fn get_import_info(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<ImportInfo, 64, wrt_foundation::safe_memory::NoStdProvider<8192>>,
    > {
        // This will be implemented in the analysis module
        use wrt_foundation::safe_memory::NoStdProvider;
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        BoundedVec::new(provider).map_err(|_| {
            wrt_error::helpers::memory_allocation_failed_error(
                "Failed to create import info vector",
            )
        })
    }
}

// Implement required traits for ExportInfo
impl wrt_foundation::traits::ToBytes for ExportInfo {
    fn serialized_size(&self) -> usize {
        self.name.len() + self.kind.len() + self.type_info.len() + 3 // 3 separators
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            writer.write_all(self.name.as_bytes())?;
            writer.write_u8(0)?; // separator
            writer.write_all(self.kind.as_bytes())?;
            writer.write_u8(0)?; // separator
            writer.write_all(self.type_info.as_bytes())?;
        }
        #[cfg(not(feature = "std"))]
        {
            if let Ok(s) = self.name.as_str() {
                writer.write_all(s.as_bytes())?;
            }
            writer.write_u8(0)?; // separator
            if let Ok(s) = self.kind.as_str() {
                writer.write_all(s.as_bytes())?;
            }
            writer.write_u8(0)?; // separator
            if let Ok(s) = self.type_info.as_str() {
                writer.write_all(s.as_bytes())?;
            }
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ExportInfo {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        {
            let mut bytes = Vec::new();
            while let Ok(byte) = reader.read_u8() {
                bytes.push(byte);
            }

            let parts: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
            if parts.len() >= 3 {
                Ok(ExportInfo {
                    name: String::from_utf8_lossy(parts[0]).to_string(),
                    kind: String::from_utf8_lossy(parts[1]).to_string(),
                    type_info: String::from_utf8_lossy(parts[2]).to_string(),
                })
            } else {
                Ok(ExportInfo::default())
            }
        }

        #[cfg(not(feature = "std"))]
        {
            // Simplified for no_std - return default
            Ok(ExportInfo::default())
        }
    }
}

impl wrt_foundation::traits::Checksummable for ExportInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        #[cfg(feature = "std")]
        {
            checksum.update_slice(self.name.as_bytes());
            checksum.update_slice(self.kind.as_bytes());
            checksum.update_slice(self.type_info.as_bytes());
        }
        #[cfg(not(feature = "std"))]
        {
            if let Ok(s) = self.name.as_str() {
                checksum.update_slice(s.as_bytes());
            }
            if let Ok(s) = self.kind.as_str() {
                checksum.update_slice(s.as_bytes());
            }
            if let Ok(s) = self.type_info.as_str() {
                checksum.update_slice(s.as_bytes());
            }
        }
    }
}

// Implement required traits for ImportInfo
impl wrt_foundation::traits::ToBytes for ImportInfo {
    fn serialized_size(&self) -> usize {
        self.module.len() + self.name.len() + self.kind.len() + self.type_info.len() + 4
        // 4 separators
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        writer.write_all(self.module.as_bytes())?;
        #[cfg(not(feature = "std"))]
        {
            if let Ok(s) = self.module.as_str() {
                writer.write_all(s.as_bytes())?;
            }
        }
        writer.write_u8(0)?; // separator
        #[cfg(feature = "std")]
        {
            writer.write_all(self.name.as_bytes())?;
            writer.write_u8(0)?; // separator
            writer.write_all(self.kind.as_bytes())?;
            writer.write_u8(0)?; // separator
            writer.write_all(self.type_info.as_bytes())?;
        }
        #[cfg(not(feature = "std"))]
        {
            if let Ok(s) = self.name.as_str() {
                writer.write_all(s.as_bytes())?;
            }
            writer.write_u8(0)?; // separator
            if let Ok(s) = self.kind.as_str() {
                writer.write_all(s.as_bytes())?;
            }
            writer.write_u8(0)?; // separator
            if let Ok(s) = self.type_info.as_str() {
                writer.write_all(s.as_bytes())?;
            }
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ImportInfo {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        {
            let mut bytes = Vec::new();
            while let Ok(byte) = reader.read_u8() {
                bytes.push(byte);
            }

            let parts: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
            if parts.len() >= 4 {
                Ok(ImportInfo {
                    module: String::from_utf8_lossy(parts[0]).to_string(),
                    name: String::from_utf8_lossy(parts[1]).to_string(),
                    kind: String::from_utf8_lossy(parts[2]).to_string(),
                    type_info: String::from_utf8_lossy(parts[3]).to_string(),
                })
            } else {
                Ok(ImportInfo::default())
            }
        }

        #[cfg(not(feature = "std"))]
        {
            // Simplified for no_std - return default
            Ok(ImportInfo::default())
        }
    }
}

impl wrt_foundation::traits::Checksummable for ImportInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        #[cfg(feature = "std")]
        {
            checksum.update_slice(self.module.as_bytes());
            checksum.update_slice(self.name.as_bytes());
            checksum.update_slice(self.kind.as_bytes());
            checksum.update_slice(self.type_info.as_bytes());
        }
        #[cfg(not(feature = "std"))]
        {
            if let Ok(s) = self.module.as_str() {
                checksum.update_slice(s.as_bytes());
            }
            if let Ok(s) = self.name.as_str() {
                checksum.update_slice(s.as_bytes());
            }
            if let Ok(s) = self.kind.as_str() {
                checksum.update_slice(s.as_bytes());
            }
            if let Ok(s) = self.type_info.as_str() {
                checksum.update_slice(s.as_bytes());
            }
        }
    }
}
