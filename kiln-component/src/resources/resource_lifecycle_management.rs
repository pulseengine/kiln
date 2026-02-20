//! Resource Lifecycle Management for WebAssembly Component Model
//!
//! SW-REQ-ID: REQ_CMP_020 - Component Resource Lifecycle Management
//!
//! This module implements comprehensive resource lifecycle management with
//! drop handlers, lifetime validation, and automatic cleanup for the Component Model.

#[cfg(not(feature = "std"))]
use core::{fmt, mem, ptr};
#[cfg(feature = "std")]
use std::{fmt, mem, ptr};

#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};

use wrt_foundation::{
    bounded::{ BoundedString},
    prelude::*,
    safe_memory::NoStdProvider,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::{
    async_types::{StreamHandle, FutureHandle},
    types::{ValType, Value},
};

use wrt_error::{Error, ErrorCategory, Result};

/// Maximum number of resources in no_std environments
const MAX_RESOURCES: usize = 1024;

/// Maximum number of drop handlers per resource
const MAX_DROP_HANDLERS: usize = 8;

/// Maximum call stack depth for drop operations
const MAX_DROP_STACK_DEPTH: usize = 32;

// ResourceProvider removed - using capability-based allocation via safe_managed_alloc!

/// Resource lifecycle manager
#[derive(Debug)]
pub struct ResourceLifecycleManager {
    /// Active resources
    #[cfg(feature = "std")]
    resources: Vec<ResourceEntry>,
    #[cfg(not(feature = "std"))]
    resources: BoundedVec<ResourceEntry, MAX_RESOURCES, ResourceProvider>,
    
    /// Drop handlers registry
    #[cfg(feature = "std")]
    drop_handlers: Vec<DropHandler>,
    #[cfg(not(feature = "std"))]
    drop_handlers: BoundedVec<DropHandler, MAX_RESOURCES, ResourceProvider>,
    
    /// Lifecycle policies
    policies: LifecyclePolicies,
    
    /// Statistics
    stats: LifecycleStats,
    
    /// Next resource ID
    next_resource_id: u32,
    
    /// GC state
    gc_state: GarbageCollectionState,
}

/// Entry for a managed resource
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    /// Resource ID
    pub id: ResourceId,
    /// Resource type
    pub resource_type: ResourceType,
    /// Current state
    pub state: ResourceState,
    /// Reference count
    pub ref_count: u32,
    /// Owning component
    pub owner: ComponentId,
    /// Associated handlers
    #[cfg(feature = "std")]
    pub handlers: Vec<DropHandlerId>,
    #[cfg(not(feature = "std"))]
    pub handlers: BoundedVec<DropHandlerId, MAX_DROP_HANDLERS, ResourceProvider>,
    /// Creation time (for debugging)
    pub created_at: u64,
    /// Last access time (for GC)
    pub last_access: u64,
    /// Resource metadata
    pub metadata: ResourceMetadata,
}

/// Drop handler for resource cleanup
#[derive(Debug, Clone)]
pub struct DropHandler {
    /// Handler ID
    pub id: DropHandlerId,
    /// Resource type this handler applies to
    pub resource_type: ResourceType,
    /// Handler function
    pub handler_fn: DropHandlerFunction,
    /// Priority (lower number = higher priority)
    pub priority: u8,
    /// Whether handler is required for cleanup
    pub required: bool,
}

/// Resource lifecycle policies
#[derive(Debug, Clone)]
pub struct LifecyclePolicies {
    /// Enable automatic garbage collection
    pub enable_gc: bool,
    /// GC interval in milliseconds
    pub gc_interval_ms: u64,
    /// Maximum resource lifetime before forced cleanup (ms)
    pub max_lifetime_ms: Option<u64>,
    /// Enable strict reference counting
    pub strict_ref_counting: bool,
    /// Enable resource leak detection
    pub leak_detection: bool,
    /// Maximum memory usage before triggering cleanup
    pub max_memory_bytes: Option<usize>,
}

/// Garbage collection state
#[derive(Debug, Clone)]
pub struct GarbageCollectionState {
    /// Last GC run time
    pub last_gc_time: u64,
    /// Number of GC cycles
    pub gc_cycles: u64,
    /// Resources collected in last cycle
    pub last_collected: u32,
    /// Whether GC is currently running
    pub gc_running: bool,
}

/// Lifecycle statistics
#[derive(Debug, Clone)]
pub struct LifecycleStats {
    /// Total resources created
    pub resources_created: u64,
    /// Total resources destroyed
    pub resources_destroyed: u64,
    /// Current active resources
    pub active_resources: u32,
    /// Total drop handlers executed
    pub drop_handlers_executed: u64,
    /// Total memory used by resources
    pub memory_used_bytes: usize,
    /// Number of resource leaks detected
    pub leaks_detected: u32,
}

/// Resource type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// Stream resource
    Stream,
    /// Future resource
    Future,
    /// Memory buffer
    MemoryBuffer,
    /// File handle
    FileHandle,
    /// Network connection
    NetworkConnection,
    /// Custom user-defined resource
    Custom(u32),
}

/// Resource state
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
    /// Resource is in error state
    Error,
}

/// Resource metadata
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Resource name for debugging
    pub name: BoundedString<64, ResourceProvider>,
    /// Resource size in bytes
    pub size_bytes: usize,
    /// Tags for categorization
    #[cfg(feature = "std")]
    pub tags: Vec<BoundedString<32, ResourceProvider>>,
    #[cfg(not(feature = "std"))]
    pub tags: BoundedVec<BoundedString<32, ResourceProvider>, 8, ResourceProvider>,
    /// Additional properties
    #[cfg(feature = "std")]
    pub properties: Vec<(BoundedString<32, ResourceProvider>, Value)>,
    #[cfg(not(feature = "std"))]
    pub properties: BoundedVec<(BoundedString<32, ResourceProvider>, Value), 16, ResourceProvider>,
}

/// Drop handler function type
#[derive(Debug, Clone)]
pub enum DropHandlerFunction {
    /// Stream cleanup
    StreamCleanup,
    /// Future cleanup
    FutureCleanup,
    /// Memory cleanup
    MemoryCleanup,
    /// Custom cleanup function
    Custom {
        name: BoundedString<64, ResourceProvider>,
        // In a real implementation, this would be a function pointer
        placeholder: u32,
    },
}

/// Resource creation request
#[derive(Debug, Clone)]
pub struct ResourceCreateRequest {
    /// Resource type
    pub resource_type: ResourceType,
    /// Initial metadata
    pub metadata: ResourceMetadata,
    /// Owning component
    pub owner: ComponentId,
    /// Custom drop handlers
    #[cfg(feature = "std")]
    pub custom_handlers: Vec<DropHandlerFunction>,
    #[cfg(not(feature = "std"))]
    pub custom_handlers: BoundedVec<DropHandlerFunction, MAX_DROP_HANDLERS, ResourceProvider>,
}

/// Drop operation result
#[derive(Debug, Clone)]
pub enum DropResult {
    /// Drop completed successfully
    Success,
    /// Drop deferred (will be retried)
    Deferred,
    /// Drop failed with error
    Failed(Error),
    /// Drop skipped (resource already cleaned up)
    Skipped,
}

/// Garbage collection result
#[derive(Debug, Clone)]
pub struct GcResult {
    /// Number of resources collected
    pub collected_count: u32,
    /// Memory freed in bytes
    pub memory_freed_bytes: usize,
    /// Time taken for GC (microseconds)
    pub gc_time_us: u64,
    /// Whether full GC was performed
    pub full_gc: bool,
}

/// Resource ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u32;

/// Drop handler ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DropHandlerId(pub u32;

/// Component ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32;

impl ResourceLifecycleManager {
    /// Create new resource lifecycle manager
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            resources: Vec::new(),
            #[cfg(not(feature = "std"))]
            resources: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)
                    .expect(".expect("Failed to allocate memory for resources"));")
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            drop_handlers: Vec::new(),
            #[cfg(not(feature = "std"))]
            drop_handlers: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)
                    .expect(".expect("Failed to allocate memory for drop handlers"));")
                BoundedVec::new().unwrap()
            },
            policies: LifecyclePolicies::default(),
            stats: LifecycleStats::new(),
            next_resource_id: 1,
            gc_state: GarbageCollectionState::new(),
        }
    }

    /// Create new resource lifecycle manager with custom policies
    pub fn with_policies(policies: LifecyclePolicies) -> Self {
        let mut manager = Self::new();
        manager.policies = policies;
        manager
    }

    /// Create a new resource
    pub fn create_resource(&mut self, request: ResourceCreateRequest) -> Result<ResourceId> {
        let resource_id = ResourceId(self.next_resource_id;
        self.next_resource_id += 1;

        // Register drop handlers for this resource
        #[cfg(feature = "std")]
        let mut handler_ids = Vec::new();
        #[cfg(not(any(feature = "std", )))]
        let mut handler_ids = {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            BoundedVec::<DropHandlerId, MAX_DROP_HANDLERS, ResourceProvider>::new(provider).unwrap()
        };
        
        for handler_fn in request.custom_handlers.iter() {
            let handler_id = self.register_drop_handler(
                request.resource_type,
                handler_fn.clone(),
                0, // Default priority
                false, // Not required
            )?;
            #[cfg(feature = "std")]
            handler_ids.push(handler_id);
            #[cfg(not(any(feature = "std", )))]
            handler_ids.push(handler_id).map_err(|_| {
                Error::runtime_execution_error("Error occurred")
            })?;
        }

        let entry = ResourceEntry {
            id: resource_id,
            resource_type: request.resource_type,
            state: ResourceState::Creating,
            ref_count: 1,
            owner: request.owner,
            handlers: handler_ids,
            created_at: self.get_current_time(),
            last_access: self.get_current_time(),
            metadata: request.metadata,
        };

        self.resources.push(entry).map_err(|_| {
            Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Too many resources")
        })?;

        // Update statistics
        self.stats.resources_created += 1;
        self.stats.active_resources += 1;
        self.stats.memory_used_bytes += self.get_resource(resource_id)?.metadata.size_bytes;

        // Mark resource as active
        self.update_resource_state(resource_id, ResourceState::Active)?;

        Ok(resource_id)
    }

    /// Add a reference to a resource
    pub fn add_reference(&mut self, resource_id: ResourceId) -> Result<u32> {
        let resource = self.get_resource_mut(resource_id)?;
        
        if resource.state != ResourceState::Active {
            return Err(Error::runtime_execution_error("Error occurred"))
        }

        resource.ref_count += 1;
        resource.last_access = self.get_current_time);
        Ok(resource.ref_count)
    }

    /// Remove a reference from a resource
    pub fn remove_reference(&mut self, resource_id: ResourceId) -> Result<u32> {
        let should_drop = {
            let resource = self.get_resource_mut(resource_id)?;
            
            if resource.ref_count == 0 {
                return Err(Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::EXECUTION_ERROR,
                    "Reference count already zero";
            }

            resource.ref_count -= 1;
            resource.last_access = self.get_current_time);
            
            resource.ref_count == 0
        };

        if should_drop {
            self.drop_resource(resource_id)?;
        }

        Ok(self.get_resource(resource_id)?.ref_count)
    }

    /// Drop a resource immediately
    pub fn drop_resource(&mut self, resource_id: ResourceId) -> Result<DropResult> {
        let resource = self.get_resource_mut(resource_id)?;
        
        if resource.state == ResourceState::Destroyed {
            return Ok(DropResult::Skipped;
        }

        // Mark as destroying
        resource.state = ResourceState::Destroying;

        // Execute drop handlers
        #[cfg(feature = "std")]
        let handler_ids: Vec<DropHandlerId> = resource.handlers.iter().cloned().collect();
        #[cfg(not(any(feature = "std", )))]
        let handler_ids = resource.handlers.clone();
        
        for handler_id in handler_ids {
            let result = self.execute_drop_handler(handler_id, resource_id)?;
            if let DropResult::Failed(error) = result {
                // If a required handler fails, mark resource as error state
                if self.is_handler_required(handler_id)? {
                    self.update_resource_state(resource_id, ResourceState::Error)?;
                    return Ok(DropResult::Failed(error;
                }
            }
        }

        // Update statistics
        self.stats.resources_destroyed += 1;
        self.stats.active_resources -= 1;
        self.stats.memory_used_bytes -= self.get_resource(resource_id)?.metadata.size_bytes;

        // Mark as destroyed
        self.update_resource_state(resource_id, ResourceState::Destroyed)?;

        Ok(DropResult::Success)
    }

    /// Register a drop handler
    pub fn register_drop_handler(
        &mut self,
        resource_type: ResourceType,
        handler_fn: DropHandlerFunction,
        priority: u8,
        required: bool,
    ) -> Result<DropHandlerId> {
        let handler_id = DropHandlerId(self.drop_handlers.len() as u32;
        
        let handler = DropHandler {
            id: handler_id,
            resource_type,
            handler_fn,
            priority,
            required,
        };

        self.drop_handlers.push(handler).map_err(|_| {
            Error::runtime_execution_error("Error occurred")
        })?;

        Ok(handler_id)
    }

    /// Run garbage collection
    pub fn run_garbage_collection(&mut self, force_full_gc: bool) -> Result<GcResult> {
        if self.gc_state.gc_running {
            return Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::EXECUTION_ERROR,
                "Garbage collection already running";
        }

        let start_time = self.get_current_time);
        self.gc_state.gc_running = true;
        
        let mut collected_count = 0;
        let mut memory_freed = 0;

        // Find resources to collect
        #[cfg(feature = "std")]
        let mut resources_to_drop = Vec::new();
        #[cfg(not(any(feature = "std", )))]
        let mut resources_to_drop = {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            BoundedVec::<ResourceId, 64, ResourceProvider>::new(provider).unwrap()
        };
        
        for resource in &self.resources {
            let should_collect = if force_full_gc {
                resource.ref_count == 0
            } else {
                resource.ref_count == 0 && self.should_collect_resource(resource)
            };
            
            if should_collect {
                let _ = resources_to_drop.push(resource.id);
            }
        }

        // Drop collected resources
        for resource_id in &resources_to_drop {
            if let Ok(resource) = self.get_resource(*resource_id) {
                memory_freed += resource.metadata.size_bytes;
            }
            
            if self.drop_resource(*resource_id).is_ok() {
                collected_count += 1;
            }
        }

        // Remove destroyed resources from list
        #[cfg(feature = "std")]
        {
            self.resources.retain(|r| r.state != ResourceState::Destroyed;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let mut i = 0;
            while i < self.resources.len() {
                if self.resources[i].state == ResourceState::Destroyed {
                    self.resources.remove(i;
                } else {
                    i += 1;
                }
            }
        }

        // Update GC state
        let gc_time = self.get_current_time() - start_time;
        self.gc_state.gc_running = false;
        self.gc_state.last_gc_time = self.get_current_time);
        self.gc_state.gc_cycles += 1;
        self.gc_state.last_collected = collected_count;

        Ok(GcResult {
            collected_count,
            memory_freed_bytes: memory_freed,
            gc_time_us: gc_time,
            full_gc: force_full_gc,
        })
    }

    /// Get resource by ID
    pub fn get_resource(&self, resource_id: ResourceId) -> Result<&ResourceEntry> {
        self.resources
            .iter()
            .find(|r| r.id == resource_id)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })
    }

    /// Get mutable resource by ID
    pub fn get_resource_mut(&mut self, resource_id: ResourceId) -> Result<&mut ResourceEntry> {
        self.resources
            .iter_mut()
            .find(|r| r.id == resource_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::EXECUTION_ERROR,
                    "Resource not found")
            })
    }

    /// Get lifecycle statistics
    pub fn get_stats(&self) -> &LifecycleStats {
        &self.stats
    }

    /// Get current policies
    pub fn get_policies(&self) -> &LifecyclePolicies {
        &self.policies
    }

    /// Update lifecycle policies
    pub fn update_policies(&mut self, policies: LifecyclePolicies) {
        self.policies = policies;
    }

    /// Check for resource leaks
    #[cfg(feature = "std")]
    pub fn check_for_leaks(&mut self) -> Result<Vec<ResourceId>> {
        if !self.policies.leak_detection {
            return Ok(Vec::new();
        }

        let mut leaked_resources = Vec::new();
        let current_time = self.get_current_time);

        for resource in &self.resources {
            if let Some(max_lifetime) = self.policies.max_lifetime_ms {
                let age_ms = current_time - resource.created_at;
                if age_ms > max_lifetime && resource.ref_count > 0 {
                    let _ = leaked_resources.push(resource.id);
                }
            }
        }

        self.stats.leaks_detected += leaked_resources.len() as u32;
        Ok(leaked_resources)
    }
    
    /// Check for resource leaks (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn check_for_leaks(&mut self) -> core::result::Result<BoundedVec<ResourceId, 64, ResourceProvider>, Error> {
        if !self.policies.leak_detection {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            return Ok(BoundedVec::new().unwrap();
        }

        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut leaked_resources = BoundedVec::new().unwrap();
        let current_time = self.get_current_time);

        for resource in &self.resources {
            if let Some(max_lifetime) = self.policies.max_lifetime_ms {
                let age_ms = current_time - resource.created_at;
                if age_ms > max_lifetime && resource.ref_count > 0 {
                    let _ = leaked_resources.push(resource.id);
                }
            }
        }

        self.stats.leaks_detected += leaked_resources.len() as u32;
        Ok(leaked_resources)
    }

    // Private helper methods

    fn update_resource_state(&mut self, resource_id: ResourceId, new_state: ResourceState) -> Result<()> {
        let resource = self.get_resource_mut(resource_id)?;
        resource.state = new_state;
        Ok(()
    }

    fn execute_drop_handler(&mut self, handler_id: DropHandlerId, resource_id: ResourceId) -> Result<DropResult> {
        let handler = self.drop_handlers
            .iter()
            .find(|h| h.id == handler_id)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;

        // Simplified handler execution - in real implementation this would
        // call the actual drop handler function
        match &handler.handler_fn {
            DropHandlerFunction::StreamCleanup => {
                // Simulate stream cleanup
                self.stats.drop_handlers_executed += 1;
                Ok(DropResult::Success)
            }
            DropHandlerFunction::FutureCleanup => {
                // Simulate future cleanup
                self.stats.drop_handlers_executed += 1;
                Ok(DropResult::Success)
            }
            DropHandlerFunction::MemoryCleanup => {
                // Simulate memory cleanup
                self.stats.drop_handlers_executed += 1;
                Ok(DropResult::Success)
            }
            DropHandlerFunction::Custom { .. } => {
                // Simulate custom cleanup
                self.stats.drop_handlers_executed += 1;
                Ok(DropResult::Success)
            }
        }
    }

    fn is_handler_required(&self, handler_id: DropHandlerId) -> Result<bool> {
        let handler = self.drop_handlers
            .iter()
            .find(|h| h.id == handler_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::EXECUTION_ERROR,
                    "Drop handler not found")
            })?;
        
        Ok(handler.required)
    }

    fn should_collect_resource(&self, resource: &ResourceEntry) -> bool {
        if resource.ref_count > 0 {
            return false;
        }

        // Check age
        if let Some(max_lifetime) = self.policies.max_lifetime_ms {
            let age = self.get_current_time() - resource.created_at;
            if age > max_lifetime {
                return true;
            }
        }

        // Check last access time
        let idle_time = self.get_current_time() - resource.last_access;
        idle_time > 60000 // 1 minute idle time
    }

    fn get_current_time(&self) -> u64 {
        // Simplified time implementation - in real implementation would use proper time
        self.gc_state.gc_cycles * 1000 // Simulate time progression
    }
}

impl ResourceMetadata {
    /// Create new resource metadata
    pub fn new(name: &str) -> Result<Self> {
        Ok(Self {
            name: BoundedString::try_from_str(name).unwrap_or_default(),
            size_bytes: 0,
            #[cfg(feature = "std")]
            tags: Vec::new(),
            #[cfg(not(feature = "std"))]
            tags: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            properties: Vec::new(),
            #[cfg(not(feature = "std"))]
            properties: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
        })
    }

    /// Add a tag to the metadata
    pub fn add_tag(&mut self, tag: &str) -> Result<()> {
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_tag = BoundedString::try_from_str(tag).map_err(|_| {
            Error::runtime_execution_error("Error occurred")
        })?;
        
        self.tags.push(bounded_tag).map_err(|_| {
            Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Too many tags")
        })
    }

    /// Add a property to the metadata
    pub fn add_property(&mut self, key: &str, value: Value) -> Result<()> {
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_key = BoundedString::try_from_str(key).map_err(|_| {
            Error::runtime_execution_error("Error occurred")
        })?;
        
        self.properties.push((bounded_key, value)).map_err(|_| {
            Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Too many properties")
        })
    }
}

impl Default for LifecyclePolicies {
    fn default() -> Self {
        Self {
            enable_gc: true,
            gc_interval_ms: 10000, // 10 seconds
            max_lifetime_ms: Some(3600000), // 1 hour
            strict_ref_counting: true,
            leak_detection: true,
            max_memory_bytes: Some(100 * 1024 * 1024), // 100MB
        }
    }
}

impl LifecycleStats {
    /// Create new lifecycle statistics
    pub fn new() -> Self {
        Self {
            resources_created: 0,
            resources_destroyed: 0,
            active_resources: 0,
            drop_handlers_executed: 0,
            memory_used_bytes: 0,
            leaks_detected: 0,
        }
    }
}

impl GarbageCollectionState {
    /// Create new GC state
    pub fn new() -> Self {
        Self {
            last_gc_time: 0,
            gc_cycles: 0,
            last_collected: 0,
            gc_running: false,
        }
    }
}

impl Default for ResourceLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Stream => write!(f, "stream"),
            ResourceType::Future => write!(f, "future"),
            ResourceType::MemoryBuffer => write!(f, "memory-buffer"),
            ResourceType::FileHandle => write!(f, "file-handle"),
            ResourceType::NetworkConnection => write!(f, "network-connection"),
            ResourceType::Custom(id) => write!(f, "custom-{}", id),
        }
    }
}

impl fmt::Display for ResourceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceState::Creating => write!(f, "creating"),
            ResourceState::Active => write!(f, "active"),
            ResourceState::Destroying => write!(f, "destroying"),
            ResourceState::Destroyed => write!(f, "destroyed"),
            ResourceState::Error => write!(f, "error"),
        }

}
