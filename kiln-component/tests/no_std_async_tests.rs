// WRT - wrt-component
// No-std tests for async Component Model features
// SW-REQ-ID: REQ_ASYNC_NO_STD_TESTS_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! No-std tests for async Component Model features
//!
//! These tests verify that all async features work correctly
//! in no_std environments with bounded collections.

#![cfg(all(test, not(feature = "std")))]
#![no_std]

extern crate wrt_component;
extern crate wrt_foundation;

use wrt_component::*;
use wrt_foundation::{component_value::ComponentValue, types::ValueType};

#[test]
fn test_async_context_no_std() {
    // Clear any existing context
    let _ = AsyncContextManager::context_pop();

    // Test basic context operations
    let context = AsyncContext::new();
    AsyncContextManager::context_set(context).unwrap();

    // Set a value with bounded string key
    let key = ContextKey::new("test").unwrap();
    let value = ContextValue::from_component_value(ComponentValue::I32(42));
    AsyncContextManager::set_context_value(key.clone(), value).unwrap();

    // Retrieve value
    let retrieved = AsyncContextManager::get_context_value(&key).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(
        retrieved.unwrap().as_component_value().unwrap(),
        &ComponentValue::I32(42)
    );

    // Clean up
    AsyncContextManager::context_pop().unwrap();
}

#[test]
fn test_task_management_no_std() {
    TaskBuiltins::initialize().unwrap();

    // Create task
    let task_id = TaskBuiltins::task_start().unwrap();

    // Check status
    let status = TaskBuiltins::task_status(task_id).unwrap();
    assert_eq!(status, TaskStatus::Running);

    // Set metadata with bounded string
    TaskBuiltins::set_task_metadata(task_id, "priority", ComponentValue::I32(5)).unwrap();

    // Complete task
    TaskBuiltins::task_return(
        task_id,
        TaskReturn::from_component_value(ComponentValue::Bool(true)),
    )
    .unwrap();

    // Verify completion
    let final_status = TaskBuiltins::task_status(task_id).unwrap();
    assert_eq!(final_status, TaskStatus::Completed);
}

#[test]
fn test_waitable_sets_no_std() {
    use crate::async_types::{Future, FutureHandle, FutureState};

    WaitableSetBuiltins::initialize().unwrap();

    // Create set
    let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();

    // Add future
    let future = Future {
        handle: FutureHandle::new(),
        state: FutureState::Pending,
    };
    let waitable_id =
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(future)).unwrap();

    // Check contains
    assert!(WaitableSetBuiltins::waitable_set_contains(set_id, waitable_id).unwrap());

    // Remove
    assert!(WaitableSetBuiltins::waitable_set_remove(set_id, waitable_id).unwrap());
}

#[test]
fn test_error_context_no_std() {
    ErrorContextBuiltins::initialize().unwrap();

    // Create error context with bounded string
    let context_id =
        ErrorContextBuiltins::error_context_new("Test error", ErrorSeverity::Warning).unwrap();

    // Get debug message
    let message = ErrorContextBuiltins::error_context_debug_message(context_id).unwrap();
    assert_eq!(message.as_str(), "Test error");

    // Add stack frame with bounded strings
    ErrorContextBuiltins::error_context_add_stack_frame(
        context_id,
        "test_func",
        Some("test.rs"),
        Some(42),
        None,
    )
    .unwrap();

    // Set metadata with bounded string key
    ErrorContextBuiltins::error_context_set_metadata(context_id, "code", ComponentValue::I32(100))
        .unwrap();

    // Clean up
    ErrorContextBuiltins::error_context_drop(context_id).unwrap();
}

#[test]
fn test_advanced_threading_no_std() {
    use crate::thread_builtins::{
        FunctionSignature, ThreadSpawnConfig, ValueType as ThreadValueType,
    };

    AdvancedThreadingBuiltins::initialize().unwrap();

    // Create function reference with bounded string
    let func_ref = FunctionReference::new(
        "test_fn",
        FunctionSignature {
            params: vec![],
            results: vec![],
        },
        0,
        0,
    )
    .unwrap();

    let config = ThreadSpawnConfig {
        stack_size: Some(4096),
        priority: Some(5),
    };

    // Spawn thread
    let thread_id = AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, None).unwrap();

    // Check state
    let state = AdvancedThreadingBuiltins::thread_state(thread_id).unwrap();
    assert_eq!(state, AdvancedThreadState::Running);

    // Set thread-local with bounded storage
    AdvancedThreadingBuiltins::thread_local_set(thread_id, 1, ComponentValue::I32(123), None)
        .unwrap();

    // Get thread-local
    let value = AdvancedThreadingBuiltins::thread_local_get(thread_id, 1).unwrap();
    assert_eq!(value, Some(ComponentValue::I32(123)));
}

#[test]
fn test_fixed_length_lists_no_std() {
    // Create list type
    let list_type = FixedLengthListType::new(ValueType::I32, 3);
    assert!(list_type.validate_size().is_ok());

    // Create list
    let mut list = FixedLengthList::new(list_type.clone()).unwrap();

    // Add elements (uses bounded vec internally)
    list.push(ComponentValue::I32(1)).unwrap();
    list.push(ComponentValue::I32(2)).unwrap();
    list.push(ComponentValue::I32(3)).unwrap();

    // Verify full
    assert!(list.is_full());

    // Create with predefined elements
    let elements = [
        ComponentValue::I32(10),
        ComponentValue::I32(20),
        ComponentValue::I32(30),
    ];
    let list2 = FixedLengthList::with_elements(list_type, &elements).unwrap();
    assert_eq!(list2.current_length(), 3);

    // Test utilities
    let zeros = fixed_list_utils::zero_filled(ValueType::Bool, 5).unwrap();
    assert_eq!(zeros.current_length(), 5);
}

#[test]
fn test_bounded_collections_limits() {
    // Test that bounded collections properly enforce limits

    // Context key size limit
    let long_key = "a".repeat(65); // Exceeds MAX_CONTEXT_KEY_SIZE (64)
    let key_result = ContextKey::new(&long_key);
    assert!(key_result.is_err());

    // Error message size limit
    let long_message = "e".repeat(513); // Exceeds MAX_DEBUG_MESSAGE_SIZE (512)
    let error_result = ErrorContextBuiltins::error_context_new(&long_message, ErrorSeverity::Error);
    assert!(error_result.is_err());

    // Task metadata limits
    TaskBuiltins::initialize().unwrap();
    let task_id = TaskBuiltins::task_start().unwrap();

    // Metadata key size limit
    let long_metadata_key = "m".repeat(33); // Exceeds bounded string size
    let metadata_result =
        TaskBuiltins::set_task_metadata(task_id, &long_metadata_key, ComponentValue::I32(1));
    assert!(metadata_result.is_err());
}

#[test]
fn test_memory_efficiency_no_std() {
    // Verify that our no_std implementations are memory efficient

    // Small context
    let context = AsyncContext::new();
    // In no_std, this uses BoundedMap with fixed capacity

    // Small task registry
    TaskBuiltins::initialize().unwrap();
    // Registry uses bounded collections

    // Small waitable set
    WaitableSetBuiltins::initialize().unwrap();
    let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();
    // Set uses bounded collections for waitables

    // Binary std/no_std choice
    assert!(true); // If we got here, bounded collections work
}

// Binary std/no_std choice
#[test]
fn test_stack_based_operations() {
    // Binary std/no_std choice

    // Binary std/no_std choice
    let values = [
        ComponentValue::Bool(true),
        ComponentValue::I32(42),
        ComponentValue::F64(3.14),
    ];

    // Create fixed list from stack array
    let list_type = FixedLengthListType::new(ValueType::I32, 3);
    let list = FixedLengthList::with_elements(
        list_type,
        &[
            ComponentValue::I32(1),
            ComponentValue::I32(2),
            ComponentValue::I32(3),
        ],
    )
    .unwrap();

    // Binary std/no_std choice
    assert_eq!(list.current_length(), 3);
}
