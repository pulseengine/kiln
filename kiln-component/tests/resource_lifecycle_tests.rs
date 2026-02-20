//! Comprehensive tests for the resource lifecycle implementation

use wrt_component::{
    borrowed_handles::{BorrowHandle, HandleLifetimeTracker, OwnHandle, with_lifetime_scope},
    resource_lifecycle_management::{
        ComponentId, DropHandlerFunction, LifecyclePolicies, ResourceCreateRequest, ResourceId,
        ResourceLifecycleManager, ResourceMetadata, ResourceType,
    },
    resource_representation::{
        FileHandle, MemoryBuffer, NetworkHandle, RepresentationValue,
        ResourceRepresentationManager, canon_resource_drop, canon_resource_new, canon_resource_rep,
    },
    task_cancellation::{CancellationToken, SubtaskManager, with_cancellation_scope},
    task_manager::TaskId,
};

#[test]
fn test_resource_lifecycle_basic() {
    let mut manager = ResourceLifecycleManager::new();

    // Create a resource
    let request = ResourceCreateRequest {
        resource_type: ResourceType::Stream,
        metadata: ResourceMetadata::new("test-stream"),
        owner: ComponentId(1),
        custom_handlers: Vec::new(),
    };

    let resource_id = manager.create_resource(request).unwrap();
    assert_eq!(resource_id, ResourceId(1));

    // Check statistics
    let stats = manager.get_stats();
    assert_eq!(stats.resources_created, 1);
    assert_eq!(stats.active_resources, 1);

    // Add reference
    let ref_count = manager.add_reference(resource_id).unwrap();
    assert_eq!(ref_count, 2);

    // Remove reference
    let ref_count = manager.remove_reference(resource_id).unwrap();
    assert_eq!(ref_count, 1);

    // Drop resource
    let ref_count = manager.remove_reference(resource_id).unwrap();
    assert_eq!(ref_count, 0);

    // Verify statistics
    let stats = manager.get_stats();
    assert_eq!(stats.resources_destroyed, 1);
    assert_eq!(stats.active_resources, 0);
}

#[test]
fn test_handle_lifetime_tracking() {
    let mut tracker = HandleLifetimeTracker::new();

    // Create owned handle
    let owned: OwnHandle<u32> = tracker
        .create_owned_handle(ResourceId(1), ComponentId(1), "test-resource")
        .unwrap();

    // Create scope and borrow handle
    let result = with_lifetime_scope(&mut tracker, ComponentId(1), TaskId(1), |scope| {
        let borrowed = tracker.borrow_handle(&owned, ComponentId(2), scope).unwrap();

        // Validate borrow
        let validation = tracker.validate_borrow(&borrowed);
        assert!(matches!(
            validation,
            wrt_component::borrowed_handles::BorrowValidation::Valid
        ));

        // Return borrowed handle for testing
        Ok(borrowed)
    });

    let borrowed = result.unwrap();

    // After scope ends, borrow should be invalid
    let validation = tracker.validate_borrow(&borrowed);
    assert!(matches!(
        validation,
        wrt_component::borrowed_handles::BorrowValidation::ScopeEnded
    ));

    // Cleanup tracker
    tracker.cleanup().unwrap();
}

#[test]
fn test_resource_representation() {
    let mut manager = ResourceRepresentationManager::with_builtin_representations();

    // Create a file handle resource
    let resource_id = ResourceId(1);
    let owner = ComponentId(1);
    let initial_repr = RepresentationValue::U32(42); // File descriptor

    let handle =
        canon_resource_new::<FileHandle>(&mut manager, resource_id, owner, initial_repr).unwrap();

    // Get representation
    let repr = canon_resource_rep(&mut manager, handle).unwrap();
    assert!(matches!(repr, RepresentationValue::U32(42)));

    // Validate handle
    let is_valid = manager.validate_handle(handle).unwrap();
    assert!(is_valid);

    // Drop resource
    canon_resource_drop(&mut manager, handle).unwrap();

    // Handle should be invalid after drop
    let is_valid = manager.validate_handle(handle).unwrap();
    assert!(!is_valid);
}

#[test]
fn test_cancellation_tokens() {
    // Test basic cancellation
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());

    token.cancel().unwrap();
    assert!(token.is_cancelled());

    // Test child cancellation
    let parent = CancellationToken::new();
    let child = parent.child();

    assert!(!child.is_cancelled());

    parent.cancel().unwrap();
    assert!(parent.is_cancelled());
    assert!(child.is_cancelled());
}

#[test]
fn test_subtask_management() {
    let mut manager = SubtaskManager::new(TaskId(1));
    let parent_token = CancellationToken::new();

    // Spawn subtask
    let subtask_token = manager
        .spawn_subtask(
            wrt_component::async_execution_engine::ExecutionId(1),
            TaskId(2),
            &parent_token,
        )
        .unwrap();

    // Check stats
    let stats = manager.get_stats();
    assert_eq!(stats.created, 1);
    assert_eq!(stats.active, 1);

    // Update subtask state
    use wrt_component::task_cancellation::SubtaskState;
    manager
        .update_subtask_state(
            wrt_component::async_execution_engine::ExecutionId(1),
            SubtaskState::Running,
        )
        .unwrap();

    // Cancel subtask
    manager
        .cancel_subtask(wrt_component::async_execution_engine::ExecutionId(1))
        .unwrap();
    assert!(subtask_token.is_cancelled());

    // Complete subtask
    manager
        .update_subtask_state(
            wrt_component::async_execution_engine::ExecutionId(1),
            SubtaskState::Cancelled,
        )
        .unwrap();

    let stats = manager.get_stats();
    assert_eq!(stats.cancelled, 1);
    assert_eq!(stats.active, 0);
}

#[test]
fn test_garbage_collection() {
    let mut manager = ResourceLifecycleManager::new();

    // Create resources
    for i in 0..3 {
        let request = ResourceCreateRequest {
            resource_type: ResourceType::MemoryBuffer,
            metadata: ResourceMetadata::new(&format!("buffer-{}", i)),
            owner: ComponentId(1),
            custom_handlers: Vec::new(),
        };

        let resource_id = manager.create_resource(request).unwrap();

        // Drop references for first two resources
        if i < 2 {
            manager.remove_reference(resource_id).unwrap();
        }
    }

    // Run garbage collection
    let gc_result = manager.run_garbage_collection(true).unwrap();
    assert_eq!(gc_result.collected_count, 2);
    assert!(gc_result.full_gc);

    // Check stats
    let stats = manager.get_stats();
    assert_eq!(stats.resources_created, 3);
    assert_eq!(stats.resources_destroyed, 2);
    assert_eq!(stats.active_resources, 1);
}

#[test]
fn test_resource_with_drop_handlers() {
    let mut manager = ResourceLifecycleManager::new();

    // Register drop handler
    let handler_id = manager
        .register_drop_handler(
            ResourceType::Stream,
            DropHandlerFunction::StreamCleanup,
            0,
            true,
        )
        .unwrap();

    // Create resource with custom handlers
    let request = ResourceCreateRequest {
        resource_type: ResourceType::Stream,
        metadata: ResourceMetadata::new("stream-with-handler"),
        owner: ComponentId(1),
        custom_handlers: vec![DropHandlerFunction::StreamCleanup],
    };

    let resource_id = manager.create_resource(request).unwrap();

    // Drop resource - handlers should be called
    manager.drop_resource(resource_id).unwrap();

    let stats = manager.get_stats();
    assert!(stats.drop_handlers_executed > 0);
}

#[test]
fn test_lifecycle_policies() {
    let policies = LifecyclePolicies {
        enable_gc: true,
        gc_interval_ms: 5000,
        max_lifetime_ms: Some(60000),
        strict_ref_counting: true,
        leak_detection: true,
        max_memory_bytes: Some(1024 * 1024),
    };

    let mut manager = ResourceLifecycleManager::with_policies(policies);

    // Create a resource
    let request = ResourceCreateRequest {
        resource_type: ResourceType::FileHandle,
        metadata: ResourceMetadata::new("policy-test"),
        owner: ComponentId(1),
        custom_handlers: Vec::new(),
    };

    let resource_id = manager.create_resource(request).unwrap();

    // Check for leaks (should be none)
    let leaks = manager.check_for_leaks().unwrap();
    assert_eq!(leaks.len(), 0);

    // Verify policies are applied
    let current_policies = manager.get_policies();
    assert!(current_policies.enable_gc);
    assert!(current_policies.leak_detection);
}

#[test]
fn test_with_cancellation_scope() {
    let result = with_cancellation_scope(true, |token| {
        assert!(!token.is_cancelled());
        Ok(42)
    })
    .unwrap();

    assert_eq!(result, 42);
}

#[test]
fn test_complex_resource_scenario() {
    // This test simulates a more complex scenario with multiple resources,
    // borrowing, and cleanup coordination

    let mut lifecycle_manager = ResourceLifecycleManager::new();
    let mut handle_tracker = HandleLifetimeTracker::new();
    let mut repr_manager = ResourceRepresentationManager::with_builtin_representations();

    // Create multiple resources
    let resources: Vec<_> = (0..3)
        .map(|i| {
            let request = ResourceCreateRequest {
                resource_type: ResourceType::Custom(i as u32),
                metadata: ResourceMetadata::new(&format!("resource-{}", i)),
                owner: ComponentId(1),
                custom_handlers: Vec::new(),
            };
            lifecycle_manager.create_resource(request).unwrap()
        })
        .collect();

    // Create owned handles for resources
    let owned_handles: Vec<_> = resources
        .iter()
        .enumerate()
        .map(|(i, &resource_id)| {
            handle_tracker
                .create_owned_handle::<u32>(resource_id, ComponentId(1), &format!("handle-{}", i))
                .unwrap()
        })
        .collect();

    // Create a scope and borrow handles
    with_lifetime_scope(&mut handle_tracker, ComponentId(1), TaskId(1), |scope| {
        for owned in &owned_handles {
            let _borrowed = handle_tracker.borrow_handle(owned, ComponentId(2), scope).unwrap();
        }

        // Verify all borrows are valid within scope
        let stats = handle_tracker.get_stats();
        assert_eq!(stats.active_borrowed, 3);

        Ok(())
    })
    .unwrap();

    // After scope, all borrows should be invalidated
    let stats = handle_tracker.get_stats();
    assert_eq!(stats.borrowed_invalidated, 3);
    assert_eq!(stats.active_borrowed, 0);

    // Clean up resources
    for resource_id in resources {
        lifecycle_manager.remove_reference(resource_id).unwrap();
    }

    // Run garbage collection
    let gc_result = lifecycle_manager.run_garbage_collection(true).unwrap();
    assert_eq!(gc_result.collected_count, 3);
}
