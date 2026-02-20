// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component model validation
//!
//! This module provides validation for WebAssembly Component Model components.
//! It verifies that components conform to the Component Model specification.

#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(feature = "std")]
use wrt_format::component::{
    Alias, AliasTarget, Canon, CanonOperation, Component, ComponentType, ComponentTypeDefinition,
    Export, ExternType, Import, Instance, Sort,
};

// Import component model types from crate
// Import prelude for String and other types

// Component validation is only available with std feature due to complex
// recursive types
#[cfg(feature = "std")]
mod component_validation {
    use wrt_error::{Error, ErrorCategory, codes};

    use super::*;

    /// Maximum reasonable number of types in a component for validation
    const MAX_TYPES: u32 = 100_000;

    /// Maximum reasonable number of imports/exports
    const MAX_IMPORTS_EXPORTS: u32 = 10_000;

    /// Validation configuration for component model validation
    ///
    /// This allows controlling which features of the Component Model are
    /// validated, in case some implementations don't support the full
    /// model.
    #[derive(Debug, Clone)]
    pub struct ValidationConfig {
        /// Enable value section validation (🪙)
        pub enable_value_section: bool,
        /// Enable resource types validation (🔧)
        pub enable_resource_types: bool,
    }

    impl Default for ValidationConfig {
        fn default() -> Self {
            Self {
                enable_value_section: true,
                enable_resource_types: true,
            }
        }
    }

    impl ValidationConfig {
        /// Create a new validation config with default settings
        pub fn new() -> Self {
            Self::default()
        }

        /// Create a validation config with all features enabled
        pub fn all_enabled() -> Self {
            Self {
                enable_value_section: true,
                enable_resource_types: true,
            }
        }

        /// Create a validation config with only MVP features enabled
        pub fn mvp_only() -> Self {
            Self {
                enable_value_section: false,
                enable_resource_types: false,
            }
        }
    }

    /// Validation context for tracking component structure during validation
    struct ValidationContext<'a> {
        /// Component being validated
        component: &'a Component,
        /// Configuration for validation
        config: &'a ValidationConfig,
        /// Track defined type indices
        #[cfg(not(any(feature = "std",)))]
        defined_types: BoundedVec<u32, 1000>,
        #[cfg(feature = "std")]
        defined_types: Vec<u32>,
        /// Track defined import names to detect duplicates
        #[cfg(not(any(feature = "std",)))]
        import_names: HashMap<WasmName, u32, 100>,
        #[cfg(feature = "std")]
        import_names: HashMap<String, u32>,
        /// Track defined export names to detect duplicates
        #[cfg(not(any(feature = "std",)))]
        export_names: HashMap<WasmName, u32, 100>,
        #[cfg(feature = "std")]
        export_names: HashMap<String, u32>,
        /// Track defined instance indices
        #[cfg(not(any(feature = "std",)))]
        defined_instances: BoundedVec<u32, 1000>,
        #[cfg(feature = "std")]
        defined_instances: Vec<u32>,
    }

    impl<'a> ValidationContext<'a> {
        /// Create a new validation context
        fn new(component: &'a Component, config: &'a ValidationConfig) -> Self {
            Self {
                component,
                config,
                #[cfg(not(any(feature = "std",)))]
                defined_types: BoundedVec::new(),
                #[cfg(feature = "std")]
                defined_types: Vec::new(),
                #[cfg(not(any(feature = "std",)))]
                import_names: HashMap::new(),
                #[cfg(feature = "std")]
                import_names: HashMap::new(),
                #[cfg(not(any(feature = "std",)))]
                export_names: HashMap::new(),
                #[cfg(feature = "std")]
                export_names: HashMap::new(),
                #[cfg(not(any(feature = "std",)))]
                defined_instances: BoundedVec::new(),
                #[cfg(feature = "std")]
                defined_instances: Vec::new(),
            }
        }

        /// Add a defined type index
        fn add_type(&mut self, idx: u32) -> Result<(), Error> {
            #[cfg(not(any(feature = "std",)))]
            {
                self.defined_types
                    .push(idx)
                    .map_err(|_| Error::validation_error("too many types in component"))?;
            }
            #[cfg(feature = "std")]
            {
                if self.defined_types.len() >= MAX_TYPES as usize {
                    return Err(Error::validation_error("too many types in component"));
                }
                self.defined_types.push(idx);
            }
            Ok(())
        }

        /// Check if a type index is valid
        fn is_type_valid(&self, idx: u32) -> bool {
            self.defined_types.contains(&idx)
        }

        /// Add an import name and check for duplicates
        fn add_import_name(&mut self, name: &str) -> Result<(), Error> {
            #[cfg(not(any(feature = "std",)))]
            {
                let wasm_name = WasmName::try_from(name)
                    .map_err(|_| Error::validation_error("import name too long"))?;
                if self.import_names.contains_key(&wasm_name) {
                    return Err(Error::validation_error("duplicate import name"));
                }
                self.import_names
                    .insert(wasm_name, self.import_names.len() as u32)
                    .map_err(|_| Error::validation_error("too many imports"))?;
            }
            #[cfg(feature = "std")]
            {
                if self.import_names.contains_key(name) {
                    return Err(Error::validation_error("duplicate import name"));
                }
                if self.import_names.len() >= MAX_IMPORTS_EXPORTS as usize {
                    return Err(Error::validation_error("too many imports"));
                }
                self.import_names.insert(name.to_string(), self.import_names.len() as u32);
            }
            Ok(())
        }

        /// Add an export name and check for duplicates
        fn add_export_name(&mut self, name: &str) -> Result<(), Error> {
            #[cfg(not(any(feature = "std",)))]
            {
                let wasm_name = WasmName::try_from(name)
                    .map_err(|_| Error::validation_error("export name too long"))?;
                if self.export_names.contains_key(&wasm_name) {
                    return Err(Error::validation_error("duplicate export name"));
                }
                self.export_names
                    .insert(wasm_name, self.export_names.len() as u32)
                    .map_err(|_| Error::validation_error("too many exports"))?;
            }
            #[cfg(feature = "std")]
            {
                if self.export_names.contains_key(name) {
                    return Err(Error::validation_error("duplicate export name"));
                }
                if self.export_names.len() >= MAX_IMPORTS_EXPORTS as usize {
                    return Err(Error::validation_error("too many exports"));
                }
                self.export_names.insert(name.to_string(), self.export_names.len() as u32);
            }
            Ok(())
        }

        /// Add a defined instance index
        fn add_instance(&mut self, idx: u32) -> Result<(), Error> {
            #[cfg(not(any(feature = "std",)))]
            {
                self.defined_instances
                    .push(idx)
                    .map_err(|_| Error::validation_error("too many instances in component"))?;
            }
            #[cfg(feature = "std")]
            {
                if self.defined_instances.len() >= MAX_TYPES as usize {
                    return Err(Error::validation_error("too many instances in component"));
                }
                self.defined_instances.push(idx);
            }
            Ok(())
        }

        /// Check if an instance index is valid
        fn is_instance_valid(&self, idx: u32) -> bool {
            self.defined_instances.contains(&idx)
        }
    }

    /// Validate all types in the component
    fn validate_types(ctx: &mut ValidationContext) -> Result<(), Error> {
        for (idx, component_type) in ctx.component.types.iter().enumerate() {
            validate_component_type(ctx, component_type)?;
            ctx.add_type(idx as u32)?;
        }
        Ok(())
    }

    /// Validate a single component type
    fn validate_component_type(
        _ctx: &ValidationContext,
        component_type: &ComponentType,
    ) -> Result<(), Error> {
        match &component_type.definition {
            // TODO: ComponentTypeDefinition no longer has Module variant
            // ComponentTypeDefinition::Module(_module_type) => {
            //     // Module types are validated during parsing
            //     Ok(())
            // }
            ComponentTypeDefinition::Component { imports, exports } => {
                // Nested component types are validated recursively
                _ = (imports, exports); // Suppress unused warnings
                Ok(())
            },
            ComponentTypeDefinition::Instance { exports } => {
                // Instance types are validated during parsing
                _ = exports; // Suppress unused warning
                Ok(())
            },
            ComponentTypeDefinition::Function { params, results } => {
                // Function types are validated during parsing
                _ = (params, results); // Suppress unused warnings
                Ok(())
            },
            ComponentTypeDefinition::Value(val_type) => {
                // Value types are validated during parsing
                _ = val_type; // Suppress unused warning
                Ok(())
            },
            ComponentTypeDefinition::Resource {
                representation,
                nullable,
            } => {
                // Resource types are validated during parsing
                _ = (representation, nullable); // Suppress unused warnings
                Ok(())
            },
        }
    }

    /// Validate an alias
    fn validate_alias(ctx: &ValidationContext, alias: &Alias) -> Result<(), Error> {
        match &alias.target {
            AliasTarget::CoreInstanceExport {
                instance_idx,
                name,
                kind,
            } => {
                if !ctx.is_instance_valid(*instance_idx) {
                    return Err(Error::validation_error(
                        "invalid instance index in core export alias",
                    ));
                }
                // Further validation would check if the export exists in the instance
                _ = (name, kind); // Suppress unused warnings
            },
            AliasTarget::InstanceExport {
                instance_idx,
                name,
                kind,
            } => {
                if !ctx.is_instance_valid(*instance_idx) {
                    return Err(Error::validation_error(
                        "invalid instance index in export alias",
                    ));
                }
                // Further validation would check if the export exists in the instance
                _ = (name, kind); // Suppress unused warnings
            },
            AliasTarget::Outer { count, kind, idx } => {
                // Outer aliases reference parent components
                // Validation would check if we're nested deep enough
                _ = (count, kind, idx); // Suppress unused warnings
            },
        }
        Ok(())
    }

    /// Validate all imports in the component
    fn validate_imports(ctx: &mut ValidationContext) -> Result<(), Error> {
        for import in &ctx.component.imports {
            validate_import(ctx, import)?;
        }
        Ok(())
    }

    /// Validate a single import
    fn validate_import(ctx: &mut ValidationContext, import: &Import) -> Result<(), Error> {
        // Check for duplicate import names
        ctx.add_import_name(&import.name.name)?;

        // Validate the import type reference
        match &import.ty {
            ExternType::Type(type_idx) => {
                if !ctx.is_type_valid(*type_idx) {
                    return Err(Error::validation_error("invalid type index in import"));
                }
            },
            _ => {
                // Other extern types are handled separately
            },
        }

        Ok(())
    }

    /// Validate all exports in the component
    fn validate_exports(ctx: &mut ValidationContext) -> Result<(), Error> {
        for export in &ctx.component.exports {
            validate_export(ctx, export)?;
        }
        Ok(())
    }

    /// Validate a single export
    fn validate_export(ctx: &mut ValidationContext, export: &Export) -> Result<(), Error> {
        // Check for duplicate export names
        ctx.add_export_name(&export.name.name)?;

        // Validate the export reference
        match &export.sort {
            Sort::Core(_) => {
                // Core module export
                if export.idx >= ctx.component.modules.len() as u32 {
                    return Err(Error::validation_error("invalid module index in export"));
                }
            },
            Sort::Function => {
                // Function export
                // Would need to track defined functions
            },
            Sort::Type => {
                // Type export
                if !ctx.is_type_valid(export.idx) {
                    return Err(Error::validation_error("invalid type index in export"));
                }
            },
            Sort::Instance => {
                // Instance export
                if !ctx.is_instance_valid(export.idx) {
                    return Err(Error::validation_error("invalid instance index in export"));
                }
            },
            Sort::Component => {
                // Component export
                if export.idx >= ctx.component.components.len() as u32 {
                    return Err(Error::validation_error("invalid component index in export"));
                }
            },
            Sort::Value => {
                // Value export - validate if needed
            },
        }

        Ok(())
    }

    /// Validate all instances in the component
    fn validate_instances(ctx: &mut ValidationContext) -> Result<(), Error> {
        for (idx, instance) in ctx.component.instances.iter().enumerate() {
            validate_instance(ctx, instance)?;
            ctx.add_instance(idx as u32)?;
        }
        Ok(())
    }

    /// Validate a single instance
    fn validate_instance(_ctx: &ValidationContext, instance: &Instance) -> Result<(), Error> {
        {
            // TODO: Implement proper instance validation once Instance enum structure is
            // clarified
            _ = instance; // Suppress unused warning
        }
        Ok(())
    }

    /// Validate all canonical functions in the component
    fn validate_canonicals(ctx: &ValidationContext) -> Result<(), Error> {
        for canon in &ctx.component.canonicals {
            validate_canonical(ctx, canon)?;
        }
        Ok(())
    }

    /// Validate a single canonical function
    fn validate_canonical(ctx: &ValidationContext, canon: &Canon) -> Result<(), Error> {
        match &canon.operation {
            CanonOperation::Lift {
                func_idx, type_idx, ..
            } => {
                // Validate function type index
                if !ctx.is_type_valid(*type_idx) {
                    return Err(Error::validation_error(
                        "invalid function type in canon lift",
                    ));
                }
                // Would validate func_idx if we had function tracking
                _ = func_idx; // Suppress unused warning
            },
            CanonOperation::Lower { func_idx, options } => {
                // Would need to track defined functions
                _ = (func_idx, options); // Suppress unused warnings
            },
            CanonOperation::Resource(resource_op) => {
                if !ctx.config.enable_resource_types {
                    return Err(Error::validation_error("resource types not enabled"));
                }
                // Validate resource operation if needed
                _ = resource_op; // Suppress unused warning for now
            },
            CanonOperation::Realloc {
                alloc_func_idx,
                memory_idx,
            } => {
                // Validate allocation function and memory indices
                _ = (alloc_func_idx, memory_idx); // Suppress unused warnings
                // for now
            },
            CanonOperation::PostReturn { func_idx } => {
                // Validate post-return function index
                _ = func_idx; // Suppress unused warning for now
            },
            CanonOperation::MemoryCopy {
                src_memory_idx,
                dst_memory_idx,
                func_idx,
            } => {
                // Validate memory copy operation
                _ = (src_memory_idx, dst_memory_idx, func_idx); // Suppress unused warnings for now
            },
            CanonOperation::Async {
                func_idx,
                type_idx,
                options,
            } => {
                // Validate async operation
                _ = (func_idx, type_idx, options); // Suppress unused warnings
                // for now
            },
        }
        Ok(())
    }

    /// Validate a component with specific configuration options
    pub fn validate_component_with_config(
        component: &Component,
        config: &ValidationConfig,
    ) -> Result<(), Error> {
        let mut ctx = ValidationContext::new(component, config);

        // Validate in dependency order
        validate_types(&mut ctx)?;
        validate_imports(&mut ctx)?;
        validate_instances(&mut ctx)?;
        validate_canonicals(&ctx)?;
        validate_exports(&mut ctx)?;

        // Additional validation for value sections if enabled
        if config.enable_value_section && !component.values.is_empty() {
            // Value section validation would go here
        }

        Ok(())
    }

    /// Validate a component with default configuration
    pub fn validate_component(component: &Component) -> Result<(), Error> {
        validate_component_with_config(component, &ValidationConfig::default())
    }
} // end of component_validation module

// Re-export public APIs when std feature is enabled
#[cfg(feature = "std")]
pub use component_validation::{
    ValidationConfig, validate_component, validate_component_with_config,
};

// No-std stub implementations
#[cfg(not(feature = "std"))]
pub mod no_std_stubs {
    use wrt_error::{Error, ErrorCategory, Result, codes};

    /// Validation configuration stub for no_std environments
    #[derive(Debug, Clone)]
    pub struct ValidationConfig;

    impl Default for ValidationConfig {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ValidationConfig {
        pub fn new() -> Self {
            Self
        }

        pub fn default() -> Self {
            Self
        }

        pub fn all_enabled() -> Self {
            Self
        }

        pub fn mvp_only() -> Self {
            Self
        }
    }

    /// Stub component type for no_std validation
    #[derive(Debug, Clone)]
    pub struct Component;

    /// Validate a component (no_std stub)
    pub fn validate_component(_component: &Component) -> Result<()> {
        Err(Error::runtime_execution_error(
            "No-std validation not supported",
        ))
    }

    /// Validate a component with config (no_std stub)
    pub fn validate_component_with_config(
        _component: &Component,
        _config: &ValidationConfig,
    ) -> Result<()> {
        Err(Error::new(
            ErrorCategory::Validation,
            codes::UNSUPPORTED_OPERATION,
            "No-std validation with config not supported",
        ))
    }
}

#[cfg(not(feature = "std"))]
pub use no_std_stubs::*;
