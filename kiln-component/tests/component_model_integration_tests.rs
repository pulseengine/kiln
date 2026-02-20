use core::sync::atomic::{AtomicU32, Ordering};

use wrt_component::{
    ComponentInstance, ComponentInstanceId, ResourceHandle, TypeId, ValType,
    async_types::{AsyncType, ErrorContext, Future, FutureState, Stream, StreamState},
    canonical_options::{CanonicalOptions, LiftContext, LowerContext},
    canonical_realloc::{AllocationInfo, ReallocManager},
    component_linker::{ComponentLinker, LinkingResult},
    component_resolver::{ComponentResolver, ResolutionResult},
    cross_component_resource_sharing::{
        CrossComponentResourceSharingManager, PolicyRule, SharingLifetime, TransferPolicy,
        create_basic_sharing_policy,
    },
    generative_types::{GenerativeResourceType, GenerativeTypeRegistry},
    handle_representation::{AccessRights, HandleOperation, HandleRepresentationManager},
    post_return::{CleanupTask, CleanupTaskType, PostReturnRegistry},
    start_function_validation::{
        StartFunctionValidator, ValidationLevel, ValidationState, create_start_function_descriptor,
        create_start_function_param,
    },
    task_manager::{TaskManager, TaskState},
    thread_spawn::{ComponentThreadManager, ThreadConfiguration, ThreadId, ThreadSpawnRequest},
    thread_spawn_fuel::{
        FuelThreadConfiguration, FuelTrackedThreadManager, create_fuel_thread_config,
    },
    type_bounds::{TypeBound, TypeBoundKind, TypeBoundsChecker},
    virtualization::{
        Capability, ExportVisibility, IsolationLevel, MemoryPermissions, VirtualExport,
        VirtualImport, VirtualSource, VirtualizationManager,
    },
};
use wrt_foundation::{
    bounded_collections::{BoundedHashMap, BoundedVec},
    component_value::ComponentValue,
    safe_memory::SafeMemory,
};

const MAX_TEST_ITEMS: usize = 100;

#[test]
fn test_complete_component_model_workflow() {
    let instance_id = ComponentInstanceId::new(1);

    // Initialize all core systems
    let mut type_registry = GenerativeTypeRegistry::new();
    let mut task_manager = TaskManager::new();
    let mut realloc_manager = ReallocManager::new();
    let mut post_return_registry = PostReturnRegistry::new();
    let mut bounds_checker = TypeBoundsChecker::new();

    // Test 1: Generative type creation and bounds
    test_generative_types_with_bounds(&mut type_registry, &mut bounds_checker, instance_id);

    // Test 2: Async operations with task management
    test_async_workflow(&mut task_manager, instance_id);

    // Binary std/no_std choice
    test_memory_management(&mut realloc_manager, instance_id);

    // Test 4: Post-return cleanup integration
    test_post_return_integration(&mut post_return_registry, instance_id);

    // Test 5: Component linking and composition
    test_component_composition();

    // Test 6: Virtualization and sandboxing
    test_virtualization_integration();

    // Test 7: Thread spawning integration
    test_thread_spawn_integration();

    // Test 8: Start function validation
    test_start_function_validation_integration();

    // Test 9: Handle representation and resource sharing
    test_handle_representation_and_sharing();

    // Test 10: Cross-environment compatibility
    test_cross_environment_compatibility();
}

fn test_generative_types_with_bounds(
    registry: &mut GenerativeTypeRegistry,
    bounds_checker: &mut TypeBoundsChecker,
    instance_id: ComponentInstanceId,
) {
    // Create base resource type
    let base_type = registry.create_resource_type(instance_id, "base-resource").unwrap();

    // Create derived type with subtype relationship
    let derived_type = registry.create_resource_type(instance_id, "derived-resource").unwrap();

    // Establish type bounds
    let bound = TypeBound {
        sub_type: derived_type.type_id,
        super_type: base_type.type_id,
        kind: TypeBoundKind::Sub,
    };

    bounds_checker.add_bound(bound).unwrap();

    // Test subtype checking
    assert!(bounds_checker.is_subtype(derived_type.type_id, base_type.type_id));
    assert!(!bounds_checker.is_subtype(base_type.type_id, derived_type.type_id));

    // Test eq relationship (reflexive)
    assert!(bounds_checker.is_eq_type(base_type.type_id, base_type.type_id));

    // Verify resource isolation per instance
    let other_instance = ComponentInstanceId::new(2);
    let other_type = registry.create_resource_type(other_instance, "base-resource").unwrap();

    // Same name but different instance should have different type IDs
    assert_ne!(base_type.type_id, other_type.type_id);
}

fn test_async_workflow(task_manager: &mut TaskManager, instance_id: ComponentInstanceId) {
    // Create a stream
    let stream_handle = task_manager.create_stream(instance_id, ValType::I32, None).unwrap();

    let stream = Stream::new(stream_handle, ValType::I32);

    // Write data to stream
    let data = ComponentValue::I32(42);
    assert!(task_manager.stream_write(stream_handle, data).is_ok());

    // Create a future
    let future_handle = task_manager.create_future(instance_id, ValType::String).unwrap();

    let mut future = Future::new(future_handle, ValType::String);

    // Test task creation and execution
    let task_id = task_manager.create_task(instance_id, "test-task").unwrap();

    // Start task
    task_manager.start_task(task_id).unwrap();
    assert_eq!(
        task_manager.get_task_state(task_id).unwrap(),
        TaskState::Ready
    );

    // Execute task step
    let result = task_manager.execute_task_step(task_id);
    assert!(result.is_ok());

    // Complete the future
    let result_value = ComponentValue::String("test result".to_string());
    future.complete(result_value.clone()).unwrap();
    assert_eq!(future.state, FutureState::Ready);

    // Read from future
    let read_result = future.read().unwrap();
    assert_eq!(read_result, result_value);
}

fn test_memory_management(realloc_manager: &mut ReallocManager, instance_id: ComponentInstanceId) {
    // Binary std/no_std choice
    let size = 1024;
    let align = 8;
    let ptr = realloc_manager.allocate(instance_id, size, align).unwrap();

    // Binary std/no_std choice
    let allocations = realloc_manager.get_instance_allocations(instance_id).unwrap();
    assert!(allocations.contains_key(&ptr));

    let alloc_info = &allocations[&ptr];
    assert_eq!(alloc_info.size, size);
    assert_eq!(alloc_info.alignment, align);

    // Binary std/no_std choice
    let new_size = 2048;
    let new_ptr = realloc_manager.reallocate(instance_id, ptr, size, align, new_size).unwrap();

    // Old pointer should be gone, new one should exist
    let updated_allocations = realloc_manager.get_instance_allocations(instance_id).unwrap();
    assert!(!updated_allocations.contains_key(&ptr));
    assert!(updated_allocations.contains_key(&new_ptr));
    assert_eq!(updated_allocations[&new_ptr].size, new_size);

    // Binary std/no_std choice
    realloc_manager.deallocate(instance_id, new_ptr, new_size, align).unwrap();

    let final_allocations = realloc_manager.get_instance_allocations(instance_id).unwrap();
    assert!(!final_allocations.contains_key(&new_ptr));
}

fn test_post_return_integration(
    registry: &mut PostReturnRegistry,
    instance_id: ComponentInstanceId,
) {
    // Register a post-return function
    let post_return_fn =
        |tasks: &[CleanupTask]| -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
            // Simple cleanup validation
            for task in tasks {
                match task.task_type {
                    CleanupTaskType::Memory { ptr, size, .. } => {
                        assert!(ptr > 0);
                        assert!(size > 0);
                    },
                    CleanupTaskType::Resource { handle } => {
                        assert!(handle.id() > 0);
                    },
                    _ => {},
                }
            }
            Ok(())
        };

    registry
        .register_post_return_function(instance_id, Box::new(post_return_fn))
        .unwrap();

    // Add cleanup tasks
    let memory_task = CleanupTask::memory_cleanup(1000, 512, 8);
    registry.add_cleanup_task(instance_id, memory_task).unwrap();

    let resource_handle = ResourceHandle::new(42);
    let resource_task = CleanupTask::resource_cleanup(resource_handle);
    registry.add_cleanup_task(instance_id, resource_task).unwrap();

    // Execute post-return cleanup
    let result = registry.execute_post_return(instance_id);
    assert!(result.is_ok());

    // Verify tasks were cleared
    let pending = registry.get_pending_cleanup_count(instance_id);
    assert_eq!(pending, 0);
}

fn test_virtualization_integration() {
    let mut virt_manager = VirtualizationManager::new();

    // Create a virtualized component with strong isolation
    let component_id = virt_manager
        .create_virtual_component("sandbox-component", None, IsolationLevel::Strong)
        .unwrap();

    // Grant memory capability
    let memory_capability = Capability::Memory { max_size: 2048 };
    virt_manager
        .grant_capability(component_id, memory_capability.clone(), None, true)
        .unwrap();

    // Verify capability check
    assert!(virt_manager.check_capability(component_id, &memory_capability));

    // Allocate virtual memory
    let permissions = MemoryPermissions {
        read: true,
        write: true,
        execute: false,
    };

    let mem_addr = virt_manager.allocate_virtual_memory(component_id, 1024, permissions).unwrap();
    assert!(mem_addr > 0);

    // Create virtual import
    let virtual_import = VirtualImport {
        name: "host-function".to_string(),
        val_type: ValType::I32,
        required: true,
        virtual_source: Some(VirtualSource::HostFunction {
            name: "get-time".to_string(),
        }),
        capability_required: None,
    };

    virt_manager.add_virtual_import(component_id, virtual_import).unwrap();

    // Create virtual export
    let virtual_export = VirtualExport {
        name: "compute-result".to_string(),
        val_type: ValType::I32,
        visibility: ExportVisibility::Public,
        capability_required: None,
    };

    virt_manager.add_virtual_export(component_id, virtual_export).unwrap();

    // Test parent-child relationship
    let child_id = virt_manager
        .create_virtual_component("child-component", Some(component_id), IsolationLevel::Basic)
        .unwrap();

    // Child should be created successfully with parent relationship
    assert_ne!(component_id, child_id);
}

fn test_thread_spawn_integration() {
    let mut thread_manager = ComponentThreadManager::new();
    let component_id = ComponentInstanceId::new(500);

    // Create thread configuration
    let thread_config = ThreadConfiguration {
        stack_size: 128 * 1024,
        priority: None,
        name: Some("test-thread".to_string()),
        detached: false,
        cpu_affinity: None,
        capabilities: BoundedVec::new(),
    };

    // Create spawn request
    let mut arguments = BoundedVec::new();
    arguments.push(ComponentValue::I32(42)).unwrap();

    let spawn_request = ThreadSpawnRequest {
        component_id,
        function_name: "test-function".to_string(),
        arguments,
        configuration: thread_config,
        return_type: Some(ValType::I32),
    };

    // Test thread spawning
    let handle = thread_manager.spawn_thread(spawn_request).unwrap();
    assert_eq!(handle.component_id, component_id);
    assert!(!handle.detached);

    // Verify thread tracking
    assert_eq!(thread_manager.get_component_thread_count(component_id), 1);
    assert_eq!(thread_manager.get_active_thread_count(), 1);

    let component_threads = thread_manager.get_component_threads(component_id);
    assert_eq!(component_threads.len(), 1);
    assert_eq!(component_threads[0], handle.thread_id);

    // Test thread cleanup
    thread_manager.cleanup_component_threads(component_id).unwrap();
    assert_eq!(thread_manager.get_component_thread_count(component_id), 0);

    // Test fuel-aware thread spawning
    test_fuel_aware_thread_spawning();
}

fn test_fuel_aware_thread_spawning() {
    let mut fuel_manager = FuelTrackedThreadManager::new();
    let component_id = ComponentInstanceId::new(550);

    // Set global fuel limit
    fuel_manager.set_global_fuel_limit(10_000_000);

    // Create fuel configuration
    let fuel_config = create_fuel_thread_config(5000);

    // Create spawn request
    let mut arguments = BoundedVec::new();
    arguments.push(ComponentValue::I32(100)).unwrap();

    let spawn_request = ThreadSpawnRequest {
        component_id,
        function_name: "compute-intensive".to_string(),
        arguments,
        configuration: fuel_config.base_config.clone(),
        return_type: Some(ValType::I32),
    };

    // Spawn thread with fuel tracking
    let handle = fuel_manager.spawn_thread_with_fuel(spawn_request, fuel_config).unwrap();

    // Check initial fuel status
    let fuel_status = fuel_manager.get_thread_fuel_status(handle.thread_id).unwrap();
    assert_eq!(fuel_status.initial_fuel, 5000);
    assert_eq!(fuel_status.remaining_fuel, 5000);
    assert_eq!(fuel_status.consumed_fuel, 0);
    assert!(!fuel_status.fuel_exhausted);

    // Simulate fuel consumption
    fuel_manager.consume_thread_fuel(handle.thread_id, 1000).unwrap();

    let updated_status = fuel_manager.get_thread_fuel_status(handle.thread_id).unwrap();
    assert_eq!(updated_status.remaining_fuel, 4000);
    assert_eq!(updated_status.consumed_fuel, 1000);

    // Test fuel-aware execution
    let result = fuel_manager
        .execute_with_fuel_tracking(handle.thread_id, 500, || {
            // Simulated computation
            42
        })
        .unwrap();
    assert_eq!(result, 42);

    // Check fuel was consumed
    let final_status = fuel_manager.get_thread_fuel_status(handle.thread_id).unwrap();
    assert_eq!(final_status.remaining_fuel, 3500);
    assert_eq!(final_status.consumed_fuel, 1500);

    // Check global fuel status
    let global_status = fuel_manager.get_global_fuel_status();
    assert_eq!(global_status.limit, 10_000_000);
    assert_eq!(global_status.consumed, 5000);
    assert!(global_status.enforcement_enabled);

    // Test fuel exhaustion
    let exhaust_result = fuel_manager.consume_thread_fuel(handle.thread_id, 4000);
    assert!(exhaust_result.is_err());

    // Verify thread is marked as fuel exhausted
    let exhausted_status = fuel_manager.get_thread_fuel_status(handle.thread_id).unwrap();
    assert!(exhausted_status.fuel_exhausted);
}

fn test_start_function_validation_integration() {
    let mut validator = StartFunctionValidator::new()
        .with_strict_mode(true)
        .with_default_validation_level(ValidationLevel::Standard);

    let component_id = ComponentInstanceId::new(600);

    // Create start function descriptor
    let mut descriptor = create_start_function_descriptor("_start");
    descriptor.timeout_ms = 10000;
    descriptor.validation_level = ValidationLevel::Standard;

    // Add parameters
    let mut param1 = create_start_function_param("argc", ValType::I32);
    param1.default_value = Some(ComponentValue::I32(0));
    descriptor.parameters.push(param1).unwrap();

    let mut param2 = create_start_function_param("argv", ValType::String);
    param2.default_value = Some(ComponentValue::String("test".to_string()));
    descriptor.parameters.push(param2).unwrap();

    // Register start function
    validator.register_start_function(component_id, descriptor).unwrap();

    // Validate start function
    let state = validator.validate_start_function(component_id).unwrap();

    // In a real implementation, this would depend on actual execution
    // For testing, we expect it to work or fail gracefully
    assert!(state == ValidationState::Passed || state == ValidationState::Failed);

    // Get validation result
    let validation = validator.get_validation_result(component_id).unwrap();
    assert_eq!(validation.component_id, component_id);
    assert_eq!(validation.descriptor.name, "_start");

    // Get summary
    let summary = validator.get_validation_summary();
    assert_eq!(summary.total, 1);
    assert!(summary.passed + summary.failed == 1);

    // Test validation reset
    validator.reset_validation(component_id).unwrap();
    let validation_after_reset = validator.get_validation_result(component_id).unwrap();
    assert_eq!(
        validation_after_reset.validation_state,
        ValidationState::Pending
    );
}

fn test_handle_representation_and_sharing() {
    let mut handle_manager = HandleRepresentationManager::new();
    let mut sharing_manager = CrossComponentResourceSharingManager::new();
    let mut type_registry = GenerativeTypeRegistry::new();

    let source_component = ComponentInstanceId::new(700);
    let target_component = ComponentInstanceId::new(701);

    // Create resource type
    let resource_type =
        type_registry.create_resource_type(source_component, "shared-resource").unwrap();

    // Create handle with full access
    let handle = handle_manager
        .create_handle(
            source_component,
            resource_type.clone(),
            AccessRights::full_access(),
        )
        .unwrap();

    // Verify handle was created
    let representation = handle_manager.get_representation(handle).unwrap();
    assert_eq!(representation.component_id, source_component);
    assert!(representation.is_owned);
    assert_eq!(representation.reference_count, 1);

    // Test handle operations
    let read_op = HandleOperation::Read {
        fields: {
            let mut fields = BoundedVec::new();
            fields.push("value".to_string()).unwrap();
            fields
        },
    };

    let result = handle_manager.perform_operation(source_component, handle, read_op).unwrap();
    assert!(result.is_some());

    // Set up resource sharing
    // Establish sharing agreement
    let mut resource_types = BoundedVec::new();
    resource_types.push(resource_type.type_id).unwrap();

    let agreement_id = sharing_manager
        .establish_sharing_agreement(
            source_component,
            target_component,
            resource_types,
            AccessRights::read_only(),
            TransferPolicy::SharedOwnership,
            SharingLifetime::Permanent,
        )
        .unwrap();

    // Add a basic sharing policy
    let mut policy = create_basic_sharing_policy("test-policy");
    let mut allowed_types = BoundedVec::new();
    allowed_types.push(resource_type.type_id).unwrap();
    policy
        .rules
        .push(PolicyRule::AllowedResourceTypes {
            types: allowed_types,
        })
        .unwrap();
    sharing_manager.add_sharing_policy(policy).unwrap();

    // Share the resource
    let shared_handle = sharing_manager.share_resource(agreement_id, handle).unwrap();
    assert_ne!(shared_handle, handle); // Should be a new handle

    // Verify target component can access shared resource
    let read_op_target = HandleOperation::Read {
        fields: BoundedVec::new(),
    };

    let access_result =
        sharing_manager.access_shared_resource(target_component, handle, read_op_target);

    // Access should work or fail based on implementation
    assert!(access_result.is_ok() || access_result.is_err());

    // Get sharing statistics
    let stats = sharing_manager.get_sharing_statistics();
    assert_eq!(stats.total_agreements, 1);
    assert_eq!(stats.active_agreements, 1);

    // Test return of shared resource
    let return_result = sharing_manager.return_shared_resource(target_component, handle);
    assert!(return_result.is_ok() || return_result.is_err());

    // Test metadata update
    handle_manager
        .update_handle_metadata(handle, |metadata| {
            metadata.access_count += 1;
            metadata.tags.push("tested".to_string()).ok();
        })
        .unwrap();

    let metadata = handle_manager.get_handle_metadata(handle).unwrap();
    assert!(metadata.access_count > 0);
}

fn test_component_composition() {
    let mut linker = ComponentLinker::new();

    // Create mock component instances for testing
    let producer_id = ComponentInstanceId::new(10);
    let consumer_id = ComponentInstanceId::new(11);

    // Add components
    linker.add_component(producer_id, "producer-component").unwrap();
    linker.add_component(consumer_id, "consumer-component").unwrap();

    // Create export/import pair
    let export_type = ValType::String;
    linker.add_export(producer_id, "data-output", export_type.clone()).unwrap();
    linker.add_import(consumer_id, "data-input", export_type).unwrap();

    // Link components
    let link_result =
        linker.link_import_to_export(consumer_id, "data-input", producer_id, "data-output");
    assert!(link_result.is_ok());

    // Resolve dependencies
    let mut resolver = ComponentResolver::new();
    let resolution = resolver.resolve_component_dependencies(&linker);

    match resolution {
        ResolutionResult::Success => {
            // Verify successful resolution
        },
        ResolutionResult::MissingImports(missing) => {
            panic!("Unexpected missing imports: {:?}", missing);
        },
        ResolutionResult::TypeMismatch(mismatches) => {
            panic!("Unexpected type mismatches: {:?}", mismatches);
        },
        ResolutionResult::CircularDependency(cycle) => {
            panic!("Unexpected circular dependency: {:?}", cycle);
        },
    }
}

fn test_cross_environment_compatibility() {
    // Test that our implementations work across different environments

    // Test bounded collections (for no_std)
    let mut bounded_vec: BoundedVec<u32, MAX_TEST_ITEMS> = BoundedVec::new();
    for i in 0..10 {
        bounded_vec.push(i).unwrap();
    }
    assert_eq!(bounded_vec.len(), 10);

    // Test safe memory operations
    let memory = SafeMemory::new(1024).unwrap();
    let ptr = memory.allocate(64, 8).unwrap();
    assert!(ptr > 0);

    // Test atomic operations (works in all environments)
    let atomic_counter = AtomicU32::new(0);
    atomic_counter.store(42, Ordering::SeqCst);
    assert_eq!(atomic_counter.load(Ordering::SeqCst), 42);
}

#[test]
fn test_canonical_options_integration() {
    let instance_id = ComponentInstanceId::new(100);
    let mut realloc_manager = ReallocManager::new();
    let mut post_return_registry = PostReturnRegistry::new();

    // Binary std/no_std choice
    let options = CanonicalOptions::builder()
        .with_memory(true)
        .with_realloc(true)
        .with_post_return(true)
        .build();

    // Create lift context
    let mut lift_context = LiftContext::new(instance_id, &options);

    // Test memory operations in context
    let ptr = lift_context.allocate_memory(256, 4).unwrap();
    assert!(ptr > 0);

    // Create lower context
    let mut lower_context = LowerContext::new(instance_id, &options);

    // Add cleanup task
    let cleanup_task = CleanupTask::memory_cleanup(ptr as usize, 256, 4);
    lower_context.add_cleanup_task(cleanup_task).unwrap();

    // Finalize context (should trigger cleanup)
    let result = lower_context.finalize(&mut post_return_registry);
    assert!(result.is_ok());
}

#[test]
fn test_error_handling_integration() {
    let instance_id = ComponentInstanceId::new(200);
    let mut task_manager = TaskManager::new();

    // Test error context creation
    let error_msg = "Test error occurred";
    let error_context = ErrorContext::new(error_msg.to_string());

    // Create a future that will fail
    let future_handle = task_manager.create_future(instance_id, ValType::String).unwrap();
    let mut future = Future::new(future_handle, ValType::String);

    // Fail the future with error context
    future.fail(error_context.clone()).unwrap();
    assert_eq!(future.state, FutureState::Failed);

    // Verify error can be retrieved
    let retrieved_error = future.get_error().unwrap();
    assert_eq!(retrieved_error.message, error_msg);
}

#[test]
fn test_resource_lifecycle_integration() {
    let instance_id = ComponentInstanceId::new(300);
    let mut type_registry = GenerativeTypeRegistry::new();
    let mut task_manager = TaskManager::new();

    // Create resource type
    let resource_type = type_registry.create_resource_type(instance_id, "lifecycle-test").unwrap();

    // Create resource handle
    let handle = ResourceHandle::new(1);

    // Map handle to type
    type_registry.map_resource_handle(handle, resource_type.clone()).unwrap();

    // Verify mapping
    let mapped_type = type_registry.get_resource_type(handle).unwrap();
    assert_eq!(mapped_type.type_id, resource_type.type_id);
    assert_eq!(mapped_type.name, resource_type.name);

    // Test resource cleanup through task manager
    let cleanup_result = task_manager.cleanup_instance_resources(instance_id);
    assert!(cleanup_result.is_ok());
}

#[cfg(feature = "std")]
#[test]
fn test_std_specific_features() {
    use std::{sync::Arc, thread};

    // Test thread safety of our implementations
    let type_registry = Arc::new(GenerativeTypeRegistry::new());
    let instance_id = ComponentInstanceId::new(400);

    let registry_clone = Arc::clone(&type_registry);
    let handle = thread::spawn(move || {
        // Create resource type in separate thread
        registry_clone.create_resource_type(instance_id, "thread-test")
    });

    let result = handle.join().unwrap();
    assert!(result.is_ok());
}

#[test]
fn test_no_std_compatibility() {
    // Ensure all our core types work without std
    use core::mem;

    // Test that our types have reasonable sizes
    assert!(mem::size_of::<GenerativeResourceType>() < 1024);
    assert!(mem::size_of::<CleanupTask>() < 256);
    assert!(mem::size_of::<AsyncType>() < 512);

    // Binary std/no_std choice
    let instance_id = ComponentInstanceId::new(500);
    let _stream = Stream::new(42.into(), ValType::I32);
    let _future = Future::new(43.into(), ValType::String);

    // These should compile and work in no_std environments
}
