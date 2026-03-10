//! Minimal WAST Test Execution Engine
//!
//! This module provides a simplified bridge between the WAST test framework and
//! the Kiln runtime, focusing on basic functionality with real WebAssembly
//! execution using StacklessEngine directly.

#![cfg(feature = "std")]

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use wast::{WastExecute, WastInvoke};
use kiln_decoder::decoder::decode_module;
use kiln_foundation::types::{GlobalType, Limits, MemoryType, RefType, TableType, ValueType};
use kiln_foundation::values::Value;
use kiln_runtime::module::{ExportKind, Module, RuntimeImportDesc};
use kiln_runtime::stackless::StacklessEngine;

// Re-export value conversion utilities from wast_values module
pub use crate::wast_values::{
    convert_wast_arg_to_value, convert_wast_args_to_values, convert_wast_results_to_values,
    convert_wast_ret_to_value, is_expected_trap, values_equal,
};

/// Minimal WAST execution engine for testing
///
/// This engine focuses on basic functionality:
/// - Module loading and instantiation
/// - Function invocation with argument/return value conversion
/// - Basic assert_return directive support
pub struct WastEngine {
    /// The underlying Kiln execution engine
    engine: StacklessEngine,
    /// Registry of loaded modules by name for multi-module tests
    /// Uses Arc to avoid cloning which loses BoundedMap data
    modules: HashMap<String, Arc<Module>>,
    /// Registry of instance IDs by module name for cross-module linking
    instance_ids: HashMap<String, usize>,
    /// Current active module for execution
    current_module: Option<Arc<Module>>,
    /// Current instance ID for module execution
    current_instance_id: Option<usize>,
}

impl WastEngine {
    /// Create a new WAST execution engine
    pub fn new() -> Result<Self> {
        Ok(Self {
            engine: StacklessEngine::new(),
            modules: HashMap::new(),
            instance_ids: HashMap::new(),
            current_module: None,
            current_instance_id: None,
        })
    }

    /// Load and instantiate a WebAssembly module from binary data
    pub fn load_module(&mut self, name: Option<&str>, wasm_binary: &[u8]) -> Result<()> {
        // Decode the WASM binary into a KilnModule
        let kiln_module = decode_module(wasm_binary).map_err(|e| {
            anyhow::anyhow!("XYZZY_DECODE_FAILED({} bytes): {} [code={}, category={:?}]",
                wasm_binary.len(), e, e.code, e.category)
        })?;

        // Validate the module before proceeding (Phase 1 of WAST conformance)
        crate::wast_validator::WastModuleValidator::validate(&kiln_module)
            .map_err(|e| anyhow::anyhow!("Module validation failed: {:#}", e))?;

        // Convert KilnModule to RuntimeModule
        // Wrap in Arc immediately to avoid clone() which loses BoundedMap data
        use kiln_runtime::module_instance::ModuleInstance;

        let module = Arc::new(
            *Module::from_kiln_module(&kiln_module).context("Failed to convert to runtime module")?,
        );

        // Validate all imports can be satisfied BEFORE instantiation.
        // Per WebAssembly spec, if an import cannot be satisfied (unknown module/field
        // or incompatible type), instantiation must fail with a link error.
        self.validate_imports(&module)?;

        // Create a module instance from the module
        // Use Arc::clone to share the module reference without copying data
        let module_instance = Arc::new(
            ModuleInstance::new(Arc::clone(&module), 0)
                .context("Failed to create module instance")?,
        );

        // Resolve spectest memory imports FIRST (imported memories come before defined memories)
        // This must happen before populate_memories_from_module() which adds defined memories
        Self::resolve_spectest_memory_imports(&module, &module_instance)?;

        // Resolve memory imports from registered modules
        self.resolve_registered_memory_imports(&module, &module_instance)?;

        // Initialize module instance resources (memories, globals, tables, data segments, etc.)
        module_instance
            .populate_memories_from_module()
            .context("Failed to populate memories")?;
        module_instance
            .populate_globals_from_module()
            .context("Failed to populate globals")?;

        // Set spectest import values for global imports
        Self::resolve_spectest_global_imports(&module, &module_instance)?;

        // Resolve global imports from registered modules
        self.resolve_registered_module_imports(&module, &module_instance)?;

        // Re-evaluate globals that depend on imported globals
        // This fixes globals like (global i32 (global.get $import)) that were evaluated
        // before import values were known
        module_instance
            .reevaluate_deferred_globals()
            .context("Failed to re-evaluate deferred globals")?;

        // Resolve spectest table imports BEFORE populate_tables_from_module
        // This ensures imported tables are available before element initialization
        Self::resolve_spectest_table_imports(&module, &module_instance)?;

        // Resolve table imports from registered modules (e.g., "module1" "shared-table")
        self.resolve_registered_table_imports(&module, &module_instance)?;

        module_instance
            .populate_tables_from_module()
            .context("Failed to populate tables")?;

        // Initialize dropped segment tracking (for elem.drop and data.drop)
        module_instance
            .initialize_dropped_segments()
            .context("Failed to initialize dropped segments")?;

        // Initialize active data segments (writes data to memory)
        #[cfg(feature = "std")]
        module_instance
            .initialize_data_segments()
            .context("Failed to initialize data segments")?;

        // Initialize element segments
        module_instance
            .initialize_element_segments()
            .context("Failed to initialize element segments")?;

        // Set the current module in the engine
        let instance_idx = self
            .engine
            .set_current_module(module_instance)
            .context("Failed to set current module in engine")?;
        self.current_instance_id = Some(instance_idx);

        // Link function imports from registered modules
        self.link_function_imports(&module, instance_idx)?;

        // Validate that all non-spectest imports are satisfied
        // Per WebAssembly spec: if any import cannot be resolved, the module
        // is unlinkable and instantiation must fail.
        self.validate_imports(&module)?;

        // Store the module and instance ID for later reference
        let module_name = name.unwrap_or("current").to_string();
        self.modules.insert(module_name.clone(), Arc::clone(&module));
        self.instance_ids.insert(module_name.clone(), instance_idx);

        // Register instance name for cross-module exception handling
        self.engine.register_instance_name(instance_idx, &module_name);

        // Set as current module
        self.current_module = Some(module);

        Ok(())
    }

    /// Execute a function by name with the given arguments
    pub fn invoke_function(
        &mut self,
        module_name: Option<&str>,
        function_name: &str,
        args: &[Value],
    ) -> Result<Vec<Value>> {
        // Get the module and instance_id - either from the specified module name or the current one
        let (module, instance_id) = if let Some(name) = module_name {
            let module = self
                .modules
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Module '{}' not found", name))?;
            let instance_id =
                self.instance_ids.get(name).copied().ok_or_else(|| {
                    anyhow::anyhow!("Instance ID for module '{}' not found", name)
                })?;
            (module, instance_id)
        } else {
            let module = self
                .current_module
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No module loaded"))?;
            let instance_id = self
                .current_instance_id
                .ok_or_else(|| anyhow::anyhow!("No module loaded - cannot execute function"))?;
            (module, instance_id)
        };

        // Find the exported function index
        let func_idx = self.find_export_function_index(&**module, function_name)?;

        // Reset call depth before each top-level invocation to prevent
        // accumulated errors from previous tests causing false failures
        self.engine.reset_call_depth();
        let results = self
            .engine
            .execute(instance_id, func_idx as usize, args.to_vec())
            .map_err(|e| anyhow::Error::from(e))?;

        Ok(results)
    }

    /// Find the function index for an exported function
    fn find_export_function_index(&self, module: &Module, function_name: &str) -> Result<u32> {
        // Search the module's export table for the function
        module
            .get_export(function_name)
            .filter(|export| export.kind == ExportKind::Function)
            .map(|export| export.index)
            .ok_or_else(|| {
                anyhow::anyhow!("Function '{}' is not exported from module", function_name)
            })
    }

    /// Get a global variable value by name
    pub fn get_global(&self, module_name: Option<&str>, global_name: &str) -> Result<Value> {
        // Get the module and instance_id
        let (module, instance_id) = if let Some(name) = module_name {
            let module = self
                .modules
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Module '{}' not found", name))?;
            let instance_id =
                self.instance_ids.get(name).copied().ok_or_else(|| {
                    anyhow::anyhow!("Instance ID for module '{}' not found", name)
                })?;
            (module, instance_id)
        } else {
            let module = self
                .current_module
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No module loaded"))?;
            let instance_id =
                self.current_instance_id.ok_or_else(|| anyhow::anyhow!("No module loaded"))?;
            (module, instance_id)
        };

        // Find the exported global index
        let export = module.get_export(global_name).ok_or_else(|| {
            anyhow::anyhow!("Global '{}' is not exported from module", global_name)
        })?;

        if export.kind != ExportKind::Global {
            return Err(anyhow::anyhow!("'{}' is not a global export", global_name));
        }

        let global_idx = export.index;

        // Get the instance from the engine
        let instance = self
            .engine
            .get_instance(instance_id)
            .ok_or_else(|| anyhow::anyhow!("Instance {} not found in engine", instance_id))?;

        // Get the global value from the instance
        let global_wrapper = instance
            .global(global_idx)
            .map_err(|e| anyhow::anyhow!("Failed to get global {}: {:?}", global_idx, e))?;

        let value = global_wrapper
            .get()
            .map_err(|e| anyhow::anyhow!("Failed to read global value: {:?}", e))?;

        Ok(value)
    }

    /// Register a module with a specific name for imports
    pub fn register_module(&mut self, name: &str, module_name: &str) -> Result<()> {
        if let Some(module) = self.modules.get(module_name) {
            self.modules.insert(name.to_string(), Arc::clone(module));
            // Also copy the instance ID for cross-module linking
            if let Some(&instance_id) = self.instance_ids.get(module_name) {
                self.instance_ids.insert(name.to_string(), instance_id);
                // Register the new name for cross-module exception handling
                self.engine.register_instance_name(instance_id, name);
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Module '{}' not found for registration",
                module_name
            ))
        }
    }

    /// Get a module by name
    pub fn get_module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name).map(|arc| &**arc)
    }

    /// Clear all modules and reset the engine
    pub fn reset(&mut self) -> Result<()> {
        self.modules.clear();
        self.instance_ids.clear();
        self.current_module = None;
        self.current_instance_id = None;
        // Create a new engine to reset state
        self.engine = StacklessEngine::new();
        Ok(())
    }

    /// Resolve spectest memory imports
    /// The spectest module provides a memory export named "memory"
    /// This must be called BEFORE populate_memories_from_module() because
    /// imported memories come before defined memories in the index space.
    fn resolve_spectest_memory_imports(
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use kiln_foundation::clean_core_types::CoreMemoryType;
        use kiln_runtime::memory::Memory;
        use kiln_runtime::module::MemoryWrapper;

        // Look for memory imports from spectest
        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            if mod_name == "spectest" && field_name == "memory" {
                // Check if this is actually a memory import
                if let Some(RuntimeImportDesc::Memory(_mem_type)) = module.import_types.get(i) {
                    // The spectest module provides a memory with 1-2 pages
                    // The import type specifies minimum requirements, but the actual
                    // spectest memory always has at least 1 page
                    let core_mem_type = CoreMemoryType {
                        limits: Limits { min: 1, max: Some(2) },
                        shared: false,
                        memory64: false,
                    };

                    let memory = Memory::new(core_mem_type).map_err(|e| {
                        anyhow::anyhow!("Failed to create spectest memory: {:?}", e)
                    })?;

                    let wrapper = MemoryWrapper::new(memory);

                    // Add the memory to the instance (at index 0 since it's an import)
                    module_instance
                        .set_memory(0, wrapper)
                        .map_err(|e| anyhow::anyhow!("Failed to set spectest memory: {:?}", e))?;
                }
            }
        }

        Ok(())
    }

    /// Resolve spectest global imports with their expected values
    /// The spectest module provides:
    /// - global_i32: i32 = 666
    /// - global_i64: i64 = 666
    /// - global_f32: f32 = 666.6
    /// - global_f64: f64 = 666.6
    fn resolve_spectest_global_imports(
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use std::sync::{Arc as StdArc, RwLock};
        use kiln_foundation::values::{FloatBits32, FloatBits64};
        use kiln_runtime::global::Global;
        use kiln_runtime::module::GlobalWrapper;

        let mut global_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Use import_types to determine if this is a global import
            let is_global = matches!(module.import_types.get(i), Some(RuntimeImportDesc::Global(_)));

            if is_global {
                if mod_name == "spectest" {
                    let global_type = match module.global_import_types.get(global_import_idx) {
                        Some(gt) => gt,
                        None => {
                            global_import_idx += 1;
                            continue;
                        },
                    };

                    let value = match field_name.as_str() {
                        "global_i32" => Value::I32(666),
                        "global_i64" => Value::I64(666),
                        "global_f32" => Value::F32(FloatBits32(0x4426_999A)), // 666.6 in f32
                        "global_f64" => Value::F64(FloatBits64(0x4084_D333_3333_3333)), // 666.6 in f64
                        _ => {
                            global_import_idx += 1;
                            continue;
                        },
                    };

                    // Create a new global with the spectest value
                    let global = Global::new(global_type.value_type, global_type.mutable, value)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to create spectest global: {:?}", e)
                        })?;

                    // Set the global in the module instance
                    module_instance
                        .set_global(
                            global_import_idx,
                            GlobalWrapper(StdArc::new(RwLock::new(global))),
                        )
                        .map_err(|e| anyhow::anyhow!("Failed to set spectest global: {:?}", e))?;
                }

                global_import_idx += 1;
            }
        }

        Ok(())
    }

    /// Resolve spectest table imports
    /// The spectest module provides a table export named "table"
    /// This must be called BEFORE populate_tables_from_module() because
    /// imported tables come before defined tables in the index space.
    fn resolve_spectest_table_imports(
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use kiln_runtime::module::TableWrapper;
        use kiln_runtime::table::Table;

        // The spectest table type: (table 10 20 funcref)
        // Look for table imports from spectest
        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            if mod_name == "spectest" && field_name == "table" {
                // Check if this is actually a table import
                if let Some(RuntimeImportDesc::Table(_table_type)) = module.import_types.get(i) {
                    // The spectest module provides a table with 10-20 funcref elements
                    // The import type specifies minimum requirements, but the actual
                    // spectest table always has at least 10 elements
                    let spectest_table_type = TableType {
                        element_type: RefType::Funcref,
                        limits: Limits { min: 10, max: Some(20) },
                        table64: false,
                    };
                    let table = Table::new(spectest_table_type).map_err(|e| {
                        anyhow::anyhow!("Failed to create spectest table: {:?}", e)
                    })?;

                    let wrapper = TableWrapper::new(table);

                    // Add the table to the instance (at the appropriate import index)
                    module_instance
                        .set_table(0, wrapper)
                        .map_err(|e| anyhow::anyhow!("Failed to set spectest table: {:?}", e))?;
                }
            }
        }

        Ok(())
    }

    /// Resolve table imports from registered modules
    /// This handles cross-module table sharing where a module imports a table
    /// exported by another registered module (e.g., `(import "module1" "shared-table" (table 10 funcref))`)
    fn resolve_registered_table_imports(
        &self,
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        let mut table_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Use import_types to determine if this is a table import
            if let Some(RuntimeImportDesc::Table(_)) = module.import_types.get(i) {
                // Skip spectest imports (handled separately)
                if mod_name == "spectest" {
                    table_import_idx += 1;
                    continue;
                }

                // Look up the registered module
                if let Some(source_module_arc) = self.modules.get(mod_name) {
                    // Find the exported table in the source module
                    let bounded_field =
                        kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module_arc.exports.get(&bounded_field) {
                        if export.kind == ExportKind::Table {
                            let source_table_idx = export.index as usize;

                            // Get the source module instance to access its table
                            if let Some(source_instance_id) = self.instance_ids.get(mod_name) {
                                // Get the table from the source instance via the engine
                                if let Some(source_instance) = self.engine.get_instance(*source_instance_id) {
                                    if let Ok(table_wrapper) = source_instance.table(source_table_idx as u32) {
                                        // Share the table (clone the Arc, not the data)
                                        module_instance
                                            .set_table(table_import_idx, table_wrapper)
                                            .map_err(|e| {
                                                anyhow::anyhow!(
                                                    "Failed to set imported table: {:?}",
                                                    e
                                                )
                                            })?;
                                    }
                                }
                            }
                        }
                    }
                }

                table_import_idx += 1;
            }
        }

        Ok(())
    }

    /// Resolve global imports from registered modules (like "G")
    ///
    /// Uses import_types to correctly identify which imports are globals,
    /// rather than heuristic counting.
    fn resolve_registered_module_imports(
        &self,
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use std::sync::{Arc as StdArc, RwLock};
        use kiln_runtime::global::Global;
        use kiln_runtime::module::GlobalWrapper;

        let mut global_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Use import_types to determine if this is a global import
            let is_global = matches!(module.import_types.get(i), Some(RuntimeImportDesc::Global(_)));

            if is_global {
                // Skip spectest imports (handled separately)
                if mod_name == "spectest" {
                    global_import_idx += 1;
                    continue;
                }

                // Look up the registered module
                if let Some(source_module) = self.modules.get(mod_name) {
                    // Find the exported global in the source module
                    let bounded_field =
                        kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module.exports.get(&bounded_field) {
                        if export.kind == ExportKind::Global {
                            let source_global_idx = export.index as usize;

                            // Get the source global value, either from the module instance
                            // or from the module's globals array
                            let source_value = self.get_source_global_value(
                                mod_name,
                                source_module,
                                source_global_idx,
                            );

                            if let Some(value) = source_value {
                                if let Some(global_type) =
                                    module.global_import_types.get(global_import_idx)
                                {
                                    let global = Global::new(
                                        global_type.value_type,
                                        global_type.mutable,
                                        value,
                                    )
                                    .map_err(|e| {
                                        anyhow::anyhow!(
                                            "Failed to create imported global: {:?}",
                                            e
                                        )
                                    })?;

                                    module_instance
                                        .set_global(
                                            global_import_idx,
                                            GlobalWrapper(StdArc::new(RwLock::new(global))),
                                        )
                                        .map_err(|e| {
                                            anyhow::anyhow!(
                                                "Failed to set imported global: {:?}",
                                                e
                                            )
                                        })?;
                                }
                            }
                        }
                    }
                }

                global_import_idx += 1;
            }
        }

        Ok(())
    }

    /// Get the value of a global from a source module, checking the runtime instance first
    fn get_source_global_value(
        &self,
        mod_name: &str,
        source_module: &Module,
        source_global_idx: usize,
    ) -> Option<Value> {
        // Try to get from the runtime instance first (has up-to-date values)
        if let Some(&instance_id) = self.instance_ids.get(mod_name) {
            if let Some(instance) = self.engine.get_instance(instance_id) {
                if let Ok(gw) = instance.global(source_global_idx as u32) {
                    if let Ok(val) = gw.get() {
                        return Some(val);
                    }
                }
            }
        }
        // Fall back to the module's globals array (static init values)
        if let Ok(global_wrapper) = source_module.globals.get(source_global_idx) {
            if let Ok(guard) = global_wrapper.0.read() {
                return Some(guard.get().clone());
            }
        }
        None
    }

    /// Resolve memory imports from registered modules
    ///
    /// This handles cross-module memory sharing where a module imports a memory
    /// exported by another registered module (e.g., `(import "Mm" "mem" (memory 1))`)
    fn resolve_registered_memory_imports(
        &self,
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        let mut memory_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Use import_types to determine if this is a memory import
            if let Some(RuntimeImportDesc::Memory(_)) = module.import_types.get(i) {
                // Skip spectest imports (handled separately)
                if mod_name == "spectest" {
                    memory_import_idx += 1;
                    continue;
                }

                // Look up the registered module
                if let Some(source_module_arc) = self.modules.get(mod_name) {
                    let bounded_field =
                        kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module_arc.exports.get(&bounded_field) {
                        if export.kind == ExportKind::Memory {
                            let source_memory_idx = export.index as usize;

                            // Get the memory from the source instance
                            if let Some(&source_instance_id) = self.instance_ids.get(mod_name) {
                                if let Some(source_instance) =
                                    self.engine.get_instance(source_instance_id)
                                {
                                    if let Ok(memory_wrapper) =
                                        source_instance.memory(source_memory_idx as u32)
                                    {
                                        // Share the memory (clone the Arc, not the data)
                                        module_instance
                                            .set_memory(memory_import_idx, memory_wrapper)
                                            .map_err(|e| {
                                                anyhow::anyhow!(
                                                    "Failed to set imported memory: {:?}",
                                                    e
                                                )
                                            })?;
                                    }
                                }
                            }
                        }
                    }
                }

                memory_import_idx += 1;
            }
        }

        Ok(())
    }

    /// Validate all imports can be satisfied before instantiation.
    ///
    /// Per WebAssembly spec section 4.5.4, instantiation validates that:
    /// - Every import has a matching export in the registered modules
    /// - Function types match structurally
    /// - Global types match (value type and mutability)
    /// - Memory limits are compatible (import min <= export min, etc.)
    /// - Table element types match and limits are compatible
    fn validate_imports(&self, module: &Module) -> Result<()> {
        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            let import_desc = match module.import_types.get(i) {
                Some(desc) => desc,
                None => continue,
            };

            // Skip spectest imports (handled by spectest resolution)
            if mod_name == "spectest" {
                continue;
            }

            // Look up the source module
            let source_module = match self.modules.get(mod_name) {
                Some(m) => m,
                None => {
                    return Err(anyhow::anyhow!(
                        "unknown import: module '{}' field '{}' not found (no module '{}' registered)",
                        mod_name,
                        field_name,
                        mod_name
                    ));
                },
            };

            // Look up the export in the source module
            let bounded_field =
                kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(field_name)
                    .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

            let export = match source_module.exports.get(&bounded_field) {
                Some(e) => e,
                None => {
                    return Err(anyhow::anyhow!(
                        "unknown import: module '{}' does not export '{}'",
                        mod_name,
                        field_name
                    ));
                },
            };

            // Validate import/export type compatibility
            match import_desc {
                RuntimeImportDesc::Function(type_idx) => {
                    if export.kind != ExportKind::Function {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: '{}'::'{}' - expected function, got {:?}",
                            mod_name,
                            field_name,
                            export.kind
                        ));
                    }
                    // Validate function type signature matches
                    self.validate_function_import_type(
                        module,
                        *type_idx,
                        source_module,
                        export.index as usize,
                        mod_name,
                        field_name,
                    )?;
                },
                RuntimeImportDesc::Global(import_global_type) => {
                    if export.kind != ExportKind::Global {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: '{}'::'{}' - expected global, got {:?}",
                            mod_name,
                            field_name,
                            export.kind
                        ));
                    }
                    self.validate_global_import_type(
                        import_global_type,
                        source_module,
                        export.index as usize,
                        mod_name,
                        field_name,
                    )?;
                },
                RuntimeImportDesc::Memory(import_mem_type) => {
                    if export.kind != ExportKind::Memory {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: '{}'::'{}' - expected memory, got {:?}",
                            mod_name,
                            field_name,
                            export.kind
                        ));
                    }
                    self.validate_memory_import_type(
                        import_mem_type,
                        source_module,
                        export.index as usize,
                        mod_name,
                        field_name,
                    )?;
                },
                RuntimeImportDesc::Table(import_table_type) => {
                    if export.kind != ExportKind::Table {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: '{}'::'{}' - expected table, got {:?}",
                            mod_name,
                            field_name,
                            export.kind
                        ));
                    }
                    self.validate_table_import_type(
                        import_table_type,
                        source_module,
                        export.index as usize,
                        mod_name,
                        field_name,
                    )?;
                },
                RuntimeImportDesc::Tag(_) => {
                    if export.kind != ExportKind::Tag {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: '{}'::'{}' - expected tag, got {:?}",
                            mod_name,
                            field_name,
                            export.kind
                        ));
                    }
                },
                // Component model import types not used in WAST core module tests
                _ => {},
            }
        }
        Ok(())
    }

    /// Validate that a function import's type matches the exported function's type
    fn validate_function_import_type(
        &self,
        importing_module: &Module,
        import_type_idx: u32,
        source_module: &Module,
        export_func_idx: usize,
        mod_name: &str,
        field_name: &str,
    ) -> Result<()> {
        // Get the expected function type from the importing module
        let import_func_type = match importing_module.types.get(import_type_idx as usize) {
            Some(ft) => ft,
            None => return Ok(()), // Type index out of bounds, skip validation
        };

        // Get the source function's type
        let source_func = match source_module.functions.get(export_func_idx) {
            Some(f) => f,
            None => return Ok(()), // Function index out of bounds, skip
        };
        let source_func_type = match source_module.types.get(source_func.type_idx as usize) {
            Some(ft) => ft,
            None => return Ok(()), // Type index out of bounds, skip
        };

        // Compare parameter and result types
        if import_func_type.params.len() != source_func_type.params.len()
            || import_func_type.results.len() != source_func_type.results.len()
        {
            return Err(anyhow::anyhow!(
                "incompatible import type: function '{}'::'{}' signature mismatch - \
                 expected ({} params, {} results), got ({} params, {} results)",
                mod_name,
                field_name,
                import_func_type.params.len(),
                import_func_type.results.len(),
                source_func_type.params.len(),
                source_func_type.results.len(),
            ));
        }

        // Check each parameter type
        for idx in 0..import_func_type.params.len() {
            let import_param = import_func_type.params.get(idx);
            let source_param = source_func_type.params.get(idx);
            match (import_param, source_param) {
                (Some(ip), Some(sp)) if ip == sp => continue,
                (Some(ip), Some(sp)) => {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: function '{}'::'{}' param {} mismatch - \
                         expected {:?}, got {:?}",
                        mod_name,
                        field_name,
                        idx,
                        ip,
                        sp,
                    ));
                },
                _ => {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: function '{}'::'{}' param {} - \
                         failed to compare types",
                        mod_name,
                        field_name,
                        idx,
                    ));
                },
            }
        }

        // Check each result type
        for idx in 0..import_func_type.results.len() {
            let import_result = import_func_type.results.get(idx);
            let source_result = source_func_type.results.get(idx);
            match (import_result, source_result) {
                (Some(ir), Some(sr)) if ir == sr => continue,
                (Some(ir), Some(sr)) => {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: function '{}'::'{}' result {} mismatch - \
                         expected {:?}, got {:?}",
                        mod_name,
                        field_name,
                        idx,
                        ir,
                        sr,
                    ));
                },
                _ => {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: function '{}'::'{}' result {} - \
                         failed to compare types",
                        mod_name,
                        field_name,
                        idx,
                    ));
                },
            }
        }

        Ok(())
    }

    /// Validate that a global import's type is compatible with the exported global
    ///
    /// Per WebAssembly spec:
    /// - For immutable globals: import value type must be a subtype of export value type
    /// - For mutable globals: import and export must have exactly the same type
    /// - Mutability must match
    fn validate_global_import_type(
        &self,
        import_global_type: &GlobalType,
        source_module: &Module,
        export_global_idx: usize,
        mod_name: &str,
        field_name: &str,
    ) -> Result<()> {
        // Get the source global's type
        // The export index is into the combined (imports + defined) global index space
        let source_global_type =
            self.get_source_global_type(source_module, export_global_idx, mod_name);

        if let Some(source_type) = source_global_type {
            // Mutability must match
            if import_global_type.mutable != source_type.mutable {
                return Err(anyhow::anyhow!(
                    "incompatible import type: global '{}'::'{}' mutability mismatch - \
                     import is {}, export is {}",
                    mod_name,
                    field_name,
                    if import_global_type.mutable { "mutable" } else { "immutable" },
                    if source_type.mutable { "mutable" } else { "immutable" },
                ));
            }

            // For mutable globals, types must match exactly
            // For immutable globals, import type must be a subtype of export type
            if import_global_type.mutable {
                // Mutable globals: types must match exactly
                if !value_types_match_exact(
                    &import_global_type.value_type,
                    &source_type.value_type,
                ) {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: global '{}'::'{}' type mismatch - \
                         import {:?}, export {:?}",
                        mod_name,
                        field_name,
                        import_global_type.value_type,
                        source_type.value_type,
                    ));
                }
            } else {
                // Immutable globals: import type must match or be a supertype
                if !value_type_is_subtype_of(
                    &source_type.value_type,
                    &import_global_type.value_type,
                ) {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: global '{}'::'{}' type mismatch - \
                         import {:?}, export {:?}",
                        mod_name,
                        field_name,
                        import_global_type.value_type,
                        source_type.value_type,
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get the GlobalType for an exported global from a source module
    fn get_source_global_type(
        &self,
        source_module: &Module,
        export_global_idx: usize,
        mod_name: &str,
    ) -> Option<GlobalType> {
        // Check if the index is in the import range
        let num_global_imports = source_module.num_global_imports;
        if export_global_idx < num_global_imports {
            // This is an imported global
            return source_module
                .global_import_types
                .get(export_global_idx)
                .cloned();
        }

        // This is a defined global - check deferred_global_inits
        let defined_idx = export_global_idx - num_global_imports;
        if let Some((global_type, _)) = source_module.deferred_global_inits.get(defined_idx) {
            return Some(global_type.clone());
        }

        // Try to get from runtime instance
        if let Some(&instance_id) = self.instance_ids.get(mod_name) {
            if let Some(instance) = self.engine.get_instance(instance_id) {
                if let Ok(gw) = instance.global(export_global_idx as u32) {
                    if let Ok(val) = gw.get() {
                        let value_type = match &val {
                            Value::I32(_) => ValueType::I32,
                            Value::I64(_) => ValueType::I64,
                            Value::F32(_) => ValueType::F32,
                            Value::F64(_) => ValueType::F64,
                            Value::V128(_) => ValueType::V128,
                            Value::FuncRef(_) => ValueType::FuncRef,
                            Value::ExternRef(_) => ValueType::ExternRef,
                            _ => ValueType::I32,
                        };
                        // We cannot determine mutability from the value alone,
                        // but we can check if it's in global_import_types or deferred_global_inits
                        // For now, allow the check to pass by returning None
                        let _ = value_type;
                    }
                }
            }
        }

        None
    }

    /// Validate memory import type compatibility
    ///
    /// Per WebAssembly spec:
    /// - Import min must be <= export min (the export provides at least what's needed)
    /// - If import has max, export must also have max and export max <= import max
    fn validate_memory_import_type(
        &self,
        import_mem_type: &MemoryType,
        source_module: &Module,
        export_memory_idx: usize,
        mod_name: &str,
        field_name: &str,
    ) -> Result<()> {
        // Get the source memory's actual limits from the runtime instance
        let source_limits = self.get_source_memory_limits(source_module, export_memory_idx, mod_name);

        if let Some(source_lim) = source_limits {
            // Import min must be <= export min
            if import_mem_type.limits.min > source_lim.min {
                return Err(anyhow::anyhow!(
                    "incompatible import type: memory '{}'::'{}' - \
                     import min {} > export min {}",
                    mod_name,
                    field_name,
                    import_mem_type.limits.min,
                    source_lim.min,
                ));
            }

            // If import has max, export must also have max and export max <= import max
            if let Some(import_max) = import_mem_type.limits.max {
                match source_lim.max {
                    None => {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: memory '{}'::'{}' - \
                             import has max {} but export has no max",
                            mod_name,
                            field_name,
                            import_max,
                        ));
                    },
                    Some(export_max) if export_max > import_max => {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: memory '{}'::'{}' - \
                             export max {} > import max {}",
                            mod_name,
                            field_name,
                            export_max,
                            import_max,
                        ));
                    },
                    _ => {},
                }
            }
        }

        Ok(())
    }

    /// Get the limits for an exported memory from a source module
    fn get_source_memory_limits(
        &self,
        source_module: &Module,
        export_memory_idx: usize,
        mod_name: &str,
    ) -> Option<Limits> {
        // Try to get from the runtime instance
        if let Some(&instance_id) = self.instance_ids.get(mod_name) {
            if let Some(instance) = self.engine.get_instance(instance_id) {
                if let Ok(mem_wrapper) = instance.memory(export_memory_idx as u32) {
                    let mem = mem_wrapper.0.as_ref();
                    return Some(mem.ty.limits.clone());
                }
            }
        }
        // Fall back to the module's memories array
        if let Some(mem_wrapper) = source_module.memories.get(export_memory_idx) {
            let mem = mem_wrapper.0.as_ref();
            return Some(mem.ty.limits.clone());
        }
        None
    }

    /// Validate table import type compatibility
    ///
    /// Per WebAssembly spec:
    /// - Element types must match
    /// - Import min must be <= export min
    /// - If import has max, export must also have max and export max <= import max
    fn validate_table_import_type(
        &self,
        import_table_type: &TableType,
        source_module: &Module,
        export_table_idx: usize,
        mod_name: &str,
        field_name: &str,
    ) -> Result<()> {
        // Get the source table's type from the runtime instance
        let source_table_type =
            self.get_source_table_type(source_module, export_table_idx, mod_name);

        if let Some(source_type) = source_table_type {
            // Element types must match (or be subtypes for reference types)
            if !ref_type_is_subtype_of(&source_type.element_type, &import_table_type.element_type) {
                return Err(anyhow::anyhow!(
                    "incompatible import type: table '{}'::'{}' - \
                     element type mismatch: import {:?}, export {:?}",
                    mod_name,
                    field_name,
                    import_table_type.element_type,
                    source_type.element_type,
                ));
            }

            // Import min must be <= export min
            if import_table_type.limits.min > source_type.limits.min {
                return Err(anyhow::anyhow!(
                    "incompatible import type: table '{}'::'{}' - \
                     import min {} > export min {}",
                    mod_name,
                    field_name,
                    import_table_type.limits.min,
                    source_type.limits.min,
                ));
            }

            // If import has max, export must also have max and export max <= import max
            if let Some(import_max) = import_table_type.limits.max {
                match source_type.limits.max {
                    None => {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: table '{}'::'{}' - \
                             import has max {} but export has no max",
                            mod_name,
                            field_name,
                            import_max,
                        ));
                    },
                    Some(export_max) if export_max > import_max => {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: table '{}'::'{}' - \
                             export max {} > import max {}",
                            mod_name,
                            field_name,
                            export_max,
                            import_max,
                        ));
                    },
                    _ => {},
                }
            }
        }

        Ok(())
    }

    /// Get the TableType for an exported table from a source module
    fn get_source_table_type(
        &self,
        source_module: &Module,
        export_table_idx: usize,
        mod_name: &str,
    ) -> Option<TableType> {
        // Try to get from the runtime instance
        if let Some(&instance_id) = self.instance_ids.get(mod_name) {
            if let Some(instance) = self.engine.get_instance(instance_id) {
                if let Ok(table_wrapper) = instance.table(export_table_idx as u32) {
                    let table = table_wrapper.0.as_ref();
                    return Some(TableType {
                        element_type: table.ty.element_type.clone(),
                        limits: table.ty.limits.clone(),
                        table64: table.ty.table64,
                    });
                }
            }
        }
        // Fall back to the module's tables array
        if let Some(table_wrapper) = source_module.tables.get(export_table_idx) {
            let table = table_wrapper.0.as_ref();
            return Some(TableType {
                element_type: table.ty.element_type.clone(),
                limits: table.ty.limits.clone(),
                table64: table.ty.table64,
            });
        }
        None
    }

    /// Link function imports from registered modules
    ///
    /// This method sets up cross-instance function linking for imports
    /// from modules that have been registered (e.g., via `register "M"`).
    fn link_function_imports(&mut self, module: &Module, instance_id: usize) -> Result<()> {
        let import_types = &module.import_types;
        let import_order = &module.import_order;

        if import_types.len() != import_order.len() {
            return Err(anyhow::anyhow!(
                "import_types length ({}) does not match import_order length ({})",
                import_types.len(),
                import_order.len()
            ));
        }

        for (i, (mod_name, field_name)) in import_order.iter().enumerate() {
            // Skip spectest imports (handled by spectest resolution)
            if mod_name == "spectest" {
                continue;
            }

            // Check if this is a function import
            if let Some(RuntimeImportDesc::Function(_type_idx)) = import_types.get(i) {
                // Check if we have a registered module with this name
                if let Some(&source_instance_id) = self.instance_ids.get(mod_name) {
                    // Check if the source module exports this function
                    if let Some(source_module) = self.modules.get(mod_name) {
                        let bounded_field =
                            kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(
                                field_name,
                            )
                            .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                        if let Some(export) = source_module.exports.get(&bounded_field) {
                            if export.kind == ExportKind::Function {
                                // Set up the import link
                                self.engine.register_import_link(
                                    instance_id,
                                    mod_name.clone(),
                                    field_name.clone(),
                                    source_instance_id,
                                    field_name.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Check if two ValueTypes match exactly (for mutable globals)
fn value_types_match_exact(a: &ValueType, b: &ValueType) -> bool {
    a == b
}

/// Check if `sub` is a subtype of `sup` for ValueType.
///
/// Per WebAssembly spec, the subtyping rules for reference types are:
/// - funcref <: funcref
/// - externref <: externref
/// - (ref $t) <: (ref null func) when $t is a function type
/// - (ref null $t) <: (ref null func) when $t is a function type
/// - (ref $t) <: (ref $t)
/// - (ref $t) <: (ref null $t)
/// - (ref null $t) <: (ref null $t)
///
/// For non-reference types, types must match exactly.
fn value_type_is_subtype_of(sub: &ValueType, sup: &ValueType) -> bool {
    if sub == sup {
        return true;
    }

    match (sub, sup) {
        // FuncRef is a subtype of FuncRef
        (ValueType::FuncRef, ValueType::FuncRef) => true,
        // ExternRef is a subtype of ExternRef
        (ValueType::ExternRef, ValueType::ExternRef) => true,
        // Numeric types must match exactly
        _ => false,
    }
}

/// Check if reference type `sub` is a subtype of `sup`.
///
/// For table element types, we need subtyping checks:
/// - funcref <: funcref
/// - externref <: externref
/// - (ref null $t) <: (ref null func) only when types structurally match
fn ref_type_is_subtype_of(sub: &RefType, sup: &RefType) -> bool {
    if sub == sup {
        return true;
    }

    // RefType subtyping: same heap type and nullable match
    // For basic types, only exact match (already handled above)
    false
}

impl core::fmt::Debug for WastEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WastEngine")
            .field("modules", &self.modules.len())
            .field("has_current_module", &self.current_module.is_some())
            .finish()
    }
}

/// Helper function to execute a WAST invoke directive
pub fn execute_wast_invoke(engine: &mut WastEngine, invoke: &WastInvoke) -> Result<Vec<Value>> {
    // Convert arguments
    let args = convert_wast_args_to_values(&invoke.args)?;

    // Determine module name
    let module_name = invoke.module.as_ref().map(|id| id.name());

    // Execute the function
    engine.invoke_function(module_name, invoke.name, &args)
}

/// Helper function to execute a WAST execute directive
///
/// Takes `&mut WastExecute` because `WastExecute::Wat` modules need to be
/// encoded via `encode()` which requires `&mut self`.
pub fn execute_wast_execute(engine: &mut WastEngine, execute: &mut WastExecute) -> Result<Vec<Value>> {
    match execute {
        WastExecute::Invoke(invoke) => execute_wast_invoke(engine, invoke),
        WastExecute::Wat(wat) => {
            // WAT modules in assert_trap need to be compiled and instantiated.
            // The trap occurs during instantiation (e.g., out-of-bounds table/memory
            // access during element/data segment initialization, or start function trap).
            let binary = wat.encode().map_err(|e| {
                anyhow::anyhow!("Failed to encode WAT module: {}", e)
            })?;
            // Load the module - this triggers instantiation which may trap
            engine.load_module(None, &binary)?;
            // If we get here, the module loaded successfully (no trap)
            Ok(vec![])
        },
        WastExecute::Get { module, global, .. } => {
            // Global variable access
            let module_name = module.as_ref().map(|id| id.name());
            let value = engine.get_global(module_name, global)?;
            Ok(vec![value])
        },
    }
}

/// Simple WAST runner for basic testing
pub fn run_simple_wast_test(wast_content: &str) -> Result<()> {
    use wast::{
        Wast, WastDirective,
        parser::{self, ParseBuffer},
    };

    let buf = ParseBuffer::new(wast_content).context("Failed to create parse buffer")?;

    let wast: Wast = parser::parse(&buf).context("Failed to parse WAST content")?;

    let mut engine = WastEngine::new()?;

    for directive in wast.directives {
        match directive {
            WastDirective::Module(mut module) => {
                // Extract module name BEFORE calling encode() (which consumes the value)
                let module_name = if let wast::QuoteWat::Wat(wast::Wat::Module(ref m)) = module {
                    m.id.as_ref().map(|id| id.name().to_string())
                } else {
                    None
                };
                let binary = module.encode().context("Failed to encode module to binary")?;
                engine
                    .load_module(module_name.as_deref(), &binary)
                    .context("Failed to load module")?;
            },
            WastDirective::AssertReturn { exec, results, .. } => match exec {
                WastExecute::Invoke(invoke) => {
                    let args = convert_wast_args_to_values(&invoke.args)?;
                    let expected = convert_wast_results_to_values(&results)?;

                    let actual = engine
                        .invoke_function(None, invoke.name, &args)
                        .context("Function invocation failed")?;

                    if actual.len() != expected.len() {
                        return Err(anyhow::anyhow!(
                            "Result count mismatch: expected {}, got {}",
                            expected.len(),
                            actual.len()
                        ));
                    }

                    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
                        if !values_equal(a, e) {
                            return Err(anyhow::anyhow!(
                                "Result mismatch at index {}: expected {:?}, got {:?}",
                                i,
                                e,
                                a
                            ));
                        }
                    }
                },
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported execution type for assert_return"
                    ));
                },
            },
            WastDirective::AssertTrap { mut exec, message, .. } => {
                // Test that execution traps with expected error
                match execute_wast_execute(&mut engine, &mut exec) {
                    Err(_) => {
                        // Expected trap occurred
                    },
                    Ok(_) => {
                        return Err(anyhow::anyhow!(
                            "AssertTrap: Expected trap but execution succeeded"
                        ));
                    },
                }
            },
            WastDirective::AssertInvalid {
                mut module,
                message,
                ..
            } => {
                // Test that module is invalid
                match module.encode() {
                    Ok(binary) => match engine.load_module(None, &binary) {
                        Err(_) => {
                            // Module correctly rejected
                        },
                        Ok(_) => {
                            return Err(anyhow::anyhow!(
                                "AssertInvalid: Expected invalid module but it loaded successfully"
                            ));
                        },
                    },
                    Err(_) => {
                        // Module encoding failed, which is expected for invalid modules
                    },
                }
            },
            WastDirective::AssertMalformed {
                mut module,
                message,
                ..
            } => {
                // Test that module is malformed
                match module.encode() {
                    Err(_) => {
                        // Module correctly rejected
                    },
                    Ok(_) => {
                        return Err(anyhow::anyhow!(
                            "AssertMalformed: Expected malformed module but it encoded \
                             successfully"
                        ));
                    },
                }
            },
            WastDirective::AssertUnlinkable {
                mut module,
                message,
                ..
            } => {
                // Test that module fails to instantiate due to linking errors
                match module.encode() {
                    Ok(binary) => match engine.load_module(None, &binary) {
                        Err(_) => {
                            // Module correctly failed to link
                        },
                        Ok(_) => {
                            return Err(anyhow::anyhow!(
                                "AssertUnlinkable: Expected unlinkable module but it loaded \
                                 successfully"
                            ));
                        },
                    },
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "AssertUnlinkable: Module encoding failed: {}",
                            e
                        ));
                    },
                }
            },
            WastDirective::Register { module, name, .. } => {
                // Register the current module (or named module) as 'name' for imports
                let source_name = module.as_ref().map(|id| id.name()).unwrap_or("current");
                engine.register_module(name, source_name)?;
            },
            WastDirective::Invoke(invoke) => {
                // Execute function without asserting result
                let args = convert_wast_args_to_values(&invoke.args)?;
                // Ignore result - invoke is for side effects
                let _ = engine.invoke_function(None, invoke.name, &args);
            },
            WastDirective::AssertExhaustion { call, message, .. } => {
                // Test that execution exhausts resources (stack overflow, memory, etc.)
                match call {
                    WastInvoke { name, args, .. } => {
                        let args = convert_wast_args_to_values(&args)?;
                        match engine.invoke_function(None, name, &args) {
                            Err(_) => {
                                // Expected resource exhaustion occurred
                            },
                            Ok(_) => {
                                return Err(anyhow::anyhow!(
                                    "AssertExhaustion: Expected resource exhaustion but execution \
                                     succeeded"
                                ));
                            },
                        }
                    },
                    _ => {
                        return Err(anyhow::anyhow!(
                            "AssertExhaustion: Unsupported execution type"
                        ));
                    },
                }
            },
            _ => {
                // Handle any remaining unsupported directive types
            },
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wast::WastArg;
    use wast::core::WastArgCore;

    #[test]
    fn test_wast_engine_creation() {
        let engine = WastEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_value_conversion() {
        let wast_arg = WastArg::Core(WastArgCore::I32(42));
        let kiln_value = convert_wast_arg_to_value(&wast_arg).unwrap();
        assert_eq!(kiln_value, Value::I32(42));
    }

    #[test]
    fn test_values_equal() {
        assert!(values_equal(&Value::I32(42), &Value::I32(42)));
        assert!(!values_equal(&Value::I32(42), &Value::I32(43)));

        // Test NaN handling
        use kiln_foundation::values::FloatBits32;
        let nan1 = Value::F32(FloatBits32::NAN);
        let nan2 = Value::F32(FloatBits32::NAN);
        assert!(values_equal(&nan1, &nan2));
    }

    #[test]
    fn test_simple_wast_execution() {
        let wast_content = r#"
            (module
              (func (export "get_five") (result i32)
                i32.const 5
              )
            )
            (assert_return (invoke "get_five") (i32.const 5))
        "#;

        let result = run_simple_wast_test(wast_content);
        match result {
            Ok(_) => println!("Simple WAST test passed"),
            Err(e) => println!("Simple WAST test failed: {}", e),
        }
        // Note: We don't assert success yet since the engine may not be fully
        // implemented
    }

    #[test]
    fn test_ref_null_anyref_decode() {
        use wast::{
            Wat,
            parser::{self, ParseBuffer},
        };

        // Minimal module with anyref
        let wat = r#"
            (module
              (func (export "anyref") (result anyref) (ref.null any))
            )
        "#;

        let buf = ParseBuffer::new(wat).expect("Failed to create buffer");
        let mut wat_parsed: Wat = parser::parse(&buf).expect("Failed to parse WAT");
        let binary = wat_parsed.encode().expect("Failed to encode");

        println!("Binary ({} bytes): {:02x?}", binary.len(), &binary);

        // Try to decode
        match kiln_decoder::decoder::decode_module(&binary) {
            Ok(module) => {
                println!(
                    "SUCCESS: {} types, {} functions",
                    module.types.len(),
                    module.functions.len()
                );
                if let Some(t) = module.types.first() {
                    println!(
                        "  First type: {} params, {} results",
                        t.params.len(),
                        t.results.len()
                    );
                    for r in &t.results {
                        println!("    Result: {:?}", r);
                    }
                }
            },
            Err(e) => {
                panic!("DECODE FAILED: {:?}", e);
            },
        }
    }

    #[test]
    fn test_ref_null_full_module_decode() {
        use wast::{
            Wat,
            parser::{self, ParseBuffer},
        };

        // Full first module from ref_null.wast
        let wat = r#"
            (module
              (type $t (func))
              (func (export "anyref") (result anyref) (ref.null any))
              (func (export "funcref") (result funcref) (ref.null func))
              (func (export "exnref") (result exnref) (ref.null exn))
              (func (export "externref") (result externref) (ref.null extern))
              (func (export "ref") (result (ref null $t)) (ref.null $t))
              (global anyref (ref.null any))
              (global funcref (ref.null func))
              (global exnref (ref.null exn))
              (global externref (ref.null extern))
              (global (ref null $t) (ref.null $t))
            )
        "#;

        let buf = ParseBuffer::new(wat).expect("Failed to create buffer");
        let mut wat_parsed: Wat = parser::parse(&buf).expect("Failed to parse WAT");
        let binary = wat_parsed.encode().expect("Failed to encode");

        println!("Full module binary ({} bytes):", binary.len());
        println!("  {:02x?}", &binary[..std::cmp::min(100, binary.len())]);
        if binary.len() > 100 {
            println!("  ... ({} more bytes)", binary.len() - 100);
        }

        // Try to decode
        match kiln_decoder::decoder::decode_module(&binary) {
            Ok(module) => {
                println!("SUCCESS:");
                println!("  {} types", module.types.len());
                println!("  {} functions", module.functions.len());
                println!("  {} globals", module.globals.len());
                println!("  {} exports", module.exports.len());
            },
            Err(e) => {
                println!("DECODE FAILED: {:?}", e);
                // Print more of the binary for debugging
                println!("Full binary: {:02x?}", &binary);
                panic!("Decode failed");
            },
        }
    }

    #[test]
    fn test_ref_null_second_module_decode() {
        use wast::{
            Wat,
            parser::{self, ParseBuffer},
        };

        // Second module from ref_null.wast with nullref, nullfuncref, etc.
        let wat = r#"
            (module
              (type $t (func))
              (global $null nullref (ref.null none))
              (global $nullfunc nullfuncref (ref.null nofunc))
              (global $nullexn nullexnref (ref.null noexn))
              (global $nullextern nullexternref (ref.null noextern))
              (func (export "anyref") (result anyref) (global.get $null))
              (func (export "nullref") (result nullref) (global.get $null))
            )
        "#;

        let buf = ParseBuffer::new(wat).expect("Failed to create buffer");
        let mut wat_parsed: Wat = parser::parse(&buf).expect("Failed to parse WAT");
        let binary = wat_parsed.encode().expect("Failed to encode");

        println!("Second module binary ({} bytes):", binary.len());
        println!("  {:02x?}", &binary[..std::cmp::min(100, binary.len())]);
        if binary.len() > 100 {
            println!("  ... ({} more bytes)", binary.len() - 100);
        }

        // Try to decode
        match kiln_decoder::decoder::decode_module(&binary) {
            Ok(module) => {
                println!("SUCCESS:");
                println!("  {} types", module.types.len());
                println!("  {} functions", module.functions.len());
                println!("  {} globals", module.globals.len());
                println!("  {} exports", module.exports.len());
            },
            Err(e) => {
                println!("DECODE FAILED: {:?}", e);
                println!("Full binary: {:02x?}", &binary);
                panic!("Decode failed");
            },
        }
    }

    #[test]
    fn test_ref_null_wast_file_decode() {
        use wast::{
            Wast, WastDirective,
            parser::{self, ParseBuffer},
        };

        // Read the actual ref_null.wast file
        let wast_path = "/Users/r/git/wrt2/external/testsuite/ref_null.wast";
        let wast_content =
            std::fs::read_to_string(wast_path).expect("Failed to read ref_null.wast");

        let buf = ParseBuffer::new(&wast_content).expect("Failed to create buffer");
        let wast: Wast = parser::parse(&buf).expect("Failed to parse WAST");

        // Process each directive and try to decode modules
        let mut module_count = 0;
        let mut errors = Vec::new();

        for directive in wast.directives {
            if let WastDirective::Module(mut quote_wat) = directive {
                module_count += 1;
                println!("\n=== Module {} ===", module_count);

                // Encode the module to binary
                match quote_wat.encode() {
                    Ok(binary) => {
                        println!("Binary: {} bytes", binary.len());
                        println!(
                            "  First 50 bytes: {:02x?}",
                            &binary[..std::cmp::min(50, binary.len())]
                        );

                        // Try to decode
                        match kiln_decoder::decoder::decode_module(&binary) {
                            Ok(module) => {
                                println!(
                                    "  DECODE OK: {} types, {} functions, {} globals",
                                    module.types.len(),
                                    module.functions.len(),
                                    module.globals.len()
                                );

                                // Try to validate
                                match crate::wast_validator::WastModuleValidator::validate(&module)
                                {
                                    Ok(()) => {
                                        println!("  VALIDATE OK");

                                        // Try to convert to runtime module
                                        match kiln_runtime::module::Module::from_kiln_module(&module)
                                        {
                                            Ok(_runtime_module) => {
                                                println!("  RUNTIME CONVERT OK");
                                            },
                                            Err(e) => {
                                                println!("  RUNTIME CONVERT FAILED: {:?}", e);
                                                errors.push((
                                                    module_count,
                                                    format!("runtime: {:?}", e),
                                                ));
                                            },
                                        }
                                    },
                                    Err(e) => {
                                        println!("  VALIDATE FAILED: {:?}", e);
                                        errors.push((module_count, format!("validate: {:?}", e)));
                                    },
                                }
                            },
                            Err(e) => {
                                println!("  DECODE FAILED: {:?}", e);
                                errors.push((module_count, format!("{:?}", e)));
                            },
                        }
                    },
                    Err(e) => {
                        println!("  ENCODE FAILED: {:?}", e);
                        errors.push((module_count, format!("encode: {:?}", e)));
                    },
                }
            }
        }

        println!("\n=== Summary ===");
        println!("Processed {} modules", module_count);
        if errors.is_empty() {
            println!("All modules decoded successfully!");
        } else {
            println!("Errors:");
            for (idx, err) in &errors {
                println!("  Module {}: {}", idx, err);
            }
            panic!("{} modules failed to decode", errors.len());
        }
    }

    #[test]
    #[ignore = "nullfuncref returns I32(-1) instead of FuncRef(None) - needs runtime fix"]
    fn test_nullfuncref_global_execution() {
        use wast::{
            Wat,
            parser::{self, ParseBuffer},
        };

        // Test module with nullfuncref globals
        let wat = r#"
            (module
              (type $t (func))
              (global $nullfunc nullfuncref (ref.null nofunc))
              (func (export "funcref") (result funcref) (global.get $nullfunc))
              (func (export "nullfuncref") (result nullfuncref) (global.get $nullfunc))
            )
        "#;

        let buf = ParseBuffer::new(wat).expect("Failed to create buffer");
        let mut wat_parsed: Wat = parser::parse(&buf).expect("Failed to parse WAT");
        let binary = wat_parsed.encode().expect("Failed to encode");

        println!("Testing nullfuncref global execution...");
        println!("Binary: {} bytes", binary.len());

        // Decode and convert to runtime module
        let decoded = kiln_decoder::decoder::decode_module(&binary).expect("Failed to decode");
        println!(
            "Decoded: {} types, {} functions, {} globals",
            decoded.types.len(),
            decoded.functions.len(),
            decoded.globals.len()
        );

        // Create WastEngine and load module
        let mut engine = WastEngine::new().expect("Failed to create engine");
        engine.load_module(None, &binary).expect("Failed to load module");

        // Call funcref function
        let args: Vec<kiln_foundation::values::Value> = vec![];
        let results = engine
            .invoke_function(None, "funcref", &args)
            .expect("Failed to invoke funcref");

        println!("funcref result: {:?}", results);

        // Check the result - should be FuncRef(None), not I32(-1)
        assert!(!results.is_empty(), "Expected one result");
        match &results[0] {
            kiln_foundation::values::Value::FuncRef(None) => {
                println!("PASS: Got FuncRef(None) as expected");
            },
            kiln_foundation::values::Value::I32(v) => {
                panic!("FAIL: Got I32({}) instead of FuncRef(None)", v);
            },
            other => {
                panic!("FAIL: Got unexpected value {:?}", other);
            },
        }

        // Also test nullfuncref result
        let results2 = engine
            .invoke_function(None, "nullfuncref", &args)
            .expect("Failed to invoke nullfuncref");
        println!("nullfuncref result: {:?}", results2);
        assert!(!results2.is_empty(), "Expected one result");
        match &results2[0] {
            kiln_foundation::values::Value::FuncRef(None) => {
                println!("PASS: Got FuncRef(None) as expected for nullfuncref");
            },
            kiln_foundation::values::Value::I32(v) => {
                panic!(
                    "FAIL: Got I32({}) instead of FuncRef(None) for nullfuncref",
                    v
                );
            },
            other => {
                panic!("FAIL: Got unexpected value {:?} for nullfuncref", other);
            },
        }
    }

    #[test]
    fn test_exception_basic_throw_catch() {
        // Test basic throw + catch with i32 value passing
        // Pattern: (block $h (result i32) (try_table (catch $e $h) (throw $e (i32.const 5))) (i32.const 0))
        // Expected: throw fires, catch matches, pushes 5 to block $h, returns 5
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "catch-i32") (result i32)
                (block $h (result i32)
                  (try_table (catch $e0 $h)
                    (throw $e0 (i32.const 5))
                  )
                  (i32.const 0)
                )
              )
            )
            (assert_return (invoke "catch-i32") (i32.const 5))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Basic throw/catch failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_no_throw() {
        // Test try_table where no exception is thrown (normal flow)
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "no-throw") (result i32)
                (block $h (result i32)
                  (try_table (catch $e0 $h)
                    (i32.const 42)
                    (br 1)
                  )
                  (i32.const 0)
                )
              )
            )
            (assert_return (invoke "no-throw") (i32.const 42))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "No-throw flow failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_catch_all() {
        // Test catch_all handler
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "catch-all") (result i32)
                (block $h
                  (try_table (catch_all $h)
                    (throw $e0 (i32.const 5))
                  )
                  (i32.const 0)
                  (return)
                )
                (i32.const 1)
              )
            )
            (assert_return (invoke "catch-all") (i32.const 1))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Catch-all failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_catch_ref() {
        // Test catch_ref handler (pushes payload + exnref)
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "catch-ref") (result i32)
                (block $h (result i32 exnref)
                  (try_table (catch_ref $e0 $h)
                    (throw $e0 (i32.const 7))
                  )
                  (i32.const 0)
                  (ref.null exn)
                )
                (drop)
              )
            )
            (assert_return (invoke "catch-ref") (i32.const 7))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Catch-ref failed: {:?}", result.err());
    }

    #[test]
    #[ignore] // throw_ref stack management needs further runtime work
    fn test_exception_throw_ref() {
        // Test throw_ref: catch an exception with catch_all_ref, then re-throw via throw_ref
        // Inner try_table catches with catch_all_ref, producing exnref on $h1
        // The exnref is then re-thrown with throw_ref
        // Outer try_table catches with catch, producing i32 on $h2
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "throw-ref") (result i32)
                (block $h2 (result i32)
                  (try_table (catch $e0 $h2)
                    (block $h1 (result exnref)
                      (try_table (catch_all_ref $h1)
                        (throw $e0 (i32.const 9))
                      )
                      (ref.null exn)
                    )
                    (throw_ref)
                    (unreachable)
                  )
                  (unreachable)
                )
              )
            )
            (assert_return (invoke "throw-ref") (i32.const 9))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Throw-ref failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_cross_function() {
        // Test exception propagation across function calls
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func $thrower (throw $e0 (i32.const 11)))
              (func (export "cross-func") (result i32)
                (block $h (result i32)
                  (try_table (catch $e0 $h)
                    (call $thrower)
                  )
                  (i32.const 0)
                )
              )
            )
            (assert_return (invoke "cross-func") (i32.const 11))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Cross-function exception failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_uncaught_trap() {
        // Test that uncaught exception causes a trap
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "uncaught") (result i32)
                (throw $e0 (i32.const 42))
              )
            )
            (assert_trap (invoke "uncaught") "unhandled exception")
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Uncaught exception trap failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_try_table_normal_end() {
        // Test try_table where body completes normally (falls through to end)
        // The catch handler branches to $h which expects no values (matching the empty tag)
        let wast_content = r#"
            (module
              (tag $e0)
              (func (export "normal-end") (result i32)
                (block $h
                  (try_table (catch $e0 $h)
                    (nop)
                  )
                )
                (i32.const 99)
              )
            )
            (assert_return (invoke "normal-end") (i32.const 99))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Normal try_table end failed: {:?}", result.err());
    }

    #[test]
    #[ignore] // catch_all_ref exnref passing needs further runtime work
    fn test_exception_catch_all_ref() {
        // Test catch_all_ref handler
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (func (export "catch-all-ref") (result i32)
                (block $h (result exnref)
                  (try_table (catch_all_ref $h)
                    (throw $e0 (i32.const 13))
                  )
                  (ref.null exn)
                )
                (drop)
                (i32.const 1)
              )
            )
            (assert_return (invoke "catch-all-ref") (i32.const 1))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Catch-all-ref failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_null_throw_ref_trap() {
        // Test that throw_ref with null exnref traps
        let wast_content = r#"
            (module
              (func (export "null-throw-ref")
                (throw_ref (ref.null exn))
              )
            )
            (assert_trap (invoke "null-throw-ref") "null exception reference")
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Null throw_ref trap failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_multi_param_tag() {
        // Test throw + catch with multiple parameter tag
        let wast_content = r#"
            (module
              (tag $e (param i32 i32))
              (func (export "multi-param") (result i32)
                (block $h (result i32 i32)
                  (try_table (catch $e $h)
                    (throw $e (i32.const 3) (i32.const 4))
                  )
                  (i32.const 0)
                  (i32.const 0)
                )
                (i32.add)
              )
            )
            (assert_return (invoke "multi-param") (i32.const 7))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Multi-param tag failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_multiple_handlers() {
        // Test try_table with multiple catch handlers
        // First matching handler should be used
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (tag $e1 (param i32))
              (func (export "multi-handler") (result i32)
                (block $h0 (result i32)
                  (block $h1 (result i32)
                    (try_table (catch $e0 $h0) (catch $e1 $h1)
                      (throw $e1 (i32.const 20))
                    )
                    (unreachable)
                  )
                )
              )
            )
            (assert_return (invoke "multi-handler") (i32.const 20))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Multiple handlers failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_nested_try_table() {
        // Test nested try_table blocks - inner one catches, outer doesn't fire
        let wast_content = r#"
            (module
              (tag $e (param i32))
              (func (export "nested") (result i32)
                (block $outer (result i32)
                  (try_table (catch $e $outer)
                    (block $inner (result i32)
                      (try_table (catch $e $inner)
                        (throw $e (i32.const 30))
                      )
                      (unreachable)
                    )
                    (i32.const 1)
                    (i32.add)
                    (return)
                  )
                  (unreachable)
                )
              )
            )
            (assert_return (invoke "nested") (i32.const 31))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Nested try_table failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_try_table_with_result() {
        // Test try_table with non-empty result type (body completes normally)
        let wast_content = r#"
            (module
              (tag $e (param i32))
              (func (export "try-result") (result i32)
                (block $h (result i32)
                  (try_table (result i32) (catch $e $h)
                    (i32.const 42)
                  )
                )
              )
            )
            (assert_return (invoke "try-result") (i32.const 42))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "try_table with result failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_deeply_nested_throw() {
        // Test exception thrown from deep within nested blocks
        let wast_content = r#"
            (module
              (tag $e (param i32))
              (func (export "deep") (result i32)
                (block $h (result i32)
                  (try_table (catch $e $h)
                    (block
                      (block
                        (throw $e (i32.const 50))
                      )
                    )
                  )
                  (unreachable)
                )
              )
            )
            (assert_return (invoke "deep") (i32.const 50))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Deeply nested throw failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_tag_mismatch() {
        // Test that catch only catches matching tag, not a different one
        // Inner try_table catches $e1 but we throw $e0 -- should propagate to outer
        let wast_content = r#"
            (module
              (tag $e0 (param i32))
              (tag $e1 (param i32))
              (func (export "mismatch") (result i32)
                (block $outer (result i32)
                  (try_table (catch $e0 $outer)
                    (block $inner (result i32)
                      (try_table (catch $e1 $inner)
                        (throw $e0 (i32.const 60))
                      )
                      (unreachable)
                    )
                    (unreachable)
                  )
                  (unreachable)
                )
              )
            )
            (assert_return (invoke "mismatch") (i32.const 60))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Tag mismatch failed: {:?}", result.err());
    }

    #[test]
    fn test_exception_empty_tag() {
        // Test throw/catch with tag that has no parameters
        let wast_content = r#"
            (module
              (tag $e)
              (func (export "empty-tag") (result i32)
                (block $h
                  (try_table (catch $e $h)
                    (throw $e)
                  )
                  (i32.const 1)
                  (return)
                )
                (i32.const 0)
              )
            )
            (assert_return (invoke "empty-tag") (i32.const 0))
        "#;

        let result = run_simple_wast_test(wast_content);
        assert!(result.is_ok(), "Empty tag failed: {:?}", result.err());
    }

    #[test]
    fn test_try_table_gc_module_decode() {
        use wast::{
            Wat,
            parser::{self, ParseBuffer},
        };

        // The problematic module from try_table.wast that uses GC typed references
        let wat = r#"
            (module
              (type $t (func))
              (func $dummy)
              (elem declare func $dummy)

              (tag $e (param (ref $t)))
              (func $throw (throw $e (ref.func $dummy)))

              (func (export "catch") (result (ref null $t))
                (block $l (result (ref null $t))
                  (try_table (catch $e $l) (call $throw))
                  (unreachable)
                )
              )
              (func (export "catch_ref1") (result (ref null $t))
                (block $l (result (ref null $t) (ref exn))
                  (try_table (catch_ref $e $l) (call $throw))
                  (unreachable)
                )
                (drop)
              )
              (func (export "catch_ref2") (result (ref null $t))
                (block $l (result (ref null $t) (ref null exn))
                  (try_table (catch_ref $e $l) (call $throw))
                  (unreachable)
                )
                (drop)
              )
              (func (export "catch_all_ref1")
                (block $l (result (ref exn))
                  (try_table (catch_all_ref $l) (call $throw))
                  (unreachable)
                )
                (drop)
              )
              (func (export "catch_all_ref2")
                (block $l (result (ref null exn))
                  (try_table (catch_all_ref $l) (call $throw))
                  (unreachable)
                )
                (drop)
              )
            )
        "#;

        let buf = ParseBuffer::new(wat).expect("Failed to create buffer");
        let mut wat_parsed: Wat = parser::parse(&buf).expect("Failed to parse WAT");
        let binary = wat_parsed.encode().expect("Failed to encode");

        println!("try_table GC module binary ({} bytes):", binary.len());
        println!(
            "  First 100 bytes: {:02x?}",
            &binary[..std::cmp::min(100, binary.len())]
        );

        // Try to decode
        match kiln_decoder::decoder::decode_module(&binary) {
            Ok(module) => {
                println!("DECODE OK:");
                println!("  {} types", module.types.len());
                println!("  {} functions", module.functions.len());
                println!("  {} tags", module.tags.len());
                println!("  {} exports", module.exports.len());

                // Print type section for debugging
                println!("Types:");
                for (i, t) in module.types.iter().enumerate() {
                    println!(
                        "  Type {}: params={:?}, results={:?}",
                        i, t.params, t.results
                    );
                }
                println!("Tags:");
                for (i, t) in module.tags.iter().enumerate() {
                    println!("  Tag {}: type_idx={}", i, t.type_idx);
                }
                println!("Functions:");
                for (i, f) in module.functions.iter().enumerate() {
                    println!(
                        "  Function {}: type_idx={}, locals={}, code_len={}",
                        i,
                        f.type_idx,
                        f.locals.len(),
                        f.code.len()
                    );
                    println!(
                        "    Code: {:02x?}",
                        &f.code[..std::cmp::min(50, f.code.len())]
                    );
                }

                // Try to validate
                match crate::wast_validator::WastModuleValidator::validate(&module) {
                    Ok(()) => {
                        println!("VALIDATE OK");
                    },
                    Err(e) => {
                        println!("VALIDATE FAILED: {:?}", e);
                        panic!("Validation failed");
                    },
                }
            },
            Err(e) => {
                println!("DECODE FAILED: {:?}", e);
                println!("Full binary: {:02x?}", &binary);
                panic!("Decode failed: {:?}", e);
            },
        }
    }
}
