// WRT - wrt-component
// Module: Task Management Built-ins
// SW-REQ-ID: REQ_TASK_BUILTINS_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Task Management Built-ins
//!
//! This module provides implementation of the `task.*` built-in functions
//! required by the WebAssembly Component Model for managing async tasks.

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
// Simplified AtomicRefCell for this implementation
use core::cell::RefCell as AtomicRefCell;
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result};
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
use wrt_foundation::{BoundedMap, types::ValueType};
#[cfg(not(any(feature = "std",)))]
use wrt_foundation::{
    BoundedString, BoundedVec, budget_aware_provider::CrateId, safe_managed_alloc,
    safe_memory::NoStdProvider,
};

use crate::threading::task_cancellation::{CancellationToken, with_cancellation_scope};

// Constants for no_std environments
#[cfg(not(any(feature = "std",)))]
const MAX_TASKS: usize = 64;
#[cfg(not(any(feature = "std",)))]
const MAX_TASK_RESULT_SIZE: usize = 512;

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new() -> Self {
        static TASK_COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        Self(TASK_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst))
    }

    /// Create a task identifier from a specific value
    pub const fn from_u64(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Task execution status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task was cancelled
    Cancelled,
    /// Task failed with an error
    Failed,
}

impl TaskStatus {
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
}

/// Task return value
#[derive(Debug, Clone)]
pub enum TaskReturn {
    /// Task returned a component value
    Value(ComponentValue),
    /// Task returned binary data
    #[cfg(feature = "std")]
    Binary(Vec<u8>),
    #[cfg(not(any(feature = "std",)))]
    Binary(BoundedVec<u8, MAX_TASK_RESULT_SIZE>),
    /// Task returned nothing (void)
    Void,
}

impl TaskReturn {
    pub fn from_component_value(value: ComponentValue) -> Self {
        Self::Value(value)
    }

    #[cfg(feature = "std")]
    pub fn from_binary(data: Vec<u8>) -> Self {
        Self::Binary(data)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        let bounded_data = BoundedVec::new_from_slice(data)
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        Ok(Self::Binary(bounded_data))
    }

    pub fn void() -> Self {
        Self::Void
    }

    pub fn as_component_value(&self) -> Option<&ComponentValue> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            #[cfg(feature = "std")]
            Self::Binary(data) => Some(data),
            #[cfg(not(any(feature = "std",)))]
            Self::Binary(data) => Some(data.as_slice()),
            _ => None,
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }
}

/// Task execution context and metadata
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub return_value: Option<TaskReturn>,
    pub cancellation_token: CancellationToken,
    #[cfg(feature = "std")]
    pub metadata: HashMap<String, ComponentValue>,
    #[cfg(not(any(feature = "std",)))]
    pub metadata: BoundedMap<BoundedString<32>, ComponentValue, 8>,
}

impl Task {
    pub fn new() -> Self {
        Self {
            id: TaskId::new(),
            status: TaskStatus::Pending,
            return_value: None,
            cancellation_token: CancellationToken::new(),
            #[cfg(feature = "std")]
            metadata: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            metadata: BoundedMap::new(),
        }
    }

    pub fn with_cancellation_token(token: CancellationToken) -> Self {
        Self {
            id: TaskId::new(),
            status: TaskStatus::Pending,
            return_value: None,
            cancellation_token: token,
            #[cfg(feature = "std")]
            metadata: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            metadata: BoundedMap::new(),
        }
    }

    pub fn start(&mut self) {
        if self.status == TaskStatus::Pending {
            self.status = TaskStatus::Running;
        }
    }

    pub fn complete(&mut self, return_value: TaskReturn) {
        if self.status == TaskStatus::Running {
            self.status = TaskStatus::Completed;
            self.return_value = Some(return_value);
        }
    }

    pub fn cancel(&mut self) {
        if self.status.is_active() {
            self.status = TaskStatus::Cancelled;
            self.cancellation_token.cancel();
        }
    }

    pub fn fail(&mut self) {
        if self.status.is_active() {
            self.status = TaskStatus::Failed;
        }
    }

    #[cfg(feature = "std")]
    pub fn set_metadata(&mut self, key: String, value: ComponentValue) {
        self.metadata.insert(key, value);
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn set_metadata(&mut self, key: &str, value: ComponentValue) -> Result<()> {
        let bounded_key = BoundedString::try_from_str(key)
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        self.metadata.insert(bounded_key, value).map_err(|_| {
            Error::new(
                ErrorCategory::Memory,
                wrt_error::codes::MEMORY_ALLOCATION_FAILED,
                "Error message needed",
            )
        })?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn get_metadata(&self, key: &str) -> Option<&ComponentValue> {
        self.metadata.get(key)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn get_metadata(&self, key: &str) -> Option<&ComponentValue> {
        if let Ok(bounded_key) = BoundedString::try_from_str(key) {
            self.metadata.get(&bounded_key)
        } else {
            None
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

impl Default for Task {
    fn default() -> Self {
        Self::new()
    }
}

/// Global task registry
static TASK_REGISTRY: AtomicRefCell<Option<TaskRegistry>> = AtomicRefCell::new(None);

/// Task registry that manages all active tasks
#[derive(Debug)]
pub struct TaskRegistry {
    #[cfg(feature = "std")]
    tasks: HashMap<TaskId, Task>,
    #[cfg(not(any(feature = "std",)))]
    tasks: BoundedMap<TaskId, Task, MAX_TASKS>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            tasks: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedMap::new(),
        }
    }

    pub fn register_task(&mut self, task: Task) -> Result<TaskId> {
        let id = task.id;
        #[cfg(feature = "std")]
        {
            self.tasks.insert(id, task);
            Ok(id)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.tasks
                .insert(id, task)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            Ok(id)
        }
    }

    pub fn get_task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn get_task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    pub fn remove_task(&mut self, id: TaskId) -> Option<Task> {
        self.tasks.remove(&id)
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn cleanup_finished_tasks(&mut self) {
        #[cfg(feature = "std")]
        {
            self.tasks.retain(|_, task| !task.status.is_finished);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // For no_std, we need to collect keys first
            let mut finished_keys = BoundedVec::<TaskId, MAX_TASKS>::new();
            for (id, task) in self.tasks.iter() {
                if task.status.is_finished() {
                    let _ = finished_keys.push(*id);
                }
            }
            for id in finished_keys.iter() {
                self.tasks.remove(id);
            }
        }
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Task manager providing canonical built-in functions
pub struct TaskBuiltins;

impl TaskBuiltins {
    /// Initialize the global task registry
    pub fn initialize() -> Result<()> {
        let mut registry_ref = TASK_REGISTRY
            .try_borrow_mut()
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        *registry_ref = Some(TaskRegistry::new());
        Ok(())
    }

    /// Get the global task registry
    fn with_registry<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&TaskRegistry) -> R,
    {
        let registry_ref = TASK_REGISTRY.try_borrow().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        let registry = registry_ref
            .as_ref()
            .ok_or_else(|| Error::runtime_execution_error("Error occurred"))?;
        Ok(f(registry))
    }

    /// Get the global task registry mutably
    fn with_registry_mut<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut TaskRegistry) -> Result<R>,
    {
        let mut registry_ref = TASK_REGISTRY.try_borrow_mut().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        let registry = registry_ref
            .as_mut()
            .ok_or_else(|| Error::runtime_execution_error("Error occurred"))?;
        f(registry)
    }

    /// `task.start` canonical built-in
    /// Creates and starts a new task
    pub fn task_start() -> Result<TaskId> {
        let task = Task::new();
        Self::with_registry_mut(|registry| {
            let id = registry.register_task(task)?;
            // Start the task immediately
            if let Some(task) = registry.get_task_mut(id) {
                task.start();
            }
            Ok(id)
        })?
    }

    /// `task.return` canonical built-in
    /// Returns a value from the current task
    pub fn task_return(task_id: TaskId, return_value: TaskReturn) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(task) = registry.get_task_mut(task_id) {
                task.complete(return_value);
                Ok(())
            } else {
                Err(Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::RESOURCE_INVALID_HANDLE,
                    "Error message needed",
                ))
            }
        })
    }

    /// `task.status` canonical built-in
    /// Gets the status of a task
    pub fn task_status(task_id: TaskId) -> Result<TaskStatus> {
        Self::with_registry(|registry| {
            if let Some(task) = registry.get_task(task_id) {
                task.status.clone()
            } else {
                TaskStatus::Failed
            }
        })
    }

    /// `task.cancel` canonical built-in
    /// Cancels a task
    pub fn task_cancel(task_id: TaskId) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(task) = registry.get_task_mut(task_id) {
                task.cancel();
                Ok(())
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// `task.wait` canonical built-in
    /// Waits for a task to complete and returns its result
    pub fn task_wait(task_id: TaskId) -> Result<Option<TaskReturn>> {
        // In a real implementation, this would block until the task completes
        // For now, we just check if it's already completed
        Self::with_registry(|registry| {
            if let Some(task) = registry.get_task(task_id) {
                if task.status.is_finished() { task.return_value.clone() } else { None }
            } else {
                None
            }
        })
    }

    /// Get task metadata
    pub fn get_task_metadata(task_id: TaskId, key: &str) -> Result<Option<ComponentValue>> {
        Self::with_registry(|registry| {
            if let Some(task) = registry.get_task(task_id) {
                task.get_metadata(key).cloned()
            } else {
                None
            }
        })
    }

    /// Set task metadata
    pub fn set_task_metadata(task_id: TaskId, key: &str, value: ComponentValue) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(task) = registry.get_task_mut(task_id) {
                #[cfg(feature = "std")]
                {
                    task.set_metadata(key.to_string(), value);
                    Ok(())
                }
                #[cfg(not(any(feature = "std",)))]
                {
                    task.set_metadata(key, value)
                }
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// Cleanup finished tasks
    pub fn cleanup_finished_tasks() -> Result<()> {
        Self::with_registry_mut(|registry| {
            registry.cleanup_finished_tasks();
            Ok(())
        })?
    }

    /// Get task count
    pub fn task_count() -> Result<usize> {
        Self::with_registry(|registry| registry.task_count())
    }
}

/// Convenience functions for working with tasks
pub mod task_helpers {
    use super::*;

    /// Execute a function within a task context
    pub fn with_task<F, R>(f: F) -> Result<TaskId>
    where
        F: FnOnce() -> Result<R>,
        R: Into<TaskReturn>,
    {
        let task_id = TaskBuiltins::task_start()?;

        match f() {
            Ok(result) => {
                TaskBuiltins::task_return(task_id, result.into())?;
            },
            Err(_) => {
                TaskBuiltins::task_cancel(task_id)?;
            },
        }

        Ok(task_id)
    }

    /// Execute a function with cancellation support
    pub fn with_cancellable_task<F, R>(f: F) -> Result<TaskId>
    where
        F: FnOnce(CancellationToken) -> Result<R>,
        R: Into<TaskReturn>,
    {
        let token = CancellationToken::new();
        let task_id = TaskBuiltins::task_start()?;

        // Execute within cancellation scope
        let result = with_cancellation_scope(token.clone(), || f(token.clone()));

        match result {
            Ok(Ok(value)) => {
                TaskBuiltins::task_return(task_id, value.into())?;
            },
            _ => {
                TaskBuiltins::task_cancel(task_id)?;
            },
        }

        Ok(task_id)
    }

    /// Wait for multiple tasks to complete
    #[cfg(feature = "std")]
    pub fn wait_for_tasks(task_ids: Vec<TaskId>) -> Result<Vec<Option<TaskReturn>>> {
        let mut results = Vec::new();
        for task_id in task_ids {
            let result = TaskBuiltins::task_wait(task_id)?;
            results.push(result);
        }
        Ok(results)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn wait_for_tasks(
        task_ids: &[TaskId],
    ) -> Result<BoundedVec<Option<TaskReturn, 256>, MAX_TASKS>> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut results =
            BoundedVec::new().map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        for &task_id in task_ids {
            let result = TaskBuiltins::task_wait(task_id)?;
            results
                .push(result)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        }
        Ok(results)
    }
}

/// Conversion implementations for TaskReturn
impl From<ComponentValue> for TaskReturn {
    fn from(value: ComponentValue) -> Self {
        Self::Value(value)
    }
}

impl From<()> for TaskReturn {
    fn from(_: ()) -> Self {
        Self::Void
    }
}

impl From<bool> for TaskReturn {
    fn from(value: bool) -> Self {
        Self::Value(ComponentValue::Bool(value))
    }
}

impl From<i32> for TaskReturn {
    fn from(value: i32) -> Self {
        Self::Value(ComponentValue::I32(value))
    }
}

impl From<i64> for TaskReturn {
    fn from(value: i64) -> Self {
        Self::Value(ComponentValue::I64(value))
    }
}

impl From<f32> for TaskReturn {
    fn from(value: f32) -> Self {
        Self::Value(ComponentValue::F32(value))
    }
}

impl From<f64> for TaskReturn {
    fn from(value: f64) -> Self {
        Self::Value(ComponentValue::F64(value))
    }
}
