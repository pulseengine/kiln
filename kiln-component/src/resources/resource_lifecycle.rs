//! Resource Lifecycle Management for WebAssembly Component Model
//!
//! This module provides comprehensive resource management including creation,
//! destruction, ownership transfer, and borrowing semantics as defined by
//! the Component Model specification.

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap as HashMap, vec::Vec};
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticVec;

use wrt_error::{Error, Result};
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    bounded::BoundedString,
    collections::{StaticMap as SimpleHashMap, StaticVec as BoundedVec},
    safe_memory::NoStdProvider,
};

/// Maximum number of active resources in pure no_std environments
#[cfg(not(feature = "std"))]
const MAX_RESOURCES: usize = 1024;

/// Maximum number of active borrows per resource in pure no_std
#[cfg(not(feature = "std"))]
const MAX_BORROWS_PER_RESOURCE: usize = 16;

/// Resource handle type
pub type ResourceHandle = u32;

/// Invalid resource handle constant
pub const INVALID_HANDLE: ResourceHandle = 0;

/// Resource representation in the Component Model
#[derive(Debug, Clone)]
pub struct Resource {
    /// Unique handle for this resource
    pub handle: ResourceHandle,
    /// Type of the resource
    pub resource_type: ResourceType,
    /// Current state of the resource
    pub state: ResourceState,
    /// Reference count for borrows
    pub borrow_count: u32,
    /// Metadata associated with the resource
    pub metadata: ResourceMetadata,
}

/// Resource type information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceType {
    /// Type index in the component
    pub type_idx: u32,
    /// Resource type name
    #[cfg(feature = "std")]
    pub name: String,
    #[cfg(not(feature = "std"))]
    pub name: BoundedString<64>,
    /// Destructor function index (if any)
    pub destructor: Option<u32>,
}

/// Resource state in its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceState {
    /// Resource is active and can be used
    Active,
    /// Resource is borrowed (read-only access)
    Borrowed,
    /// Resource is being transferred (ownership change)
    Transferring,
    /// Resource has been dropped/destroyed
    Dropped,
}

/// Resource metadata for tracking and debugging
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Creation timestamp (if available)
    pub created_at: Option<u64>,
    /// Last access timestamp
    pub last_accessed: Option<u64>,
    /// Creator component instance
    pub creator: u32,
    /// Current owner component instance
    pub owner: u32,
    /// Custom user data
    #[cfg(feature = "std")]
    pub user_data: Option<Vec<u8>>,
    #[cfg(not(feature = "std"))]
    pub user_data: Option<BoundedVec<u8, 256>>,
}

/// Resource lifecycle manager
#[derive(Debug)]
pub struct ResourceLifecycleManager {
    /// Next available handle
    next_handle: ResourceHandle,
    /// Active resources
    #[cfg(feature = "std")]
    resources: HashMap<ResourceHandle, Resource>,
    #[cfg(not(feature = "std"))]
    resources: SimpleHashMap<ResourceHandle, Resource, MAX_RESOURCES>,
    /// Borrow tracking
    #[cfg(feature = "std")]
    borrows: HashMap<ResourceHandle, Vec<BorrowInfo>>,
    #[cfg(not(feature = "std"))]
    borrows: SimpleHashMap<
        ResourceHandle,
        BoundedVec<BorrowInfo, MAX_BORROWS_PER_RESOURCE>,
        MAX_RESOURCES,
    >,
    /// Resource type registry
    #[cfg(feature = "std")]
    types: HashMap<u32, ResourceType>,
    #[cfg(not(feature = "std"))]
    types: SimpleHashMap<u32, ResourceType, 256>,
    /// Lifecycle hooks
    hooks: LifecycleHooks,
    /// Metrics
    metrics: ResourceMetrics,
}

/// Information about a resource borrow
#[derive(Debug, Clone)]
pub struct BorrowInfo {
    /// Component instance that holds the borrow
    pub borrower: u32,
    /// When the borrow was created
    pub borrowed_at: Option<u64>,
    /// Borrow flags
    pub flags: BorrowFlags,
}

/// Flags for resource borrows
#[derive(Debug, Clone, Copy)]
pub struct BorrowFlags {
    /// Whether this is a mutable borrow
    pub is_mutable: bool,
    /// Whether the borrow is transient (auto-released)
    pub is_transient: bool,
}

/// Lifecycle hooks for custom behavior
#[derive(Debug, Default)]
pub struct LifecycleHooks {
    /// Called when a resource is created
    pub on_create: Option<fn(&Resource) -> Result<()>>,
    /// Called when a resource is destroyed
    pub on_destroy: Option<fn(&Resource) -> Result<()>>,
    /// Called when a resource is borrowed
    pub on_borrow: Option<fn(&Resource, &BorrowInfo) -> Result<()>>,
    /// Called when a borrow is released
    pub on_release: Option<fn(&Resource, &BorrowInfo) -> Result<()>>,
    /// Called when ownership is transferred
    pub on_transfer: Option<fn(&Resource, u32, u32) -> Result<()>>,
}

/// Resource lifecycle metrics
#[derive(Debug, Default, Clone)]
pub struct ResourceMetrics {
    /// Total resources created
    pub total_created: u64,
    /// Total resources destroyed
    pub total_destroyed: u64,
    /// Current active resources
    pub active_count: u32,
    /// Total borrows created
    pub total_borrows: u64,
    /// Current active borrows
    pub active_borrows: u32,
    /// Peak resource count
    pub peak_resources: u32,
    /// Failed operations
    pub failed_operations: u64,
}

impl ResourceLifecycleManager {
    /// Create a new resource lifecycle manager
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        Self {
            next_handle: 1, // 0 is reserved for invalid handle
            resources: HashMap::new(),
            borrows: HashMap::new(),
            types: HashMap::new(),
            hooks: LifecycleHooks::default(),
            metrics: ResourceMetrics::default(),
        }
    }

    /// Create a new resource lifecycle manager (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn new() -> Self {
        Self {
            next_handle: 1, // 0 is reserved for invalid handle
            resources: SimpleHashMap::new(),
            borrows: SimpleHashMap::new(),
            types: SimpleHashMap::new(),
            hooks: LifecycleHooks::default(),
            metrics: ResourceMetrics::default(),
        }
    }

    /// Register a resource type
    pub fn register_type(
        &mut self,
        type_idx: u32,
        name: &str,
        destructor: Option<u32>,
    ) -> Result<()> {
        let resource_type = ResourceType {
            type_idx,
            #[cfg(feature = "std")]
            name: name.to_string(),
            #[cfg(not(feature = "std"))]
            name: {
                use wrt_foundation::{budget_aware_provider::CrateId, safe_managed_alloc};
                let provider = safe_managed_alloc!(512, CrateId::Component).map_err(|_| {
                    Error::resource_error("Failed to allocate memory for resource name")
                })?;
                BoundedString::try_from_str(name)
                    .map_err(|_| Error::resource_error("Resource type name too long"))?
            },
            destructor,
        };

        #[cfg(feature = "std")]
        {
            self.types.insert(type_idx, resource_type);
        }
        #[cfg(not(feature = "std"))]
        {
            self.types
                .insert(type_idx, resource_type)
                .map_err(|_| Error::resource_error("Failed to register resource type"))?;
        }

        Ok(())
    }

    /// Create a new resource
    pub fn create_resource(
        &mut self,
        type_idx: u32,
        creator: u32,
        user_data: Option<&[u8]>,
    ) -> Result<ResourceHandle> {
        // Verify type exists
        #[cfg(feature = "std")]
        let resource_type = self
            .types
            .get(&type_idx)
            .ok_or_else(|| Error::resource_error("Component not found"))?
            .clone();

        #[cfg(not(feature = "std"))]
        let resource_type = self
            .types
            .get(&type_idx)
            .ok_or_else(|| Error::resource_error("Unknown resource type"))?
            .clone();

        // Allocate handle
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle == INVALID_HANDLE {
            self.next_handle = 1; // Skip invalid handle
        }

        // Create resource
        let resource = Resource {
            handle,
            resource_type,
            state: ResourceState::Active,
            borrow_count: 0,
            metadata: ResourceMetadata {
                created_at: Some(self.get_timestamp()),
                last_accessed: Some(self.get_timestamp()),
                creator,
                owner: creator,
                #[cfg(feature = "std")]
                user_data: user_data.map(|d| d.to_vec()),
                #[cfg(not(feature = "std"))]
                user_data: user_data.and_then(|d| {
                    let mut vec = BoundedVec::new();
                    for &byte in d {
                        vec.push(byte).ok()?;
                    }
                    Some(vec)
                }),
            },
        };

        // Call creation hook
        if let Some(on_create) = self.hooks.on_create {
            on_create(&resource)?;
        }

        // Store resource
        #[cfg(feature = "std")]
        {
            self.resources.insert(handle, resource);
        }
        #[cfg(not(feature = "std"))]
        {
            self.resources
                .insert(handle, resource)
                .map_err(|_| Error::resource_error("Failed to store resource"))?;
        }

        // Update metrics
        self.metrics.total_created += 1;
        self.metrics.active_count += 1;
        if self.metrics.active_count > self.metrics.peak_resources {
            self.metrics.peak_resources = self.metrics.active_count;
        }

        Ok(handle)
    }

    /// Drop (destroy) a resource
    pub fn drop_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        // Get resource
        #[cfg(feature = "std")]
        let mut resource = self
            .resources
            .remove(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Component not found"))?;

        #[cfg(not(feature = "std"))]
        let mut resource = self
            .resources
            .remove(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

        // Check state
        if resource.state == ResourceState::Dropped {
            return Err(Error::resource_error("Resource already dropped"));
        }

        // Check borrows
        if resource.borrow_count > 0 {
            return Err(Error::resource_error(
                "Cannot drop resource with active borrows",
            ));
        }

        // Update state
        resource.state = ResourceState::Dropped;

        // Call destruction hook
        if let Some(on_destroy) = self.hooks.on_destroy {
            on_destroy(&resource)?;
        }

        // Remove any borrow info
        #[cfg(feature = "std")]
        self.borrows.remove(&handle);

        #[cfg(not(feature = "std"))]
        let _ = self.borrows.remove(&handle);

        // Update metrics
        self.metrics.total_destroyed += 1;
        self.metrics.active_count = self.metrics.active_count.saturating_sub(1);

        Ok(())
    }

    /// Borrow a resource
    pub fn borrow_resource(
        &mut self,
        handle: ResourceHandle,
        borrower: u32,
        is_mutable: bool,
    ) -> Result<()> {
        // Get timestamp before mutable borrow to avoid borrow checker conflict
        let timestamp = self.get_timestamp();

        // Get resource
        #[cfg(feature = "std")]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Component not found"))?;

        #[cfg(not(feature = "std"))]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

        // Check state
        if resource.state != ResourceState::Active && resource.state != ResourceState::Borrowed {
            return Err(Error::resource_error(
                "Resource not available for borrowing",
            ));
        }

        // Check mutable borrow rules
        if is_mutable && resource.borrow_count > 0 {
            return Err(Error::resource_error(
                "Cannot create mutable borrow with existing borrows",
            ));
        }

        // Create borrow info
        let borrow_info = BorrowInfo {
            borrower,
            borrowed_at: Some(timestamp),
            flags: BorrowFlags {
                is_mutable,
                is_transient: false,
            },
        };

        // Call borrow hook
        if let Some(on_borrow) = self.hooks.on_borrow {
            on_borrow(resource, &borrow_info)?;
        }

        // Update resource state
        resource.state = ResourceState::Borrowed;
        resource.borrow_count += 1;
        resource.metadata.last_accessed = Some(timestamp);

        // Store borrow info
        #[cfg(feature = "std")]
        {
            self.borrows.entry(handle).or_insert_with(Vec::new).push(borrow_info);
        }

        #[cfg(not(feature = "std"))]
        {
            let borrows = self
                .borrows
                .get_mut_or_insert(handle, StaticVec::new())
                .map_err(|_| Error::resource_error("Failed to store borrow info"))?;
            borrows
                .push(borrow_info)
                .map_err(|_| Error::resource_error("Too many borrows for resource"))?;
        }

        // Update metrics
        self.metrics.total_borrows += 1;
        self.metrics.active_borrows += 1;

        Ok(())
    }

    /// Release a borrow
    pub fn release_borrow(&mut self, handle: ResourceHandle, borrower: u32) -> Result<()> {
        // Get resource
        #[cfg(feature = "std")]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Component not found"))?;

        #[cfg(not(feature = "std"))]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

        // Find and remove borrow
        #[cfg(feature = "std")]
        let borrow_info = {
            let borrows = self
                .borrows
                .get_mut(&handle)
                .ok_or_else(|| Error::resource_error("No borrows for resource"))?;

            let pos = borrows
                .iter()
                .position(|b| b.borrower == borrower)
                .ok_or_else(|| Error::resource_error("Borrow not found for borrower"))?;

            borrows.remove(pos)
        };

        #[cfg(not(feature = "std"))]
        let borrow_info = {
            let borrows = self
                .borrows
                .get_mut(&handle)
                .ok_or_else(|| Error::resource_error("No borrows for resource"))?;

            let pos = borrows
                .iter()
                .position(|b| b.borrower == borrower)
                .ok_or_else(|| Error::resource_error("Borrow not found for borrower"))?;

            borrows.remove(pos)
        };

        // Call release hook
        if let Some(on_release) = self.hooks.on_release {
            on_release(resource, &borrow_info)?;
        }

        // Update resource state
        resource.borrow_count = resource.borrow_count.saturating_sub(1);
        if resource.borrow_count == 0 {
            resource.state = ResourceState::Active;
        }

        // Update metrics
        self.metrics.active_borrows = self.metrics.active_borrows.saturating_sub(1);

        Ok(())
    }

    /// Transfer ownership of a resource
    pub fn transfer_ownership(&mut self, handle: ResourceHandle, from: u32, to: u32) -> Result<()> {
        // Get timestamp before mutable borrow to avoid borrow checker conflict
        let timestamp = self.get_timestamp();

        // Get resource
        #[cfg(feature = "std")]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Component not found"))?;

        #[cfg(not(feature = "std"))]
        let resource = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

        // Check ownership
        if resource.metadata.owner != from {
            return Err(Error::resource_error("Not the owner of the resource"));
        }

        // Check state
        if resource.state != ResourceState::Active {
            return Err(Error::resource_error("Resource not in transferable state"));
        }

        // Check borrows
        if resource.borrow_count > 0 {
            return Err(Error::resource_error(
                "Cannot transfer resource with active borrows",
            ));
        }

        // Call transfer hook
        if let Some(on_transfer) = self.hooks.on_transfer {
            on_transfer(resource, from, to)?;
        }

        // Update ownership
        resource.state = ResourceState::Transferring;
        resource.metadata.owner = to;
        resource.metadata.last_accessed = Some(timestamp);
        resource.state = ResourceState::Active;

        Ok(())
    }

    /// Get resource information
    pub fn get_resource(&self, handle: ResourceHandle) -> Result<&Resource> {
        #[cfg(feature = "std")]
        {
            self.resources
                .get(&handle)
                .ok_or_else(|| Error::resource_invalid_handle("Component not found"))
        }

        #[cfg(not(feature = "std"))]
        {
            self.resources
                .get(&handle)
                .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))
        }
    }

    /// Get metrics
    pub fn get_metrics(&self) -> ResourceMetrics {
        self.metrics.clone()
    }

    /// Set lifecycle hooks
    pub fn set_hooks(&mut self, hooks: LifecycleHooks) {
        self.hooks = hooks;
    }

    /// Get current timestamp (mock implementation)
    fn get_timestamp(&self) -> u64 {
        // In a real implementation, this would use platform-specific time
        0
    }
}

impl Default for ResourceLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource representation for canonical ABI
#[derive(Debug, Clone, Copy)]
pub enum ResourceRep {
    /// Owned resource (transfers ownership)
    Own(ResourceHandle),
    /// Borrowed resource (temporary access)
    Borrow(ResourceHandle),
}

impl ResourceRep {
    /// Check if this is an owned resource
    pub fn is_own(&self) -> bool {
        matches!(self, ResourceRep::Own(_))
    }

    /// Check if this is a borrowed resource
    pub fn is_borrow(&self) -> bool {
        matches!(self, ResourceRep::Borrow(_))
    }

    /// Get the handle
    pub fn handle(&self) -> ResourceHandle {
        match self {
            ResourceRep::Own(h) | ResourceRep::Borrow(h) => *h,
        }
    }
}

/// Helper for resource cleanup in RAII style
pub struct ResourceGuard<'a> {
    manager: &'a mut ResourceLifecycleManager,
    handle: ResourceHandle,
    is_borrow: bool,
    borrower: Option<u32>,
}

impl<'a> ResourceGuard<'a> {
    /// Create a guard for an owned resource
    pub fn new_own(manager: &'a mut ResourceLifecycleManager, handle: ResourceHandle) -> Self {
        Self {
            manager,
            handle,
            is_borrow: false,
            borrower: None,
        }
    }

    /// Create a guard for a borrowed resource
    pub fn new_borrow(
        manager: &'a mut ResourceLifecycleManager,
        handle: ResourceHandle,
        borrower: u32,
    ) -> Self {
        Self {
            manager,
            handle,
            is_borrow: true,
            borrower: Some(borrower),
        }
    }

    /// Get the resource handle
    pub fn handle(&self) -> ResourceHandle {
        self.handle
    }
}

impl<'a> Drop for ResourceGuard<'a> {
    fn drop(&mut self) {
        if self.is_borrow {
            if let Some(borrower) = self.borrower {
                let _ = self.manager.release_borrow(self.handle, borrower);
            }
        } else {
            let _ = self.manager.drop_resource(self.handle);
        }
    }
}
