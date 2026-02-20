//! Resource Management System for WebAssembly Component Model
//!
//! This module provides comprehensive resource management functionality for the
//! WebAssembly Component Model, implementing the resource system as specified
//! in the Component Model specification.
//!
//! # Features
//!
//! - **Resource Handle Management**: Creation, tracking, and cleanup of resource handles
//! - **Ownership Semantics**: Proper own/borrow semantics for resource transfer
//! - **Cross-Component Sharing**: Safe resource transfer between component instances
//! - **Lifecycle Management**: Automatic cleanup and finalization
//! - **Cross-Environment Support**: Works in std, no_std+alloc, and pure no_std
//! - **Memory Safety**: Comprehensive validation and bounds checking
//! - **Performance Optimized**: Efficient resource operations with minimal overhead
//!
//! # Core Concepts
//!
//! - **Resource**: A typed, opaque value managed by the runtime
//! - **Handle**: A unique identifier for a resource instance
//! - **ResourceTable**: Container for managing resource handles within an instance
//! - **Ownership**: Resources can be owned or borrowed across component boundaries
//! - **Finalization**: Automatic cleanup when resources are no longer needed
//!
//! # Example
//!
//! ```no_run
//! use wrt_component::resource_management::{ResourceManager, ResourceType, ResourceHandle};
//!
//! // Create a resource manager
//! let mut manager = ResourceManager::new();
//!
//! // Register a resource type
//! let file_type = manager.register_resource_type("file")?;
//!
//! // Create a resource instance
//! let file_handle = manager.create_resource(file_type, file_data)?;
//!
//! // Transfer ownership to another component
//! manager.transfer_ownership(file_handle, target_instance_id)?;
//! ```


// Cross-environment imports
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, format, string::String, vec::Vec};

#[cfg(all(not(feature = "std")))]
use std::{boxed::Box, collections::BTreeMap as HashMap, format, string::String, vec::Vec};

#[cfg(not(any(feature = "std", )))]
use wrt_foundation::{BoundedString as String, BoundedVec as Vec, NoStdHashMap as HashMap};

use crate::component_instantiation::InstanceId;
use wrt_error::{codes, Error, ErrorCategory, Result};

/// Maximum number of resource types
const MAX_RESOURCE_TYPES: usize = 1024;

/// Maximum number of resources per instance
const MAX_RESOURCES_PER_INSTANCE: usize = 65536;

/// Maximum number of resource handles globally
const MAX_GLOBAL_RESOURCES: usize = 1024 * 1024;

/// Invalid resource handle constant
pub const INVALID_HANDLE: ResourceHandle = ResourceHandle(u32::MAX);

/// Resource handle - unique identifier for a resource instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceHandle(pub u32);

/// Resource type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceTypeId(pub u32);

/// Resource instance state
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceState {
    /// Resource is active and available
    Active,
    /// Resource is borrowed by another component
    Borrowed {
        /// Instance that borrowed the resource
        borrower: InstanceId,
        /// Borrow timestamp
        borrowed_at: u64,
    },
    /// Resource is being finalized
    Finalizing,
    /// Resource has been dropped/destroyed
    Dropped,
}

/// Resource ownership model
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceOwnership {
    /// Resource is owned by the instance
    Owned,
    /// Resource is borrowed from another instance
    Borrowed {
        /// Owner instance
        owner: InstanceId,
        /// Original handle in owner's table
        owner_handle: ResourceHandle,
    },
}

/// Resource type definition
#[derive(Debug, Clone)]
pub struct ResourceType {
    /// Unique type identifier
    pub id: ResourceTypeId,
    /// Human-readable type name
    pub name: String,
    /// Type description
    pub description: String,
    /// Whether resources of this type support borrowing
    pub borrowable: bool,
    /// Whether resources require explicit finalization
    pub needs_finalization: bool,
    /// Maximum number of instances of this type
    pub max_instances: Option<u32>,
    /// Type-specific metadata
    pub metadata: ResourceTypeMetadata,
}

/// Resource type metadata
#[derive(Debug, Clone)]
pub struct ResourceTypeMetadata {
    /// Size hint for the resource (in bytes)
    pub size_hint: Option<u32>,
    /// Alignment requirements
    pub alignment: u32,
    /// Custom metadata fields
    pub custom_fields: HashMap<String, String>,
}

/// Resource instance
#[derive(Debug, Clone)]
pub struct Resource {
    /// Resource handle
    pub handle: ResourceHandle,
    /// Resource type
    pub resource_type: ResourceTypeId,
    /// Current state
    pub state: ResourceState,
    /// Ownership information
    pub ownership: ResourceOwnership,
    /// Instance that owns this resource
    pub owner_instance: InstanceId,
    /// Creation timestamp
    pub created_at: u64,
    /// Last access timestamp
    pub last_accessed: u64,
    /// Reference count for borrowed resources
    pub ref_count: u32,
    /// Resource data (opaque to the runtime)
    pub data: ResourceData,
}

/// Resource data storage
#[derive(Debug, Clone)]
pub enum ResourceData {
    /// No data (placeholder)
    Empty,
    /// Byte data
    Bytes(Vec<u8>),
    /// Handle to external resource
    ExternalHandle(u64),
    /// Custom data with type information
    Custom {
        /// Data type identifier
        type_id: String,
        /// Serialized data
        data: Vec<u8>,
    },
}

/// Resource table for managing resources within a component instance
#[derive(Debug, Clone)]
pub struct ResourceTable {
    /// Instance that owns this table
    pub instance_id: InstanceId,
    /// Resources in this table
    resources: HashMap<ResourceHandle, Resource>,
    /// Next available handle
    next_handle: u32,
    /// Resource type mappings
    type_mappings: HashMap<ResourceTypeId, Vec<ResourceHandle>>,
    /// Table statistics
    stats: ResourceTableStats,
}

/// Resource table statistics
#[derive(Debug, Clone, Default)]
pub struct ResourceTableStats {
    /// Total resources created
    pub resources_created: u64,
    /// Total resources dropped
    pub resources_dropped: u64,
    /// Current active resources
    pub active_resources: u32,
    /// Current borrowed resources
    pub borrowed_resources: u32,
    /// Peak resource count
    pub peak_resources: u32,
    /// Total finalization operations
    pub finalizations: u64,
}

/// Global resource manager
#[derive(Debug, Clone)]
pub struct ResourceManager {
    /// Registered resource types
    resource_types: HashMap<ResourceTypeId, ResourceType>,
    /// Next available type ID
    next_type_id: u32,
    /// Global resource registry
    global_resources: HashMap<ResourceHandle, InstanceId>,
    /// Next global handle
    next_global_handle: u32,
    /// Resource tables by instance
    instance_tables: HashMap<InstanceId, ResourceTable>,
    /// Manager configuration
    config: ResourceManagerConfig,
    /// Global statistics
    stats: ResourceManagerStats,
}

/// Resource manager configuration
#[derive(Debug, Clone)]
pub struct ResourceManagerConfig {
    /// Enable automatic garbage collection
    pub auto_gc: bool,
    /// GC interval in operations
    pub gc_interval: u32,
    /// Enable resource borrowing
    pub allow_borrowing: bool,
    /// Maximum borrow duration (microseconds)
    pub max_borrow_duration: u64,
    /// Enable cross-instance resource sharing
    pub allow_cross_instance_sharing: bool,
    /// Resource validation level
    pub validation_level: ResourceValidationLevel,
}

/// Resource validation levels
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceValidationLevel {
    /// No validation
    None,
    /// Basic validation
    Basic,
    /// Full validation with type checking
    Full,
    /// Paranoid validation for debugging
    Paranoid,
}

/// Global resource manager statistics
#[derive(Debug, Clone, Default)]
pub struct ResourceManagerStats {
    /// Total resource types registered
    pub types_registered: u32,
    /// Total instances managed
    pub instances_managed: u32,
    /// Total global resources
    pub global_resources: u32,
    /// Total cross-instance transfers
    pub cross_instance_transfers: u64,
    /// Total garbage collections
    pub garbage_collections: u64,
    /// Last GC timestamp
    pub last_gc_at: u64,
}

/// Resource operation errors
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceError {
    /// Resource handle not found
    HandleNotFound(ResourceHandle),
    /// Resource type not found
    TypeNotFound(ResourceTypeId),
    /// Invalid resource state for operation
    InvalidState(ResourceHandle, ResourceState),
    /// Resource access denied
    AccessDenied(ResourceHandle),
    /// Resource limit exceeded
    LimitExceeded(String),
    /// Type mismatch
    TypeMismatch(String),
    /// Ownership violation
    OwnershipViolation(String),
    /// Resource already exists
    AlreadyExists(ResourceHandle),
}

impl Default for ResourceManagerConfig {
    fn default() -> Self {
        Self {
            auto_gc: true,
            gc_interval: 1000,
            allow_borrowing: true,
            max_borrow_duration: 30_000_000, // 30 seconds
            allow_cross_instance_sharing: true,
            validation_level: ResourceValidationLevel::Full,
        }
    }
}

impl Default for ResourceTypeMetadata {
    fn default() -> Self {
        Self { size_hint: None, alignment: 1, custom_fields: HashMap::new() }
    }
}

impl ResourceHandle {
    /// Create a new resource handle
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the raw handle value
    pub fn value(self) -> u32 {
        self.0
    }

    /// Check if handle is valid
    pub fn is_valid(self) -> bool {
        self != INVALID_HANDLE
    }
}

impl ResourceTypeId {
    /// Create a new resource type ID
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the raw type ID value
    pub fn value(self) -> u32 {
        self.0
    }
}

impl ResourceTable {
    /// Create a new resource table
    pub fn new(instance_id: InstanceId) -> Self {
        Self {
            instance_id,
            resources: HashMap::new(),
            next_handle: 1,
            type_mappings: HashMap::new(),
            stats: ResourceTableStats::default(),
        }
    }

    /// Create a new resource in this table
    pub fn create_resource(
        &mut self,
        resource_type: ResourceTypeId,
        data: ResourceData,
        ownership: ResourceOwnership,
    ) -> Result<ResourceHandle> {
        if self.resources.len() >= MAX_RESOURCES_PER_INSTANCE {
            return Err(Error::resource_exhausted("Maximum resources per instance exceeded";
        }

        let handle = ResourceHandle::new(self.next_handle;
        self.next_handle += 1;

        let resource = Resource {
            handle,
            resource_type,
            state: ResourceState::Active,
            ownership,
            owner_instance: self.instance_id,
            created_at: 0, // Would use actual timestamp
            last_accessed: 0,
            ref_count: 1,
            data,
        };

        self.resources.insert(handle, resource;

        // Update type mappings
        self.type_mappings.entry(resource_type).or_insert_with(Vec::new).push(handle);

        // Update statistics
        self.stats.resources_created += 1;
        self.stats.active_resources += 1;
        if self.stats.active_resources > self.stats.peak_resources {
            self.stats.peak_resources = self.stats.active_resources;
        }

        Ok(handle)
    }

    /// Get a resource by handle
    pub fn get_resource(&self, handle: ResourceHandle) -> Option<&Resource> {
        self.resources.get(&handle)
    }

    /// Get a mutable resource by handle
    pub fn get_resource_mut(&mut self, handle: ResourceHandle) -> Option<&mut Resource> {
        if let Some(resource) = self.resources.get_mut(&handle) {
            resource.last_accessed = 0; // Would use actual timestamp
            Some(resource)
        } else {
            None
        }
    }

    /// Drop a resource from this table
    pub fn drop_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        let resource = self.resources.remove(&handle).ok_or_else(|| {
            Error::resource_not_found("Resource handle not found")
        })?;

        // Update type mappings
        if let Some(handles) = self.type_mappings.get_mut(&resource.resource_type) {
            handles.retain(|&h| h != handle;
        }

        // Update statistics
        self.stats.resources_dropped += 1;
        if self.stats.active_resources > 0 {
            self.stats.active_resources -= 1;
        }

        Ok(()
    }

    /// Borrow a resource to another instance
    pub fn borrow_resource(&mut self, handle: ResourceHandle, borrower: InstanceId) -> Result<()> {
        let resource = self.resources.get_mut(&handle).ok_or_else(|| {
            Error::resource_not_found("Resource handle not found")
        })?;

        match resource.state {
            ResourceState::Active => {
                resource.state = ResourceState::Borrowed {
                    borrower,
                    borrowed_at: 0, // Would use actual timestamp
                };
                resource.ref_count += 1;
                self.stats.borrowed_resources += 1;
                Ok(()
            }
            _ => Err(Error::runtime_invalid_state("Resource not in a borrowable state"),
            )),
        }
    }

    /// Return a borrowed resource
    pub fn return_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        let resource = self.resources.get_mut(&handle).ok_or_else(|| {
            Error::resource_not_found("Resource handle not found")
        })?;

        match resource.state {
            ResourceState::Borrowed { .. } => {
                if resource.ref_count > 1 {
                    resource.ref_count -= 1;
                } else {
                    resource.state = ResourceState::Active;
                    resource.ref_count = 1;
                    if self.stats.borrowed_resources > 0 {
                        self.stats.borrowed_resources -= 1;
                    }
                }
                Ok(()
            }
            _ => Err(Error::runtime_invalid_state("Resource is not borrowed"),
            )),
        }
    }

    /// Get all resources of a specific type
    pub fn get_resources_by_type(&self, resource_type: ResourceTypeId) -> Vec<ResourceHandle> {
        self.type_mappings
            .get(&resource_type)
            .map(|handles| handles.clone()
            .unwrap_or_else(Vec::new)
    }

    /// Get table statistics
    pub fn get_stats(&self) -> &ResourceTableStats {
        &self.stats
    }

    /// Cleanup expired resources
    pub fn cleanup_expired(&mut self, max_age: u64) -> Result<u32> {
        let current_time = 0; // Would use actual timestamp
        let mut cleaned = 0;

        let expired_handles: Vec<ResourceHandle> = self
            .resources
            .iter()
            .filter(|(_, resource)| {
                matches!(resource.state, ResourceState::Dropped)
                    || (current_time - resource.last_accessed > max_age)
            })
            .map(|(&handle, _)| handle)
            .collect();

        for handle in expired_handles {
            self.drop_resource(handle)?;
            cleaned += 1;
        }

        Ok(cleaned)
    }

    /// Clear all resources (for instance termination)
    pub fn clear_all(&mut self) {
        let handle_count = self.resources.len() as u64;
        self.resources.clear);
        self.type_mappings.clear);
        self.stats.resources_dropped += handle_count;
        self.stats.active_resources = 0;
        self.stats.borrowed_resources = 0;
    }
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new() -> Self {
        Self::with_config(ResourceManagerConfig::default()
    }

    /// Create a new resource manager with custom configuration
    pub fn with_config(config: ResourceManagerConfig) -> Self {
        Self {
            resource_types: HashMap::new(),
            next_type_id: 1,
            global_resources: HashMap::new(),
            next_global_handle: 1,
            instance_tables: HashMap::new(),
            config,
            stats: ResourceManagerStats::default(),
        }
    }

    /// Register a new resource type
    pub fn register_resource_type(
        &mut self,
        name: String,
        description: String,
        borrowable: bool,
        needs_finalization: bool,
    ) -> Result<ResourceTypeId> {
        if self.resource_types.len() >= MAX_RESOURCE_TYPES {
            return Err(Error::resource_exhausted("Maximum resource types exceeded";
        }

        let type_id = ResourceTypeId::new(self.next_type_id;
        self.next_type_id += 1;

        let resource_type = ResourceType {
            id: type_id,
            name,
            description,
            borrowable,
            needs_finalization,
            max_instances: None,
            metadata: ResourceTypeMetadata::default(),
        };

        self.resource_types.insert(type_id, resource_type;
        self.stats.types_registered += 1;

        Ok(type_id)
    }

    /// Get a resource type by ID
    pub fn get_resource_type(&self, type_id: ResourceTypeId) -> Option<&ResourceType> {
        self.resource_types.get(&type_id)
    }

    /// Create a resource table for an instance
    pub fn create_instance_table(&mut self, instance_id: InstanceId) -> Result<()> {
        if self.instance_tables.contains_key(&instance_id) {
            return Err(Error::runtime_execution_error("Error occurred",
            ;
        }

        let table = ResourceTable::new(instance_id;
        self.instance_tables.insert(instance_id, table;
        self.stats.instances_managed += 1;

        Ok(()
    }

    /// Remove an instance table
    pub fn remove_instance_table(&mut self, instance_id: InstanceId) -> Result<()> {
        if let Some(mut table) = self.instance_tables.remove(&instance_id) {
            // Clean up all resources
            table.clear_all);
            if self.stats.instances_managed > 0 {
                self.stats.instances_managed -= 1;
            }
            Ok(()
        } else {
            Err(Error::instance_not_found("Missing error message")
        }
    }

    /// Get an instance table
    pub fn get_instance_table(&self, instance_id: InstanceId) -> Option<&ResourceTable> {
        self.instance_tables.get(&instance_id)
    }

    /// Get a mutable instance table
    pub fn get_instance_table_mut(
        &mut self,
        instance_id: InstanceId,
    ) -> Option<&mut ResourceTable> {
        self.instance_tables.get_mut(&instance_id)
    }

    /// Create a resource in an instance
    pub fn create_resource(
        &mut self,
        instance_id: InstanceId,
        resource_type: ResourceTypeId,
        data: ResourceData,
    ) -> Result<ResourceHandle> {
        // Validate resource type exists
        if !self.resource_types.contains_key(&resource_type) {
            return Err(Error::runtime_execution_error("Error occurred",
            ;
        }

        // Get instance table
        let table = self.instance_tables.get_mut(&instance_id).ok_or_else(|| {
            Error::instance_not_found("Missing error message")
        })?;

        // Create resource
        let handle = table.create_resource(resource_type, data, ResourceOwnership::Owned)?;

        // Register globally
        self.global_resources.insert(handle, instance_id;
        self.stats.global_resources += 1;

        Ok(handle)
    }

    /// Transfer resource ownership between instances
    pub fn transfer_ownership(
        &mut self,
        handle: ResourceHandle,
        from_instance: InstanceId,
        to_instance: InstanceId,
    ) -> Result<ResourceHandle> {
        if !self.config.allow_cross_instance_sharing {
            return Err(Error::runtime_execution_error("Error occurred",
            ;
        }

        // Remove from source table
        let source_table = self.instance_tables.get_mut(&from_instance).ok_or_else(|| {
            Error::instance_not_found("Missing error message")
        })?;

        let resource = source_table.resources.remove(&handle).ok_or_else(|| {
            Error::resource_not_found("Resource not found in source instance")
        })?;

        // Add to target table
        let target_table = self.instance_tables.get_mut(&to_instance).ok_or_else(|| {
            Error::instance_not_found("Target instance not found")
        })?;

        let new_handle = target_table.create_resource(
            resource.resource_type,
            resource.data,
            ResourceOwnership::Owned,
        )?;

        // Update global registry
        self.global_resources.insert(new_handle, to_instance;
        self.global_resources.remove(&handle;

        // Update statistics
        self.stats.cross_instance_transfers += 1;

        Ok(new_handle)
    }

    /// Borrow a resource across instances
    pub fn borrow_resource(
        &mut self,
        handle: ResourceHandle,
        owner_instance: InstanceId,
        borrower_instance: InstanceId,
    ) -> Result<ResourceHandle> {
        if !self.config.allow_borrowing {
            return Err(Error::runtime_execution_error("Error occurred",
            ;
        }

        // Check if resource type supports borrowing
        let owner_table = self.instance_tables.get(&owner_instance).ok_or_else(|| {
            Error::instance_not_found("Missing error message")
        })?;

        let resource = owner_table.get_resource(handle).ok_or_else(|| {
            Error::resource_not_found("Resource not found")
        })?;

        let resource_type = self.get_resource_type(resource.resource_type).ok_or_else(|| {
            Error::type_not_found("Resource type not found")
        })?;

        if !resource_type.borrowable {
            return Err(Error::runtime_execution_error("Error occurred",
            ;
        }

        // Mark as borrowed in owner table
        let owner_table = self.instance_tables.get_mut(&owner_instance).unwrap();
        owner_table.borrow_resource(handle, borrower_instance)?;

        // Create borrowed reference in borrower table
        let borrower_table = self.instance_tables.get_mut(&borrower_instance).ok_or_else(|| {
            Error::instance_not_found("Missing error message")
        })?;

        let borrowed_handle = borrower_table.create_resource(
            resource.resource_type,
            ResourceData::Empty, // Borrowed resources don't duplicate data
            ResourceOwnership::Borrowed { owner: owner_instance, owner_handle: handle },
        )?;

        Ok(borrowed_handle)
    }

    /// Return a borrowed resource
    pub fn return_borrowed_resource(
        &mut self,
        borrowed_handle: ResourceHandle,
        borrower_instance: InstanceId,
    ) -> Result<()> {
        // Get borrowed resource info
        let borrower_table = self.instance_tables.get_mut(&borrower_instance).ok_or_else(|| {
            Error::instance_not_found("Borrower instance not found")
        })?;

        let borrowed_resource = borrower_table.get_resource(borrowed_handle).ok_or_else(|| {
            Error::resource_not_found("Borrowed resource not found")
        })?;

        let (owner_instance, owner_handle) = match borrowed_resource.ownership {
            ResourceOwnership::Borrowed { owner, owner_handle } => (owner, owner_handle),
            _ => {
                return Err(Error::runtime_invalid_state("Resource is not borrowed"),
            })?;
            }
        };

        // Remove from borrower table
        borrower_table.drop_resource(borrowed_handle)?;

        // Return in owner table
        let owner_table = self.instance_tables.get_mut(&owner_instance).ok_or_else(|| {
            Error::instance_not_found("Owner instance not found")
        })?;

        owner_table.return_resource(owner_handle)?;

        Ok(()
    }

    /// Get global manager statistics
    pub fn get_stats(&self) -> &ResourceManagerStats {
        &self.stats
    }

    /// Perform garbage collection
    pub fn garbage_collect(&mut self) -> Result<u32> {
        let mut total_cleaned = 0;

        for table in self.instance_tables.values_mut() {
            let cleaned = table.cleanup_expired(self.config.max_borrow_duration)?;
            total_cleaned += cleaned;
        }

        self.stats.garbage_collections += 1;
        self.stats.last_gc_at = 0; // Would use actual timestamp

        Ok(total_cleaned)
    }

    /// Validate all resources
    pub fn validate_all_resources(&self) -> Result<()> {
        if self.config.validation_level == ResourceValidationLevel::None {
            return Ok();
        }

        for table in self.instance_tables.values() {
            self.validate_table(table)?;
        }

        Ok(()
    }

    fn validate_table(&self, table: &ResourceTable) -> Result<()> {
        for resource in table.resources.values() {
            // Check resource type exists
            if !self.resource_types.contains_key(&resource.resource_type) {
                return Err(Error::validation_error("Resource references unknown type";
            }

            // Check ownership consistency
            match resource.ownership {
                ResourceOwnership::Owned => {
                    if resource.owner_instance != table.instance_id {
                        return Err(Error::validation_error("Owned resource has incorrect owner";
                    }
                }
                ResourceOwnership::Borrowed { owner, .. } => {
                    if !self.instance_tables.contains_key(&owner) {
                        return Err(Error::validation_error("Borrowed resource references unknown owner";
                    }
                }
            }
        }

        Ok(()
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResourceError::HandleNotFound(handle) => {
                write!(f, "Resource handle {} not found", handle.value()
            }
            ResourceError::TypeNotFound(type_id) => {
                write!(f, "Resource type {} not found", type_id.value()
            }
            ResourceError::InvalidState(handle, state) => {
                write!(f, "Resource {} in invalid state: {:?}", handle.value(), state)
            }
            ResourceError::AccessDenied(handle) => {
                write!(f, "Access denied to resource {}", handle.value()
            }
            ResourceError::LimitExceeded(msg) => write!(f, "Resource limit exceeded: {}", msg),
            ResourceError::TypeMismatch(msg) => write!(f, "Resource type mismatch: {}", msg),
            ResourceError::OwnershipViolation(msg) => write!(f, "Ownership violation: {}", msg),
            ResourceError::AlreadyExists(handle) => {
                write!(f, "Resource {} already exists", handle.value()
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResourceError {}

/// Create a resource type with default settings
pub fn create_resource_type(name: String, description: String) -> (String, String, bool, bool) {
    (name, description, true, false) // borrowable=true, needs_finalization=false
}

/// Create resource data from bytes
pub fn create_resource_data_bytes(data: Vec<u8>) -> ResourceData {
    ResourceData::Bytes(data)
}

/// Create external resource data
pub fn create_resource_data_external(handle: u64) -> ResourceData {
    ResourceData::ExternalHandle(handle)
}

/// Create custom resource data
pub fn create_resource_data_custom(type_id: String, data: Vec<u8>) -> ResourceData {
    ResourceData::Custom { type_id, data }
}

}
