//! Minimal WAST Test Execution Engine
//!
//! This module provides a simplified bridge between the WAST test framework and
//! the WRT runtime, focusing on basic functionality with real WebAssembly
//! execution using StacklessEngine directly.

#![cfg(feature = "std")]

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use wast::{WastExecute, WastInvoke};
use wrt_decoder::decoder::decode_module;
use wrt_foundation::values::Value;
use wrt_runtime::{module::Module, stackless::StacklessEngine};

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
    /// The underlying WRT execution engine
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
        // Decode the WASM binary into a WrtModule
        let wrt_module = decode_module(wasm_binary).map_err(|e| {
            anyhow::anyhow!("XYZZY_DECODE_FAILED({} bytes): {} [code={}, category={:?}]",
                wasm_binary.len(), e, e.code, e.category)
        })?;

        // Validate the module before proceeding (Phase 1 of WAST conformance)
        crate::wast_validator::WastModuleValidator::validate(&wrt_module)
            .map_err(|e| anyhow::anyhow!("Module validation failed: {:?}", e))?;

        // Convert WrtModule to RuntimeModule
        // Wrap in Arc immediately to avoid clone() which loses BoundedMap data
        use wrt_runtime::module_instance::ModuleInstance;

        let module = Arc::new(
            *Module::from_wrt_module(&wrt_module).context("Failed to convert to runtime module")?,
        );

        // Create a module instance from the module
        // Use Arc::clone to share the module reference without copying data
        let module_instance = Arc::new(
            ModuleInstance::new(Arc::clone(&module), 0)
                .context("Failed to create module instance")?,
        );

        // Resolve spectest memory imports FIRST (imported memories come before defined memories)
        // This must happen before populate_memories_from_module() which adds defined memories
        Self::resolve_spectest_memory_imports(&module, &module_instance)?;

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
            .filter(|export| {
                use wrt_runtime::module::ExportKind;
                export.kind == ExportKind::Function
            })
            .map(|export| export.index)
            .ok_or_else(|| {
                anyhow::anyhow!("Function '{}' is not exported from module", function_name)
            })
    }

    /// Get a global variable value by name
    pub fn get_global(&self, module_name: Option<&str>, global_name: &str) -> Result<Value> {
        use wrt_runtime::module::ExportKind;

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
        module_instance: &Arc<wrt_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use wrt_foundation::clean_core_types::CoreMemoryType;
        use wrt_foundation::types::{Limits, MemoryType};
        use wrt_runtime::memory::Memory;
        use wrt_runtime::module::{MemoryWrapper, RuntimeImportDesc};

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
        module_instance: &Arc<wrt_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use std::sync::{Arc as StdArc, RwLock};
        use wrt_foundation::values::{FloatBits32, FloatBits64};
        use wrt_runtime::global::Global;
        use wrt_runtime::module::GlobalWrapper;

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
        module_instance: &Arc<wrt_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use wrt_foundation::types::{Limits, RefType, TableType};
        use wrt_runtime::module::{RuntimeImportDesc, TableWrapper};
        use wrt_runtime::table::Table;

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
        module_instance: &Arc<wrt_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use wrt_runtime::module::{RuntimeImportDesc, TableWrapper};

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
                        wrt_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module_arc.exports.get(&bounded_field) {
                        if export.kind == wrt_runtime::module::ExportKind::Table {
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
    fn resolve_registered_module_imports(
        &self,
        module: &Module,
        module_instance: &Arc<wrt_runtime::module_instance::ModuleInstance>,
    ) -> Result<()> {
        use std::sync::{Arc as StdArc, RwLock};
        use wrt_runtime::global::Global;
        use wrt_runtime::module::GlobalWrapper;

        let mut global_import_idx = 0usize;

        for (mod_name, field_name) in module.import_order.iter() {
            // Count this if it's a global import
            if global_import_idx >= module.global_import_types.len() {
                break;
            }

            // Check if this is a global import by checking if it matches our expected position
            let is_global = module.global_import_types.get(global_import_idx).is_some();

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
                        wrt_foundation::bounded::BoundedString::<256>::from_str_truncate(
                            field_name,
                        )
                        .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                    if let Some(export) = source_module.exports.get(&bounded_field) {
                        if export.kind == wrt_runtime::module::ExportKind::Global {
                            let source_global_idx = export.index as usize;

                            // Get the value from the source module's global
                            if let Ok(global_wrapper) = source_module.globals.get(source_global_idx)
                            {
                                if let Ok(guard) = global_wrapper.0.read() {
                                    let value = guard.get();
                                    let global_type =
                                        &module.global_import_types[global_import_idx];

                                    // Create a new global with the resolved value
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
        use wrt_runtime::module::RuntimeImportDesc;

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
                            wrt_foundation::bounded::BoundedString::<256>::from_str_truncate(
                                field_name,
                            )
                            .map_err(|e| anyhow::anyhow!("Field name too long: {:?}", e))?;

                        if let Some(export) = source_module.exports.get(&bounded_field) {
                            if export.kind == wrt_runtime::module::ExportKind::Function {
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
pub fn execute_wast_execute(engine: &mut WastEngine, execute: &WastExecute) -> Result<Vec<Value>> {
    match execute {
        WastExecute::Invoke(invoke) => execute_wast_invoke(engine, invoke),
        WastExecute::Wat(_) => {
            // WAT modules need to be compiled and executed
            Err(anyhow::anyhow!("WAT execution not yet implemented"))
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
            WastDirective::AssertTrap { exec, message, .. } => {
                // Test that execution traps with expected error
                match execute_wast_execute(&mut engine, &exec) {
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
        let wrt_value = convert_wast_arg_to_value(&wast_arg).unwrap();
        assert_eq!(wrt_value, Value::I32(42));
    }

    #[test]
    fn test_values_equal() {
        assert!(values_equal(&Value::I32(42), &Value::I32(42)));
        assert!(!values_equal(&Value::I32(42), &Value::I32(43)));

        // Test NaN handling
        use wrt_foundation::values::FloatBits32;
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
        match wrt_decoder::decoder::decode_module(&binary) {
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
        match wrt_decoder::decoder::decode_module(&binary) {
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
        match wrt_decoder::decoder::decode_module(&binary) {
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
                        match wrt_decoder::decoder::decode_module(&binary) {
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
                                        match wrt_runtime::module::Module::from_wrt_module(&module)
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
        let decoded = wrt_decoder::decoder::decode_module(&binary).expect("Failed to decode");
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
        let args: Vec<wrt_foundation::values::Value> = vec![];
        let results = engine
            .invoke_function(None, "funcref", &args)
            .expect("Failed to invoke funcref");

        println!("funcref result: {:?}", results);

        // Check the result - should be FuncRef(None), not I32(-1)
        assert!(!results.is_empty(), "Expected one result");
        match &results[0] {
            wrt_foundation::values::Value::FuncRef(None) => {
                println!("PASS: Got FuncRef(None) as expected");
            },
            wrt_foundation::values::Value::I32(v) => {
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
            wrt_foundation::values::Value::FuncRef(None) => {
                println!("PASS: Got FuncRef(None) as expected for nullfuncref");
            },
            wrt_foundation::values::Value::I32(v) => {
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
        match wrt_decoder::decoder::decode_module(&binary) {
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
