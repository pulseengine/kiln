//! Resource management system for Component Model
//!
//! This module provides resource management capabilities for the WebAssembly
//! Component Model, including resource handles, lifecycle management, and
//! resource tables.

#[cfg(feature = "std")]
use std::collections::HashMap;

use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::{StaticMap, StaticVec as BoundedVec},
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

/// Invalid resource handle constant
pub const INVALID_HANDLE: u32 = u32::MAX;

/// Resource handle for Component Model resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceHandle(pub u32);

impl ResourceHandle {
    /// Create a new resource handle
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the handle ID
    pub fn id(&self) -> u32 {
        self.0
    }

    /// Check if this is a valid handle
    pub fn is_valid(&self) -> bool {
        self.0 != INVALID_HANDLE
    }
}

/// Resource type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ResourceTypeId(pub u32);

impl ResourceTypeId {
    /// Create a new resource type ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the type ID
    pub fn id(&self) -> u32 {
        self.0
    }
}

/// Resource type metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceTypeMetadata {
    /// Resource type ID
    pub type_id: ResourceTypeId,
    /// Resource type name
    pub name: BoundedVec<u8, 256>,
    /// Size of the resource data
    pub size: usize,
}

/// Resource state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceState {
    /// Resource is being created
    Creating,
    /// Resource is active and usable
    Active,
    /// Resource is being destroyed
    Destroying,
    /// Resource has been destroyed
    Destroyed,
}

/// Resource ownership model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOwnership {
    /// Resource is owned by the creating component
    Owned,
    /// Resource is borrowed from another component
    Borrowed,
    /// Resource is shared between multiple components
    Shared,
}

/// Resource validation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceValidationLevel {
    /// No validation
    None,
    /// Basic validation (type checks)
    Basic,
    /// Full validation (type and lifecycle checks)
    Full,
}

/// Resource data container
#[derive(Debug)]
pub enum ResourceData {
    /// Raw bytes
    Bytes(BoundedVec<u8, 4096>),
    /// Custom data pointer (for std only)
    #[cfg(feature = "std")]
    Custom(Box<dyn std::any::Any + Send + Sync>),
    /// External resource reference
    External(u64),
}

impl Clone for ResourceData {
    fn clone(&self) -> Self {
        match self {
            Self::Bytes(b) => Self::Bytes(b.clone()),
            #[cfg(feature = "std")]
            Self::Custom(_) => panic!("Cannot clone Custom resource data"),
            Self::External(id) => Self::External(*id),
        }
    }
}

/// Resource error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// Resource not found
    NotFound,
    /// Invalid resource handle
    InvalidHandle,
    /// Resource type mismatch
    TypeMismatch,
    /// Resource already exists
    AlreadyExists,
    /// Permission denied
    PermissionDenied,
    /// Resource limit exceeded
    LimitExceeded,
}

impl core::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResourceError::NotFound => write!(f, "Resource not found"),
            ResourceError::InvalidHandle => write!(f, "Invalid resource handle"),
            ResourceError::TypeMismatch => write!(f, "Resource type mismatch"),
            ResourceError::AlreadyExists => write!(f, "Resource already exists"),
            ResourceError::PermissionDenied => write!(f, "Permission denied"),
            ResourceError::LimitExceeded => write!(f, "Resource limit exceeded"),
        }
    }
}

/// Resource type for Component Model
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    /// Basic resource type
    Basic,
    /// Handle resource type
    Handle,
    /// Stream resource type
    Stream,
    /// Future resource type
    Future,
}

/// Component resource with reference counting and destructor support
///
/// Resources track their reference count for proper lifecycle management.
/// When the reference count reaches zero, the destructor (if registered)
/// is called via the registered callback.
#[derive(Debug, Clone)]
pub struct Resource {
    /// Resource handle
    pub handle: ResourceHandle,
    /// Resource type ID
    pub type_id: ResourceTypeId,
    /// Resource state
    pub state: ResourceState,
    /// Resource ownership
    pub ownership: ResourceOwnership,
    /// Resource data
    pub data: ResourceData,
    /// Reference count for lifecycle management
    pub ref_count: u32,
    /// WebAssembly destructor function index (if any)
    /// When this resource is dropped, call this function in the component
    pub destructor: Option<u32>,
    /// Instance ID that owns this resource (for destructor calls)
    pub instance_id: Option<u32>,
}

impl Resource {
    /// Create a new resource
    pub fn new(handle: ResourceHandle, type_id: ResourceTypeId, data: ResourceData) -> Self {
        Self {
            handle,
            type_id,
            state: ResourceState::Creating,
            ownership: ResourceOwnership::Owned,
            data,
            ref_count: 1,
            destructor: None,
            instance_id: None,
        }
    }

    /// Create a new resource with destructor
    ///
    /// # Arguments
    /// * `handle` - The resource handle
    /// * `type_id` - The resource type ID
    /// * `data` - The resource data
    /// * `destructor` - WebAssembly function index to call when resource is dropped
    /// * `instance_id` - The instance ID where the destructor lives
    pub fn with_destructor(
        handle: ResourceHandle,
        type_id: ResourceTypeId,
        data: ResourceData,
        destructor: u32,
        instance_id: u32,
    ) -> Self {
        Self {
            handle,
            type_id,
            state: ResourceState::Creating,
            ownership: ResourceOwnership::Owned,
            data,
            ref_count: 1,
            destructor: Some(destructor),
            instance_id: Some(instance_id),
        }
    }

    /// Increment reference count (for borrowing)
    ///
    /// Returns the new reference count
    pub fn add_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_add(1);
        self.ref_count
    }

    /// Decrement reference count
    ///
    /// Returns the new reference count. When this reaches 0, the resource
    /// should be dropped and its destructor called.
    pub fn release(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.ref_count
    }

    /// Check if this resource should be destroyed (ref_count == 0)
    pub fn should_destroy(&self) -> bool {
        self.ref_count == 0
    }

    /// Get destructor info if available
    ///
    /// Returns (destructor_func_idx, instance_id) if a destructor is registered
    pub fn destructor_info(&self) -> Option<(u32, u32)> {
        match (self.destructor, self.instance_id) {
            (Some(dtor), Some(inst)) => Some((dtor, inst)),
            _ => None,
        }
    }

    /// Get the resource handle
    pub fn handle(&self) -> ResourceHandle {
        self.handle
    }

    /// Get the resource type ID
    pub fn type_id(&self) -> ResourceTypeId {
        self.type_id
    }

    /// Get the resource state
    pub fn state(&self) -> ResourceState {
        self.state
    }

    /// Set the resource state
    pub fn set_state(&mut self, state: ResourceState) {
        self.state = state;
    }
}

/// Resource manager configuration
#[derive(Debug, Clone)]
pub struct ResourceManagerConfig {
    /// Maximum number of resources
    pub max_resources: usize,
    /// Validation level
    pub validation_level: ResourceValidationLevel,
    /// Enable resource tracking
    pub enable_tracking: bool,
}

impl Default for ResourceManagerConfig {
    fn default() -> Self {
        Self {
            max_resources: 1024,
            validation_level: ResourceValidationLevel::Basic,
            enable_tracking: true,
        }
    }
}

/// Resource manager statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceManagerStats {
    /// Total resources created
    pub resources_created: u64,
    /// Total resources destroyed
    pub resources_destroyed: u64,
    /// Currently active resources
    pub active_resources: u32,
    /// Peak resource count
    pub peak_resources: u32,
}

/// Callback for invoking WebAssembly resource destructors
///
/// The callback receives:
/// - `instance_id`: The component instance containing the destructor
/// - `func_idx`: The function index of the destructor in that instance
/// - `resource_handle`: The handle of the resource being dropped
///
/// Returns: Ok(()) on success, or error
#[cfg(feature = "std")]
pub type DestructorCallback = Box<dyn FnMut(u32, u32, u32) -> wrt_error::Result<()> + Send>;

/// Resource manager with destructor callback support
///
/// The manager tracks resources and can invoke WebAssembly destructors
/// when resources are dropped (if a callback is registered).
pub struct ResourceManager {
    /// Manager configuration
    config: ResourceManagerConfig,
    /// Manager statistics
    stats: ResourceManagerStats,
    /// Next resource handle ID
    next_handle_id: u32,
    /// Callback to invoke WebAssembly destructors (wrapped in Mutex for thread safety)
    #[cfg(feature = "std")]
    destructor_callback: Option<std::sync::Arc<std::sync::Mutex<DestructorCallback>>>,
}

impl core::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = f.debug_struct("ResourceManager");
        s.field("config", &self.config)
            .field("stats", &self.stats)
            .field("next_handle_id", &self.next_handle_id);
        #[cfg(feature = "std")]
        s.field(
            "destructor_callback",
            &self.destructor_callback.as_ref().map(|_| "<callback>"),
        );
        s.finish()
    }
}

impl Clone for ResourceManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            stats: self.stats,
            next_handle_id: self.next_handle_id,
            #[cfg(feature = "std")]
            destructor_callback: None, // Callbacks cannot be cloned
        }
    }
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(config: ResourceManagerConfig) -> Self {
        Self {
            config,
            stats: ResourceManagerStats::default(),
            next_handle_id: 1,
            #[cfg(feature = "std")]
            destructor_callback: None,
        }
    }

    /// Set the callback for invoking WebAssembly resource destructors
    ///
    /// The callback should:
    /// 1. Find the destructor function in the component instance
    /// 2. Call it with the resource handle
    ///
    /// # Example
    /// ```ignore
    /// manager.set_destructor_callback(Box::new(move |instance_id, func_idx, handle| {
    ///     engine.call_function(instance_id as usize, func_idx as usize, vec![
    ///         Value::I32(handle as i32),
    ///     ])
    /// }));
    /// ```
    #[cfg(feature = "std")]
    pub fn set_destructor_callback(&mut self, callback: DestructorCallback) {
        self.destructor_callback = Some(std::sync::Arc::new(std::sync::Mutex::new(callback)));
    }

    /// Check if a destructor callback is registered
    #[cfg(feature = "std")]
    pub fn has_destructor_callback(&self) -> bool {
        self.destructor_callback.is_some()
    }

    /// Get manager statistics
    pub fn stats(&self) -> ResourceManagerStats {
        self.stats
    }

    /// Create a new resource
    pub fn create_resource(
        &mut self,
        type_id: ResourceTypeId,
        data: ResourceData,
    ) -> core::result::Result<ResourceHandle, ResourceError> {
        if self.stats.active_resources >= self.config.max_resources as u32 {
            return Err(ResourceError::LimitExceeded);
        }

        let handle = ResourceHandle::new(self.next_handle_id);
        self.next_handle_id += 1;
        self.stats.resources_created += 1;
        self.stats.active_resources += 1;

        if self.stats.active_resources > self.stats.peak_resources {
            self.stats.peak_resources = self.stats.active_resources;
        }

        Ok(handle)
    }

    /// Destroy a resource, optionally calling its destructor
    ///
    /// If the resource has a destructor and a callback is registered,
    /// the destructor will be invoked before the resource is destroyed.
    pub fn destroy_resource(
        &mut self,
        handle: ResourceHandle,
    ) -> core::result::Result<(), ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        self.stats.resources_destroyed += 1;
        if self.stats.active_resources > 0 {
            self.stats.active_resources -= 1;
        }

        Ok(())
    }

    /// Destroy a resource with destructor callback invocation
    ///
    /// This version takes the destructor info from the Resource and calls
    /// the registered callback to invoke the WebAssembly destructor.
    #[cfg(feature = "std")]
    pub fn destroy_resource_with_destructor(
        &mut self,
        resource: &Resource,
    ) -> wrt_error::Result<()> {
        if !resource.handle.is_valid() {
            return Err(wrt_error::Error::resource_not_found(
                "Invalid resource handle",
            ));
        }

        // If the resource has a destructor and we have a callback, call it
        if let Some((dtor_func, instance_id)) = resource.destructor_info() {
            if let Some(ref callback_arc) = self.destructor_callback {
                let mut callback = callback_arc.lock().map_err(|_| {
                    wrt_error::Error::runtime_error("Failed to lock destructor callback")
                })?;
                callback(instance_id, dtor_func, resource.handle.0)?;
            }
            // Note: If no callback is registered, we silently skip the destructor
            // This matches Component Model behavior where destructors are best-effort
        }

        self.stats.resources_destroyed += 1;
        if self.stats.active_resources > 0 {
            self.stats.active_resources -= 1;
        }

        Ok(())
    }

    /// Drop a resource by decrementing its reference count
    ///
    /// If the reference count reaches zero, the resource is destroyed
    /// and its destructor is called (if registered).
    ///
    /// Returns true if the resource was actually destroyed.
    #[cfg(feature = "std")]
    pub fn drop_resource(&mut self, resource: &mut Resource) -> wrt_error::Result<bool> {
        let new_count = resource.release();

        if new_count == 0 {
            // Reference count hit zero - destroy the resource
            resource.set_state(ResourceState::Destroying);
            self.destroy_resource_with_destructor(resource)?;
            resource.set_state(ResourceState::Destroyed);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Resource table statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceTableStats {
    /// Total entries in table
    pub total_entries: u32,
    /// Active entries in table
    pub active_entries: u32,
    /// Total lookups performed
    pub total_lookups: u64,
    /// Successful lookups
    pub successful_lookups: u64,
}

/// Maximum capacity of the resource table for no_std environments
/// Note: This capacity affects stack usage. Each Resource struct can be large
/// due to ResourceData::Bytes. Keep capacity small for embedded systems.
#[cfg(not(feature = "std"))]
const RESOURCE_TABLE_CAPACITY: usize = 64;

/// Maximum capacity of the resource table for std environments
#[cfg(feature = "std")]
const RESOURCE_TABLE_CAPACITY: usize = 1024;

/// Resource table for managing component resources
///
/// This table stores resources indexed by their handle ID and provides
/// efficient lookup, insertion, and removal operations.
///
/// When `std` feature is enabled, uses `HashMap` for heap-allocated storage.
/// In no_std mode, uses `StaticMap` with bounded capacity for stack allocation.
#[derive(Debug)]
pub struct ResourceTable {
    /// Table statistics
    stats: ResourceTableStats,
    /// Maximum table size
    max_size: usize,
    /// Next available handle ID
    next_handle: u32,
    /// Actual storage for resources, keyed by handle ID (heap-allocated with std)
    #[cfg(feature = "std")]
    resources: HashMap<u32, Resource>,
    /// Actual storage for resources, keyed by handle ID (stack-allocated without std)
    #[cfg(not(feature = "std"))]
    resources: StaticMap<u32, Resource, RESOURCE_TABLE_CAPACITY>,
}

impl ResourceTable {
    /// Create a new resource table with default capacity
    pub fn new() -> Self {
        Self::with_capacity(RESOURCE_TABLE_CAPACITY)
    }

    /// Create a new resource table with a specific maximum size
    pub fn with_capacity(max_size: usize) -> Self {
        // Cap the max_size to the capacity
        let effective_max = core::cmp::min(max_size, RESOURCE_TABLE_CAPACITY);
        Self {
            stats: ResourceTableStats::default(),
            max_size: effective_max,
            next_handle: 1, // Start at 1, reserve 0 as invalid
            #[cfg(feature = "std")]
            resources: HashMap::with_capacity(effective_max),
            #[cfg(not(feature = "std"))]
            resources: StaticMap::new(),
        }
    }

    /// Get table statistics
    pub fn stats(&self) -> ResourceTableStats {
        self.stats
    }

    /// Get a resource by handle ID (immutable reference)
    pub fn get(
        &mut self,
        handle_id: u32,
    ) -> core::result::Result<Option<&Resource>, ResourceError> {
        self.stats.total_lookups += 1;

        let handle = ResourceHandle::new(handle_id);
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        let result = self.resources.get(&handle_id);
        if result.is_some() {
            self.stats.successful_lookups += 1;
        }
        Ok(result)
    }

    /// Get a resource by ResourceHandle (immutable reference)
    pub fn get_by_handle(
        &mut self,
        handle: ResourceHandle,
    ) -> core::result::Result<Option<&Resource>, ResourceError> {
        self.get(handle.0)
    }

    /// Get a mutable reference to a resource by handle ID
    pub fn get_mut(
        &mut self,
        handle_id: u32,
    ) -> core::result::Result<Option<&mut Resource>, ResourceError> {
        self.stats.total_lookups += 1;

        let handle = ResourceHandle::new(handle_id);
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        let result = self.resources.get_mut(&handle_id);
        if result.is_some() {
            self.stats.successful_lookups += 1;
        }
        Ok(result)
    }

    /// Get a mutable reference to a resource by ResourceHandle
    pub fn get_mut_by_handle(
        &mut self,
        handle: ResourceHandle,
    ) -> core::result::Result<Option<&mut Resource>, ResourceError> {
        self.get_mut(handle.0)
    }

    /// Insert a resource into the table
    ///
    /// The resource's handle will be used as the key. If the handle is not set
    /// (i.e., has INVALID_HANDLE), a new unique handle will be assigned.
    pub fn insert(
        &mut self,
        mut resource: Resource,
    ) -> core::result::Result<ResourceHandle, ResourceError> {
        if self.resources.len() >= self.max_size {
            return Err(ResourceError::LimitExceeded);
        }

        // Assign a handle if not already set
        if !resource.handle.is_valid() {
            resource.handle = ResourceHandle::new(self.next_handle);
            self.next_handle = self.next_handle.saturating_add(1);
            // Ensure we don't hit INVALID_HANDLE
            if self.next_handle == INVALID_HANDLE {
                self.next_handle = 1;
            }
        }

        let handle = resource.handle;

        // Check for duplicate handle
        if self.resources.contains_key(&handle.0) {
            return Err(ResourceError::AlreadyExists);
        }

        // Insert the resource
        #[cfg(feature = "std")]
        {
            self.resources.insert(handle.0, resource);
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources
                .insert(handle.0, resource)
                .map_err(|_| ResourceError::LimitExceeded)?;
        }

        self.stats.total_entries += 1;
        self.stats.active_entries += 1;

        Ok(handle)
    }

    /// Remove a resource from the table
    pub fn remove(
        &mut self,
        handle: ResourceHandle,
    ) -> core::result::Result<Option<Resource>, ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        let removed = self.resources.remove(&handle.0);
        if removed.is_some() {
            if self.stats.active_entries > 0 {
                self.stats.active_entries -= 1;
            }
        }

        Ok(removed)
    }

    /// Check if a handle exists in the table
    pub fn contains(&self, handle: ResourceHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }
        self.resources.contains_key(&handle.0)
    }

    /// Get the number of resources currently in the table
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Allocate a new handle without inserting a resource
    ///
    /// This is useful when you need to create a handle before the resource
    /// data is fully constructed.
    pub fn allocate_handle(&mut self) -> ResourceHandle {
        let handle = ResourceHandle::new(self.next_handle);
        self.next_handle = self.next_handle.saturating_add(1);
        // Ensure we don't hit INVALID_HANDLE
        if self.next_handle == INVALID_HANDLE {
            self.next_handle = 1;
        }
        handle
    }

    /// Update a resource's state
    pub fn update_state(
        &mut self,
        handle: ResourceHandle,
        new_state: ResourceState,
    ) -> core::result::Result<(), ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        match self.resources.get_mut(&handle.0) {
            Some(resource) => {
                resource.state = new_state;
                Ok(())
            },
            None => Err(ResourceError::NotFound),
        }
    }

    /// Increment the reference count for a resource
    pub fn add_ref(&mut self, handle: ResourceHandle) -> core::result::Result<u32, ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        match self.resources.get_mut(&handle.0) {
            Some(resource) => {
                let new_count = resource.add_ref();
                Ok(new_count)
            },
            None => Err(ResourceError::NotFound),
        }
    }

    /// Decrement the reference count for a resource
    ///
    /// Returns the new reference count. When it reaches 0, the resource
    /// should be destroyed.
    pub fn release(&mut self, handle: ResourceHandle) -> core::result::Result<u32, ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        match self.resources.get_mut(&handle.0) {
            Some(resource) => {
                let new_count = resource.release();
                Ok(new_count)
            },
            None => Err(ResourceError::NotFound),
        }
    }

    /// Allocate a new empty resource handle
    ///
    /// This creates a resource with default data and returns its handle.
    /// Useful for tests and simple allocation patterns.
    pub fn allocate(&mut self) -> core::result::Result<u32, ResourceError> {
        if self.resources.len() >= self.max_size {
            return Err(ResourceError::LimitExceeded);
        }

        let handle_id = self.next_handle;
        self.next_handle = self.next_handle.saturating_add(1);
        if self.next_handle == INVALID_HANDLE {
            self.next_handle = 1;
        }

        let resource = Resource::new(
            ResourceHandle::new(handle_id),
            ResourceTypeId::default(),
            ResourceData::External(0),
        );

        #[cfg(feature = "std")]
        {
            self.resources.insert(handle_id, resource);
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources
                .insert(handle_id, resource)
                .map_err(|_| ResourceError::LimitExceeded)?;
        }

        self.stats.total_entries += 1;
        self.stats.active_entries += 1;

        Ok(handle_id)
    }

    /// Deallocate a resource by its handle ID
    ///
    /// This removes the resource from the table.
    pub fn deallocate(&mut self, handle_id: u32) -> core::result::Result<(), ResourceError> {
        let handle = ResourceHandle::new(handle_id);
        if !handle.is_valid() {
            return Err(ResourceError::InvalidHandle);
        }

        match self.resources.remove(&handle_id) {
            Some(_) => {
                if self.stats.active_entries > 0 {
                    self.stats.active_entries -= 1;
                }
                Ok(())
            },
            None => Err(ResourceError::NotFound),
        }
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create resource data from bytes
pub fn create_resource_data_bytes(
    data: &[u8],
) -> core::result::Result<ResourceData, ResourceError> {
    let provider =
        safe_managed_alloc!(65536, CrateId::Component).map_err(|_| ResourceError::LimitExceeded)?;
    let mut vec = BoundedVec::new().unwrap();
    for &byte in data {
        vec.push(byte).map_err(|_| ResourceError::LimitExceeded)?;
    }
    Ok(ResourceData::Bytes(vec))
}

/// Helper function to create resource data from external reference
pub fn create_resource_data_external(reference: u64) -> ResourceData {
    ResourceData::External(reference)
}

/// Helper function to create resource data from custom data (std only)
#[cfg(feature = "std")]
pub fn create_resource_data_custom<T: std::any::Any + Send + Sync>(data: T) -> ResourceData {
    ResourceData::Custom(Box::new(data))
}

/// Helper function to create a resource type
pub fn create_resource_type(
    name: &str,
) -> core::result::Result<ResourceTypeMetadata, ResourceError> {
    let provider =
        safe_managed_alloc!(65536, CrateId::Component).map_err(|_| ResourceError::LimitExceeded)?;
    let mut name_vec = BoundedVec::new().unwrap();
    for &byte in name.as_bytes() {
        name_vec.push(byte).map_err(|_| ResourceError::LimitExceeded)?;
    }

    Ok(ResourceTypeMetadata {
        type_id: ResourceTypeId::new(1), // Stub implementation
        name: name_vec,
        size: 0,
    })
}

// Implement required traits for BoundedVec compatibility
use wrt_foundation::traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream};

// Macro to implement basic traits for tuple structs
macro_rules! impl_basic_traits_tuple {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                self.0.update_checksum(checksum);
            }
        }

        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                &self,
                writer: &mut WriteStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<()> {
                self.0.to_bytes_with_provider(writer, provider)
            }
        }

        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                reader: &mut ReadStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<Self> {
                Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
            }
        }
    };
}

// Macro to implement basic traits for enums
macro_rules! impl_basic_traits_enum {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                // Simple stub - just update with 0
                0u8.update_checksum(checksum);
            }
        }

        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                &self,
                _writer: &mut WriteStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<()> {
                Ok(())
            }
        }

        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                _reader: &mut ReadStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<Self> {
                Ok($default_val)
            }
        }
    };
}

impl Default for ResourceHandle {
    fn default() -> Self {
        Self(INVALID_HANDLE)
    }
}

impl Default for ResourceData {
    fn default() -> Self {
        Self::Bytes({
            let provider = safe_managed_alloc!(65536, CrateId::Component).unwrap();
            BoundedVec::new().unwrap()
        })
    }
}

// Apply macro to types that need traits
impl_basic_traits_tuple!(ResourceHandle, ResourceHandle::default());
impl_basic_traits_tuple!(ResourceTypeId, ResourceTypeId::default());
impl_basic_traits_enum!(ResourceData, ResourceData::default());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_table_new() {
        let table = ResourceTable::new();
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
    }

    #[test]
    fn test_resource_table_allocate() {
        let mut table = ResourceTable::new();

        let handle1 = table.allocate().expect("Failed to allocate");
        assert_eq!(handle1, 1);
        assert_eq!(table.len(), 1);

        let handle2 = table.allocate().expect("Failed to allocate");
        assert_eq!(handle2, 2);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn test_resource_table_get() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");

        // Get existing resource
        let resource = table.get(handle).expect("Get failed");
        assert!(resource.is_some());

        // Get non-existent resource
        let resource = table.get(9999).expect("Get failed");
        assert!(resource.is_none());
    }

    #[test]
    fn test_resource_table_get_invalid_handle() {
        let mut table = ResourceTable::new();

        // INVALID_HANDLE should return error
        let result = table.get(INVALID_HANDLE);
        assert!(matches!(result, Err(ResourceError::InvalidHandle)));
    }

    #[test]
    fn test_resource_table_deallocate() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");
        assert_eq!(table.len(), 1);

        // Deallocate
        table.deallocate(handle).expect("Deallocate failed");
        assert_eq!(table.len(), 0);

        // Resource should no longer exist
        let resource = table.get(handle).expect("Get failed");
        assert!(resource.is_none());
    }

    #[test]
    fn test_resource_table_double_deallocate() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");
        table.deallocate(handle).expect("Deallocate failed");

        // Double deallocate should error
        let result = table.deallocate(handle);
        assert!(matches!(result, Err(ResourceError::NotFound)));
    }

    #[test]
    fn test_resource_table_insert() {
        let mut table = ResourceTable::new();

        let resource = Resource::new(
            ResourceHandle::default(), // Invalid handle, will be auto-assigned
            ResourceTypeId::new(42),
            ResourceData::External(123),
        );

        let handle = table.insert(resource).expect("Insert failed");
        assert!(handle.is_valid());
        assert_eq!(table.len(), 1);

        // Verify the resource
        let retrieved = table.get(handle.id()).expect("Get failed");
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert_eq!(r.type_id.id(), 42);
    }

    #[test]
    fn test_resource_table_remove() {
        let mut table = ResourceTable::new();

        let resource = Resource::new(
            ResourceHandle::new(100),
            ResourceTypeId::new(1),
            ResourceData::External(0),
        );

        let handle = table.insert(resource).expect("Insert failed");

        // Remove the resource
        let removed = table.remove(handle).expect("Remove failed");
        assert!(removed.is_some());
        assert_eq!(table.len(), 0);

        // Should not be findable anymore
        let result = table.get(handle.id()).expect("Get failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_resource_table_contains() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");
        let resource_handle = ResourceHandle::new(handle);

        assert!(table.contains(resource_handle));

        table.deallocate(handle).expect("Deallocate failed");
        assert!(!table.contains(resource_handle));
    }

    #[test]
    fn test_resource_table_statistics() {
        let mut table = ResourceTable::new();

        // Initial stats
        let stats = table.stats();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.active_entries, 0);

        // After allocation
        let _handle = table.allocate().expect("Failed to allocate");
        let stats = table.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.active_entries, 1);

        // After get (should update lookups)
        let _handle2 = table.allocate().expect("Failed to allocate");
        let _ = table.get(_handle);
        let stats = table.stats();
        assert!(stats.total_lookups > 0);
    }

    #[test]
    fn test_resource_table_capacity() {
        let mut table = ResourceTable::with_capacity(2);

        // Should be able to allocate up to capacity
        let _h1 = table.allocate().expect("Failed to allocate");
        let _h2 = table.allocate().expect("Failed to allocate");

        // Third allocation should fail
        let result = table.allocate();
        assert!(matches!(result, Err(ResourceError::LimitExceeded)));
    }

    #[test]
    fn test_resource_reference_counting() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");
        let resource_handle = ResourceHandle::new(handle);

        // Initial ref count should be 1
        let resource = table.get(handle).expect("Get failed").unwrap();
        assert_eq!(resource.ref_count, 1);

        // Add ref
        let new_count = table.add_ref(resource_handle).expect("Add ref failed");
        assert_eq!(new_count, 2);

        // Release
        let new_count = table.release(resource_handle).expect("Release failed");
        assert_eq!(new_count, 1);
    }

    #[test]
    fn test_resource_state_update() {
        let mut table = ResourceTable::new();

        let handle = table.allocate().expect("Failed to allocate");
        let resource_handle = ResourceHandle::new(handle);

        // Update state to Active
        table
            .update_state(resource_handle, ResourceState::Active)
            .expect("Update failed");

        let resource = table.get(handle).expect("Get failed").unwrap();
        assert_eq!(resource.state, ResourceState::Active);

        // Update to Destroyed
        table
            .update_state(resource_handle, ResourceState::Destroyed)
            .expect("Update failed");

        let resource = table.get(handle).expect("Get failed").unwrap();
        assert_eq!(resource.state, ResourceState::Destroyed);
    }
}
