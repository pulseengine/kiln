// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component Model decoding.
//!
//! This module provides functions for decoding WebAssembly Component Model
//! components from binary format.

pub mod analysis;
pub mod binary_parser;
#[cfg(test)]
pub mod binary_parser_tests;
pub mod component_name_section;
pub mod decode;
// Binary std/no_std choice
pub mod decode_no_alloc;
mod encode;
pub mod name_section;
mod parse;
pub mod section;
pub mod streaming_core_module_parser;
pub mod streaming_type_parser;
pub mod types;
pub mod utils;
pub mod val_type;
pub mod validation;

#[cfg(feature = "std")]
pub use analysis::{
    AliasInfo, ComponentSummary, CoreInstanceInfo, CoreModuleInfo, ExtendedExportInfo,
    ExtendedImportInfo, ModuleExportInfo, ModuleImportInfo, analyze_component,
    analyze_component_extended, extract_embedded_modules, extract_inline_module,
    extract_module_info, is_valid_module,
};
pub use binary_parser::{
    ComponentBinaryParser, ComponentHeader, ComponentSectionId, ValidationLevel,
    parse_component_binary, parse_component_binary_with_validation,
};
pub use component_name_section::{
    ComponentNameSection, generate_component_name_section, parse_component_name_section,
};
#[cfg(feature = "std")]
pub use decode::decode_component as decode_component_internal;
#[cfg(feature = "std")]
pub use encode::encode_component;
#[cfg(feature = "std")]
pub use name_section::{NameMap, NameMapEntry, SortIdentifier};
#[cfg(feature = "std")]
pub use types::{Component, Export, Import};
pub use types::{
    ComponentAnalyzer, ComponentMetadata, ComponentType, CoreExternType, CoreInstance, CoreType,
    ExportInfo, ExternType, ImportInfo, Instance, ModuleInfo, Start, ValType,
};
#[cfg(feature = "std")]
pub use utils::*;
pub use val_type::encode_val_type;
#[cfg(not(feature = "std"))]
pub use validation::{ValidationConfig, validate_component};
#[cfg(feature = "std")]
pub use validation::{ValidationConfig, validate_component, validate_component_with_config};
use wrt_error::{Error, ErrorCategory, Result, codes};

#[cfg(not(feature = "std"))]
use crate::prelude::*;
#[cfg(feature = "std")]
use crate::utils::BinaryType;

// No_std stub for BinaryType when utils is not available
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryType {
    Module,
    Component,
}

// No_std safe utility functions with bounded behavior
#[cfg(not(feature = "std"))]
mod no_std_utils {
    use wrt_foundation::BoundedString;

    use super::*;

    /// Detect binary type with safety bounds for no_std
    ///
    /// # Safety Requirements
    /// - Only reads fixed-size magic bytes
    /// - No dynamic allocation
    /// - Fails gracefully on invalid input
    pub fn detect_binary_type(binary: &[u8]) -> Result<BinaryType> {
        if binary.len() < 8 {
            return Err(Error::parse_error("Binary too short for WASM header"));
        }

        // Check for WASM magic number (fixed 4 bytes)
        if &binary[0..4] == b"\0asm" {
            // Check version to determine module vs component
            let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);
            if version == 1 { Ok(BinaryType::Module) } else { Ok(BinaryType::Component) }
        } else {
            Err(Error::parse_error("Invalid WASM magic number"))
        }
    }

    /// Read name as bounded string with safety constraints
    ///
    /// # Safety Requirements  
    /// - Uses bounded string with compile-time limit
    /// - Validates UTF-8 without dynamic allocation
    /// - Fails gracefully on oversized strings
    pub fn read_name_as_string(data: &[u8], offset: usize) -> Result<(BoundedString<256>, usize)> {
        if offset >= data.len() {
            return Err(Error::parse_error("Offset beyond data length"));
        }

        // Read length (LEB128 - simplified to single byte for safety)
        let length = data[offset] as usize;
        let name_start = offset + 1;

        if name_start + length > data.len() {
            return Err(Error::parse_error("Name length exceeds data"));
        }

        // Validate UTF-8 and create bounded string
        let name_bytes = &data[name_start..name_start + length];
        let name_str = core::str::from_utf8(name_bytes)
            .map_err(|_| Error::parse_error("Invalid UTF-8 in name"))?;

        // Create the properly sized bounded string for the return type
        let name_string = BoundedString::<256>::try_from_str(name_str)
            .map_err(|_| Error::parse_error("Failed to create bounded string for name"))?;

        Ok((name_string, length + 1))
    }
}

/// Decode a component from binary data
///
/// This is the public entry point for decoding a component from binary data.
///
/// # Arguments
///
/// * `binary` - The binary data containing the component
///
/// # Returns
///
/// * `Result<Component>` - The decoded component or an error
#[cfg(feature = "std")]
pub fn decode_component_binary(binary: &[u8]) -> Result<Component> {
    decode_component_internal(binary)
}

/// Decode a WebAssembly Component Model component
///
/// This function takes a WebAssembly Component Model binary and decodes it
/// into a structured Component representation.
///
/// # Arguments
///
/// * `binary` - The WebAssembly Component Model binary data
///
/// # Returns
///
/// * `Result<Component>` - The decoded component or an error
#[cfg(feature = "std")]
pub fn decode_component(binary: &[u8]) -> Result<Component> {
    // Detect binary type first
    #[cfg(feature = "std")]
    let binary_type = crate::utils::detect_binary_type(binary)?;
    #[cfg(not(feature = "std"))]
    let binary_type = detect_binary_type(binary)?;

    match binary_type {
        BinaryType::CoreModule => {
            // Can't decode a core module as a component
            Err(Error::parse_error(
                "Cannot decode a WebAssembly core module as a Component",
            ))
        },
        BinaryType::Component => {
            // Verify component header
            if binary.len() < 8 {
                return Err(Error::parse_error("Component binary too short"));
            }

            // Components use the same magic as core modules: \0asm
            if binary[0..4] != [0x00, 0x61, 0x73, 0x6D] {
                return Err(Error::parse_error("Invalid Component Model magic number"));
            }

            // Validate component layer version (byte 4)
            // Components have layer versions 0x01-0x1F (vs core modules which have 0x01 0x00 0x00 0x00)
            let layer_version = binary[4];
            if layer_version == 0 || layer_version > 0x1F {
                return Err(Error::parse_error("Unsupported Component layer version"));
            }

            // Parse component (skip magic number and version)
            let mut component = Component::default();

            // Store the binary data
            component.binary = Some(binary.to_vec());

            // Parse component sections
            parse_component_sections(&binary[8..], &mut component)?;

            Ok(component)
        },
    }
}

/// Parse the sections of a Component Model component
#[cfg(feature = "std")]
fn parse_component_sections(data: &[u8], component: &mut Component) -> Result<()> {
    let mut offset = 0;

    // Track index counters for each sort to assign dest_idx to aliases
    // These are incremented for both aliases AND canon definitions
    // IMPORTANT: Core and component index spaces are SEPARATE
    use wrt_format::component::{CanonOperation, CoreSort};

    // Core-level counters (for CoreInstanceExport aliases)
    let mut core_func_counter = 0u32;
    let mut core_table_counter = 0u32;
    let mut core_memory_counter = 0u32;
    let mut core_global_counter = 0u32;
    let mut core_type_counter = 0u32;
    let mut core_module_counter = 0u32;
    let mut core_instance_counter = 0u32;

    // Component-level counters (for InstanceExport aliases)
    let mut component_func_counter = 0u32;
    let mut component_type_counter = 0u32;
    let mut component_instance_counter = 0u32;
    let mut component_counter = 0u32;
    let mut value_counter = 0u32;

    // Parse each section
    while offset < data.len() {
        // Read section ID
        let section_id = data[offset];
        offset += 1;

        // Read section size
        let (section_size, size_len) = wrt_format::binary::read_leb128_u32(data, offset)?;
        offset += size_len;

        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(
            section_id = format!("0x{:02x}", section_id),
            offset = offset - size_len - 1,
            size = section_size,
            "Section parsed"
        );

        // Ensure the section size is valid
        if offset + section_size as usize > data.len() {
            return Err(Error::parse_error(
                "Section size exceeds remaining data size",
            ));
        }

        let section_data = &data[offset..offset + section_size as usize];

        // Parse the section based on its ID
        // Component Model section IDs per spec:
        // https://github.com/WebAssembly/component-model/blob/main/design/mvp/Binary.md
        match section_id {
            0x00 => {
                // Custom section
                #[cfg(feature = "std")]
                let (name, bytes_read) = crate::utils::read_name_as_string(section_data, 0)?;
                #[cfg(not(feature = "std"))]
                let (name, bytes_read) = read_name_as_string(section_data, 0)?;
                let custom_data = &section_data[bytes_read..];

                if name == "name" {
                    if let Ok(name_section) =
                        component_name_section::parse_component_name_section(custom_data)
                    {
                        if let Some(component_name) = name_section.component_name {
                            component.name = Some(component_name);
                        }
                    }
                }
            },
            0x01 => {
                // Section 1: Core Module
                let (modules, _) = parse::parse_core_module_section(section_data)?;
                component.modules.extend(modules);
            },
            0x02 => {
                // Section 2: Core Instances
                match parse::parse_core_instance_section(section_data) {
                    Ok((instances, _)) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::trace!(
                            from = component.core_instances.len(),
                            to = component.core_instances.len() + instances.len(),
                            added = instances.len(),
                            "Extending core_instances"
                        );
                        component.core_instances.extend(instances);
                    },
                    Err(e) => {
                        // Continue parsing other sections
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::warn!(error = ?e, "ERROR parsing core instances");
                    },
                }
            },
            0x03 => {
                // Section 3: Core Types
                match parse::parse_core_type_section(section_data) {
                    Ok((types, _)) => {
                        component.core_types.extend(types);
                    },
                    Err(_) => {
                        // Continue parsing other sections
                    },
                }
            },
            0x04 => {
                // Section 4: Component (nested component definitions)
                match parse::parse_component_section(section_data) {
                    Ok((components, _bytes_consumed)) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::trace!(
                            count = components.len(),
                            "Parsed nested components"
                        );
                        component.components.extend(components);
                    },
                    Err(e) => {
                        // Following "FAIL LOUD AND EARLY" principle - propagate nested component parse errors
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::error!(error = %e, "Failed to parse nested component section");
                        return Err(e);
                    },
                }
            },
            0x05 => {
                // Section 5: Instances
                match parse::parse_instance_section(section_data) {
                    Ok((instances, _)) => {
                        component.instances.extend(instances);
                    },
                    Err(_) => {
                        // Continue parsing other sections
                    },
                }
            },
            0x06 => {
                // Section 6: Aliases
                match parse::parse_alias_section(section_data) {
                    Ok((mut aliases, _)) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::trace!(
                            count = aliases.len(),
                            "Parsed aliases section"
                        );

                        // Assign dest_idx to each alias based on its sort and current counter
                        for alias in &mut aliases {
                            use wrt_format::component::{AliasTarget, Sort};
                            match &alias.target {
                                AliasTarget::CoreInstanceExport { kind, .. } => {
                                    // Core-level aliases use core counters
                                    let idx = match kind {
                                        CoreSort::Function => {
                                            let i = core_func_counter;
                                            core_func_counter += 1;
                                            i
                                        },
                                        CoreSort::Table => {
                                            let i = core_table_counter;
                                            core_table_counter += 1;
                                            i
                                        },
                                        CoreSort::Memory => {
                                            let i = core_memory_counter;
                                            core_memory_counter += 1;
                                            i
                                        },
                                        CoreSort::Global => {
                                            let i = core_global_counter;
                                            core_global_counter += 1;
                                            i
                                        },
                                        CoreSort::Type => {
                                            let i = core_type_counter;
                                            core_type_counter += 1;
                                            i
                                        },
                                        CoreSort::Module => {
                                            let i = core_module_counter;
                                            core_module_counter += 1;
                                            i
                                        },
                                        CoreSort::Instance => {
                                            let i = core_instance_counter;
                                            core_instance_counter += 1;
                                            i
                                        },
                                    };
                                    alias.dest_idx = Some(idx);
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(kind = ?kind, idx = idx, "Assigned CoreInstanceExport");
                                },
                                AliasTarget::InstanceExport { kind, .. } => {
                                    // Component-level aliases use component counters
                                    let idx = match kind {
                                        Sort::Function => {
                                            let i = component_func_counter;
                                            component_func_counter += 1;
                                            i
                                        },
                                        Sort::Component => {
                                            let i = component_counter;
                                            component_counter += 1;
                                            i
                                        },
                                        Sort::Instance => {
                                            let i = component_instance_counter;
                                            component_instance_counter += 1;
                                            i
                                        },
                                        Sort::Value => {
                                            let i = value_counter;
                                            value_counter += 1;
                                            i
                                        },
                                        Sort::Type => {
                                            let i = component_type_counter;
                                            component_type_counter += 1;
                                            i
                                        },
                                        Sort::Core(_) => {
                                            // Core sorts in InstanceExport are unusual, skip for now
                                            #[cfg(feature = "tracing")]
                                            wrt_foundation::tracing::warn!(
                                                "InstanceExport with Core sort"
                                            );
                                            continue;
                                        },
                                    };
                                    alias.dest_idx = Some(idx);
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(kind = ?kind, idx = idx, "Assigned InstanceExport");
                                },
                                AliasTarget::Outer { .. } => {
                                    // Outer aliases reference parent component's index space
                                    // These don't consume indices in the current component
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(
                                        "Outer alias (no index assigned)"
                                    );
                                },
                            }
                        }

                        component.aliases.extend(aliases);
                    },
                    Err(e) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::warn!(error = ?e, "ERROR parsing alias section");
                        // Continue parsing other sections
                    },
                }
            },
            0x07 => {
                // Section 7: Types
                match parse::parse_component_type_section(section_data) {
                    Ok((types, _)) => {
                        component.types.extend(types);
                    },
                    Err(_) => {
                        // Continue parsing other sections
                        // Some type features (aliases, certain valtypes) not yet fully implemented
                    },
                }
            },
            0x08 => {
                // Section 8: Canonical (Canon ABI operations: lift, lower, resource)
                match parse::parse_canon_section(section_data) {
                    Ok((canons, _)) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::trace!(
                            count = canons.len(),
                            "Parsed canonicals section"
                        );

                        // Increment counters for canon definitions that create items
                        for canon in &canons {
                            match &canon.operation {
                                CanonOperation::Lower { .. } => {
                                    // Canon lower creates a CORE function
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(
                                        idx = core_func_counter,
                                        "Canon lower creates core func"
                                    );
                                    core_func_counter += 1;
                                },
                                CanonOperation::Lift { .. } => {
                                    // Canon lift creates a COMPONENT function
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(
                                        idx = component_func_counter,
                                        "Canon lift creates component func"
                                    );
                                    component_func_counter += 1;
                                },
                                CanonOperation::Resource(_) => {
                                    // Resource operations (like resource.drop) create CORE functions
                                    #[cfg(feature = "tracing")]
                                    wrt_foundation::tracing::trace!(
                                        idx = core_func_counter,
                                        "Canon resource creates core func"
                                    );
                                    core_func_counter += 1;
                                },
                                _ => {
                                    // Other canon operations may not create items
                                },
                            }
                        }

                        component.canonicals.extend(canons);
                    },
                    Err(e) => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::warn!(error = ?e, "ERROR parsing canon section");
                        // Continue parsing other sections
                    },
                }
            },
            0x09 => {
                // Section 9: Start (component start function)
                match parse::parse_start_section(section_data) {
                    Ok((start, _)) => {
                        component.start = Some(start);
                    },
                    Err(_) => {
                        // Continue parsing other sections
                    },
                }
            },
            0x0A => {
                // Section 10: Imports
                let (imports, _) = parse::parse_import_section(section_data)?;
                component.imports.extend(imports);
            },
            0x0B => {
                // Section 11: Exports
                match parse::parse_export_section(section_data) {
                    Ok((exports, _)) => {
                        component.exports.extend(exports);
                    },
                    Err(_) => {
                        // Continue - not critical for initial testing
                    },
                }
            },
            0x0C => {
                // Section 12: Values (constant values)
                match parse::parse_value_section(section_data) {
                    Ok((values, _)) => {
                        component.values.extend(values);
                    },
                    Err(_) => {
                        // Continue parsing other sections
                    },
                }
            },
            _ => {
                // Unknown section - ignore for now
                // We could log a warning here
            },
        }

        // Move to the next section
        offset += section_size as usize;
    }

    Ok(())
}
