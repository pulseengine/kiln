//! Example demonstrating WebAssembly Component Model async WITHOUT Rust futures
//!
//! The Component Model has its own async primitives that don't require
//! the futures crate or Rust's async/await syntax.

use wrt_component::{
    ComponentInstanceId, ValType,
    async_canonical::AsyncCanonicalAbi,
    async_types::{Future, FutureHandle, FutureState, Stream, StreamHandle, StreamState},
    task_manager::{TaskManager, TaskState},
};
use wrt_foundation::{bounded_collections::BoundedVec, component_value::ComponentValue};

fn main() {
    println!("=== WebAssembly Component Model Async Example ===\n");

    // Initialize the async infrastructure
    let mut task_manager = TaskManager::new();
    let mut async_abi = AsyncCanonicalAbi::new();
    let component_id = ComponentInstanceId::new(1);

    // Example 1: Using Component Model Streams (no Rust futures needed!)
    println!("1. Stream Example:");
    example_stream(&mut task_manager, &mut async_abi, component_id);

    // Example 2: Using Component Model Futures (no Rust futures needed!)
    println!("\n2. Future Example:");
    example_future(&mut task_manager, &mut async_abi, component_id);

    // Example 3: Task-based async execution
    println!("\n3. Task-based Async:");
    example_task_async(&mut task_manager, component_id);
}

fn example_stream(
    task_manager: &mut TaskManager,
    async_abi: &mut AsyncCanonicalAbi,
    component_id: ComponentInstanceId,
) {
    // Create a stream - this is NOT a Rust Stream!
    let stream_handle = async_abi.stream_new(ValType::String).unwrap();
    println!("Created stream with handle: {:?}", stream_handle);

    // Write values to the stream
    let values = vec![
        ComponentValue::String("Hello".to_string()),
        ComponentValue::String("Component".to_string()),
        ComponentValue::String("Model".to_string()),
        ComponentValue::String("Async".to_string()),
    ];

    for value in values {
        match async_abi.stream_write(stream_handle, value.clone()) {
            Ok(_) => println!("  Wrote to stream: {:?}", value),
            Err(e) => println!("  Error writing: {:?}", e),
        }
    }

    // Read from the stream - no async/await needed!
    println!("  Reading from stream:");
    loop {
        match async_abi.stream_read(stream_handle) {
            Ok(Some(value)) => println!("    Read: {:?}", value),
            Ok(None) => {
                println!("    Stream empty (would block)");
                break;
            },
            Err(e) => {
                println!("    Error reading: {:?}", e);
                break;
            },
        }
    }

    // Close the stream
    async_abi.stream_close_writable(stream_handle).unwrap();
    async_abi.stream_close_readable(stream_handle).unwrap();
    println!("  Stream closed");
}

fn example_future(
    task_manager: &mut TaskManager,
    async_abi: &mut AsyncCanonicalAbi,
    component_id: ComponentInstanceId,
) {
    // Create a future - this is NOT a Rust Future!
    let future_handle = async_abi.future_new(ValType::I32).unwrap();
    println!("Created future with handle: {:?}", future_handle);

    // Check if future is ready (it shouldn't be yet)
    match async_abi.future_read(future_handle) {
        Ok(Some(value)) => println!("  Future unexpectedly ready: {:?}", value),
        Ok(None) => println!("  Future pending (as expected)"),
        Err(e) => println!("  Error: {:?}", e),
    }

    // Complete the future with a value
    let result = ComponentValue::I32(42);
    async_abi.future_write(future_handle, result.clone()).unwrap();
    println!("  Completed future with value: {:?}", result);

    // Now read the completed future
    match async_abi.future_read(future_handle) {
        Ok(Some(value)) => println!("  Future ready with value: {:?}", value),
        Ok(None) => println!("  Future still pending (unexpected)"),
        Err(e) => println!("  Error: {:?}", e),
    }

    // Close the future
    async_abi.future_close_writable(future_handle).unwrap();
    async_abi.future_close_readable(future_handle).unwrap();
    println!("  Future closed");
}

fn example_task_async(task_manager: &mut TaskManager, component_id: ComponentInstanceId) {
    // Create a task - Component Model's unit of async execution
    let task_id = task_manager.create_task(component_id, "async-operation").unwrap();
    println!("Created task with ID: {:?}", task_id);

    // Start the task
    task_manager.start_task(task_id).unwrap();
    println!("  Task started");

    // Simulate task execution steps
    for step in 0..3 {
        let result = task_manager.execute_task_step(task_id);
        match result {
            Ok(state) => println!("  Step {}: Task state = {:?}", step, state),
            Err(e) => println!("  Step {} error: {:?}", step, e),
        }

        // Check task state
        if let Ok(state) = task_manager.get_task_state(task_id) {
            if state == TaskState::Completed {
                println!("  Task completed!");
                break;
            }
        }
    }

    // Clean up
    task_manager.cleanup_task(task_id).unwrap();
    println!("  Task cleaned up");
}

// Example showing manual polling without Rust's async runtime
fn manual_async_example() {
    println!("\n=== Manual Async Polling (No Tokio/async-std needed) ===");

    let mut task_manager = TaskManager::new();
    let component_id = ComponentInstanceId::new(1);

    // Create multiple futures
    let mut futures = Vec::new();
    for i in 0..3 {
        let future = Future::<i32>::new(FutureHandle(i), ValType::I32);
        futures.push(future);
    }

    // Poll them manually in a loop
    let mut completed = 0;
    while completed < futures.len() {
        for (i, future) in futures.iter_mut().enumerate() {
            if future.state == FutureState::Pending {
                // Simulate async completion
                if i == 1 {
                    // Complete the second future
                    future.set_value(i as i32 * 10).unwrap();
                    println!("Future {} completed with value {}", i, i * 10);
                    completed += 1;
                }
            }
        }

        // In a real implementation, we would:
        // 1. Check for I/O readiness
        // 2. Process network events
        // 3. Handle timers
        // 4. Wake waiting tasks
        // All without Rust's async runtime!
    }
}
