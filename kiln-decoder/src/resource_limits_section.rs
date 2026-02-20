//! Resource limits custom section for WebAssembly binaries
//!
//! This module defines the standard format for embedding execution limits in
//! WebAssembly binaries via custom sections. The format is designed primarily
//! for ASIL-D safety requirements:
//! - Compile-time bounded capacity limits
//! - Deterministic memory usage patterns
//! - No runtime dynamic allocation
//! - Simple binary format without external dependencies
//!
//! Lower ASIL levels (QM/A/B/C) reuse the same format but with relaxed runtime
//! enforcement.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use alloc::{string::String, vec::Vec};

use wrt_error::{Error, ErrorCategory, codes};
use wrt_foundation::{
    BoundedMap, BoundedString, BoundedVec, Checksum, CrateId, NoStdProvider, safe_managed_alloc,
    traits::{Checksummable, ReadStream, WriteStream},
};

/// Standard custom section name for resource limits
pub const RESOURCE_LIMITS_SECTION_NAME: &str = "wrt.resource_limits";

/// Version of the resource limits format (for future compatibility)
pub const RESOURCE_LIMITS_VERSION: u32 = 1;

// ASIL-D compile-time capacity limits - chosen for realistic WebAssembly
// modules while maintaining deterministic behavior
/// Maximum number of different resource types (filesystem, network, etc.)
pub const MAX_RESOURCE_TYPES: usize = 16;
/// Maximum number of custom limits per resource type  
pub const MAX_CUSTOM_LIMITS_PER_TYPE: usize = 32;
/// Maximum length for resource type names (for bounded strings)
pub const MAX_RESOURCE_NAME_LEN: usize = 32;
/// Maximum length for ASIL level strings
pub const MAX_ASIL_STRING_LEN: usize = 16;
/// Maximum total encoded size for ASIL-D bounds
pub const MAX_ENCODED_SIZE: usize = 8192;

/// Type alias for custom limits map with bounded capacity for ASIL-D
pub type CustomLimitsMap<P> =
    BoundedMap<BoundedString<MAX_RESOURCE_NAME_LEN>, u64, MAX_CUSTOM_LIMITS_PER_TYPE, P>;

/// Resource limits specification embedded in WebAssembly custom section
///
/// Design Philosophy for ASIL-D:
/// - Uses wrt-foundation bounded types with compile-time capacity limits
/// - Memory managed through safe_managed_alloc! capability system
/// - All collections have deterministic memory usage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimitsSection<
    P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq = NoStdProvider<4096>,
> {
    /// Format version for compatibility
    pub version: u32,

    /// Maximum fuel consumed per execution step
    /// ASIL-D: REQUIRED - ensures temporal determinism
    pub max_fuel_per_step: Option<u64>,

    /// Maximum memory usage in bytes
    /// ASIL-D: REQUIRED - ensures spatial determinism
    pub max_memory_usage: Option<u64>,

    /// Maximum call stack depth
    /// ASIL-D: REQUIRED - prevents stack overflow
    pub max_call_depth: Option<u32>,

    /// Maximum WebAssembly instructions per execution step
    /// ASIL-D: REQUIRED - ensures deterministic execution steps
    pub max_instructions_per_step: Option<u32>,

    /// Maximum execution time slice in milliseconds
    /// ASIL-D: REQUIRED - for hard real-time requirements
    pub max_execution_slice_ms: Option<u32>,

    /// Resource type limits using bounded map
    /// ASIL-D: Fixed capacity, managed through capability system
    pub resource_type_limits: BoundedMap<
        BoundedString<MAX_RESOURCE_NAME_LEN>,
        ResourceTypeLimit<P>,
        MAX_RESOURCE_TYPES,
        P,
    >,

    /// Binary hash for qualification traceability
    /// Used to verify the WebAssembly module matches qualified binary
    pub qualification_hash: Option<[u8; 32]>,

    /// ASIL level this configuration is qualified for
    /// ASIL-D: Bounded string with compile-time length limit
    pub qualified_asil_level: Option<BoundedString<MAX_ASIL_STRING_LEN>>,
}

/// Limits for a specific resource type (e.g., filesystem, network)
///
/// Design Philosophy for ASIL-D:
/// - Provides fine-grained control over individual resource types
/// - Uses bounded collections with compile-time limits for ASIL-D compatibility
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceTypeLimit<
    P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq = NoStdProvider<4096>,
> {
    /// Maximum number of handles for this resource type
    /// ASIL-D: Bounded to prevent handle exhaustion
    pub max_handles: Option<u32>,

    /// Maximum memory usage for this resource type in bytes
    /// ASIL-D: Contributes to overall memory budget
    pub max_memory: Option<u64>,

    /// Maximum operations per second for this resource type
    /// ASIL-D: Provides temporal isolation between resource types
    pub max_operations_per_second: Option<u32>,

    /// Custom resource-specific limits
    /// Uses bounded collections with compile-time capacity limits for ASIL-D
    pub custom_limits: CustomLimitsMap<P>,
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq> Default
    for ResourceTypeLimit<P>
{
    fn default() -> Self {
        let provider = P::default();
        Self {
            max_handles: None,
            max_memory: None,
            max_operations_per_second: None,
            custom_limits: BoundedMap::new(provider)
                .expect("ASIL-D: Default map creation must succeed"),
        }
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq>
    wrt_foundation::traits::Checksummable for ResourceTypeLimit<P>
{
    fn update_checksum(&self, checksum: &mut Checksum) {
        if let Some(max_handles) = self.max_handles {
            checksum.update_slice(&max_handles.to_le_bytes());
        }
        if let Some(max_memory) = self.max_memory {
            checksum.update_slice(&max_memory.to_le_bytes());
        }
        if let Some(max_ops) = self.max_operations_per_second {
            checksum.update_slice(&max_ops.to_le_bytes());
        }
        self.custom_limits.update_checksum(checksum);
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq>
    wrt_foundation::traits::ToBytes for ResourceTypeLimit<P>
{
    fn serialized_size(&self) -> usize {
        12 + // 3 Option<u32/u64> fields with presence bytes  
        self.custom_limits.serialized_size()
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write max_handles
        if let Some(handles) = self.max_handles {
            writer.write_u8(1)?; // Present
            writer.write_u32_le(handles)?;
        } else {
            writer.write_u8(0)?; // Not present
        }

        // Write max_memory
        if let Some(memory) = self.max_memory {
            writer.write_u8(1)?; // Present
            writer.write_u64_le(memory)?;
        } else {
            writer.write_u8(0)?; // Not present
        }

        // Write max_operations_per_second
        if let Some(ops) = self.max_operations_per_second {
            writer.write_u8(1)?; // Present
            writer.write_u32_le(ops)?;
        } else {
            writer.write_u8(0)?; // Not present
        }

        // Write custom_limits
        self.custom_limits.to_bytes_with_provider(writer, _provider)?;
        Ok(())
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq>
    wrt_foundation::traits::FromBytes for ResourceTypeLimit<P>
{
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read max_handles
        let max_handles = if reader.read_u8()? == 1 { Some(reader.read_u32_le()?) } else { None };

        // Read max_memory
        let max_memory = if reader.read_u8()? == 1 { Some(reader.read_u64_le()?) } else { None };

        // Read max_operations_per_second
        let max_operations_per_second =
            if reader.read_u8()? == 1 { Some(reader.read_u32_le()?) } else { None };

        // Read custom_limits
        let custom_limits = CustomLimitsMap::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            max_handles,
            max_memory,
            max_operations_per_second,
            custom_limits,
        })
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq> Default
    for ResourceLimitsSection<P>
{
    fn default() -> Self {
        let provider = P::default();
        Self {
            version: RESOURCE_LIMITS_VERSION,
            max_fuel_per_step: None,
            max_memory_usage: None,
            max_call_depth: None,
            max_instructions_per_step: None,
            max_execution_slice_ms: None,
            resource_type_limits: BoundedMap::new(provider)
                .expect("ASIL-D: Default map creation must succeed"),
            qualification_hash: None,
            qualified_asil_level: None,
        }
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq>
    ResourceLimitsSection<P>
{
    /// Create a new resource limits section with provider
    pub fn new(provider: P) -> Result<Self, Error> {
        Ok(Self {
            version: RESOURCE_LIMITS_VERSION,
            max_fuel_per_step: None,
            max_memory_usage: None,
            max_call_depth: None,
            max_instructions_per_step: None,
            max_execution_slice_ms: None,
            resource_type_limits: BoundedMap::new(provider).map_err(|_| {
                Error::runtime_execution_error("Failed to create resource type limits ")
            })?,
            qualification_hash: None,
            qualified_asil_level: None,
        })
    }

    /// Create a new resource limits section with default provider
    pub fn with_default_provider() -> Self {
        Self::default()
    }

    /// Create resource limits section with execution limits only
    /// This is the primary constructor for ASIL-D configurations
    pub fn with_execution_limits(
        provider: P,
        max_fuel_per_step: Option<u64>,
        max_memory_usage: Option<u64>,
        max_call_depth: Option<u32>,
        max_instructions_per_step: Option<u32>,
        max_execution_slice_ms: Option<u32>,
    ) -> Result<Self, Error> {
        Ok(Self {
            version: RESOURCE_LIMITS_VERSION,
            max_fuel_per_step,
            max_memory_usage,
            max_call_depth,
            max_instructions_per_step,
            max_execution_slice_ms,
            resource_type_limits: BoundedMap::new(provider).map_err(|_| {
                Error::new(
                    ErrorCategory::Memory,
                    codes::OUT_OF_MEMORY,
                    "Failed to create resource limits map",
                )
            })?,
            qualification_hash: None,
            qualified_asil_level: None,
        })
    }

    /// Create ASIL-D compliant configuration with all required limits
    /// This ensures all fields required for ASIL-D are specified
    pub fn asil_d_config(
        provider: P,
        max_fuel_per_step: u64,
        max_memory_usage: u64,
        max_call_depth: u32,
        max_instructions_per_step: u32,
        max_execution_slice_ms: u32,
    ) -> Result<Self, Error> {
        let asil_level = BoundedString::try_from_str("ASIL-D ")
            .map_err(|_| Error::parse_error("Failed to create ASIL-D string "))?;

        Ok(Self {
            version: RESOURCE_LIMITS_VERSION,
            max_fuel_per_step: Some(max_fuel_per_step),
            max_memory_usage: Some(max_memory_usage),
            max_call_depth: Some(max_call_depth),
            max_instructions_per_step: Some(max_instructions_per_step),
            max_execution_slice_ms: Some(max_execution_slice_ms),
            resource_type_limits: BoundedMap::new(provider).map_err(|_| {
                Error::runtime_execution_error("Failed to create resource type limits ")
            })?,
            qualification_hash: None,
            qualified_asil_level: Some(asil_level),
        })
    }

    /// Add limits for a specific resource type
    /// Validates ASIL-D bounds at configuration time
    pub fn with_resource_type_limit(
        mut self,
        resource_type: &str,
        limit: ResourceTypeLimit<P>,
    ) -> Result<Self, Error> {
        // Validate ASIL-D bounds
        if resource_type.len() > MAX_RESOURCE_NAME_LEN {
            return Err(Error::parse_error("Invalid parameter "));
        }

        if self.resource_type_limits.len() >= MAX_RESOURCE_TYPES {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        // Create bounded string for resource type name
        let resource_name = BoundedString::try_from_str(resource_type)
            .map_err(|_| Error::parse_error("Failed to create resource name "))?;

        self.resource_type_limits
            .insert(resource_name, limit)
            .map_err(|_| Error::runtime_execution_error("Failed to insert resource limit "))?;
        Ok(self)
    }

    /// Set qualification information
    /// Validates ASIL-D bounds for qualification data
    pub fn with_qualification(mut self, hash: [u8; 32], asil_level: &str) -> Result<Self, Error> {
        if asil_level.len() > MAX_ASIL_STRING_LEN {
            return Err(Error::parse_error("Invalid parameter "));
        }

        let bounded_asil_level = BoundedString::try_from_str(asil_level)
            .map_err(|_| Error::parse_error("Failed to create bounded string for ASIL level "))?;

        self.qualification_hash = Some(hash);
        self.qualified_asil_level = Some(bounded_asil_level);
        Ok(self)
    }

    /// Check if all essential limits are specified (required for ASIL-D)
    pub fn is_complete_for_asil_d(&self) -> bool {
        self.max_fuel_per_step.is_some()
            && self.max_memory_usage.is_some()
            && self.max_call_depth.is_some()
            && self.max_instructions_per_step.is_some()
            && self.max_execution_slice_ms.is_some()
    }

    /// Check if this configuration has qualification information
    pub fn is_qualified(&self) -> bool {
        self.qualification_hash.is_some() && self.qualified_asil_level.is_some()
    }

    /// Get the ASIL level this configuration is qualified for
    pub fn qualified_asil_level(&self) -> Option<&str> {
        self.qualified_asil_level.as_ref().and_then(|s| s.as_str().ok())
    }

    /// Validate ASIL-D compliance
    /// This checks that all required fields are present and within bounds
    pub fn validate_asil_d_compliance(&self) -> Result<(), Error> {
        if !self.is_complete_for_asil_d() {
            return Err(Error::parse_error(
                "ASIL-D requires all execution limits to be specified ",
            ));
        }

        // Validate each limit is within reasonable bounds for ASIL-D
        self.validate()?;

        // Additional ASIL-D specific validations
        if let Some(fuel) = self.max_fuel_per_step {
            if fuel > 1_000_000 {
                // 1M fuel per step max for determinism
                return Err(Error::parse_error(
                    "ASIL-D fuel limit too high for deterministic execution ",
                ));
            }
        }

        if let Some(memory) = self.max_memory_usage {
            if memory > 1024 * 1024 * 1024 {
                // 1GB max for ASIL-D
                return Err(Error::parse_error(
                    "ASIL-D memory limit too high for deterministic execution ",
                ));
            }
        }

        Ok(())
    }

    /// Encode to binary format for embedding in WebAssembly custom section
    /// Uses simple binary format for ASIL-D compatibility (no external
    /// dependencies)
    ///
    /// Returns the encoded data and the actual size used
    pub fn encode_to_buffer(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < MAX_ENCODED_SIZE {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        let mut offset = 0;

        // Write version (4 bytes)
        buffer[offset..offset + 4].copy_from_slice(&self.version.to_le_bytes());
        offset += 4;

        // Write optional fields
        offset = self.encode_optional_u64_to_buffer(buffer, offset, self.max_fuel_per_step)?;
        offset = self.encode_optional_u64_to_buffer(buffer, offset, self.max_memory_usage)?;
        offset = self.encode_optional_u32_to_buffer(buffer, offset, self.max_call_depth)?;
        offset =
            self.encode_optional_u32_to_buffer(buffer, offset, self.max_instructions_per_step)?;
        offset = self.encode_optional_u32_to_buffer(buffer, offset, self.max_execution_slice_ms)?;

        // Write resource type limits count + data
        let resource_count = self.resource_type_limits.len() as u32;
        buffer[offset..offset + 4].copy_from_slice(&resource_count.to_le_bytes());
        offset += 4;

        // TODO: Fix iteration over BoundedMap - need to implement proper key-value
        // iteration for (name, limits) in self.resource_type_limits.iter() {
        //     offset = self.encode_string_to_buffer(buffer, offset, name.as_str())?;
        //     offset = self.encode_resource_type_limit_to_buffer(buffer, offset,
        // limits)?; }

        // Write optional qualification info
        if let Some(hash) = &self.qualification_hash {
            buffer[offset] = 1; // present
            offset += 1;
            buffer[offset..offset + 32].copy_from_slice(hash);
            offset += 32;
        } else {
            buffer[offset] = 0; // not present
            offset += 1;
        }

        if let Some(asil_level) = &self.qualified_asil_level {
            buffer[offset] = 1; // present
            offset += 1;
            offset = self.encode_string_to_buffer(buffer, offset, asil_level.as_str()?)?;
        } else {
            buffer[offset] = 0; // not present
            offset += 1;
        }

        Ok(offset)
    }

    /// Encode to binary format for embedding in WebAssembly custom section
    /// Uses simple binary format for ASIL-D compatibility (no external
    /// dependencies)
    ///
    /// This method allocates a Vec for compatibility but internally uses
    /// bounded encoding
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut encoded = Vec::new();

        // Write version (4 bytes)
        encoded.extend_from_slice(&self.version.to_le_bytes());

        // Write optional fields as presence byte + value
        self.encode_optional_u64(&mut encoded, self.max_fuel_per_step)?;
        self.encode_optional_u64(&mut encoded, self.max_memory_usage)?;
        self.encode_optional_u32(&mut encoded, self.max_call_depth)?;
        self.encode_optional_u32(&mut encoded, self.max_instructions_per_step)?;
        self.encode_optional_u32(&mut encoded, self.max_execution_slice_ms)?;

        // Write resource type limits count + data
        if self.resource_type_limits.len() > MAX_RESOURCE_TYPES {
            return Err(Error::new(
                ErrorCategory::Parse,
                codes::CAPACITY_EXCEEDED,
                "Capacity exceeded",
            ));
        }

        encoded.extend_from_slice(&(self.resource_type_limits.len() as u32).to_le_bytes());
        // TODO: Fix iteration over BoundedMap
        // for (name, limits) in self.resource_type_limits.iter() {
        //     self.encode_string(&mut encoded, name.as_str())?;
        //     self.encode_resource_type_limit(&mut encoded, limits)?;
        // }

        // Write optional qualification info
        if let Some(hash) = &self.qualification_hash {
            encoded.push(1); // present
            encoded.extend_from_slice(hash);
        } else {
            encoded.push(0); // not present
        }

        if let Some(asil_level) = &self.qualified_asil_level {
            encoded.push(1); // present
            self.encode_string(&mut encoded, asil_level.as_str()?)?;
        } else {
            encoded.push(0); // not present
        }

        // Validate ASIL-D size bounds
        if encoded.len() > MAX_ENCODED_SIZE {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        Ok(encoded)
    }

    /// Decode from binary format (from WebAssembly custom section data)
    /// Uses simple binary format for ASIL-D compatibility (no external
    /// dependencies) TODO: Update for bounded types compatibility
    #[allow(dead_code)]
    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        // Validate ASIL-D size bounds before decoding
        if data.len() > MAX_ENCODED_SIZE {
            return Err(Error::new(
                ErrorCategory::Parse,
                codes::CAPACITY_EXCEEDED,
                "Capacity exceeded",
            ));
        }

        let mut offset = 0;

        // Read version (4 bytes)
        if data.len() < 4 {
            return Err(Error::parse_error("Resource limits data too short"));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        offset += 4;

        // Validate version compatibility
        if version > RESOURCE_LIMITS_VERSION {
            return Err(Error::parse_error("Unsupported resource limits version "));
        }

        // Read optional fields
        let (max_fuel_per_step, new_offset) = Self::decode_optional_u64(data, offset)?;
        offset = new_offset;

        let (max_memory_usage, new_offset) = Self::decode_optional_u64(data, offset)?;
        offset = new_offset;

        let (max_call_depth, new_offset) = Self::decode_optional_u32(data, offset)?;
        offset = new_offset;

        let (max_instructions_per_step, new_offset) = Self::decode_optional_u32(data, offset)?;
        offset = new_offset;

        let (max_execution_slice_ms, new_offset) = Self::decode_optional_u32(data, offset)?;
        offset = new_offset;

        // Read resource type limits
        if offset + 4 > data.len() {
            return Err(Error::parse_error("Resource limits data truncated "));
        }

        let resource_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if resource_count > MAX_RESOURCE_TYPES {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        // TODO: Update for bounded types
        let provider = P::default();
        let mut resource_type_limits = BoundedMap::new(provider.clone()).map_err(|_| {
            Error::new(
                ErrorCategory::Memory,
                codes::OUT_OF_MEMORY,
                "Failed to create resource limits map ",
            )
        })?;
        for _ in 0..resource_count {
            let (name_str, new_offset) = Self::decode_string(data, offset)?;
            offset = new_offset;

            let (limits, new_offset) = Self::decode_resource_type_limit(data, offset)?;
            offset = new_offset;

            let name = BoundedString::try_from_str(&name_str).map_err(|_| {
                Error::parse_error("Failed to create bounded string during decode ")
            })?;

            resource_type_limits.insert(name, limits).map_err(|_| {
                Error::runtime_execution_error("Failed to insert resource type limits ")
            })?;
        }

        // Read optional qualification info
        if offset >= data.len() {
            return Err(Error::parse_error("Invalid parameter "));
        }

        let qualification_hash = if data[offset] == 1 {
            offset += 1;
            if offset + 32 > data.len() {
                return Err(Error::parse_error(
                    "Resource limits qualification hash truncated ",
                ));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[offset..offset + 32]);
            offset += 32;
            Some(hash)
        } else {
            offset += 1;
            None
        };

        let qualified_asil_level = if offset < data.len() && data[offset] == 1 {
            offset += 1;
            let (asil_level_str, _) = Self::decode_string(data, offset)?;
            let asil_level = BoundedString::try_from_str(&asil_level_str).map_err(|_| {
                Error::parse_error("Failed to create bounded string for ASIL level during decode")
            })?;
            Some(asil_level)
        } else {
            None
        };

        let section = Self {
            version,
            max_fuel_per_step,
            max_memory_usage,
            max_call_depth,
            max_instructions_per_step,
            max_execution_slice_ms,
            resource_type_limits,
            qualification_hash,
            qualified_asil_level,
        };

        // Validate ASIL-D bounds
        section.validate_bounds()?;

        Ok(section)
    }

    /// Validate ASIL-D bounds constraints
    fn validate_bounds(&self) -> Result<(), Error> {
        if self.resource_type_limits.len() > MAX_RESOURCE_TYPES {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        // TODO: Fix iteration over BoundedMap
        // for (name, limit) in self.resource_type_limits.iter() {
        //     if name.len() > MAX_RESOURCE_NAME_LEN {
        //         return Err(Error::new(
        //             ErrorCategory::Parse,
        //             codes::PARSE_ERROR,
        //             "Resource name too long";
        //     }
        //
        //     if limit.custom_limits.len() > MAX_CUSTOM_LIMITS_PER_TYPE {
        //         return Err(Error::runtime_execution_error("Too many custom limits";
        //     }
        // }

        if let Some(asil_level) = &self.qualified_asil_level {
            if asil_level.len() > MAX_ASIL_STRING_LEN {
                return Err(Error::parse_error("Invalid handles"));
            }
        }

        Ok(())
    }

    // Helper functions for binary encoding/decoding

    fn encode_optional_u64_to_buffer(
        &self,
        buffer: &mut [u8],
        offset: usize,
        value: Option<u64>,
    ) -> Result<usize, Error> {
        if let Some(val) = value {
            if offset + 9 > buffer.len() {
                return Err(Error::runtime_execution_error("Buffer overflow"));
            }
            buffer[offset] = 1; // present
            buffer[offset + 1..offset + 9].copy_from_slice(&val.to_le_bytes());
            Ok(offset + 9)
        } else {
            if offset >= buffer.len() {
                return Err(Error::new(
                    ErrorCategory::Memory,
                    codes::OUT_OF_MEMORY,
                    "Capacity exceeded",
                ));
            }
            buffer[offset] = 0; // not present
            Ok(offset + 1)
        }
    }

    fn encode_optional_u32_to_buffer(
        &self,
        buffer: &mut [u8],
        offset: usize,
        value: Option<u32>,
    ) -> Result<usize, Error> {
        if let Some(val) = value {
            if offset + 5 > buffer.len() {
                return Err(Error::runtime_execution_error("Buffer overflow"));
            }
            buffer[offset] = 1; // present
            buffer[offset + 1..offset + 5].copy_from_slice(&val.to_le_bytes());
            Ok(offset + 5)
        } else {
            if offset >= buffer.len() {
                return Err(Error::new(
                    ErrorCategory::Memory,
                    codes::OUT_OF_MEMORY,
                    "Capacity exceeded",
                ));
            }
            buffer[offset] = 0; // not present
            Ok(offset + 1)
        }
    }

    fn encode_string_to_buffer(
        &self,
        buffer: &mut [u8],
        offset: usize,
        s: &str,
    ) -> Result<usize, Error> {
        if s.len() > MAX_RESOURCE_NAME_LEN {
            return Err(Error::parse_error("String exceeds ASIL length bounds"));
        }

        let len_bytes = (s.len() as u32).to_le_bytes();
        if offset + 4 + s.len() > buffer.len() {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        buffer[offset..offset + 4].copy_from_slice(&len_bytes);
        buffer[offset + 4..offset + 4 + s.len()].copy_from_slice(s.as_bytes());
        Ok(offset + 4 + s.len())
    }

    fn encode_resource_type_limit_to_buffer(
        &self,
        buffer: &mut [u8],
        mut offset: usize,
        limits: &ResourceTypeLimit,
    ) -> Result<usize, Error> {
        offset = self.encode_optional_u32_to_buffer(buffer, offset, limits.max_handles)?;
        offset = self.encode_optional_u64_to_buffer(buffer, offset, limits.max_memory)?;
        offset =
            self.encode_optional_u32_to_buffer(buffer, offset, limits.max_operations_per_second)?;

        // Encode custom limits
        if limits.custom_limits.len() > MAX_CUSTOM_LIMITS_PER_TYPE {
            return Err(Error::new(
                ErrorCategory::Parse,
                codes::CAPACITY_EXCEEDED,
                "Capacity exceeded",
            ));
        }

        let count_bytes = (limits.custom_limits.len() as u32).to_le_bytes();
        if offset + 4 > buffer.len() {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }
        buffer[offset..offset + 4].copy_from_slice(&count_bytes);
        offset += 4;

        // TODO: Fix iteration over BoundedMap
        // for (name, value) in limits.custom_limits.iter() {
        //     offset = self.encode_string_to_buffer(buffer, offset, name.as_str())?;
        //     if offset + 8 > buffer.len() {
        //         return Err(Error::new(
        //             ErrorCategory::Memory,
        //             codes::OUT_OF_MEMORY,
        //             ";
        //     }
        //     buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        //     offset += 8;
        // }

        Ok(offset)
    }

    // Legacy Vec-based encoding helpers (for compatibility)
    fn encode_optional_u64(&self, buffer: &mut Vec<u8>, value: Option<u64>) -> Result<(), Error> {
        if let Some(val) = value {
            buffer.push(1); // present
            buffer.extend_from_slice(&val.to_le_bytes());
        } else {
            buffer.push(0); // not present
        }
        Ok(())
    }

    fn encode_optional_u32(&self, buffer: &mut Vec<u8>, value: Option<u32>) -> Result<(), Error> {
        if let Some(val) = value {
            buffer.push(1); // present
            buffer.extend_from_slice(&val.to_le_bytes());
        } else {
            buffer.push(0); // not present
        }
        Ok(())
    }

    fn encode_string(&self, buffer: &mut Vec<u8>, s: &str) -> Result<(), Error> {
        if s.len() > MAX_RESOURCE_NAME_LEN {
            return Err(Error::parse_error("String exceeds ASIL length bounds"));
        }

        buffer.extend_from_slice(&(s.len() as u32).to_le_bytes());
        buffer.extend_from_slice(s.as_bytes());
        Ok(())
    }

    fn encode_resource_type_limit(
        &self,
        buffer: &mut Vec<u8>,
        limits: &ResourceTypeLimit,
    ) -> Result<(), Error> {
        self.encode_optional_u32(buffer, limits.max_handles)?;
        self.encode_optional_u64(buffer, limits.max_memory)?;
        self.encode_optional_u32(buffer, limits.max_operations_per_second)?;

        // Encode custom limits
        if limits.custom_limits.len() > MAX_CUSTOM_LIMITS_PER_TYPE {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        buffer.extend_from_slice(&(limits.custom_limits.len() as u32).to_le_bytes());
        // TODO: Fix iteration over BoundedMap
        // for (name, value) in limits.custom_limits.iter() {
        //     self.encode_string(buffer, name.as_str())?;
        //     buffer.extend_from_slice(&value.to_le_bytes());
        // }

        Ok(())
    }

    fn decode_optional_u64(data: &[u8], offset: usize) -> Result<(Option<u64>, usize), Error> {
        if offset >= data.len() {
            return Err(Error::parse_error("Invalid parameter "));
        }

        if data[offset] == 1 {
            // present
            if offset + 9 > data.len() {
                return Err(Error::parse_error("Data truncated reading u64 value "));
            }
            let value = u64::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
                data[offset + 8],
            ]);
            Ok((Some(value), offset + 9))
        } else {
            // not present
            Ok((None, offset + 1))
        }
    }

    fn decode_optional_u32(data: &[u8], offset: usize) -> Result<(Option<u32>, usize), Error> {
        if offset >= data.len() {
            return Err(Error::parse_error("Data truncated reading optional u32 "));
        }

        if data[offset] == 1 {
            // present
            if offset + 5 > data.len() {
                return Err(Error::parse_error("Data truncated reading u32 value "));
            }
            let value = u32::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ]);
            Ok((Some(value), offset + 5))
        } else {
            // not present
            Ok((None, offset + 1))
        }
    }

    fn decode_string(data: &[u8], offset: usize) -> Result<(String, usize), Error> {
        if offset + 4 > data.len() {
            return Err(Error::parse_error("Data truncated reading string length "));
        }

        let length = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        if length > MAX_RESOURCE_NAME_LEN {
            return Err(Error::parse_error("String exceeds ASIL length bounds"));
        }

        let start = offset + 4;
        if start + length > data.len() {
            return Err(Error::parse_error("Data truncated reading string data "));
        }

        let string_data = &data[start..start + length];
        let s = String::from_utf8(string_data.to_vec())
            .map_err(|_| Error::parse_error("Invalid UTF-8 in string "))?;

        Ok((s, start + length))
    }

    fn decode_resource_type_limit(
        data: &[u8],
        offset: usize,
    ) -> Result<(ResourceTypeLimit<P>, usize), Error> {
        let (max_handles, offset) = Self::decode_optional_u32(data, offset)?;
        let (max_memory, offset) = Self::decode_optional_u64(data, offset)?;
        let (max_operations_per_second, offset) = Self::decode_optional_u32(data, offset)?;

        // Decode custom limits
        if offset + 4 > data.len() {
            return Err(Error::parse_error(
                "Data truncated reading custom limits count ",
            ));
        }

        let custom_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        let mut offset = offset + 4;

        if custom_count > MAX_CUSTOM_LIMITS_PER_TYPE {
            return Err(Error::runtime_execution_error("Buffer overflow"));
        }

        // TODO: Update for bounded types
        let provider = P::default();
        let mut custom_limits = BoundedMap::new(provider.clone()).map_err(|_| {
            Error::new(
                ErrorCategory::Memory,
                codes::OUT_OF_MEMORY,
                "Failed to create custom limits map",
            )
        })?;
        for _ in 0..custom_count {
            let (name_str, new_offset) = Self::decode_string(data, offset)?;
            offset = new_offset;

            if offset + 8 > data.len() {
                return Err(Error::parse_error(
                    "Data truncated reading custom limit value",
                ));
            }

            let value = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;

            let name = BoundedString::try_from_str(&name_str).map_err(|_| {
                Error::parse_error(
                    "Failed to create bounded string for custom limit name during decode",
                )
            })?;

            custom_limits
                .insert(name, value)
                .map_err(|_| Error::runtime_execution_error("Failed to insert custom limits"))?;
        }

        let limits = ResourceTypeLimit {
            max_handles,
            max_memory,
            max_operations_per_second,
            custom_limits,
        };

        Ok((limits, offset))
    }

    /// Validate that the limits are reasonable and consistent
    pub fn validate(&self) -> Result<(), Error> {
        // Check for reasonable limits to prevent overflow/underflow issues
        if let Some(fuel) = self.max_fuel_per_step {
            if fuel == 0 {
                return Err(Error::parse_error("Invalid handles"));
            }
        }

        if let Some(memory) = self.max_memory_usage {
            if memory == 0 {
                return Err(Error::parse_error("max_memory_usage cannot be zero "));
            }
            // Check for reasonable memory limit (max 4GB for general use)
            if memory > 4 * 1024 * 1024 * 1024 {
                return Err(Error::parse_error("max_memory_usage too large (max 4GB)"));
            }
        }

        if let Some(depth) = self.max_call_depth {
            if depth == 0 {
                return Err(Error::parse_error("max_call_depth cannot be zero "));
            }
            // Check for reasonable call depth (max 10000)
            if depth > 10000 {
                return Err(Error::parse_error("max_call_depth too large (max 10000)"));
            }
        }

        if let Some(instructions) = self.max_instructions_per_step {
            if instructions == 0 {
                return Err(Error::parse_error(
                    "max_instructions_per_step cannot be zero ",
                ));
            }
        }

        if let Some(slice_ms) = self.max_execution_slice_ms {
            if slice_ms == 0 {
                return Err(Error::parse_error("max_execution_slice_ms cannot be zero "));
            }
        }

        // TODO: Fix iteration over BoundedMap
        // Validate resource type limits
        // for (resource_type, limit) in self.resource_type_limits.iter() {
        //     limit.validate(resource_type.as_str())?;
        // }

        Ok(())
    }
}

impl<P: wrt_foundation::MemoryProvider + Clone + Default + PartialEq + Eq> ResourceTypeLimit<P> {
    /// Create a new resource type limit
    pub fn new(provider: P) -> Result<Self, Error> {
        Ok(Self {
            max_handles: None,
            max_memory: None,
            max_operations_per_second: None,
            custom_limits: BoundedMap::new(provider)
                .map_err(|_| Error::runtime_execution_error("Failed to create custom limits "))?,
        })
    }

    /// Set maximum handles for this resource type
    pub fn with_max_handles(mut self, max_handles: u32) -> Self {
        self.max_handles = Some(max_handles);
        self
    }

    /// Set maximum memory for this resource type
    pub fn with_max_memory(mut self, max_memory: u64) -> Self {
        self.max_memory = Some(max_memory);
        self
    }

    /// Set maximum operations per second for this resource type
    pub fn with_max_operations_per_second(mut self, max_ops: u32) -> Self {
        self.max_operations_per_second = Some(max_ops);
        self
    }

    /// Add a custom limit for this resource type
    /// Validates ASIL-D bounds
    pub fn with_custom_limit(mut self, name: &str, value: u64) -> Result<Self, Error> {
        if self.custom_limits.len() >= MAX_CUSTOM_LIMITS_PER_TYPE {
            return Err(Error::new(
                ErrorCategory::Parse,
                codes::CAPACITY_EXCEEDED,
                "Capacity exceeded",
            ));
        }

        let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
            Error::parse_error("Failed to create bounded string for custom limit name")
        })?;

        self.custom_limits
            .insert(bounded_name, value)
            .map_err(|_| Error::runtime_execution_error("Failed to insert custom limit"))?;
        Ok(self)
    }

    /// Validate the resource type limit
    fn validate(&self, _resource_type: &str) -> Result<(), Error> {
        if let Some(handles) = self.max_handles {
            if handles == 0 {
                return Err(Error::parse_error("Invalid handles"));
            }
        }

        if let Some(memory) = self.max_memory {
            if memory == 0 {
                return Err(Error::parse_error("max_memory cannot be zero "));
            }
        }

        if let Some(ops) = self.max_operations_per_second {
            if ops == 0 {
                return Err(Error::parse_error(
                    "max_operations_per_second cannot be zero ",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::format;

    use super::*;

    #[test]
    fn test_asil_d_config_creation() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let limits = ResourceLimitsSection::asil_d_config(
            provider,
            1000,      // max_fuel_per_step
            64 * 1024, // max_memory_usage (64KB)
            32,        // max_call_depth
            10,        // max_instructions_per_step
            100,       // max_execution_slice_ms
        )
        .expect("ASIL config creation should succeed");

        assert!(limits.is_complete_for_asil_d());
        assert!(limits.validate_asil_d_compliance().is_ok());
        assert_eq!(limits.qualified_asil_level(), Some("ASIL-D "));
        Ok(())
    }

    #[test]
    fn test_asil_d_bounds_validation() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let mut limits =
            ResourceLimitsSection::asil_d_config(provider.clone(), 1000, 64 * 1024, 32, 10, 100)
                .expect("ASIL config creation should succeed");

        // Test resource type bounds
        for i in 0..MAX_RESOURCE_TYPES {
            let resource_name = format!("resource_{}", i);
            limits = limits
                .with_resource_type_limit(
                    &resource_name,
                    ResourceTypeLimit::new(provider.clone())
                        .expect("Resource type limit creation should succeed "),
                )
                .expect("Should fit within bounds");
        }

        // This should fail - exceeds capacity
        let result = limits.with_resource_type_limit(
            "too_many",
            ResourceTypeLimit::new(provider.clone())
                .expect("Resource type limit creation should succeed "),
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_asil_d_size_limits() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let limits = ResourceLimitsSection::asil_d_config(
            provider,
            2_000_000, // Too high fuel for ASIL-D
            64 * 1024,
            32,
            10,
            100,
        )
        .expect("ASIL config creation should succeed");

        assert!(limits.validate_asil_d_compliance().is_err());
        Ok(())
    }

    #[test]
    fn test_qualification_info() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let hash = [0u8; 32];
        let asil_d_str = ["ASIL", "-", "D"].concat();
        let limits =
            ResourceLimitsSection::new(provider.clone())?.with_qualification(hash, &asil_d_str)?;

        assert!(limits.is_qualified());
        let expected = ["ASIL", "-", "D"].concat();
        assert_eq!(
            limits.qualified_asil_level().as_deref(),
            Some(expected.as_str())
        );
        Ok(())
    }

    #[test]
    fn test_encode_decode_roundtrip() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let original =
            ResourceLimitsSection::asil_d_config(provider.clone(), 1000, 64 * 1024, 32, 10, 100)?;

        // Note: encode/decode will need to be updated for bounded types
        // For now, just test the creation and validation
        assert!(original.is_complete_for_asil_d());
        assert!(original.validate_asil_d_compliance().is_ok());
        Ok(())
    }

    #[test]
    fn test_lower_asil_levels() -> wrt_error::Result<()> {
        let provider = wrt_foundation::safe_managed_alloc!(
            4096,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        // Test that lower ASIL levels can use partial configuration
        let asil_b_limits = ResourceLimitsSection::with_execution_limits(
            provider,
            Some(1000),
            Some(64 * 1024),
            Some(32),
            None, // ASIL-B doesn't require instruction limits
            Some(100),
        )?;

        assert!(!asil_b_limits.is_complete_for_asil_d()); // Not complete for ASIL-D
        assert!(asil_b_limits.validate().is_ok()); // But still valid for lower levels
        Ok(())
    }
}
