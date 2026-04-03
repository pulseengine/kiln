//! Module instance implementation for WebAssembly runtime
//!
//! This module provides the implementation of a WebAssembly module instance,
//! which represents a runtime instance of a WebAssembly module with its own
//! memory, tables, globals, and functions.

// alloc is imported in lib.rs with proper feature gates

#[cfg(feature = "debug-full")]
use kiln_debug::FunctionInfo;
#[cfg(feature = "debug")]
use kiln_debug::{
    DwarfDebugInfo,
    LineInfo,
};
use kiln_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    traits::{
        BoundedCapacity,
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
};
use kiln_instructions::reference_ops::ReferenceOperations;

// Type alias for FuncType to make signatures more readable - uses unified RuntimeProvider
use crate::bounded_runtime_infra::{
    create_runtime_provider,
    BoundedImportExportName,
    BoundedImportMap,
    RuntimeProvider,
};
use crate::{
    global::Global,
    memory::Memory,
    module::{
        GlobalWrapper,
        MemoryWrapper,
        Module,
        TableWrapper,
    },
    prelude::{
        Debug,
        Error,
        ErrorCategory,
        FuncType,
        Result,
    },
    table::Table,
};
type KilnFuncType = kiln_foundation::types::FuncType;

// Import format! macro for string formatting
use std::format;
use std::sync::{
    Arc,
    Mutex,
};

/// Represents a runtime instance of a WebAssembly module
#[cfg_attr(not(feature = "debug"), derive(Debug))]
pub struct ModuleInstance {
    /// The module this instance was instantiated from
    module:      Arc<Module>,
    /// The instance's memories
    memories:    Arc<Mutex<Vec<MemoryWrapper>>>,
    /// The instance's tables
    tables:      Arc<Mutex<Vec<TableWrapper>>>,
    /// The instance's globals
    globals:     Arc<Mutex<Vec<GlobalWrapper>>>,
    /// Instance ID for debugging
    instance_id: usize,
    /// Imported instance indices to resolve imports
    imports:     BoundedImportMap<BoundedImportMap<(usize, usize)>>,
    /// Tracks which element segments have been dropped via elem.drop
    /// After dropping, table.init will treat the segment as having 0 length
    dropped_elements: Arc<Mutex<Vec<bool>>>,
    /// Tracks which data segments have been dropped via data.drop
    /// After dropping, memory.init will treat the segment as having 0 length
    dropped_data: Arc<Mutex<Vec<bool>>>,
    /// Debug information (optional)
    #[cfg(feature = "debug")]
    debug_info:  Option<DwarfDebugInfo<'static>>,
}

// Manual Debug implementation when debug feature is enabled
#[cfg(feature = "debug")]
impl Debug for ModuleInstance {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ModuleInstance")
            .field("module", &self.module)
            .field("instance_id", &self.instance_id)
            .field("debug_info", &self.debug_info.is_some())
            .finish()
    }
}

impl ModuleInstance {
    /// Create a new module instance from a module (accepts Arc to avoid deep clones)
    pub fn new(module: Arc<Module>, instance_id: usize) -> Result<Self> {
        Ok(Self {
            module,
            memories: Arc::new(Mutex::new(Vec::new())),
            tables: Arc::new(Mutex::new(Vec::new())),
            globals: Arc::new(Mutex::new(Vec::new())),
            instance_id,
            imports: Default::default(),
            dropped_elements: Arc::new(Mutex::new(Vec::new())),
            dropped_data: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "debug")]
            debug_info: None,
        })
    }

    /// Get the module associated with this instance
    #[must_use]
    pub fn module(&self) -> &Arc<Module> {
        &self.module
    }

    /// Get the instance ID (ModuleInstance-level ID, not engine ID)
    #[must_use]
    pub fn instance_id(&self) -> usize {
        self.instance_id
    }

    /// Get a memory from this instance
    pub fn memory(&self, idx: u32) -> Result<MemoryWrapper> {
        let memories = self
            .memories
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock memories"))?;
        let memory = memories
            .get(idx as usize)
            .ok_or_else(|| Error::runtime_execution_error("Memory index out of bounds"))?;
        Ok(memory.clone())
    }

    /// Set a memory at a specific index (for imported memories)
    /// This is used during instantiation to replace placeholder memories with imported ones
    pub fn set_memory(&self, idx: usize, memory: MemoryWrapper) -> Result<()> {
        let mut memories = self
            .memories
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock memories"))?;
        if idx < memories.len() {
            memories[idx] = memory;
        } else {
            // Fill gaps with placeholder memories for import slots resolved out of order
            use kiln_foundation::clean_core_types::CoreMemoryType;
            use kiln_foundation::types::Limits;
            while memories.len() < idx {
                let placeholder = crate::memory::Memory::new(CoreMemoryType {
                    limits: Limits { min: 0, max: None },
                    shared: false,
                    memory64: false,
                    page_size: None,
                }).map_err(|e| Error::runtime_error("Failed to create placeholder memory"))?;
                memories.push(MemoryWrapper::new(placeholder));
            }
            memories.push(memory);
        }
        Ok(())
    }

    /// Get a memory by export name from this instance
    pub fn memory_by_name(&self, name: &str) -> Result<MemoryWrapper> {
        use crate::module::ExportKind;

        // Find the export in module.exports (DirectMap iteration)
        for (_key, export) in self.module.exports.iter() {
            let export_name = export.name.as_str().unwrap_or("");
            if export_name == name && export.kind == ExportKind::Memory {
                return self.memory(export.index);
            }
        }
        Err(Error::resource_not_found("Memory export not found"))
    }

    /// Get a table from this instance
    pub fn table(&self, idx: u32) -> Result<TableWrapper> {
        let tables =
            self.tables.lock().map_err(|_| Error::runtime_error("Failed to lock tables"))?;
        let table = tables
            .get(idx as usize)
            .ok_or_else(|| Error::resource_table_not_found("Runtime operation error"))?;
        Ok(table.clone())
    }

    /// Set a table at a specific index (for imported tables)
    /// This is used during instantiation to replace placeholder tables with imported ones
    pub fn set_table(&self, idx: usize, table: TableWrapper) -> Result<()> {
        let mut tables = self
            .tables
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock tables"))?;
        if idx < tables.len() {
            tables[idx] = table;
        } else {
            // Fill gaps with placeholder tables for import slots resolved out of order
            use kiln_foundation::types::{Limits, RefType, TableType};
            while tables.len() < idx {
                let placeholder = crate::table::Table::new(TableType {
                    element_type: RefType::Funcref,
                    limits: Limits { min: 0, max: None },
                    table64: false,
                }).map_err(|e| Error::runtime_error("Failed to create placeholder table"))?;
                tables.push(TableWrapper::new(placeholder));
            }
            tables.push(table);
        }
        Ok(())
    }

    /// Get a table by export name from this instance
    pub fn table_by_name(&self, name: &str) -> Result<TableWrapper> {
        use crate::module::ExportKind;

        // Find the export in module.exports (DirectMap iteration)
        for (_key, export) in self.module.exports.iter() {
            let export_name = export.name.as_str().unwrap_or("");
            if export_name == name && export.kind == ExportKind::Table {
                return self.table(export.index);
            }
        }
        Err(Error::resource_not_found("Table export not found"))
    }

    /// Get a global from this instance
    pub fn global(&self, idx: u32) -> Result<GlobalWrapper> {
        let globals = self
            .globals
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock globals"))?;
        let global = globals
            .get(idx as usize)
            .ok_or_else(|| Error::resource_global_not_found("Runtime operation error"))?;
        Ok(global.clone())
    }

    /// Set a global at a specific index (for imported globals)
    /// This is used during instantiation to replace placeholder globals with imported ones
    pub fn set_global(&self, idx: usize, global: GlobalWrapper) -> Result<()> {
        let mut globals = self
            .globals
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock globals"))?;
        if idx < globals.len() {
            globals[idx] = global;
            Ok(())
        } else if idx == globals.len() {
            globals.push(global);
            Ok(())
        } else {
            Err(Error::runtime_error("Global index out of bounds for set_global"))
        }
    }

    /// Re-evaluate globals that depend on imported globals after import values are set.
    /// This fixes the deferred initialization problem where globals using global.get
    /// of imported globals were evaluated before import values were known.
    pub fn reevaluate_deferred_globals(&self) -> Result<()> {
        use crate::module::GlobalWrapper;
        use crate::global::Global;
        use std::sync::{Arc as StdArc, RwLock};

        // Lock globals to get the current values (including resolved imports)
        let globals_guard = self
            .globals
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock globals for deferred evaluation"))?;

        // Convert to slice for the reevaluate function
        let globals_slice: &[GlobalWrapper] = &globals_guard;

        // Call the module's reevaluate function
        let updates = self.module.reevaluate_deferred_globals(globals_slice)?;

        // Drop the immutable borrow to allow mutable access
        drop(globals_guard);

        // Apply the updates using set_initial_value (bypasses mutability check)
        for (idx, new_value) in updates {
            let global_wrapper = {
                let globals = self.globals.lock()
                    .map_err(|_| Error::runtime_error("Failed to lock globals for update"))?;
                globals.get(idx).cloned()
            };

            if let Some(wrapper) = global_wrapper {
                if let Ok(mut guard) = wrapper.0.write() {
                    guard.set_initial_value(&new_value)?;
                }
            }
        }

        Ok(())
    }

    /// Get a global by export name from this instance
    pub fn global_by_name(&self, name: &str) -> Result<GlobalWrapper> {
        use crate::module::ExportKind;

        // Find the export in module.exports (DirectMap iteration)
        for (_key, export) in self.module.exports.iter() {
            let export_name = export.name.as_str().unwrap_or("");
            if export_name == name && export.kind == ExportKind::Global {
                return self.global(export.index);
            }
        }
        Err(Error::resource_not_found("Global export not found"))
    }

    /// Get the function type for a function
    pub fn function_type(&self, idx: u32) -> Result<crate::prelude::CoreFuncType> {
        let function = self
            .module
            .functions
            .get(idx as usize)
            .ok_or_else(|| Error::runtime_function_not_found("Function index not found"))?;

        let ty = self
            .module
            .types
            .get(function.type_idx as usize)
            .ok_or_else(|| Error::validation_type_mismatch("Type index not found"))?;

        // Convert from provider-aware FuncType to clean CoreFuncType
        // Create BoundedVecs manually since FromIterator isn't implemented
        let params_slice = ty.params.as_slice();
        let results_slice = ty.results.as_slice();

        let mut params = kiln_foundation::bounded::BoundedVec::<
            kiln_foundation::ValueType,
            128,
            RuntimeProvider,
        >::new(create_runtime_provider()?)
        .map_err(|_| Error::memory_error("Failed to create params vec"))?;

        let mut results = kiln_foundation::bounded::BoundedVec::<
            kiln_foundation::ValueType,
            128,
            RuntimeProvider,
        >::new(create_runtime_provider()?)
        .map_err(|_| Error::memory_error("Failed to create results vec"))?;

        for param in params_slice {
            params
                .push(*param)
                .map_err(|_| Error::capacity_limit_exceeded("Too many params"))?;
        }

        for result in results_slice {
            results
                .push(*result)
                .map_err(|_| Error::capacity_limit_exceeded("Too many results"))?;
        }

        // Use FuncType::new() instead of struct literal
        // Note: BoundedVec's iter() yields ValueType by value, not by reference
        let param_types: Vec<_> = params.iter().collect();
        let result_types: Vec<_> = results.iter().collect();
        crate::prelude::CoreFuncType::new(param_types, result_types)
    }

    /// Add a memory to this instance
    pub fn add_memory(&self, memory: Memory) -> Result<()> {
        let mut memories = self
            .memories
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock memories"))?;
        memories.push(MemoryWrapper::new(Box::new(memory)));
        Ok(())
    }

    /// Add a table to this instance
    pub fn add_table(&self, table: Table) -> Result<()> {
        let mut tables =
            self.tables.lock().map_err(|_| Error::runtime_error("Failed to lock tables"))?;
        tables.push(TableWrapper::new(table));
        Ok(())
    }

    /// Add a global to this instance
    pub fn add_global(&self, global: Global) -> Result<()> {
        let mut globals = self
            .globals
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock globals"))?;
        globals.push(GlobalWrapper::new(global));
        Ok(())
    }

    /// Populate globals from the module into this instance
    /// This copies all global variables from the module definition to the instance,
    /// accounting for imported globals in the index space.
    ///
    /// Global indices in WebAssembly are:
    /// - Indices 0..N-1 are imported globals
    /// - Indices N+ are defined globals
    pub fn populate_globals_from_module(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, info};

        #[cfg(feature = "tracing")]
        info!("Populating globals from module for instance {}", self.instance_id);

        // Use the pre-computed count of global imports from the module
        let num_global_imports = self.module.num_global_imports;

        {
            let mut globals = self
                .globals
                .lock()
                .map_err(|_| Error::runtime_error("Failed to lock globals"))?;

            // First, create placeholder globals for imports using the direct global_import_types vector
            // This bypasses the broken nested BoundedMap serialization issue
            for (idx, global_type) in self.module.global_import_types.iter().enumerate() {
                use kiln_foundation::values::{Value, FloatBits32, FloatBits64};

                // Create a placeholder global with default value
                let default_value = match global_type.value_type {
                    kiln_foundation::ValueType::I32 => Value::I32(0),
                    kiln_foundation::ValueType::I64 => Value::I64(0),
                    kiln_foundation::ValueType::F32 => Value::F32(FloatBits32(0)),
                    kiln_foundation::ValueType::F64 => Value::F64(FloatBits64(0)),
                    kiln_foundation::ValueType::FuncRef => Value::FuncRef(None),
                    kiln_foundation::ValueType::NullFuncRef => Value::FuncRef(None),
                    kiln_foundation::ValueType::TypedFuncRef(_, _) => Value::FuncRef(None),
                    kiln_foundation::ValueType::ExternRef => Value::ExternRef(None),
                    kiln_foundation::ValueType::V128 => Value::V128(kiln_foundation::values::V128 { bytes: [0u8; 16] }),
                    kiln_foundation::ValueType::ExnRef => Value::ExnRef(None),
                    kiln_foundation::ValueType::AnyRef => Value::ExternRef(None),
                    kiln_foundation::ValueType::EqRef => Value::I31Ref(None),
                    kiln_foundation::ValueType::I31Ref => Value::I31Ref(None),
                    kiln_foundation::ValueType::StructRef(_) => Value::StructRef(None),
                    kiln_foundation::ValueType::ArrayRef(_) => Value::ArrayRef(None),
                    _ => Value::I32(0),
                };
                let placeholder = Global::new(global_type.value_type, global_type.mutable, default_value)
                    .map_err(|_| Error::runtime_error("Failed to create placeholder global"))?;
                #[cfg(feature = "tracing")]
                debug!(
                    "Creating placeholder for imported global {} ({:?}) - is_mutable: {}",
                    idx,
                    global_type.value_type,
                    global_type.mutable
                );
                globals.push(GlobalWrapper::new(placeholder));
            }

            #[cfg(feature = "tracing")]
            debug!("Created {} placeholder globals for imports", num_global_imports);

            // Now copy defined globals
            for idx in 0..self.module.globals.len() {
                if let Some(global_wrapper) = self.module.globals.get(idx) {
                    #[cfg(feature = "tracing")]
                    debug!(
                        "Copying defined global {} (global index {}) to instance",
                        idx,
                        globals.len()
                    );
                    globals.push(global_wrapper.clone());
                }
            }
            #[cfg(feature = "tracing")]
            info!(
                "Populated {} globals for instance {} ({} imports + {} defined)",
                globals.len(),
                self.instance_id,
                num_global_imports,
                self.module.globals.len()
            );
        }

        Ok(())
    }

    /// Populate memories from the module into this instance
    /// This copies all memory instances from the module definition to the instance
    pub fn populate_memories_from_module(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, info};

        #[cfg(feature = "tracing")]
        info!("Populating memories from module for instance {}", self.instance_id);

        {
            let mut memories = self
                .memories
                .lock()
                .map_err(|_| Error::runtime_error("Failed to lock memories"))?;

            for (idx, memory_wrapper) in self.module.memories.iter().enumerate() {
                #[cfg(feature = "tracing")]
                debug!("Copying memory {} to instance", idx);
                memories.push(memory_wrapper.clone());
            }
            #[cfg(feature = "tracing")]
            info!("Populated {} memories for instance {}", self.module.memories.len(), self.instance_id);
        }

        Ok(())
    }

    /// Populate tables from the module into this instance
    /// This copies all table instances from the module definition to the instance
    pub fn populate_tables_from_module(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, info};

        #[cfg(feature = "tracing")]
        info!("Populating tables from module for instance {}", self.instance_id);

        {
            let mut tables = self
                .tables
                .lock()
                .map_err(|_| Error::runtime_error("Failed to lock tables"))?;

            for (idx, table_wrapper) in self.module.tables.iter().enumerate() {
                #[cfg(feature = "tracing")]
                debug!("Copying table {} to instance (size={})", idx, table_wrapper.size());
                tables.push(table_wrapper.clone());
            }
            #[cfg(feature = "tracing")]
            info!("Populated {} tables for instance {}", self.module.tables.len(), self.instance_id);

            #[cfg(feature = "tracing")]
            kiln_foundation::tracing::trace!(
                table_count = self.module.tables.len(),
                instance_id = self.instance_id,
                "Populated tables for instance"
            );
        }

        Ok(())
    }

    /// Evaluate table init expressions and fill tables with the computed values.
    /// This must be called after globals have been fully resolved (including
    /// imported globals), because table init expressions can reference globals.
    ///
    /// Per the WebAssembly spec, tables with init expressions should have all
    /// their elements initialized to the value produced by the expression.
    pub fn evaluate_table_init_exprs(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, trace};

        let table_init_exprs = &self.module.table_init_exprs;
        if table_init_exprs.is_empty() {
            return Ok(());
        }

        #[cfg(feature = "tracing")]
        debug!(count = table_init_exprs.len(), "Evaluating table init expressions");

        // Lock globals for reading init expression values
        let globals_guard = self
            .globals
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock globals for table init expr evaluation"))?;
        let globals_slice: &[crate::module::GlobalWrapper] = &globals_guard;

        // Lock tables for writing
        let mut tables_guard = self
            .tables
            .lock()
            .map_err(|_| Error::runtime_error("Failed to lock tables for init expr evaluation"))?;

        // Count imported tables to get the correct offset for defined table indices
        let num_table_imports = self.module.import_types.iter()
            .filter(|desc| matches!(desc, crate::module::RuntimeImportDesc::Table(_)))
            .count();

        for (defined_idx, init_expr_opt) in table_init_exprs.iter().enumerate() {
            let init_bytes = match init_expr_opt {
                Some(bytes) => bytes,
                None => continue,
            };

            // Evaluate the init expression using instance globals
            let init_value = crate::module::Module::evaluate_const_expr_with_instance_globals(
                init_bytes,
                self.module.num_global_imports,
                globals_slice,
                &self.module.gc_types,
            )?;

            #[cfg(feature = "tracing")]
            trace!(defined_idx = defined_idx, value = ?init_value, "Table init expression evaluated");

            // The table index in the instance includes imported tables first
            let instance_table_idx = num_table_imports + defined_idx;
            if let Some(table_wrapper) = tables_guard.get(instance_table_idx) {
                let table_size = table_wrapper.size();
                // Fill all elements with the init value
                for slot in 0..table_size {
                    table_wrapper.0.set_shared(slot, Some(init_value.clone()))?;
                }

                #[cfg(feature = "tracing")]
                debug!(
                    instance_table_idx = instance_table_idx,
                    table_size = table_size,
                    "Filled table with init expression value"
                );
            }
        }

        Ok(())
    }

    /// Initialize dropped segment tracking arrays based on module's segment counts
    /// Call this during instance initialization before any elem.drop/data.drop operations
    pub fn initialize_dropped_segments(&self) -> Result<()> {
        let mut dropped_elems = self.dropped_elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock dropped_elements"))?;
        let mut dropped_data = self.dropped_data.lock()
            .map_err(|_| Error::runtime_error("Failed to lock dropped_data"))?;

        // Resize to match module's element and data segment counts
        dropped_elems.resize(self.module.elements.len(), false);
        dropped_data.resize(self.module.data.len(), false);
        Ok(())
    }

    /// Mark an element segment as dropped (called by elem.drop instruction)
    pub fn drop_element_segment(&self, segment_idx: u32) -> Result<()> {
        let mut dropped = self.dropped_elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock dropped_elements"))?;
        if (segment_idx as usize) < dropped.len() {
            dropped[segment_idx as usize] = true;
        }
        Ok(())
    }

    /// Mark a data segment as dropped (called by data.drop instruction)
    pub fn drop_data_segment(&self, segment_idx: u32) -> Result<()> {
        let mut dropped = self.dropped_data.lock()
            .map_err(|_| Error::runtime_error("Failed to lock dropped_data"))?;
        if (segment_idx as usize) < dropped.len() {
            dropped[segment_idx as usize] = true;
        }
        Ok(())
    }

    /// Check if an element segment has been dropped
    pub fn is_element_segment_dropped(&self, segment_idx: u32) -> bool {
        if let Ok(dropped) = self.dropped_elements.lock() {
            dropped.get(segment_idx as usize).copied().unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if a data segment has been dropped
    pub fn is_data_segment_dropped(&self, segment_idx: u32) -> bool {
        if let Ok(dropped) = self.dropped_data.lock() {
            dropped.get(segment_idx as usize).copied().unwrap_or(false)
        } else {
            false
        }
    }

    /// Evaluate an offset constant expression using a stack-based evaluator.
    ///
    /// Supports extended constant expressions including:
    /// - `i32.const`, `i64.const` — push constants
    /// - `global.get` — push global value
    /// - `i32.add`, `i32.sub`, `i32.mul` — binary arithmetic on i32 values
    /// - `i64.add`, `i64.sub`, `i64.mul` — binary arithmetic on i64 values
    ///
    /// Returns the final stack value as a `u32` offset.
    fn evaluate_offset_expr(
        instructions: &[kiln_foundation::types::Instruction<RuntimeProvider>],
        globals: &[GlobalWrapper],
    ) -> Result<u32> {
        use kiln_foundation::types::Instruction;
        use kiln_foundation::values::Value;

        let mut stack: Vec<Value> = Vec::new();

        for instr in instructions {
            match instr {
                Instruction::I32Const(value) => {
                    stack.push(Value::I32(*value));
                }
                Instruction::I64Const(value) => {
                    stack.push(Value::I64(*value));
                }
                Instruction::GlobalGet(global_idx) => {
                    let global_wrapper = globals.iter().nth(*global_idx as usize)
                        .ok_or_else(|| Error::runtime_error(
                            "Global index out of bounds in offset expression"
                        ))?;
                    let global = global_wrapper.0.read()
                        .map_err(|_| Error::runtime_error(
                            "Failed to read global in offset expression"
                        ))?;
                    stack.push(global.get().clone());
                }
                Instruction::I32Add => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.add offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.add offset expression"
                    ))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_add(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i32.add offset expression"
                        ));
                    }
                }
                Instruction::I32Sub => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.sub offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.sub offset expression"
                    ))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_sub(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i32.sub offset expression"
                        ));
                    }
                }
                Instruction::I32Mul => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.mul offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i32.mul offset expression"
                    ))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_mul(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i32.mul offset expression"
                        ));
                    }
                }
                Instruction::I64Add => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.add offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.add offset expression"
                    ))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_add(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i64.add offset expression"
                        ));
                    }
                }
                Instruction::I64Sub => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.sub offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.sub offset expression"
                    ))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_sub(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i64.sub offset expression"
                        ));
                    }
                }
                Instruction::I64Mul => {
                    let b = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.mul offset expression"
                    ))?;
                    let a = stack.pop().ok_or_else(|| Error::runtime_error(
                        "Stack underflow in i64.mul offset expression"
                    ))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_mul(vb)));
                    } else {
                        return Err(Error::runtime_error(
                            "Type mismatch in i64.mul offset expression"
                        ));
                    }
                }
                Instruction::End => {
                    break;
                }
                _other => {
                    return Err(Error::runtime_error(
                        "Unsupported instruction in offset expression"
                    ));
                }
            }
        }

        // Extract the final value from the stack as u32
        let result = stack.pop().ok_or_else(|| Error::runtime_error(
            "Empty stack after evaluating offset expression"
        ))?;
        match result {
            Value::I32(v) => Ok(v as u32),
            Value::I64(v) => u32::try_from(v).map_err(|_| {
                Error::runtime_error("Offset expression result too large for u32")
            }),
            _ => Err(Error::runtime_error(
                "Offset expression did not produce an integer value"
            )),
        }
    }

    /// Initialize data segments into memory
    /// This copies the static data from data segments into the appropriate memory locations
    pub fn initialize_data_segments(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, info};
        use kiln_foundation::DataMode as KilnDataMode;

        #[cfg(feature = "tracing")]
        info!("Initializing data segments for instance {} - module has {} data segments",
              self.instance_id, self.module.data.len());

        #[cfg(feature = "tracing")]
        kiln_foundation::tracing::trace!(
            instance_id = self.instance_id,
            segment_count = self.module.data.len(),
            "Initializing data segments"
        );

        // Iterate through all data segments in the module
        for (idx, data_segment) in self.module.data.iter().enumerate() {
            #[cfg(feature = "tracing")]
            debug!("Processing data segment {}", idx);
            // Only process active data segments
            if let KilnDataMode::Active { .. } = &data_segment.mode {
                #[cfg(feature = "tracing")]
                debug!("Processing active data segment {}", idx);

                // Get the memory index (default to 0 if not specified)
                let memory_idx = data_segment.memory_idx.unwrap_or(0);

                // Get the offset expression and evaluate it using stack-based evaluation
                // to support extended constant expressions (i32.add, i32.sub, i32.mul, etc.)
                let offset = if let Some(ref offset_expr) = data_segment.offset_expr {
                    let globals = self.globals.lock()
                        .map_err(|_| Error::runtime_error("Failed to lock globals for data segment offset"))?;
                    let value = Self::evaluate_offset_expr(&offset_expr.instructions, &globals)?;
                    drop(globals);
                    #[cfg(feature = "tracing")]
                    debug!("Data segment {} offset evaluated to: {}", idx, value);
                    value
                } else {
                    0
                };

                #[cfg(feature = "tracing")]
                debug!("Data segment {} targets memory {} at offset {:#x}", idx, memory_idx, offset);

                // Get the memory instance
                let memories = self.memories.lock()
                    .map_err(|_| Error::runtime_error("Failed to lock memories"))?;

                if memory_idx as usize >= memories.len() {
                    return Err(Error::runtime_error("Data segment references invalid memory index"));
                }

                // Find the memory at the specified index using an iterator
                let memory_wrapper = memories.iter()
                    .nth(memory_idx as usize)
                    .ok_or_else(|| Error::runtime_error("Failed to get memory from collection"))?;
                let memory = &memory_wrapper.0;

                // Write the data to memory
                let init_data = &data_segment.init[..];
                #[cfg(feature = "tracing")]
                debug!("Writing {} bytes of data to memory at offset {:#x}", init_data.len(), offset);

                #[cfg(feature = "tracing")]
                kiln_foundation::tracing::trace!(
                    bytes = init_data.len(),
                    memory_idx = memory_idx,
                    offset = format!("{:#x}", offset),
                    "Writing data to memory"
                );

                // Use the thread-safe write_shared method for Arc<Memory>
                memory.write_shared(offset, init_data)?;

                #[cfg(feature = "tracing")]
                kiln_foundation::tracing::trace!(segment_idx = idx, "Successfully wrote data segment");

                #[cfg(feature = "tracing")]
                info!("Successfully initialized data segment {} ({} bytes)", idx, init_data.len());
            } else {
                #[cfg(feature = "tracing")]
                debug!("Skipping passive data segment {}", idx);
            }
        }

        #[cfg(feature = "tracing")]
        info!("Data segment initialization complete for instance {}", self.instance_id);
        Ok(())
    }

    /// Initialize element segments into tables
    /// This populates table entries from active element segments
    pub fn initialize_element_segments(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        use kiln_foundation::tracing::{debug, info};
        use kiln_foundation::types::ElementMode as KilnElementMode;
        use kiln_foundation::values::{Value as KilnValue, FuncRef as KilnFuncRef};

        #[cfg(feature = "tracing")]
        info!("Initializing element segments for instance {} - module has {} element segments",
              self.instance_id, self.module.elements.len());

        #[cfg(feature = "tracing")]
        kiln_foundation::tracing::trace!(
            instance_id = self.instance_id,
            segment_count = self.module.elements.len(),
            "Initializing element segments"
        );

        // Get access to tables
        let tables = self.tables.lock()
            .map_err(|_| Error::runtime_error("Failed to lock tables"))?;

        // Iterate through all element segments in the module
        {
            // Get access to globals for evaluating offset expressions
            let globals = self.globals.lock()
                .map_err(|_| Error::runtime_error("Failed to lock globals for element init"))?;

            for (idx, elem_segment) in self.module.elements.iter().enumerate() {
                #[cfg(feature = "tracing")]
                debug!("Processing element segment {}", idx);
                // Only process active element segments
                if let KilnElementMode::Active { table_index, offset: mode_offset } = &elem_segment.mode {
                    // Evaluate the actual offset using stack-based evaluation
                    // to support extended constant expressions (i32.add, i32.sub, i32.mul, etc.)
                    let actual_offset = if let Some(ref offset_expr) = elem_segment.offset_expr {
                        let value = Self::evaluate_offset_expr(&offset_expr.instructions, &globals)?;
                        #[cfg(feature = "tracing")]
                        debug!("Element segment {} offset evaluated to: {}", idx, value);
                        value
                    } else {
                        *mode_offset
                    };

                    #[cfg(feature = "tracing")]
                    debug!("Processing active element segment {}: table={}, offset={}, items={}",
                           idx, table_index, actual_offset, elem_segment.items.len());

                    #[cfg(feature = "tracing")]
                    kiln_foundation::tracing::trace!(
                        segment_idx = idx,
                        table_index = table_index,
                        offset = actual_offset,
                        items = elem_segment.items.len(),
                        "Processing active element segment"
                    );

                    // Get the table
                    let table_idx = *table_index as usize;
                    if table_idx >= tables.len() {
                        return Err(Error::runtime_error("Element segment references invalid table index"));
                    }

                    let table_wrapper = &tables[table_idx];
                    let table = table_wrapper.inner();

                    // Set each element in the table
                    // Use the element segment's type to determine if we're dealing with
                    // funcref or externref elements
                    let is_externref = matches!(elem_segment.element_type, kiln_foundation::types::RefType::Externref);

                    for (item_idx, func_idx) in elem_segment.items.iter().enumerate() {
                        let table_offset = actual_offset + item_idx as u32;

                        // Handle sentinel values from module conversion:
                        // u32::MAX = ref.null (null reference)
                        // u32::MAX - 1 = deferred (will be evaluated later by item_exprs)
                        let value = if func_idx == u32::MAX {
                            // ref.null - set to null reference based on element type
                            if is_externref {
                                Some(KilnValue::ExternRef(None))
                            } else {
                                Some(KilnValue::FuncRef(None))
                            }
                        } else if func_idx == u32::MAX - 1 {
                            // Deferred - skip, will be set by item_exprs processing below
                            continue;
                        } else {
                            // Normal function reference (only valid for funcref elements)
                            // Stamp with this instance's ID for cross-instance table sharing
                            let fref = KilnFuncRef::from_index_with_instance(func_idx, self.instance_id as u32);
                            Some(KilnValue::FuncRef(Some(fref)))
                        };

                        // Use set_shared which provides interior mutability
                        table.set_shared(table_offset, value)?;

                        #[cfg(feature = "tracing")]
                        if item_idx < 3 || item_idx == elem_segment.items.len() - 1 {
                            kiln_foundation::tracing::trace!(table_offset = table_offset, func_idx = func_idx, "Set table element");
                        }
                    }

                    // Evaluate and set deferred item expressions (e.g., global.get, ref.i31, struct.new)
                    for (item_idx, expr) in elem_segment.item_exprs.iter() {
                        let table_offset = actual_offset + *item_idx;
                        // Evaluate the constant expression by interpreting its instructions
                        use kiln_foundation::types::Instruction as KilnInstr;
                        let mut eval_stack: Vec<KilnValue> = Vec::new();
                        for instr in &expr.instructions {
                            match instr {
                                KilnInstr::I32Const(val) => {
                                    eval_stack.push(KilnValue::I32(*val));
                                }
                                KilnInstr::I64Const(val) => {
                                    eval_stack.push(KilnValue::I64(*val));
                                }
                                KilnInstr::GlobalGet(global_idx) => {
                                    if let Some(global_wrapper) = globals.iter().nth(*global_idx as usize) {
                                        if let Ok(global) = global_wrapper.0.read() {
                                            eval_stack.push(global.get().clone());
                                        }
                                    }
                                }
                                KilnInstr::RefFunc(func_idx) => {
                                    let fref = KilnFuncRef::from_index_with_instance(*func_idx, self.instance_id as u32);
                                    eval_stack.push(KilnValue::FuncRef(Some(fref)));
                                }
                                KilnInstr::RefNull(_) => {
                                    // Push appropriate null based on element type
                                    eval_stack.push(KilnValue::FuncRef(None));
                                }
                                KilnInstr::RefI31 => {
                                    // ref.i31: pop i32, push i31ref
                                    if let Some(KilnValue::I32(n)) = eval_stack.pop() {
                                        eval_stack.push(KilnValue::I31Ref(Some(n & 0x7FFFFFFF)));
                                    }
                                }
                                KilnInstr::End => {
                                    // End of expression - stop evaluating
                                    break;
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]
                                    kiln_foundation::tracing::trace!(
                                        table_offset = table_offset,
                                        instr = ?instr,
                                        "Unhandled instruction in element expression"
                                    );
                                }
                            }
                        }
                        // Set the final value from the evaluation stack
                        if let Some(value) = eval_stack.pop() {
                            #[cfg(feature = "tracing")]
                            kiln_foundation::tracing::trace!(
                                table_offset = table_offset,
                                value = ?value,
                                "Set table element from expression"
                            );
                            table.set_shared(table_offset, Some(value))?;
                        }
                    }

                    #[cfg(feature = "tracing")]
                    info!("Initialized element segment {} ({} items) into table {} at offset {}",
                          idx, elem_segment.items.len(), table_index, actual_offset);
                } else {
                    #[cfg(feature = "tracing")]
                    debug!("Skipping non-active element segment {}", idx);
                }
            }
        }

        #[cfg(feature = "tracing")]
        info!("Element segment initialization complete for instance {}", self.instance_id);
        Ok(())
    }

    /// Initialize debug information for this instance
    #[cfg(feature = "debug")]
    pub fn init_debug_info(&mut self, module_bytes: &'static [u8]) -> Result<()> {
        let debug_info = DwarfDebugInfo::new(module_bytes)?;

        // TODO: Extract debug section offsets from the module
        // For now, this is a placeholder that would need module parsing integration

        self.debug_info = Some(debug_info);
        Ok(())
    }

    /// Get line information for a given program counter
    #[cfg(feature = "debug")]
    pub fn get_line_info(&mut self, pc: u32) -> Result<Option<LineInfo>> {
        if let Some(ref mut debug_info) = self.debug_info {
            debug_info
                .find_line_info(pc)
                .map_err(|e| Error::runtime_debug_info_error("Runtime operation error"))
        } else {
            Ok(None)
        }
    }

    /// Get function information for a given program counter
    #[cfg(feature = "debug-full")]
    pub fn get_function_info(&self, pc: u32) -> Option<&FunctionInfo> {
        self.debug_info.as_ref()?.find_function_info(pc)
    }

    /// Check if debug information is available
    #[cfg(feature = "debug")]
    pub fn has_debug_info(&self) -> bool {
        self.debug_info.as_ref().is_some_and(|di| di.has_debug_info())
    }

    /// Get a function by index - alias for compatibility with tail_call.rs
    pub fn get_function(&self, idx: usize) -> Result<crate::module::Function> {
        self.module
            .functions
            .get(idx)
            .cloned()
            .ok_or_else(|| Error::runtime_function_not_found("Function index not found"))
    }

    /// Get function type by index - alias for compatibility with tail_call.rs
    pub fn get_function_type(&self, idx: usize) -> Result<KilnFuncType> {
        let function = self
            .module
            .functions
            .get(idx)
            .ok_or_else(|| Error::runtime_function_not_found("Function index not found"))?;

        self.module.types.get(function.type_idx as usize)
            .cloned()
            .ok_or_else(|| Error::runtime_error("Function type index out of bounds"))
    }

    /// Get a table by index - alias for compatibility with tail_call.rs
    pub fn get_table(&self, idx: usize) -> Result<TableWrapper> {
        self.table(idx as u32)
    }

    /// Get a type by index - alias for compatibility with tail_call.rs
    pub fn get_type(&self, idx: usize) -> Result<KilnFuncType> {
        self.module.types.get(idx)
            .cloned()
            .ok_or_else(|| Error::runtime_error("Type index out of bounds"))
    }
}

/// Implementation of ReferenceOperations trait for ModuleInstance
impl ReferenceOperations for ModuleInstance {
    fn get_function(&self, function_index: u32) -> Result<Option<u32>> {
        // Check if function exists in module
        if (function_index as usize) < self.module.functions.len() {
            Ok(Some(function_index))
        } else {
            Ok(None)
        }
    }

    fn validate_function_index(&self, function_index: u32) -> Result<()> {
        if (function_index as usize) < self.module.functions.len() {
            Ok(())
        } else {
            Err(Error::runtime_function_not_found(
                "Function index out of bounds",
            ))
        }
    }
}

// Implement the ModuleInstance trait for module_instance - temporarily disabled
// impl crate::stackless::extensions::ModuleInstance for ModuleInstance {
// fn module(&self) -> &Module {
//     &self.module
// }

// fn memory(&self, idx: u32) -> Result<MemoryWrapper> {
//     self.memory(idx)
// }

// fn table(&self, idx: u32) -> Result<TableWrapper> {
//     self.table(idx)
// }

// fn global(&self, idx: u32) -> Result<GlobalWrapper> {
//     self.global(idx)
// }

// fn function_type(&self, idx: u32) -> Result<FuncType> {
//     self.function_type(idx)
// }
// } // End of commented impl block

/// Manual trait implementations for ModuleInstance since fields don't support
/// automatic derivation
/// REMOVED: Default implementation causes stack overflow through Module::empty()
/// Use ModuleInstance::new() with proper initialization instead
/* DISABLED - CAUSES STACK OVERFLOW
impl Default for ModuleInstance {
    fn default() -> Self {
        // Create a default module instance with an empty module
        let default_module = Module::empty();
        // Default implementation must succeed for basic functionality
        // Use minimal memory allocation that should always work
        match Self::new(Arc::new(default_module), 0) {
            Ok(instance) => instance,
            Err(_) => {
                // Create minimal instance using RuntimeProvider for type consistency
                // This maintains controllability while avoiding allocation failures
                use crate::bounded_runtime_infra::create_runtime_provider;
                // Use the factory function - if this fails, we have a fundamental system issue
                let runtime_provider = match create_runtime_provider() {
                    Ok(provider) => provider,
                    Err(_) => {
                        // Last resort: try to create a minimal provider
                        // This should work even in constrained environments
                        match create_runtime_provider() {
                            Ok(provider) => provider,
                            Err(_) => {
                                // System is in unrecoverable state - but we must return something
                                // Create an invalid instance that will fail safely later
                                return Self {
                                    module: Arc::new(Module::empty()),
                                    memories: Arc::new(Mutex::new(Default::default())),
                                    tables: Arc::new(Mutex::new(Default::default())),
                                    globals: Arc::new(Mutex::new(Default::default())),
                                    instance_id: 0,
                                    imports: Default::default(),
                                    #[cfg(feature = "debug")]
                                    debug_info: None,
                                };
                            },
                        }
                    },
                };
                Self {
                    module: Arc::new(Module::empty()),
                    memories: Arc::new(Mutex::new(
                        // Try to create with RuntimeProvider, fallback to empty vector creation
                        kiln_foundation::bounded::BoundedVec::new(runtime_provider.clone())
                            .unwrap_or_else(|_| {
                                // Last resort: try creating another provider
                                let fallback_provider = create_runtime_provider()
                                    .expect("Failed to create fallback runtime provider");
                                kiln_foundation::bounded::BoundedVec::new(fallback_provider)
                                    .expect("Failed to create even minimal memory vector")
                            }),
                    )),
                    tables: Arc::new(Mutex::new(
                        kiln_foundation::bounded::BoundedVec::new(runtime_provider.clone())
                            .unwrap_or_else(|_| {
                                let fallback_provider = create_runtime_provider()
                                    .expect("Failed to create fallback runtime provider");
                                kiln_foundation::bounded::BoundedVec::new(fallback_provider)
                                    .expect("Failed to create even minimal table vector")
                            }),
                    )),
                    globals: Arc::new(Mutex::new(
                        kiln_foundation::bounded::BoundedVec::new(runtime_provider).unwrap_or_else(
                            |_| {
                                let fallback_provider = create_runtime_provider()
                                    .expect("Failed to create fallback runtime provider");
                                kiln_foundation::bounded::BoundedVec::new(fallback_provider)
                                    .expect("Failed to create even minimal global vector")
                            },
                        ),
                    )),
                    instance_id: 0,
                    imports: Default::default(),
                    #[cfg(feature = "debug")]
                    debug_info: None,
                }
            },
        }
    }
}
*/ // End of DISABLED Default impl

impl Clone for ModuleInstance {
    fn clone(&self) -> Self {
        // IMPORTANT: Clone must share the same memories/tables/globals via Arc
        // A previous buggy implementation called Self::new() which creates fresh
        // empty containers - this caused memory writes during cabi_realloc to be lost!
        Self {
            module: Arc::clone(&self.module),
            memories: Arc::clone(&self.memories),
            tables: Arc::clone(&self.tables),
            globals: Arc::clone(&self.globals),
            dropped_elements: Arc::clone(&self.dropped_elements),
            dropped_data: Arc::clone(&self.dropped_data),
            instance_id: self.instance_id,
            imports: self.imports.clone(),
            #[cfg(feature = "debug")]
            debug_info: None, // Debug info is not cloned for simplicity
        }
    }
}

impl PartialEq for ModuleInstance {
    fn eq(&self, other: &Self) -> bool {
        // Compare based on instance ID and module equality
        self.instance_id == other.instance_id && self.module == other.module
    }
}

impl Eq for ModuleInstance {}

/// Trait implementations for ModuleInstance to support BoundedMap usage
impl Checksummable for ModuleInstance {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Use instance ID and module checksum for unique identification
        checksum.update_slice(&self.instance_id.to_le_bytes());

        // Include module checksum if the module implements Checksummable
        // For now, use a simplified approach with module validation status
        if let Some(name) = self.module.name.as_ref() {
            if let Ok(name_str) = name.as_str() {
                checksum.update_slice(name_str.as_bytes());
            } else {
                checksum.update_slice(b"invalid_module_name");
            }
        } else {
            checksum.update_slice(b"unnamed_module_instance");
        }

        // Include counts of resources for uniqueness
        let memories_count = self.memories.lock().map_or(0, |m| m.len()) as u32;
        let tables_count = self.tables.lock().map_or(0, |t| t.len()) as u32;
        let globals_count = self.globals.lock().map_or(0, |g| g.len()) as u32;

        checksum.update_slice(&memories_count.to_le_bytes());
        checksum.update_slice(&tables_count.to_le_bytes());
        checksum.update_slice(&globals_count.to_le_bytes());
    }
}

impl ToBytes for ModuleInstance {
    fn serialized_size(&self) -> usize {
        // Simplified size calculation for module instance metadata
        // instance_id (8) + resource counts (12) + module name length estimation (64)
        8 + 12 + 64
    }

    fn to_bytes_with_provider<'a, PStream: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> Result<()> {
        // Write instance ID
        writer.write_all(&self.instance_id.to_le_bytes())?;

        // Write resource counts
        let memories_count = self.memories.lock().map_or(0, |m| m.len()) as u32;
        let tables_count = self.tables.lock().map_or(0, |t| t.len()) as u32;
        let globals_count = self.globals.lock().map_or(0, |g| g.len()) as u32;

        writer.write_all(&memories_count.to_le_bytes())?;
        writer.write_all(&tables_count.to_le_bytes())?;
        writer.write_all(&globals_count.to_le_bytes())?;

        // Write module name (simplified)
        if let Some(name) = self.module.name.as_ref() {
            if let Ok(name_str) = name.as_str() {
                let name_bytes = name_str.as_bytes();
                writer.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            } else {
                // Write zero length for invalid name
                writer.write_all(&0u32.to_le_bytes())?;
            }
        } else {
            // Write zero length for no name
            writer.write_all(&0u32.to_le_bytes())?;
        }

        Ok(())
    }
}

impl FromBytes for ModuleInstance {
    fn from_bytes_with_provider<'a, PStream: kiln_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> Result<Self> {
        // Read instance ID
        let mut instance_id_bytes = [0u8; 8];
        reader.read_exact(&mut instance_id_bytes)?;
        let instance_id = usize::from_le_bytes(instance_id_bytes);

        // Read resource counts (for validation, but create empty collections)
        let mut counts = [0u8; 12];
        reader.read_exact(&mut counts)?;

        // Read module name length
        let mut name_len_bytes = [0u8; 4];
        reader.read_exact(&mut name_len_bytes)?;
        let name_len = u32::from_le_bytes(name_len_bytes) as usize;

        // Skip reading the name for now (simplified implementation)
        if name_len > 0 {
            let mut name_bytes = std::vec![0u8; name_len];
            reader.read_exact(&mut name_bytes)?;
        }

        // Create a default module instance with empty collections using create_runtime_provider
        // This is a simplified implementation - in a real scenario,
        // you'd need to reconstruct the actual module
        let provider = crate::bounded_runtime_infra::create_runtime_provider()?;

        let default_module = Module {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            tags: Vec::new(),
            elements: Vec::new(),
            data: Vec::new(),
            start: None,
            custom_sections: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            exports: kiln_foundation::direct_map::DirectMap::new(),
            name: None,
            binary: None,
            validated: false,
            num_global_imports: 0,
            global_import_types: Vec::new(),
            deferred_global_inits: Vec::new(),
            import_types: Vec::new(),
            num_import_functions: 0,
            gc_types: Vec::new(),
            type_supertypes: Vec::new(),
            table_init_exprs: Vec::new(),
        };

        // Create the instance using the new method
        Self::new(Arc::new(default_module), instance_id).map_err(|_| {
            kiln_error::Error::runtime_error("Failed to create module instance from bytes")
        })
    }
}
