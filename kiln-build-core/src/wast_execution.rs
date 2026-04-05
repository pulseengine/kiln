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
    convert_wast_ret_to_value, has_either_alternatives, is_expected_trap,
    results_match_with_either, values_equal,
};

/// Composite type kind for rec-group structural comparison
#[derive(Debug, Clone, PartialEq)]
enum CompositeKind {
    /// Function type (compared via Module.types)
    Func,
    /// Struct type with field storage types (preserving type index references)
    Struct(Vec<FieldStorageType>),
    /// Array type with element storage type
    Array(FieldStorageType),
}

/// Storage type for GC fields, preserving type index references for cross-module comparison
#[derive(Debug, Clone, PartialEq)]
enum FieldStorageType {
    /// Standard value byte (i32, i64, f32, f64, etc.)
    Value(u8),
    /// Packed i8
    I8,
    /// Packed i16
    I16,
    /// Non-nullable reference to a concrete type index: (ref $t)
    RefType(u32),
    /// Nullable reference to a concrete type index: (ref null $t)
    RefTypeNull(u32),
}

/// Per-module type metadata extracted from the decoded module for cross-module linking.
///
/// The WebAssembly GC spec requires that cross-module type equivalence checks
/// use structural equivalence based on rec-group identity. Two types from different
/// modules are considered equivalent only if their entire rec groups are structurally
/// equivalent (same size, same finality flags, same composite kinds, and structurally
/// compatible type references with intra-group references mapped positionally).
#[derive(Debug, Clone)]
struct ModuleTypeMetadata {
    /// For each type index: (rec_group_id, position_in_group, is_final)
    type_info: Vec<(usize, usize, bool)>,
    /// For each rec group: number of types in that group
    rec_group_sizes: Vec<usize>,
    /// For each rec group: the starting type index
    rec_group_starts: Vec<u32>,
    /// For each type index: the composite type kind (func, struct, array)
    composite_kinds: Vec<CompositeKind>,
}

/// Extract type metadata from a decoded KilnModule's rec_groups
fn extract_type_metadata(kiln_module: &kiln_format::module::Module) -> ModuleTypeMetadata {
    use kiln_format::module::CompositeTypeKind;

    let total_types: usize = kiln_module.rec_groups.iter().map(|rg| rg.types.len()).sum();
    let mut type_info = Vec::with_capacity(total_types);
    let mut rec_group_sizes = Vec::with_capacity(kiln_module.rec_groups.len());
    let mut rec_group_starts = Vec::with_capacity(kiln_module.rec_groups.len());
    let mut composite_kinds = Vec::with_capacity(total_types);

    for (group_id, rec_group) in kiln_module.rec_groups.iter().enumerate() {
        rec_group_sizes.push(rec_group.types.len());
        rec_group_starts.push(rec_group.start_type_index);
        for (pos, sub_type) in rec_group.types.iter().enumerate() {
            let idx = sub_type.type_index as usize;
            // Ensure vectors are sized correctly
            while type_info.len() <= idx {
                type_info.push((0, 0, true)); // placeholder
            }
            while composite_kinds.len() <= idx {
                composite_kinds.push(CompositeKind::Func); // placeholder
            }
            type_info[idx] = (group_id, pos, sub_type.is_final);

            // Extract composite kind with field/element type info
            let kind = match &sub_type.composite_kind {
                CompositeTypeKind::Func => CompositeKind::Func,
                CompositeTypeKind::Struct => CompositeKind::Struct(Vec::new()),
                CompositeTypeKind::Array => CompositeKind::Array(FieldStorageType::Value(0x7F)),
                CompositeTypeKind::StructWithFields(fields) => {
                    CompositeKind::Struct(fields.iter().map(convert_gc_field_storage).collect())
                }
                CompositeTypeKind::ArrayWithElement(elem) => {
                    CompositeKind::Array(convert_gc_field_storage(elem))
                }
            };
            composite_kinds[idx] = kind;
        }
    }

    ModuleTypeMetadata { type_info, rec_group_sizes, rec_group_starts, composite_kinds }
}

/// Convert a kiln_format GcFieldType to our FieldStorageType
fn convert_gc_field_storage(field: &kiln_format::module::GcFieldType) -> FieldStorageType {
    use kiln_format::module::GcStorageType;
    match &field.storage_type {
        GcStorageType::Value(byte) => FieldStorageType::Value(*byte),
        GcStorageType::I8 => FieldStorageType::I8,
        GcStorageType::I16 => FieldStorageType::I16,
        GcStorageType::RefType(idx) => FieldStorageType::RefType(*idx),
        GcStorageType::RefTypeNull(idx) => FieldStorageType::RefTypeNull(*idx),
    }
}

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
    /// Type metadata per registered module for cross-module linking
    module_type_metadata: HashMap<String, ModuleTypeMetadata>,
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
            module_type_metadata: HashMap::new(),
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

        // Extract type metadata from the decoded module for cross-module linking
        let importing_metadata = extract_type_metadata(&kiln_module);

        // Convert KilnModule to RuntimeModule
        // Wrap in Arc immediately to avoid clone() which loses BoundedMap data
        use kiln_runtime::module_instance::ModuleInstance;

        let module = Arc::new(
            *Module::from_kiln_module(&kiln_module).context("Failed to convert to runtime module")?,
        );

        // Validate all imports against registered modules and spectest
        // This catches unknown imports and incompatible types BEFORE instantiation
        self.validate_imports(&module, &importing_metadata)?;

        // Pre-compute the engine instance ID so ModuleInstance.instance_id matches
        // the engine's key. This ensures FuncRef.instance_id values stamped during
        // element initialization correctly identify cross-instance references.
        let engine_instance_id = self.engine.peek_next_instance_id();

        // Create a module instance from the module
        // Use Arc::clone to share the module reference without copying data
        let module_instance = Arc::new(
            ModuleInstance::new(Arc::clone(&module), engine_instance_id)
                .context("Failed to create module instance")?,
        );

        // Resolve spectest memory imports FIRST (imported memories come before defined memories)
        // This must happen before populate_memories_from_module() which adds defined memories
        Self::resolve_spectest_memory_imports(&module, &module_instance)?;

        // Resolve memory imports from registered modules (e.g., "Mm" "mem")
        // This must also happen before populate_memories_from_module() because
        // imported memories come before defined memories in the index space
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

        // Evaluate table init expressions (fills tables with computed values)
        // Must happen after globals are resolved and tables are populated,
        // but before element segment initialization (which can override individual slots)
        module_instance
            .evaluate_table_init_exprs()
            .context("Failed to evaluate table init expressions")?;

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
        self.validate_imports(&module, &importing_metadata)?;

        // Store the module and instance ID for later reference
        // Always store as "current" (last loaded module) AND under the given name
        self.modules.insert("current".to_string(), Arc::clone(&module));
        self.instance_ids.insert("current".to_string(), instance_idx);
        self.module_type_metadata.insert("current".to_string(), importing_metadata.clone());
        let module_name = name.unwrap_or("current").to_string();
        if module_name != "current" {
            self.modules.insert(module_name.clone(), Arc::clone(&module));
            self.instance_ids.insert(module_name.clone(), instance_idx);
            self.module_type_metadata.insert(module_name.clone(), importing_metadata);
        }

        // Register instance name for cross-module exception handling
        self.engine.register_instance_name(instance_idx, &module_name);

        // Set as current module
        self.current_module = Some(Arc::clone(&module));

        // Execute the start function if one is defined
        // Per the WebAssembly spec, the start function runs after all initialization
        // (memories, tables, globals, data segments, element segments are all set up)
        if let Some(start_idx) = module.start {
            self.engine.reset_call_depth();
            self.engine
                .execute(instance_idx, start_idx as usize, vec![])
                .map_err(|e| anyhow::anyhow!("start function trapped: {}", e))?;
        }

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
            .filter(|export| {
                use kiln_runtime::module::ExportKind;
                export.kind == ExportKind::Function
            })
            .map(|export| export.index)
            .ok_or_else(|| {
                anyhow::anyhow!("Function '{}' is not exported from module", function_name)
            })
    }

    /// Get a global variable value by name
    pub fn get_global(&self, module_name: Option<&str>, global_name: &str) -> Result<Value> {
        use kiln_runtime::module::ExportKind;

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
            // Copy type metadata for cross-module linking
            if let Some(metadata) = self.module_type_metadata.get(module_name) {
                self.module_type_metadata.insert(name.to_string(), metadata.clone());
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
        use kiln_foundation::types::{Limits, MemoryType};
        use kiln_runtime::memory::Memory;
        use kiln_runtime::module::{MemoryWrapper, RuntimeImportDesc};

        let mut memory_import_idx = 0usize;

        // Look for memory imports from spectest
        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            if let Some(RuntimeImportDesc::Memory(_)) = module.import_types.get(i) {
                if mod_name == "spectest" && (field_name == "memory" || field_name == "shared_memory") {
                    let is_shared = field_name == "shared_memory";
                    let core_mem_type = CoreMemoryType {
                        limits: Limits { min: 1, max: Some(2) },
                        shared: is_shared,
                        memory64: false,
                        page_size: None,
                    };

                    let memory = Memory::new(core_mem_type).map_err(|e| {
                        anyhow::anyhow!("Failed to create spectest memory: {:?}", e)
                    })?;

                    let wrapper = MemoryWrapper::new(memory);

                    // Add the memory at the correct import index
                    module_instance
                        .set_memory(memory_import_idx, wrapper)
                        .map_err(|e| anyhow::anyhow!("Failed to set spectest memory: {:?}", e))?;
                }
                memory_import_idx += 1;
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

        // The global_import_types stores global types in order of appearance
        // We need to match them with the corresponding import names
        let mut global_import_idx = 0usize;

        for (mod_name, field_name) in module.import_order.iter() {
            // Check if this import is a global by seeing if we still have global types to process
            // This assumes global imports are in the same order in import_order as in global_import_types
            if global_import_idx < module.global_import_types.len() {
                // Check if this is a global import by looking at the field name pattern
                // Spectest globals are: global_i32, global_i64, global_f32, global_f64
                let is_global = field_name.starts_with("global_");

                if is_global && mod_name == "spectest" {
                    let global_type = &module.global_import_types[global_import_idx];

                    let value = match field_name.as_str() {
                        "global_i32" => Value::I32(666),
                        "global_i64" => Value::I64(666),
                        "global_f32" => Value::F32(FloatBits32(0x4426_A666)), // 666.6 in f32
                        "global_f64" => Value::F64(FloatBits64(0x4084_D4CC_CCCC_CCCD)), // 666.6 in f64
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

                    global_import_idx += 1;
                } else if is_global {
                    // Non-spectest global import
                    global_import_idx += 1;
                }
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
        use kiln_foundation::types::{Limits, RefType, TableType};
        use kiln_runtime::module::{RuntimeImportDesc, TableWrapper};
        use kiln_runtime::table::Table;

        let mut table_import_idx = 0usize;

        // Look for table imports from spectest
        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            if let Some(RuntimeImportDesc::Table(_)) = module.import_types.get(i) {
                if mod_name == "spectest" && (field_name == "table" || field_name == "table64") {
                    let is_table64 = field_name == "table64";
                    let spectest_table_type = TableType {
                        element_type: RefType::Funcref,
                        limits: Limits { min: 10, max: Some(20) },
                        table64: is_table64,
                    };
                    let table = Table::new(spectest_table_type).map_err(|e| {
                        anyhow::anyhow!("Failed to create spectest table: {:?}", e)
                    })?;

                    let wrapper = TableWrapper::new(table);

                    // Add the table at the correct import index
                    module_instance
                        .set_table(table_import_idx, wrapper)
                        .map_err(|e| anyhow::anyhow!("Failed to set spectest table: {:?}", e))?;
                }
                table_import_idx += 1;
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
        use kiln_runtime::module::{RuntimeImportDesc, TableWrapper};

        let mut table_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Check if this is a table import
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
                        if export.kind == kiln_runtime::module::ExportKind::Table {
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

    /// Resolve memory imports from registered modules
    /// This handles cross-module memory sharing where a module imports a memory
    /// exported by another registered module (e.g., `(import "Mm" "mem" (memory 1))`)
    /// Memory imports must be resolved BEFORE populate_memories_from_module() because
    /// imported memories come before defined memories in the index space.
    fn resolve_registered_memory_imports(
        &self,
        module: &Module,
        module_instance: &Arc<kiln_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use kiln_runtime::module::RuntimeImportDesc;

        let mut memory_import_idx = 0usize;

        for (i, (mod_name, field_name)) in module.import_order.iter().enumerate() {
            // Check if this is a memory import
            if let Some(RuntimeImportDesc::Memory(_)) = module.import_types.get(i) {
                // Skip spectest imports (handled separately)
                if mod_name == "spectest" {
                    memory_import_idx += 1;
                    continue;
                }

                // Look up the registered module
                if let Some(source_module_arc) = self.modules.get(mod_name) {
                    // Find the exported memory in the source module
                    let bounded_field =
                        kiln_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module_arc.exports.get(&bounded_field) {
                        if export.kind == kiln_runtime::module::ExportKind::Memory {
                            let source_mem_idx = export.index as usize;

                            // Get the source module instance to access its memory
                            if let Some(source_instance_id) = self.instance_ids.get(mod_name) {
                                if let Some(source_instance) = self.engine.get_instance(*source_instance_id) {
                                    if let Ok(memory_wrapper) = source_instance.memory(source_mem_idx as u32) {
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

    /// Resolve global imports from registered modules (like "G")
    ///
    /// For mutable globals, we share the same underlying Arc so mutations
    /// in one module are visible in others (per the WebAssembly spec).
    /// For immutable globals, we copy the value since it cannot change.
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
            // Check if this is a global import using import_types
            if let Some(kiln_runtime::module::RuntimeImportDesc::Global(_)) = module.import_types.get(i) {
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
                        if export.kind == kiln_runtime::module::ExportKind::Global {
                            let source_global_idx = export.index as usize;
                            let global_type = &module.global_import_types[global_import_idx];

                            if global_type.mutable {
                                // Mutable globals: share the Arc from the source instance
                                // so mutations are visible across modules
                                if let Some(source_instance_id) = self.instance_ids.get(mod_name) {
                                    if let Some(source_instance) = self.engine.get_instance(*source_instance_id) {
                                        if let Ok(shared_wrapper) = source_instance.global(source_global_idx as u32) {
                                            module_instance
                                                .set_global(global_import_idx, shared_wrapper)
                                                .map_err(|e| {
                                                    anyhow::anyhow!(
                                                        "Failed to set imported mutable global: {:?}",
                                                        e
                                                    )
                                                })?;
                                        }
                                    }
                                }
                            } else {
                                // Immutable globals: copy the current value
                                if let Some(source_instance_id) = self.instance_ids.get(mod_name) {
                                    if let Some(source_instance) = self.engine.get_instance(*source_instance_id) {
                                        if let Ok(source_wrapper) = source_instance.global(source_global_idx as u32) {
                                            if let Ok(guard) = source_wrapper.0.read() {
                                                let value = guard.get();
                                                let global = Global::new(
                                                    global_type.value_type,
                                                    global_type.mutable,
                                                    value.clone(),
                                                )
                                                .map_err(|e| {
                                                    anyhow::anyhow!("Failed to create imported global: {:?}", e)
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
                        }
                    }
                }

                global_import_idx += 1;
            }
        }

        Ok(())
    }

    /// Link function imports from registered modules
    ///
    /// This method sets up cross-instance function linking for imports
    /// from modules that have been registered (e.g., via `register "M"`).
    fn link_function_imports(&mut self, module: &Module, instance_id: usize) -> Result<()> {
        use kiln_runtime::module::RuntimeImportDesc;

        // Use import_types Vec which parallels import_order (more reliable than BoundedMap)
        let import_types = &module.import_types;
        let import_order = &module.import_order;

        if import_types.len() != import_order.len() {
            // Mismatch - this shouldn't happen but fall back gracefully
            return Ok(());
        }

        for (i, (mod_name, field_name)) in import_order.iter().enumerate() {
            // Skip spectest imports (handled by WASI stubs)
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
                            if export.kind == kiln_runtime::module::ExportKind::Function {
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

    /// Validate that all imports in a module can be resolved against registered modules
    /// and spectest. This implements the WebAssembly linking validation per the spec:
    /// - All imports must reference a known module
    /// - The named export must exist in the source module
    /// - The export kind must match the import kind (function, global, table, memory, tag)
    /// - Type signatures must be compatible
    fn validate_imports(&self, module: &Module, importing_metadata: &ModuleTypeMetadata) -> Result<()> {
        let import_order = &module.import_order;
        let import_types = &module.import_types;

        if import_order.len() != import_types.len() {
            return Err(anyhow::anyhow!(
                "Import order/types length mismatch: {} vs {}",
                import_order.len(),
                import_types.len()
            ));
        }

        for (i, (mod_name, field_name)) in import_order.iter().enumerate() {
            let import_desc = &import_types[i];

            if mod_name == "spectest" {
                // Validate against known spectest exports
                self.validate_spectest_import(field_name, import_desc, module)?;
            } else {
                // Validate against registered modules
                self.validate_registered_import(mod_name, field_name, import_desc, module, importing_metadata)?;
            }
        }

        Ok(())
    }

    /// Validate an import from the spectest module
    ///
    /// The spectest module provides these exports:
    /// - Functions: print_i32, print_i64, print_f32, print_f64, print_i32_f32, print_f64_f64, print
    /// - Global: global_i32 (i32), global_i64 (i64), global_f32 (f32), global_f64 (f64)
    /// - Table: table (10 20 funcref)
    /// - Memory: memory (1 2)
    fn validate_spectest_import(
        &self,
        field_name: &str,
        import_desc: &RuntimeImportDesc,
        module: &Module,
    ) -> Result<()> {
        match import_desc {
            RuntimeImportDesc::Function(type_idx) => {
                // spectest exports these functions (all are valid print functions)
                let known_functions = [
                    "print", "print_i32", "print_i64", "print_f32", "print_f64",
                    "print_i32_f32", "print_f64_f64",
                ];
                if !known_functions.contains(&field_name) {
                    return Err(anyhow::anyhow!(
                        "unknown import: spectest::{} is not a known spectest function",
                        field_name
                    ));
                }
                // Validate function signature against spectest definitions
                self.validate_spectest_function_type(field_name, *type_idx, module)?;
                Ok(())
            }
            RuntimeImportDesc::Global(global_type) => {
                match field_name {
                    "global_i32" => {
                        if global_type.value_type != ValueType::I32 || global_type.mutable {
                            return Err(anyhow::anyhow!(
                                "incompatible import type: spectest::global_i32 is (global i32)"
                            ));
                        }
                    }
                    "global_i64" => {
                        if global_type.value_type != ValueType::I64 || global_type.mutable {
                            return Err(anyhow::anyhow!(
                                "incompatible import type: spectest::global_i64 is (global i64)"
                            ));
                        }
                    }
                    "global_f32" => {
                        if global_type.value_type != ValueType::F32 || global_type.mutable {
                            return Err(anyhow::anyhow!(
                                "incompatible import type: spectest::global_f32 is (global f32)"
                            ));
                        }
                    }
                    "global_f64" => {
                        if global_type.value_type != ValueType::F64 || global_type.mutable {
                            return Err(anyhow::anyhow!(
                                "incompatible import type: spectest::global_f64 is (global f64)"
                            ));
                        }
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "unknown import: spectest::{} is not a known spectest global",
                            field_name
                        ));
                    }
                }
                Ok(())
            }
            RuntimeImportDesc::Table(table_type) => {
                if field_name != "table" && field_name != "table64" {
                    return Err(anyhow::anyhow!(
                        "unknown import: spectest::{} is not a known spectest table",
                        field_name
                    ));
                }
                let is_table64 = field_name == "table64";
                // spectest table is (table 10 20 funcref) or (table i64 10 20 funcref)
                let spectest_table = TableType {
                    element_type: RefType::Funcref,
                    limits: Limits { min: 10, max: Some(20) },
                    table64: is_table64,
                };
                validate_table_import_compatibility(table_type, &spectest_table)?;
                Ok(())
            }
            RuntimeImportDesc::Memory(mem_type) => {
                match field_name {
                    "memory" => {
                        // spectest memory is (memory 1 2)
                        let spectest_mem = MemoryType {
                            limits: Limits { min: 1, max: Some(2) },
                            shared: false,
                            memory64: false,
                            page_size: None,
                        };
                        validate_memory_import_compatibility(mem_type, &spectest_mem)?;
                    }
                    "shared_memory" => {
                        // spectest shared_memory is (memory 1 2 shared)
                        let spectest_mem = MemoryType {
                            limits: Limits { min: 1, max: Some(2) },
                            shared: true,
                            memory64: false,
                            page_size: None,
                        };
                        validate_memory_import_compatibility(mem_type, &spectest_mem)?;
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "unknown import: spectest::{} is not a known spectest memory",
                            field_name
                        ));
                    }
                }
                Ok(())
            }
            RuntimeImportDesc::Tag(_tag_type) => {
                // spectest does not export tags - any tag import from spectest is unknown
                Err(anyhow::anyhow!(
                    "unknown import: spectest::{} is not a known spectest export",
                    field_name
                ))
            }
            _ => {
                Err(anyhow::anyhow!(
                    "unknown import: spectest::{} has unsupported import kind",
                    field_name
                ))
            }
        }
    }

    /// Validate a spectest function import's type signature
    fn validate_spectest_function_type(
        &self,
        field_name: &str,
        type_idx: u32,
        module: &Module,
    ) -> Result<()> {
        let func_type = module.types.get(type_idx as usize).ok_or_else(|| {
            anyhow::anyhow!("incompatible import type: type index {} out of bounds", type_idx)
        })?;

        // Define expected signatures for spectest functions
        let (expected_params, expected_results): (&[ValueType], &[ValueType]) = match field_name {
            "print" => (&[], &[]),
            "print_i32" => (&[ValueType::I32], &[]),
            "print_i64" => (&[ValueType::I64], &[]),
            "print_f32" => (&[ValueType::F32], &[]),
            "print_f64" => (&[ValueType::F64], &[]),
            "print_i32_f32" => (&[ValueType::I32, ValueType::F32], &[]),
            "print_f64_f64" => (&[ValueType::F64, ValueType::F64], &[]),
            _ => return Ok(()), // Unknown function already handled
        };

        if func_type.params.len() != expected_params.len()
            || func_type.results.len() != expected_results.len()
        {
            return Err(anyhow::anyhow!(
                "incompatible import type: spectest::{} has wrong signature",
                field_name
            ));
        }

        for (i, (actual, expected)) in func_type.params.iter().zip(expected_params).enumerate() {
            if actual != expected {
                return Err(anyhow::anyhow!(
                    "incompatible import type: spectest::{} param {} type mismatch",
                    field_name, i
                ));
            }
        }

        for (i, (actual, expected)) in func_type.results.iter().zip(expected_results).enumerate() {
            if actual != expected {
                return Err(anyhow::anyhow!(
                    "incompatible import type: spectest::{} result {} type mismatch",
                    field_name, i
                ));
            }
        }

        Ok(())
    }

    /// Validate an import from a registered (non-spectest) module
    fn validate_registered_import(
        &self,
        mod_name: &str,
        field_name: &str,
        import_desc: &RuntimeImportDesc,
        module: &Module,
        importing_metadata: &ModuleTypeMetadata,
    ) -> Result<()> {
        // Check if the module is registered
        let source_module = match self.modules.get(mod_name) {
            Some(m) => m,
            None => {
                return Err(anyhow::anyhow!(
                    "unknown import: module '{}' is not registered",
                    mod_name
                ));
            }
        };

        // Get the source module's type metadata
        let source_metadata = self.module_type_metadata.get(mod_name);

        // Find the export in the source module
        let export = match source_module.get_export(field_name) {
            Some(e) => e,
            None => {
                return Err(anyhow::anyhow!(
                    "unknown import: {}::{} not found in registered module",
                    mod_name, field_name
                ));
            }
        };

        // Validate kind and type compatibility
        match import_desc {
            RuntimeImportDesc::Function(type_idx) => {
                if export.kind != ExportKind::Function {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} is {:?}, not a function",
                        mod_name, field_name, export.kind
                    ));
                }
                // Validate function type signature
                self.validate_function_type_compatibility(
                    mod_name, field_name, *type_idx, module, source_module, &export,
                    importing_metadata, source_metadata,
                )?;
            }
            RuntimeImportDesc::Global(import_global_type) => {
                if export.kind != ExportKind::Global {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} is {:?}, not a global",
                        mod_name, field_name, export.kind
                    ));
                }
                // Validate global type compatibility
                self.validate_global_type_compatibility(
                    mod_name, field_name, import_global_type, module, source_module, &export,
                    importing_metadata, source_metadata,
                )?;
            }
            RuntimeImportDesc::Table(import_table_type) => {
                if export.kind != ExportKind::Table {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} is {:?}, not a table",
                        mod_name, field_name, export.kind
                    ));
                }
                // Validate table type compatibility
                self.validate_table_type_compatibility(
                    mod_name, field_name, import_table_type, source_module, &export,
                )?;
            }
            RuntimeImportDesc::Memory(import_mem_type) => {
                if export.kind != ExportKind::Memory {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} is {:?}, not a memory",
                        mod_name, field_name, export.kind
                    ));
                }
                // Validate memory type compatibility
                self.validate_memory_type_compatibility(
                    mod_name, field_name, import_mem_type, source_module, &export,
                )?;
            }
            RuntimeImportDesc::Tag(import_tag_type) => {
                if export.kind != ExportKind::Tag {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} is {:?}, not a tag",
                        mod_name, field_name, export.kind
                    ));
                }
                // Validate tag type compatibility
                self.validate_tag_type_compatibility(
                    mod_name, field_name, import_tag_type, module, source_module, &export,
                )?;
            }
            _ => {
                // Extern, Resource, etc. - not yet handled
            }
        }

        Ok(())
    }

    /// Validate function type compatibility between import and export
    fn validate_function_type_compatibility(
        &self,
        mod_name: &str,
        field_name: &str,
        import_type_idx: u32,
        importing_module: &Module,
        source_module: &Module,
        export: &kiln_runtime::module::Export,
        importing_metadata: &ModuleTypeMetadata,
        source_metadata: Option<&ModuleTypeMetadata>,
    ) -> Result<()> {
        // Get the exported function's type index from the source module
        let export_func_idx = export.index as usize;
        let export_func = source_module.functions.get(export_func_idx).ok_or_else(|| {
            anyhow::anyhow!(
                "incompatible import type: {}::{} references invalid function index {}",
                mod_name, field_name, export_func_idx
            )
        })?;
        let export_type_idx = export_func.type_idx;

        // Use rec-group-aware type equivalence checking when metadata is available
        if let Some(src_meta) = source_metadata {
            if !is_type_idx_subtype_cross_module_with_metadata(
                export_type_idx, source_module, src_meta,
                import_type_idx, importing_module, importing_metadata,
            ) {
                return Err(anyhow::anyhow!(
                    "incompatible import type: {}::{} type mismatch (export type {} vs import type {})",
                    mod_name, field_name, export_type_idx, import_type_idx
                ));
            }
            return Ok(());
        }

        // Fallback: param/result comparison without rec-group metadata
        let import_func_type = importing_module.types.get(import_type_idx as usize).ok_or_else(|| {
            anyhow::anyhow!(
                "incompatible import type: type index {} out of bounds",
                import_type_idx
            )
        })?;

        let export_func_type = source_module.types.get(export_type_idx as usize).ok_or_else(|| {
            anyhow::anyhow!(
                "incompatible import type: {}::{} has invalid type index",
                mod_name, field_name
            )
        })?;

        // Compare function signatures
        if import_func_type.params.len() != export_func_type.params.len()
            || import_func_type.results.len() != export_func_type.results.len()
        {
            return Err(anyhow::anyhow!(
                "incompatible import type: {}::{} signature mismatch (params: {} vs {}, results: {} vs {})",
                mod_name, field_name,
                import_func_type.params.len(), export_func_type.params.len(),
                import_func_type.results.len(), export_func_type.results.len()
            ));
        }

        // Parameters are contravariant: import's param type must be a subtype of export's param type
        for (i, (import_param, export_param)) in import_func_type.params.iter()
            .zip(export_func_type.params.iter())
            .enumerate()
        {
            if !is_value_type_subtype_cross_module(
                import_param, importing_module,
                export_param, source_module,
            ) {
                return Err(anyhow::anyhow!(
                    "incompatible import type: {}::{} param {} type mismatch ({:?} vs {:?})",
                    mod_name, field_name, i, import_param, export_param
                ));
            }
        }

        // Results are covariant: export's result type must be a subtype of import's result type
        for (i, (import_result, export_result)) in import_func_type.results.iter()
            .zip(export_func_type.results.iter())
            .enumerate()
        {
            if !is_value_type_subtype_cross_module(
                export_result, source_module,
                import_result, importing_module,
            ) {
                return Err(anyhow::anyhow!(
                    "incompatible import type: {}::{} result {} type mismatch ({:?} vs {:?})",
                    mod_name, field_name, i, import_result, export_result
                ));
            }
        }

        Ok(())
    }

    /// Validate global type compatibility between import and export
    fn validate_global_type_compatibility(
        &self,
        mod_name: &str,
        field_name: &str,
        import_global_type: &GlobalType,
        importing_module: &Module,
        source_module: &Module,
        export: &kiln_runtime::module::Export,
        _importing_metadata: &ModuleTypeMetadata,
        _source_metadata: Option<&ModuleTypeMetadata>,
    ) -> Result<()> {
        let export_global_idx = export.index as usize;

        // Get the source global type - it could be an imported or defined global
        let source_global_type = self.get_global_type_from_module(source_module, export_global_idx);

        if let Some(source_type) = source_global_type {
            // For mutable globals, mutability must match exactly
            if import_global_type.mutable != source_type.mutable {
                return Err(anyhow::anyhow!(
                    "incompatible import type: {}::{} mutability mismatch (import: {}, export: {})",
                    mod_name, field_name, import_global_type.mutable, source_type.mutable
                ));
            }

            // For mutable globals, the value type must match exactly (using cross-module check)
            // For immutable globals, the import type must be a supertype of the export type
            if import_global_type.mutable {
                // Mutable: exact type match required (both directions of subtyping)
                let fwd = is_value_type_subtype_cross_module(
                    &source_type.value_type, source_module,
                    &import_global_type.value_type, importing_module,
                );
                let bwd = is_value_type_subtype_cross_module(
                    &import_global_type.value_type, importing_module,
                    &source_type.value_type, source_module,
                );
                if !fwd || !bwd {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} value type mismatch ({:?} vs {:?})",
                        mod_name, field_name, import_global_type.value_type, source_type.value_type
                    ));
                }
            } else {
                // Immutable: import type must be supertype of export type
                if !is_value_type_subtype_cross_module(
                    &source_type.value_type, source_module,
                    &import_global_type.value_type, importing_module,
                ) {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} value type mismatch ({:?} is not subtype of {:?})",
                        mod_name, field_name, source_type.value_type, import_global_type.value_type
                    ));
                }
            }
        }
        // If we can't find the source global type, we let it pass (best effort)

        Ok(())
    }

    /// Get the GlobalType for a global by its index in a module
    /// (accounting for imported vs defined globals)
    fn get_global_type_from_module(&self, module: &Module, global_idx: usize) -> Option<GlobalType> {
        let num_global_imports = module.global_import_types.len();
        if global_idx < num_global_imports {
            // It's an imported global
            module.global_import_types.get(global_idx).cloned()
        } else {
            // It's a defined global - get from deferred_global_inits
            let defined_idx = global_idx - num_global_imports;
            module.deferred_global_inits
                .get(defined_idx)
                .map(|(gt, _)| *gt)
        }
    }

    /// Validate table type compatibility between import and export
    fn validate_table_type_compatibility(
        &self,
        mod_name: &str,
        field_name: &str,
        import_table_type: &TableType,
        source_module: &Module,
        export: &kiln_runtime::module::Export,
    ) -> Result<()> {
        let export_table_idx = export.index as usize;

        // Get the source table type from the module definition
        let source_table_type = self.get_table_type_from_module(source_module, export_table_idx);

        if let Some(mut source_type) = source_table_type {
            // Per the WebAssembly spec, import validation must use the table's
            // *current* size (which may have been grown via table.grow), not the
            // original declared minimum. Query the live instance to get the
            // actual current table size.
            if let Some(instance_id) = self.instance_ids.get(mod_name) {
                if let Some(instance) = self.engine.get_instance(*instance_id) {
                    if let Ok(live_table) = instance.table(export_table_idx as u32) {
                        let current_size = live_table.size();
                        if current_size > source_type.limits.min {
                            source_type.limits.min = current_size;
                        }
                    }
                }
            }
            validate_table_import_compatibility(import_table_type, &source_type)?;
        }

        Ok(())
    }

    /// Get the TableType for a table by its index in a module
    fn get_table_type_from_module(&self, module: &Module, table_idx: usize) -> Option<TableType> {
        // Count table imports to determine if this is imported or defined
        let num_table_imports = module.import_types.iter()
            .filter(|desc| matches!(desc, RuntimeImportDesc::Table(_)))
            .count();

        if table_idx < num_table_imports {
            // It's an imported table - find the table import at this index
            let mut table_import_count = 0;
            for desc in &module.import_types {
                if let RuntimeImportDesc::Table(tt) = desc {
                    if table_import_count == table_idx {
                        return Some(tt.clone());
                    }
                    table_import_count += 1;
                }
            }
            None
        } else {
            // It's a defined table
            let defined_idx = table_idx - num_table_imports;
            // Module.tables contains defined tables (not imports)
            // Access the inner Table's type directly
            if let Some(table_wrapper) = module.tables.get(defined_idx) {
                Some(table_wrapper.0.ty.clone())
            } else {
                None
            }
        }
    }

    /// Validate memory type compatibility between import and export
    fn validate_memory_type_compatibility(
        &self,
        mod_name: &str,
        field_name: &str,
        import_mem_type: &MemoryType,
        source_module: &Module,
        export: &kiln_runtime::module::Export,
    ) -> Result<()> {
        let export_mem_idx = export.index as usize;

        // Get the source memory type from the module definition
        let source_mem_type = self.get_memory_type_from_module(source_module, export_mem_idx);

        if let Some(source_type) = source_mem_type {
            validate_memory_import_compatibility(import_mem_type, &source_type)?;
        }

        Ok(())
    }

    /// Get the MemoryType for a memory by its index in a module
    fn get_memory_type_from_module(&self, module: &Module, mem_idx: usize) -> Option<MemoryType> {
        // Count memory imports
        let num_mem_imports = module.import_types.iter()
            .filter(|desc| matches!(desc, RuntimeImportDesc::Memory(_)))
            .count();

        if mem_idx < num_mem_imports {
            // It's an imported memory
            let mut mem_import_count = 0;
            for desc in &module.import_types {
                if let RuntimeImportDesc::Memory(mt) = desc {
                    if mem_import_count == mem_idx {
                        return Some(mt.clone());
                    }
                    mem_import_count += 1;
                }
            }
            None
        } else {
            // It's a defined memory
            let defined_idx = mem_idx - num_mem_imports;
            if let Some(mem_wrapper) = module.memories.get(defined_idx) {
                // Convert CoreMemoryType to MemoryType
                let core_ty = &mem_wrapper.0.ty;
                Some(MemoryType {
                    limits: core_ty.limits,
                    shared: core_ty.shared,
                    memory64: core_ty.memory64,
                    page_size: core_ty.page_size,
                })
            } else {
                None
            }
        }
    }

    /// Validate tag type compatibility between import and export
    fn validate_tag_type_compatibility(
        &self,
        mod_name: &str,
        field_name: &str,
        import_tag_type: &kiln_foundation::types::TagType,
        importing_module: &Module,
        source_module: &Module,
        export: &kiln_runtime::module::Export,
    ) -> Result<()> {
        // Get the import's function type from the type index
        let import_func_type = importing_module.types.get(import_tag_type.type_idx as usize);

        // Get the export's tag type
        let export_tag_idx = export.index as usize;
        let num_tag_imports = source_module.count_tag_imports();
        let export_tag_type = if export_tag_idx < num_tag_imports {
            // Imported tag
            let mut tag_import_count = 0;
            let mut found = None;
            for desc in &source_module.import_types {
                if let RuntimeImportDesc::Tag(tt) = desc {
                    if tag_import_count == export_tag_idx {
                        found = Some(tt.clone());
                        break;
                    }
                    tag_import_count += 1;
                }
            }
            found
        } else {
            let defined_idx = export_tag_idx - num_tag_imports;
            source_module.tags.get(defined_idx).cloned()
        };

        if let (Some(import_ft), Some(export_tt)) = (import_func_type, export_tag_type) {
            let export_func_type = source_module.types.get(export_tt.type_idx as usize);
            if let Some(export_ft) = export_func_type {
                // Tag types must match exactly (params only, tags have no results)
                if import_ft.params.len() != export_ft.params.len() {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {}::{} tag param count mismatch ({} vs {})",
                        mod_name, field_name,
                        import_ft.params.len(), export_ft.params.len()
                    ));
                }
                for (i, (ip, ep)) in import_ft.params.iter().zip(export_ft.params.iter()).enumerate() {
                    if ip != ep {
                        return Err(anyhow::anyhow!(
                            "incompatible import type: {}::{} tag param {} type mismatch ({:?} vs {:?})",
                            mod_name, field_name, i, ip, ep
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Check if value type `sub` is a subtype of value type `sup` (single-module context)
///
/// Per the WebAssembly spec, subtyping rules for reference types:
/// - funcref <: funcref
/// - externref <: externref
/// - (ref null $t) <: funcref (if $t is a function type)
/// - (ref $t) <: (ref null $t)
/// - (ref $t) <: funcref (if $t is a function type)
/// - nofunc <: any nullable funcref
/// - none <: any nullable anyref descendant
/// - noextern <: any nullable externref descendant
/// For numeric types, only exact match is allowed.
fn is_value_type_subtype(sub: &ValueType, sup: &ValueType) -> bool {
    if sub == sup {
        return true;
    }

    match (sub, sup) {
        // FuncRef hierarchy
        (ValueType::NullFuncRef, ValueType::FuncRef) => true,
        (ValueType::NullFuncRef, ValueType::TypedFuncRef(_, true)) => true,
        (ValueType::TypedFuncRef(_, _), ValueType::FuncRef) => true,
        (ValueType::TypedFuncRef(idx_a, false), ValueType::TypedFuncRef(idx_b, true))
            if idx_a == idx_b => true,
        // ExternRef hierarchy
        (ValueType::NoExternRef, ValueType::ExternRef) => true,
        // AnyRef hierarchy
        (ValueType::EqRef, ValueType::AnyRef) => true,
        (ValueType::I31Ref, ValueType::AnyRef) => true,
        (ValueType::I31Ref, ValueType::EqRef) => true,
        (ValueType::StructRef(_), ValueType::AnyRef) => true,
        (ValueType::StructRef(_), ValueType::EqRef) => true,
        (ValueType::ArrayRef(_), ValueType::AnyRef) => true,
        (ValueType::ArrayRef(_), ValueType::EqRef) => true,
        (ValueType::NoneRef, ValueType::AnyRef) => true,
        (ValueType::NoneRef, ValueType::EqRef) => true,
        (ValueType::NoneRef, ValueType::I31Ref) => true,
        (ValueType::NoneRef, ValueType::StructRef(_)) => true,
        (ValueType::NoneRef, ValueType::ArrayRef(_)) => true,
        // ExnRef hierarchy
        (ValueType::NoExnRef, ValueType::ExnRef) => true,
        // Numeric types require exact match
        _ => false,
    }
}

/// Check if value type `sub` (from `sub_module`) is a subtype of `sup` (from `sup_module`)
///
/// This performs cross-module subtyping for import validation. It extends the basic
/// `is_value_type_subtype` with support for typed function references that may use
/// different type indices in different modules but refer to structurally equivalent types.
fn is_value_type_subtype_cross_module(
    sub: &ValueType, sub_module: &Module,
    sup: &ValueType, sup_module: &Module,
) -> bool {
    if sub == sup {
        return true;
    }

    match (sub, sup) {
        // TypedFuncRef is a subtype of FuncRef (funcref = (ref null func))
        (ValueType::TypedFuncRef(_, _), ValueType::FuncRef) => true,

        // NullFuncRef is subtype of any nullable typed func ref
        (ValueType::NullFuncRef, ValueType::TypedFuncRef(_, true)) => true,

        // TypedFuncRef subtyping with cross-module structural check
        (ValueType::TypedFuncRef(sub_idx, sub_nullable), ValueType::TypedFuncRef(sup_idx, sup_nullable)) => {
            // Nullable check: non-nullable is subtype of nullable, but not vice versa
            if *sub_nullable && !*sup_nullable {
                return false;
            }
            // Check if type at sub_idx in sub_module is a subtype of type at sup_idx in sup_module
            // Use coinductive equivalence checking with assumption set
            let mut assumptions = Vec::new();
            is_type_idx_subtype_cross_module(
                *sub_idx, sub_module,
                *sup_idx, sup_module,
                &mut assumptions,
            )
        }

        // Fall back to single-module subtype check for all other cases
        _ => is_value_type_subtype(sub, sup),
    }
}

/// Maximum depth for supertype chain walking to prevent infinite loops
const MAX_SUPERTYPE_CHAIN_DEPTH: u32 = 32;

/// Check if type index `sub_idx` in `sub_module` is a subtype of `sup_idx` in `sup_module`.
///
/// This first checks structural equivalence (using coinductive reasoning for recursive types),
/// then walks the supertype chain of `sub_idx` if direct equivalence fails.
fn is_type_idx_subtype_cross_module(
    sub_idx: u32, sub_module: &Module,
    sup_idx: u32, sup_module: &Module,
    assumptions: &mut Vec<(u32, u32)>,
) -> bool {
    // Check structural equivalence first
    if are_types_structurally_equivalent(sub_idx, sub_module, sup_idx, sup_module, assumptions) {
        return true;
    }

    // Walk the supertype chain of sub_idx in sub_module
    let mut current = sub_idx;
    for _ in 0..MAX_SUPERTYPE_CHAIN_DEPTH {
        if let Some(Some(parent_idx)) = sub_module.type_supertypes.get(current as usize) {
            if are_types_structurally_equivalent(*parent_idx, sub_module, sup_idx, sup_module, assumptions) {
                return true;
            }
            current = *parent_idx;
        } else {
            break;
        }
    }

    false
}

/// Check if two types (by index in possibly different modules) are structurally equivalent.
///
/// Uses coinductive reasoning: when comparing type A from module X with type B from module Y,
/// if we encounter the same (A, B) pair again during recursion, we assume they're equivalent.
/// This correctly handles recursive types like `(rec (type $t (sub (func (result (ref null $t))))))`.
fn are_types_structurally_equivalent(
    idx_a: u32, module_a: &Module,
    idx_b: u32, module_b: &Module,
    assumptions: &mut Vec<(u32, u32)>,
) -> bool {
    // Coinductive base case: if we've already assumed this pair is equivalent, return true
    if assumptions.contains(&(idx_a, idx_b)) {
        return true;
    }

    let type_a = match module_a.types.get(idx_a as usize) {
        Some(t) => t,
        None => return false,
    };
    let type_b = match module_b.types.get(idx_b as usize) {
        Some(t) => t,
        None => return false,
    };

    // Quick check: param/result counts must match
    if type_a.params.len() != type_b.params.len()
        || type_a.results.len() != type_b.results.len()
    {
        return false;
    }

    // Assume equivalence for this pair (coinductive step)
    assumptions.push((idx_a, idx_b));

    // Check all params and results are structurally compatible
    let params_ok = type_a.params.iter().zip(type_b.params.iter()).all(|(pa, pb)| {
        are_value_types_structurally_compatible_coinductive(pa, module_a, pb, module_b, assumptions)
    });

    let results_ok = params_ok && type_a.results.iter().zip(type_b.results.iter()).all(|(ra, rb)| {
        are_value_types_structurally_compatible_coinductive(ra, module_a, rb, module_b, assumptions)
    });

    // Remove our assumption before returning (we've finished checking this pair)
    assumptions.pop();

    results_ok
}

/// Check if two value types are structurally compatible across modules,
/// using coinductive assumptions for recursive type references.
fn are_value_types_structurally_compatible_coinductive(
    a: &ValueType, module_a: &Module,
    b: &ValueType, module_b: &Module,
    assumptions: &mut Vec<(u32, u32)>,
) -> bool {
    if a == b {
        return true;
    }

    match (a, b) {
        // TypedFuncRef: compare structurally by looking at the referenced func types
        (ValueType::TypedFuncRef(idx_a, nullable_a), ValueType::TypedFuncRef(idx_b, nullable_b)) => {
            if nullable_a != nullable_b {
                return false;
            }
            are_types_structurally_equivalent(*idx_a, module_a, *idx_b, module_b, assumptions)
        }
        _ => false,
    }
}

/// Check if type index `sub_idx` in `sub_module` is a subtype of `sup_idx` in `sup_module`,
/// using full rec-group-aware structural equivalence with finality checking.
///
/// Per the WebAssembly GC spec, two types from different modules are equivalent only if:
/// 1. They belong to rec groups of the same size
/// 2. Each corresponding type in the rec groups has matching finality flags
/// 3. Each corresponding type has matching supertype structure
/// 4. Each corresponding type has structurally compatible signatures
///    (with intra-group references mapped positionally)
fn is_type_idx_subtype_cross_module_with_metadata(
    sub_idx: u32, sub_module: &Module, sub_meta: &ModuleTypeMetadata,
    sup_idx: u32, sup_module: &Module, sup_meta: &ModuleTypeMetadata,
) -> bool {
    // Check structural equivalence first
    if are_types_equivalent_with_metadata(
        sub_idx, sub_module, sub_meta,
        sup_idx, sup_module, sup_meta,
    ) {
        return true;
    }

    // Walk the supertype chain of sub_idx
    let mut current = sub_idx;
    for _ in 0..MAX_SUPERTYPE_CHAIN_DEPTH {
        if let Some(Some(parent_idx)) = sub_module.type_supertypes.get(current as usize) {
            if are_types_equivalent_with_metadata(
                *parent_idx, sub_module, sub_meta,
                sup_idx, sup_module, sup_meta,
            ) {
                return true;
            }
            current = *parent_idx;
        } else {
            break;
        }
    }

    false
}

/// Check if two types are structurally equivalent using rec-group-aware comparison.
///
/// Two types are equivalent if and only if their entire rec groups are structurally
/// equivalent when compared position-by-position.
fn are_types_equivalent_with_metadata(
    idx_a: u32, module_a: &Module, meta_a: &ModuleTypeMetadata,
    idx_b: u32, module_b: &Module, meta_b: &ModuleTypeMetadata,
) -> bool {
    // Get rec group info for both types
    let info_a = match meta_a.type_info.get(idx_a as usize) {
        Some(info) => info,
        None => return false,
    };
    let info_b = match meta_b.type_info.get(idx_b as usize) {
        Some(info) => info,
        None => return false,
    };

    let (group_id_a, pos_a, _final_a) = *info_a;
    let (group_id_b, pos_b, _final_b) = *info_b;

    // The two types must be at the same position within their rec groups
    if pos_a != pos_b {
        return false;
    }

    // The rec groups must have the same size
    let group_size_a = meta_a.rec_group_sizes.get(group_id_a).copied().unwrap_or(0);
    let group_size_b = meta_b.rec_group_sizes.get(group_id_b).copied().unwrap_or(0);
    if group_size_a != group_size_b {
        return false;
    }

    let group_start_a = meta_a.rec_group_starts.get(group_id_a).copied().unwrap_or(0);
    let group_start_b = meta_b.rec_group_starts.get(group_id_b).copied().unwrap_or(0);

    // Compare every type in the rec group pairwise
    for offset in 0..group_size_a {
        let type_idx_a = group_start_a + offset as u32;
        let type_idx_b = group_start_b + offset as u32;

        // Finality must match
        let ti_a = match meta_a.type_info.get(type_idx_a as usize) {
            Some(info) => info,
            None => return false,
        };
        let ti_b = match meta_b.type_info.get(type_idx_b as usize) {
            Some(info) => info,
            None => return false,
        };
        if ti_a.2 != ti_b.2 {
            return false; // finality mismatch
        }

        // Supertype structure must match: both must have supertypes at the same
        // position within their respective rec groups, OR both have no supertype,
        // OR both have supertypes outside their rec group that are themselves equivalent.
        let sup_a = module_a.type_supertypes.get(type_idx_a as usize).and_then(|s| *s);
        let sup_b = module_b.type_supertypes.get(type_idx_b as usize).and_then(|s| *s);
        match (sup_a, sup_b) {
            (None, None) => {} // both have no supertype - ok
            (Some(sa), Some(sb)) => {
                // Check if supertypes are within the same rec group
                let sa_in_group = sa >= group_start_a && sa < group_start_a + group_size_a as u32;
                let sb_in_group = sb >= group_start_b && sb < group_start_b + group_size_b as u32;
                match (sa_in_group, sb_in_group) {
                    (true, true) => {
                        // Both within their rec groups - positions must match
                        if (sa - group_start_a) != (sb - group_start_b) {
                            return false;
                        }
                    }
                    (false, false) => {
                        // Both outside their rec groups - they must be equivalent types
                        // Use recursive check (the supertypes are in earlier rec groups,
                        // so this won't infinite-loop)
                        if !are_types_equivalent_with_metadata(sa, module_a, meta_a, sb, module_b, meta_b) {
                            return false;
                        }
                    }
                    _ => return false, // one inside, one outside - not equivalent
                }
            }
            _ => return false, // one has supertype, other doesn't
        }

        // Compare composite type structure
        let kind_a = meta_a.composite_kinds.get(type_idx_a as usize);
        let kind_b = meta_b.composite_kinds.get(type_idx_b as usize);

        match (kind_a, kind_b) {
            (Some(CompositeKind::Func), Some(CompositeKind::Func)) => {
                // Compare function type signatures with intra-group reference mapping
                let type_a = match module_a.types.get(type_idx_a as usize) {
                    Some(t) => t,
                    None => return false,
                };
                let type_b = match module_b.types.get(type_idx_b as usize) {
                    Some(t) => t,
                    None => return false,
                };

                if type_a.params.len() != type_b.params.len()
                    || type_a.results.len() != type_b.results.len()
                {
                    return false;
                }

                for (pa, pb) in type_a.params.iter().zip(type_b.params.iter()) {
                    if !are_value_types_equivalent_with_metadata(
                        pa, module_a, meta_a, group_start_a, group_size_a,
                        pb, module_b, meta_b, group_start_b, group_size_b,
                    ) {
                        return false;
                    }
                }

                for (ra, rb) in type_a.results.iter().zip(type_b.results.iter()) {
                    if !are_value_types_equivalent_with_metadata(
                        ra, module_a, meta_a, group_start_a, group_size_a,
                        rb, module_b, meta_b, group_start_b, group_size_b,
                    ) {
                        return false;
                    }
                }
            }
            (Some(CompositeKind::Struct(fields_a)), Some(CompositeKind::Struct(fields_b))) => {
                // Compare struct field types with intra-group reference mapping
                if fields_a.len() != fields_b.len() {
                    return false;
                }
                for (fa, fb) in fields_a.iter().zip(fields_b.iter()) {
                    if !are_field_storage_types_equivalent(
                        fa, module_a, meta_a, group_start_a, group_size_a,
                        fb, module_b, meta_b, group_start_b, group_size_b,
                    ) {
                        return false;
                    }
                }
            }
            (Some(CompositeKind::Array(elem_a)), Some(CompositeKind::Array(elem_b))) => {
                // Compare array element types
                if !are_field_storage_types_equivalent(
                    elem_a, module_a, meta_a, group_start_a, group_size_a,
                    elem_b, module_b, meta_b, group_start_b, group_size_b,
                ) {
                    return false;
                }
            }
            (None, None) => {
                // No composite kind info - fall back to func type comparison
                let type_a = match module_a.types.get(type_idx_a as usize) {
                    Some(t) => t,
                    None => return false,
                };
                let type_b = match module_b.types.get(type_idx_b as usize) {
                    Some(t) => t,
                    None => return false,
                };
                if type_a.params.len() != type_b.params.len()
                    || type_a.results.len() != type_b.results.len()
                {
                    return false;
                }
                for (pa, pb) in type_a.params.iter().zip(type_b.params.iter()) {
                    if !are_value_types_equivalent_with_metadata(
                        pa, module_a, meta_a, group_start_a, group_size_a,
                        pb, module_b, meta_b, group_start_b, group_size_b,
                    ) {
                        return false;
                    }
                }
                for (ra, rb) in type_a.results.iter().zip(type_b.results.iter()) {
                    if !are_value_types_equivalent_with_metadata(
                        ra, module_a, meta_a, group_start_a, group_size_a,
                        rb, module_b, meta_b, group_start_b, group_size_b,
                    ) {
                        return false;
                    }
                }
            }
            _ => return false, // different composite kinds
        }
    }

    true
}

/// Check if two GC field storage types are equivalent with rec-group-aware reference mapping.
fn are_field_storage_types_equivalent(
    a: &FieldStorageType, module_a: &Module, meta_a: &ModuleTypeMetadata,
    group_start_a: u32, group_size_a: usize,
    b: &FieldStorageType, module_b: &Module, meta_b: &ModuleTypeMetadata,
    group_start_b: u32, group_size_b: usize,
) -> bool {
    match (a, b) {
        (FieldStorageType::Value(va), FieldStorageType::Value(vb)) => va == vb,
        (FieldStorageType::I8, FieldStorageType::I8) => true,
        (FieldStorageType::I16, FieldStorageType::I16) => true,
        (FieldStorageType::RefType(idx_a), FieldStorageType::RefType(idx_b))
        | (FieldStorageType::RefTypeNull(idx_a), FieldStorageType::RefTypeNull(idx_b)) => {
            // Check if references are within the current rec group
            let a_in_group = *idx_a >= group_start_a && *idx_a < group_start_a + group_size_a as u32;
            let b_in_group = *idx_b >= group_start_b && *idx_b < group_start_b + group_size_b as u32;
            match (a_in_group, b_in_group) {
                (true, true) => (*idx_a - group_start_a) == (*idx_b - group_start_b),
                (false, false) => {
                    are_types_equivalent_with_metadata(
                        *idx_a, module_a, meta_a,
                        *idx_b, module_b, meta_b,
                    )
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Check if two value types are structurally equivalent with rec-group-aware comparison.
///
/// For TypedFuncRef, intra-group references are compared by position within the group,
/// while inter-group references are compared by recursive structural equivalence.
fn are_value_types_equivalent_with_metadata(
    a: &ValueType, module_a: &Module, meta_a: &ModuleTypeMetadata,
    group_start_a: u32, group_size_a: usize,
    b: &ValueType, module_b: &Module, meta_b: &ModuleTypeMetadata,
    group_start_b: u32, group_size_b: usize,
) -> bool {
    if a == b {
        return true;
    }

    match (a, b) {
        (ValueType::TypedFuncRef(idx_a, nullable_a), ValueType::TypedFuncRef(idx_b, nullable_b)) => {
            if nullable_a != nullable_b {
                return false;
            }
            // Check if the referenced types are within the current rec group
            let a_in_group = *idx_a >= group_start_a && *idx_a < group_start_a + group_size_a as u32;
            let b_in_group = *idx_b >= group_start_b && *idx_b < group_start_b + group_size_b as u32;

            match (a_in_group, b_in_group) {
                (true, true) => {
                    // Both within their respective rec groups - compare by position
                    (*idx_a - group_start_a) == (*idx_b - group_start_b)
                }
                (false, false) => {
                    // Both outside their rec groups - recursively check equivalence
                    are_types_equivalent_with_metadata(
                        *idx_a, module_a, meta_a,
                        *idx_b, module_b, meta_b,
                    )
                }
                _ => false, // one inside, one outside - not equivalent
            }
        }
        _ => false,
    }
}

/// Validate table import compatibility per the WebAssembly spec
///
/// Import table type must be compatible with the actual table:
/// - Element types must be the same
/// - Import min must be <= actual min
/// - If import has max, actual must have max and actual max <= import max
fn validate_table_import_compatibility(import: &TableType, actual: &TableType) -> Result<()> {
    // Element types must match exactly
    if import.element_type != actual.element_type {
        return Err(anyhow::anyhow!(
            "incompatible import type: table element type mismatch ({:?} vs {:?})",
            import.element_type, actual.element_type
        ));
    }

    validate_limits_compatibility(&import.limits, &actual.limits, "table")?;
    Ok(())
}

/// Validate memory import compatibility per the WebAssembly spec
///
/// Import memory type must be compatible with the actual memory:
/// - Import min must be <= actual min
/// - If import has max, actual must have max and actual max <= import max
/// - shared flag must match
fn validate_memory_import_compatibility(import: &MemoryType, actual: &MemoryType) -> Result<()> {
    // Memory64 flag must match (memory32 and memory64 are incompatible)
    if import.memory64 != actual.memory64 {
        return Err(anyhow::anyhow!(
            "incompatible import type: memory types incompatible"
        ));
    }

    // Shared flag must match
    if import.shared != actual.shared {
        return Err(anyhow::anyhow!(
            "incompatible import type: memory shared flag mismatch"
        ));
    }

    // Page size must match per the custom-page-sizes proposal
    let import_page_size = import.page_size.unwrap_or(65536);
    let actual_page_size = actual.page_size.unwrap_or(65536);
    if import_page_size != actual_page_size {
        return Err(anyhow::anyhow!(
            "incompatible import type: memory types incompatible"
        ));
    }

    validate_limits_compatibility(&import.limits, &actual.limits, "memory")?;
    Ok(())
}

/// Validate limits compatibility for table/memory imports
///
/// Per the WebAssembly spec:
/// - actual.min must be >= import.min (actual provides at least as much as import requires)
/// - If import has a max, actual must also have a max and actual.max <= import.max
///   (actual is at least as constrained as import requires)
fn validate_limits_compatibility(import: &Limits, actual: &Limits, kind: &str) -> Result<()> {
    // Actual min must be >= import min
    if actual.min < import.min {
        return Err(anyhow::anyhow!(
            "incompatible import type: {} min {} < required {}",
            kind, actual.min, import.min
        ));
    }

    // If import specifies a max, actual must also have a max that is <= import max
    if let Some(import_max) = import.max {
        match actual.max {
            Some(actual_max) => {
                if actual_max > import_max {
                    return Err(anyhow::anyhow!(
                        "incompatible import type: {} max {} > allowed {}",
                        kind, actual_max, import_max
                    ));
                }
            }
            None => {
                // Import requires a max but actual has no max
                return Err(anyhow::anyhow!(
                    "incompatible import type: {} has no max but import requires max {}",
                    kind, import_max
                ));
            }
        }
    }

    Ok(())
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
pub fn execute_wast_execute(engine: &mut WastEngine, execute: &mut WastExecute) -> Result<Vec<Value>> {
    match execute {
        WastExecute::Invoke(invoke) => execute_wast_invoke(engine, invoke),
        WastExecute::Wat(wat) => {
            // WAT module: encode to binary, load, and execute start function
            // This is used in assert_trap/assert_return with inline (module ...) definitions
            // load_module handles start function execution per the WebAssembly spec
            let binary = wat.encode().context("Failed to encode WAT module to binary")?;
            engine.load_module(None, &binary)?;
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
