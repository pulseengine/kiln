//! Task Manager bridge for async Component Model integration
//!
//! This module provides the bridge between the Component Model's TaskManager
//! and the fuel-based async executor, enabling seamless async task lifecycle
//! management.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Weak;
use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    task::{Context, Poll},
};
#[cfg(feature = "std")]
use std::sync::Weak;

use wrt_foundation::{
    Arc, CrateId, Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    component_value::ComponentValue,
    safe_managed_alloc,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::{
    CallFrame, Task, TaskContext, TaskId, TaskManager, TaskState, TaskType,
};
#[cfg(feature = "component-model-threading")]
use crate::threading::thread_spawn_fuel::FuelTrackedThreadManager;

// Fallback types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskManager = ();
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;
#[cfg(not(feature = "component-model-threading"))]
pub enum TaskType {
    AsyncOperation,
}
#[cfg(not(feature = "component-model-threading"))]
pub enum TaskState {
    Ready,
    Completed,
}
use crate::{
    ComponentInstanceId,
    async_::{
        async_types::{Future, FutureHandle, Stream, StreamHandle, Waitable, WaitableSet},
        component_async_bridge::{ComponentAsyncBridge, PollResult},
        fuel_async_executor::{AsyncTaskState, AsyncTaskStatus, FuelAsyncExecutor},
        fuel_dynamic_manager::FuelAllocationPolicy,
        fuel_preemption_support::PreemptionPolicy,
    },
    prelude::*,
};

// Placeholder types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type FuelTrackedThreadManager = ();

/// Maximum async contexts per component
const MAX_ASYNC_CONTEXTS: usize = 256;

/// Async task wrapper for Component Model futures
#[derive(Debug)]
pub struct ComponentAsyncTask {
    /// Component Model task ID
    pub component_task_id: TaskId,
    /// Executor task ID
    #[cfg(feature = "component-model-threading")]
    pub executor_task_id: crate::threading::task_manager::TaskId,
    #[cfg(not(feature = "component-model-threading"))]
    pub executor_task_id: u32,
    /// Component instance
    pub component_id: ComponentInstanceId,
    /// Task type
    pub task_type: ComponentAsyncTaskType,
    /// Future handle (if applicable)
    pub future_handle: Option<FutureHandle>,
    /// Stream handle (if applicable)
    pub stream_handle: Option<StreamHandle>,
    /// Waitables being monitored
    pub waitables: Option<WaitableSet>,
    /// Task priority
    pub priority: Priority,
    /// Creation timestamp
    pub created_at: u64,
    /// Last activity timestamp
    pub last_activity: AtomicU64,
}

impl Clone for ComponentAsyncTask {
    fn clone(&self) -> Self {
        Self {
            component_task_id: self.component_task_id,
            executor_task_id: self.executor_task_id,
            component_id: self.component_id,
            task_type: self.task_type,
            future_handle: self.future_handle,
            stream_handle: self.stream_handle,
            waitables: self.waitables.clone(),
            priority: self.priority,
            created_at: self.created_at,
            last_activity: AtomicU64::new(self.last_activity.load(Ordering::Relaxed)),
        }
    }
}

impl PartialEq for ComponentAsyncTask {
    fn eq(&self, other: &Self) -> bool {
        self.component_task_id == other.component_task_id
            && self.executor_task_id == other.executor_task_id
            && self.component_id == other.component_id
            && self.task_type == other.task_type
            && self.future_handle == other.future_handle
            && self.stream_handle == other.stream_handle
            && self.priority == other.priority
            && self.created_at == other.created_at
            && self.last_activity.load(Ordering::Relaxed)
                == other.last_activity.load(Ordering::Relaxed)
    }
}

impl Eq for ComponentAsyncTask {}

impl Default for ComponentAsyncTask {
    fn default() -> Self {
        Self {
            component_task_id: 0,
            executor_task_id: 0,
            component_id: ComponentInstanceId::new(0),
            task_type: ComponentAsyncTaskType::AsyncOperation,
            future_handle: None,
            stream_handle: None,
            waitables: None,
            priority: 128, // Normal priority as u8
            created_at: 0,
            last_activity: AtomicU64::new(0),
        }
    }
}

impl wrt_runtime::Checksummable for ComponentAsyncTask {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.component_task_id.update_checksum(checksum);
        self.executor_task_id.update_checksum(checksum);
        self.component_id.update_checksum(checksum);
        self.task_type.update_checksum(checksum);
        self.future_handle.update_checksum(checksum);
        self.stream_handle.update_checksum(checksum);
        self.priority.update_checksum(checksum);
        self.created_at.update_checksum(checksum);
        self.last_activity.load(Ordering::Relaxed).update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ComponentAsyncTask {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.component_task_id.to_bytes_with_provider(writer, provider)?;
        self.executor_task_id.to_bytes_with_provider(writer, provider)?;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.task_type.to_bytes_with_provider(writer, provider)?;
        self.future_handle.to_bytes_with_provider(writer, provider)?;
        self.stream_handle.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        self.created_at.to_bytes_with_provider(writer, provider)?;
        self.last_activity
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_runtime::FromBytes for ComponentAsyncTask {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            component_task_id: u32::from_bytes_with_provider(reader, provider)?,
            executor_task_id: u32::from_bytes_with_provider(reader, provider)?,
            component_id: ComponentInstanceId::new(u32::from_bytes_with_provider(
                reader, provider,
            )?),
            task_type: ComponentAsyncTaskType::from_bytes_with_provider(reader, provider)?,
            future_handle: Option::<FutureHandle>::from_bytes_with_provider(reader, provider)?,
            stream_handle: Option::<StreamHandle>::from_bytes_with_provider(reader, provider)?,
            waitables: None,
            priority: Priority::from_bytes_with_provider(reader, provider)?,
            created_at: u64::from_bytes_with_provider(reader, provider)?,
            last_activity: AtomicU64::new(u64::from_bytes_with_provider(reader, provider)?),
        })
    }
}

/// Type of async task in Component Model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentAsyncTaskType {
    /// Regular async function call
    AsyncFunction,
    /// Future value waiting
    FutureWait,
    /// Stream consumption
    StreamConsume,
    /// Resource async operation
    ResourceAsync,
    /// Component lifecycle async
    LifecycleAsync,
    /// Generic async operation
    #[default]
    AsyncOperation,
}

impl wrt_runtime::Checksummable for ComponentAsyncTaskType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        let discriminant = match self {
            ComponentAsyncTaskType::AsyncFunction => 0u8,
            ComponentAsyncTaskType::FutureWait => 1u8,
            ComponentAsyncTaskType::StreamConsume => 2u8,
            ComponentAsyncTaskType::ResourceAsync => 3u8,
            ComponentAsyncTaskType::LifecycleAsync => 4u8,
            ComponentAsyncTaskType::AsyncOperation => 5u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ComponentAsyncTaskType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        let discriminant = match self {
            ComponentAsyncTaskType::AsyncFunction => 0u8,
            ComponentAsyncTaskType::FutureWait => 1u8,
            ComponentAsyncTaskType::StreamConsume => 2u8,
            ComponentAsyncTaskType::ResourceAsync => 3u8,
            ComponentAsyncTaskType::LifecycleAsync => 4u8,
            ComponentAsyncTaskType::AsyncOperation => 5u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for ComponentAsyncTaskType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(ComponentAsyncTaskType::AsyncFunction),
            1 => Ok(ComponentAsyncTaskType::FutureWait),
            2 => Ok(ComponentAsyncTaskType::StreamConsume),
            3 => Ok(ComponentAsyncTaskType::ResourceAsync),
            4 => Ok(ComponentAsyncTaskType::LifecycleAsync),
            5 => Ok(ComponentAsyncTaskType::AsyncOperation),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid ComponentAsyncTaskType discriminant",
            )),
        }
    }
}

/// Task Manager Async Bridge
pub struct TaskManagerAsyncBridge {
    /// Component Model task manager
    task_manager: Arc<Mutex<TaskManager>>,
    /// Async executor bridge
    async_bridge: ComponentAsyncBridge,
    /// Active async tasks
    async_tasks: BoundedMap<TaskId, ComponentAsyncTask, MAX_ASYNC_CONTEXTS>,
    /// Task mapping (component task -> executor task)
    #[cfg(feature = "component-model-threading")]
    task_mapping: BoundedMap<TaskId, crate::threading::task_manager::TaskId, MAX_ASYNC_CONTEXTS>,
    #[cfg(not(feature = "component-model-threading"))]
    task_mapping: BoundedMap<TaskId, u32, MAX_ASYNC_CONTEXTS>,
    /// Component async contexts
    async_contexts: BoundedMap<ComponentInstanceId, ComponentAsyncContext, 128>,
    /// Bridge statistics
    bridge_stats: BridgeStatistics,
    /// Bridge configuration
    config: BridgeConfiguration,
}

/// Per-component async context
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ComponentAsyncContext {
    component_id: ComponentInstanceId,
    /// Active async tasks for this component
    active_tasks: BoundedVec<TaskId, 64>,
    /// Future handles owned by component
    futures: BoundedMap<FutureHandle, TaskId, 64>,
    /// Stream handles owned by component
    streams: BoundedMap<StreamHandle, TaskId, 64>,
    /// Component async state
    async_state: ComponentAsyncState,
    /// Resource limits
    resource_limits: ComponentResourceLimits,
}

impl wrt_runtime::Checksummable for ComponentAsyncContext {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.component_id.update_checksum(checksum);
        self.active_tasks.update_checksum(checksum);
        self.futures.update_checksum(checksum);
        self.streams.update_checksum(checksum);
        self.async_state.update_checksum(checksum);
        self.resource_limits.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ComponentAsyncContext {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.active_tasks.to_bytes_with_provider(writer, provider)?;
        self.futures.to_bytes_with_provider(writer, provider)?;
        self.streams.to_bytes_with_provider(writer, provider)?;
        self.async_state.to_bytes_with_provider(writer, provider)?;
        self.resource_limits.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_runtime::FromBytes for ComponentAsyncContext {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            component_id: ComponentInstanceId::new(u32::from_bytes_with_provider(
                reader, provider,
            )?),
            active_tasks: BoundedVec::from_bytes_with_provider(reader, provider)?,
            futures: BoundedMap::from_bytes_with_provider(reader, provider)?,
            streams: BoundedMap::from_bytes_with_provider(reader, provider)?,
            async_state: ComponentAsyncState::from_bytes_with_provider(reader, provider)?,
            resource_limits: ComponentResourceLimits::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Component async state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentAsyncState {
    /// Component supports async operations
    #[default]
    Active,
    /// Component is suspending async operations
    Suspending,
    /// Component async operations are suspended
    Suspended,
    /// Component is terminating async operations
    Terminating,
    /// Component async operations are terminated
    Terminated,
}

impl wrt_runtime::Checksummable for ComponentAsyncState {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        let discriminant = match self {
            ComponentAsyncState::Active => 0u8,
            ComponentAsyncState::Suspending => 1u8,
            ComponentAsyncState::Suspended => 2u8,
            ComponentAsyncState::Terminating => 3u8,
            ComponentAsyncState::Terminated => 4u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ComponentAsyncState {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        let discriminant = match self {
            ComponentAsyncState::Active => 0u8,
            ComponentAsyncState::Suspending => 1u8,
            ComponentAsyncState::Suspended => 2u8,
            ComponentAsyncState::Terminating => 3u8,
            ComponentAsyncState::Terminated => 4u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for ComponentAsyncState {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(ComponentAsyncState::Active),
            1 => Ok(ComponentAsyncState::Suspending),
            2 => Ok(ComponentAsyncState::Suspended),
            3 => Ok(ComponentAsyncState::Terminating),
            4 => Ok(ComponentAsyncState::Terminated),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid ComponentAsyncState discriminant",
            )),
        }
    }
}

/// Resource limits for component async operations
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentResourceLimits {
    max_concurrent_tasks: usize,
    max_futures: usize,
    max_streams: usize,
    fuel_budget: u64,
    memory_limit: usize,
}

impl wrt_runtime::Checksummable for ComponentResourceLimits {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.max_concurrent_tasks.update_checksum(checksum);
        self.max_futures.update_checksum(checksum);
        self.max_streams.update_checksum(checksum);
        self.fuel_budget.update_checksum(checksum);
        self.memory_limit.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ComponentResourceLimits {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.max_concurrent_tasks.to_bytes_with_provider(writer, provider)?;
        self.max_futures.to_bytes_with_provider(writer, provider)?;
        self.max_streams.to_bytes_with_provider(writer, provider)?;
        self.fuel_budget.to_bytes_with_provider(writer, provider)?;
        self.memory_limit.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_runtime::FromBytes for ComponentResourceLimits {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            max_concurrent_tasks: usize::from_bytes_with_provider(reader, provider)?,
            max_futures: usize::from_bytes_with_provider(reader, provider)?,
            max_streams: usize::from_bytes_with_provider(reader, provider)?,
            fuel_budget: u64::from_bytes_with_provider(reader, provider)?,
            memory_limit: usize::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Bridge statistics
#[derive(Debug, Default)]
struct BridgeStatistics {
    total_async_tasks: AtomicU64,
    completed_tasks: AtomicU64,
    failed_tasks: AtomicU64,
    cancelled_tasks: AtomicU64,
    futures_created: AtomicU64,
    streams_created: AtomicU64,
    preemptions: AtomicU64,
    fuel_exhaustions: AtomicU64,
}

/// Bridge configuration
#[derive(Debug, Clone)]
pub struct BridgeConfiguration {
    /// Enable async task preemption
    pub enable_preemption: bool,
    /// Enable dynamic fuel management  
    pub enable_dynamic_fuel: bool,
    /// Default fuel allocation policy
    pub fuel_policy: FuelAllocationPolicy,
    /// Default preemption policy
    pub preemption_policy: PreemptionPolicy,
    /// Default component resource limits
    pub default_limits: ComponentResourceLimits,
}

impl Default for BridgeConfiguration {
    fn default() -> Self {
        Self {
            enable_preemption: true,
            enable_dynamic_fuel: true,
            fuel_policy: FuelAllocationPolicy::Adaptive,
            preemption_policy: PreemptionPolicy::PriorityBased,
            default_limits: ComponentResourceLimits {
                max_concurrent_tasks: 32,
                max_futures: 64,
                max_streams: 16,
                fuel_budget: 50_000,
                memory_limit: 1024 * 1024, // 1MB
            },
        }
    }
}

impl TaskManagerAsyncBridge {
    /// Create a new task manager async bridge
    pub fn new(
        task_manager: Arc<Mutex<TaskManager>>,
        thread_manager: Arc<Mutex<FuelTrackedThreadManager>>,
        config: BridgeConfiguration,
    ) -> Result<Self> {
        let async_bridge = ComponentAsyncBridge::new(task_manager.clone(), thread_manager)?;
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;

        Ok(Self {
            task_manager,
            async_bridge,
            async_tasks: BoundedMap::new(),
            task_mapping: BoundedMap::new(),
            async_contexts: BoundedMap::new(),
            bridge_stats: BridgeStatistics::default(),
            config,
        })
    }

    /// Initialize component for async operations
    pub fn initialize_component_async(
        &mut self,
        component_id: ComponentInstanceId,
        limits: Option<ComponentResourceLimits>,
    ) -> Result<()> {
        let limits = limits.unwrap_or_else(|| self.config.default_limits.clone());

        // Register with async bridge
        self.async_bridge.register_component(
            component_id,
            limits.max_concurrent_tasks,
            limits.fuel_budget,
            128, // Normal priority
        )?;

        // Create async context
        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        let context = ComponentAsyncContext {
            component_id,
            active_tasks: BoundedVec::new().unwrap(),
            futures: BoundedMap::new(),
            streams: BoundedMap::new(),
            async_state: ComponentAsyncState::Active,
            resource_limits: limits,
        };

        self.async_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component async contexts"))?;

        Ok(())
    }

    /// Spawn an async task from Component Model
    pub fn spawn_async_task<F>(
        &mut self,
        component_id: ComponentInstanceId,
        function_index: Option<u32>,
        future: F,
        task_type: ComponentAsyncTaskType,
        priority: Priority,
    ) -> Result<TaskId>
    where
        F: CoreFuture<Output = Result<Vec<ComponentValue<ComponentProvider>>>> + Send + 'static,
    {
        // Extract timestamp first to avoid borrow conflict
        let timestamp = self.get_timestamp();

        // Check component async context
        let context = self.async_contexts.get_mut(&component_id).ok_or_else(|| {
            Error::validation_invalid_input("Component not initialized for async")
        })?;

        if context.async_state != ComponentAsyncState::Active {
            return Err(Error::validation_invalid_state(
                "Component async operations not active",
            ));
        }

        // Check resource limits
        if context.active_tasks.len() >= context.resource_limits.max_concurrent_tasks {
            return Err(Error::resource_limit_exceeded(
                "Component async task limit exceeded",
            ));
        }

        // Extract fuel budget before mutable borrows
        let fuel_budget = context.resource_limits.fuel_budget
            / context.resource_limits.max_concurrent_tasks as u64;

        // Create Component Model task
        let component_task_id = {
            #[cfg(feature = "component-model-threading")]
            {
                let mut tm = self.task_manager.lock();
                tm.spawn_task(TaskType::AsyncOperation, component_id.0, function_index)?
            }
            #[cfg(not(feature = "component-model-threading"))]
            {
                // When threading is disabled, generate a task ID directly
                0u32
            }
        };

        // Convert future to Result<(), Error> for executor
        let executor_future = async move {
            match future.await {
                Ok(_values) => Ok(()), // Success - values handled elsewhere
                Err(e) => Err(e),
            }
        };

        // Spawn in async executor via bridge
        let executor_task_id = self.async_bridge.spawn_component_async(
            component_id,
            executor_future,
            Some(fuel_budget),
        )?;
        let async_task = ComponentAsyncTask {
            component_task_id,
            executor_task_id,
            component_id,
            task_type,
            future_handle: None,
            stream_handle: None,
            waitables: None,
            priority,
            created_at: timestamp,
            last_activity: AtomicU64::new(timestamp),
        };

        // Store task mappings
        self.async_tasks
            .insert(component_task_id, async_task)
            .map_err(|_| Error::resource_limit_exceeded("Too many async tasks"))?;

        self.task_mapping
            .insert(component_task_id, executor_task_id)
            .map_err(|_| Error::resource_limit_exceeded("Task mapping table full"))?;

        // Update component context
        context
            .active_tasks
            .push(component_task_id)
            .map_err(|_| Error::resource_limit_exceeded("Component task list full"))?;

        // Update statistics
        self.bridge_stats.total_async_tasks.fetch_add(1, Ordering::Relaxed);

        Ok(component_task_id)
    }

    /// Create a future handle for async waiting
    pub fn create_future_handle<T>(
        &mut self,
        component_id: ComponentInstanceId,
        future: crate::async_::async_types::Future<T>,
    ) -> Result<FutureHandle>
    where
        T: wrt_runtime::Checksummable
            + wrt_runtime::ToBytes
            + wrt_runtime::FromBytes
            + Default
            + Clone
            + PartialEq
            + Eq
            + Send
            + 'static,
    {
        // Check limits before spawning
        {
            let context = self
                .async_contexts
                .get(&component_id)
                .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

            if context.futures.len() >= context.resource_limits.max_futures {
                return Err(Error::resource_limit_exceeded(
                    "Component future limit exceeded",
                ));
            }
        }

        // Generate unique handle
        let handle = FutureHandle::new(self.generate_handle_id());

        // Spawn task to handle future
        let task_id = self.spawn_async_task(
            component_id,
            None,
            async move {
                // In real implementation, would poll the future
                Ok(vec![])
            },
            ComponentAsyncTaskType::FutureWait,
            128, // Normal priority
        )?;

        // Store handle mapping
        let context = self
            .async_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;
        context
            .futures
            .insert(handle, task_id)
            .map_err(|_| Error::resource_limit_exceeded("Future handle table full"))?;

        self.bridge_stats.futures_created.fetch_add(1, Ordering::Relaxed);

        Ok(handle)
    }

    /// Create a stream handle for async iteration
    pub fn create_stream_handle<T>(
        &mut self,
        component_id: ComponentInstanceId,
        stream: crate::async_::async_types::Stream<T>,
    ) -> Result<StreamHandle>
    where
        T: wrt_runtime::Checksummable
            + wrt_runtime::ToBytes
            + wrt_runtime::FromBytes
            + Default
            + Clone
            + PartialEq
            + Eq
            + Send
            + 'static,
    {
        // Generate handle and check limits before spawning
        let handle = StreamHandle::new(self.generate_handle_id());

        {
            let context = self
                .async_contexts
                .get(&component_id)
                .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

            if context.streams.len() >= context.resource_limits.max_streams {
                return Err(Error::resource_limit_exceeded(
                    "Component stream limit exceeded",
                ));
            }
        }

        // Spawn task to handle stream
        let task_id = self.spawn_async_task(
            component_id,
            None,
            async move {
                // In real implementation, would consume the stream
                Ok(vec![])
            },
            ComponentAsyncTaskType::StreamConsume,
            128, // Normal priority
        )?;

        // Store handle mapping
        let context = self
            .async_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;
        context
            .streams
            .insert(handle, task_id)
            .map_err(|_| Error::resource_limit_exceeded("Stream handle table full"))?;

        self.bridge_stats.streams_created.fetch_add(1, Ordering::Relaxed);

        Ok(handle)
    }

    /// Wait on multiple waitables (task.wait implementation)
    pub fn task_wait(&mut self, waitables: WaitableSet) -> Result<u32> {
        #[cfg(feature = "component-model-threading")]
        {
            let current_task = {
                let tm = self.task_manager.lock();
                tm.current_task_id()
                    .ok_or_else(|| Error::validation_invalid_state("No current task"))?
            };

            // Check if any waitables are immediately ready
            if let Some(ready_index) = waitables.first_ready() {
                return Ok(ready_index);
            }

            // Update task with waitables
            if let Some(async_task) = self.async_tasks.get_mut(&current_task) {
                async_task.waitables = Some(waitables.clone());
                async_task.last_activity.store(self.get_timestamp(), Ordering::Release);
            }

            // Delegate to task manager
            let mut tm = self.task_manager.lock();
            tm.task_wait(waitables)
        }
        #[cfg(not(feature = "component-model-threading"))]
        {
            // When threading is disabled, check if any waitables are ready
            waitables
                .first_ready()
                .ok_or_else(|| Error::validation_invalid_state("No waitables ready"))
        }
    }

    /// Poll waitables without blocking (task.poll implementation)
    pub fn task_poll(&self, waitables: &WaitableSet) -> Result<Option<u32>> {
        #[cfg(feature = "component-model-threading")]
        {
            let tm = self.task_manager.lock();
            tm.task_poll(waitables)
        }
        #[cfg(not(feature = "component-model-threading"))]
        {
            Ok(waitables.first_ready())
        }
    }

    /// Yield current task (task.yield implementation)
    pub fn task_yield(&mut self) -> Result<()> {
        #[cfg(feature = "component-model-threading")]
        {
            let current_task = {
                let tm = self.task_manager.lock();
                tm.current_task_id()
                    .ok_or_else(|| Error::validation_invalid_state("No current task"))?
            };

            // Update task activity
            if let Some(async_task) = self.async_tasks.get(&current_task) {
                async_task.last_activity.store(self.get_timestamp(), Ordering::Release);
            }

            // Delegate to task manager
            let mut tm = self.task_manager.lock();
            tm.task_yield()
        }
        #[cfg(not(feature = "component-model-threading"))]
        {
            // When threading is disabled, yielding is a no-op
            Ok(())
        }
    }

    /// Poll all async tasks and update state
    pub fn poll_async_tasks(&mut self) -> Result<PollResult> {
        // Poll the async bridge
        let mut result = self.async_bridge.poll_async_tasks()?;

        // Update Component Model task states
        #[cfg(feature = "component-model-threading")]
        {
            let mut completed_tasks = Vec::new();
            for (comp_task_id, async_task) in self.async_tasks.iter() {
                if self.async_bridge.is_task_ready(*comp_task_id)? {
                    // Task is ready, update Component Model task state
                    let mut tm = self.task_manager.lock();
                    if let Some(task) = tm.get_task_mut(*comp_task_id) {
                        task.state = TaskState::Ready;
                    }
                }

                // Check if task completed via executor
                // In real implementation, would check executor status
            }
        }

        // Update statistics
        self.bridge_stats
            .completed_tasks
            .fetch_add(result.tasks_completed as u64, Ordering::Relaxed);
        self.bridge_stats
            .failed_tasks
            .fetch_add(result.tasks_failed as u64, Ordering::Relaxed);

        Ok(result)
    }

    /// Suspend component async operations
    pub fn suspend_component_async(&mut self, component_id: ComponentInstanceId) -> Result<()> {
        let context = self
            .async_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not found"))?;

        context.async_state = ComponentAsyncState::Suspending;

        // Cancel all active tasks
        #[cfg(feature = "component-model-threading")]
        {
            for &task_id in context.active_tasks.iter() {
                if let Some(_async_task) = self.async_tasks.get(&task_id) {
                    // In real implementation, would gracefully suspend tasks
                    let mut tm = self.task_manager.lock();
                    tm.task_cancel(task_id)?;
                }
            }
        }
        #[cfg(not(feature = "component-model-threading"))]
        {
            // When threading is disabled, just clear the task list
            // Tasks are already tracked elsewhere
        }

        context.async_state = ComponentAsyncState::Suspended;
        Ok(())
    }

    /// Get bridge statistics
    /// Check if a task is ready
    pub fn is_task_ready(&self, task_id: TaskId) -> Result<bool> {
        self.async_bridge.is_task_ready(task_id)
    }

    /// Get a reference to the async bridge
    pub fn async_bridge(&self) -> &ComponentAsyncBridge {
        &self.async_bridge
    }

    pub fn get_bridge_statistics(&self) -> BridgeStats {
        BridgeStats {
            total_async_tasks: self.bridge_stats.total_async_tasks.load(Ordering::Relaxed),
            completed_tasks: self.bridge_stats.completed_tasks.load(Ordering::Relaxed),
            failed_tasks: self.bridge_stats.failed_tasks.load(Ordering::Relaxed),
            cancelled_tasks: self.bridge_stats.cancelled_tasks.load(Ordering::Relaxed),
            futures_created: self.bridge_stats.futures_created.load(Ordering::Relaxed),
            streams_created: self.bridge_stats.streams_created.load(Ordering::Relaxed),
            active_components: self.async_contexts.len() as u64,
        }
    }

    // Private helper methods

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }

    fn generate_handle_id(&self) -> u32 {
        // In real implementation, would generate unique IDs
        42
    }

    /// Cleanup completed async task
    fn cleanup_async_task(&mut self, task_id: TaskId) -> Result<()> {
        if let Some(async_task) = self.async_tasks.remove(&task_id) {
            // Remove from component context
            if let Some(context) = self.async_contexts.get_mut(&async_task.component_id) {
                context.active_tasks.retain(|&id| id != task_id);
            }

            // Remove mapping
            self.task_mapping.remove(&task_id);
        }
        Ok(())
    }
}

/// Bridge statistics
#[derive(Debug, Clone)]
pub struct BridgeStats {
    pub total_async_tasks: u64,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub cancelled_tasks: u64,
    pub futures_created: u64,
    pub streams_created: u64,
    pub active_components: u64,
}
