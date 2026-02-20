//! Instance implementation for the WebAssembly Component Model.
//!
//! This module provides the instance types for component instances.
//! It supports both std and no_std environments.

// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use wrt_format::component::ComponentTypeDefinition;

use crate::export::Export;
use crate::prelude::*;

// ============================================================================
// std implementation - uses String and Vec
// ============================================================================

#[cfg(feature = "std")]
mod std_impl {
    use std::{string::String, vec::Vec};

    use wrt_format::component::ComponentTypeDefinition;

    use crate::export::Export;

    // Type alias for compatibility
    pub type Instance = InstanceValue;

    /// Represents an instance value in a component
    #[derive(Debug, Clone)]
    pub struct InstanceValue {
        /// The name of the instance
        pub name: String,
        /// Instance type
        pub ty: ComponentTypeDefinition,
        /// Instance exports
        pub exports: Vec<Export>,
    }

    impl InstanceValue {
        /// Creates a new instance value
        pub fn new(name: String, ty: ComponentTypeDefinition, exports: Vec<Export>) -> Self {
            Self { name, ty, exports }
        }

        /// Gets an export by name
        pub fn get_export(&self, name: &str) -> Option<&Export> {
            self.exports.iter().find(|export| export.name == name)
        }

        /// Gets a mutable export by name
        pub fn get_export_mut(&mut self, name: &str) -> Option<&mut Export> {
            self.exports.iter_mut().find(|export| export.name == name)
        }
    }
}

// ============================================================================
// no_std implementation - uses BoundedString and BoundedVec
// ============================================================================

#[cfg(not(feature = "std"))]
mod no_std_impl {
    extern crate alloc;
    use alloc::vec;
    use wrt_error::Error;
    use wrt_format::component::ComponentTypeDefinition;
    use wrt_foundation::{
        MemoryProvider,
        bounded::{BoundedString, BoundedVec, MAX_WASM_NAME_LENGTH},
        budget_aware_provider::CrateId,
        safe_managed_alloc,
        safe_memory::NoStdProvider,
        traits::{Checksummable, FromBytes, ToBytes},
        verification::Checksum,
    };

    use crate::export::Export;

    /// Maximum number of exports per instance
    pub const MAX_INSTANCE_EXPORTS: usize = 64;

    // Type alias for compatibility
    pub type Instance = InstanceValue;

    /// No-std compatible representation of an instance value in a component
    #[derive(Debug, Clone)]
    pub struct InstanceValue {
        /// The name of the instance
        pub name: BoundedString<MAX_WASM_NAME_LENGTH>,
        /// Instance type
        pub ty: ComponentTypeDefinition,
        /// Instance exports
        pub exports: BoundedVec<Export, MAX_INSTANCE_EXPORTS, NoStdProvider<16384>>,
    }

    impl PartialEq for InstanceValue {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
        }
    }

    impl Eq for InstanceValue {}

    impl Default for InstanceValue {
        fn default() -> Self {
            Self {
                name: BoundedString::from_str_truncate("")
                    .unwrap_or_else(|_| panic!("Failed to create default InstanceValue name")),
                ty: ComponentTypeDefinition::Instance { exports: vec![] },
                exports: BoundedVec::new(NoStdProvider::default())
                    .expect("Failed to create default exports"),
            }
        }
    }

    impl Checksummable for InstanceValue {
        fn update_checksum(&self, checksum: &mut Checksum) {
            if let Ok(bytes) = self.name.as_bytes() {
                checksum.update_slice(bytes.as_ref());
            }
        }
    }

    impl ToBytes for InstanceValue {
        fn to_bytes_with_provider<'a, P: MemoryProvider>(
            &self,
            writer: &mut wrt_foundation::traits::WriteStream<'a>,
            _provider: &P,
        ) -> wrt_error::Result<()> {
            // Simple stub implementation - just write the name length
            writer.write_u32_le(self.name.len() as u32)?;
            Ok(())
        }
    }

    impl FromBytes for InstanceValue {
        fn from_bytes_with_provider<'a, P: MemoryProvider>(
            _reader: &mut wrt_foundation::traits::ReadStream<'a>,
            _provider: &P,
        ) -> wrt_error::Result<Self> {
            Ok(Self::default())
        }
    }

    impl InstanceValue {
        /// Creates a new instance value
        pub fn new(
            name: &str,
            ty: ComponentTypeDefinition,
            exports: &[Export],
        ) -> wrt_error::Result<Self> {
            let _name_provider = safe_managed_alloc!(512, CrateId::Component)?;
            let bounded_name = BoundedString::try_from_str(name)
                .map_err(|_| Error::parameter_validation_error("Instance name too long"))?;

            let provider = safe_managed_alloc!(16384, CrateId::Component)?;
            let mut bounded_exports = BoundedVec::new(provider)?;
            for export in exports {
                bounded_exports
                    .push(export.clone())
                    .map_err(|_| Error::capacity_exceeded("Maximum number of exports exceeded"))?;
            }

            Ok(Self {
                name: bounded_name,
                ty,
                exports: bounded_exports,
            })
        }

        /// Gets an export by name
        pub fn get_export(&self, name: &str) -> Option<Export> {
            self.exports.iter().find(|export| export.name == name)
        }

        /// Gets a mutable export by name
        /// Note: Due to BoundedVec's serialization architecture, this returns an owned value
        pub fn get_export_mut(&mut self, name: &str) -> Option<Export> {
            for i in 0..self.exports.len() {
                if let Ok(export) = self.exports.get(i) {
                    if export.name == name {
                        return Some(export);
                    }
                }
            }
            None
        }

        /// Creates a builder for this instance value
        pub fn builder() -> InstanceValueBuilder {
            InstanceValueBuilder::new()
        }
    }

    /// Builder for InstanceValue
    pub struct InstanceValueBuilder {
        /// The name of the instance
        name: Option<alloc::string::String>,
        /// Instance type
        ty: Option<ComponentTypeDefinition>,
        /// Instance exports
        exports: alloc::vec::Vec<Export>,
    }

    impl InstanceValueBuilder {
        /// Creates a new instance value builder
        pub fn new() -> Self {
            Self {
                name: None,
                ty: None,
                exports: alloc::vec::Vec::new(),
            }
        }

        /// Sets the instance name
        pub fn with_name(mut self, name: &str) -> Self {
            self.name = Some(name.into());
            self
        }

        /// Sets the instance type
        pub fn with_type(mut self, ty: ComponentTypeDefinition) -> Self {
            self.ty = Some(ty);
            self
        }

        /// Adds an export to the instance
        pub fn with_export(mut self, export: Export) -> Self {
            self.exports.push(export);
            self
        }

        /// Adds multiple exports to the instance
        pub fn with_exports(mut self, exports: alloc::vec::Vec<Export>) -> Self {
            self.exports.extend(exports);
            self
        }

        /// Builds the instance value
        pub fn build(self) -> wrt_error::Result<InstanceValue> {
            let name = self
                .name
                .ok_or_else(|| Error::parameter_validation_error("Instance name is required"))?;

            let ty = self
                .ty
                .ok_or_else(|| Error::parameter_validation_error("Instance type is required"))?;

            InstanceValue::new(&name, ty, &self.exports)
        }
    }

    impl Default for InstanceValueBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    /// A collection of instances with bounded capacity
    pub struct InstanceCollection {
        instances: BoundedVec<InstanceValue, MAX_INSTANCE_EXPORTS, NoStdProvider<65536>>,
    }

    impl InstanceCollection {
        /// Creates a new empty instance collection
        pub fn new() -> wrt_error::Result<Self> {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            Ok(Self {
                instances: BoundedVec::new(provider)?,
            })
        }

        /// Adds an instance to the collection
        pub fn add_instance(&mut self, instance: InstanceValue) -> wrt_error::Result<()> {
            self.instances
                .push(instance)
                .map_err(|_| Error::capacity_exceeded("Maximum number of instances exceeded"))
        }

        /// Gets an instance by name
        pub fn get_instance(&self, name: &str) -> Option<InstanceValue> {
            self.instances.iter().find(|instance| instance.name.as_str().ok() == Some(name))
        }

        /// Gets a mutable instance by name
        pub fn get_instance_mut(&mut self, name: &str) -> Option<InstanceValue> {
            for i in 0..self.instances.len() {
                if let Ok(instance) = self.instances.get(i) {
                    if instance.name.as_str().ok() == Some(name) {
                        return Some(instance);
                    }
                }
            }
            None
        }

        /// Returns the number of instances in the collection
        pub fn len(&self) -> usize {
            self.instances.len()
        }

        /// Checks if the collection is empty
        pub fn is_empty(&self) -> bool {
            self.instances.is_empty()
        }

        /// Returns an iterator over the instances
        pub fn iter(&self) -> impl Iterator<Item = InstanceValue> + '_ {
            self.instances.iter()
        }
    }

    impl Default for InstanceCollection {
        fn default() -> Self {
            Self::new().expect("Failed to create default InstanceCollection")
        }
    }
}

// ============================================================================
// Re-export the appropriate implementation
// ============================================================================

#[cfg(feature = "std")]
pub use std_impl::*;

#[cfg(not(feature = "std"))]
pub use no_std_impl::*;
