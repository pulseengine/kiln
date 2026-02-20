//! Resource limits loader for WebAssembly binaries
//!
//! This module provides functionality to extract resource limits from
//! WebAssembly binaries' custom sections and convert them to execution
//! configurations.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::format;

#[cfg(feature = "decoder")]
use wrt_decoder::resource_limits_section::{RESOURCE_LIMITS_SECTION_NAME, ResourceLimitsSection};

// Placeholder types when decoder is not available
#[cfg(not(feature = "decoder"))]
pub struct ResourceLimitsSection {
    pub qualification_hash: Option<[u8; 32]>,
    pub max_fuel_per_step: Option<u64>,
    pub max_memory_usage: Option<u64>,
    pub max_call_depth: Option<u32>,
    pub max_instructions_per_step: Option<u32>,
    pub max_execution_slice_ms: Option<u32>,
}
#[cfg(not(feature = "decoder"))]
impl ResourceLimitsSection {
    pub fn decode(_data: &[u8]) -> Result<Self> {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "ResourceLimitsSection::decode requires decoder feature",
        ))
    }
    pub fn qualified_asil_level(&self) -> Option<&str> {
        None
    }
}
#[cfg(not(feature = "decoder"))]
pub const RESOURCE_LIMITS_SECTION_NAME: &str = "resource_limits";
use wrt_error::{Error, ErrorCategory, codes};
use wrt_foundation::NoStdProvider;

use crate::{
    async_::fuel_async_executor::{ASILExecutionConfig, ASILExecutionMode, ExecutionLimitsConfig},
    prelude::*,
};

/// Extract resource limits from a WebAssembly binary
///
/// This function searches for the "wrt.resource_limits" custom section
/// and decodes it into an ASILExecutionConfig.
pub fn extract_resource_limits_from_binary(
    wasm_bytes: &[u8],
    default_asil_mode: ASILExecutionMode,
) -> Result<Option<ASILExecutionConfig>> {
    // Parse WebAssembly custom sections
    let custom_section_data = find_custom_section(wasm_bytes, RESOURCE_LIMITS_SECTION_NAME)?;

    if let Some(section_data) = custom_section_data {
        // Decode resource limits section
        let limits_section = ResourceLimitsSection::decode(&section_data)?;

        // Convert to ASILExecutionConfig
        let config = convert_to_asil_config(&limits_section, default_asil_mode)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

/// Find a custom section in WebAssembly binary
///
/// This is a simple implementation that looks for custom sections (type 0)
/// with the specified name.
fn find_custom_section(wasm_bytes: &[u8], section_name: &str) -> Result<Option<Vec<u8>>> {
    if wasm_bytes.len() < 8 {
        return Err(Error::parse_error("WebAssembly binary too small"));
    }

    // Verify magic number
    if &wasm_bytes[0..4] != b"\0asm" {
        return Err(Error::parse_error("Invalid WebAssembly magic number"));
    }

    // Verify version
    let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
    if version != 1 {
        return Err(Error::parse_error("Unsupported WebAssembly version"));
    }

    let mut offset = 8;

    while offset < wasm_bytes.len() {
        // Read section type
        if offset >= wasm_bytes.len() {
            break;
        }
        let section_type = wasm_bytes[offset];
        offset += 1;

        // Read section size (LEB128)
        let (section_size, new_offset) = read_leb128_u32(&wasm_bytes[offset..])?;
        offset += new_offset;

        let section_end = offset + section_size as usize;
        if section_end > wasm_bytes.len() {
            return Err(Error::parse_error("Section extends beyond binary"));
        }

        // Check if this is a custom section (type 0)
        if section_type == 0 {
            // Read name length and name
            let (name_len, name_offset) = read_leb128_u32(&wasm_bytes[offset..])?;
            let name_start = offset + name_offset;
            let name_end = name_start + name_len as usize;

            if name_end > section_end {
                return Err(Error::parse_error(
                    "Custom section name extends beyond section",
                ));
            }

            let name = core::str::from_utf8(&wasm_bytes[name_start..name_end])
                .map_err(|_| Error::parse_error("Invalid UTF-8 in section name"))?;

            if name == section_name {
                // Found the section, extract data
                let data_start = name_end;
                let data = wasm_bytes[data_start..section_end].to_vec();
                return Ok(Some(data));
            }
        }

        offset = section_end;
    }

    Ok(None)
}

/// Read LEB128 encoded u32
fn read_leb128_u32(bytes: &[u8]) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if offset >= bytes.len() {
            return Err(Error::parse_error("Unexpected end of LEB128"));
        }

        let byte = bytes[offset];
        offset += 1;

        if shift >= 32 {
            return Err(Error::parse_error("LEB128 too large for u32"));
        }

        result |= ((byte & 0x7F) as u32) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
    }

    Ok((result, offset))
}

/// Convert ResourceLimitsSection to ASILExecutionConfig
fn convert_to_asil_config(
    limits: &ResourceLimitsSection,
    default_mode: ASILExecutionMode,
) -> Result<ASILExecutionConfig> {
    // Determine ASIL mode from qualification info or use default
    let asil_mode = if let Some(asil_level) = limits.qualified_asil_level() {
        match asil_level {
            "ASIL-D" => ASILExecutionMode::AsilD,
            "ASIL-C" => ASILExecutionMode::AsilC,
            "ASIL-B" => ASILExecutionMode::AsilB,
            "ASIL-A" => ASILExecutionMode::AsilA,
            "QM" => ASILExecutionMode::QM,
            _ => default_mode,
        }
    } else {
        default_mode
    };

    // Extract binary hash if available
    let binary_hash = limits.qualification_hash;

    // Create execution limits config
    let defaults = ExecutionLimitsConfig::default_for_asil(asil_mode);
    let execution_limits = ExecutionLimitsConfig {
        max_fuel_per_step: limits.max_fuel_per_step.or(defaults.max_fuel_per_step),
        max_memory_usage: limits.max_memory_usage.or(defaults.max_memory_usage),
        max_call_depth: limits.max_call_depth.or(defaults.max_call_depth),
        max_stack_depth: limits.max_call_depth.or(defaults.max_stack_depth),
        max_instructions_per_step: limits
            .max_instructions_per_step
            .or(defaults.max_instructions_per_step),
        max_execution_slice_ms: limits.max_execution_slice_ms.or(defaults.max_execution_slice_ms),
        max_async_operations: defaults.max_async_operations,
        max_waitables_per_task: defaults.max_waitables_per_task,
        max_concurrent_tasks: defaults.max_concurrent_tasks,
        max_yields_per_step: defaults.max_yields_per_step,
        limit_source: if binary_hash.is_some() {
            use crate::async_::fuel_async_executor::LimitSource;
            LimitSource::BinaryMetadata {
                section_name: String::from("wrt.resource_limits"),
                verified_hash: binary_hash.unwrap(),
            }
        } else {
            defaults.limit_source
        },
    };

    // Create ASIL execution config
    let config = ASILExecutionConfig {
        mode: asil_mode,
        limits: execution_limits,
        qualified_for_binary: binary_hash.map(|h| format!("{:?}", h)),
    };

    // Validate the configuration
    config.validate()?;

    Ok(config)
}
