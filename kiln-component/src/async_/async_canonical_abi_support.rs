//! Async Canonical ABI support for WebAssembly Component Model
//!
//! This module implements async support for the Canonical ABI, enabling
//! async lifting/lowering of function calls, resource operations, and streams.

use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};

use wrt_foundation::{
    CrateId,
    clean_types::{ComponentType, FuncType, ValType},
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    // resource::ResourceHandle, // Not available - using local placeholder
    safe_managed_alloc,
};
use wrt_platform::advanced_sync::Priority;

// Placeholder ResourceHandle type until proper resource system is implemented
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceHandle(u32);

impl ResourceHandle {
    /// Create a new ResourceHandle with the given ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw ID
    pub fn id(&self) -> u32 {
        self.0
    }
}

impl wrt_foundation::traits::Checksummable for ResourceHandle {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for ResourceHandle {
    fn serialized_size(&self) -> usize {
        core::mem::size_of::<u32>()
    }

    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for ResourceHandle {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

// Placeholder types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type FuelTrackedThreadManager = ();

use crate::{
    ComponentInstanceId,
    async_::{
        async_types::{Future, FutureHandle, Stream, StreamHandle, Waitable, WaitableSet},
        fuel_async_executor::AsyncTaskState,
        task_manager_async_bridge::{ComponentAsyncTaskType, TaskManagerAsyncBridge},
    },
    bounded_component_infra::ComponentProvider,
    canonical_abi::{CanonicalOptions, LiftingContext, LoweringContext},
    prelude::*,
    types::Value,
};

/// Maximum async ABI operations per component
const MAX_ASYNC_ABI_OPS: usize = 512;

/// Async fuel costs for ABI operations
const ABI_ASYNC_CALL_FUEL: u64 = 50;
const ABI_ASYNC_LIFT_FUEL: u64 = 30;
const ABI_ASYNC_LOWER_FUEL: u64 = 30;
const ABI_RESOURCE_ASYNC_FUEL: u64 = 40;

/// Async Canonical ABI support
pub struct AsyncCanonicalAbiSupport {
    /// Task manager bridge
    bridge: TaskManagerAsyncBridge,
    /// Active async ABI operations
    async_operations: BoundedMap<AsyncAbiOperationId, AsyncAbiOperation, MAX_ASYNC_ABI_OPS>,
    /// Component ABI contexts
    abi_contexts: BoundedMap<ComponentInstanceId, ComponentAbiContext, 128>,
    /// Next operation ID
    next_operation_id: AtomicU64,
    /// ABI statistics
    abi_stats: AbiStatistics,
}

/// Async ABI operation identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct AsyncAbiOperationId(u64);

impl wrt_foundation::traits::Checksummable for AsyncAbiOperationId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for AsyncAbiOperationId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for AsyncAbiOperationId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u64::from_bytes_with_provider(reader, provider)?))
    }
}

/// Async ABI operation
#[derive(Debug)]
struct AsyncAbiOperation {
    id: AsyncAbiOperationId,
    component_id: ComponentInstanceId,
    operation_type: AsyncAbiOperationType,
    function_type: Option<FuncType>,
    resource_handle: Option<ResourceHandle>,
    lifting_context: Option<LiftingContext>,
    lowering_context: Option<LoweringContext>,
    #[cfg(feature = "component-model-threading")]
    task_id: Option<crate::threading::task_manager::TaskId>,
    #[cfg(not(feature = "component-model-threading"))]
    task_id: Option<u32>,
    created_at: u64,
    fuel_consumed: AtomicU64,
}

/// Type of async ABI operation
#[derive(Debug, Clone, PartialEq)]
pub enum AsyncAbiOperationType {
    /// Async function call
    AsyncCall {
        function_name: String,
        args: Vec<WrtComponentValue<ComponentProvider>>,
    },
    /// Async resource method call
    ResourceAsync {
        method_name: String,
        args: Vec<WrtComponentValue<ComponentProvider>>,
    },
    /// Async value lifting
    AsyncLift {
        target_type: ComponentType,
        source_values: Vec<Value>,
    },
    /// Async value lowering
    AsyncLower {
        source_values: Vec<WrtComponentValue<ComponentProvider>>,
        target_type: ValType,
    },
    /// Future handling
    FutureOperation {
        future_handle: FutureHandle,
        operation: FutureOp,
    },
    /// Stream handling
    StreamOperation {
        stream_handle: StreamHandle,
        operation: StreamOp,
    },
}

/// Future operation type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FutureOp {
    /// Read future value
    Read,
    /// Cancel future
    Cancel,
    /// Check if ready
    Poll,
}

/// Stream operation type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamOp {
    /// Read next value
    ReadNext,
    /// Write value
    Write(WrtComponentValue<ComponentProvider>),
    /// Close stream
    Close,
    /// Check available
    Poll,
}

/// Component ABI context
#[derive(Debug)]
struct ComponentAbiContext {
    component_id: ComponentInstanceId,
    /// Default canonical options for async operations
    default_options: CanonicalOptions,
    /// Active async calls
    active_calls: BoundedVec<AsyncAbiOperationId, 64>,
    /// Resource async operations
    resource_operations: BoundedMap<ResourceHandle, Vec<AsyncAbiOperationId>, 32>,
    /// Future callbacks
    future_callbacks: BoundedMap<FutureHandle, AsyncAbiOperationId, 64>,
    /// Stream callbacks
    stream_callbacks: BoundedMap<StreamHandle, AsyncAbiOperationId, 32>,
}

/// ABI operation statistics
#[derive(Debug, Default)]
struct AbiStatistics {
    total_async_calls: AtomicU64,
    completed_calls: AtomicU64,
    failed_calls: AtomicU64,
    async_lifts: AtomicU64,
    async_lowers: AtomicU64,
    resource_operations: AtomicU64,
    future_operations: AtomicU64,
    stream_operations: AtomicU64,
    total_fuel_consumed: AtomicU64,
}

impl AsyncCanonicalAbiSupport {
    /// Create new async ABI support
    pub fn new(bridge: TaskManagerAsyncBridge) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        Ok(Self {
            bridge,
            async_operations: BoundedMap::new(),
            abi_contexts: BoundedMap::new(),
            next_operation_id: AtomicU64::new(1),
            abi_stats: AbiStatistics::default(),
        })
    }

    /// Initialize component for async ABI operations
    pub fn initialize_component_abi(
        &mut self,
        component_id: ComponentInstanceId,
        default_options: CanonicalOptions,
    ) -> Result<()> {
        let provider = safe_managed_alloc!(2048, CrateId::Component)?;

        let context = ComponentAbiContext {
            component_id,
            default_options,
            active_calls: BoundedVec::new().unwrap(),
            resource_operations: BoundedMap::new(),
            future_callbacks: BoundedMap::new(),
            stream_callbacks: BoundedMap::new(),
        };

        self.abi_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component ABI contexts"))?;

        Ok(())
    }

    /// Perform async function call
    pub fn async_call(
        &mut self,
        component_id: ComponentInstanceId,
        function_name: String,
        func_type: FuncType,
        args: Vec<ComponentValue<ComponentProvider>>,
        options: Option<CanonicalOptions>,
    ) -> Result<AsyncAbiOperationId> {
        // Extract default options before mutable borrow
        let default_options = self
            .abi_contexts
            .get(&component_id)
            .ok_or_else(|| {
                Error::validation_invalid_input("Component not initialized for async ABI")
            })?
            .default_options
            .clone();

        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let options = options.unwrap_or(default_options);

        // Create async operation
        let operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::AsyncCall {
                function_name: function_name.clone(),
                args: args.clone(),
            },
            function_type: Some(func_type.clone()),
            resource_handle: None,
            lifting_context: Some(LiftingContext::new(options.clone())),
            lowering_context: Some(LoweringContext::new(options)),
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for the call
        let operation_id_copy = operation_id;
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None, // function index determined at runtime
            async move {
                // Simulate async function execution
                // In real implementation, would:
                // 1. Lower arguments
                // 2. Call WebAssembly function
                // 3. Lift results
                // 4. Handle async yields/waits

                // Consume fuel for async call
                let fuel_cost = ABI_ASYNC_CALL_FUEL + (args.len() as u64 * 10);
                // Would track fuel consumption here

                Ok(vec![]) // Return lifted values
            },
            ComponentAsyncTaskType::AsyncFunction,
            128, // Normal priority
        )?;

        // Store operation
        let mut stored_operation = operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async ABI operations"))?;

        // Add to component context - get mutable reference after all other operations
        if let Some(context) = self.abi_contexts.get_mut(&component_id) {
            context
                .active_calls
                .push(operation_id)
                .map_err(|_| Error::resource_limit_exceeded("Component active calls full"))?;
        }

        // Update statistics
        self.abi_stats.total_async_calls.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Perform async resource operation
    pub fn async_resource_call(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
        method_name: String,
        args: Vec<ComponentValue<ComponentProvider>>,
    ) -> Result<AsyncAbiOperationId> {
        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::ResourceAsync {
                method_name: method_name.clone(),
                args: args.clone(),
            },
            function_type: None,
            resource_handle: Some(resource_handle),
            lifting_context: None,
            lowering_context: None,
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for resource operation
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None,
            async move {
                // Simulate async resource operation
                // In real implementation would call resource method
                Ok(vec![])
            },
            ComponentAsyncTaskType::ResourceAsync,
            128, // Normal priority
        )?;

        let mut stored_operation = operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async operations"))?;

        // Track in component context
        if let Some(context) = self.abi_contexts.get_mut(&component_id) {
            context
                .resource_operations
                .entry(resource_handle)
                .or_insert_with(Vec::new)?
                .push(operation_id);
        }

        self.abi_stats.resource_operations.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Perform async value lifting
    pub fn async_lift(
        &mut self,
        component_id: ComponentInstanceId,
        source_values: Vec<Value>,
        target_type: ComponentType,
        options: Option<CanonicalOptions>,
    ) -> Result<AsyncAbiOperationId> {
        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let context = self
            .abi_contexts
            .get(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

        let options = options.unwrap_or_else(|| context.default_options.clone());

        let operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::AsyncLift {
                target_type: target_type.clone(),
                source_values: source_values.clone(),
            },
            function_type: None,
            resource_handle: None,
            lifting_context: Some(LiftingContext::new(options)),
            lowering_context: None,
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for lifting
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None,
            async move {
                // Simulate async lifting
                // In real implementation, would perform complex lifting
                Ok(vec![])
            },
            ComponentAsyncTaskType::AsyncFunction,
            128, // Normal priority
        )?;

        let mut stored_operation = operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async operations"))?;

        self.abi_stats.async_lifts.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Perform async value lowering
    pub fn async_lower(
        &mut self,
        component_id: ComponentInstanceId,
        source_values: Vec<WrtComponentValue<ComponentProvider>>,
        target_type: ValType,
        options: Option<CanonicalOptions>,
    ) -> Result<AsyncAbiOperationId> {
        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let context = self
            .abi_contexts
            .get(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

        let options = options.unwrap_or_else(|| context.default_options.clone());

        let operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::AsyncLower {
                source_values: source_values.clone(),
                target_type: target_type.clone(),
            },
            function_type: None,
            resource_handle: None,
            lifting_context: None,
            lowering_context: Some(LoweringContext::new(options)),
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for lowering
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None,
            async move {
                // Simulate async lowering
                Ok(vec![])
            },
            ComponentAsyncTaskType::AsyncFunction,
            128, // Normal priority
        )?;

        let mut stored_operation = operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async operations"))?;

        self.abi_stats.async_lowers.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Handle future operation
    pub fn handle_future_operation(
        &mut self,
        component_id: ComponentInstanceId,
        future_handle: FutureHandle,
        operation: FutureOp,
    ) -> Result<AsyncAbiOperationId> {
        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let abi_operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::FutureOperation {
                future_handle,
                operation: operation.clone(),
            },
            function_type: None,
            resource_handle: None,
            lifting_context: None,
            lowering_context: None,
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for future operation
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None,
            async move {
                match operation {
                    FutureOp::Read => {
                        // Read future value
                        Ok(vec![])
                    },
                    FutureOp::Cancel => {
                        // Cancel future
                        Ok(vec![])
                    },
                    FutureOp::Poll => {
                        // Poll future readiness
                        Ok(vec![])
                    },
                }
            },
            ComponentAsyncTaskType::FutureWait,
            128, // Normal priority
        )?;

        let mut stored_operation = abi_operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async operations"))?;

        // Register callback
        if let Some(context) = self.abi_contexts.get_mut(&component_id) {
            context.future_callbacks.insert(future_handle, operation_id).ok();
        }

        self.abi_stats.future_operations.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Handle stream operation
    pub fn handle_stream_operation(
        &mut self,
        component_id: ComponentInstanceId,
        stream_handle: StreamHandle,
        operation: StreamOp,
    ) -> Result<AsyncAbiOperationId> {
        let operation_id =
            AsyncAbiOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let abi_operation = AsyncAbiOperation {
            id: operation_id,
            component_id,
            operation_type: AsyncAbiOperationType::StreamOperation {
                stream_handle,
                operation: operation.clone(),
            },
            function_type: None,
            resource_handle: None,
            lifting_context: None,
            lowering_context: None,
            task_id: None,
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        // Spawn async task for stream operation
        let task_id = self.bridge.spawn_async_task(
            component_id,
            None,
            async move {
                match operation {
                    StreamOp::ReadNext => {
                        // Read next stream value
                        Ok(vec![])
                    },
                    StreamOp::Write(_value) => {
                        // Write to stream
                        Ok(vec![])
                    },
                    StreamOp::Close => {
                        // Close stream
                        Ok(vec![])
                    },
                    StreamOp::Poll => {
                        // Poll stream availability
                        Ok(vec![])
                    },
                }
            },
            ComponentAsyncTaskType::StreamConsume,
            128, // Normal priority
        )?;

        let mut stored_operation = abi_operation;
        stored_operation.task_id = Some(task_id);

        self.async_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many async operations"))?;

        // Register callback
        if let Some(context) = self.abi_contexts.get_mut(&component_id) {
            context.stream_callbacks.insert(stream_handle, operation_id).ok();
        }

        self.abi_stats.stream_operations.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Check operation status
    pub fn check_operation_status(
        &self,
        operation_id: AsyncAbiOperationId,
    ) -> Result<AsyncAbiOperationStatus> {
        let operation = self
            .async_operations
            .get(&operation_id)
            .ok_or_else(|| Error::validation_invalid_input("Operation not found"))?;

        let task_status = if let Some(task_id) = operation.task_id {
            self.bridge.async_bridge().is_task_ready(task_id)?
        } else {
            false
        };

        Ok(AsyncAbiOperationStatus {
            operation_id,
            component_id: operation.component_id,
            operation_type: operation.operation_type.clone(),
            is_ready: task_status,
            fuel_consumed: operation.fuel_consumed.load(Ordering::Acquire),
            created_at: operation.created_at,
        })
    }

    /// Poll all async ABI operations
    pub fn poll_async_operations(&mut self) -> Result<AbiPollResult> {
        // Poll underlying bridge
        let bridge_result = self.bridge.poll_async_tasks()?;

        let mut completed_operations = Vec::new();
        let mut ready_operations = 0;

        // Check operation statuses
        for (op_id, operation) in self.async_operations.iter() {
            if let Some(task_id) = operation.task_id {
                if self.bridge.async_bridge().is_task_ready(task_id)? {
                    ready_operations += 1;
                    // In real implementation, would check if completed
                }
            }
        }

        // Clean up completed operations
        for op_id in completed_operations {
            self.cleanup_operation(op_id)?;
        }

        Ok(AbiPollResult {
            ready_operations,
            completed_operations: bridge_result.tasks_completed,
            failed_operations: bridge_result.tasks_failed,
            total_fuel_consumed: bridge_result.total_fuel_consumed,
        })
    }

    /// Get ABI statistics
    pub fn get_abi_statistics(&self) -> AbiStats {
        AbiStats {
            total_async_calls: self.abi_stats.total_async_calls.load(Ordering::Relaxed),
            completed_calls: self.abi_stats.completed_calls.load(Ordering::Relaxed),
            failed_calls: self.abi_stats.failed_calls.load(Ordering::Relaxed),
            async_lifts: self.abi_stats.async_lifts.load(Ordering::Relaxed),
            async_lowers: self.abi_stats.async_lowers.load(Ordering::Relaxed),
            resource_operations: self.abi_stats.resource_operations.load(Ordering::Relaxed),
            future_operations: self.abi_stats.future_operations.load(Ordering::Relaxed),
            stream_operations: self.abi_stats.stream_operations.load(Ordering::Relaxed),
            active_operations: self.async_operations.len() as u64,
        }
    }

    // Private helper methods

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }

    fn cleanup_operation(&mut self, operation_id: AsyncAbiOperationId) -> Result<()> {
        if let Some(operation) = self.async_operations.remove(&operation_id) {
            // Remove from component context
            if let Some(context) = self.abi_contexts.get_mut(&operation.component_id) {
                context.active_calls.retain(|&id| id != operation_id);

                // Clean up callbacks
                match &operation.operation_type {
                    AsyncAbiOperationType::FutureOperation { future_handle, .. } => {
                        context.future_callbacks.remove(future_handle);
                    },
                    AsyncAbiOperationType::StreamOperation { stream_handle, .. } => {
                        context.stream_callbacks.remove(stream_handle);
                    },
                    _ => {},
                }
            }
        }
        Ok(())
    }
}

/// Async ABI operation status
#[derive(Debug, Clone)]
pub struct AsyncAbiOperationStatus {
    pub operation_id: AsyncAbiOperationId,
    pub component_id: ComponentInstanceId,
    pub operation_type: AsyncAbiOperationType,
    pub is_ready: bool,
    pub fuel_consumed: u64,
    pub created_at: u64,
}

/// ABI poll result
#[derive(Debug, Clone)]
pub struct AbiPollResult {
    pub ready_operations: usize,
    pub completed_operations: usize,
    pub failed_operations: usize,
    pub total_fuel_consumed: u64,
}

/// ABI statistics
#[derive(Debug, Clone)]
pub struct AbiStats {
    pub total_async_calls: u64,
    pub completed_calls: u64,
    pub failed_calls: u64,
    pub async_lifts: u64,
    pub async_lowers: u64,
    pub resource_operations: u64,
    pub future_operations: u64,
    pub stream_operations: u64,
    pub active_operations: u64,
}
