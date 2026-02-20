//! Resource Representation (canon resource.rep) Implementation
//!
//! This module implements the `canon resource.rep` built-in for getting the
//! underlying representation of resource handles in the Component Model.

#[cfg(not(feature = "std"))]
use core::{fmt, mem, any::TypeId};
#[cfg(feature = "std")]
use std::{fmt, mem, any::TypeId};

#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec, collections::HashMap};

use wrt_foundation::{
    collections::StaticVec,
    bounded::BoundedString,
    prelude::*,
};

use crate::{
    borrowed_handles::{OwnHandle, BorrowHandle, HandleLifetimeTracker},
    resource_lifecycle_management::{ResourceId, ComponentId, ResourceType},
    types::{ValType, Value},
};

use wrt_error::{Error, ErrorCategory, Result};

/// Maximum number of resource representations in no_std
const MAX_RESOURCE_REPRESENTATIONS: usize = 256;

/// Resource representation manager
#[derive(Debug)]
pub struct ResourceRepresentationManager {
    /// Resource representations by type
    #[cfg(feature = "std")]
    representations: HashMap<TypeId, Box<dyn ResourceRepresentation>>,
    #[cfg(not(any(feature = "std", )))]
    representations: StaticVec<(TypeId, ResourceRepresentationEntry), MAX_RESOURCE_REPRESENTATIONS>,

    /// Handle to resource mapping
    #[cfg(feature = "std")]
    handle_to_resource: HashMap<u32, ResourceEntry>,
    #[cfg(not(any(feature = "std", )))]
    handle_to_resource: StaticVec<(u32, ResourceEntry), MAX_RESOURCE_REPRESENTATIONS>,
    
    /// Next representation ID
    next_representation_id: u32,
    
    /// Statistics
    stats: RepresentationStats,
}

/// Resource representation trait
pub trait ResourceRepresentation: fmt::Debug + Send + Sync {
    /// Get the underlying representation of a resource handle
    fn get_representation(&self, handle: u32) -> Result<RepresentationValue>;
    
    /// Set the underlying representation of a resource handle
    fn set_representation(&mut self, handle: u32, value: RepresentationValue) -> Result<()>;
    
    /// Get the type name this representation handles
    fn type_name(&self) -> &str;
    
    /// Get the size of the representation in bytes
    fn representation_size(&self) -> usize;
    
    /// Check if a handle is valid for this representation
    fn is_valid_handle(&self, handle: u32) -> bool;
    
    /// Clone the representation (for no_std compatibility)
    fn clone_representation(&self) -> Box<dyn ResourceRepresentation>;
}

/// Value that represents the underlying resource data
#[derive(Debug, Clone)]
pub enum RepresentationValue {
    /// 32-bit unsigned integer (e.g., file descriptor, object ID)
    U32(u32),
    
    /// 64-bit unsigned integer (e.g., pointer, large ID)
    U64(u64),
    
    /// Byte array (e.g., UUID, hash, binary data)
    Bytes(Vec<u8>),
    
    /// String representation (e.g., URL, path, name)
    String(String),
    
    /// Structured representation with multiple fields
    Structured(Vec<(String, RepresentationValue)>),
    
    /// Opaque pointer (platform-specific)
    Pointer(usize),
    
    /// Handle to another resource
    Handle(u32),
}

/// Entry for resource in the manager
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    /// Resource ID
    pub resource_id: ResourceId,
    
    /// Resource type
    pub resource_type: ResourceType,
    
    /// Owning component
    pub owner: ComponentId,
    
    /// Type ID for representation lookup
    pub type_id: TypeId,
    
    /// Handle value
    pub handle: u32,
    
    /// Current representation
    pub representation: RepresentationValue,
    
    /// Metadata
    pub metadata: ResourceMetadata,
}

/// Metadata about a resource representation
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Type name
    pub type_name: BoundedString<64>,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Last access timestamp
    pub last_accessed: u64,
    
    /// Access count
    pub access_count: u64,
    
    /// Whether representation can be modified
    pub mutable: bool,
}

/// Statistics for resource representations
#[derive(Debug, Clone)]
pub struct RepresentationStats {
    /// Total representations registered
    pub representations_registered: u32,
    
    /// Total get operations
    pub get_operations: u64,
    
    /// Total set operations
    pub set_operations: u64,
    
    /// Total validation checks
    pub validation_checks: u64,
    
    /// Failed operations
    pub failed_operations: u64,
}

/// No-std compatible representation entry
#[cfg(not(any(feature = "std", )))]
#[derive(Debug)]
pub struct ResourceRepresentationEntry {
    /// Type ID
    pub type_id: TypeId,
    
    /// Representation implementation
    pub representation: ConcreteResourceRepresentation,
}

/// Concrete implementation for no_std environments
#[derive(Debug, Clone)]
pub struct ConcreteResourceRepresentation {
    /// Type name
    pub type_name: BoundedString<64>,
    
    /// Representation size
    pub size: usize,
    
    /// Valid handles
    pub valid_handles: StaticVec<u32, 64>,
    
    /// Handle to representation mapping
    pub handle_values: StaticVec<(u32, RepresentationValue), 64>,
}

/// Built-in representations for common types

/// File handle representation
#[derive(Debug, Clone)]
pub struct FileHandleRepresentation {
    /// Platform-specific file descriptors
    #[cfg(feature = "std")]
    file_descriptors: HashMap<u32, i32>,
    #[cfg(not(any(feature = "std", )))]
    file_descriptors: StaticVec<(u32, i32), 64>,
}

/// Memory buffer representation
#[derive(Debug, Clone)]
pub struct MemoryBufferRepresentation {
    /// Buffer pointers and sizes
    #[cfg(feature = "std")]
    buffers: HashMap<u32, (usize, usize)>, // (pointer, size)
    #[cfg(not(any(feature = "std", )))]
    buffers: StaticVec<(u32, (usize, usize)), 64>,
}

/// Network connection representation
#[derive(Debug, Clone)]
pub struct NetworkConnectionRepresentation {
    /// Connection details
    #[cfg(feature = "std")]
    connections: HashMap<u32, NetworkConnection>,
    #[cfg(not(any(feature = "std", )))]
    connections: StaticVec<(u32, NetworkConnection), 32>,
}

/// Network connection details
#[derive(Debug, Clone)]
pub struct NetworkConnection {
    /// Socket file descriptor or handle
    pub socket_fd: i32,
    
    /// Local address
    pub local_addr: BoundedString<64>,
    
    /// Remote address
    pub remote_addr: BoundedString<64>,
    
    /// Connection state
    pub state: ConnectionState,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    
    /// Connection is active
    Connected,
    
    /// Connection is being closed
    Closing,
    
    /// Connection is closed
    Closed,
    
    /// Connection failed
    Failed,
}

impl ResourceRepresentationManager {
    /// Create new resource representation manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            representations: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            representations: StaticVec::new(),
            
            #[cfg(feature = "std")]
            handle_to_resource: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            handle_to_resource: StaticVec::new(),
            
            next_representation_id: 1,
            stats: RepresentationStats::new(),
        })
    }
    
    /// Create with common built-in representations
    pub fn with_builtin_representations() -> wrt_error::Result<Self> {
        let mut manager = Self::new()?;
        
        // Register built-in representations
        let _ = manager.register_representation::<FileHandle>(Box::new(FileHandleRepresentation::new()?;
        let _ = manager.register_representation::<MemoryBuffer>(Box::new(MemoryBufferRepresentation::new()?;
        let _ = manager.register_representation::<NetworkHandle>(Box::new(NetworkConnectionRepresentation::new()?;
        
        Ok(manager)
    }
    
    /// Register a resource representation for a type
    pub fn register_representation<T: 'static>(
        &mut self,
        representation: Box<dyn ResourceRepresentation>,
    ) -> Result<()> {
        let type_id = TypeId::of::<T>);
        
        #[cfg(feature = "std")]
        {
            self.representations.insert(type_id, representation;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            // Convert to concrete representation for no_std
            let concrete = ConcreteResourceRepresentation {
                type_name: BoundedString::try_from_str(representation.type_name()).unwrap_or_default(),
                size: representation.representation_size(),
                valid_handles: StaticVec::new(),
                handle_values: StaticVec::new(),
            };
            
            let entry = ResourceRepresentationEntry {
                type_id,
                representation: concrete,
            };
            
            self.representations.push(entry).map_err(|_| {
                Error::runtime_execution_error("Resource operation failed")
            })?;
        }
        
        self.stats.representations_registered += 1;
        Ok(()
    }
    
    /// Get the representation of a resource handle
    pub fn get_resource_representation(&mut self, handle: u32) -> Result<RepresentationValue> {
        // Find the resource entry
        let resource_entry = self.find_resource_entry(handle)?;
        let type_id = resource_entry.type_id;
        
        // Find the representation
        #[cfg(feature = "std")]
        {
            let representation = self.representations.get(&type_id)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            let result = representation.get_representation(handle;
            self.stats.get_operations += 1;
            
            if result.is_ok() {
                // Update access metadata
                if let Ok(entry) = self.find_resource_entry_mut(handle) {
                    entry.metadata.last_accessed = self.get_current_time);
                    entry.metadata.access_count += 1;
                }
            } else {
                self.stats.failed_operations += 1;
            }
            
            result
        }
        #[cfg(not(any(feature = "std")))]
        {
            // Find representation entry
            let repr_entry = self.representations
                .iter()
                .find(|(tid, _)| *tid == type_id)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            // Find handle value
            let handle_value = repr_entry.1.representation.handle_values
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, v)| v.clone()
                .ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Runtime,
                        wrt_error::codes::EXECUTION_ERROR,
                        ")
                })?;
            
            self.stats.get_operations += 1;
            Ok(handle_value)
        }
    }
    
    /// Set the representation of a resource handle
    pub fn set_resource_representation(
        &mut self,
        handle: u32,
        value: RepresentationValue,
    ) -> Result<()> {
        // Find the resource entry
        let resource_entry = self.find_resource_entry(handle)?;
        let type_id = resource_entry.type_id;
        
        // Check if representation is mutable
        if !resource_entry.metadata.mutable {
            return Err(Error::runtime_execution_error("Resource validation failed"
            ;
        }
        
        // Find the representation
        #[cfg(feature = "std")]
        {
            let representation = self.representations.get_mut(&type_id)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            let result = representation.set_representation(handle, value.clone();
            self.stats.set_operations += 1;
            
            if result.is_ok() {
                // Update the cached representation
                if let Ok(entry) = self.find_resource_entry_mut(handle) {
                    entry.representation = value;
                    entry.metadata.last_accessed = self.get_current_time);
                }
            } else {
                self.stats.failed_operations += 1;
            }
            
            result
        }
        #[cfg(not(any(feature = "std")))]
        {
            // Find representation entry
            let repr_entry = self.representations
                .iter_mut()
                .find(|(tid, _)| *tid == type_id)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            // Update handle value
            if let Some((_, existing_value)) = repr_entry.1.representation.handle_values
                .iter_mut()
                .find(|(h, _)| *h == handle) {
                *existing_value = value.clone();
            } else {
                repr_entry.1.representation.handle_values.push((handle, value.clone())).map_err(|_| {
                    Error::new(
                        ErrorCategory::Resource,
                        wrt_error::codes::RESOURCE_EXHAUSTED,
                        ")
                })?;
            }
            
            // Update resource entry
            if let Ok(entry) = self.find_resource_entry_mut(handle) {
                entry.representation = value;
                entry.metadata.last_accessed = self.get_current_time);
            }
            
            self.stats.set_operations += 1;
            Ok(()
        }
    }
    
    /// Register a resource handle with its representation
    pub fn register_resource_handle(
        &mut self,
        handle: u32,
        resource_id: ResourceId,
        resource_type: ResourceType,
        owner: ComponentId,
        type_id: TypeId,
        initial_representation: RepresentationValue,
        mutable: bool,
    ) -> Result<()> {
        let metadata = ResourceMetadata {
            type_name: BoundedString::try_from_str(&"Component not found").unwrap_or_default(),
            created_at: self.get_current_time(),
            last_accessed: self.get_current_time(),
            access_count: 0,
            mutable,
        };
        
        let entry = ResourceEntry {
            resource_id,
            resource_type,
            owner,
            type_id,
            handle,
            representation: initial_representation,
            metadata,
        };
        
        #[cfg(feature = "std")]
        {
            self.handle_to_resource.insert(handle, entry;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.handle_to_resource.push((handle, entry)).map_err(|_| {
                Error::runtime_execution_error("Resource operation failed")
            })?;
        }
        
        Ok(()
    }
    
    /// Validate a resource handle
    pub fn validate_handle(&mut self, handle: u32) -> Result<bool> {
        self.stats.validation_checks += 1;
        
        let resource_entry = match self.find_resource_entry(handle) {
            Ok(entry) => entry,
            Err(_) => return Ok(false),
        };
        
        let type_id = resource_entry.type_id;
        
        #[cfg(feature = "std")]
        {
            if let Some(representation) = self.representations.get(&type_id) {
                Ok(representation.is_valid_handle(handle)
            } else {
                Ok(false)
            }
        }
        #[cfg(not(any(feature = "std", )))]
        {
            if let Some((_, repr_entry)) = self.representations.iter().find(|(tid, _)| *tid == type_id) {
                Ok(repr_entry.representation.valid_handles.iter().any(|&h| h == handle)
            } else {
                Ok(false)
            }
        }
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> &RepresentationStats {
        &self.stats
    }
    
    // Private helper methods
    
    fn find_resource_entry(&self, handle: u32) -> Result<&ResourceEntry> {
        #[cfg(feature = "std")]
        {
            self.handle_to_resource.get(&handle)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })
        }
        #[cfg(not(any(feature = "std")))]
        {
            self.handle_to_resource
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, entry)| entry)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })
        }
    }
    
    fn find_resource_entry_mut(&mut self, handle: u32) -> Result<&mut ResourceEntry> {
        #[cfg(feature = "std")]
        {
            self.handle_to_resource.get_mut(&handle)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })
        }
        #[cfg(not(any(feature = "std")))]
        {
            self.handle_to_resource
                .iter_mut()
                .find(|(h, _)| *h == handle)
                .map(|(_, entry)| entry)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })
        }
    }
    
    fn get_current_time(&self) -> u64 {
        // Simplified time implementation
        0
    }
}

// Built-in representation implementations

impl FileHandleRepresentation {
    /// Create new file handle representation
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            file_descriptors: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            file_descriptors: {
StaticVec::new()
            },
        })
    }
}

impl ResourceRepresentation for FileHandleRepresentation {
    fn get_representation(&self, handle: u32) -> Result<RepresentationValue> {
        #[cfg(feature = "std")]
        {
            let fd = self.file_descriptors.get(&handle)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            Ok(RepresentationValue::U32(*fd as u32)
        }
        #[cfg(not(any(feature = "std")))]
        {
            let fd = self.file_descriptors
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, fd)| *fd)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            Ok(RepresentationValue::U32(fd as u32)
        }
    }
    
    fn set_representation(&mut self, handle: u32, value: RepresentationValue) -> Result<()> {
        let fd = match value {
            RepresentationValue::U32(fd) => fd as i32,
            _ => return Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::EXECUTION_ERROR,
                "Error message needed")),
        };
        
        #[cfg(feature = "std")]
        {
            self.file_descriptors.insert(handle, fd;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            if let Some((_, existing_fd)) = self.file_descriptors.iter_mut().find(|(h, _)| *h == handle) {
                *existing_fd = fd;
            } else {
                self.file_descriptors.push((handle, fd)).map_err(|_| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            }
        }
        
        Ok(()
    }
    
    fn type_name(&self) -> &str {
        ") -> usize {
        4 // i32 file descriptor
    }
    
    fn is_valid_handle(&self, handle: u32) -> bool {
        #[cfg(feature = "std")]
        {
            self.file_descriptors.contains_key(&handle)
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.file_descriptors.iter().any(|(h, _)| *h == handle)
        }
    }
    
    fn clone_representation(&self) -> Box<dyn ResourceRepresentation> {
        Box::new(self.clone()
    }
}

impl MemoryBufferRepresentation {
    /// Create new memory buffer representation
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            buffers: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            buffers: {
StaticVec::new()
            },
        })
    }
}

impl ResourceRepresentation for MemoryBufferRepresentation {
    fn get_representation(&self, handle: u32) -> Result<RepresentationValue> {
        #[cfg(feature = "std")]
        {
            let (ptr, size) = self.buffers.get(&handle)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            Ok(RepresentationValue::Structured(vec![
                ("), RepresentationValue::U64(*ptr as u64)),
                ("size".to_owned(), RepresentationValue::U64(*size as u64)),
            ])
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let (ptr, size) = self.buffers
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, buf)| *buf)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            use wrt_foundation::budget_aware_provider::CrateId;
            use crate::bounded_component_infra::ComponentProvider;
            let mut fields = StaticVec::new();
            fields.push(("), RepresentationValue::U64(ptr as u64))).unwrap();
            fields.push(("size".to_owned(), RepresentationValue::U64(size as u64))).unwrap();

            Ok(RepresentationValue::Structured(fields.iter().cloned().collect())
        }
    }
    
    fn set_representation(&mut self, handle: u32, value: RepresentationValue) -> Result<()> {
        let (ptr, size) = match value {
            RepresentationValue::Structured(fields) => {
                let mut ptr = 0usize;
                let mut size = 0usize;
                
                for (key, val) in fields {
                    match (key.as_str(), val) {
                        ("pointer", RepresentationValue::U64(p)) => ptr = p as usize,
                        ("size", RepresentationValue::U64(s)) => size = s as usize,
                        _ => {}
                    }
                }
                
                (ptr, size)
            }
            _ => return Err(Error::runtime_execution_error("Error occurred"
            )),
        };
        
        #[cfg(feature = "std")]
        {
            self.buffers.insert(handle, (ptr, size;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            if let Some((_, existing_buf)) = self.buffers.iter_mut().find(|(h, _)| *h == handle) {
                *existing_buf = (ptr, size;
            } else {
                self.buffers.push((handle, (ptr, size))).map_err(|_| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            }
        }
        
        Ok(()
    }
    
    fn type_name(&self) -> &str {
        ") -> usize {
        16 // pointer + size (8 bytes each on 64-bit systems)
    }
    
    fn is_valid_handle(&self, handle: u32) -> bool {
        #[cfg(feature = "std")]
        {
            self.buffers.contains_key(&handle)
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.buffers.iter().any(|(h, _)| *h == handle)
        }
    }
    
    fn clone_representation(&self) -> Box<dyn ResourceRepresentation> {
        Box::new(self.clone()
    }
}

impl NetworkConnectionRepresentation {
    /// Create new network connection representation
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            connections: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            connections: {
StaticVec::new()
            },
        })
    }
}

impl ResourceRepresentation for NetworkConnectionRepresentation {
    fn get_representation(&self, handle: u32) -> Result<RepresentationValue> {
        #[cfg(feature = "std")]
        {
            let conn = self.connections.get(&handle)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            Ok(RepresentationValue::Structured(vec![
                ("), RepresentationValue::U32(conn.socket_fd as u32)),
                ("local_addr".to_owned(), RepresentationValue::String(conn.local_addr.to_string())),
                ("remote_addr".to_owned(), RepresentationValue::String(conn.remote_addr.to_string())),
                ("state".to_owned(), RepresentationValue::U32(conn.state as u32)),
            ])
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let conn = self.connections
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, conn)| conn)
                .ok_or_else(|| {
                    Error::runtime_execution_error("Resource lookup failed"
                })?;
                })?;
            
            use wrt_foundation::budget_aware_provider::CrateId;
            use crate::bounded_component_infra::ComponentProvider;
            let mut fields = StaticVec::new();
            fields.push(("), RepresentationValue::U32(conn.socket_fd as u32))).unwrap();
            fields.push(("local_addr".to_owned(), RepresentationValue::String(conn.local_addr.to_string()))).unwrap();
            fields.push(("remote_addr".to_owned(), RepresentationValue::String(conn.remote_addr.to_string()))).unwrap();
            fields.push(("state".to_owned(), RepresentationValue::U32(conn.state as u32))).unwrap();

            Ok(RepresentationValue::Structured(fields.iter().cloned().collect())
        }
    }
    
    fn set_representation(&mut self, _handle: u32, _value: RepresentationValue) -> Result<()> {
        // Network connections are typically read-only
        Err(Error::runtime_execution_error("Error occurred"
        )
    }
    
    fn type_name(&self) -> &str {
        ") -> usize {
        256 // Estimated size for connection details
    }
    
    fn is_valid_handle(&self, handle: u32) -> bool {
        #[cfg(feature = "std")]
        {
            self.connections.contains_key(&handle)
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.connections.iter().any(|(h, _)| *h == handle)
        }
    }
    
    fn clone_representation(&self) -> Box<dyn ResourceRepresentation> {
        Box::new(self.clone()
    }
}

impl RepresentationStats {
    /// Create new representation statistics
    pub fn new() -> Self {
        Self {
            representations_registered: 0,
            get_operations: 0,
            set_operations: 0,
            validation_checks: 0,
            failed_operations: 0,
        }
    }
}

impl Default for ResourceRepresentationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RepresentationStats {
    fn default() -> Self {
        Self::new()
    }
}

// Type markers for built-in resource types
#[derive(Debug)]
pub struct FileHandle;

#[derive(Debug)]
pub struct MemoryBuffer;

#[derive(Debug)]
pub struct NetworkHandle;

/// Canonical ABI built-in: `canon resource.rep`
pub fn canon_resource_rep(
    manager: &mut ResourceRepresentationManager,
    handle: u32,
) -> Result<RepresentationValue> {
    manager.get_resource_representation(handle)
}

/// Canonical ABI built-in: `canon resource.new` (for dynamic resource creation)
pub fn canon_resource_new<T: 'static>(
    manager: &mut ResourceRepresentationManager,
    resource_id: ResourceId,
    owner: ComponentId,
    initial_representation: RepresentationValue,
) -> Result<u32> {
    let handle = manager.next_representation_id;
    manager.next_representation_id += 1;
    
    let type_id = TypeId::of::<T>);
    let resource_type = ResourceType::Custom(type_id.into()); // Simplified
    
    manager.register_resource_handle(
        handle,
        resource_id,
        resource_type,
        owner,
        type_id,
        initial_representation,
        true, // mutable by default
    )?;
    
    Ok(handle)
}

/// Canonical ABI built-in: `canon resource.drop`
pub fn canon_resource_drop(
    manager: &mut ResourceRepresentationManager,
    handle: u32,
) -> Result<()> {
    // In a full implementation, this would:
    // 1. Validate the handle
    // 2. Call any drop handlers
    // 3. Remove the handle from the manager
    // 4. Free any associated resources
    
    if !manager.validate_handle(handle)? {
        return Err(Error::runtime_execution_error("Error occurred"
        ;
    }
    
    // Remove from handle mapping
    #[cfg(feature = "std")]
    {
        manager.handle_to_resource.remove(&handle;
    }
    #[cfg(not(any(feature = "std", )))]
    {
        let mut i = 0;
        while i < manager.handle_to_resource.len() {
            if manager.handle_to_resource[i].0 == handle {
                manager.handle_to_resource.remove(i;
                break;
            } else {
                i += 1;
            }
        }
    }
    
    Ok(()
}

}
