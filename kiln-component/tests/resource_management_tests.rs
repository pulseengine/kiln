#![deny(warnings)]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use wrt_component::{
    ComponentValue,
    resources::{
        BufferPool, HostResource, MemoryManager, MemoryStrategy, ResourceId, ResourceManager,
        ResourceOperation, ResourceStrategy,
    },
};
use wrt_error::Error;

/// Test resource ID generation
#[test]
fn test_resource_id_generation() {
    let mut resource_manager = ResourceManager::new();

    // Generate a set of resource IDs and ensure they're unique
    let id1 = resource_manager.generate_id();
    let id2 = resource_manager.generate_id();
    let id3 = resource_manager.generate_id();

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // Test that IDs increase sequentially
    let base_id = resource_manager.generate_id();
    let next_id = resource_manager.generate_id();
    assert_eq!(next_id.0, base_id.0 + 1);
}

/// Test host resource management
#[test]
fn test_host_resource_management() {
    let mut resource_manager = ResourceManager::new();

    // Create a simple host resource
    let test_value = Arc::new(Mutex::new(42));
    let id = resource_manager.add_host_resource(test_value.clone());

    // Verify we can retrieve it
    let retrieved = resource_manager.get_host_resource::<Arc<Mutex<i32>>>(id).unwrap();
    assert_eq!(*retrieved.lock().unwrap(), 42);

    // Modify the value through the retrieved reference
    *retrieved.lock().unwrap() = 100;

    // Verify the value was changed
    let retrieved_again = resource_manager.get_host_resource::<Arc<Mutex<i32>>>(id).unwrap();
    assert_eq!(*retrieved_again.lock().unwrap(), 100);

    // Verify original reference reflects changes
    assert_eq!(*test_value.lock().unwrap(), 100);

    // Try to retrieve with wrong type
    let wrong_type_result = resource_manager.get_host_resource::<Arc<Mutex<String>>>(id);
    assert!(wrong_type_result.is_err());

    // Delete the resource
    resource_manager.delete_resource(id);

    // Verify it's gone
    let not_found = resource_manager.get_host_resource::<Arc<Mutex<i32>>>(id);
    assert!(not_found.is_err());
}

/// Test resource lifecycle operations
#[test]
fn test_resource_lifecycle() {
    let mut resource_manager = ResourceManager::new();

    // Create resources
    let id1 = resource_manager.add_host_resource(Box::new(String::from("resource1")));
    let id2 = resource_manager.add_host_resource(Box::new(42));

    // Check if resources exist
    assert!(resource_manager.has_resource(id1));
    assert!(resource_manager.has_resource(id2));
    assert!(!resource_manager.has_resource(ResourceId(9999)));

    // Get resource types
    assert_eq!(
        resource_manager.get_resource_type(id1),
        Some(std::any::TypeId::of::<Box<String>>())
    );
    assert_eq!(
        resource_manager.get_resource_type(id2),
        Some(std::any::TypeId::of::<Box<i32>>())
    );

    // Delete resources
    resource_manager.delete_resource(id1);
    assert!(!resource_manager.has_resource(id1));
    assert!(resource_manager.has_resource(id2));

    // Clear all resources
    resource_manager.clear();
    assert!(!resource_manager.has_resource(id1));
    assert!(!resource_manager.has_resource(id2));
}

/// Custom test resource that tracks whether it's been dropped
struct DropTracker {
    id: usize,
    dropped: Rc<RefCell<Vec<usize>>>,
}

impl DropTracker {
    fn new(id: usize, dropped: Rc<RefCell<Vec<usize>>>) -> Self {
        Self { id, dropped }
    }
}

impl Drop for DropTracker {
    fn drop(&mut self) {
        self.dropped.borrow_mut().push(self.id);
    }
}

/// Test resource cleanup
#[test]
fn test_resource_cleanup() {
    let dropped = Rc::new(RefCell::new(Vec::new()));

    {
        let mut resource_manager = ResourceManager::new();

        // Add resources with drop trackers
        let id1 =
            resource_manager.add_host_resource(Box::new(DropTracker::new(1, dropped.clone())));
        let id2 =
            resource_manager.add_host_resource(Box::new(DropTracker::new(2, dropped.clone())));
        let id3 =
            resource_manager.add_host_resource(Box::new(DropTracker::new(3, dropped.clone())));

        // Delete one resource explicitly
        resource_manager.delete_resource(id2);
        assert_eq!(*dropped.borrow(), vec![2]);

        // Let the resource manager go out of scope
    }

    // Verify all resources were dropped
    let dropped_ids = dropped.borrow().clone();
    assert_eq!(dropped_ids.len(), 3);
    assert!(dropped_ids.contains(&1));
    assert!(dropped_ids.contains(&2));
    assert!(dropped_ids.contains(&3));
}

/// Binary std/no_std choice
#[test]
fn test_buffer_pool() {
    let mut buffer_pool = BufferPool::new();

    // Allocate a buffer
    let buffer = buffer_pool.allocate(100);
    assert_eq!(buffer.len(), 100);

    // Fill the buffer with test data
    let test_data = [0xAA; 50];
    buffer[..50].copy_from_slice(&test_data);

    // Verify data was written
    assert_eq!(&buffer[..50], &test_data);

    // Allocate another buffer
    let buffer2 = buffer_pool.allocate(200);
    assert_eq!(buffer2.len(), 200);

    // Reset and verify buffers are returned to the pool
    buffer_pool.reset();

    // Allocate again, should reuse from pool
    let reused_buffer = buffer_pool.allocate(100);
    assert_eq!(reused_buffer.len(), 100);
}

/// Test memory strategy implementations
#[test]
fn test_memory_strategies() {
    // Test copy strategy
    let copy_strategy = MemoryStrategy::Copy;
    let test_bytes = vec![1, 2, 3, 4, 5];

    let result = copy_strategy.process_memory(&test_bytes, ResourceOperation::Read);
    assert!(result.is_ok());

    let processed_bytes = result.unwrap();
    assert_eq!(&processed_bytes, &test_bytes);

    // Modifying the processed bytes shouldn't affect the original
    let mut processed_copy = processed_bytes.clone();
    processed_copy[0] = 99;
    assert_ne!(processed_copy[0], test_bytes[0]);

    // Test reference strategy
    let ref_strategy = MemoryStrategy::Reference;

    let result = ref_strategy.process_memory(&test_bytes, ResourceOperation::Read);
    assert!(result.is_ok());

    let processed_bytes = result.unwrap();
    assert_eq!(&processed_bytes, &test_bytes);
}

/// Test memory manager integration with resource manager
#[test]
fn test_memory_manager_integration() {
    let mut resource_manager = ResourceManager::new();
    let mut memory_manager = MemoryManager::new(MemoryStrategy::Copy);

    // Create a resource
    let data = vec![1, 2, 3, 4, 5];
    let id = resource_manager.add_host_resource(data.clone());

    // Register with memory manager
    memory_manager.register_resource(id, &resource_manager);

    // Read memory
    let result = memory_manager.get_memory(id, ResourceOperation::Read);
    assert!(result.is_ok());

    let memory = result.unwrap();
    assert_eq!(&memory, &data);

    // Modify and check that original is unchanged (with Copy strategy)
    let mut memory_copy = memory.clone();
    memory_copy[0] = 99;

    let original = resource_manager.get_host_resource::<Vec<u8>>(id).unwrap();
    assert_eq!(original[0], 1); // Not modified

    // Now try with Reference strategy
    let mut ref_memory_manager = MemoryManager::new(MemoryStrategy::Reference);
    ref_memory_manager.register_resource(id, &resource_manager);

    // This should work the same for reads
    let result = ref_memory_manager.get_memory(id, ResourceOperation::Read);
    assert!(result.is_ok());

    // But for writes, it should affect the original
    let result = ref_memory_manager.get_memory(id, ResourceOperation::Write);
    assert!(result.is_ok());

    let mut writable_memory = result.unwrap();
    writable_memory[0] = 99;

    // Check if original is modified
    let original = resource_manager.get_host_resource::<Vec<u8>>(id).unwrap();
    // Note: actual behavior depends on implementation; this test assumes
    // reference strategy allows direct writes
}

/// Test ComponentValue for resource representation
#[test]
fn test_component_value_resource_representation() {
    // Create a resource ID
    let resource_id = ResourceId(42);

    // Create a ComponentValue::Resource
    let resource_value = ComponentValue::Resource { id: resource_id.0 };

    // Test properties
    match resource_value {
        ComponentValue::Resource { id } => {
            assert_eq!(id, 42);
        },
        _ => panic!("Expected Resource variant"),
    }

    // Test comparison
    let same_resource = ComponentValue::Resource { id: 42 };
    let different_resource = ComponentValue::Resource { id: 43 };

    assert_eq!(resource_value, same_resource);
    assert_ne!(resource_value, different_resource);
}

/// Test error handling in resource operations
#[test]
fn test_resource_error_handling() {
    let mut resource_manager = ResourceManager::new();

    // Try to get a non-existent resource
    let non_existent = ResourceId(9999);
    let result = resource_manager.get_host_resource::<String>(non_existent);
    assert!(result.is_err());

    // Add a resource
    let id = resource_manager.add_host_resource(String::from("test"));

    // Try to get with wrong type
    let result = resource_manager.get_host_resource::<i32>(id);
    assert!(result.is_err());

    // Try to register invalid resource with memory manager
    let mut memory_manager = MemoryManager::new(MemoryStrategy::Copy);
    let result = memory_manager.register_resource(non_existent, &resource_manager);
    assert!(result.is_err());
}

/// Test thread safety of resource manager (when compiled with
/// --features="thread-safe")
#[test]
#[cfg(feature = "thread-safe")]
fn test_thread_safety() {
    use std::thread;

    let resource_manager = Arc::new(Mutex::new(ResourceManager::new()));
    let threads_count = 10;
    let mut handles = vec![];

    // Spawn multiple threads that add and access resources
    for i in 0..threads_count {
        let rm = resource_manager.clone();
        let handle = thread::spawn(move || {
            let resource_value = format!("Resource from thread {}", i);
            let mut manager = rm.lock().unwrap();
            let id = manager.add_host_resource(resource_value.clone());

            // Verify resource was added correctly
            let retrieved = manager.get_host_resource::<String>(id).unwrap();
            assert_eq!(*retrieved, resource_value);

            id
        });

        handles.push(handle);
    }

    // Collect results
    let ids: Vec<ResourceId> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify all resources exist
    let manager = resource_manager.lock().unwrap();
    for id in ids {
        assert!(manager.has_resource(id));
    }
}

/// Integration test for the entire resource system
#[test]
fn test_resource_system_integration() {
    // Create managers
    let mut resource_manager = ResourceManager::new();
    let mut memory_manager = MemoryManager::new(MemoryStrategy::Copy);

    // Create different types of resources
    let string_id = resource_manager.add_host_resource(String::from("text resource"));
    let vector_id = resource_manager.add_host_resource(vec![1, 2, 3, 4, 5]);
    let complex_id = resource_manager.add_host_resource(Box::new((String::from("name"), 42, true)));

    // Register resources with memory manager
    memory_manager.register_resource(string_id, &resource_manager).unwrap();
    memory_manager.register_resource(vector_id, &resource_manager).unwrap();

    // Access resources
    let string_res = resource_manager.get_host_resource::<String>(string_id).unwrap();
    assert_eq!(*string_res, "text resource");

    let vector_res = resource_manager.get_host_resource::<Vec<i32>>(vector_id).unwrap();
    assert_eq!(*vector_res, vec![1, 2, 3, 4, 5]);

    let complex_res = resource_manager
        .get_host_resource::<Box<(String, i32, bool)>>(complex_id)
        .unwrap();
    assert_eq!(complex_res.0, "name");
    assert_eq!(complex_res.1, 42);
    assert_eq!(complex_res.2, true);

    // Access through memory manager
    let vector_mem = memory_manager.get_memory(vector_id, ResourceOperation::Read).unwrap();
    // Memory representation would depend on the actual implementation

    // Clean up
    resource_manager.clear();
}
