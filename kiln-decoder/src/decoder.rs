//! Main decoder module for WebAssembly binaries
//!
//! This module provides the high-level API for decoding WebAssembly modules
//! from binary format into the runtime representation.

#[cfg(not(feature = "std"))]
extern crate alloc;

// Import Vec type explicitly
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use alloc::vec::Vec;

use wrt_format::module::{ImportDesc, Module as WrtModule};
use wrt_foundation::safe_memory::NoStdProvider;

use crate::prelude::*;

/// Default provider for decoder operations
type DecoderProvider = NoStdProvider<65536>;

/// Decode a WebAssembly module from binary format
///
/// This function uses streaming processing to minimize memory usage.
/// The entire binary is not loaded into intermediate structures.
///
/// # Arguments
/// * `binary` - The WebAssembly binary data
///
/// # Returns
/// * A decoded Module ready for runtime use
#[cfg(feature = "std")]
pub fn decode_module(binary: &[u8]) -> Result<WrtModule> {
    // Use streaming decoder for std builds
    crate::streaming_decoder::decode_module_streaming(binary)
}

/// Decode a WebAssembly module from binary format (no_std version)
#[cfg(not(feature = "std"))]
pub fn decode_module(binary: &[u8]) -> Result<WrtModule<DecoderProvider>> {
    // Use the streaming decoder for minimal memory usage
    let module = crate::streaming_decoder::decode_module_streaming(binary)?;
    // Convert from the streaming decoder's provider to our provider
    // This is a temporary conversion - ideally we'd use the same provider
    Ok(convert_module_provider(module))
}

/// Convert a module from one provider to another (std version)
#[cfg(feature = "std")]
fn convert_module_provider(source: WrtModule) -> WrtModule {
    // In std mode, no conversion needed
    source
}

/// Convert a module from one provider to another (no_std version)
#[cfg(not(feature = "std"))]
fn convert_module_provider(_source: WrtModule<NoStdProvider<8192>>) -> WrtModule<DecoderProvider> {
    // For now, create a new empty module
    // In a real implementation, we would copy all fields
    let provider = DecoderProvider::default();
    WrtModule::default()
}

/// Convert import descriptor from foundation to format types (std version)
#[cfg(feature = "std")]
fn convert_import_desc(
    desc: wrt_foundation::types::ImportDesc<DecoderProvider>,
) -> wrt_format::module::ImportDesc {
    match desc {
        wrt_foundation::types::ImportDesc::Function(idx) => {
            wrt_format::module::ImportDesc::Function(idx)
        },
        wrt_foundation::types::ImportDesc::Table(table_type) => {
            // wrt_format::module::Table is a type alias for WrtTableType
            wrt_format::module::ImportDesc::Table(table_type)
        },
        wrt_foundation::types::ImportDesc::Memory(mem_type) => {
            // wrt_format::module::Memory is a type alias for WrtMemoryType
            wrt_format::module::ImportDesc::Memory(mem_type)
        },
        wrt_foundation::types::ImportDesc::Global(global_type) => {
            wrt_format::module::ImportDesc::Global(wrt_format::types::FormatGlobalType {
                value_type: global_type.value_type,
                mutable: global_type.mutable,
            })
        },
        wrt_foundation::types::ImportDesc::Extern(_) => {
            // For now, treat extern as function
            wrt_format::module::ImportDesc::Function(0)
        },
        wrt_foundation::types::ImportDesc::Resource(_) => {
            // For now, treat resource as function
            wrt_format::module::ImportDesc::Function(0)
        },
        wrt_foundation::types::ImportDesc::_Phantom(_) => {
            // This should never occur in practice, but we need to handle it
            wrt_format::module::ImportDesc::Function(0)
        },
        wrt_foundation::types::ImportDesc::Tag(tag_type) => {
            wrt_format::module::ImportDesc::Tag(tag_type.type_idx)
        },
    }
}

#[cfg(feature = "std")]
fn build_module_from_sections(sections: Vec<crate::sections::Section>) -> Result<WrtModule> {
    let mut module = WrtModule {
        types: Vec::new(),
        functions: Vec::new(),
        tables: Vec::new(),
        memories: Vec::new(),
        globals: Vec::new(),
        elements: Vec::new(),
        data: Vec::new(),
        exports: Vec::new(),
        imports: Vec::new(),
        start: None,
        custom_sections: Vec::new(),
        binary: None,
        core_version: wrt_format::types::CoreWasmVersion::default(),
        type_info_section: None,
        tags: Vec::new(),
    };

    for section in sections {
        match section {
            crate::sections::Section::Type(types) => {
                for func_type in types {
                    // Convert FuncType to CleanCoreFuncType
                    let clean_func_type = wrt_foundation::CleanCoreFuncType {
                        params: func_type.params.into_iter().collect(),
                        results: func_type.results.into_iter().collect(),
                    };
                    module.types.push(clean_func_type);
                }
            },
            crate::sections::Section::Import(imports) => {
                for import in imports {
                    // Convert from wrt_foundation Import to wrt_format Import
                    let format_import = wrt_format::module::Import {
                        module: import.module_name.as_str().unwrap_or("").to_string(),
                        name: import.item_name.as_str().unwrap_or("").to_string(),
                        desc: convert_import_desc(import.desc.clone()),
                    };
                    module.imports.push(format_import);

                    // CRITICAL: Add imported functions to the functions vector
                    // This ensures proper function indexing
                    if let wrt_foundation::types::ImportDesc::Function(type_idx) = import.desc {
                        let func = wrt_format::module::Function {
                            type_idx,
                            locals: Vec::new(),
                            code: Vec::new(), // Imported functions have no code
                        };
                        module.functions.push(func);
                    }
                }
            },
            crate::sections::Section::Function(func_indices) => {
                for type_idx in func_indices {
                    let func = wrt_format::module::Function {
                        type_idx,
                        locals: Vec::new(),
                        code: Vec::new(),
                    };
                    module.functions.push(func);
                }
            },
            crate::sections::Section::Table(tables) => {
                for table in tables {
                    module.tables.push(table);
                }
            },
            crate::sections::Section::Memory(memories) => {
                for memory in memories {
                    module.memories.push(memory);
                }
            },
            crate::sections::Section::Global(globals) => {
                for global_type in globals {
                    // Convert from GlobalType to Global
                    let global = wrt_format::module::Global {
                        global_type: wrt_format::types::FormatGlobalType {
                            value_type: global_type.value_type,
                            mutable: global_type.mutable,
                        },
                        init: Vec::new(), /* Empty init - should be populated from code
                                           * section */
                    };
                    module.globals.push(global);
                }
            },
            crate::sections::Section::Export(exports) => {
                for export in exports {
                    module.exports.push(export);
                }
            },
            crate::sections::Section::Start(start_idx) => {
                module.start = Some(start_idx);
            },
            crate::sections::Section::Element(elements) => {
                for element in elements {
                    module.elements.push(element);
                }
            },
            crate::sections::Section::Data(data_segments) => {
                for data in data_segments {
                    module.data.push(data);
                }
            },
            crate::sections::Section::Code(code_bodies) => {
                // Update function bodies
                // CRITICAL: The code section only contains code for local functions.
                // Imported functions are already in the functions vector with empty code.
                // We need to skip imported functions when assigning code.
                let num_imports = module
                    .imports
                    .iter()
                    .filter(|import| matches!(import.desc, ImportDesc::Function(..)))
                    .count();

                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::trace!(
                    code_bodies = code_bodies.len(),
                    num_imports = num_imports,
                    total_functions = module.functions.len(),
                    "Code section parsed"
                );

                for (idx, body) in code_bodies.into_iter().enumerate() {
                    // The actual function index is num_imports + idx
                    let func_idx = num_imports + idx;

                    #[cfg(feature = "tracing")]
                    wrt_foundation::tracing::trace!(
                        idx = idx,
                        func_idx = func_idx,
                        "Assigning code body to function"
                    );

                    if let Some(func) = module.functions.get_mut(func_idx) {
                        func.code = body.into_iter().collect();
                    } else {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::warn!(
                            func_idx = func_idx,
                            "Function index not found in functions vector"
                        );
                    }
                }
            },
            crate::sections::Section::Custom { name, data } => {
                let custom = wrt_format::section::CustomSection {
                    name: name.clone(),
                    data: data.into_iter().collect(),
                };
                module.custom_sections.push(custom);
            },
            crate::sections::Section::DataCount(_) => {
                // Data count is used for validation only
            },
        }
    }

    Ok(module)
}

/// Build a module from parsed sections (no_std version)
#[cfg(not(feature = "std"))]
fn build_module_from_sections(
    sections: crate::bounded_decoder_infra::BoundedSectionVec<crate::sections::Section>,
) -> Result<WrtModule<DecoderProvider>> {
    let provider = DecoderProvider::default();
    let mut module: WrtModule<DecoderProvider> = WrtModule::new();

    for section in sections {
        match section {
            crate::sections::Section::Type(types) => {
                for func_type in types {
                    // TODO: Fix type conversion from WrtFuncType to ValueType
                }
            },
            crate::sections::Section::Import(imports) => {
                for import in imports {
                    // TODO: Fix type conversion for imports
                }
            },
            crate::sections::Section::Function(func_indices) => {
                for type_idx in func_indices {
                    #[cfg(feature = "std")]
                    let func = wrt_format::module::Function {
                        type_idx,
                        locals: Vec::new(),
                        code: Vec::new(),
                    };
                    #[cfg(not(feature = "std"))]
                    let func = wrt_format::module::Function {
                        type_idx,
                        locals: alloc::vec::Vec::new(),
                        code: alloc::vec::Vec::new(),
                    };
                    let _ = module.functions.push(func);
                }
            },
            crate::sections::Section::Table(tables) => {
                for table in tables {
                    let _ = module.tables.push(table);
                }
            },
            crate::sections::Section::Memory(memories) => {
                for memory in memories {
                    let _ = module.memories.push(memory);
                }
            },
            crate::sections::Section::Global(globals) => {
                for global in globals {
                    // TODO: Fix type conversion for globals
                }
            },
            crate::sections::Section::Export(exports) => {
                for export in exports {
                    // TODO: Fix type conversion for exports
                }
            },
            crate::sections::Section::Start(start_idx) => {
                module.start = Some(start_idx);
            },
            crate::sections::Section::Element(elements) => {
                for element in elements {
                    // TODO: Fix type conversion for elements
                }
            },
            crate::sections::Section::Data(data_segments) => {
                for data in data_segments {
                    // TODO: Fix type conversion for data
                }
            },
            crate::sections::Section::Code(code_bodies) => {
                // Update function bodies
                // CRITICAL: The code section only contains code for local functions.
                // We need to skip imported functions when assigning code.
                let num_imports = module
                    .imports
                    .iter()
                    .filter(|import| matches!(import.desc, ImportDesc::Function(..)))
                    .count();

                for (idx, body) in code_bodies.into_iter().enumerate() {
                    // The actual function index is num_imports + idx
                    let func_idx = num_imports + idx;

                    if let Some(func) = module.functions.get_mut(func_idx) {
                        #[cfg(feature = "std")]
                        {
                            func.code = body.into_iter().collect();
                        }
                        #[cfg(not(feature = "std"))]
                        {
                            // For no_std, convert to Vec
                            func.code = body.into_iter().collect();
                        }
                    }
                }
            },
            crate::sections::Section::Custom { name, data } => {
                let custom = wrt_format::section::CustomSection {
                    name: alloc::string::String::from(name.as_str().unwrap_or("")),
                    data: data.into_iter().collect(),
                };
                let _ = module.custom_sections.push(custom);
            },
            crate::sections::Section::DataCount(_) => {
                // Data count is used for validation only
            },
            crate::sections::Section::Empty => {
                // Empty section, nothing to do
            },
        }
    }

    Ok(module)
}
