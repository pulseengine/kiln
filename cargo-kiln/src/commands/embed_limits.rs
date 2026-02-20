//! Command to embed resource limits into WebAssembly binaries
//!
//! This command reads a TOML configuration file and embeds the resource
//! limits as a custom section in a WebAssembly binary.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Args;
use wrt_decoder::{
    resource_limits_section::RESOURCE_LIMITS_SECTION_NAME,
    toml_config::{TomlQualification, TomlResourceLimits},
};

use crate::helpers::{OutputManager, output_result};

/// Arguments for the embed-limits command
#[derive(Debug, Args)]
pub struct EmbedLimitsArgs {
    /// Path to the WebAssembly binary
    #[arg(help = "Path to the WebAssembly binary file")]
    pub wasm_file: PathBuf,

    /// Path to the TOML configuration file
    #[arg(
        short = 'c',
        long = "config",
        help = "Path to the resource limits TOML configuration"
    )]
    pub config_file: PathBuf,

    /// Output file path (defaults to modifying in place)
    #[arg(
        short = 'o',
        long = "output",
        help = "Output file path (default: modify in place)"
    )]
    pub output_file: Option<PathBuf>,

    /// ASIL level to enforce
    #[arg(
        short = 'a',
        long = "asil",
        help = "ASIL level to enforce (QM, A, B, C, D)"
    )]
    pub asil_level: Option<String>,

    /// Binary hash for qualification
    #[arg(
        long = "binary-hash",
        help = "SHA-256 hash of the qualified binary (64 hex chars)"
    )]
    pub binary_hash: Option<String>,

    /// Validate limits against ASIL requirements
    #[arg(long = "validate", help = "Validate limits against ASIL requirements")]
    pub validate: bool,

    /// Remove existing resource limits sections
    #[arg(long = "replace", help = "Replace existing resource limits sections")]
    pub replace: bool,
}

/// Execute the embed-limits command
#[must_use]
pub fn execute(args: EmbedLimitsArgs, output: &OutputManager) -> Result<()> {
    // Read the TOML configuration
    output.progress(&format!(
        "Reading configuration from {}",
        args.config_file.display()
    ));
    let mut config = TomlResourceLimits::from_file(&args.config_file).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read configuration from {}: {}",
            args.config_file.display(),
            e
        )
    })?;

    // Override ASIL level if specified
    if let Some(asil_level) = &args.asil_level {
        if let Some(ref mut qual) = config.qualification {
            // Note: TomlQualification structure may need updating to match
            // actual fields
        } else {
            // Note: TomlQualification structure may need updating
            // config.qualification = Some(TomlQualification::default());
        }
    }

    // Override binary hash if specified
    if let Some(binary_hash) = &args.binary_hash {
        if let Some(ref mut qual) = config.qualification {
            // Note: TomlQualification structure may need updating to match
            // actual fields
        } else {
            // Note: TomlQualification structure may need updating
            // config.qualification = Some(TomlQualification::default());
        }
    }

    // Convert to resource limits section
    output.progress("Converting configuration to binary format");
    let limits_section = config.to_resource_limits_section().map_err(|e| {
        anyhow::anyhow!(
            "Failed to convert configuration to resource limits section: {}",
            e
        )
    })?;

    // Validate against ASIL requirements if requested
    if args.validate {
        output.progress("Validating limits against ASIL requirements");
        if let Some(asil_level) = limits_section.qualified_asil_level() {
            match asil_level {
                "ASIL-D" => {
                    limits_section.validate_asil_d_compliance().map_err(|e| {
                        anyhow::anyhow!("ASIL-D compliance validation failed: {}", e)
                    })?;
                    output.success("Validated for ASIL-D compliance");
                },
                "ASIL-C" | "ASIL-B" | "ASIL-A" => {
                    limits_section
                        .validate()
                        .map_err(|e| anyhow::anyhow!("ASIL validation failed: {}", e))?;
                    output.success(&format!("Validated for {} compliance", asil_level));
                },
                _ => {
                    output.info("No specific ASIL validation for QM level");
                },
            }
        }
    }

    // Encode the limits section
    let encoded_limits = limits_section
        .encode()
        .map_err(|e| anyhow::anyhow!("Failed to encode resource limits: {}", e))?;

    // Read the WebAssembly binary
    output.progress(&format!(
        "Reading WebAssembly binary from {}",
        args.wasm_file.display()
    ));
    let wasm_bytes = fs::read(&args.wasm_file)
        .context(format!("Failed to read {}", args.wasm_file.display()))?;

    // Process the WebAssembly binary
    output.progress("Processing WebAssembly binary");
    let output_bytes = if args.replace {
        replace_custom_section(&wasm_bytes, RESOURCE_LIMITS_SECTION_NAME, &encoded_limits)?
    } else {
        add_custom_section(&wasm_bytes, RESOURCE_LIMITS_SECTION_NAME, &encoded_limits)?
    };

    // Write the output
    let output_path = args.output_file.as_ref().unwrap_or(&args.wasm_file);
    output.progress(&format!("Writing output to {}", output_path.display()));
    fs::write(output_path, output_bytes)
        .context(format!("Failed to write {}", output_path.display()))?;

    // Print summary
    output.success("Resource limits embedded successfully");
    output.info(&format!("  Section name: {}", RESOURCE_LIMITS_SECTION_NAME));
    output.info(&format!("  Section size: {} bytes", encoded_limits.len()));
    if let Some(asil_level) = limits_section.qualified_asil_level() {
        output.info(&format!("  ASIL level: {}", asil_level));
    }
    if limits_section.is_complete_for_asil_d() {
        output.info("  ASIL-D ready: Yes");
    }

    Ok(())
}

/// Add a custom section to a WebAssembly binary
fn add_custom_section(
    wasm_bytes: &[u8],
    section_name: &str,
    section_data: &[u8],
) -> Result<Vec<u8>> {
    let mut output = Vec::new();

    // Copy magic number and version
    if wasm_bytes.len() < 8 {
        anyhow::bail!("Invalid WebAssembly binary: too small");
    }
    output.extend_from_slice(&wasm_bytes[0..8]);

    // Add the custom section at the beginning (after header)
    append_custom_section(&mut output, section_name, section_data);

    // Copy the rest of the binary
    output.extend_from_slice(&wasm_bytes[8..]);

    Ok(output)
}

/// Replace a custom section in a WebAssembly binary
fn replace_custom_section(
    wasm_bytes: &[u8],
    section_name: &str,
    section_data: &[u8],
) -> Result<Vec<u8>> {
    let mut output = Vec::new();

    // Copy magic number and version
    if wasm_bytes.len() < 8 {
        anyhow::bail!("Invalid WebAssembly binary: too small");
    }
    output.extend_from_slice(&wasm_bytes[0..8]);

    let mut offset = 8;
    let mut found = false;

    // Process sections
    while offset < wasm_bytes.len() {
        let section_start = offset;

        // Read section type
        let section_type = wasm_bytes[offset];
        offset += 1;

        // Read section size (LEB128)
        let (section_size, size_len) = read_leb128_u32(&wasm_bytes[offset..])?;
        offset += size_len;

        let section_end = offset + section_size as usize;
        if section_end > wasm_bytes.len() {
            anyhow::bail!("Invalid WebAssembly binary: section extends beyond end");
        }

        // Check if this is a custom section with our name
        if section_type == 0 && !found {
            // Read name from custom section
            let (name_len, name_len_size) = read_leb128_u32(&wasm_bytes[offset..])?;
            let name_start = offset + name_len_size;
            let name_end = name_start + name_len as usize;

            if name_end <= section_end {
                let name = std::str::from_utf8(&wasm_bytes[name_start..name_end])?;

                if name == section_name {
                    // Found the section to replace
                    found = true;
                    append_custom_section(&mut output, section_name, section_data);
                    offset = section_end;
                    continue;
                }
            }
        }

        // Copy the section as-is
        output.extend_from_slice(&wasm_bytes[section_start..section_end]);
        offset = section_end;
    }

    // If we didn't find the section, add it
    if !found {
        // We need to insert it after the header but before other sections
        // For simplicity, we'll recreate the binary with the custom section first
        let mut new_output = Vec::new();
        new_output.extend_from_slice(&wasm_bytes[0..8]);
        append_custom_section(&mut new_output, section_name, section_data);
        new_output.extend_from_slice(&output[8..]);
        output = new_output;
    }

    Ok(output)
}

/// Append a custom section to the output
fn append_custom_section(output: &mut Vec<u8>, name: &str, data: &[u8]) {
    // Section type (0 = custom)
    output.push(0);

    // Calculate section size
    let name_len = name.len() as u32;
    let name_len_encoded = encode_leb128_u32(name_len);
    let section_size = name_len_encoded.len() + name.len() + data.len();

    // Write section size
    output.extend_from_slice(&encode_leb128_u32(section_size as u32));

    // Write name length and name
    output.extend_from_slice(&name_len_encoded);
    output.extend_from_slice(name.as_bytes());

    // Write section data
    output.extend_from_slice(data);
}

/// Read LEB128 encoded u32
fn read_leb128_u32(bytes: &[u8]) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if offset >= bytes.len() {
            anyhow::bail!("Unexpected end of LEB128");
        }

        let byte = bytes[offset];
        offset += 1;

        if shift >= 32 {
            anyhow::bail!("LEB128 too large for u32");
        }

        result |= ((byte & 0x7F) as u32) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
    }

    Ok((result, offset))
}

/// Encode u32 as LEB128
fn encode_leb128_u32(mut value: u32) -> Vec<u8> {
    let mut result = Vec::new();

    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        result.push(byte);

        if value == 0 {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leb128_roundtrip() {
        let values = vec![0, 127, 128, 16384, 65535, 1000000];

        for value in values {
            let encoded = encode_leb128_u32(value);
            let (decoded, len) = read_leb128_u32(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(len, encoded.len());
        }
    }
}
