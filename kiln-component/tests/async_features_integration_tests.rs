// WRT - wrt-component
// Integration tests for async Component Model features
// SW-REQ-ID: REQ_ASYNC_INTEGRATION_TESTS_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Comprehensive integration tests for async Component Model features
//!
//! These tests verify the correct implementation and interaction of:
//! - Async context management
//! - Task management built-ins
//! - Waitable set operations
//! - Error context built-ins
//! - Advanced threading
//! - Fixed-length lists

#![cfg(test)]

use wrt_component::*;
use wrt_foundation::{component_value::ComponentValue, types::ValueType};

#[cfg(feature = "std")]
mod async_context_tests {
    use super::*;

    #[test]
    fn test_context_lifecycle() {
        // Test basic context get/set
        let initial = AsyncContextManager::context_get().unwrap();
        assert!(initial.is_none());

        let context = AsyncContext::new();
        AsyncContextManager::context_set(context.clone()).unwrap();

        let retrieved = AsyncContextManager::context_get().unwrap();
        assert!(retrieved.is_some());

        // Clean up
        AsyncContextManager::context_pop().unwrap();
    }

    #[test]
    fn test_context_values() {
        let key = ContextKey::new("test_key".to_string());
        let value = ContextValue::from_component_value(ComponentValue::I32(42));

        AsyncContextManager::set_context_value(key.clone(), value).unwrap();

        let retrieved = AsyncContextManager::get_context_value(&key).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().as_component_value().unwrap(),
            &ComponentValue::I32(42)
        );

        // Clean up
        AsyncContextManager::clear_context().unwrap();
    }

    #[test]
    fn test_context_scope() {
        let original_count =
            AsyncContextManager::context_get().unwrap().map(|c| c.len()).unwrap_or(0);

        {
            let _scope = AsyncContextScope::enter_empty().unwrap();
            let context = AsyncContextManager::context_get().unwrap();
            assert!(context.is_some());
        }

        // Context should be popped after scope
        let final_count = AsyncContextManager::context_get().unwrap().map(|c| c.len()).unwrap_or(0);
        assert_eq!(original_count, final_count);
    }

    #[test]
    fn test_nested_contexts() {
        let _scope1 = AsyncContextScope::enter_empty().unwrap();
        AsyncContextManager::set_context_value(
            ContextKey::new("level1".to_string()),
            ContextValue::from_component_value(ComponentValue::I32(1)),
        )
        .unwrap();

        {
            let _scope2 = AsyncContextScope::enter_empty().unwrap();
            AsyncContextManager::set_context_value(
                ContextKey::new("level2".to_string()),
                ContextValue::from_component_value(ComponentValue::I32(2)),
            )
            .unwrap();

            // Level 2 context should only have level2 key
            let level2_val =
                AsyncContextManager::get_context_value(&ContextKey::new("level2".to_string()))
                    .unwrap();
            assert!(level2_val.is_some());
        }

        // Back to level 1, level2 key should be gone
        let level2_val =
            AsyncContextManager::get_context_value(&ContextKey::new("level2".to_string())).unwrap();
        assert!(level2_val.is_none());
    }
}

#[cfg(feature = "std")]
mod task_management_tests {
    use super::*;

    #[test]
    fn test_task_lifecycle() {
        TaskBuiltins::initialize().unwrap();

        let task_id = TaskBuiltins::task_start().unwrap();

        let status = TaskBuiltins::task_status(task_id).unwrap();
        assert_eq!(status, TaskStatus::Running);

        TaskBuiltins::task_return(
            task_id,
            TaskReturn::from_component_value(ComponentValue::Bool(true)),
        )
        .unwrap();

        let final_status = TaskBuiltins::task_status(task_id).unwrap();
        assert_eq!(final_status, TaskStatus::Completed);

        let result = TaskBuiltins::task_wait(task_id).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_task_cancellation() {
        TaskBuiltins::initialize().unwrap();

        let task_id = TaskBuiltins::task_start().unwrap();
        TaskBuiltins::task_cancel(task_id).unwrap();

        let status = TaskBuiltins::task_status(task_id).unwrap();
        assert_eq!(status, TaskStatus::Cancelled);
    }

    #[test]
    fn test_task_metadata() {
        TaskBuiltins::initialize().unwrap();

        let task_id = TaskBuiltins::task_start().unwrap();

        TaskBuiltins::set_task_metadata(task_id, "priority", ComponentValue::I32(5)).unwrap();

        let metadata = TaskBuiltins::get_task_metadata(task_id, "priority").unwrap();
        assert_eq!(metadata, Some(ComponentValue::I32(5)));
    }

    #[test]
    fn test_multiple_tasks() {
        TaskBuiltins::initialize().unwrap();

        let task1 = TaskBuiltins::task_start().unwrap();
        let task2 = TaskBuiltins::task_start().unwrap();
        let task3 = TaskBuiltins::task_start().unwrap();

        assert_ne!(task1, task2);
        assert_ne!(task2, task3);
        assert_ne!(task1, task3);

        let count = TaskBuiltins::task_count().unwrap();
        assert!(count >= 3);
    }
}

#[cfg(feature = "std")]
mod waitable_set_tests {
    use super::*;
    use crate::async_types::{
        Future, FutureHandle, FutureState, Stream, StreamHandle, StreamState,
    };

    #[test]
    fn test_waitable_set_lifecycle() {
        WaitableSetBuiltins::initialize().unwrap();

        let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();
        assert_eq!(WaitableSetBuiltins::waitable_set_count(set_id).unwrap(), 0);

        let future = Future {
            handle: FutureHandle::new(),
            state: FutureState::Pending,
        };

        let waitable_id =
            WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(future)).unwrap();

        assert!(WaitableSetBuiltins::waitable_set_contains(set_id, waitable_id).unwrap();
        assert_eq!(WaitableSetBuiltins::waitable_set_count(set_id).unwrap(), 1));

        WaitableSetBuiltins::waitable_set_remove(set_id, waitable_id).unwrap();
        assert_eq!(WaitableSetBuiltins::waitable_set_count(set_id).unwrap(), 0);
    }

    #[test]
    fn test_waitable_set_waiting() {
        WaitableSetBuiltins::initialize().unwrap();

        let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();

        // Add pending future
        let pending = Future {
            handle: FutureHandle::new(),
            state: FutureState::Pending,
        };
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(pending)).unwrap();

        // Wait should timeout since nothing is ready
        let result = WaitableSetBuiltins::waitable_set_wait(set_id).unwrap();
        assert!(result.is_timeout();

        // Add resolved future
        let resolved = Future {
            handle: FutureHandle::new(),
            state:  FutureState::Resolved(ComponentValue::I32(42)),
        };
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(resolved)).unwrap();

        // Now wait should find the ready future
        let ready_waitables = WaitableSetBuiltins::waitable_set_poll_all(set_id).unwrap();
        assert_eq!(ready_waitables.len(), 1));
    }

    #[test]
    fn test_mixed_waitables() {
        WaitableSetBuiltins::initialize().unwrap();

        let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();

        // Add different types of waitables
        let future = Future {
            handle: FutureHandle::new(),
            state: FutureState::Pending,
        };
        let stream = Stream {
            handle: StreamHandle::new(),
            state: StreamState::Open,
        };

        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(future)).unwrap();
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Stream(stream)).unwrap();

        assert_eq!(WaitableSetBuiltins::waitable_set_count(set_id).unwrap(), 2);
    }

    #[test]
    fn test_waitable_set_helpers() {
        WaitableSetBuiltins::initialize().unwrap();

        let futures = vec![
            Future {
                handle: FutureHandle::new(),
                state: FutureState::Pending,
            },
            Future {
                handle: FutureHandle::new(),
                state: FutureState::Resolved(ComponentValue::Bool(true)),
            },
        ];

        let set_id = waitable_set_helpers::create_waitable_set_with(
            futures.into_iter().map(Waitable::Future).collect(),
        )
        .unwrap();

        assert_eq!(WaitableSetBuiltins::waitable_set_count(set_id).unwrap(), 2);
    }
}

#[cfg(feature = "std")]
mod error_context_tests {
    use super::*;

    #[test]
    fn test_error_context_lifecycle() {
        ErrorContextBuiltins::initialize().unwrap();

        let context_id =
            ErrorContextBuiltins::error_context_new("Test error".to_string(), ErrorSeverity::Error)
                .unwrap();

        let message = ErrorContextBuiltins::error_context_debug_message(context_id).unwrap();
        assert_eq!(message, "Test error");

        let severity = ErrorContextBuiltins::error_context_severity(context_id).unwrap();
        assert_eq!(severity, ErrorSeverity::Error);

        ErrorContextBuiltins::error_context_drop(context_id).unwrap();
    }

    #[test]
    fn test_error_context_metadata() {
        ErrorContextBuiltins::initialize().unwrap();

        let context_id = ErrorContextBuiltins::error_context_new(
            "Test error".to_string(),
            ErrorSeverity::Warning,
        )
        .unwrap();

        ErrorContextBuiltins::error_context_set_metadata(
            context_id,
            "component".to_string(),
            ComponentValue::String("test_component".to_string()),
        )
        .unwrap();

        let metadata =
            ErrorContextBuiltins::error_context_get_metadata(context_id, "component").unwrap();
        assert_eq!(
            metadata,
            Some(ComponentValue::String("test_component".to_string()))
        );
    }

    #[test]
    fn test_error_context_stack_trace() {
        ErrorContextBuiltins::initialize().unwrap();

        let context_id = ErrorContextBuiltins::error_context_new(
            "Stack trace test".to_string(),
            ErrorSeverity::Error,
        )
        .unwrap();

        ErrorContextBuiltins::error_context_add_stack_frame(
            context_id,
            "test_function".to_string(),
            Some("test.rs".to_string()),
            Some(42),
            Some(10),
        )
        .unwrap();

        let stack_trace = ErrorContextBuiltins::error_context_stack_trace(context_id).unwrap();
        assert!(stack_trace.contains("test_function"));
        assert!(stack_trace.contains("test.rs"));
    }

    #[test]
    fn test_error_severity_conversions() {
        assert_eq!(ErrorSeverity::Info.as_u32(), 0);
        assert_eq!(ErrorSeverity::Warning.as_u32(), 1);
        assert_eq!(ErrorSeverity::Error.as_u32(), 2);
        assert_eq!(ErrorSeverity::Critical.as_u32(), 3);

        assert_eq!(ErrorSeverity::from_u32(0), Some(ErrorSeverity::Info));
        assert_eq!(ErrorSeverity::from_u32(3), Some(ErrorSeverity::Critical));
        assert_eq!(ErrorSeverity::from_u32(999), None);
    }
}

#[cfg(feature = "std")]
mod advanced_threading_tests {
    use super::*;
    use crate::thread_builtins::{
        FunctionSignature, ThreadSpawnConfig, ValueType as ThreadValueType,
    };

    #[test]
    fn test_advanced_thread_lifecycle() {
        AdvancedThreadingBuiltins::initialize().unwrap();

        let func_ref = FunctionReference::new(
            "test_func".to_string(),
            FunctionSignature {
                params: vec![ThreadValueType::I32],
                results: vec![ThreadValueType::I32],
            },
            0,
            42,
        );

        let config = ThreadSpawnConfig {
            stack_size: Some(65536),
            priority: Some(5),
        };

        let thread_id =
            AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, None).unwrap();

        let state = AdvancedThreadingBuiltins::thread_state(thread_id).unwrap();
        assert_eq!(state, AdvancedThreadState::Running);
    }

    #[test]
    fn test_thread_local_storage() {
        AdvancedThreadingBuiltins::initialize().unwrap();

        let func_ref = FunctionReference::new(
            "test_func".to_string(),
            FunctionSignature {
                params: vec![],
                results: vec![],
            },
            0,
            0,
        );

        let config = ThreadSpawnConfig {
            stack_size: Some(65536),
            priority: Some(5),
        };

        let thread_id =
            AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, None).unwrap();

        // Set thread-local values
        AdvancedThreadingBuiltins::thread_local_set(
            thread_id,
            1,
            ComponentValue::String("test_value".to_string()),
            None,
        )
        .unwrap();

        AdvancedThreadingBuiltins::thread_local_set(
            thread_id,
            2,
            ComponentValue::I32(42),
            Some(100), // destructor function index
        )
        .unwrap();

        // Get thread-local values
        let value1 = AdvancedThreadingBuiltins::thread_local_get(thread_id, 1).unwrap();
        assert_eq!(
            value1,
            Some(ComponentValue::String("test_value".to_string()))
        );

        let value2 = AdvancedThreadingBuiltins::thread_local_get(thread_id, 2).unwrap();
        assert_eq!(value2, Some(ComponentValue::I32(42)));
    }

    #[test]
    fn test_indirect_thread_spawn() {
        AdvancedThreadingBuiltins::initialize().unwrap();

        let indirect_call = IndirectCall::new(
            0,  // table_index
            10, // function_index
            1,  // type_index
            vec![ComponentValue::I32(123), ComponentValue::Bool(true)],
        );

        let config = ThreadSpawnConfig {
            stack_size: Some(65536),
            priority: Some(5),
        };

        let thread_id =
            AdvancedThreadingBuiltins::thread_spawn_indirect(indirect_call, config, None).unwrap();

        let state = AdvancedThreadingBuiltins::thread_state(thread_id).unwrap();
        assert_eq!(state, AdvancedThreadState::Running);
    }

    #[test]
    fn test_parent_child_threads() {
        AdvancedThreadingBuiltins::initialize().unwrap();

        let func_ref = FunctionReference::new(
            "parent_func".to_string(),
            FunctionSignature {
                params: vec![],
                results: vec![],
            },
            0,
            0,
        );

        let config = ThreadSpawnConfig {
            stack_size: Some(65536),
            priority: Some(5),
        };

        let parent_id =
            AdvancedThreadingBuiltins::thread_spawn_ref(func_ref.clone(), config.clone(), None)
                .unwrap();

        let child_id =
            AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, Some(parent_id)).unwrap();

        assert_ne!(parent_id, child_id);
    }
}

#[cfg(feature = "std")]
mod fixed_length_list_tests {
    use super::*;

    #[test]
    fn test_fixed_list_creation() {
        let list_type = FixedLengthListType::new(ValueType::I32, 5);
        assert_eq!(list_type.length(), 5);
        assert!(!list_type.is_mutable());
        assert_eq!(list_type.size_in_bytes(), 20); // 5 * 4 bytes

        let list = FixedLengthList::new(list_type).unwrap();
        assert_eq!(list.length(), 5);
        assert_eq!(list.current_length(), 0);
        assert!(!list.is_full());
    }

    #[test]
    fn test_fixed_list_operations() {
        let list_type = FixedLengthListType::new_mutable(ValueType::I32, 3);
        let mut list = FixedLengthList::new(list_type).unwrap();

        // Test push
        list.push(ComponentValue::I32(10)).unwrap();
        list.push(ComponentValue::I32(20)).unwrap();
        list.push(ComponentValue::I32(30)).unwrap();

        assert!(list.is_full());
        assert_eq!(list.current_length(), 3);

        // Test get
        assert_eq!(list.get(0), Some(&ComponentValue::I32(10)));
        assert_eq!(list.get(1), Some(&ComponentValue::I32(20)));
        assert_eq!(list.get(2), Some(&ComponentValue::I32(30)));
        assert_eq!(list.get(3), None);

        // Test set
        list.set(1, ComponentValue::I32(25)).unwrap();
        assert_eq!(list.get(1), Some(&ComponentValue::I32(25)));
    }

    #[test]
    fn test_fixed_list_type_validation() {
        // Test zero length
        let zero_type = FixedLengthListType::new(ValueType::I32, 0);
        assert!(zero_type.validate_size().is_err());

        // Test valid length
        let valid_type = FixedLengthListType::new(ValueType::I32, 100);
        assert!(valid_type.validate_size().is_ok());
    }

    #[test]
    fn test_fixed_list_utilities() {
        // Test zero_filled
        let zeros = fixed_list_utils::zero_filled(ValueType::I32, 3).unwrap();
        assert_eq!(zeros.current_length(), 3);
        assert_eq!(zeros.get(0), Some(&ComponentValue::I32(0)));
        assert_eq!(zeros.get(1), Some(&ComponentValue::I32(0)));
        assert_eq!(zeros.get(2), Some(&ComponentValue::I32(0)));

        // Test from_range
        let range = fixed_list_utils::from_range(5, 8).unwrap();
        assert_eq!(range.current_length(), 3);
        assert_eq!(range.get(0), Some(&ComponentValue::I32(5)));
        assert_eq!(range.get(1), Some(&ComponentValue::I32(6)));
        assert_eq!(range.get(2), Some(&ComponentValue::I32(7)));

        // Test repeat_element
        let repeated =
            fixed_list_utils::repeat_element(ValueType::Bool, ComponentValue::Bool(true), 4)
                .unwrap();
        assert_eq!(repeated.current_length(), 4);
        for i in 0..4 {
            assert_eq!(repeated.get(i), Some(&ComponentValue::Bool(true)));
        }
    }

    #[test]
    fn test_fixed_list_type_registry() {
        let mut registry = FixedLengthListTypeRegistry::new();

        let type1 = FixedLengthListType::new(ValueType::I32, 10);
        let index1 = registry.register_type(type1.clone()).unwrap();
        assert_eq!(index1, 0);

        let type2 = FixedLengthListType::new(ValueType::F64, 5);
        let index2 = registry.register_type(type2).unwrap();
        assert_eq!(index2, 1);

        // Duplicate should return existing index
        let dup_index = registry.register_type(type1).unwrap();
        assert_eq!(dup_index, 0);

        assert_eq!(registry.type_count(), 2);

        // Test retrieval
        let retrieved = registry.get_type(0).unwrap();
        assert_eq!(retrieved.element_type(), &ValueType::I32);
        assert_eq!(retrieved.length(), 10);

        // Test find
        assert_eq!(registry.find_type(&ValueType::I32, 10), Some(0));
        assert_eq!(registry.find_type(&ValueType::F64, 5), Some(1));
        assert_eq!(registry.find_type(&ValueType::Bool, 10), None);
    }

    #[test]
    fn test_list_concatenation() {
        let list1_type = FixedLengthListType::new(ValueType::I32, 2);
        let list1 = FixedLengthList::with_elements(
            list1_type,
            vec![ComponentValue::I32(1), ComponentValue::I32(2)],
        )
        .unwrap();

        let list2_type = FixedLengthListType::new(ValueType::I32, 2);
        let list2 = FixedLengthList::with_elements(
            list2_type,
            vec![ComponentValue::I32(3), ComponentValue::I32(4)],
        )
        .unwrap();

        let concatenated = fixed_list_utils::concatenate(&list1, &list2).unwrap();
        assert_eq!(concatenated.length(), 4);
        assert_eq!(concatenated.get(0), Some(&ComponentValue::I32(1)));
        assert_eq!(concatenated.get(1), Some(&ComponentValue::I32(2)));
        assert_eq!(concatenated.get(2), Some(&ComponentValue::I32(3)));
        assert_eq!(concatenated.get(3), Some(&ComponentValue::I32(4)));
    }

    #[test]
    fn test_list_slicing() {
        let list_type = FixedLengthListType::new(ValueType::I32, 5);
        let list = FixedLengthList::with_elements(
            list_type,
            vec![
                ComponentValue::I32(10),
                ComponentValue::I32(20),
                ComponentValue::I32(30),
                ComponentValue::I32(40),
                ComponentValue::I32(50),
            ],
        )
        .unwrap();

        let sliced = fixed_list_utils::slice(&list, 1, 3).unwrap();
        assert_eq!(sliced.length(), 3);
        assert_eq!(sliced.get(0), Some(&ComponentValue::I32(20)));
        assert_eq!(sliced.get(1), Some(&ComponentValue::I32(30)));
        assert_eq!(sliced.get(2), Some(&ComponentValue::I32(40)));
    }
}

#[cfg(feature = "std")]
mod cross_feature_integration_tests {
    use super::*;

    #[test]
    fn test_async_context_with_tasks() {
        // Initialize systems
        AsyncContextManager::context_pop().ok(); // Clear any existing context
        TaskBuiltins::initialize().unwrap();

        // Set up async context
        let context = AsyncContext::new();
        AsyncContextManager::context_set(context).unwrap();

        // Set context value
        AsyncContextManager::set_context_value(
            ContextKey::new("task_group".to_string()),
            ContextValue::from_component_value(ComponentValue::String(
                "integration_test".to_string(),
            )),
        )
        .unwrap();

        // Create task within context
        let task_id = TaskBuiltins::task_start().unwrap();

        // Verify context is available during task
        let group =
            AsyncContextManager::get_context_value(&ContextKey::new("task_group".to_string()))
                .unwrap();
        assert!(group.is_some());

        // Complete task
        TaskBuiltins::task_return(task_id, TaskReturn::void()).unwrap();

        // Clean up
        AsyncContextManager::context_pop().unwrap();
    }

    #[test]
    fn test_error_context_with_tasks() {
        TaskBuiltins::initialize().unwrap();
        ErrorContextBuiltins::initialize().unwrap();

        // Create a task
        let task_id = TaskBuiltins::task_start().unwrap();

        // Create error context for the task
        let error_id = ErrorContextBuiltins::error_context_new(
            "Task execution error".to_string(),
            ErrorSeverity::Error,
        )
        .unwrap();

        // Add task metadata to error
        ErrorContextBuiltins::error_context_set_metadata(
            error_id,
            "task_id".to_string(),
            ComponentValue::U64(task_id.as_u64()),
        )
        .unwrap();

        // Fail the task
        TaskBuiltins::task_cancel(task_id).unwrap();
        assert_eq!(
            TaskBuiltins::task_status(task_id).unwrap(),
            TaskStatus::Cancelled
        );

        // Verify error context has task info
        let task_id_from_error =
            ErrorContextBuiltins::error_context_get_metadata(error_id, "task_id").unwrap();
        assert_eq!(
            task_id_from_error,
            Some(ComponentValue::U64(task_id.as_u64()))
        );
    }

    #[test]
    fn test_waitable_sets_with_multiple_features() {
        WaitableSetBuiltins::initialize().unwrap();

        let set_id = WaitableSetBuiltins::waitable_set_new().unwrap();

        // Add future
        let future = Future {
            handle: FutureHandle::new(),
            state: FutureState::Resolved(ComponentValue::I32(42)),
        };
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Future(future)).unwrap();

        // Add stream
        let stream = Stream {
            handle: StreamHandle::new(),
            state: StreamState::Open,
        };
        WaitableSetBuiltins::waitable_set_add(set_id, Waitable::Stream(stream)).unwrap();

        // Check for ready items
        let ready = WaitableSetBuiltins::waitable_set_poll_all(set_id).unwrap();
        assert!(ready.len() >= 1); // At least the resolved future should be
        // ready
    }

    #[test]
    fn test_threading_with_fixed_lists() {
        AdvancedThreadingBuiltins::initialize().unwrap();

        // Create a fixed list for thread arguments
        let arg_list_type = FixedLengthListType::new(ValueType::I32, 3);
        let args = FixedLengthList::with_elements(
            arg_list_type,
            vec![
                ComponentValue::I32(10),
                ComponentValue::I32(20),
                ComponentValue::I32(30),
            ],
        )
        .unwrap();

        // Create function reference that takes a list
        let func_ref = FunctionReference::new(
            "list_processor".to_string(),
            FunctionSignature {
                params: vec![ThreadValueType::I32], // Simplified for test
                results: vec![ThreadValueType::I32],
            },
            0,
            100,
        );

        let config = ThreadSpawnConfig {
            stack_size: Some(65536),
            priority: Some(5),
        };

        let thread_id =
            AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, None).unwrap();

        // Store list data in thread-local storage
        let list_value: ComponentValue = args.into();
        AdvancedThreadingBuiltins::thread_local_set(thread_id, 1, list_value, None).unwrap();

        // Verify stored
        let retrieved = AdvancedThreadingBuiltins::thread_local_get(thread_id, 1).unwrap();
        assert!(retrieved.is_some());
    }
}

// Test utilities
#[cfg(feature = "std")]
mod test_helpers {
    use super::*;

    pub fn create_test_future(resolved: bool) -> Future {
        Future {
            handle: FutureHandle::new(),
            state: if resolved {
                FutureState::Resolved(ComponentValue::Bool(true))
            } else {
                FutureState::Pending
            },
        }
    }

    pub fn create_test_stream(open: bool) -> Stream {
        Stream {
            handle: StreamHandle::new(),
            state: if open { StreamState::Open } else { StreamState::Closed },
        }
    }

    pub fn create_test_error_context(message: &str) -> Result<ErrorContextId, wrt_error::Error> {
        ErrorContextBuiltins::error_context_new(message.to_string(), ErrorSeverity::Error)
    }

    pub fn assert_task_status(task_id: TaskBuiltinId, expected: TaskStatus) {
        let actual = TaskBuiltins::task_status(task_id).unwrap();
        assert_eq!(actual, expected, "Task status mismatch");
    }
}

// Performance benchmarks (when benchmarking is enabled)
#[cfg(all(test, feature = "std", feature = "bench"))]
mod benchmarks {
    use test::Bencher;

    use super::*;

    #[bench]
    fn bench_context_get_set(b: &mut Bencher) {
        let key = ContextKey::new("bench_key".to_string());
        let value = ContextValue::from_component_value(ComponentValue::I32(42));

        b.iter(|| {
            AsyncContextManager::set_context_value(key.clone(), value.clone()).unwrap();
            let _ = AsyncContextManager::get_context_value(&key).unwrap();
        });
    }

    #[bench]
    fn bench_task_lifecycle(b: &mut Bencher) {
        TaskBuiltins::initialize().unwrap();

        b.iter(|| {
            let task_id = TaskBuiltins::task_start().unwrap();
            TaskBuiltins::task_return(task_id, TaskReturn::void()).unwrap();
            let _ = TaskBuiltins::task_wait(task_id).unwrap();
        });
    }

    #[bench]
    fn bench_fixed_list_creation(b: &mut Bencher) {
        let list_type = FixedLengthListType::new(ValueType::I32, 100);

        b.iter(|| {
            let _ = FixedLengthList::new(list_type.clone()).unwrap();
        });
    }
}
