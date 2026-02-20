//! Component import/export resolution
//!
//! This module provides functionality for resolving imports and exports
//! during component instantiation and linking.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec::Vec};
#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};

use wrt_foundation::{
    bounded::{BoundedString, BoundedVec, MAX_GENERATIVE_TYPES},
    budget_aware_provider::CrateId,
    prelude::*,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

use crate::{
    bounded_component_infra::ComponentProvider,
    generative_types::GenerativeTypeRegistry,
    prelude::WrtComponentValue,
    type_bounds::{RelationKind, TypeBoundsChecker},
    types::{ComponentError, ComponentInstanceId, TypeId, ValType},
};

/// Import resolution result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedImport {
    /// Import name
    pub name: BoundedString<64>,
    /// Resolved value
    pub value: ImportValue,
    /// Type information
    pub val_type: Option<ValType>,
}

/// Export resolution result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedExport {
    /// Export name
    pub name: BoundedString<64>,
    /// Resolved value
    pub value: ExportValue,
    /// Type information
    pub val_type: Option<ValType>,
}

/// Import value types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImportValue {
    /// Function import
    Function { type_id: TypeId, func_ref: u32 },
    /// Global import
    Global { type_id: TypeId, global_ref: u32 },
    /// Memory import
    Memory {
        min_pages: u32,
        max_pages: Option<u32>,
    },
    /// Table import
    Table {
        min_size: u32,
        max_size: Option<u32>,
    },
    /// Instance import
    Instance { instance_id: ComponentInstanceId },
    /// Value import
    Value {
        val_type: ValType,
        value: WrtComponentValue<ComponentProvider>,
    },
}

impl Default for ImportValue {
    fn default() -> Self {
        ImportValue::Memory {
            min_pages: 0,
            max_pages: None,
        }
    }
}

/// Export value types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExportValue {
    /// Function export
    Function { type_id: TypeId, func_ref: u32 },
    /// Global export
    Global { type_id: TypeId, global_ref: u32 },
    /// Memory export
    Memory { memory_ref: u32 },
    /// Table export
    Table { table_ref: u32 },
    /// Instance export
    Instance { instance_id: ComponentInstanceId },
    /// Value export
    Value {
        val_type: ValType,
        value: WrtComponentValue<ComponentProvider>,
    },
}

impl Default for ExportValue {
    fn default() -> Self {
        ExportValue::Memory { memory_ref: 0 }
    }
}

// ComponentValue is imported from wrt_foundation via prelude
// Use WrtComponentValue<ComponentProvider> for all component values

/// Component resolver for import/export resolution
#[derive(Debug)]
pub struct ComponentResolver {
    /// Type registry
    type_registry: GenerativeTypeRegistry,
    /// Type bounds checker
    bounds_checker: TypeBoundsChecker,
    /// Import resolution cache
    import_cache: BTreeMap<(ComponentInstanceId, BoundedString<64>), ResolvedImport>,
    /// Export resolution cache
    export_cache: BTreeMap<(ComponentInstanceId, BoundedString<64>), ResolvedExport>,
}

impl ComponentResolver {
    pub fn new() -> core::result::Result<Self, ComponentError> {
        Ok(Self {
            type_registry: GenerativeTypeRegistry::new(),
            bounds_checker: TypeBoundsChecker::new()?,
            import_cache: BTreeMap::new(),
            export_cache: BTreeMap::new(),
        })
    }

    /// Resolve an import for a component instance
    pub fn resolve_import(
        &mut self,
        instance_id: ComponentInstanceId,
        import_name: BoundedString<64>,
        provided_value: ImportValue,
    ) -> core::result::Result<ResolvedImport, ComponentError> {
        // Check cache first
        let cache_key = (instance_id, import_name.clone());
        if let Some(cached) = self.import_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Validate import type compatibility
        let val_type = self.get_import_type(&provided_value)?;

        let resolved = ResolvedImport {
            name: import_name,
            value: provided_value,
            val_type,
        };

        self.import_cache.insert(cache_key, resolved.clone());
        Ok(resolved)
    }

    /// Resolve an export from a component instance
    pub fn resolve_export(
        &mut self,
        instance_id: ComponentInstanceId,
        export_name: BoundedString<64>,
        export_value: ExportValue,
    ) -> core::result::Result<ResolvedExport, ComponentError> {
        // Check cache first
        let cache_key = (instance_id, export_name.clone());
        if let Some(cached) = self.export_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Validate export type
        let val_type = self.get_export_type(&export_value)?;

        let resolved = ResolvedExport {
            name: export_name,
            value: export_value,
            val_type,
        };

        self.export_cache.insert(cache_key, resolved.clone());
        Ok(resolved)
    }

    /// Check if an import can be satisfied by an export
    pub fn can_satisfy_import(
        &mut self,
        import: &ResolvedImport,
        export: &ResolvedExport,
    ) -> core::result::Result<bool, ComponentError> {
        match (&import.value, &export.value) {
            (
                ImportValue::Function {
                    type_id: import_type,
                    ..
                },
                ExportValue::Function {
                    type_id: export_type,
                    ..
                },
            ) => self.check_type_compatibility(*import_type, *export_type),
            (
                ImportValue::Global {
                    type_id: import_type,
                    ..
                },
                ExportValue::Global {
                    type_id: export_type,
                    ..
                },
            ) => self.check_type_compatibility(*import_type, *export_type),
            (
                ImportValue::Memory {
                    min_pages: import_min,
                    max_pages: import_max,
                },
                ExportValue::Memory { .. },
            ) => {
                // Memory compatibility checks would go here
                Ok(true)
            },
            (
                ImportValue::Table {
                    min_size: import_min,
                    max_size: import_max,
                },
                ExportValue::Table { .. },
            ) => {
                // Table compatibility checks would go here
                Ok(true)
            },
            (ImportValue::Instance { .. }, ExportValue::Instance { .. }) => {
                // Instance compatibility would check all nested imports/exports
                Ok(true)
            },
            (
                ImportValue::Value {
                    val_type: import_type,
                    ..
                },
                ExportValue::Value {
                    val_type: export_type,
                    ..
                },
            ) => Ok(self.are_types_compatible(import_type, export_type)),
            _ => Ok(false), // Different kinds of imports/exports
        }
    }

    /// Check type compatibility using the bounds checker
    fn check_type_compatibility(
        &mut self,
        import_type: TypeId,
        export_type: TypeId,
    ) -> core::result::Result<bool, ComponentError> {
        // Check if export type is a subtype of import type
        let result = self.bounds_checker.check_type_bound(
            export_type,
            import_type,
            crate::generative_types::BoundKind::Sub,
        );

        Ok(result == crate::type_bounds::RelationResult::Satisfied)
    }

    /// Check if two ValTypes are compatible
    fn are_types_compatible(&self, import_type: &ValType, export_type: &ValType) -> bool {
        match (import_type, export_type) {
            // Exact matches
            (ValType::Bool, ValType::Bool) => true,
            (ValType::U8, ValType::U8) => true,
            (ValType::U16, ValType::U16) => true,
            (ValType::U32, ValType::U32) => true,
            (ValType::U64, ValType::U64) => true,
            (ValType::S8, ValType::S8) => true,
            (ValType::S16, ValType::S16) => true,
            (ValType::S32, ValType::S32) => true,
            (ValType::S64, ValType::S64) => true,
            (ValType::F32, ValType::F32) => true,
            (ValType::F64, ValType::F64) => true,
            (ValType::Char, ValType::Char) => true,
            (ValType::String, ValType::String) => true,

            // Structural types need deep comparison
            (ValType::List(t1), ValType::List(t2)) => self.are_types_compatible(t1, t2),
            (ValType::Option(t1), ValType::Option(t2)) => self.are_types_compatible(t1, t2),

            // TODO: Add more structural type comparisons
            _ => false,
        }
    }

    /// Get the type of an import value
    fn get_import_type(
        &self,
        import: &ImportValue,
    ) -> core::result::Result<Option<ValType>, ComponentError> {
        match import {
            ImportValue::Function { .. } => Ok(None), // Function types are handled separately
            ImportValue::Global { .. } => Ok(None),   // Global types are handled separately
            ImportValue::Memory { .. } => Ok(None),
            ImportValue::Table { .. } => Ok(None),
            ImportValue::Instance { .. } => Ok(None),
            ImportValue::Value { val_type, .. } => Ok(Some(val_type.clone())),
        }
    }

    /// Get the type of an export value
    fn get_export_type(
        &self,
        export: &ExportValue,
    ) -> core::result::Result<Option<ValType>, ComponentError> {
        match export {
            ExportValue::Function { .. } => Ok(None), // Function types are handled separately
            ExportValue::Global { .. } => Ok(None),   // Global types are handled separately
            ExportValue::Memory { .. } => Ok(None),
            ExportValue::Table { .. } => Ok(None),
            ExportValue::Instance { .. } => Ok(None),
            ExportValue::Value { val_type, .. } => Ok(Some(val_type.clone())),
        }
    }

    /// Clear resolution caches
    pub fn clear_caches(&mut self) {
        self.import_cache.clear();
        self.export_cache.clear();
    }

    /// Get type registry reference
    pub fn type_registry(&self) -> &GenerativeTypeRegistry {
        &self.type_registry
    }

    /// Get type registry mutable reference
    pub fn type_registry_mut(&mut self) -> &mut GenerativeTypeRegistry {
        &mut self.type_registry
    }

    /// Get bounds checker reference
    pub fn bounds_checker(&self) -> &TypeBoundsChecker {
        &self.bounds_checker
    }

    /// Get bounds checker mutable reference
    pub fn bounds_checker_mut(&mut self) -> &mut TypeBoundsChecker {
        &mut self.bounds_checker
    }
}

impl Default for ComponentResolver {
    fn default() -> Self {
        Self::new().expect("Failed to create ComponentResolver")
    }
}

/// Import resolution helper
#[derive(Debug, Clone)]
pub struct ImportResolution {
    /// Import name
    pub name: BoundedString<64>,
    /// Instance ID
    pub instance_id: ComponentInstanceId,
    /// Resolved value
    pub resolved_value: WrtComponentValue<ComponentProvider>,
}

/// Export resolution helper
#[derive(Debug, Clone)]
pub struct ExportResolution {
    /// Export name
    pub name: BoundedString<64>,
    /// Instance ID
    pub instance_id: ComponentInstanceId,
    /// Exported value
    pub exported_value: WrtComponentValue<ComponentProvider>,
}
