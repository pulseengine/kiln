// Safety-critical imports for WRT allocator
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(all(feature = "std", not(feature = "safety-critical")))]
use std::collections::HashMap;
use std::{format, sync::Weak};

use crate::resources::ResourceInterceptor;

#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{WrtHashMap, WrtVec};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    resource::ResourceOperation as FormatResourceOperation,
    safe_managed_alloc,
};
use wrt_intercept::{InterceptionResult, builtins::InterceptContext as InterceptionContext};

use super::{
    buffer_pool::BufferPool,
    resource_operation::{from_format_resource_operation, to_format_resource_operation},
    size_class_buffer_pool::SizeClassBufferPool,
};
use crate::{bounded_component_infra::ComponentProvider, prelude::*};

/// Maximum number of resources that can be stored in a resource table
const MAX_RESOURCES: usize = 1024;

/// Resource instance representation
pub struct Resource {
    /// Resource type index
    pub type_idx: u32,
    /// Resource data (implementation-specific)
    pub data: Arc<dyn Any + Send + Sync>,
    /// Debug name for the resource (optional)
    pub name: Option<String>,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
}

impl Resource {
    /// Create a new resource
    pub fn new(type_idx: u32, data: Arc<dyn Any + Send + Sync>) -> Self {
        let now = Instant::now();
        Self {
            type_idx,
            data,
            name: None,
            created_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Create a new resource with a debug name
    pub fn new_with_name(type_idx: u32, data: Arc<dyn Any + Send + Sync>, name: &str) -> Self {
        let mut resource = Self::new(type_idx, data);
        resource.name = Some(name.to_string());
        resource
    }

    /// Record access to this resource
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Memory strategy for resource operations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryStrategy {
    /// Zero-copy strategy for trusted components
    ZeroCopy,
    /// Bounded-copy strategy with buffer pooling
    BoundedCopy,
    /// Full isolation with validation
    Isolated,
    /// Copy strategy - creates a copy of memory for safety
    Copy,
    /// Reference strategy - provides a direct reference to memory
    Reference,
    /// Full isolation with complete memory validation
    FullIsolation,
}

impl Default for MemoryStrategy {
    fn default() -> Self {
        Self::for_current_safety_level()
    }
}

impl MemoryStrategy {
    /// Get memory strategy based on current safety level features
    pub fn for_current_safety_level() -> Self {
        #[cfg(feature = "maximum-safety")]
        {
            MemoryStrategy::FullIsolation
        }
        #[cfg(all(feature = "static-memory-safety", not(feature = "maximum-safety")))]
        {
            MemoryStrategy::Isolated
        }
        #[cfg(all(
            feature = "bounded-collections",
            not(feature = "static-memory-safety"),
            not(feature = "maximum-safety")
        ))]
        {
            MemoryStrategy::BoundedCopy
        }
        #[cfg(all(
            feature = "dynamic-allocation",
            not(feature = "bounded-collections"),
            not(feature = "static-memory-safety"),
            not(feature = "maximum-safety")
        ))]
        {
            MemoryStrategy::ZeroCopy
        }
        #[cfg(not(any(
            feature = "dynamic-allocation",
            feature = "bounded-collections",
            feature = "static-memory-safety",
            feature = "maximum-safety"
        )))]
        {
            MemoryStrategy::BoundedCopy // Safe default
        }
    }
}

impl MemoryStrategy {
    /// Convert from u8 to MemoryStrategy
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(MemoryStrategy::ZeroCopy),
            1 => Some(MemoryStrategy::BoundedCopy),
            2 => Some(MemoryStrategy::Isolated),
            3 => Some(MemoryStrategy::Copy),
            4 => Some(MemoryStrategy::Reference),
            5 => Some(MemoryStrategy::FullIsolation),
            _ => None,
        }
    }

    /// Convert from MemoryStrategy to u8
    pub fn to_u8(self) -> u8 {
        match self {
            MemoryStrategy::ZeroCopy => 0,
            MemoryStrategy::BoundedCopy => 1,
            MemoryStrategy::Isolated => 2,
            MemoryStrategy::Copy => 3,
            MemoryStrategy::Reference => 4,
            MemoryStrategy::FullIsolation => 5,
        }
    }
}

/// Resource entry in the resource table (budget-aware)
#[derive(Clone)]
struct ResourceEntry {
    /// The resource instance
    resource: Arc<Mutex<Resource>>,
    /// Weak references to borrowed resources (budget-aware)
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    borrows: WrtVec<Weak<Mutex<Resource>>, { CrateId::Component as u8 }, 32>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    borrows: Vec<Weak<Mutex<Resource>>>,
    #[cfg(not(feature = "std"))]
    borrows: BoundedVec<Weak<Mutex<Resource>>, 32>,
    /// Memory strategy for this resource
    memory_strategy: MemoryStrategy,
    /// Verification level
    verification_level: VerificationLevel,
}

/// Verification level for resource operations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationLevel {
    /// No verification, fastest performance
    None,
    /// Verify only critical operations
    Critical,
    /// Verify all operations
    Full,
}

impl Default for VerificationLevel {
    fn default() -> Self {
        Self::for_current_safety_level()
    }
}

impl VerificationLevel {
    /// Get verification level based on current safety level features
    pub fn for_current_safety_level() -> Self {
        #[cfg(any(feature = "maximum-safety", feature = "verified-static-allocation"))]
        {
            VerificationLevel::Full
        }
        #[cfg(all(
            any(feature = "static-memory-safety", feature = "bounded-collections"),
            not(feature = "maximum-safety"),
            not(feature = "verified-static-allocation")
        ))]
        {
            VerificationLevel::Critical
        }
        #[cfg(all(
            feature = "dynamic-allocation",
            not(feature = "bounded-collections"),
            not(feature = "static-memory-safety"),
            not(feature = "maximum-safety")
        ))]
        {
            VerificationLevel::Off
        }
        #[cfg(not(any(
            feature = "dynamic-allocation",
            feature = "bounded-collections",
            feature = "static-memory-safety",
            feature = "maximum-safety"
        )))]
        {
            VerificationLevel::Critical // Safe default
        }
    }
}

/// Trait for buffer pools that can be used by ResourceTable
pub trait BufferPoolTrait: Send + Sync {
    /// Allocate a buffer of at least the specified size
    fn allocate(&mut self, size: usize) -> Vec<u8>;

    /// Return a buffer to the pool
    fn return_buffer(&mut self, buffer: Vec<u8>);

    /// Reset the buffer pool
    fn reset(&mut self);
}

impl BufferPoolTrait for BufferPool {
    fn allocate(&mut self, size: usize) -> Vec<u8> {
        self.allocate(size)
    }

    fn return_buffer(&mut self, buffer: Vec<u8>) {
        self.return_buffer(buffer)
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl BufferPoolTrait for SizeClassBufferPool {
    fn allocate(&mut self, size: usize) -> Vec<u8> {
        self.allocate(size)
    }

    fn return_buffer(&mut self, buffer: Vec<u8>) {
        self.return_buffer(buffer)
    }

    fn reset(&mut self) {
        self.reset()
    }
}

/// Budget-aware resource table with bounded memory allocation
///
/// This table integrates with the WRT memory budget system to ensure
/// static memory allocation for ASIL-D compliance.
///
/// SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
/// SW-REQ-ID: REQ_MEM_002 - Budget enforcement  
/// SW-REQ-ID: REQ_COMP_001 - Component isolation
#[derive(Clone)]
pub struct ResourceTable {
    /// Map of resource handles to resource entries (budget-aware)
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    resources: WrtHashMap<u32, ResourceEntry, { CrateId::Component as u8 }, 1024>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    resources: HashMap<u32, ResourceEntry>,
    #[cfg(not(feature = "std"))]
    resources: BoundedMap<u32, ResourceEntry, 1024, ComponentProvider>,
    /// Next available resource handle
    next_handle: u32,
    /// Maximum allowed resources
    max_resources: usize,
    /// Default memory strategy
    default_memory_strategy: MemoryStrategy,
    /// Default verification level
    default_verification_level: VerificationLevel,
    /// Buffer pool for bounded copy operations
    buffer_pool: Arc<Mutex<dyn BufferPoolTrait + Send + Sync>>,
    /// Interceptors for resource operations (budget-aware)
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    interceptors: WrtVec<Arc<dyn ResourceInterceptor>, { CrateId::Component as u8 }, 16>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    interceptors: Vec<Arc<dyn ResourceInterceptor>>,
    #[cfg(not(feature = "std"))]
    interceptors: BoundedVec<Arc<dyn ResourceInterceptor>, 16>,
    /// Memory budget guard for this resource table
    #[cfg(not(feature = "std"))]
    _memory_guard: ComponentProvider,
}

impl fmt::Debug for ResourceTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceTable")
            .field("resource_count", &self.resources.len())
            .field("next_handle", &self.next_handle)
            .field("max_resources", &self.max_resources)
            .field("default_memory_strategy", &self.default_memory_strategy)
            .field(
                "default_verification_level",
                &self.default_verification_level,
            )
            .field("interceptor_count", &self.interceptors.len())
            .finish()
    }
}

impl ResourceTable {
    /// Create a new budget-aware resource table with default settings
    ///
    /// This constructor integrates with the WRT memory budget system
    /// to ensure compile-time allocation enforcement.
    ///
    /// # ASIL-D Safety
    ///
    /// - Uses bounded collections with compile-time size limits
    /// - Integrates with budget enforcement system
    /// - Prevents runtime allocation failures
    pub fn new() -> wrt_error::Result<Self> {
        #[cfg(not(feature = "std"))]
        let memory_guard = wrt_foundation::safety_aware_alloc!(131072, CrateId::Component)?; // 128KB for resource table

        Ok(Self {
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            resources: WrtHashMap::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            resources: HashMap::new(),
            #[cfg(not(feature = "std"))]
            resources: BoundedMap::new(memory_guard.clone())?,
            next_handle: 1, // Start at 1 as 0 is reserved
            max_resources: MAX_RESOURCES,
            default_memory_strategy: MemoryStrategy::default(),
            default_verification_level: VerificationLevel::default(),
            buffer_pool: Arc::new(Mutex::new(BufferPool::new()))
                as Arc<Mutex<dyn BufferPoolTrait + Send + Sync>>,
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            interceptors: WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            interceptors: Vec::new(),
            #[cfg(not(feature = "std"))]
            interceptors: BoundedVec::new(memory_guard.clone())?,
            #[cfg(not(feature = "std"))]
            _memory_guard: memory_guard,
        })
    }

    /// Create a budget-aware ResourceTable factory function
    ///
    /// This is a convenience function that integrates with the memory budget
    /// system and should be used by component instantiation code.
    ///
    /// # ASIL-D Safety
    ///
    /// This function ensures all resource table allocations are tracked
    /// by the budget enforcement system.
    pub fn new_budget_aware() -> wrt_error::Result<Self> {
        Self::new()
    }

    /// Create a new resource table with optimized size-class buffer pool
    pub fn new_with_optimized_memory() -> wrt_error::Result<Self> {
        #[cfg(not(feature = "std"))]
        let memory_guard = wrt_foundation::safety_aware_alloc!(131072, CrateId::Component)?; // 128KB for resource table

        Ok(Self {
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            resources: WrtHashMap::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            resources: HashMap::new(),
            #[cfg(not(feature = "std"))]
            resources: BoundedMap::new(memory_guard.clone())?,
            next_handle: 1,
            max_resources: MAX_RESOURCES,
            default_memory_strategy: MemoryStrategy::default(),
            default_verification_level: VerificationLevel::default(),
            buffer_pool: Arc::new(Mutex::new(SizeClassBufferPool::new()))
                as Arc<Mutex<dyn BufferPoolTrait + Send + Sync>>,
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            interceptors: WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            interceptors: Vec::new(),
            #[cfg(not(feature = "std"))]
            interceptors: BoundedVec::new(memory_guard.clone())?,
            #[cfg(not(feature = "std"))]
            _memory_guard: memory_guard,
        })
    }

    /// Create a new resource table with custom settings
    pub fn new_with_config(
        max_resources: usize,
        memory_strategy: MemoryStrategy,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            #[cfg(feature = "safety-critical")]
            resources: WrtHashMap::new(),
            #[cfg(not(feature = "safety-critical"))]
            resources: HashMap::new(),
            next_handle: 1,
            max_resources,
            default_memory_strategy: memory_strategy,
            default_verification_level: verification_level,
            buffer_pool: Arc::new(Mutex::new(BufferPool::new()))
                as Arc<Mutex<dyn BufferPoolTrait + Send + Sync>>,
            #[cfg(feature = "safety-critical")]
            interceptors: WrtVec::new(),
            #[cfg(not(feature = "safety-critical"))]
            interceptors: Vec::new(),
        }
    }

    /// Create a new resource table with custom settings and optimized memory
    pub fn new_with_config_and_optimized_memory(
        max_resources: usize,
        memory_strategy: MemoryStrategy,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            #[cfg(feature = "safety-critical")]
            resources: WrtHashMap::new(),
            #[cfg(not(feature = "safety-critical"))]
            resources: HashMap::new(),
            next_handle: 1,
            max_resources,
            default_memory_strategy: memory_strategy,
            default_verification_level: verification_level,
            buffer_pool: Arc::new(Mutex::new(SizeClassBufferPool::new()))
                as Arc<Mutex<dyn BufferPoolTrait + Send + Sync>>,
            #[cfg(feature = "safety-critical")]
            interceptors: WrtVec::new(),
            #[cfg(not(feature = "safety-critical"))]
            interceptors: Vec::new(),
        }
    }

    /// Add a resource interceptor
    pub fn add_interceptor(&mut self, interceptor: Arc<dyn ResourceInterceptor>) -> Result<()> {
        #[cfg(feature = "safety-critical")]
        {
            self.interceptors
                .push(interceptor)
                .map_err(|_| Error::resource_exhausted("Resource operation failed"))
        }
        #[cfg(not(feature = "safety-critical"))]
        {
            self.interceptors.push(interceptor);
            Ok(())
        }
    }

    /// Create a new resource
    pub fn create_resource(
        &mut self,
        type_idx: u32,
        data: Arc<dyn Any + Send + Sync>,
    ) -> Result<u32> {
        // Check if we've reached the maximum number of resources
        if self.resources.len() >= self.max_resources {
            return Err(Error::resource_exhausted(
                "Maximum number of resources reached",
            ));
        }

        // Create the resource
        let resource = Resource::new(type_idx, data);

        // Notify interceptors about resource creation
        for interceptor in &self.interceptors {
            interceptor.on_resource_create(type_idx, &resource)?;
        }

        // Assign a handle
        let handle = self.next_handle;
        self.next_handle += 1;

        let entry = ResourceEntry {
            resource: Arc::new(Mutex::new(resource)),
            #[cfg(feature = "safety-critical")]
            borrows: WrtVec::new(),
            #[cfg(not(feature = "safety-critical"))]
            borrows: Vec::new(),
            memory_strategy: self
                .get_strategy_from_interceptors(handle)
                .unwrap_or(self.default_memory_strategy),
            verification_level: self.default_verification_level,
        };

        #[cfg(feature = "safety-critical")]
        {
            self.resources.insert(handle, entry).map_err(|_| {
                Error::resource_exhausted("Failed to insert resource: capacity exceeded")
            })?;
        }
        #[cfg(not(feature = "safety-critical"))]
        {
            self.resources.insert(handle, entry);
        }

        Ok(handle)
    }

    /// Create a borrowed reference to a resource
    pub fn borrow_resource(&mut self, handle: u32) -> Result<u32> {
        // Check if the resource exists
        let resource_opt = self.resources.get(&handle).map(|entry| entry.resource.clone());

        let resource = match resource_opt {
            Some(r) => r,
            None => {
                return Err(Error::resource_error("Resource not found"));
            },
        };

        // Notify interceptors about resource borrowing
        for interceptor in &self.interceptors {
            interceptor.on_resource_borrow(handle)?;
        }

        // Create a new handle for the borrowed resource
        let borrow_handle = self.next_handle;
        self.next_handle += 1;

        // Store the weak reference in the original resource
        let weak_ref = Arc::downgrade(&resource);
        if let Some(entry) = self.resources.get_mut(&handle) {
            #[cfg(feature = "safety-critical")]
            {
                entry.borrows.push(weak_ref).map_err(|_| {
                    Error::resource_exhausted("Failed to add resource borrow: capacity exceeded")
                })?;
            }
            #[cfg(not(feature = "safety-critical"))]
            {
                entry.borrows.push(weak_ref);
            }
        }

        // Store the borrowed resource
        #[cfg(feature = "safety-critical")]
        {
            self.resources
                .insert(
                    borrow_handle,
                    ResourceEntry {
                        resource,
                        borrows: WrtVec::new(),
                        memory_strategy: self.default_memory_strategy,
                        verification_level: self.default_verification_level,
                    },
                )
                .map_err(|_| {
                    Error::resource_exhausted(
                        "Failed to store borrowed resource: capacity exceeded",
                    )
                })?;
        }
        #[cfg(not(feature = "safety-critical"))]
        {
            self.resources.insert(
                borrow_handle,
                ResourceEntry {
                    resource,
                    borrows: Vec::new(),
                    memory_strategy: self.default_memory_strategy,
                    verification_level: self.default_verification_level,
                },
            );
        }

        Ok(borrow_handle)
    }

    /// Drop a resource
    pub fn drop_resource(&mut self, handle: u32) -> Result<()> {
        // Check if the resource exists
        if !self.resources.contains_key(&handle) {
            return Err(Error::resource_error("Resource not found"));
        }

        // Notify interceptors about resource dropping
        for interceptor in &self.interceptors {
            interceptor.on_resource_drop(handle)?;
        }

        // Remove the resource
        self.resources.remove(&handle);

        Ok(())
    }

    /// Get a resource by handle
    pub fn get_resource(&self, handle: u32) -> Result<Arc<Mutex<Resource>>> {
        // Check if the resource exists
        let entry = self
            .resources
            .get(&handle)
            .ok_or_else(|| Error::resource_error("Resource not found"))?;

        // Record access
        {
            let mut resource = entry.resource.lock();
            resource.record_access();
        }

        // Notify interceptors about resource access
        for interceptor in &self.interceptors {
            interceptor.on_resource_access(handle)?;
        }

        Ok(entry.resource.clone())
    }

    /// Apply an operation to a resource
    pub fn apply_operation(
        &mut self,
        handle: u32,
        operation: FormatResourceOperation,
    ) -> Result<ComponentValue<ComponentProvider>> {
        // Check if the resource exists
        if !self.resources.contains_key(&handle) {
            return Err(Error::resource_error("Resource not found"));
        }

        // Check interceptors first
        for interceptor in &self.interceptors {
            // Pass the format operation to interceptors
            interceptor.on_resource_operation(handle, &operation)?;

            // Check if the interceptor will override the operation
            if let Some(result) = interceptor.intercept_resource_operation(handle, &operation)? {
                // If the interceptor provides a result, use it
                // Use the conversion utilities from type_conversion module
                return Ok(ComponentValue::U32(handle));
            }
        }

        // Apply the operation based on the resource
        match operation {
            FormatResourceOperation::Rep => {
                // Representation operation - convert resource to its representation
                let _resource = self.resources.get(&handle).unwrap();
                Ok(ComponentValue::U32(handle))
            },
            FormatResourceOperation::Drop => {
                // Drop operation - remove the resource from the table
                let _resource = self.resources.remove(&handle).unwrap();
                Ok(ComponentValue::Void)
            },
            FormatResourceOperation::New => {
                // New operation - creates a resource from its representation
                // Binary std/no_std choice
                // working with an existing handle
                Ok(ComponentValue::U32(handle))
            },
            _ => {
                // For other operations, return a default value
                Ok(ComponentValue::Void)
            },
        }
    }

    /// Set memory strategy for a resource
    pub fn set_memory_strategy(&mut self, handle: u32, strategy: MemoryStrategy) -> Result<()> {
        // Check if the resource exists
        let entry = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_error("Resource not found"))?;

        entry.memory_strategy = strategy;
        Ok(())
    }

    /// Set verification level for a resource
    pub fn set_verification_level(&mut self, handle: u32, level: VerificationLevel) -> Result<()> {
        // Check if the resource exists
        let entry = self
            .resources
            .get_mut(&handle)
            .ok_or_else(|| Error::resource_error("Resource not found"))?;

        entry.verification_level = level;
        Ok(())
    }

    /// Set default verification level for new resources
    pub fn set_default_verification_level(&mut self, level: VerificationLevel) {
        self.default_verification_level = level;
    }

    /// Get the number of resources in the table
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Clean up unused resources
    pub fn cleanup_unused_resources(&mut self) -> usize {
        let handles_to_remove: Vec<u32> = self
            .resources
            .iter()
            .filter(|(_, entry)| Arc::strong_count(&entry.resource) <= 1)
            .map(|(handle, _)| *handle)
            .collect();

        for handle in &handles_to_remove {
            self.resources.remove(handle);
        }

        handles_to_remove.len()
    }

    /// Get a buffer from the pool
    pub fn get_buffer(&mut self, size: usize) -> Vec<u8> {
        self.buffer_pool.lock().allocate(size)
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&mut self, buffer: Vec<u8>) {
        self.buffer_pool.lock().return_buffer(buffer)
    }

    /// Reset the buffer pool, clearing all pooled buffers
    pub fn reset_buffer_pool(&mut self) {
        self.buffer_pool.lock().reset()
    }

    /// Get memory strategy from interceptors
    pub fn get_strategy_from_interceptors(&self, handle: u32) -> Option<MemoryStrategy> {
        for interceptor in &self.interceptors {
            if let Some(strategy_val) = interceptor.get_memory_strategy(handle) {
                if let Some(strategy) = MemoryStrategy::from_u8(strategy_val) {
                    return Some(strategy);
                }
            }
        }
        None
    }
}
