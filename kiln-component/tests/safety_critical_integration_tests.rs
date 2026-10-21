//! Safety-Critical Integration Tests
//!
//! This module tests the bounded collections together with the resource
//! management types of the component crate.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_INT_001 - Resource lifecycle integrity
//! - SW-REQ-ID: REQ_INT_002 - Bounded collections used by resource tracking
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

use kiln_component::{
    bounded_component_infra::*,
    resource_management::ResourceTable,
    resources::resource_lifecycle::{ResourceLifecycleManager, ResourceState},
};

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test resource table allocation within its capacity
    #[test]
    fn test_resource_table_allocation_integration() {
        let mut resource_table = ResourceTable::new();

        let mut handles = Vec::new();
        for _ in 0..100 {
            handles.push(resource_table.allocate().expect("allocation within capacity"));
        }

        assert_eq!(handles.len(), 100);
        assert_eq!(resource_table.len(), 100);

        // Handles are unique and valid
        let mut sorted = handles.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), handles.len());
        for handle in &handles {
            assert!(*handle > 0);
        }
    }

    /// Test resource sharing between components
    #[test]
    fn test_resource_sharing_integration() {
        let mut lifecycle_manager = ResourceLifecycleManager::new();

        lifecycle_manager
            .register_type(1, "SharedResource", Some(100))
            .expect("Failed to register resource type");

        // Component 1 creates resource
        let handle = lifecycle_manager
            .create_resource(1, 1, None)
            .expect("Failed to create resource");

        // Component 2 borrows resource
        assert!(lifecycle_manager.borrow_resource(handle, 2, false).is_ok());

        // Verify resource state
        let resource = lifecycle_manager.get_resource(handle).unwrap();
        assert_eq!(resource.state, ResourceState::Borrowed);
        assert_eq!(resource.borrow_count, 1);

        // Multiple shared borrows should be allowed
        assert!(lifecycle_manager.borrow_resource(handle, 3, false).is_ok());
        assert_eq!(
            lifecycle_manager.get_resource(handle).unwrap().borrow_count,
            2
        );

        // Release borrows
        assert!(lifecycle_manager.release_borrow(handle, 2).is_ok());
        assert!(lifecycle_manager.release_borrow(handle, 3).is_ok());

        // Transfer ownership
        assert!(lifecycle_manager.transfer_ownership(handle, 1, 2).is_ok());
        let resource = lifecycle_manager.get_resource(handle).unwrap();
        assert_eq!(resource.metadata.owner, 2);
    }

    /// Test memory allocation across components
    #[test]
    fn test_memory_allocation_integration() {
        struct ComponentMemoryUsage {
            types: BoundedTypeMap<String>,
            resources: BoundedResourceVec<u32>,
        }

        let mut components = Vec::new();

        // Create multiple components with memory allocations
        for _ in 0..5 {
            components.push(Box::new(ComponentMemoryUsage {
                types: new_type_map(),
                resources: new_resource_vec(),
            }));
        }

        // Fill each component's collections partially
        for (idx, comp) in components.iter_mut().enumerate() {
            // Add types
            for j in 0..20 {
                comp.types.insert(j, format!("type_{}_{}", idx, j)).unwrap();
            }

            // Add resources
            for j in 0..30 {
                comp.resources.push(j).unwrap();
            }
        }

        // Verify all components have their data
        for (idx, comp) in components.iter().enumerate() {
            assert_eq!(comp.types.len(), 20);
            assert_eq!(comp.resources.len(), 30);
            assert_eq!(comp.types.get(&7), Some(&format!("type_{}_7", idx)));
        }
    }
}
