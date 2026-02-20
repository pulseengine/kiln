//! Async Built-in Functions for WebAssembly Component Model MVP
//!
//! This module implements the missing async built-in functions required
//! by the latest Component Model MVP specification, including task and
//! subtask cancellation operations.

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::fmt;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(any(feature = "std", feature = "alloc")))]
type String = wrt_foundation::bounded::BoundedString<256>;

use wrt_error::{Error, ErrorCategory, Result};
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
use wrt_foundation::{
    MemoryProvider,
    bounded::BoundedString,
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    values::Value,
    verification::Checksum,
};

#[cfg(not(feature = "std"))]
use crate::types::Value as ComponentValue;
use crate::{
    async_::async_types::{
        ErrorContext, FutureHandle, FutureState, StreamHandle, StreamState, Waitable, WaitableSet,
    },
    prelude::*,
    types::ValType,
};

/// Task handle for task cancellation operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TaskHandle(pub u32);

impl Checksummable for TaskHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for TaskHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for TaskHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Subtask handle for subtask cancellation operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SubtaskHandle(pub u32);

impl Checksummable for SubtaskHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for SubtaskHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for SubtaskHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Task cancellation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelResult {
    /// Task was successfully cancelled
    Cancelled,
    /// Task was already completed
    AlreadyCompleted,
    /// Task was already cancelled
    AlreadyCancelled,
    /// Task not found
    NotFound,
    /// Cancellation failed due to error
    Failed(String),
}

impl fmt::Display for CancelResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancelResult::Cancelled => write!(f, "cancelled"),
            CancelResult::AlreadyCompleted => write!(f, "already-completed"),
            CancelResult::AlreadyCancelled => write!(f, "already-cancelled"),
            CancelResult::NotFound => write!(f, "not-found"),
            CancelResult::Failed(msg) => {
                #[cfg(any(feature = "std", feature = "alloc"))]
                {
                    write!(f, "failed: {}", msg)
                }
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                {
                    // BoundedString::as_str() returns Result<&str> in no_std
                    match msg.as_str() {
                        Ok(s) => write!(f, "failed: {}", s),
                        Err(_) => write!(f, "failed: <invalid string>"),
                    }
                }
            },
        }
    }
}

/// Global task registry for tracking active tasks
pub struct TaskRegistry {
    #[cfg(feature = "std")]
    tasks: std::collections::HashMap<TaskHandle, TaskInfo>,
    #[cfg(not(feature = "std"))]
    tasks: BoundedVec<(TaskHandle, TaskInfo), 1024>,

    #[cfg(feature = "std")]
    subtasks: std::collections::HashMap<SubtaskHandle, SubtaskInfo>,
    #[cfg(not(feature = "std"))]
    subtasks: BoundedVec<(SubtaskHandle, SubtaskInfo), 1024>,

    next_task_id: u32,
    next_subtask_id: u32,
}

/// Information about a tracked task
#[derive(Debug, Clone, PartialEq)]
pub struct TaskInfo {
    pub handle: TaskHandle,
    pub state: TaskState,
    pub future_handle: Option<FutureHandle>,
    pub stream_handle: Option<StreamHandle>,
    pub parent_task: Option<TaskHandle>,
    #[cfg(feature = "std")]
    pub subtasks: std::vec::Vec<SubtaskHandle>,
    #[cfg(not(feature = "std"))]
    pub subtasks: BoundedVec<SubtaskHandle, 64>,
    /// Task-local context storage
    #[cfg(feature = "std")]
    pub context: std::collections::HashMap<String, ComponentValue<ComponentProvider>>,
    #[cfg(not(feature = "std"))]
    pub context: BoundedVec<(BoundedString<64>, ComponentValue), 64>,
}

impl Eq for TaskInfo {}

impl Default for TaskInfo {
    fn default() -> Self {
        Self {
            handle: TaskHandle::default(),
            state: TaskState::Running,
            future_handle: None,
            stream_handle: None,
            parent_task: None,
            #[cfg(feature = "std")]
            subtasks: std::vec::Vec::new(),
            #[cfg(not(feature = "std"))]
            subtasks: BoundedVec::new(),
            #[cfg(feature = "std")]
            context: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            context: BoundedVec::new(),
        }
    }
}

impl Checksummable for TaskInfo {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.handle.update_checksum(checksum);
        // Add other fields as needed
    }
}

impl ToBytes for TaskInfo {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)?;
        // Add other fields as needed
        Ok(())
    }
}

impl FromBytes for TaskInfo {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let handle = TaskHandle::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            handle,
            ..Default::default()
        })
    }
}

/// Information about a tracked subtask
#[derive(Debug, Clone, PartialEq)]
pub struct SubtaskInfo {
    pub handle: SubtaskHandle,
    pub state: TaskState,
    pub parent_task: TaskHandle,
    pub future_handle: Option<FutureHandle>,
    pub stream_handle: Option<StreamHandle>,
}

impl Eq for SubtaskInfo {}

impl Default for SubtaskInfo {
    fn default() -> Self {
        Self {
            handle: SubtaskHandle::default(),
            state: TaskState::Running,
            parent_task: TaskHandle::default(),
            future_handle: None,
            stream_handle: None,
        }
    }
}

impl Checksummable for SubtaskInfo {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.handle.update_checksum(checksum);
        self.parent_task.update_checksum(checksum);
    }
}

impl ToBytes for SubtaskInfo {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)?;
        self.parent_task.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for SubtaskInfo {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let handle = SubtaskHandle::from_bytes_with_provider(reader, provider)?;
        let parent_task = TaskHandle::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            handle,
            parent_task,
            ..Default::default()
        })
    }
}

/// Task state for cancellation tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl Default for TaskRegistry {
    fn default() -> Self {
        // Use new() which properly handles allocation or panic in development
        Self::new().expect("TaskRegistry allocation should not fail in default construction")
    }
}

impl TaskRegistry {
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            tasks: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            tasks: { BoundedVec::new() },

            #[cfg(feature = "std")]
            subtasks: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            subtasks: { BoundedVec::new() },

            next_task_id: 1,
            next_subtask_id: 1,
        })
    }

    /// Register a new task
    pub fn register_task(
        &mut self,
        future_handle: Option<FutureHandle>,
        stream_handle: Option<StreamHandle>,
    ) -> Result<TaskHandle> {
        let handle = TaskHandle(self.next_task_id);
        self.next_task_id += 1;

        let task_info = TaskInfo {
            handle,
            state: TaskState::Running,
            future_handle,
            stream_handle,
            parent_task: None,
            #[cfg(feature = "std")]
            subtasks: std::vec::Vec::new(),
            #[cfg(not(feature = "std"))]
            subtasks: { BoundedVec::new() },
            #[cfg(feature = "std")]
            context: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            context: { BoundedVec::new() },
        };

        #[cfg(feature = "std")]
        {
            self.tasks.insert(handle, task_info);
        }
        #[cfg(not(feature = "std"))]
        {
            self.tasks
                .push((handle, task_info))
                .map_err(|_| Error::runtime_execution_error("Failed to register task"))?;
        }

        Ok(handle)
    }

    /// Register a new subtask
    pub fn register_subtask(
        &mut self,
        parent_task: TaskHandle,
        future_handle: Option<FutureHandle>,
        stream_handle: Option<StreamHandle>,
    ) -> Result<SubtaskHandle> {
        let handle = SubtaskHandle(self.next_subtask_id);
        self.next_subtask_id += 1;

        let subtask_info = SubtaskInfo {
            handle,
            state: TaskState::Running,
            parent_task,
            future_handle,
            stream_handle,
        };

        #[cfg(feature = "std")]
        {
            self.subtasks.insert(handle, subtask_info);
            if let Some(parent_info) = self.tasks.get_mut(&parent_task) {
                parent_info.subtasks.push(handle);
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.subtasks
                .push((handle, subtask_info))
                .map_err(|_| Error::runtime_execution_error("Failed to register subtask"))?;

            // Update parent task
            for (task_handle, task_info) in &mut self.tasks {
                if *task_handle == parent_task {
                    task_info.subtasks.push(handle).map_err(|_| {
                        Error::runtime_execution_error("Failed to add subtask to parent")
                    })?;
                    break;
                }
            }
        }

        Ok(handle)
    }

    /// Cancel a task and all its subtasks
    pub fn cancel_task(&mut self, handle: TaskHandle) -> CancelResult {
        #[cfg(feature = "std")]
        {
            if let Some(task_info) = self.tasks.get_mut(&handle) {
                match task_info.state {
                    TaskState::Running => {
                        task_info.state = TaskState::Cancelled;

                        // Cancel all subtasks
                        let subtasks = task_info.subtasks.clone();
                        for subtask_handle in subtasks {
                            self.cancel_subtask(subtask_handle);
                        }

                        CancelResult::Cancelled
                    },
                    TaskState::Completed => CancelResult::AlreadyCompleted,
                    TaskState::Cancelled => CancelResult::AlreadyCancelled,
                    TaskState::Failed => CancelResult::AlreadyCompleted,
                }
            } else {
                CancelResult::NotFound
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for (task_handle, task_info) in &mut self.tasks {
                if *task_handle == handle {
                    match task_info.state {
                        TaskState::Running => {
                            task_info.state = TaskState::Cancelled;

                            // Cancel all subtasks
                            let subtasks = task_info.subtasks.clone();
                            for subtask_handle in subtasks {
                                self.cancel_subtask(subtask_handle);
                            }

                            return CancelResult::Cancelled;
                        },
                        TaskState::Completed => return CancelResult::AlreadyCompleted,
                        TaskState::Cancelled => return CancelResult::AlreadyCancelled,
                        TaskState::Failed => return CancelResult::AlreadyCompleted,
                    }
                }
            }
            CancelResult::NotFound
        }
    }

    /// Cancel a specific subtask
    pub fn cancel_subtask(&mut self, handle: SubtaskHandle) -> CancelResult {
        #[cfg(feature = "std")]
        {
            if let Some(subtask_info) = self.subtasks.get_mut(&handle) {
                match subtask_info.state {
                    TaskState::Running => {
                        subtask_info.state = TaskState::Cancelled;
                        CancelResult::Cancelled
                    },
                    TaskState::Completed => CancelResult::AlreadyCompleted,
                    TaskState::Cancelled => CancelResult::AlreadyCancelled,
                    TaskState::Failed => CancelResult::AlreadyCompleted,
                }
            } else {
                CancelResult::NotFound
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for (subtask_handle, subtask_info) in &mut self.subtasks {
                if *subtask_handle == handle {
                    match subtask_info.state {
                        TaskState::Running => {
                            subtask_info.state = TaskState::Cancelled;
                            return CancelResult::Cancelled;
                        },
                        TaskState::Completed => return CancelResult::AlreadyCompleted,
                        TaskState::Cancelled => return CancelResult::AlreadyCancelled,
                        TaskState::Failed => return CancelResult::AlreadyCompleted,
                    }
                }
            }
            CancelResult::NotFound
        }
    }

    /// Mark a task as completed
    pub fn complete_task(&mut self, handle: TaskHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(task_info) = self.tasks.get_mut(&handle) {
                task_info.state = TaskState::Completed;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for (task_handle, task_info) in &mut self.tasks {
                if *task_handle == handle {
                    task_info.state = TaskState::Completed;
                    break;
                }
            }
        }
        Ok(())
    }

    /// Mark a subtask as completed
    pub fn complete_subtask(&mut self, handle: SubtaskHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(subtask_info) = self.subtasks.get_mut(&handle) {
                subtask_info.state = TaskState::Completed;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for (subtask_handle, subtask_info) in &mut self.subtasks {
                if *subtask_handle == handle {
                    subtask_info.state = TaskState::Completed;
                    break;
                }
            }
        }
        Ok(())
    }

    /// Set a context value for a task
    #[cfg(feature = "std")]
    pub fn set_task_context(
        &mut self,
        handle: TaskHandle,
        key: &str,
        value: ComponentValue<ComponentProvider>,
    ) -> Result<()> {
        if let Some(task_info) = self.tasks.get_mut(&handle) {
            task_info.context.insert(key.to_string(), value);
        } else {
            return Err(Error::runtime_execution_error("Task not found"));
        }
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    pub fn set_task_context(
        &mut self,
        handle: TaskHandle,
        key: &str,
        value: ComponentValue,
    ) -> Result<()> {
        for (task_handle, task_info) in &mut self.tasks {
            if *task_handle == handle {
                let provider = safe_managed_alloc!(512, CrateId::Component)?;
                let key_bounded = BoundedString::try_from_str(key).map_err(|_| {
                    Error::runtime_execution_error(
                        "Failed to create bounded string for task context key",
                    )
                })?;

                // Remove existing entry if present
                let mut index_to_remove = None;
                for (i, (existing_key, _)) in task_info.context.iter().enumerate() {
                    if existing_key.as_str()? == key {
                        index_to_remove = Some(i);
                        break;
                    }
                }
                if let Some(index) = index_to_remove {
                    task_info.context.remove(index);
                }

                // Add new entry
                task_info.context.push((key_bounded, value)).map_err(|_| {
                    Error::new(
                        ErrorCategory::Resource,
                        wrt_error::codes::RESOURCE_EXHAUSTED,
                        "Failed to add task context entry",
                    )
                })?;
                break;
            }
        }
        Ok(())
    }

    /// Get a context value for a task
    #[cfg(feature = "std")]
    pub fn get_task_context(
        &self,
        handle: TaskHandle,
        key: &str,
    ) -> Option<ComponentValue<ComponentProvider>> {
        if let Some(task_info) = self.tasks.get(&handle) {
            task_info.context.get(key).cloned()
        } else {
            None
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn get_task_context(&self, handle: TaskHandle, key: &str) -> Option<ComponentValue> {
        for (task_handle, task_info) in &self.tasks {
            if *task_handle == handle {
                for (existing_key, value) in &task_info.context {
                    if existing_key.as_str().ok() == Some(key) {
                        return Some(value.clone());
                    }
                }
                break;
            }
        }
        None
    }

    /// Get current task handle (simplified implementation)
    pub fn get_current_task(&self) -> Option<TaskHandle> {
        // For this implementation, return the first running task
        // In a real implementation, this would use thread-local storage
        #[cfg(feature = "std")]
        {
            for (handle, task_info) in &self.tasks {
                if task_info.state == TaskState::Running {
                    return Some(*handle);
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for (handle, task_info) in &self.tasks {
                if task_info.state == TaskState::Running {
                    return Some(*handle);
                }
            }
        }
        None
    }
}

/// ASIL-D safe global task registry
#[cfg(feature = "std")]
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "std")]
static GLOBAL_TASK_REGISTRY: OnceLock<Mutex<TaskRegistry>> = OnceLock::new();

/// Get the global task registry (ASIL-D safe)
#[cfg(feature = "std")]
fn get_task_registry() -> Result<&'static Mutex<TaskRegistry>> {
    Ok(GLOBAL_TASK_REGISTRY.get_or_init(|| {
        Mutex::new(TaskRegistry::new().expect("Global task registry allocation should not fail"))
    }))
}

/// Get the global task registry (no_std fallback)
#[cfg(not(feature = "std"))]
fn get_task_registry() -> Result<TaskRegistry> {
    TaskRegistry::new()
}

/// Async built-in function implementations
pub mod builtins {
    use super::*;

    /// `task.cancel` canonical built-in
    /// Cancels a running task and all its subtasks
    #[cfg(feature = "std")]
    pub fn task_cancel(task_handle: u32) -> Result<ComponentValue<ComponentProvider>> {
        let handle = TaskHandle(task_handle);

        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;
        let result = registry.cancel_task(handle);

        match result {
            CancelResult::Cancelled => Ok(ComponentValue::U32(0)), // Success
            CancelResult::AlreadyCompleted => Ok(ComponentValue::U32(1)), // Already completed
            CancelResult::AlreadyCancelled => Ok(ComponentValue::U32(2)), // Already cancelled
            CancelResult::NotFound => Ok(ComponentValue::U32(3)),  // Not found
            CancelResult::Failed(_) => Ok(ComponentValue::U32(4)), // Failed
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn task_cancel(task_handle: u32) -> Result<ComponentValue> {
        let handle = TaskHandle(task_handle);
        let mut registry = get_task_registry()?;
        let result = registry.cancel_task(handle);

        match result {
            CancelResult::Cancelled => Ok(ComponentValue::U32(0)), // Success
            CancelResult::AlreadyCompleted => Ok(ComponentValue::U32(1)), // Already completed
            CancelResult::AlreadyCancelled => Ok(ComponentValue::U32(2)), // Already cancelled
            CancelResult::NotFound => Ok(ComponentValue::U32(3)),  // Not found
            CancelResult::Failed(_) => Ok(ComponentValue::U32(4)), // Failed
        }
    }

    /// `subtask.cancel` canonical built-in
    /// Cancels a specific subtask
    #[cfg(feature = "std")]
    pub fn subtask_cancel(subtask_handle: u32) -> Result<ComponentValue<ComponentProvider>> {
        let handle = SubtaskHandle(subtask_handle);
        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex.lock().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::CONCURRENCY_ERROR,
                "Failed to acquire task registry lock",
            )
        })?;
        let result = registry.cancel_subtask(handle);

        match result {
            CancelResult::Cancelled => Ok(ComponentValue::U32(0)), // Success
            CancelResult::AlreadyCompleted => Ok(ComponentValue::U32(1)), // Already completed
            CancelResult::AlreadyCancelled => Ok(ComponentValue::U32(2)), // Already cancelled
            CancelResult::NotFound => Ok(ComponentValue::U32(3)),  // Not found
            CancelResult::Failed(_) => Ok(ComponentValue::U32(4)), // Failed
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn subtask_cancel(subtask_handle: u32) -> Result<ComponentValue> {
        let handle = SubtaskHandle(subtask_handle);
        let mut registry = get_task_registry()?;
        let result = registry.cancel_subtask(handle);

        match result {
            CancelResult::Cancelled => Ok(ComponentValue::U32(0)), // Success
            CancelResult::AlreadyCompleted => Ok(ComponentValue::U32(1)), // Already completed
            CancelResult::AlreadyCancelled => Ok(ComponentValue::U32(2)), // Already cancelled
            CancelResult::NotFound => Ok(ComponentValue::U32(3)),  // Not found
            CancelResult::Failed(_) => Ok(ComponentValue::U32(4)), // Failed
        }
    }

    /// `task.spawn` canonical built-in (bonus implementation)
    /// Spawns a new async task
    #[cfg(feature = "std")]
    pub fn task_spawn(
        future_handle: Option<u32>,
        stream_handle: Option<u32>,
    ) -> Result<ComponentValue<ComponentProvider>> {
        let future_h = future_handle.map(FutureHandle);
        let stream_h = stream_handle.map(StreamHandle);

        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        match registry.register_task(future_h, stream_h) {
            Ok(handle) => Ok(ComponentValue::U32(handle.0)),
            Err(_) => Err(Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Failed to spawn task",
            )),
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn task_spawn(
        future_handle: Option<u32>,
        stream_handle: Option<u32>,
    ) -> Result<ComponentValue> {
        let future_h = future_handle.map(FutureHandle);
        let stream_h = stream_handle.map(StreamHandle);

        let mut registry = get_task_registry()?;

        match registry.register_task(future_h, stream_h) {
            Ok(handle) => Ok(ComponentValue::U32(handle.0)),
            Err(_) => Err(Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Failed to spawn task",
            )),
        }
    }

    /// `subtask.spawn` canonical built-in (bonus implementation)
    /// Spawns a new subtask under a parent task
    #[cfg(feature = "std")]
    pub fn subtask_spawn(
        parent_task: u32,
        future_handle: Option<u32>,
        stream_handle: Option<u32>,
    ) -> Result<ComponentValue<ComponentProvider>> {
        let parent_h = TaskHandle(parent_task);
        let future_h = future_handle.map(FutureHandle);
        let stream_h = stream_handle.map(StreamHandle);

        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        match registry.register_subtask(parent_h, future_h, stream_h) {
            Ok(handle) => Ok(ComponentValue::U32(handle.0)),
            Err(_) => Err(Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Failed to spawn subtask",
            )),
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn subtask_spawn(
        parent_task: u32,
        future_handle: Option<u32>,
        stream_handle: Option<u32>,
    ) -> Result<ComponentValue> {
        let parent_h = TaskHandle(parent_task);
        let future_h = future_handle.map(FutureHandle);
        let stream_h = stream_handle.map(StreamHandle);

        let mut registry = get_task_registry()?;

        match registry.register_subtask(parent_h, future_h, stream_h) {
            Ok(handle) => Ok(ComponentValue::U32(handle.0)),
            Err(_) => Err(Error::new(
                ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_EXHAUSTED,
                "Failed to spawn subtask",
            )),
        }
    }

    /// `task.status` canonical built-in (bonus implementation)
    /// Gets the status of a task
    #[cfg(feature = "std")]
    pub fn task_status(task_handle: u32) -> Result<ComponentValue<ComponentProvider>> {
        let handle = TaskHandle(task_handle);

        let registry_mutex = get_task_registry()?;
        let registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(task_info) = registry.tasks.get(&handle) {
            let status = match task_info.state {
                TaskState::Running => 0u32,
                TaskState::Completed => 1u32,
                TaskState::Cancelled => 2u32,
                TaskState::Failed => 3u32,
            };
            Ok(ComponentValue::U32(status))
        } else {
            Ok(ComponentValue::U32(4)) // Not found
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn task_status(task_handle: u32) -> Result<ComponentValue> {
        let handle = TaskHandle(task_handle);

        let registry = get_task_registry()?;

        for (task_handle_ref, task_info) in &registry.tasks {
            if *task_handle_ref == handle {
                let status = match task_info.state {
                    TaskState::Running => 0u32,
                    TaskState::Completed => 1u32,
                    TaskState::Cancelled => 2u32,
                    TaskState::Failed => 3u32,
                };
                return Ok(ComponentValue::U32(status));
            }
        }
        Ok(ComponentValue::U32(4)) // Not found
    }

    /// `subtask.status` canonical built-in (bonus implementation)
    /// Gets the status of a subtask
    #[cfg(feature = "std")]
    pub fn subtask_status(subtask_handle: u32) -> Result<ComponentValue<ComponentProvider>> {
        let handle = SubtaskHandle(subtask_handle);

        let registry_mutex = get_task_registry()?;
        let registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(subtask_info) = registry.subtasks.get(&handle) {
            let status = match subtask_info.state {
                TaskState::Running => 0u32,
                TaskState::Completed => 1u32,
                TaskState::Cancelled => 2u32,
                TaskState::Failed => 3u32,
            };
            Ok(ComponentValue::U32(status))
        } else {
            Ok(ComponentValue::U32(4)) // Not found
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn subtask_status(subtask_handle: u32) -> Result<ComponentValue> {
        let handle = SubtaskHandle(subtask_handle);

        let registry = get_task_registry()?;

        for (subtask_handle_ref, subtask_info) in &registry.subtasks {
            if *subtask_handle_ref == handle {
                let status = match subtask_info.state {
                    TaskState::Running => 0u32,
                    TaskState::Completed => 1u32,
                    TaskState::Cancelled => 2u32,
                    TaskState::Failed => 3u32,
                };
                return Ok(ComponentValue::U32(status));
            }
        }
        Ok(ComponentValue::U32(4)) // Not found
    }

    /// `context.get` canonical built-in
    /// Gets a value from the current task's context storage
    #[cfg(feature = "std")]
    pub fn context_get(key: &str) -> Result<ComponentValue<ComponentProvider>> {
        let registry_mutex = get_task_registry()?;
        let registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(current_task) = registry.get_current_task() {
            if let Some(value) = registry.get_task_context(current_task, key) {
                Ok(value)
            } else {
                Ok(ComponentValue::Option(None))
            }
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn context_get(key: &str) -> Result<ComponentValue> {
        let registry = get_task_registry()?;

        if let Some(current_task) = registry.get_current_task() {
            if let Some(value) = registry.get_task_context(current_task, key) {
                Ok(value)
            } else {
                Ok(ComponentValue::Option(None))
            }
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }

    /// `context.set` canonical built-in
    /// Sets a value in the current task's context storage
    #[cfg(feature = "std")]
    pub fn context_set(
        key: &str,
        value: ComponentValue<ComponentProvider>,
    ) -> Result<ComponentValue<ComponentProvider>> {
        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(current_task) = registry.get_current_task() {
            registry.set_task_context(current_task, key, value)?;
            Ok(ComponentValue::U32(0)) // Success
        } else {
            Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_OPERATION,
                "No current task context",
            ))
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn context_set(key: &str, value: ComponentValue) -> Result<ComponentValue> {
        let mut registry = get_task_registry()?;

        if let Some(current_task) = registry.get_current_task() {
            registry.set_task_context(current_task, key, value)?;
            Ok(ComponentValue::U32(0)) // Success
        } else {
            Err(Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_OPERATION,
                "No current task context",
            ))
        }
    }

    /// `context.has` canonical built-in (bonus implementation)
    /// Checks if a key exists in the current task's context
    #[cfg(feature = "std")]
    pub fn context_has(key: &str) -> Result<ComponentValue<ComponentProvider>> {
        let registry_mutex = get_task_registry()?;
        let registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(current_task) = registry.get_current_task() {
            let has_key = registry.get_task_context(current_task, key).is_some();
            Ok(ComponentValue::Bool(has_key))
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn context_has(key: &str) -> Result<ComponentValue> {
        let registry = get_task_registry()?;

        if let Some(current_task) = registry.get_current_task() {
            let has_key = registry.get_task_context(current_task, key).is_some();
            Ok(ComponentValue::Bool(has_key))
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }

    /// `context.clear` canonical built-in (bonus implementation)
    /// Clears all values from the current task's context
    #[cfg(feature = "std")]
    pub fn context_clear() -> Result<ComponentValue<ComponentProvider>> {
        let registry_mutex = get_task_registry()?;
        let mut registry = registry_mutex
            .lock()
            .map_err(|_| Error::runtime_execution_error("Failed to acquire task registry lock"))?;

        if let Some(current_task) = registry.get_current_task() {
            if let Some(task_info) = registry.tasks.get_mut(&current_task) {
                task_info.context.clear();
            }
            Ok(ComponentValue::U32(0)) // Success
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn context_clear() -> Result<ComponentValue> {
        let mut registry = get_task_registry()?;

        if let Some(current_task) = registry.get_current_task() {
            for (task_handle, task_info) in &mut registry.tasks {
                if *task_handle == current_task {
                    task_info.context.clear();
                    break;
                }
            }
            Ok(ComponentValue::U32(0)) // Success
        } else {
            Err(Error::runtime_execution_error("No current task"))
        }
    }
}
