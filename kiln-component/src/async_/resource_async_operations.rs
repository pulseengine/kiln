//! Async operations for WebAssembly Component Model resources
//!
//! This module provides async support for resource operations including
//! resource creation, method calls, and lifecycle management.

use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    task::{Context, Poll},
};

use wrt_foundation::{
    CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    component_value::ComponentValue,
    // ResourceHandle imported from local crate instead
    // resource::{
    //     ResourceHandle,
    //     ResourceType,
    // },
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ToBytes},
};
use wrt_platform::advanced_sync::Priority;

use crate::{
    // resource_lifecycle::ResourceLifecycleManager, // Module not available
    ComponentInstanceId,
    async_::{
        async_canonical_abi_support::{
            AsyncAbiOperationId, AsyncCanonicalAbiSupport, ResourceHandle,
        },
        task_manager_async_bridge::{ComponentAsyncTaskType, TaskManagerAsyncBridge},
    },
    prelude::*,
};

// Placeholder types for missing imports
pub type ResourceType = u32;
pub type ResourceLifecycleManager = ();

/// Maximum async resource operations per component
const MAX_ASYNC_RESOURCE_OPS: usize = 256;

/// Fuel costs for resource async operations
const RESOURCE_CREATE_FUEL: u64 = 100;
const RESOURCE_METHOD_FUEL: u64 = 50;
const RESOURCE_DROP_FUEL: u64 = 30;
const RESOURCE_BORROW_FUEL: u64 = 20;

/// Async resource operations manager
pub struct ResourceAsyncOperations {
    /// ABI support for resource operations
    abi_support: AsyncCanonicalAbiSupport,
    /// Resource lifecycle manager
    lifecycle_manager: ResourceLifecycleManager,
    /// Active resource operations
    active_operations:
        BoundedMap<ResourceOperationId, ResourceAsyncOperation, MAX_ASYNC_RESOURCE_OPS>,
    /// Component resource contexts
    resource_contexts: BoundedMap<ComponentInstanceId, ComponentResourceContext, 128>,
    /// Next operation ID
    next_operation_id: AtomicU64,
    /// Resource operation statistics
    resource_stats: ResourceOperationStats,
}

/// Resource operation identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ResourceOperationId(u64);

impl Checksummable for ResourceOperationId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for ResourceOperationId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ResourceOperationId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u64::from_bytes_with_provider(reader, provider)?))
    }
}

/// Async resource operation
struct ResourceAsyncOperation {
    id: ResourceOperationId,
    component_id: ComponentInstanceId,
    operation_type: ResourceAsyncOperationType,
    resource_handle: Option<ResourceHandle>,
    resource_type: ResourceType,
    abi_operation_id: Option<AsyncAbiOperationId>,
    created_at: u64,
    fuel_consumed: AtomicU64,
    completion_callback: Option<ResourceCompletionCallback>,
}

impl core::fmt::Debug for ResourceAsyncOperation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourceAsyncOperation")
            .field("id", &self.id)
            .field("component_id", &self.component_id)
            .field("operation_type", &self.operation_type)
            .field("resource_handle", &self.resource_handle)
            .field("resource_type", &self.resource_type)
            .field("abi_operation_id", &self.abi_operation_id)
            .field("created_at", &self.created_at)
            .field("fuel_consumed", &self.fuel_consumed)
            .field("completion_callback", &self.completion_callback.is_some())
            .finish()
    }
}

/// Type of async resource operation
#[derive(Debug, Clone)]
pub enum ResourceAsyncOperationType {
    /// Create new resource instance
    Create {
        constructor_args: Vec<ComponentValue<ComponentProvider>>,
    },
    /// Call async method on resource
    MethodCall {
        method_name: String,
        args: Vec<ComponentValue<ComponentProvider>>,
    },
    /// Borrow resource for async operation
    Borrow { borrow_type: ResourceBorrowType },
    /// Drop resource asynchronously
    Drop,
    /// Resource finalization
    Finalize,
    /// Resource state transition
    StateTransition {
        from_state: ResourceState,
        to_state: ResourceState,
    },
}

/// Resource borrow type for async operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceBorrowType {
    /// Shared borrow (read-only)
    Shared,
    /// Exclusive borrow (read-write)
    Exclusive,
    /// Async borrow (for async operations)
    Async,
}

impl Default for ResourceBorrowType {
    fn default() -> Self {
        Self::Shared
    }
}

impl Checksummable for ResourceBorrowType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            ResourceBorrowType::Shared => 0u8,
            ResourceBorrowType::Exclusive => 1u8,
            ResourceBorrowType::Async => 2u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for ResourceBorrowType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            ResourceBorrowType::Shared => 0u8,
            ResourceBorrowType::Exclusive => 1u8,
            ResourceBorrowType::Async => 2u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ResourceBorrowType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(ResourceBorrowType::Shared),
            1 => Ok(ResourceBorrowType::Exclusive),
            2 => Ok(ResourceBorrowType::Async),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid ResourceBorrowType discriminant",
            )),
        }
    }
}

/// Resource state for lifecycle management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceState {
    /// Resource is being created
    Creating,
    /// Resource is active and usable
    Active,
    /// Resource is borrowed
    Borrowed,
    /// Resource is being finalized
    Finalizing,
    /// Resource is dropped
    Dropped,
    /// Resource is in error state
    Error,
}

impl Default for ResourceState {
    fn default() -> Self {
        Self::Creating
    }
}

impl Checksummable for ResourceState {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            ResourceState::Creating => 0u8,
            ResourceState::Active => 1u8,
            ResourceState::Borrowed => 2u8,
            ResourceState::Finalizing => 3u8,
            ResourceState::Dropped => 4u8,
            ResourceState::Error => 5u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for ResourceState {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            ResourceState::Creating => 0u8,
            ResourceState::Active => 1u8,
            ResourceState::Borrowed => 2u8,
            ResourceState::Finalizing => 3u8,
            ResourceState::Dropped => 4u8,
            ResourceState::Error => 5u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ResourceState {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(ResourceState::Creating),
            1 => Ok(ResourceState::Active),
            2 => Ok(ResourceState::Borrowed),
            3 => Ok(ResourceState::Finalizing),
            4 => Ok(ResourceState::Dropped),
            5 => Ok(ResourceState::Error),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid ResourceState discriminant",
            )),
        }
    }
}

/// Component resource context
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ComponentResourceContext {
    component_id: ComponentInstanceId,
    /// Active resource handles owned by component
    owned_resources: BoundedMap<ResourceHandle, ResourceInfo, 128>,
    /// Borrowed resource handles
    borrowed_resources: BoundedMap<ResourceHandle, BorrowInfo, 64>,
    /// Active async operations
    active_operations: BoundedVec<ResourceOperationId, 64>,
    /// Resource quotas and limits
    resource_limits: ResourceLimits,
    /// Async resource configuration
    async_config: AsyncResourceConfig,
}

impl wrt_foundation::traits::Checksummable for ComponentResourceContext {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.component_id.update_checksum(checksum);
        self.owned_resources.update_checksum(checksum);
        self.borrowed_resources.update_checksum(checksum);
        self.active_operations.update_checksum(checksum);
        self.resource_limits.update_checksum(checksum);
        self.async_config.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for ComponentResourceContext {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.owned_resources.to_bytes_with_provider(writer, provider)?;
        self.borrowed_resources.to_bytes_with_provider(writer, provider)?;
        self.active_operations.to_bytes_with_provider(writer, provider)?;
        self.resource_limits.to_bytes_with_provider(writer, provider)?;
        self.async_config.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ComponentResourceContext {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            component_id: ComponentInstanceId::from_bytes_with_provider(reader, provider)?,
            owned_resources: BoundedMap::from_bytes_with_provider(reader, provider)?,
            borrowed_resources: BoundedMap::from_bytes_with_provider(reader, provider)?,
            active_operations: BoundedVec::from_bytes_with_provider(reader, provider)?,
            resource_limits: ResourceLimits::from_bytes_with_provider(reader, provider)?,
            async_config: AsyncResourceConfig::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Resource information
#[derive(Debug)]
struct ResourceInfo {
    handle: ResourceHandle,
    resource_type: ResourceType,
    state: ResourceState,
    created_at: u64,
    last_accessed: AtomicU64,
    reference_count: AtomicU32,
    async_operations: AtomicU32,
}

impl Clone for ResourceInfo {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            resource_type: self.resource_type,
            state: self.state,
            created_at: self.created_at,
            last_accessed: AtomicU64::new(self.last_accessed.load(Ordering::Relaxed)),
            reference_count: AtomicU32::new(self.reference_count.load(Ordering::Relaxed)),
            async_operations: AtomicU32::new(self.async_operations.load(Ordering::Relaxed)),
        }
    }
}

impl PartialEq for ResourceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
            && self.resource_type == other.resource_type
            && self.state == other.state
            && self.created_at == other.created_at
            && self.last_accessed.load(Ordering::Relaxed)
                == other.last_accessed.load(Ordering::Relaxed)
            && self.reference_count.load(Ordering::Relaxed)
                == other.reference_count.load(Ordering::Relaxed)
            && self.async_operations.load(Ordering::Relaxed)
                == other.async_operations.load(Ordering::Relaxed)
    }
}

impl Eq for ResourceInfo {}

impl Default for ResourceInfo {
    fn default() -> Self {
        Self {
            handle: ResourceHandle::new(0),
            resource_type: ResourceType::default(),
            state: ResourceState::default(),
            created_at: 0,
            last_accessed: AtomicU64::new(0),
            reference_count: AtomicU32::new(0),
            async_operations: AtomicU32::new(0),
        }
    }
}

impl wrt_foundation::traits::Checksummable for ResourceInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.handle.update_checksum(checksum);
        self.resource_type.update_checksum(checksum);
        self.state.update_checksum(checksum);
        self.created_at.update_checksum(checksum);
        self.last_accessed.load(Ordering::Relaxed).update_checksum(checksum);
        self.reference_count.load(Ordering::Relaxed).update_checksum(checksum);
        self.async_operations.load(Ordering::Relaxed).update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for ResourceInfo {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.handle.to_bytes_with_provider(writer, provider)?;
        self.resource_type.to_bytes_with_provider(writer, provider)?;
        self.state.to_bytes_with_provider(writer, provider)?;
        self.created_at.to_bytes_with_provider(writer, provider)?;
        self.last_accessed
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.reference_count
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.async_operations
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ResourceInfo {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            handle: ResourceHandle::from_bytes_with_provider(reader, provider)?,
            resource_type: ResourceType::from_bytes_with_provider(reader, provider)?,
            state: ResourceState::from_bytes_with_provider(reader, provider)?,
            created_at: u64::from_bytes_with_provider(reader, provider)?,
            last_accessed: AtomicU64::new(u64::from_bytes_with_provider(reader, provider)?),
            reference_count: AtomicU32::new(u32::from_bytes_with_provider(reader, provider)?),
            async_operations: AtomicU32::new(u32::from_bytes_with_provider(reader, provider)?),
        })
    }
}

/// Borrow information
#[derive(Debug, Clone)]
struct BorrowInfo {
    handle: ResourceHandle,
    borrow_type: ResourceBorrowType,
    borrowed_at: u64,
    borrowed_from: ComponentInstanceId,
}

impl Default for BorrowInfo {
    fn default() -> Self {
        Self {
            handle: ResourceHandle::new(0),
            borrow_type: ResourceBorrowType::Shared,
            borrowed_at: 0,
            borrowed_from: ComponentInstanceId::new(0),
        }
    }
}

impl PartialEq for BorrowInfo {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Eq for BorrowInfo {}

impl Checksummable for BorrowInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.handle.update_checksum(checksum);
        self.borrowed_at.update_checksum(checksum);
    }
}

impl ToBytes for BorrowInfo {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)?;
        self.borrowed_at.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for BorrowInfo {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

/// Resource limits per component
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceLimits {
    max_owned_resources: usize,
    max_borrowed_resources: usize,
    max_concurrent_operations: usize,
    resource_memory_limit: usize,
}

impl Checksummable for ResourceLimits {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.max_owned_resources.update_checksum(checksum);
        self.max_borrowed_resources.update_checksum(checksum);
        self.max_concurrent_operations.update_checksum(checksum);
        self.resource_memory_limit.update_checksum(checksum);
    }
}

impl ToBytes for ResourceLimits {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.max_owned_resources.to_bytes_with_provider(writer, provider)?;
        self.max_borrowed_resources.to_bytes_with_provider(writer, provider)?;
        self.max_concurrent_operations.to_bytes_with_provider(writer, provider)?;
        self.resource_memory_limit.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ResourceLimits {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            max_owned_resources: usize::from_bytes_with_provider(reader, provider)?,
            max_borrowed_resources: usize::from_bytes_with_provider(reader, provider)?,
            max_concurrent_operations: usize::from_bytes_with_provider(reader, provider)?,
            resource_memory_limit: usize::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Async resource configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncResourceConfig {
    enable_async_creation: bool,
    enable_async_methods: bool,
    enable_async_drop: bool,
    default_timeout_ms: u64,
    max_operation_fuel: u64,
}

impl Checksummable for AsyncResourceConfig {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.enable_async_creation.update_checksum(checksum);
        self.enable_async_methods.update_checksum(checksum);
        self.enable_async_drop.update_checksum(checksum);
        self.default_timeout_ms.update_checksum(checksum);
        self.max_operation_fuel.update_checksum(checksum);
    }
}

impl ToBytes for AsyncResourceConfig {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.enable_async_creation.to_bytes_with_provider(writer, provider)?;
        self.enable_async_methods.to_bytes_with_provider(writer, provider)?;
        self.enable_async_drop.to_bytes_with_provider(writer, provider)?;
        self.default_timeout_ms.to_bytes_with_provider(writer, provider)?;
        self.max_operation_fuel.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for AsyncResourceConfig {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            enable_async_creation: bool::from_bytes_with_provider(reader, provider)?,
            enable_async_methods: bool::from_bytes_with_provider(reader, provider)?,
            enable_async_drop: bool::from_bytes_with_provider(reader, provider)?,
            default_timeout_ms: u64::from_bytes_with_provider(reader, provider)?,
            max_operation_fuel: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

impl Default for AsyncResourceConfig {
    fn default() -> Self {
        Self {
            enable_async_creation: true,
            enable_async_methods: true,
            enable_async_drop: true,
            default_timeout_ms: 5000,
            max_operation_fuel: 10000,
        }
    }
}

/// Resource operation statistics
#[derive(Debug, Default)]
struct ResourceOperationStats {
    total_creates: AtomicU64,
    completed_creates: AtomicU64,
    total_method_calls: AtomicU64,
    completed_method_calls: AtomicU64,
    total_borrows: AtomicU64,
    completed_borrows: AtomicU64,
    total_drops: AtomicU64,
    completed_drops: AtomicU64,
    failed_operations: AtomicU64,
    total_fuel_consumed: AtomicU64,
}

/// Resource completion callback
type ResourceCompletionCallback = Box<dyn FnOnce(Result<ComponentValue<ComponentProvider>>) + Send>;

impl ResourceAsyncOperations {
    /// Create new resource async operations manager
    pub fn new(abi_support: AsyncCanonicalAbiSupport) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        Ok(Self {
            abi_support,
            lifecycle_manager: (),
            active_operations: BoundedMap::new(),
            resource_contexts: BoundedMap::new(),
            next_operation_id: AtomicU64::new(1),
            resource_stats: ResourceOperationStats::default(),
        })
    }

    /// Initialize component for async resource operations
    pub fn initialize_component_resources(
        &mut self,
        component_id: ComponentInstanceId,
        limits: Option<ResourceLimits>,
        config: Option<AsyncResourceConfig>,
    ) -> Result<()> {
        let limits = limits.unwrap_or(ResourceLimits {
            max_owned_resources: 64,
            max_borrowed_resources: 32,
            max_concurrent_operations: 16,
            resource_memory_limit: 1024 * 1024, // 1MB
        });

        let config = config.unwrap_or(AsyncResourceConfig {
            enable_async_creation: true,
            enable_async_methods: true,
            enable_async_drop: true,
            default_timeout_ms: 5000,
            max_operation_fuel: 10_000,
        });

        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        let context = ComponentResourceContext {
            component_id,
            owned_resources: BoundedMap::new(),
            borrowed_resources: BoundedMap::new(),
            active_operations: BoundedVec::new().unwrap(),
            resource_limits: limits,
            async_config: config,
        };

        self.resource_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component resource contexts"))?;

        Ok(())
    }

    /// Create resource asynchronously
    pub fn async_create_resource(
        &mut self,
        component_id: ComponentInstanceId,
        resource_type: ResourceType,
        constructor_args: Vec<ComponentValue<ComponentProvider>>,
        callback: Option<ResourceCompletionCallback>,
    ) -> Result<ResourceOperationId> {
        // Extract timestamp and operation_id before mutable borrows
        let timestamp = self.get_timestamp();
        let operation_id =
            ResourceOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let context = self.resource_contexts.get_mut(&component_id).ok_or_else(|| {
            Error::validation_invalid_input("Component not initialized for async resources")
        })?;

        if !context.async_config.enable_async_creation {
            return Err(Error::runtime_execution_error(
                "Async resource creation not enabled",
            ));
        }

        // Check limits
        if context.owned_resources.len() >= context.resource_limits.max_owned_resources {
            return Err(Error::resource_limit_exceeded("Too many owned resources"));
        }

        // Create async operation
        let operation = ResourceAsyncOperation {
            id: operation_id,
            component_id,
            operation_type: ResourceAsyncOperationType::Create {
                constructor_args: constructor_args.clone(),
            },
            resource_handle: None,
            resource_type,
            abi_operation_id: None,
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
            completion_callback: callback,
        };

        // Delegate to ABI support for actual creation
        let abi_op_id = self.abi_support.async_resource_call(
            component_id,
            ResourceHandle::new(0), // Temporary handle
            String::from("constructor"),
            constructor_args,
        )?;

        let mut stored_operation = operation;
        stored_operation.abi_operation_id = Some(abi_op_id);

        // Store operation
        self.active_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many active resource operations"))?;

        // Add to component context
        context
            .active_operations
            .push(operation_id)
            .map_err(|_| Error::resource_limit_exceeded("Component operation list full"))?;

        // Update statistics
        self.resource_stats.total_creates.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Call async method on resource
    pub fn async_call_resource_method(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
        method_name: String,
        args: Vec<ComponentValue<ComponentProvider>>,
        callback: Option<ResourceCompletionCallback>,
    ) -> Result<ResourceOperationId> {
        // Extract timestamp and operation_id before mutable borrows
        let timestamp = self.get_timestamp();
        let operation_id =
            ResourceOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let context = self
            .resource_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

        if !context.async_config.enable_async_methods {
            return Err(Error::runtime_execution_error(
                "Async resource methods not enabled",
            ));
        }

        // Verify resource ownership or borrow and extract resource_type
        let resource_type =
            if let Some(resource_info) = context.owned_resources.get(&resource_handle) {
                if resource_info.state != ResourceState::Active
                    && resource_info.state != ResourceState::Borrowed
                {
                    return Err(Error::validation_invalid_state(
                        "Resource not in valid state for method calls",
                    ));
                }
                resource_info.resource_type
            } else if context.borrowed_resources.contains_key(&resource_handle) {
                0 // Borrowed resource type - placeholder
            } else {
                return Err(Error::validation_invalid_input(
                    "Resource not owned or borrowed by component",
                ));
            };

        let operation = ResourceAsyncOperation {
            id: operation_id,
            component_id,
            operation_type: ResourceAsyncOperationType::MethodCall {
                method_name: method_name.clone(),
                args: args.clone(),
            },
            resource_handle: Some(resource_handle),
            resource_type,
            abi_operation_id: None,
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
            completion_callback: callback,
        };

        // Delegate to ABI support
        let abi_op_id = self.abi_support.async_resource_call(
            component_id,
            resource_handle,
            method_name,
            args,
        )?;

        let mut stored_operation = operation;
        stored_operation.abi_operation_id = Some(abi_op_id);

        self.active_operations
            .insert(operation_id, stored_operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many active operations"))?;

        context
            .active_operations
            .push(operation_id)
            .map_err(|_| Error::resource_limit_exceeded("Component operation list full"))?;

        // Update resource activity
        if let Some(resource_info) = context.owned_resources.get_mut(&resource_handle) {
            resource_info.last_accessed.store(timestamp, Ordering::Release);
            resource_info.async_operations.fetch_add(1, Ordering::AcqRel);
        }

        self.resource_stats.total_method_calls.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Borrow resource for async operation
    pub fn async_borrow_resource(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
        borrow_type: ResourceBorrowType,
    ) -> Result<ResourceOperationId> {
        // Extract timestamp before mutable borrow to avoid borrow conflict
        let timestamp = self.get_timestamp();
        let operation_id =
            ResourceOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let context = self
            .resource_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

        // Check borrow limits
        if context.borrowed_resources.len() >= context.resource_limits.max_borrowed_resources {
            return Err(Error::resource_limit_exceeded(
                "Component borrow limit exceeded",
            ));
        }

        let operation = ResourceAsyncOperation {
            id: operation_id,
            component_id,
            operation_type: ResourceAsyncOperationType::Borrow {
                borrow_type: borrow_type.clone(),
            },
            resource_handle: Some(resource_handle),
            resource_type: 0, // Unknown resource type - would be resolved
            abi_operation_id: None,
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
            completion_callback: None,
        };

        // Perform borrow operation
        // In real implementation, would coordinate with resource manager
        let borrow_info = BorrowInfo {
            handle: resource_handle,
            borrow_type,
            borrowed_at: timestamp,
            borrowed_from: component_id, // Would be actual owner
        };

        context
            .borrowed_resources
            .insert(resource_handle, borrow_info)
            .map_err(|_| Error::resource_limit_exceeded("Borrow table full"))?;

        self.active_operations
            .insert(operation_id, operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many operations"))?;

        self.resource_stats.total_borrows.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Drop resource asynchronously
    pub fn async_drop_resource(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
        callback: Option<ResourceCompletionCallback>,
    ) -> Result<ResourceOperationId> {
        // Extract timestamp first to avoid borrow conflict
        let timestamp = self.get_timestamp();
        let operation_id =
            ResourceOperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));

        let context = self
            .resource_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not initialized"))?;

        if !context.async_config.enable_async_drop {
            return Err(Error::runtime_execution_error(
                "Async resource drop not enabled",
            ));
        }

        // Verify ownership
        let resource_info = context
            .owned_resources
            .get_mut(&resource_handle)
            .ok_or_else(|| Error::validation_invalid_input("Resource not found"))?;

        if resource_info.state == ResourceState::Dropped {
            return Err(Error::validation_invalid_state("Resource already dropped"));
        }

        // Extract resource type before creating operation
        let resource_type = resource_info.resource_type;

        let operation = ResourceAsyncOperation {
            id: operation_id,
            component_id,
            operation_type: ResourceAsyncOperationType::Drop,
            resource_handle: Some(resource_handle),
            resource_type,
            abi_operation_id: None,
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
            completion_callback: callback,
        };

        // Mark resource as finalizing
        resource_info.state = ResourceState::Finalizing;

        self.active_operations
            .insert(operation_id, operation)
            .map_err(|_| Error::resource_limit_exceeded("Too many operations"))?;

        self.resource_stats.total_drops.fetch_add(1, Ordering::Relaxed);

        Ok(operation_id)
    }

    /// Poll all async resource operations
    pub fn poll_resource_operations(&mut self) -> Result<ResourcePollResult> {
        // Poll underlying ABI operations
        let abi_result = self.abi_support.poll_async_operations()?;

        let mut completed_operations: Vec<ResourceOperationId> = Vec::new();
        let mut failed_operations: Vec<ResourceOperationId> = Vec::new();

        // Check operation statuses
        for (op_id, operation) in self.active_operations.iter() {
            if let Some(abi_op_id) = operation.abi_operation_id {
                let abi_status = self.abi_support.check_operation_status(abi_op_id)?;

                if abi_status.is_ready {
                    // Operation completed
                    completed_operations.push(*op_id);

                    // Update statistics based on operation type
                    match &operation.operation_type {
                        ResourceAsyncOperationType::Create { .. } => {
                            self.resource_stats.completed_creates.fetch_add(1, Ordering::Relaxed);
                        },
                        ResourceAsyncOperationType::MethodCall { .. } => {
                            self.resource_stats
                                .completed_method_calls
                                .fetch_add(1, Ordering::Relaxed);
                        },
                        ResourceAsyncOperationType::Borrow { .. } => {
                            self.resource_stats.completed_borrows.fetch_add(1, Ordering::Relaxed);
                        },
                        ResourceAsyncOperationType::Drop => {
                            self.resource_stats.completed_drops.fetch_add(1, Ordering::Relaxed);
                        },
                        _ => {},
                    }
                }
            }
        }

        // Clean up completed operations
        for op_id in &completed_operations {
            self.cleanup_resource_operation(*op_id)?;
        }

        Ok(ResourcePollResult {
            completed_operations: completed_operations.len(),
            failed_operations: failed_operations.len(),
            total_fuel_consumed: abi_result.total_fuel_consumed,
            active_operations: self.active_operations.len(),
        })
    }

    /// Get resource operation statistics
    pub fn get_resource_statistics(&self) -> ResourceStats {
        ResourceStats {
            total_creates: self.resource_stats.total_creates.load(Ordering::Relaxed),
            completed_creates: self.resource_stats.completed_creates.load(Ordering::Relaxed),
            total_method_calls: self.resource_stats.total_method_calls.load(Ordering::Relaxed),
            completed_method_calls: self
                .resource_stats
                .completed_method_calls
                .load(Ordering::Relaxed),
            total_borrows: self.resource_stats.total_borrows.load(Ordering::Relaxed),
            completed_borrows: self.resource_stats.completed_borrows.load(Ordering::Relaxed),
            total_drops: self.resource_stats.total_drops.load(Ordering::Relaxed),
            completed_drops: self.resource_stats.completed_drops.load(Ordering::Relaxed),
            failed_operations: self.resource_stats.failed_operations.load(Ordering::Relaxed),
            active_operations: self.active_operations.len() as u64,
            active_components: self.resource_contexts.len() as u64,
        }
    }

    // Private helper methods

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }

    fn cleanup_resource_operation(&mut self, operation_id: ResourceOperationId) -> Result<()> {
        if let Some(operation) = self.active_operations.remove(&operation_id) {
            // Remove from component context
            if let Some(context) = self.resource_contexts.get_mut(&operation.component_id) {
                context.active_operations.retain(|&id| id != operation_id);
            }

            // Handle operation completion
            match operation.operation_type {
                ResourceAsyncOperationType::Create { .. } => {
                    // Resource creation completed - would add to
                    // owned_resources
                },
                ResourceAsyncOperationType::Drop => {
                    // Resource drop completed - remove from owned_resources
                    if let Some(handle) = operation.resource_handle {
                        if let Some(context) =
                            self.resource_contexts.get_mut(&operation.component_id)
                        {
                            context.owned_resources.remove(&handle);
                        }
                    }
                },
                _ => {},
            }

            // Call completion callback if provided
            if let Some(callback) = operation.completion_callback {
                callback(Ok(ComponentValue::U32(0))); // Success result
            }
        }
        Ok(())
    }
}

/// Resource poll result
#[derive(Debug, Clone)]
pub struct ResourcePollResult {
    pub completed_operations: usize,
    pub failed_operations: usize,
    pub total_fuel_consumed: u64,
    pub active_operations: usize,
}

/// Resource operation statistics
#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub total_creates: u64,
    pub completed_creates: u64,
    pub total_method_calls: u64,
    pub completed_method_calls: u64,
    pub total_borrows: u64,
    pub completed_borrows: u64,
    pub total_drops: u64,
    pub completed_drops: u64,
    pub failed_operations: u64,
    pub active_operations: u64,
    pub active_components: u64,
}
