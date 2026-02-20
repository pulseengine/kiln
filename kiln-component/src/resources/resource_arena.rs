//! Resource arena for managing resource lifecycles as a group
//!
//! ResourceArena provides efficient group management of resources, allowing
//! components to manage resources with a shared lifetime. This is particularly
//! useful for component instances where resources should be cleaned up together.
//!
//! This module supports both std and no_std environments with appropriate
//! implementations for each.

// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use super::{ResourceId, ResourceTable};
use crate::prelude::*;

// ============================================================================
// std implementation - uses Vec and Arc<Mutex<ResourceTable>>
// ============================================================================

#[cfg(feature = "std")]
mod std_impl {
    use std::sync::Mutex;

    use wrt_error::Error;
    use wrt_sync::WrtMutex;

    use super::*;

    /// A resource arena for managing resource lifecycles as a group
    ///
    /// ResourceArena provides efficient group management of resources.
    pub struct ResourceArena {
        /// Handles to resources managed by this arena
        resources: Vec<u32>,
        /// The resource table used for actual resource management
        table: Arc<Mutex<ResourceTable>>,
        /// Name of this arena, for debugging
        name: Option<String>,
    }

    impl ResourceArena {
        /// Create a new resource arena with the given resource table
        pub fn new(table: Arc<Mutex<ResourceTable>>) -> Self {
            Self {
                resources: Vec::new(),
                table,
                name: None,
            }
        }

        /// Create a new resource arena with the given name
        pub fn new_with_name(table: Arc<Mutex<ResourceTable>>, name: &str) -> Self {
            Self {
                resources: Vec::new(),
                table,
                name: Some(name.to_string()),
            }
        }

        /// Create a resource in this arena
        ///
        /// The resource will be automatically cleaned up when the arena is dropped
        /// or when release_all() is called.
        pub fn create_resource(
            &mut self,
            type_idx: u32,
            data: Arc<dyn Any + Send + Sync>,
        ) -> Result<u32> {
            let mut table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            let handle = table.create_resource(type_idx, data)?;
            self.resources.push(handle);
            Ok(handle)
        }

        /// Create a named resource in this arena
        pub fn create_named_resource(
            &mut self,
            type_idx: u32,
            data: Arc<dyn Any + Send + Sync>,
            name: &str,
        ) -> Result<u32> {
            let mut table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            // Create the resource
            let handle = table.create_resource(type_idx, data)?;

            // Set the name if we have access to the resource
            if let Ok(res) = table.get_resource(handle) {
                let mut res_guard = res.lock();
                res_guard.name = Some(name.to_string());
            }

            // Add to arena's managed resources
            self.resources.push(handle);

            Ok(handle)
        }

        /// Create a resource in this arena using a ResourceId
        ///
        /// Convenience method for APIs that use ResourceId
        pub fn add_resource<T: 'static + Send + Sync>(
            &mut self,
            resource: T,
        ) -> Result<ResourceId> {
            let handle = self.create_resource(0, Arc::new(resource))?;
            Ok(ResourceId(handle))
        }

        /// Get access to a resource
        pub fn get_resource(&self, handle: u32) -> Result<Arc<WrtMutex<super::super::Resource>>> {
            let table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            table.get_resource(handle)
        }

        /// Check if a resource exists
        pub fn has_resource(&self, id: ResourceId) -> Result<bool> {
            let table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            // First check if it's in our arena
            if !self.resources.contains(&id.0) {
                return Ok(false);
            }

            // Then check if it exists in the table
            match table.get_resource(id.0) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }

        /// Remove a resource from this arena's management
        ///
        /// This does not drop the resource, just removes it from the arena.
        /// Returns true if the resource was found and removed.
        pub fn remove_resource(&mut self, handle: u32) -> bool {
            if let Some(pos) = self.resources.iter().position(|&h| h == handle) {
                self.resources.swap_remove(pos);
                true
            } else {
                false
            }
        }

        /// Drop a specific resource and remove it from this arena
        pub fn drop_resource(&mut self, handle: u32) -> Result<()> {
            // First remove it from our tracking
            if !self.remove_resource(handle) {
                return Err(Error::resource_error("Error occurred"));
            }

            // Then drop it from the table
            let mut table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            table.drop_resource(handle)
        }

        /// Release all resources managed by this arena
        pub fn release_all(&mut self) -> Result<()> {
            if self.resources.is_empty() {
                return Ok(());
            }

            let mut table =
                self.table.lock().map_err(|_| Error::runtime_poisoned_lock("Error occurred"))?;

            let mut error = None;

            // Try to drop all resources, continuing even if some fail
            for handle in self.resources.drain(..) {
                if let Err(e) = table.drop_resource(handle) {
                    // Store the first error but continue trying to drop others
                    if error.is_none() {
                        error = Some(e);
                    }
                }
            }

            // Return the first error if any occurred
            if let Some(e) = error { Err(e) } else { Ok(()) }
        }

        /// Get the number of resources in this arena
        pub fn resource_count(&self) -> usize {
            self.resources.len()
        }

        /// Get the name of this arena
        pub fn name(&self) -> Option<&str> {
            self.name.as_deref()
        }

        /// Set the name of this arena
        pub fn set_name(&mut self, name: &str) {
            self.name = Some(name.to_string());
        }

        /// Get all resources managed by this arena
        pub fn resources(&self) -> &[u32] {
            &self.resources
        }
    }

    impl Drop for ResourceArena {
        fn drop(&mut self) {
            // Try to release all resources, ignoring errors
            let _ = self.release_all();
        }
    }

    impl fmt::Debug for ResourceArena {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ResourceArena")
                .field("name", &self.name)
                .field("resource_count", &self.resources.len())
                .field("resources", &self.resources)
                .finish()
        }
    }
}

// ============================================================================
// no_std implementation - uses BoundedVec and borrowed &'a Mutex<ResourceTable>
// ============================================================================

#[cfg(not(feature = "std"))]
mod no_std_impl {
    use wrt_foundation::collections::StaticVec as BoundedVec;

    use super::*;

    /// Maximum number of resources that can be managed by a ResourceArena
    pub const MAX_ARENA_RESOURCES: usize = 64;

    /// A resource arena for managing resource lifecycles as a group
    ///
    /// ResourceArena provides efficient group management of resources in no_std
    /// environments using bounded collections.
    #[derive(Clone)]
    pub struct ResourceArena<'a> {
        /// Handles to resources managed by this arena - using BoundedVec for no_std
        resources: BoundedVec<u32, MAX_ARENA_RESOURCES>,
        /// The resource table used for actual resource management
        table: &'a Mutex<ResourceTable>,
        /// Name of this arena, for debugging
        name: Option<&'a str>,
    }

    impl<'a> ResourceArena<'a> {
        /// Create a new resource arena with the given resource table
        pub fn new(table: &'a Mutex<ResourceTable>) -> Self {
            Self {
                resources: BoundedVec::new(),
                table,
                name: None,
            }
        }

        /// Create a new resource arena with the given name
        pub fn new_with_name(table: &'a Mutex<ResourceTable>, name: &'a str) -> Self {
            Self {
                resources: BoundedVec::new(),
                table,
                name: Some(name),
            }
        }

        /// Create a resource in this arena
        ///
        /// The resource will be automatically cleaned up when the arena is dropped
        /// or when release_all() is called.
        #[cfg(any(feature = "std", feature = "alloc"))]
        pub fn create_resource(
            &mut self,
            type_idx: u32,
            data: Box<dyn Any + Send + Sync>,
        ) -> Result<u32> {
            let mut table = self.table.lock();

            let handle = table.create_resource(type_idx, Arc::from(data))?;
            // Add to arena's resources, checking capacity
            if self.resources.len() >= MAX_ARENA_RESOURCES {
                return Err(Error::runtime_execution_error(
                    "Maximum arena resources exceeded",
                ));
            }
            self.resources
                .push(handle)
                .map_err(|_| Error::resource_error("Failed to add resource to arena"))?;

            Ok(handle)
        }

        /// Create a named resource in this arena
        #[cfg(any(feature = "std", feature = "alloc"))]
        pub fn create_named_resource(
            &mut self,
            type_idx: u32,
            data: Box<dyn Any + Send + Sync>,
            name: &str,
        ) -> Result<u32> {
            let mut table = self.table.lock();

            // Create the resource
            let handle = table.create_resource(type_idx, Arc::from(data))?;

            // Set the name if we have access to the resource
            if let Ok(res) = table.get_resource(handle) {
                let mut res_guard = res.lock();
                res_guard.name = Some(name.to_string());
            }

            // Add to arena's managed resources
            if self.resources.len() >= MAX_ARENA_RESOURCES {
                // Clean up the resource we just created since we can't track it
                let _ = table.drop_resource(handle);
                return Err(Error::runtime_execution_error(
                    "Maximum arena resources exceeded",
                ));
            }
            self.resources.push(handle).map_err(|_| {
                // Clean up the resource we just created since we can't track it
                let _ = table.drop_resource(handle);
                Error::resource_error("Failed to add resource to arena")
            })?;

            Ok(handle)
        }

        /// Get access to a resource
        pub fn get_resource(&self, handle: u32) -> Result<ResourceId> {
            let table = self.table.lock();

            // Verify the resource exists, then return the ResourceId
            let _resource = table.get_resource(handle)?;
            Ok(ResourceId(handle))
        }

        /// Check if a resource exists
        pub fn has_resource(&self, id: ResourceId) -> Result<bool> {
            // First check if it's in our arena
            let contains = self.resources.contains(&id.0);
            if !contains {
                return Ok(false);
            }

            // Then check if it exists in the table
            let table = self.table.lock();

            match table.get_resource(id.0) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }

        /// Remove a resource from this arena's management
        ///
        /// This does not drop the resource, just removes it from the arena.
        /// Returns true if the resource was found and removed.
        pub fn remove_resource(&mut self, handle: u32) -> bool {
            if let Some(pos) = self.resources.iter().position(|&h| h == handle) {
                self.resources.remove(pos);
                true
            } else {
                false
            }
        }

        /// Drop a specific resource and remove it from this arena
        pub fn drop_resource(&mut self, handle: u32) -> Result<()> {
            // First remove it from our tracking
            if !self.remove_resource(handle) {
                return Err(Error::resource_error("Resource not found in arena"));
            }

            // Then drop it from the table
            let mut table = self.table.lock();

            table.drop_resource(handle)
        }

        /// Release all resources managed by this arena
        pub fn release_all(&mut self) -> Result<()> {
            if self.resources.is_empty() {
                return Ok(());
            }

            let mut table = self.table.lock();

            let mut error = None;

            // Try to drop all resources, continuing even if some fail
            for handle in self.resources.iter() {
                if let Err(e) = table.drop_resource(*handle) {
                    // Store the first error but continue trying to drop others
                    if error.is_none() {
                        error = Some(e);
                    }
                }
            }

            // Clear our resources list regardless of errors
            self.resources.clear();

            // Return the first error if any occurred
            if let Some(e) = error { Err(e) } else { Ok(()) }
        }

        /// Get the number of resources in this arena
        pub fn resource_count(&self) -> usize {
            self.resources.len()
        }

        /// Get the name of this arena
        pub fn name(&self) -> Option<&str> {
            self.name
        }

        /// Get all resources managed by this arena
        pub fn resources(&self) -> &[u32] {
            self.resources.as_slice()
        }
    }

    impl<'a> Drop for ResourceArena<'a> {
        fn drop(&mut self) {
            // Try to release all resources, ignoring errors
            let _ = self.release_all();
        }
    }

    impl<'a> Debug for ResourceArena<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ResourceArena")
                .field("name", &self.name)
                .field("resource_count", &self.resources.len())
                .field("resources", &self.resources.as_slice())
                .finish()
        }
    }
}

// ============================================================================
// Re-export the appropriate implementation
// ============================================================================

#[cfg(feature = "std")]
pub use std_impl::*;

#[cfg(not(feature = "std"))]
pub use no_std_impl::*;
