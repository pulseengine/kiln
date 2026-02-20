// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component Model name section
//!
//! This module provides utilities for encoding and decoding custom name
//! sections in WebAssembly Component Model binaries.

use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(feature = "std")]
use wrt_format::binary::with_alloc::{read_leb128_u32, read_string};
#[cfg(feature = "std")]
use wrt_format::component::Sort;
#[cfg(feature = "std")]
use wrt_format::{write_leb128_u32, write_string};
#[cfg(not(feature = "std"))]
use wrt_foundation::bounded::BoundedVec;
#[cfg(not(feature = "std"))]
use wrt_foundation::safe_memory::NoStdProvider;

use crate::prelude::*;

/// Component name section
#[cfg(feature = "std")]
#[derive(Default, Debug, Clone)]
pub struct ComponentNameSection {
    /// Component name
    pub component_name: Option<String>,
    /// Names for each sort (function, instance, etc.)
    pub sort_names: Vec<(Sort, Vec<(u32, String)>)>,
    /// Import names
    pub import_names: Vec<(u32, String)>,
    /// Export names
    pub export_names: Vec<(u32, String)>,
    /// Canonical function names
    pub canonical_names: Vec<(u32, String)>,
    /// Type names
    pub type_names: Vec<(u32, String)>,
}

/// Component name section (no_std version - simplified)
#[cfg(not(feature = "std"))]
#[derive(Default, Debug, Clone)]
pub struct ComponentNameSection {
    /// Component name (simplified for no_std)
    pub component_name: Option<&'static str>,
    /// Simplified names for no_std - only sort IDs
    pub sort_names: (),
    /// Import names (disabled in no_std)
    pub import_names: (),
    /// Export names (disabled in no_std)
    pub export_names: (),
    /// Canonical function names (disabled in no_std)
    pub canonical_names: (),
    /// Type names (disabled in no_std)
    pub type_names: (),
}

/// Name subsection IDs
mod subsection {
    /// Component Name subsection ID
    pub const COMPONENT_NAME: u8 = 0;
    /// Sort Names subsection ID (for functions, instances, etc.)
    pub const SORT_NAMES: u8 = 1;
    /// Import Names subsection ID
    pub const IMPORT_NAMES: u8 = 2;
    /// Export Names subsection ID
    pub const EXPORT_NAMES: u8 = 3;
    /// Canonical Names subsection ID
    pub const CANONICAL_NAMES: u8 = 4;
    /// Type Names subsection ID
    pub const TYPE_NAMES: u8 = 5;
}

/// Sort Type IDs for name section
mod sort_type {
    /// Function sort ID
    pub const FUNCTION: u8 = 0;
    /// Value sort ID
    pub const VALUE: u8 = 1;
    /// Type sort ID
    pub const TYPE: u8 = 2;
    /// Component sort ID
    pub const COMPONENT: u8 = 3;
    /// Instance sort ID
    pub const INSTANCE: u8 = 4;
}

/// Generate binary data for a component name section
#[cfg(feature = "std")]
pub fn generate_component_name_section(section: &ComponentNameSection) -> Result<Vec<u8>> {
    let mut result = Vec::new();

    // Write component name if present
    if let Some(name) = &section.component_name {
        result.push(subsection::COMPONENT_NAME);
        let name_string_bytes = write_string(name);
        let mut name_data = Vec::new();
        name_data.extend_from_slice(&name_string_bytes);
        let len_bytes = write_leb128_u32(name_data.len() as u32);
        result.extend_from_slice(&len_bytes);
        result.extend_from_slice(&name_data);
    }

    // Continue with std implementation...
    // Write sort names
    if !section.sort_names.is_empty() {
        result.push(subsection::SORT_NAMES);
        let mut sort_data = Vec::new();

        // Write number of sorts
        sort_data.extend_from_slice(&write_leb128_u32(section.sort_names.len() as u32));

        for (sort, names) in &section.sort_names {
            // Write sort ID
            let sort_id = match sort {
                Sort::Function => sort_type::FUNCTION,
                Sort::Value => sort_type::VALUE,
                Sort::Type => sort_type::TYPE,
                Sort::Component => sort_type::COMPONENT,
                Sort::Instance => sort_type::INSTANCE,
                _ => {
                    // Skip unknown sorts
                    continue;
                },
            };
            sort_data.push(sort_id);

            // Write number of names
            sort_data.extend_from_slice(&write_leb128_u32(names.len() as u32));

            // Write each name
            for (idx, name) in names {
                sort_data.extend_from_slice(&write_leb128_u32(*idx));
                sort_data.extend_from_slice(&write_string(name));
            }
        }

        // Write the size of the sort data
        result.extend_from_slice(&write_leb128_u32(sort_data.len() as u32));
        result.extend_from_slice(&sort_data);
    }

    // Write import names
    if !section.import_names.is_empty() {
        write_name_map(&mut result, subsection::IMPORT_NAMES, &section.import_names);
    }

    // Write export names
    if !section.export_names.is_empty() {
        write_name_map(&mut result, subsection::EXPORT_NAMES, &section.export_names);
    }

    // Write canonical names
    if !section.canonical_names.is_empty() {
        write_name_map(
            &mut result,
            subsection::CANONICAL_NAMES,
            &section.canonical_names,
        );
    }

    // Write type names
    if !section.type_names.is_empty() {
        write_name_map(&mut result, subsection::TYPE_NAMES, &section.type_names);
    }

    Ok(result)
}

/// Generate binary data for a component name section (no_std version)
///
/// # Safety Requirements
/// - Uses bounded allocation with compile-time limits
/// - Fails gracefully when limits are exceeded
/// - No heap allocation or dynamic memory
#[cfg(not(feature = "std"))]
pub fn generate_component_name_section(
    section: &ComponentNameSection,
) -> Result<BoundedVec<u8, 1024, NoStdProvider<2048>>> {
    let provider = wrt_foundation::safe_managed_alloc!(
        2048,
        wrt_foundation::budget_aware_provider::CrateId::Decoder
    )?;
    let mut result = BoundedVec::new(provider).map_err(|_| {
        wrt_error::Error::platform_memory_allocation_failed("Failed to create result buffer")
    })?;

    // Write component name if present (simplified for no_std)
    if let Some(name) = &section.component_name {
        result.push(subsection::COMPONENT_NAME).map_err(|_| {
            wrt_error::Error::platform_memory_allocation_failed("Name section buffer overflow")
        })?;

        // In no_std mode, use simplified string writing
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u32;

        // Simple LEB128 encoding for length (simplified for no_std)
        let mut length_data = [0u8; 5]; // Max 5 bytes for u32 LEB128
        let mut len_bytes_count = 0;
        let mut value = name_len;
        while value >= 0x80 {
            length_data[len_bytes_count] = (value as u8) | 0x80;
            len_bytes_count += 1;
            value >>= 7;
        }
        length_data[len_bytes_count] = value as u8;
        len_bytes_count += 1;

        // Write length data
        for i in 0..len_bytes_count {
            result.push(length_data[i]).map_err(|_| {
                wrt_error::Error::platform_memory_allocation_failed("Name section buffer overflow")
            })?;
        }

        // Write name data
        for byte in name_bytes.iter() {
            result.push(*byte).map_err(|_| {
                wrt_error::Error::platform_memory_allocation_failed("Name section buffer overflow")
            })?;
        }
    }

    // Note: Complex sort names and other sections simplified for no_std safety
    // Only basic component name supported in no_std mode

    Ok(result)
}

/// Parse a component name section from binary data
#[cfg(feature = "std")]
pub fn parse_component_name_section(data: &[u8]) -> Result<ComponentNameSection> {
    let mut result = ComponentNameSection::default();
    let mut pos = 0;

    while pos < data.len() {
        // Read subsection ID
        let subsection_id = data[pos];
        pos += 1;

        // Read subsection size
        let (subsection_size, bytes_read) = read_leb128_u32(&data[pos..], 0)?;
        pos += bytes_read;

        let subsection_end = pos + subsection_size as usize;
        if subsection_end > data.len() {
            return Err(Error::parse_error(
                "Subsection extends beyond end of name section data",
            ));
        }

        match subsection_id {
            subsection::COMPONENT_NAME => {
                // Parse component name
                let (name, bytes_read) = read_string(&data[pos..subsection_end], 0)?;
                if bytes_read != subsection_size as usize {
                    return Err(Error::parse_error("Invalid component name format"));
                }
                result.component_name = Some(name);
            },
            subsection::SORT_NAMES => {
                // Parse sort names
                let mut subsection_pos = pos;

                // Read number of sorts
                let (num_sorts, bytes_read) = read_leb128_u32(&data[subsection_pos..], 0)?;
                subsection_pos += bytes_read;

                for _ in 0..num_sorts {
                    // Read sort ID
                    if subsection_pos >= subsection_end {
                        return Err(Error::parse_error(
                            "Unexpected end of sort names subsection",
                        ));
                    }

                    let sort_id = data[subsection_pos];
                    subsection_pos += 1;

                    // Map sort ID to Sort enum
                    #[cfg(feature = "std")]
                    let sort = match sort_id {
                        sort_type::FUNCTION => Sort::Function,
                        sort_type::VALUE => Sort::Value,
                        sort_type::TYPE => Sort::Type,
                        sort_type::COMPONENT => Sort::Component,
                        sort_type::INSTANCE => Sort::Instance,
                        _ => {
                            return Err(Error::parse_error("Unknown sort ID"));
                        },
                    };

                    #[cfg(not(feature = "std"))]
                    let sort = sort_id; // Just use the raw sort ID for no_std

                    // Read number of names
                    let (num_names, bytes_read) = read_leb128_u32(&data[subsection_pos..], 0)?;
                    subsection_pos += bytes_read;

                    let mut names = Vec::new();

                    // Read each name
                    for _ in 0..num_names {
                        // Read index
                        let (idx, bytes_read) = read_leb128_u32(&data[subsection_pos..], 0)?;
                        subsection_pos += bytes_read;

                        // Read name
                        let (name, bytes_read) = read_string(&data[subsection_pos..], 0)?;
                        subsection_pos += bytes_read;

                        names.push((idx, name));
                    }

                    result.sort_names.push((sort, names));
                }
            },
            subsection::IMPORT_NAMES => {
                result.import_names = read_name_map(&data[pos..subsection_end])?;
            },
            subsection::EXPORT_NAMES => {
                result.export_names = read_name_map(&data[pos..subsection_end])?;
            },
            subsection::CANONICAL_NAMES => {
                result.canonical_names = read_name_map(&data[pos..subsection_end])?;
            },
            subsection::TYPE_NAMES => {
                result.type_names = read_name_map(&data[pos..subsection_end])?;
            },
            _ => {
                // Skip unknown subsections
            },
        }

        pos = subsection_end;
    }

    Ok(result)
}

/// Parse a component name section from binary data (no_std version -
/// simplified)
#[cfg(not(feature = "std"))]
pub fn parse_component_name_section(_data: &[u8]) -> Result<ComponentNameSection> {
    // Simplified parsing for no_std - only basic functionality
    Ok(ComponentNameSection::default())
}

/// Helper function to write a name map (used for imports, exports, etc.)
#[cfg(feature = "std")]
fn write_name_map(result: &mut Vec<u8>, subsection_id: u8, names: &[(u32, String)]) {
    result.push(subsection_id);
    let mut map_data = Vec::new();

    // Write number of entries
    map_data.extend_from_slice(&write_leb128_u32(names.len() as u32));

    // Write each name
    for (idx, name) in names {
        map_data.extend_from_slice(&write_leb128_u32(*idx));
        map_data.extend_from_slice(&write_string(name));
    }

    // Write the size of the map data
    result.extend_from_slice(&write_leb128_u32(map_data.len() as u32));
    result.extend_from_slice(&map_data);
}

/// Helper function to read a name map
#[cfg(feature = "std")]
fn read_name_map(data: &[u8]) -> Result<Vec<(u32, String)>> {
    let mut result = Vec::new();
    let mut pos = 0;

    // Read number of entries
    let (num_entries, bytes_read) = read_leb128_u32(&data[pos..], 0)?;
    pos += bytes_read;

    // Read each entry
    for _ in 0..num_entries {
        // Read index
        let (idx, bytes_read) = read_leb128_u32(&data[pos..], 0)?;
        pos += bytes_read;

        // Read name
        let (name, bytes_read) = read_string(&data[pos..], 0)?;
        pos += bytes_read;

        result.push((idx, name));
    }

    Ok(result)
}
