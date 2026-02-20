//! WASI resource manager using WRT foundation patterns
//!
//! This module provides resource management for WASI handles using the proven
//! Resource<P> patterns from wrt-foundation.

#[cfg(feature = "std")]
use std::collections::HashMap;

use wrt_error::Result;
#[cfg(feature = "std")]
use wrt_foundation::capabilities::CapabilityAwareProvider;
#[cfg(not(feature = "std"))]
use wrt_foundation::budget_aware_provider::CrateId as BudgetCrateId;
use wrt_foundation::{
    resource::{
        Resource,
        ResourceOperation,
    },
    safe_memory::NoStdProvider,
    traits::{
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
    BoundedString,
};
#[cfg(not(feature = "std"))]
use wrt_foundation::{safe_managed_alloc, BoundedMap};

use crate::prelude::*;

/// Maximum number of WASI resources per manager
const MAX_WASI_RESOURCES: usize = 256;

// Type alias for provider (used for no_std BoundedMap only)
#[cfg(not(feature = "std"))]
type WasiProvider = NoStdProvider<65536>;

// Helper function to create provider (for no_std)
#[cfg(not(feature = "std"))]
fn create_wasi_provider() -> Result<WasiProvider> {
    let provider = safe_managed_alloc!(65536, BudgetCrateId::Wasi)?;
    Ok(provider)
}

/// WASI resource handle type
pub type WasiHandle = u32;

/// WASI-specific resource types
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WasiResourceType {
    /// Null/empty resource type (default)
    #[default]
    Null,
    /// File descriptor
    FileDescriptor {
        /// Path to the file being accessed.
        #[cfg(feature = "std")]
        path:     BoundedString<256>,
        /// Path to the file being accessed.
        #[cfg(not(feature = "std"))]
        path:     BoundedString<256>,
        /// Whether the file descriptor allows read operations.
        readable: bool,
        /// Whether the file descriptor allows write operations.
        writable: bool,
    },
    /// Directory handle
    DirectoryHandle {
        /// Path to the directory being accessed.
        #[cfg(feature = "std")]
        path: BoundedString<256>,
        /// Path to the directory being accessed.
        #[cfg(not(feature = "std"))]
        path: BoundedString<256>,
    },
    /// Input stream
    InputStream {
        /// Name identifier for the input stream.
        #[cfg(feature = "std")]
        name:     BoundedString<64>,
        /// Name identifier for the input stream.
        #[cfg(not(feature = "std"))]
        name:     BoundedString<64>,
        /// Current position in the stream for read operations.
        position: u64,
    },
    /// Output stream
    OutputStream {
        /// Name identifier for the output stream.
        #[cfg(feature = "std")]
        name:     BoundedString<64>,
        /// Name identifier for the output stream.
        #[cfg(not(feature = "std"))]
        name:     BoundedString<64>,
        /// Current position in the stream for write operations.
        position: u64,
    },
    /// Clock handle
    ClockHandle {
        /// Type of clock being accessed.
        clock_type: WasiClockType
    },
    /// Random generator handle
    RandomHandle {
        /// Whether to use a cryptographically secure random generator.
        secure: bool
    },
}

/// WASI clock types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WasiClockType {
    /// Realtime clock
    #[default]
    Realtime,
    /// Monotonic clock
    Monotonic,
    /// Process CPU time
    ProcessCpuTime,
    /// Thread CPU time
    ThreadCpuTime,
}

/// WASI resource manager
///
/// Manages WASI resource handles using WRT's proven resource management
/// patterns
#[derive(Debug)]
pub struct WasiResourceManager {
    /// Resource table - uses `HashMap` for std (simpler, no serialization)
    #[cfg(feature = "std")]
    resources: HashMap<WasiHandle, WasiResource>,
    /// Resource table - uses BoundedMap for no_std (bounded, memory-safe)
    #[cfg(not(feature = "std"))]
    resources: BoundedMap<WasiHandle, WasiResource, MAX_WASI_RESOURCES, WasiProvider>,
    /// Next available handle ID
    next_handle: WasiHandle,
    /// Memory provider for allocations (only needed for no_std)
    #[cfg(not(feature = "std"))]
    _provider: WasiProvider,
}

/// WASI resource wrapper using WRT Resource<P> pattern
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiResource {
    /// Base WRT resource
    #[cfg(feature = "std")]
    base:          Resource<CapabilityAwareProvider<NoStdProvider<8192>>>,
    #[cfg(not(feature = "std"))]
    base:          Resource<NoStdProvider<8192>>,
    /// WASI-specific resource type
    resource_type: WasiResourceType,
    /// Resource capabilities
    capabilities:  WasiResourceCapabilities,
}

impl Default for WasiResource {
    fn default() -> Self {
        Self {
            base:          Resource::new(
                0, // default ID
                wrt_foundation::resource::ResourceRepr::Opaque,
                None,
                wrt_foundation::verification::VerificationLevel::Standard,
            ),
            resource_type: WasiResourceType::Null,
            capabilities:  WasiResourceCapabilities::default(),
        }
    }
}

/// Capabilities for WASI resources
// Multiple bool flags represent independent capability toggles.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct WasiResourceCapabilities {
    /// Can read from this resource
    pub readable:        bool,
    /// Can write to this resource  
    pub writable:        bool,
    /// Can seek in this resource
    pub seekable:        bool,
    /// Can get metadata from this resource
    pub metadata_access: bool,
}

impl WasiResourceManager {
    /// Create a new WASI resource manager
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation for the resource table fails.
    pub fn new() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            Ok(Self {
                resources: HashMap::new(),
                next_handle: 1, // Start at 1, reserve 0 for invalid handle
            })
        }
        #[cfg(not(feature = "std"))]
        {
            let provider = create_wasi_provider()?;
            let provider2 = create_wasi_provider()?;
            let resources = BoundedMap::new(provider2)?;

            Ok(Self {
                resources,
                next_handle: 1, // Start at 1, reserve 0 for invalid handle
                _provider: provider,
            })
        }
    }

    /// Create a new WASI resource and return its handle
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The maximum number of resources (256) has been reached
    /// - Failed to insert the resource into the resource table
    pub fn create_resource(
        &mut self,
        resource_type: WasiResourceType,
        capabilities: WasiResourceCapabilities,
    ) -> Result<WasiHandle> {
        // Create base WRT resource
        let base = Resource::new(
            self.next_handle,
            wrt_foundation::resource::ResourceRepr::Opaque,
            None,
            wrt_foundation::verification::VerificationLevel::Standard,
        );

        // Create WASI resource wrapper
        let wasi_resource = WasiResource {
            base,
            resource_type,
            capabilities,
        };

        // Get next handle ID
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle == 0 {
            self.next_handle = 1; // Skip 0
        }

        // Store resource
        #[cfg(feature = "std")]
        {
            if self.resources.len() >= MAX_WASI_RESOURCES {
                return Err(Error::runtime_execution_error("Resource limit exceeded"));
            }
            self.resources.insert(handle, wasi_resource);
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources
                .insert(handle, wasi_resource)
                .map_err(|_| Error::runtime_execution_error("Failed to insert resource into map"))?;
        }

        Ok(handle)
    }

    /// Get a WASI resource by handle
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is invalid or does not exist in the resource table.
    pub fn get_resource(&self, handle: WasiHandle) -> Result<WasiResource> {
        #[cfg(feature = "std")]
        {
            self.resources.get(&handle).cloned().ok_or_else(|| {
                Error::new(
                    ErrorCategory::Resource,
                    codes::WASI_INVALID_FD,
                    "Invalid WASI handle",
                )
            })
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources.get(&handle)?.ok_or_else(|| {
                Error::new(
                    ErrorCategory::Resource,
                    codes::WASI_INVALID_FD,
                    "Invalid WASI handle",
                )
            })
        }
    }

    /// Get a WASI resource by handle (for modification via `update_resource`)
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is invalid or does not exist in the resource table.
    pub fn get_resource_mut(&mut self, handle: WasiHandle) -> Result<WasiResource> {
        #[cfg(feature = "std")]
        {
            self.resources.get(&handle).cloned().ok_or_else(|| {
                Error::runtime_execution_error("Resource not found")
            })
        }
        #[cfg(not(feature = "std"))]
        {
            // Note: BoundedMap doesn't support get_mut due to serialization constraints
            // Use get() and then update_resource() to modify
            self.resources
                .get(&handle)?
                .ok_or_else(|| Error::runtime_execution_error("Resource not found"))
        }
    }

    /// Remove a WASI resource by handle
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is invalid or does not exist in the resource table.
    pub fn remove_resource(&mut self, handle: WasiHandle) -> Result<WasiResource> {
        #[cfg(feature = "std")]
        {
            self.resources.remove(&handle).ok_or_else(|| {
                Error::new(
                    ErrorCategory::Resource,
                    codes::WASI_INVALID_FD,
                    "Invalid WASI handle",
                )
            })
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources.remove(&handle)?.ok_or_else(|| {
                Error::new(
                    ErrorCategory::Resource,
                    codes::WASI_INVALID_FD,
                    "Invalid WASI handle",
                )
            })
        }
    }

    /// Check if a handle is valid
    #[must_use]
    pub fn is_valid_handle(&self, handle: WasiHandle) -> bool {
        #[cfg(feature = "std")]
        {
            self.resources.contains_key(&handle)
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources.contains_key(&handle).unwrap_or(false)
        }
    }

    /// Get the number of active resources
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Create a file descriptor resource
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path string exceeds the maximum length (256 characters)
    /// - The resource table is full or allocation fails
    pub fn create_file_descriptor(
        &mut self,
        path: &str,
        readable: bool,
        writable: bool,
    ) -> Result<WasiHandle> {
        let path_string = BoundedString::try_from_str(path)
            .map_err(|_| Error::runtime_execution_error("Path string too long"))?;

        let resource_type = WasiResourceType::FileDescriptor {
            path: path_string,
            readable,
            writable,
        };

        let capabilities = WasiResourceCapabilities {
            readable,
            writable,
            seekable: true,
            metadata_access: true,
        };

        self.create_resource(resource_type, capabilities)
    }

    /// Create a directory handle resource
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path string exceeds the maximum length (256 characters)
    /// - The resource table is full or allocation fails
    pub fn create_directory_handle(&mut self, path: &str) -> Result<WasiHandle> {
        let path_string = BoundedString::try_from_str(path).map_err(|_| {
            Error::new(
                ErrorCategory::Resource,
                codes::WASI_RESOURCE_LIMIT,
                "Invalid WASI handle",
            )
        })?;

        let resource_type = WasiResourceType::DirectoryHandle { path: path_string };

        let capabilities = WasiResourceCapabilities {
            readable:        true,
            writable:        false,
            seekable:        false,
            metadata_access: true,
        };

        self.create_resource(resource_type, capabilities)
    }

    /// Create an input stream resource
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream name exceeds the maximum length (64 characters)
    /// - The resource table is full or allocation fails
    pub fn create_input_stream(&mut self, name: &str) -> Result<WasiHandle> {
        let name_string = BoundedString::try_from_str(name)
            .map_err(|_| Error::runtime_execution_error("Stream name too long"))?;

        let resource_type = WasiResourceType::InputStream {
            name:     name_string,
            position: 0,
        };

        let capabilities = WasiResourceCapabilities {
            readable:        true,
            writable:        false,
            seekable:        false,
            metadata_access: false,
        };

        self.create_resource(resource_type, capabilities)
    }

    /// Create an output stream resource
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream name exceeds the maximum length (64 characters)
    /// - The resource table is full or allocation fails
    pub fn create_output_stream(&mut self, name: &str) -> Result<WasiHandle> {
        let name_string = BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Resource,
                codes::WASI_RESOURCE_LIMIT,
                "Invalid WASI handle",
            )
        })?;

        let resource_type = WasiResourceType::OutputStream {
            name:     name_string,
            position: 0,
        };

        let capabilities = WasiResourceCapabilities {
            readable:        false,
            writable:        true,
            seekable:        false,
            metadata_access: false,
        };

        self.create_resource(resource_type, capabilities)
    }

    /// Create a clock handle resource
    ///
    /// # Errors
    ///
    /// Returns an error if the resource table is full or allocation fails.
    pub fn create_clock_handle(&mut self, clock_type: WasiClockType) -> Result<WasiHandle> {
        let resource_type = WasiResourceType::ClockHandle { clock_type };

        let capabilities = WasiResourceCapabilities {
            readable:        true,
            writable:        false,
            seekable:        false,
            metadata_access: false,
        };

        self.create_resource(resource_type, capabilities)
    }

    /// Create a random handle resource
    ///
    /// # Errors
    ///
    /// Returns an error if the resource table is full or allocation fails.
    pub fn create_random_handle(&mut self, secure: bool) -> Result<WasiHandle> {
        let resource_type = WasiResourceType::RandomHandle { secure };

        let capabilities = WasiResourceCapabilities {
            readable:        true,
            writable:        false,
            seekable:        false,
            metadata_access: false,
        };

        self.create_resource(resource_type, capabilities)
    }
}

impl WasiResource {
    /// Get the WASI resource type
    pub fn resource_type(&self) -> &WasiResourceType {
        &self.resource_type
    }

    /// Get the resource capabilities
    pub fn capabilities(&self) -> &WasiResourceCapabilities {
        &self.capabilities
    }

    /// Check if the resource can be read
    pub fn is_readable(&self) -> bool {
        self.capabilities.readable
    }

    /// Check if the resource can be written
    pub fn is_writable(&self) -> bool {
        self.capabilities.writable
    }

    /// Check if the resource supports seeking
    pub fn is_seekable(&self) -> bool {
        self.capabilities.seekable
    }

    /// Verify an operation is allowed on this resource
    ///
    /// # Errors
    ///
    /// This function is currently infallible and always returns `Ok`.
    /// Future implementations may return errors for unauthorized operations.
    pub fn verify_operation(&self, _operation: ResourceOperation) -> Result<()> {
        // TODO: Implement operation verification when available in wrt-foundation
        Ok(())
    }

    /// Update stream position (for stream resources)
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is not a stream type (input or output stream).
    pub fn update_position(&mut self, new_position: u64) -> Result<()> {
        match &mut self.resource_type {
            WasiResourceType::InputStream { position, .. }
            | WasiResourceType::OutputStream { position, .. } => {
                *position = new_position;
                Ok(())
            },
            _ => Err(Error::runtime_execution_error(
                "Invalid resource type for file operations",
            )),
        }
    }

    /// Get current position (for stream resources)
    pub fn get_position(&self) -> Option<u64> {
        match &self.resource_type {
            WasiResourceType::InputStream { position, .. }
            | WasiResourceType::OutputStream { position, .. } => Some(*position),
            _ => None,
        }
    }
}


// Implement required traits for WasiResource to work with BoundedMap
impl Checksummable for WasiResource {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Simple checksum based on resource type
        match &self.resource_type {
            WasiResourceType::Null => checksum.update_slice(b"null"),
            WasiResourceType::FileDescriptor {
                path,
                readable,
                writable,
            } => {
                checksum.update_slice(b"file");
                if let Ok(path_str) = path.as_str() {
                    checksum.update_slice(path_str.as_bytes());
                }
                checksum.update_slice(&[u8::from(*readable), u8::from(*writable)]);
            },
            WasiResourceType::DirectoryHandle { path } => {
                checksum.update_slice(b"dir");
                if let Ok(path_str) = path.as_str() {
                    checksum.update_slice(path_str.as_bytes());
                }
            },
            WasiResourceType::InputStream { name, position } => {
                checksum.update_slice(b"in");
                if let Ok(name_str) = name.as_str() {
                    checksum.update_slice(name_str.as_bytes());
                }
                checksum.update_slice(&position.to_le_bytes());
            },
            WasiResourceType::OutputStream { name, position } => {
                checksum.update_slice(b"out");
                if let Ok(name_str) = name.as_str() {
                    checksum.update_slice(name_str.as_bytes());
                }
                checksum.update_slice(&position.to_le_bytes());
            },
            WasiResourceType::ClockHandle { clock_type } => {
                checksum.update_slice(b"clock");
                checksum.update_slice(&[*clock_type as u8]);
            },
            WasiResourceType::RandomHandle { secure } => {
                checksum.update_slice(b"random");
                checksum.update_slice(&[u8::from(*secure)]);
            },
        }
    }
}

impl ToBytes for WasiResource {
    fn serialized_size(&self) -> usize {
        // Estimate: discriminant(1) + capabilities(4) + type-specific data (up to 256+8)
        270
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'_>,
        provider: &P,
    ) -> Result<()> {
        // Write capabilities first (4 bools as bytes)
        writer.write_u8(u8::from(self.capabilities.readable))?;
        writer.write_u8(u8::from(self.capabilities.writable))?;
        writer.write_u8(u8::from(self.capabilities.seekable))?;
        writer.write_u8(u8::from(self.capabilities.metadata_access))?;

        // Write resource type discriminant and data
        match &self.resource_type {
            WasiResourceType::Null => writer.write_u8(0)?,
            WasiResourceType::FileDescriptor { path, readable, writable } => {
                writer.write_u8(1)?;
                path.to_bytes_with_provider(writer, provider)?;
                writer.write_u8(u8::from(*readable))?;
                writer.write_u8(u8::from(*writable))?;
            },
            WasiResourceType::DirectoryHandle { path } => {
                writer.write_u8(2)?;
                path.to_bytes_with_provider(writer, provider)?;
            },
            WasiResourceType::InputStream { name, position } => {
                writer.write_u8(3)?;
                name.to_bytes_with_provider(writer, provider)?;
                writer.write_u64_le(*position)?;
            },
            WasiResourceType::OutputStream { name, position } => {
                writer.write_u8(4)?;
                name.to_bytes_with_provider(writer, provider)?;
                writer.write_u64_le(*position)?;
            },
            WasiResourceType::ClockHandle { clock_type } => {
                writer.write_u8(5)?;
                writer.write_u8(*clock_type as u8)?;
            },
            WasiResourceType::RandomHandle { secure } => {
                writer.write_u8(6)?;
                writer.write_u8(u8::from(*secure))?;
            },
        }
        Ok(())
    }
}

impl FromBytes for WasiResource {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'_>,
        provider: &P,
    ) -> Result<Self> {
        // Read capabilities first (4 bools as bytes)
        let capabilities = WasiResourceCapabilities {
            readable: reader.read_u8()? != 0,
            writable: reader.read_u8()? != 0,
            seekable: reader.read_u8()? != 0,
            metadata_access: reader.read_u8()? != 0,
        };

        // Read resource type discriminant and data
        // 0/unknown discriminants fall through to wildcard which returns Null
        let discriminant = reader.read_u8()?;
        let resource_type = match discriminant {
            1 => {
                let path = BoundedString::from_bytes_with_provider(reader, provider)?;
                let readable = reader.read_u8()? != 0;
                let writable = reader.read_u8()? != 0;
                WasiResourceType::FileDescriptor { path, readable, writable }
            },
            2 => {
                let path = BoundedString::from_bytes_with_provider(reader, provider)?;
                WasiResourceType::DirectoryHandle { path }
            },
            3 => {
                let name = BoundedString::from_bytes_with_provider(reader, provider)?;
                let position = reader.read_u64_le()?;
                WasiResourceType::InputStream { name, position }
            },
            4 => {
                let name = BoundedString::from_bytes_with_provider(reader, provider)?;
                let position = reader.read_u64_le()?;
                WasiResourceType::OutputStream { name, position }
            },
            5 => {
                let clock_byte = reader.read_u8()?;
                // 0/unknown clock_byte falls through to wildcard which returns Realtime
                let clock_type = match clock_byte {
                    1 => WasiClockType::Monotonic,
                    2 => WasiClockType::ProcessCpuTime,
                    3 => WasiClockType::ThreadCpuTime,
                    _ => WasiClockType::Realtime,
                };
                WasiResourceType::ClockHandle { clock_type }
            },
            6 => {
                let secure = reader.read_u8()? != 0;
                WasiResourceType::RandomHandle { secure }
            },
            _ => WasiResourceType::Null,
        };

        Ok(WasiResource {
            base: Resource::new(
                0, // placeholder ID
                wrt_foundation::resource::ResourceRepr::Opaque,
                None,
                wrt_foundation::verification::VerificationLevel::Standard,
            ),
            resource_type,
            capabilities,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wrt_foundation::memory_init::MemoryInitializer;

    #[test]
    fn test_resource_manager_creation() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let manager = WasiResourceManager::new()?;
        assert_eq!(manager.resource_count(), 0);
        Ok(())
    }

    #[test]
    fn test_file_descriptor_creation() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let mut manager = WasiResourceManager::new()?;

        let fd = manager.create_file_descriptor("/tmp/test.txt", true, false)?;
        assert!(manager.is_valid_handle(fd));
        assert_eq!(manager.resource_count(), 1);

        let resource = manager.get_resource(fd)?;
        assert!(resource.is_readable());
        assert!(!resource.is_writable());

        Ok(())
    }

    #[test]
    fn test_stream_creation() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let mut manager = WasiResourceManager::new()?;

        let input = manager.create_input_stream("stdin")?;
        let output = manager.create_output_stream("stdout")?;

        assert!(manager.is_valid_handle(input));
        assert!(manager.is_valid_handle(output));
        assert_eq!(manager.resource_count(), 2);

        let input_resource = manager.get_resource(input)?;
        let output_resource = manager.get_resource(output)?;

        assert!(input_resource.is_readable());
        assert!(!input_resource.is_writable());
        assert!(!output_resource.is_readable());
        assert!(output_resource.is_writable());

        Ok(())
    }

    #[test]
    fn test_resource_removal() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let mut manager = WasiResourceManager::new()?;

        let fd = manager.create_file_descriptor("/tmp/test.txt", true, true)?;
        assert_eq!(manager.resource_count(), 1);

        let removed = manager.remove_resource(fd)?;
        assert_eq!(manager.resource_count(), 0);
        assert!(!manager.is_valid_handle(fd));

        match removed.resource_type() {
            WasiResourceType::FileDescriptor {
                readable, writable, ..
            } => {
                assert!(*readable);
                assert!(*writable);
            },
            _ => panic!("Expected file descriptor"),
        }

        Ok(())
    }

    #[test]
    fn test_invalid_handle_access() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let manager = WasiResourceManager::new()?;

        let result = manager.get_resource(999);
        assert!(result.is_err());

        if let Err(error) = result {
            assert_eq!(error.code(), codes::WASI_INVALID_FD);
        }

        Ok(())
    }

    #[test]
    fn test_clock_handle_creation() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let mut manager = WasiResourceManager::new()?;

        let monotonic = manager.create_clock_handle(WasiClockType::Monotonic)?;
        let realtime = manager.create_clock_handle(WasiClockType::Realtime)?;

        assert_ne!(monotonic, realtime);
        assert_eq!(manager.resource_count(), 2);

        Ok(())
    }
}
