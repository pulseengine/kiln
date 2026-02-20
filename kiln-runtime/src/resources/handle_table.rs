//! Resource handle management for WebAssembly Component Model
//!
//! This module implements resource handle tables using bounded collections,
//! providing predictable memory usage for embedded/no_std environments.
//!
//! Based on the Component Model MVP design:
//! - Owned handles (own<T>) represent unique ownership
//! - Borrowed handles (borrow<T>) represent temporary access
//! - Handles are 32-bit integers indexing into type-specific tables

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::{
    bounded::BoundedVec,
    traits::BoundedCapacity,
    MemoryProvider,
};

/// Maximum number of resources per type
/// Component Model suggests this as a reasonable limit
pub const MAX_RESOURCES_PER_TYPE: usize = 1024;

/// Resource handle (32-bit index)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceHandle(pub u32);

/// Resource ownership type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOwnership {
    /// Owned resource - must be explicitly dropped
    Owned,
    /// Borrowed resource - temporary access
    Borrowed,
}

impl wrt_foundation::traits::Checksummable for ResourceOwnership {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            ResourceOwnership::Owned => 0u8.update_checksum(checksum),
            ResourceOwnership::Borrowed => 1u8.update_checksum(checksum),
        }
    }
}

impl wrt_foundation::traits::ToBytes for ResourceOwnership {
    fn serialized_size(&self) -> usize {
        1 // single byte for discriminant
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> Result<()> {
        match self {
            ResourceOwnership::Owned => writer.write_u8(0),
            ResourceOwnership::Borrowed => writer.write_u8(1),
        }
    }
}

impl wrt_foundation::traits::FromBytes for ResourceOwnership {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> Result<Self> {
        match reader.read_u8()? {
            0 => Ok(ResourceOwnership::Owned),
            1 => Ok(ResourceOwnership::Borrowed),
            _ => Err(Error::validation_invalid_state(
                "Invalid ResourceOwnership discriminant",
            )),
        }
    }
}

/// Resource entry in the handle table
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry<T>
where
    T: Clone + PartialEq + Eq,
{
    /// The actual resource
    pub resource:  T,
    /// Ownership type
    pub ownership: ResourceOwnership,
    /// Reference count for borrowed handles
    pub ref_count: u32,
}

impl<T> wrt_foundation::traits::Checksummable for ResourceEntry<T>
where
    T: Clone + PartialEq + Eq + wrt_foundation::traits::Checksummable,
{
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.resource.update_checksum(checksum);
        self.ownership.update_checksum(checksum);
        self.ref_count.update_checksum(checksum);
    }
}

impl<T> wrt_foundation::traits::ToBytes for ResourceEntry<T>
where
    T: Clone + PartialEq + Eq + wrt_foundation::traits::ToBytes,
{
    fn serialized_size(&self) -> usize {
        self.resource.serialized_size()
            + self.ownership.serialized_size()
            + self.ref_count.serialized_size()
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> Result<()> {
        self.resource.to_bytes_with_provider(writer, provider)?;
        self.ownership.to_bytes_with_provider(writer, provider)?;
        self.ref_count.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl<T> wrt_foundation::traits::FromBytes for ResourceEntry<T>
where
    T: Clone + PartialEq + Eq + wrt_foundation::traits::FromBytes,
{
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> Result<Self> {
        let resource = T::from_bytes_with_provider(reader, provider)?;
        let ownership = ResourceOwnership::from_bytes_with_provider(reader, provider)?;
        let ref_count = u32::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            resource,
            ownership,
            ref_count,
        })
    }
}

/// Resource handle table for a specific resource type
pub struct ResourceTable<T, P: MemoryProvider + Default + Clone + PartialEq + Eq>
where
    T: Clone
        + PartialEq
        + Eq
        + wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes,
{
    /// Table entries indexed by handle
    #[cfg(feature = "std")]
    entries:     Vec<Option<ResourceEntry<T>>>,
    #[cfg(not(feature = "std"))]
    entries:     BoundedVec<Option<ResourceEntry<T>>, MAX_RESOURCES_PER_TYPE, P>,
    /// Next available handle
    next_handle: u32,
    /// Memory provider (kept for API compatibility)
    _provider:   core::marker::PhantomData<P>,
}

impl<T, P: MemoryProvider + Default + Clone + PartialEq + Eq> ResourceTable<T, P>
where
    T: Clone
        + PartialEq
        + Eq
        + wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes,
{
    /// Create a new resource table
    pub fn new(provider: P) -> Result<Self> {
        // Start with empty table - entries are allocated lazily as needed
        Ok(Self {
            #[cfg(feature = "std")]
            entries: Vec::new(),
            #[cfg(not(feature = "std"))]
            entries: BoundedVec::new(provider)?,
            next_handle: 1, // 0 is reserved for null handle
            _provider: core::marker::PhantomData,
        })
    }

    /// Allocate a new owned resource
    pub fn new_own(&mut self, resource: T) -> Result<ResourceHandle> {
        let handle = self.allocate_handle()?;
        let entry = ResourceEntry {
            resource,
            ownership: ResourceOwnership::Owned,
            ref_count: 0,
        };

        #[cfg(feature = "std")]
        {
            // Extend Vec if needed
            while self.entries.len() <= handle.0 as usize {
                self.entries.push(None);
            }
            self.entries[handle.0 as usize] = Some(entry);
        }

        #[cfg(not(feature = "std"))]
        {
            // Extend BoundedVec if needed
            if handle.0 as usize >= self.entries.len() {
                while self.entries.len() <= handle.0 as usize {
                    self.entries.push(None).map_err(|_| {
                        Error::capacity_limit_exceeded("Resource table capacity exceeded")
                    })?;
                }
            }
            let _old_entry = self
                .entries
                .set(handle.0 as usize, Some(entry))
                .map_err(|_| Error::resource_error("Failed to set resource entry"))?;
        }

        Ok(handle)
    }

    /// Create a borrowed handle from an owned handle
    pub fn new_borrow(&mut self, owned: ResourceHandle) -> Result<ResourceHandle> {
        #[cfg(feature = "std")]
        {
            let entry = self
                .entries
                .get_mut(owned.0 as usize)
                .and_then(|opt| opt.as_mut())
                .ok_or_else(|| Error::resource_invalid_handle("Invalid owned handle"))?;

            if entry.ownership != ResourceOwnership::Owned {
                return Err(Error::resource_invalid_handle(
                    "Can only borrow from owned resources",
                ));
            }

            entry.ref_count += 1;
            Ok(owned)
        }

        #[cfg(not(feature = "std"))]
        {
            let current_entry = self
                .entries
                .get(owned.0 as usize)
                .map_err(|_| Error::resource_invalid_handle("Invalid owned handle index"))?;

            if current_entry.is_none() {
                return Err(Error::resource_invalid_handle(
                    "Invalid owned handle - no entry",
                ));
            }

            let mut entry = current_entry.unwrap();
            if entry.ownership != ResourceOwnership::Owned {
                return Err(Error::resource_invalid_handle(
                    "Can only borrow from owned resources",
                ));
            }

            entry.ref_count += 1;
            let _old = self
                .entries
                .set(owned.0 as usize, Some(entry))
                .map_err(|_| Error::resource_error("Failed to update resource entry"))?;
            Ok(owned)
        }
    }

    /// Get a resource by handle
    pub fn get(&self, handle: ResourceHandle) -> Option<&T> {
        #[cfg(feature = "std")]
        {
            self.entries
                .get(handle.0 as usize)?
                .as_ref()
                .map(|entry| &entry.resource)
        }
        #[cfg(not(feature = "std"))]
        {
            // BoundedVec's get returns Result<T, _>, not Option<&T>
            // We can't return a reference in no_std mode
            let _ = handle;
            None
        }
    }

    /// Get a mutable resource by handle (only for owned)
    pub fn get_mut(&mut self, handle: ResourceHandle) -> Option<&mut T> {
        #[cfg(feature = "std")]
        {
            self.entries
                .get_mut(handle.0 as usize)?
                .as_mut()
                .map(|entry| &mut entry.resource)
        }
        #[cfg(not(feature = "std"))]
        {
            // BoundedVec doesn't support get_mut
            let _ = handle;
            None
        }
    }

    /// Drop a resource handle
    pub fn drop_handle(&mut self, handle: ResourceHandle) -> Result<Option<T>> {
        #[cfg(feature = "std")]
        {
            let entry = self
                .entries
                .get_mut(handle.0 as usize)
                .and_then(|opt| opt.as_mut())
                .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

            // If ref_count > 0, this is dropping a borrow (decrement ref_count)
            // If ref_count == 0, this is dropping the owned resource (remove entry)
            if entry.ref_count > 0 {
                entry.ref_count -= 1;
                Ok(None)
            } else {
                // Take ownership and remove from table
                let entry = self.entries[handle.0 as usize]
                    .take()
                    .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;
                Ok(Some(entry.resource))
            }
        }

        #[cfg(not(feature = "std"))]
        {
            let entry = self
                .entries
                .get(handle.0 as usize)
                .map_err(|_| Error::resource_invalid_handle("Invalid handle index"))?
                .ok_or_else(|| Error::resource_invalid_handle("Invalid resource handle"))?;

            // Remove the entry by setting it to None
            let _old = self
                .entries
                .set(handle.0 as usize, None)
                .map_err(|_| Error::resource_error("Failed to remove resource entry"))?;

            match entry.ownership {
                ResourceOwnership::Owned => {
                    if entry.ref_count > 0 {
                        // Put it back, still has borrows
                        let _old = self
                            .entries
                            .set(handle.0 as usize, Some(entry))
                            .map_err(|_| Error::resource_error("Failed to restore resource entry"))?;
                        return Err(Error::resource_error(
                            "Cannot drop owned resource with active borrows",
                        ));
                    }
                    Ok(Some(entry.resource))
                },
                ResourceOwnership::Borrowed => {
                    // Decrement ref count on the owned resource
                    if let Ok(Some(mut owned_entry)) = self.entries.get(handle.0 as usize) {
                        owned_entry.ref_count = owned_entry.ref_count.saturating_sub(1);
                        let _old = self
                            .entries
                            .set(handle.0 as usize, Some(owned_entry))
                            .map_err(|_| Error::resource_error("Failed to update ref count"))?;
                    }
                    Ok(None)
                },
            }
        }
    }

    /// Allocate a new handle
    fn allocate_handle(&mut self) -> Result<ResourceHandle> {
        // Simple linear search for now
        let start = self.next_handle as usize;
        for i in 0..MAX_RESOURCES_PER_TYPE {
            let index = (start + i) % MAX_RESOURCES_PER_TYPE;
            if index == 0 {
                continue;
            } // Skip 0 (null handle)

            #[cfg(feature = "std")]
            let is_available = self.entries.get(index).map_or(true, |opt| opt.is_none());
            #[cfg(not(feature = "std"))]
            let is_available = self.entries.get(index).ok().flatten().is_none();

            if is_available {
                self.next_handle = (index + 1) as u32;
                return Ok(ResourceHandle(index as u32));
            }
        }

        Err(Error::resource_limit_exceeded("Resource table full"))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use std::string::String as StdString;

    use wrt_foundation::traits::DefaultMemoryProvider;

    use super::*;
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::string::String as StdString;

    #[test]
    #[cfg(feature = "std")]
    fn test_resource_table_basic() {
        // Use a provider with sufficient capacity for the test
        // BoundedVec needs space for metadata, serialization buffers, and items
        // Allocate generous capacity for the test
        use wrt_foundation::safe_memory::NoStdProvider;
        let provider = NoStdProvider::<8192>::default();
        let mut table = ResourceTable::<u32, _>::new(provider).unwrap();

        // Create owned resource
        let owned = table.new_own(42u32).unwrap();
        assert_eq!(table.get(owned), Some(&42u32));

        // Create borrowed handle (returns same handle, increments ref_count)
        let borrowed = table.new_borrow(owned).unwrap();
        assert_eq!(table.get(borrowed), Some(&42u32));
        assert_eq!(owned, borrowed); // Same handle value

        // Drop a borrow (decrements ref_count from 1 to 0)
        assert_eq!(table.drop_handle(borrowed).unwrap(), None);

        // Now can drop owned (ref_count is 0)
        let resource = table.drop_handle(owned).unwrap();
        assert_eq!(resource, Some(42u32));
    }
}
