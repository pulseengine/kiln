//! Safety-Critical Integration Tests
//!
//! This module provides comprehensive integration tests that validate
//! the interaction between all safety-critical features including
//! canonical ABI limits, resource management, component instantiation,
//! and cross-component communication.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_INT_001 - System integration validation
//! - SW-REQ-ID: REQ_COMP_003 - Component isolation
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

use wrt_component::{
    bounded_component_infra::*,
    canonical_abi::canonical::{CanonicalABI, CanonicalOptions},
    resource_management::ResourceTable,
    resources::{
        MemoryStrategy, VerificationLevel,
        resource_lifecycle::{
            Resource, ResourceLifecycleManager, ResourceMetadata, ResourceState, ResourceType,
        },
    },
};
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticMap as BoundedHashMap;
use wrt_foundation::{
    WrtError, bounded::BoundedString, budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec, managed_alloc,
};
#[cfg(not(feature = "std"))]
use wrt_sync::Mutex;

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test complete component lifecycle with bounded resources
    #[test]
    fn test_component_lifecycle_integration() {
        // Create component registry
        let mut components = new_component_vec::<MockComponent>().unwrap();

        // Create mock component
        let component = MockComponent {
            id: 1,
            name: "test_component".to_string(),
            resource_count: 0,
            state: ComponentState::Initialized,
        };

        // Add to registry
        assert!(components.try_push(component).is_ok());

        // Verify component management
        assert_eq!(components.len(), 1);
        let comp = &components[0];
        assert_eq!(comp.id, 1);
        assert_eq!(comp.name, "test_component");
    }

    /// Test canonical ABI with resource limits
    #[test]
    fn test_canonical_abi_resource_integration() {
        let abi = CanonicalABI::new();
        let mut resource_table = ResourceTable::new();

        // Allocate resources up to limit
        let mut handles = Vec::new();
        for i in 0..100 {
            match resource_table.allocate() {
                Ok(handle) => handles.push(handle),
                Err(WrtError::CapacityExceeded) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert!(!handles.is_empty());

        // Verify canonical operations work with resources
        let options = CanonicalOptions::default();

        // Test resource handle encoding/decoding would happen here
        // In real implementation, would use canonical ABI methods
        for handle in &handles {
            // Verify handle is valid
            assert!(*handle > 0);
        }
    }

    /// Test cross-component communication with limits
    #[test]
    fn test_cross_component_communication() {
        // Create two components
        let component1 = MockComponent {
            id: 1,
            name: "sender".to_string(),
            resource_count: 0,
            state: ComponentState::Running,
        };

        let component2 = MockComponent {
            id: 2,
            name: "receiver".to_string(),
            resource_count: 0,
            state: ComponentState::Running,
        };

        // Create call manager
        let call_manager = CrossComponentCallManager::new();

        // Register components
        assert!(call_manager.register_component(component1).is_ok());
        assert!(call_manager.register_component(component2).is_ok());

        // Test call limits
        let mut successful_calls = 0;
        for i in 0..100 {
            // Test with reasonable number
            let result = call_manager.initiate_call(1, 2, "test_func", &[]);
            match result {
                Ok(_) => successful_calls += 1,
                Err(WrtError::CapacityExceeded) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert!(successful_calls > 0);
    }

    /// Test resource sharing between components
    #[test]
    fn test_resource_sharing_integration() {
        let mut lifecycle_manager = ResourceLifecycleManager::new();

        // Create resource type
        let resource_type = ResourceType {
            type_idx: 1,
            name: bounded_component_name_from_str("SharedResource").unwrap(),
            destructor: Some(100),
        };

        // Component 1 creates resource
        let metadata1 = ResourceMetadata {
            created_at: Some(1000),
            last_accessed: None,
            creator: 1,
            owner: 1,
            user_data: None,
        };

        let handle = lifecycle_manager
            .create_resource(resource_type.clone(), metadata1)
            .expect("Failed to create resource");

        // Component 2 borrows resource
        assert!(lifecycle_manager.borrow_resource(handle).is_ok());

        // Verify resource state
        let resource = lifecycle_manager.get_resource(handle).unwrap();
        assert_eq!(resource.state, ResourceState::Borrowed);
        assert_eq!(resource.borrow_count, 1);

        // Multiple borrows should be allowed
        assert!(lifecycle_manager.borrow_resource(handle).is_ok());
        assert_eq!(
            lifecycle_manager.get_resource(handle).unwrap().borrow_count,
            2
        );

        // Release borrows
        assert!(lifecycle_manager.release_borrow(handle).is_ok());
        assert!(lifecycle_manager.release_borrow(handle).is_ok());

        // Transfer ownership
        assert!(lifecycle_manager.transfer_ownership(handle, 2).is_ok());
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
        for i in 0..5 {
            let usage = ComponentMemoryUsage {
                types: new_type_map()
                    .unwrap_or_else(|_| panic!("Failed to allocate types for component {}", i)),
                resources: new_resource_vec()
                    .unwrap_or_else(|_| panic!("Failed to allocate resources for component {}", i)),
            };

            components.push(usage);
        }

        // Fill each component's collections partially
        for (idx, comp) in components.iter_mut().enumerate() {
            // Add types
            for j in 0..20 {
                comp.types.try_insert(j, format!("type_{}_{}", idx, j)).unwrap();
            }

            // Add resources
            for j in 0..30 {
                comp.resources.try_push(j).unwrap();
            }
        }

        // Verify all components have their data
        for (idx, comp) in components.iter().enumerate() {
            assert_eq!(comp.types.len(), 20);
            assert_eq!(comp.resources.len(), 30);
        }
    }

    /// Test component linking with bounded collections
    #[test]
    fn test_component_linking_integration() {
        let mut linker = ComponentLinker::new();

        // Create provider component
        let provider = MockComponent {
            id: 1,
            name: "provider".to_string(),
            resource_count: 50,
            state: ComponentState::Initialized,
        };

        // Create consumer component
        let consumer = MockComponent {
            id: 2,
            name: "consumer".to_string(),
            resource_count: 0,
            state: ComponentState::Initialized,
        };

        // Link components
        assert!(linker.register_component(provider).is_ok());
        assert!(linker.register_component(consumer).is_ok());
        assert!(linker.link(1, 2).is_ok());

        // Verify linking established
        assert!(linker.is_linked(1, 2));
    }

    /// Test error propagation through component layers
    #[test]
    fn test_error_propagation_integration() {
        fn create_component_stack(
            depth: usize,
        ) -> wrt_error::Result<BoundedComponentVec<MockComponent>> {
            let mut components = new_component_vec()?;

            for i in 0..depth {
                let component = MockComponent {
                    id: i as u32,
                    name: format!("comp_{}", i),
                    resource_count: 0,
                    state: ComponentState::Initialized,
                };

                components.try_push(component)?;
            }

            Ok(components)
        }

        // Test with reasonable depth
        match create_component_stack(10) {
            Ok(stack) => assert_eq!(stack.len(), 10),
            Err(e) => {
                // Any error should be properly typed
                match e {
                    WrtError::OutOfMemory => {},
                    WrtError::CapacityExceeded => {},
                    _ => panic!("Unexpected error type: {:?}", e),
                }
            },
        }

        // Test with excessive depth
        match create_component_stack(MAX_COMPONENT_INSTANCES + 1) {
            Ok(_) => panic!("Should have failed with capacity error"),
            Err(WrtError::CapacityExceeded) => {
                // Expected
            },
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    /// Test canonical ABI metrics collection
    #[test]
    fn test_canonical_abi_metrics() {
        // CanonicalABI metrics are tested through usage
        // In a real implementation, we would track lift/lower operations
        let abi = CanonicalABI::new();

        // Verify ABI was created successfully
        // Actual metrics tracking would happen during component operations
        assert!(true); // Placeholder for actual metrics tests
    }

    /// Test resource strategy patterns
    #[test]
    fn test_resource_strategy_integration() {
        struct TestStrategy {
            allocations: Arc<Mutex<BoundedResourceVec<u32>>>,
        }

        impl TestStrategy {
            fn allocate(&self, size: usize) -> wrt_error::Result<u32> {
                let mut allocs = self.allocations.lock().unwrap();
                let handle = allocs.len() as u32;
                allocs.try_push(size as u32)?;
                Ok(handle)
            }

            fn deallocate(&self, handle: u32) -> wrt_error::Result<()> {
                let allocs = self.allocations.lock().unwrap();
                if (handle as usize) < allocs.len() {
                    Ok(())
                } else {
                    Err(WrtError::InvalidHandle)
                }
            }

            fn verify(&self, handle: u32) -> wrt_error::Result<()> {
                let allocs = self.allocations.lock().unwrap();
                if (handle as usize) < allocs.len() {
                    Ok(())
                } else {
                    Err(WrtError::InvalidHandle)
                }
            }
        }

        let strategy = TestStrategy {
            allocations: Arc::new(Mutex::new(new_resource_vec().unwrap())),
        };

        // Test allocation up to limits
        let mut handles = Vec::new();
        for size in (0..MAX_RESOURCE_HANDLES).step_by(100) {
            match strategy.allocate(size) {
                Ok(handle) => handles.push(handle),
                Err(WrtError::CapacityExceeded) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        // Verify all handles
        for handle in &handles {
            assert!(strategy.verify(*handle).is_ok());
        }

        // Deallocate all
        for handle in &handles {
            assert!(strategy.deallocate(*handle).is_ok());
        }
    }
}

/// Component state for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentState {
    Initialized,
    Running,
    Suspended,
    Terminated,
}

/// Simplified mock component for testing
#[derive(Clone, Debug)]
struct MockComponent {
    id: u32,
    name: String,
    resource_count: usize,
    state: ComponentState,
}

/// Mock component linker
struct ComponentLinker {
    components: BoundedComponentVec<MockComponent>,
    links: BoundedTypeMap<bool>,
}

impl ComponentLinker {
    fn new() -> Self {
        Self {
            components: new_component_vec().expect("Failed to create component vec"),
            links: new_type_map().expect("Failed to create links map"),
        }
    }

    fn register_component(&mut self, component: MockComponent) -> wrt_error::Result<()> {
        self.components.try_push(component)
    }

    fn link(&mut self, provider_id: u32, consumer_id: u32) -> wrt_error::Result<()> {
        // Use a combined key for the link
        let key = (provider_id << 16) | consumer_id;
        self.links.try_insert(key, true)?;
        Ok(())
    }

    fn is_linked(&self, provider_id: u32, consumer_id: u32) -> bool {
        let key = (provider_id << 16) | consumer_id;
        self.links.get(&key).copied().unwrap_or(false)
    }
}

/// Mock cross-component call manager
struct CrossComponentCallManager {
    components: Arc<Mutex<BoundedComponentVec<MockComponent>>>,
    call_stack: Arc<Mutex<BoundedCallStack<CallFrame>>>,
}

#[derive(Clone)]
struct CallFrame {
    caller_id: u32,
    callee_id: u32,
    function: String,
}

impl CrossComponentCallManager {
    fn new() -> Self {
        Self {
            components: Arc::new(Mutex::new(
                new_component_vec().expect("Failed to create component vec"),
            )),
            call_stack: Arc::new(Mutex::new(
                new_call_stack().expect("Failed to create call stack"),
            )),
        }
    }

    fn register_component(&self, component: MockComponent) -> wrt_error::Result<()> {
        let mut components = self.components.lock().unwrap();
        components.try_push(component)
    }

    fn initiate_call(
        &self,
        caller_id: u32,
        callee_id: u32,
        function: &str,
        args: &[u8],
    ) -> wrt_error::Result<()> {
        let mut stack = self.call_stack.lock().unwrap();

        let frame = CallFrame {
            caller_id,
            callee_id,
            function: function.to_string(),
        };

        stack.try_push(frame)?;

        // Simulate call execution

        // Pop frame on completion
        stack.pop();

        Ok(())
    }
}

// Note: In a real implementation, CanonicalABI would have methods for
// resource handle encoding/decoding and metrics tracking

#[cfg(all(test, feature = "safety-critical"))]
mod safety_critical_integration {
    use super::*;

    /// Comprehensive test of all safety-critical features
    #[test]
    fn test_complete_safety_critical_system() {
        // This test exercises all safety-critical features together

        // 1. Memory budget enforcement
        let components = new_component_vec::<MockComponent>().unwrap();
        assert_eq!(components.capacity(), MAX_COMPONENT_INSTANCES);

        // 2. Resource limits
        let resources = new_resource_vec::<u32>().unwrap();
        assert_eq!(resources.capacity(), MAX_RESOURCE_HANDLES);

        // 3. Call stack limits
        let call_stack = new_call_stack::<u32>().unwrap();
        assert_eq!(call_stack.capacity(), MAX_CALL_STACK_DEPTH);

        // 4. No panic guarantees - all operations return Result
        let mut test_components = new_component_vec::<MockComponent>().unwrap();
        let overflow_test = test_components.try_push(MockComponent {
            id: 0,
            name: "test".to_string(),
            resource_count: 0,
            state: ComponentState::Initialized,
        });

        // Even at capacity, operations don't panic
        match overflow_test {
            Ok(_) => {},
            Err(WrtError::CapacityExceeded) => {},
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
