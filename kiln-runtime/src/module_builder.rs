//! Module builder for WebAssembly runtime
//!
//! This module provides an implementation of the RuntimeModuleBuilder trait
//! from kiln-decoder, allowing the conversion of decoder modules to runtime
//! modules.

// Decoder imports are optional during development
// use kiln_decoder::{module::CodeSection,
// runtime_adapter::RuntimeModuleBuilder};

use std::format;
use alloc::vec::Vec;

use kiln_format::{
    DataSegment as KilnDataSegment,
    ElementSegment as KilnElementSegment,
};
use kiln_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    types::{
        CustomSection as KilnCustomSection,
        FuncType,
        GlobalType as KilnGlobalType,
        Limits as KilnLimits,
        MemoryType as KilnMemoryType,
        TableType as KilnTableType,
        ValueType as KilnValueType,
    },
    values::Value as KilnValue,
};

// Add placeholder aliases for missing types
use crate::module::{
    Export as KilnExport,
    Function,
    Import as KilnImport,
    LocalEntry,
    KilnExpr,
};
use crate::{
    bounded_runtime_infra::create_runtime_provider,
    memory_adapter::StdMemoryProvider,
    module::Module,
    prelude::*,
};

// String type for runtime
type String = alloc::string::String;

/// Trait for building WebAssembly runtime modules from decoder output.
pub trait RuntimeModuleBuilder {
    /// The type of module that this builder produces.
    type Module;

    /// Creates a new module builder.
    fn new() -> Self;
    /// Sets the module name.
    fn set_name(&mut self, name: String);
    /// Sets the start function index.
    fn set_start(&mut self, start_func: u32);
    /// Adds a function type to the module.
    fn add_type(&mut self, func_type: FuncType) -> Result<u32>;
    /// Adds a function type to the module (alias for add_type).
    fn add_function_type(&mut self, func_type: FuncType) -> Result<u32>;
    /// Adds an import to the module.
    fn add_import(&mut self, import: KilnImport) -> Result<u32>;
    /// Adds a function declaration to the module.
    fn add_function(&mut self, type_idx: u32) -> Result<u32>;
    /// Adds a function body with bytecode to the module.
    fn add_function_body(
        &mut self,
        func_idx: u32,
        type_idx: u32,
        body: kiln_foundation::bounded::BoundedVec<
            u8,
            4096,
            crate::bounded_runtime_infra::RuntimeProvider,
        >,
    ) -> Result<()>;
    /// Adds a memory declaration to the module.
    fn add_memory(&mut self, memory_type: KilnMemoryType) -> Result<u32>;
    /// Adds a table declaration to the module.
    fn add_table(&mut self, table_type: KilnTableType) -> Result<u32>;
    /// Adds a global variable to the module.
    fn add_global(&mut self, global_type: KilnGlobalType) -> Result<u32>;
    /// Adds an export to the module.
    fn add_export(&mut self, export: KilnExport) -> Result<()>;
    /// Adds an element segment to the module.
    fn add_element(&mut self, element: KilnElementSegment) -> Result<u32>;
    /// Adds a data segment to the module.
    fn add_data(&mut self, data: KilnDataSegment) -> Result<u32>;
    /// Adds a custom section to the module.
    fn add_custom_section(
        &mut self,
        section: KilnCustomSection<crate::bounded_runtime_infra::RuntimeProvider>,
    ) -> Result<()>;
    /// Builds and returns the completed module.
    fn build(self) -> Result<Self::Module>;
}

/// Builder for runtime modules
pub struct ModuleBuilder {
    /// Module being built.
    module:              Module,
    /// Keep track of imported functions to correctly index defined functions.
    imported_func_count: u32,
}

impl RuntimeModuleBuilder for ModuleBuilder {
    type Module = Module;

    /// Creates a new module builder.
    fn new() -> Self {
        // Create module directly to avoid Module::new()
        let provider = crate::bounded_runtime_infra::create_runtime_provider()
            .expect("Failed to create runtime provider");
        let module = Module {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone()).expect("Failed to create imports"),
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: kiln_foundation::bounded::BoundedVec::new(provider.clone()).expect("Failed to create globals"),
            tags: Vec::new(),
            elements: Vec::new(),
            data: Vec::new(),
            start: None,
            custom_sections: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone()).expect("Failed to create custom_sections"),
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
        };
        Self {
            module,
            imported_func_count: 0,
        }
    }

    fn set_name(&mut self, _name: String) {
        // Name setting not implemented in current Module struct
    }

    fn set_start(&mut self, _start_func: u32) {
        // Start function setting not implemented in current Module struct
    }

    fn add_type(&mut self, func_type: FuncType) -> Result<u32> {
        self.add_function_type(func_type)
    }

    fn add_import(&mut self, _import: KilnImport) -> Result<u32> {
        // Import handling not implemented
        self.imported_func_count += 1;
        Ok(self.imported_func_count - 1)
    }

    fn add_function(&mut self, _type_idx: u32) -> Result<u32> {
        // Function addition without body
        Ok(0)
    }

    fn add_export(&mut self, _export: KilnExport) -> Result<()> {
        // Export handling not implemented
        Ok(())
    }

    fn add_element(&mut self, _element: KilnElementSegment) -> Result<u32> {
        // Element segment handling not implemented
        Ok(0)
    }

    fn add_data(&mut self, _data: KilnDataSegment) -> Result<u32> {
        // Data segment handling not implemented
        Ok(0)
    }

    fn add_custom_section(
        &mut self,
        _section: KilnCustomSection<crate::bounded_runtime_infra::RuntimeProvider>,
    ) -> Result<()> {
        // Custom section handling not implemented
        Ok(())
    }

    fn add_function_type(&mut self, _func_type: FuncType) -> Result<u32> {
        // Function type addition not implemented
        Ok(0)
    }

    fn add_function_body(
        &mut self,
        func_idx: u32,
        type_idx: u32,
        body: kiln_foundation::bounded::BoundedVec<
            u8,
            4096,
            crate::bounded_runtime_infra::RuntimeProvider,
        >,
    ) -> Result<()> {
        use crate::{
            instruction_parser::parse_instructions,
            module::{
                Function,
                KilnExpr,
            },
        };

        // Convert BoundedVec to slice for parsing
        let bytecode_slice = body.as_slice();

        // For now, create empty function with proper types
        // TODO: Implement proper bytecode parsing with compatible types
        let instructions = Vec::new();

        let provider2 = create_runtime_provider()?;
        let locals = kiln_foundation::bounded::BoundedVec::new(provider2)?;

        // Create the function with proper types
        let function = Function {
            type_idx,
            locals,
            body: KilnExpr { instructions },
        };

        // Add to module's functions
        // Note: This assumes functions are stored somewhere in the module
        // The actual implementation depends on how Module stores functions

        Ok(())
    }

    fn add_memory(&mut self, _memory_type: KilnMemoryType) -> Result<u32> {
        // Memory addition not implemented
        Ok(0)
    }

    fn add_table(&mut self, _table_type: KilnTableType) -> Result<u32> {
        // Table addition not implemented
        Ok(0)
    }

    fn add_global(&mut self, _global_type: KilnGlobalType) -> Result<u32> {
        // Global addition not implemented
        Ok(0)
    }

    fn build(self) -> Result<Self::Module> {
        // Return the built module
        Ok(self.module)
    }

    // All trait methods implemented above with stub implementations
}

/// Parses local variable declarations from function body bytecode.
fn parse_locals_from_body(
    bytecode: &[u8],
) -> Result<
    kiln_foundation::bounded::BoundedVec<
        kiln_foundation::types::LocalEntry,
        64,
        crate::bounded_runtime_infra::RuntimeProvider,
    >,
> {
    use kiln_foundation::{
        bounded::BoundedVec,
        types::LocalEntry,
    };

    let provider = create_runtime_provider()?;
    let mut locals = BoundedVec::new(provider)?;

    if bytecode.is_empty() {
        return Ok(locals);
    }

    let mut offset = 0;

    // Read local count (LEB128)
    let (local_count, consumed) = read_leb128_u32(bytecode, offset)?;
    offset += consumed;

    // Parse each local entry
    for _ in 0..local_count {
        if offset >= bytecode.len() {
            return Err(Error::parse_error(
                "Unexpected end of bytecode while parsing locals",
            ));
        }

        // Read count of this local type
        let (count, consumed) = read_leb128_u32(bytecode, offset)?;
        offset += consumed;

        if offset >= bytecode.len() {
            return Err(Error::parse_error(
                "Unexpected end of bytecode while parsing local type",
            ));
        }

        // Read value type
        let value_type = match bytecode[offset] {
            0x7F => kiln_foundation::types::ValueType::I32,
            0x7E => kiln_foundation::types::ValueType::I64,
            0x7D => kiln_foundation::types::ValueType::F32,
            0x7C => kiln_foundation::types::ValueType::F64,
            _ => return Err(Error::parse_error("Invalid value type for local variable")),
        };
        offset += 1;

        let local_entry = LocalEntry { count, value_type };
        locals.push(local_entry)?;
    }

    Ok(locals)
}

/// Reads a LEB128-encoded u32 value from bytecode at the specified offset.
fn read_leb128_u32(bytecode: &[u8], offset: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut consumed = 0;

    loop {
        if offset + consumed >= bytecode.len() {
            return Err(Error::parse_error(
                "Unexpected end of bytecode while reading LEB128",
            ));
        }

        let byte = bytecode[offset + consumed];
        consumed += 1;

        result |= ((byte & 0x7F) as u32) << shift;

        if (byte & 0x80) == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            return Err(Error::parse_error("LEB128 value too large"));
        }
    }

    Ok((result, consumed))
}

impl ModuleBuilder {
    /// Creates a new module builder with an existing binary.
    pub fn with_binary(_binary: Vec<u8>) -> Result<Self> {
        // Create module directly to avoid Module::new()
        let provider = crate::bounded_runtime_infra::create_runtime_provider()?;
        let module = Module {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: kiln_foundation::bounded::BoundedVec::new(provider.clone())?,
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
        };
        Ok(Self {
            module,
            imported_func_count: 0,
        })
    }

    /// Sets the binary representation of the module.
    pub fn set_binary(&mut self, _binary: Vec<u8>) -> Result<()> {
        Ok(())
    }
}

/// Loads a module from binary data using the module builder.
pub fn load_module_from_binary(binary: &[u8]) -> Result<Module> {
    #[cfg(feature = "decoder")]
    {
        // Enter runtime scope to cover both decoding and conversion
        // This ensures decoder's Vec allocations remain valid during conversion to BoundedVec
        let _scope = kiln_foundation::capabilities::MemoryFactory::enter_module_scope(
            kiln_foundation::budget_aware_provider::CrateId::Runtime,
        )?;

        let decoder_module = Box::new(kiln_decoder::decode_module(binary)?);
        Module::from_kiln_module(&*decoder_module).map(|boxed| *boxed)  // Dereference Box
        // Scope drops here, memory available for reuse
    }
    #[cfg(not(feature = "decoder"))]
    {
        // Decoder not available - create an empty module
        Err(Error::runtime_execution_error("Decoder not available"))
    }
}
