//! Resource table implementation for no_std environments

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

// Implement required traits for BoundedVec compatibility
use wrt_foundation::{
    bounded::BoundedString,
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
};

use super::{Instant, ResourceId};

// Macro to implement basic traits for complex types
macro_rules! impl_basic_traits {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                0u32.update_checksum(checksum);
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

/// Maximum number of resources that can be stored in a resource table
const MAX_RESOURCES: usize = 1024;

/// Resource instance representation for no_std
#[derive(Debug, Clone)]
pub struct Resource {
    /// Resource type index
    pub type_idx: u32,
    /// Resource data pointer (simplified for no_std)
    pub data_ptr: usize,
    /// Debug name for the resource (optional)
    pub name: Option<BoundedString<64>>,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
}

impl Resource {
    /// Create a new resource
    pub fn new(type_idx: u32, data_ptr: usize) -> Self {
        let now = Instant::now();
        Self {
            type_idx,
            data_ptr,
            name: None,
            created_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Create a new resource with a debug name
    pub fn new_with_name(type_idx: u32, data_ptr: usize, name: &str) -> Self {
        let mut resource = Self::new(type_idx, data_ptr);
        if let Ok(provider) = safe_managed_alloc!(512, CrateId::Component) {
            resource.name = BoundedString::try_from_str(name).ok();
        }
        resource
    }

    /// Record access to this resource
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Memory strategy for no_std
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryStrategy {
    /// Use a fixed-size buffer
    FixedBuffer,
    /// Use bounded collections
    BoundedCollections,
    /// Bounded copy strategy - copy with size limits
    BoundedCopy,
    /// Zero-copy strategy - pass references
    ZeroCopy,
    /// Isolated memory regions
    Isolated,
    /// Reference-based strategy
    Reference,
    /// Full isolation between components
    FullIsolation,
    /// Copy strategy - always copy data
    Copy,
}

impl Default for MemoryStrategy {
    fn default() -> Self {
        Self::BoundedCollections
    }
}

/// Verification level for resource operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationLevel {
    /// No verification
    None,
    /// Basic checks
    Basic,
    /// Full verification
    Full,
    /// Critical safety-level verification
    Critical,
}

impl Default for VerificationLevel {
    fn default() -> Self {
        Self::Basic
    }
}

/// Buffer pool trait for no_std
pub trait BufferPoolTrait {
    /// Allocate a buffer
    fn allocate(&mut self, size: usize) -> Option<usize>;

    /// Deallocate a buffer
    fn deallocate(&mut self, ptr: usize, size: usize);

    /// Get available memory
    fn available_memory(&self) -> usize;
}

/// Resource table for managing component resources in no_std
#[derive(Debug, Clone)]
pub struct ResourceTable {
    /// Storage for resources
    resources: BoundedVec<Option<Resource>, MAX_RESOURCES>,
    /// Next available resource ID
    next_id: u32,
    /// Memory strategy
    memory_strategy: MemoryStrategy,
    /// Verification level
    verification_level: VerificationLevel,
}

impl ResourceTable {
    /// Create a new resource table
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            resources: BoundedVec::new().unwrap(),
            next_id: 1,
            memory_strategy: MemoryStrategy::default(),
            verification_level: VerificationLevel::default(),
        })
    }

    /// Create a new resource table with configuration
    pub fn with_config(
        memory_strategy: MemoryStrategy,
        verification_level: VerificationLevel,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            resources: BoundedVec::new().unwrap(),
            next_id: 1,
            memory_strategy,
            verification_level,
        })
    }

    /// Insert a resource and return its ID
    pub fn insert(&mut self, resource: Resource) -> wrt_error::Result<ResourceId> {
        let id = ResourceId(self.next_id);
        self.next_id += 1;

        // Find an empty slot or add to the end
        for (i, slot) in self.resources.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(resource);
                return Ok(ResourceId(i as u32 + 1));
            }
        }

        // No empty slot found, try to add new one
        self.resources
            .push(Some(resource))
            .map_err(|_| wrt_error::Error::resource_exhausted("Error occurred"))?;

        Ok(id)
    }

    /// Get a resource by ID
    pub fn get(&self, id: ResourceId) -> Option<&Resource> {
        let index = (id.0.saturating_sub(1)) as usize;
        self.resources.get(index)?.as_ref()
    }

    /// Get a mutable resource by ID
    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut Resource> {
        let index = (id.0.saturating_sub(1)) as usize;
        self.resources.get_mut(index)?.as_mut()
    }

    /// Remove a resource by ID
    pub fn remove(&mut self, id: ResourceId) -> Option<Resource> {
        let index = (id.0.saturating_sub(1)) as usize;
        self.resources.get_mut(index)?.take()
    }

    /// Get the number of stored resources
    pub fn len(&self) -> usize {
        self.resources.iter().filter(|r| r.is_some()).count()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get memory strategy
    pub fn memory_strategy(&self) -> MemoryStrategy {
        self.memory_strategy
    }

    /// Get verification level
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Set verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Create a resource with type and data pointer (compatibility method)
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn create_resource(
        &mut self,
        type_idx: u32,
        data: Box<dyn core::any::Any + Send + Sync>,
    ) -> wrt_error::Result<u32> {
        // Convert Box to raw pointer
        let data_ptr = Box::into_raw(data) as *mut () as usize;

        // Create resource
        let resource = Resource::new(type_idx, data_ptr);

        // Insert and return the ID as handle
        let resource_id = self.insert(resource)?;
        Ok(resource_id.0)
    }

    /// Get a resource by handle (compatibility method)
    /// Returns the resource ID for get/get_mut operations
    pub fn get_resource(&self, handle: u32) -> wrt_error::Result<ResourceId> {
        let id = ResourceId(handle);
        if self.get(id).is_some() {
            Ok(id)
        } else {
            Err(wrt_error::Error::resource_error("Resource not found"))
        }
    }

    /// Drop/remove a resource by handle (compatibility method)
    pub fn drop_resource(&mut self, handle: u32) -> wrt_error::Result<()> {
        let id = ResourceId(handle);
        self.remove(id)
            .ok_or_else(|| wrt_error::Error::resource_error("Resource not found"))?;
        Ok(())
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new().expect("Failed to create default ResourceTable")
    }
}

// Apply traits to the main types
impl_basic_traits!(Resource, Resource::new(0, 0));
impl_basic_traits!(ResourceTable, ResourceTable::new().unwrap());
impl_basic_traits!(MemoryStrategy, MemoryStrategy::default());
impl_basic_traits!(VerificationLevel, VerificationLevel::default());
