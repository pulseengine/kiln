//! TOML configuration parser for resource limits
//!
//! This module provides TOML-based configuration for resource limits,
//! primarily used by tooling (cargo-wrt) which can use std features.
//! The parsed configuration is then converted to ASIL-D compatible
//! binary format for embedding in WebAssembly modules.

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::path::Path;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use wrt_error::{Error, ErrorCategory, codes};
use wrt_foundation::{BoundedString, CrateId, NoStdProvider, safe_managed_alloc};

use crate::resource_limits_section::{
    MAX_ASIL_STRING_LEN, MAX_CUSTOM_LIMITS_PER_TYPE, MAX_RESOURCE_NAME_LEN, MAX_RESOURCE_TYPES,
    ResourceLimitsSection, ResourceTypeLimit,
};

/// TOML configuration structure for resource limits
/// This is only available with std feature for tooling
#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlResourceLimits {
    /// Version of the configuration format
    #[serde(default = "default_version")]
    pub version: u32,

    /// Execution limits
    pub execution: Option<TomlExecutionLimits>,

    /// Resource type specific limits
    #[serde(default)]
    pub resources: HashMap<String, TomlResourceTypeLimit>,

    /// Qualification information
    pub qualification: Option<TomlQualification>,
}

#[cfg(feature = "std")]
fn default_version() -> u32 {
    1
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlExecutionLimits {
    /// Maximum fuel per execution step
    pub max_fuel_per_step: Option<u64>,

    /// Maximum memory usage in bytes (supports suffixes: K, M, G)
    pub max_memory_usage: Option<String>,

    /// Maximum call stack depth
    pub max_call_depth: Option<u32>,

    /// Maximum instructions per step
    pub max_instructions_per_step: Option<u32>,

    /// Maximum execution slice in milliseconds
    pub max_execution_slice_ms: Option<u32>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlResourceTypeLimit {
    /// Maximum number of handles
    pub max_handles: Option<u32>,

    /// Maximum memory for this resource type (supports suffixes)
    pub max_memory: Option<String>,

    /// Maximum operations per second
    pub max_operations_per_second: Option<u32>,

    /// Custom limits specific to this resource type
    #[serde(default)]
    pub custom: HashMap<String, u64>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlQualification {
    /// Binary hash (as hex string)
    pub binary_hash: Option<String>,

    /// ASIL level this configuration is qualified for
    pub asil_level: Option<String>,
}

#[cfg(feature = "std")]
impl TomlResourceLimits {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let contents = fs::read_to_string(path)
            .map_err(|_| Error::runtime_execution_error("Failed to read TOML file"))?;

        Self::from_str(&contents)
    }

    /// Parse configuration from a TOML string
    pub fn from_str(toml_str: &str) -> Result<Self, Error> {
        toml::from_str(toml_str).map_err(|_| Error::parse_error("Invalid TOML format"))
    }

    /// Convert to ASIL-D compatible ResourceLimitsSection
    pub fn to_resource_limits_section(
        &self,
    ) -> Result<ResourceLimitsSection<NoStdProvider<4096>>, Error> {
        // Allocate memory for the conversion
        let provider = safe_managed_alloc!(4096, CrateId::Decoder)?;

        let mut section = ResourceLimitsSection::new(provider.clone())?;
        section.version = self.version;

        // Set execution limits
        if let Some(exec) = &self.execution {
            section.max_fuel_per_step = exec.max_fuel_per_step;
            section.max_memory_usage =
                exec.max_memory_usage.as_ref().map(|s| parse_memory_size(s)).transpose()?;
            section.max_call_depth = exec.max_call_depth;
            section.max_instructions_per_step = exec.max_instructions_per_step;
            section.max_execution_slice_ms = exec.max_execution_slice_ms;
        }

        // Validate resource count
        if self.resources.len() > MAX_RESOURCE_TYPES {
            return Err(Error::runtime_execution_error("Too many resource types"));
        }

        // Convert resource type limits
        for (name, toml_limit) in &self.resources {
            if name.len() > MAX_RESOURCE_NAME_LEN {
                return Err(Error::parse_error("Resource name too long"));
            }

            let mut limit = ResourceTypeLimit::new(provider.clone())?;
            limit.max_handles = toml_limit.max_handles;
            limit.max_memory =
                toml_limit.max_memory.as_ref().map(|s| parse_memory_size(s)).transpose()?;
            limit.max_operations_per_second = toml_limit.max_operations_per_second;

            // Validate custom limits count
            if toml_limit.custom.len() > MAX_CUSTOM_LIMITS_PER_TYPE {
                return Err(Error::runtime_execution_error("Too many custom limits"));
            }

            // Add custom limits
            for (custom_name, value) in &toml_limit.custom {
                if custom_name.len() > MAX_RESOURCE_NAME_LEN {
                    return Err(Error::parse_error("Custom limit name too long"));
                }
                limit = limit.with_custom_limit(custom_name, *value)?;
            }

            section = section.with_resource_type_limit(name, limit)?;
        }

        // Set qualification info
        if let Some(qual) = &self.qualification {
            if let Some(hash_str) = &qual.binary_hash {
                let hash = parse_hex_hash(hash_str)?;

                if let Some(asil_level) = &qual.asil_level {
                    if asil_level.len() > MAX_ASIL_STRING_LEN {
                        return Err(Error::parse_error("ASIL level exceeds max length"));
                    }
                    section = section.with_qualification(hash, asil_level)?;
                }
            }
        }

        Ok(section)
    }
}

/// Parse memory size with optional suffix (K, M, G)
#[cfg(feature = "std")]
fn parse_memory_size(s: &str) -> Result<u64, Error> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::parse_error("Empty memory size"));
    }

    let (num_part, suffix) = if s.ends_with('K') || s.ends_with('k') {
        (&s[..s.len() - 1], 1024u64)
    } else if s.ends_with('M') || s.ends_with('m') {
        (&s[..s.len() - 1], 1024u64 * 1024)
    } else if s.ends_with('G') || s.ends_with('g') {
        (&s[..s.len() - 1], 1024u64 * 1024 * 1024)
    } else {
        (s, 1u64)
    };

    let value: u64 = num_part.parse().map_err(|_| Error::parse_error("Invalid memory size"))?;

    value
        .checked_mul(suffix)
        .ok_or_else(|| Error::parse_error("Memory size overflow"))
}

/// Parse hex string to 32-byte hash
#[cfg(feature = "std")]
fn parse_hex_hash(hex: &str) -> Result<[u8; 32], Error> {
    let hex = hex.trim_start_matches("0x");

    if hex.len() != 64 {
        return Err(Error::parse_error("Hash must be 64 hex characters"));
    }

    let mut hash = [0u8; 32];
    for i in 0..32 {
        hash[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::parse_error("Invalid hex in hash"))?;
    }

    Ok(hash)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_size() {
        assert_eq!(parse_memory_size("1024").unwrap(), 1024);
        assert_eq!(parse_memory_size("2K").unwrap(), 2048);
        assert_eq!(parse_memory_size("4M").unwrap(), 4 * 1024 * 1024);
        assert_eq!(parse_memory_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("100k").unwrap(), 100 * 1024);
        assert_eq!(parse_memory_size("50m").unwrap(), 50 * 1024 * 1024);
    }

    #[test]
    fn test_toml_parsing() {
        let toml_str = r#"
version = 1

[execution]
max_fuel_per_step = 1000000
max_memory_usage = "64M"
max_call_depth = 100
max_instructions_per_step = 10000
max_execution_slice_ms = 50

[resources.filesystem]
max_handles = 128
max_memory = "16M"
max_operations_per_second = 1000

[resources.filesystem.custom]
max_file_size = 10485760
max_path_length = 4096

[resources.network]
max_handles = 64
max_memory = "8M"
max_operations_per_second = 500

[qualification]
binary_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
asil_level = "ASIL-D"
"#;

        let config = TomlResourceLimits::from_str(toml_str).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(
            config.execution.as_ref().unwrap().max_fuel_per_step,
            Some(1000000)
        );
        assert_eq!(config.resources.len(), 2);
        assert!(config.resources.contains_key("filesystem"));
        assert!(config.resources.contains_key("network"));

        // Test conversion to ResourceLimitsSection
        let section = config.to_resource_limits_section().unwrap();
        assert_eq!(section.max_fuel_per_step, Some(1000000));
        assert_eq!(section.max_memory_usage, Some(64 * 1024 * 1024));
        assert_eq!(section.qualified_asil_level(), Some("ASIL-D"));
    }
}
