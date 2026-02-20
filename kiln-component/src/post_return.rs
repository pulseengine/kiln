//! Post-return cleanup functions for WebAssembly Component Model
//!
//! This module implements the post-return cleanup mechanism that allows
//! components to perform cleanup after function calls, particularly for
//! managing resources, memory allocations, and async operations.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{fmt, mem};

#[cfg(feature = "std")]
use std::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};

use wrt_foundation::{
    ToString, bounded::BoundedString, budget_aware_provider::CrateId, prelude::*,
    safe_managed_alloc, safe_memory::NoStdProvider,
};

// For no_std, override prelude's bounded::BoundedVec with StaticVec
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticVec as BoundedVec;

use crate::{
    async_::async_types::{ErrorContextHandle, FutureHandle, StreamHandle},
    bounded_component_infra::ComponentProvider,
    canonical_abi::canonical_realloc::ReallocManager,
    handle_representation::HandleRepresentationManager,
    types::{ComponentError, ComponentInstanceId, ResourceId, TaskId, TypeId, Value},
};

// Import threading types with feature gate
#[cfg(feature = "component-model-threading")]
use crate::threading::task_cancellation::{
    CancellationToken, SubtaskManager, SubtaskResult, SubtaskState,
};

// Import resource lifecycle types
use crate::resources::resource_lifecycle::ResourceLifecycleManager;

// ComponentId is the same as ComponentInstanceId in this codebase
pub type ComponentId = ComponentInstanceId;

// Import async types - they exist in the async_ module
use crate::async_::{
    async_canonical::AsyncCanonicalAbi,
    async_execution_engine::{AsyncExecutionEngine, ExecutionId},
};

// Import ComponentValue from foundation
use wrt_foundation::component_value::ComponentValue;

use wrt_error::{Error, ErrorCategory, Result};

// Stub types when threading is not enabled
#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct CancellationToken;

#[cfg(not(feature = "component-model-threading"))]
impl CancellationToken {
    pub fn cancel(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug)]
pub struct SubtaskManager;

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum SubtaskResult {
    #[default]
    Success,
    Failure,
}

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubtaskState {
    Running,
    Completed,
    Failed,
}

// Stub types for borrowed handles (module not available)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct LifetimeScope(pub u32);

#[derive(Debug)]
pub struct HandleLifetimeTracker;

/// Post-return function signature: () -> ()
pub type PostReturnFn = fn();

/// Callback for invoking WebAssembly cleanup functions
///
/// The callback receives:
/// - `instance_id`: The component instance containing the function
/// - `func_idx`: The function index to call
/// - `args`: Arguments to pass to the function (as i32 values)
///
/// Returns: Ok(result) on success, where result is the optional i32 return value
#[cfg(feature = "std")]
pub type CleanupCallback =
    Box<dyn FnMut(u32, u32, &[i32]) -> wrt_error::Result<Option<i32>> + Send>;

/// Callback for invoking cabi_realloc for memory deallocation
///
/// The callback receives:
/// - `instance_id`: The component instance containing cabi_realloc
/// - `old_ptr`: The pointer to free
/// - `old_size`: The size of the allocation
/// - `align`: Alignment (typically 1 for free operations)
/// - `new_size`: New size (0 for free operations)
///
/// Returns: Ok(new_ptr) on success
#[cfg(feature = "std")]
pub type ReallocCleanupCallback =
    Box<dyn FnMut(u32, i32, i32, i32, i32) -> wrt_error::Result<i32> + Send>;

/// Maximum number of cleanup tasks per instance in no_std
const MAX_CLEANUP_TASKS_NO_STD: usize = 256;

/// Maximum cleanup handlers per type
const MAX_CLEANUP_HANDLERS: usize = 64;

/// Post-return cleanup registry with async support
///
/// The registry manages cleanup operations for component instances, including:
/// - Memory deallocation via cabi_realloc
/// - Resource destructor invocation
/// - Post-return function calls
///
/// All actual WebAssembly function invocations go through registered callbacks,
/// allowing the runtime engine to integrate with the cleanup system.
pub struct PostReturnRegistry {
    /// Registered post-return functions per instance
    #[cfg(feature = "std")]
    functions: BTreeMap<ComponentInstanceId, PostReturnFunction>,
    #[cfg(not(any(feature = "std",)))]
    functions: BoundedVec<(ComponentInstanceId, PostReturnFunction), MAX_CLEANUP_TASKS_NO_STD>,

    /// Cleanup tasks waiting to be executed
    #[cfg(feature = "std")]
    pending_cleanups: BTreeMap<ComponentInstanceId, Vec<CleanupTask>>,
    #[cfg(not(any(feature = "std",)))]
    pending_cleanups: BoundedVec<
        (
            ComponentInstanceId,
            BoundedVec<CleanupTask, MAX_CLEANUP_TASKS_NO_STD>,
        ),
        MAX_CLEANUP_TASKS_NO_STD,
    >,

    /// Async execution engine for async cleanup
    async_engine: Option<Arc<AsyncExecutionEngine>>,

    /// Task cancellation manager
    cancellation_manager: Option<Arc<SubtaskManager>>,

    /// Handle lifetime tracker for resource cleanup
    handle_tracker: Option<Arc<HandleLifetimeTracker>>,

    /// Resource lifecycle manager
    resource_manager: Option<Arc<ResourceLifecycleManager>>,

    /// Resource representation manager
    representation_manager: Option<Arc<HandleRepresentationManager>>,

    /// Execution metrics
    metrics: PostReturnMetrics,

    /// Maximum cleanup tasks per instance
    max_cleanup_tasks: usize,

    /// Callback for invoking WebAssembly functions during cleanup
    #[cfg(feature = "std")]
    cleanup_callback: Option<CleanupCallback>,

    /// Callback for invoking cabi_realloc for memory deallocation
    #[cfg(feature = "std")]
    realloc_callback: Option<ReallocCleanupCallback>,
}

impl core::fmt::Debug for PostReturnRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = f.debug_struct("PostReturnRegistry");
        s.field("functions", &self.functions)
            .field("pending_cleanups", &"<pending>")
            .field("metrics", &self.metrics)
            .field("max_cleanup_tasks", &self.max_cleanup_tasks);
        #[cfg(feature = "std")]
        {
            s.field(
                "cleanup_callback",
                &self.cleanup_callback.as_ref().map(|_| "<callback>"),
            );
            s.field(
                "realloc_callback",
                &self.realloc_callback.as_ref().map(|_| "<callback>"),
            );
        }
        s.finish()
    }
}

#[derive(Clone, Default)]
struct PostReturnFunction {
    /// Function index in the component
    func_index: u32,
    /// Cached function reference for performance
    #[cfg(feature = "std")]
    func_ref: Option<Arc<dyn Fn() -> Result<()> + Send + Sync>>,
    /// Whether the function is currently being executed
    executing: bool,
    /// Associated cancellation token for cleanup
    cancellation_token: Option<CancellationToken>,
}

impl core::fmt::Debug for PostReturnFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PostReturnFunction")
            .field("func_index", &self.func_index)
            .field("func_ref", &"<function>")
            .field("executing", &self.executing)
            .field("cancellation_token", &self.cancellation_token)
            .finish()
    }
}

impl PartialEq for PostReturnFunction {
    fn eq(&self, other: &Self) -> bool {
        self.func_index == other.func_index
            && self.executing == other.executing
            && self.cancellation_token == other.cancellation_token
        // Note: func_ref is not compared as function pointers can't be compared
    }
}

impl Eq for PostReturnFunction {}

impl wrt_foundation::traits::Checksummable for PostReturnFunction {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.func_index.update_checksum(checksum);
        self.executing.update_checksum(checksum);
        // Note: func_ref is not checksummed as it's a function pointer
    }
}

impl wrt_foundation::traits::ToBytes for PostReturnFunction {
    fn serialized_size(&self) -> usize {
        5 // 4 bytes for func_index + 1 byte for executing
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u32_le(self.func_index)?;
        writer.write_u8(self.executing as u8)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for PostReturnFunction {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let func_index = reader.read_u32_le()?;
        let executing = reader.read_u8()? != 0;
        Ok(Self {
            func_index,
            #[cfg(feature = "std")]
            func_ref: None,
            executing,
            cancellation_token: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupTask {
    /// Type of cleanup task
    pub task_type: CleanupTaskType,
    /// Instance that created this task
    pub source_instance: ComponentInstanceId,
    /// Priority of the task (higher = execute first)
    pub priority: u8,
    /// Task-specific data
    pub data: CleanupData,
}

impl wrt_foundation::traits::Checksummable for CleanupTask {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.task_type.update_checksum(checksum);
        self.source_instance.0.update_checksum(checksum);
        self.priority.update_checksum(checksum);
        self.data.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for CleanupTask {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.task_type.to_bytes_with_provider(writer, provider)?;
        self.source_instance.0.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        self.data.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for CleanupTask {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum CleanupTaskType {
    /// Binary std/no_std choice
    DeallocateMemory,
    /// Close resource handle
    CloseResource,
    /// Release reference
    ReleaseReference,
    /// Custom cleanup
    #[default]
    Custom,
    /// Async cleanup (for streams/futures)
    AsyncCleanup,
    /// Cancel async execution
    CancelAsyncExecution,
    /// Drop borrowed handles
    DropBorrowedHandles,
    /// End lifetime scope
    EndLifetimeScope,
    /// Release resource representation
    ReleaseResourceRepresentation,
    /// Finalize subtask
    FinalizeSubtask,
}

impl wrt_foundation::traits::Checksummable for CleanupTaskType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::DeallocateMemory => 0u8.update_checksum(checksum),
            Self::CloseResource => 1u8.update_checksum(checksum),
            Self::ReleaseReference => 2u8.update_checksum(checksum),
            Self::Custom => 3u8.update_checksum(checksum),
            Self::AsyncCleanup => 4u8.update_checksum(checksum),
            Self::CancelAsyncExecution => 5u8.update_checksum(checksum),
            Self::DropBorrowedHandles => 6u8.update_checksum(checksum),
            Self::EndLifetimeScope => 7u8.update_checksum(checksum),
            Self::ReleaseResourceRepresentation => 8u8.update_checksum(checksum),
            Self::FinalizeSubtask => 9u8.update_checksum(checksum),
        }
    }
}

impl wrt_runtime::ToBytes for CleanupTaskType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::DeallocateMemory => 0u8.to_bytes_with_provider(writer, provider),
            Self::CloseResource => 1u8.to_bytes_with_provider(writer, provider),
            Self::ReleaseReference => 2u8.to_bytes_with_provider(writer, provider),
            Self::Custom => 3u8.to_bytes_with_provider(writer, provider),
            Self::AsyncCleanup => 4u8.to_bytes_with_provider(writer, provider),
            Self::CancelAsyncExecution => 5u8.to_bytes_with_provider(writer, provider),
            Self::DropBorrowedHandles => 6u8.to_bytes_with_provider(writer, provider),
            Self::EndLifetimeScope => 7u8.to_bytes_with_provider(writer, provider),
            Self::ReleaseResourceRepresentation => 8u8.to_bytes_with_provider(writer, provider),
            Self::FinalizeSubtask => 9u8.to_bytes_with_provider(writer, provider),
        }
    }
}

impl wrt_foundation::traits::FromBytes for CleanupTaskType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let tag = u8::from_bytes_with_provider(reader, provider)?;
        Ok(match tag {
            0 => Self::DeallocateMemory,
            1 => Self::CloseResource,
            2 => Self::ReleaseReference,
            3 => Self::Custom,
            4 => Self::AsyncCleanup,
            5 => Self::CancelAsyncExecution,
            6 => Self::DropBorrowedHandles,
            7 => Self::EndLifetimeScope,
            8 => Self::ReleaseResourceRepresentation,
            9 => Self::FinalizeSubtask,
            _ => Self::Custom,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupData {
    /// Binary std/no_std choice
    Memory { ptr: i32, size: i32, align: i32 },
    /// Resource cleanup data
    Resource { handle: u32, resource_type: TypeId },
    /// Reference cleanup data
    Reference { ref_id: u32, ref_count: u32 },
    /// Custom cleanup data
    #[cfg(feature = "std")]
    Custom {
        cleanup_id: String,
        parameters: Vec<ComponentValue<ComponentProvider>>,
    },
    #[cfg(not(any(feature = "std",)))]
    Custom {
        cleanup_id: BoundedString<64>,
        parameters: BoundedVec<ComponentValue<NoStdProvider<2048>>, 16>,
    },
    /// Async cleanup data
    Async {
        stream_handle: Option<StreamHandle>,
        future_handle: Option<FutureHandle>,
        error_context_handle: Option<ErrorContextHandle>,
        task_id: Option<TaskId>,
        execution_id: Option<ExecutionId>,
        cancellation_token: Option<CancellationToken>,
    },
    /// Async execution cancellation
    AsyncExecution {
        execution_id: ExecutionId,
        force_cancel: bool,
    },
    /// Borrowed handle cleanup
    BorrowedHandle {
        borrow_handle: u32,
        lifetime_scope: LifetimeScope,
        source_component: ComponentId,
    },
    /// Lifetime scope cleanup
    LifetimeScope {
        scope: LifetimeScope,
        component: ComponentId,
        task: TaskId,
    },
    /// Resource representation cleanup
    ResourceRepresentation {
        handle: u32,
        resource_id: ResourceId,
        component: ComponentId,
    },
    /// Subtask finalization
    Subtask {
        execution_id: ExecutionId,
        task_id: TaskId,
        result: Option<SubtaskResult>,
        force_cleanup: bool,
    },
}

impl Default for CleanupData {
    fn default() -> Self {
        CleanupData::Memory {
            ptr: 0,
            size: 0,
            align: 1,
        }
    }
}

impl wrt_foundation::traits::Checksummable for CleanupData {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::Memory { ptr, size, align } => {
                0u8.update_checksum(checksum);
                ptr.update_checksum(checksum);
                size.update_checksum(checksum);
                align.update_checksum(checksum);
            },
            Self::Resource {
                handle,
                resource_type,
            } => {
                1u8.update_checksum(checksum);
                handle.update_checksum(checksum);
                resource_type.0.update_checksum(checksum);
            },
            Self::Reference { ref_id, ref_count } => {
                2u8.update_checksum(checksum);
                ref_id.update_checksum(checksum);
                ref_count.update_checksum(checksum);
            },
            Self::Custom {
                cleanup_id,
                parameters: _,
            } => {
                3u8.update_checksum(checksum);
                #[cfg(feature = "std")]
                {
                    let bytes = cleanup_id.as_bytes();
                    bytes.update_checksum(checksum);
                }
                #[cfg(not(feature = "std"))]
                {
                    // SafeSlice doesn't implement update_checksum, so we compute
                    // checksum based on the string length as a simple alternative
                    cleanup_id.len().update_checksum(checksum);
                }
            },
            Self::Async {
                stream_handle,
                future_handle,
                error_context_handle,
                task_id,
                execution_id,
                cancellation_token: _,
            } => {
                4u8.update_checksum(checksum);
                stream_handle.map(|h| h.0).unwrap_or(0).update_checksum(checksum);
                future_handle.map(|h| h.0).unwrap_or(0).update_checksum(checksum);
                error_context_handle.map(|h| h.0).unwrap_or(0).update_checksum(checksum);
                task_id.map(|t| t.0).unwrap_or(0).update_checksum(checksum);
                execution_id.map(|e| e.0).unwrap_or(0).update_checksum(checksum);
            },
            Self::AsyncExecution {
                execution_id,
                force_cancel,
            } => {
                5u8.update_checksum(checksum);
                execution_id.0.update_checksum(checksum);
                force_cancel.update_checksum(checksum);
            },
            Self::BorrowedHandle {
                borrow_handle,
                lifetime_scope,
                source_component,
            } => {
                6u8.update_checksum(checksum);
                borrow_handle.update_checksum(checksum);
                lifetime_scope.0.update_checksum(checksum);
                source_component.0.update_checksum(checksum);
            },
            Self::LifetimeScope {
                scope,
                component,
                task,
            } => {
                7u8.update_checksum(checksum);
                scope.0.update_checksum(checksum);
                component.0.update_checksum(checksum);
                task.0.update_checksum(checksum);
            },
            Self::ResourceRepresentation {
                handle,
                resource_id,
                component,
            } => {
                8u8.update_checksum(checksum);
                handle.update_checksum(checksum);
                resource_id.0.update_checksum(checksum);
                component.0.update_checksum(checksum);
            },
            Self::Subtask {
                execution_id,
                task_id,
                result: _,
                force_cleanup,
            } => {
                9u8.update_checksum(checksum);
                execution_id.0.update_checksum(checksum);
                task_id.0.update_checksum(checksum);
                force_cleanup.update_checksum(checksum);
            },
        }
    }
}

impl wrt_runtime::ToBytes for CleanupData {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::Memory { ptr, size, align } => {
                0u8.to_bytes_with_provider(writer, provider)?;
                ptr.to_bytes_with_provider(writer, provider)?;
                size.to_bytes_with_provider(writer, provider)?;
                align.to_bytes_with_provider(writer, provider)
            },
            Self::Resource {
                handle,
                resource_type,
            } => {
                1u8.to_bytes_with_provider(writer, provider)?;
                handle.to_bytes_with_provider(writer, provider)?;
                resource_type.0.to_bytes_with_provider(writer, provider)
            },
            Self::Reference { ref_id, ref_count } => {
                2u8.to_bytes_with_provider(writer, provider)?;
                ref_id.to_bytes_with_provider(writer, provider)?;
                ref_count.to_bytes_with_provider(writer, provider)
            },
            Self::Custom {
                cleanup_id,
                parameters: _,
            } => {
                3u8.to_bytes_with_provider(writer, provider)?;
                #[cfg(feature = "std")]
                {
                    (cleanup_id.len() as u32).to_bytes_with_provider(writer, provider)?;
                    cleanup_id.as_bytes().to_bytes_with_provider(writer, provider)
                }
                #[cfg(not(feature = "std"))]
                {
                    (cleanup_id.len() as u32).to_bytes_with_provider(writer, provider)?;
                    cleanup_id.as_bytes()?.as_ref().to_bytes_with_provider(writer, provider)
                }
            },
            _ => {
                // For complex variants, write a default tag
                255u8.to_bytes_with_provider(writer, provider)
            },
        }
    }
}

impl wrt_foundation::traits::FromBytes for CleanupData {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

#[derive(Debug, Default, Clone)]
pub struct PostReturnMetrics {
    /// Total post-return functions executed
    pub total_executions: u64,
    /// Total cleanup tasks processed
    pub total_cleanup_tasks: u64,
    /// Failed cleanup attempts
    pub failed_cleanups: u64,
    /// Average cleanup time (microseconds)
    pub avg_cleanup_time_us: u64,
    /// Peak pending cleanup tasks
    pub peak_pending_tasks: usize,
    /// Async cleanup operations
    pub async_cleanups: u64,
    /// Resource cleanups
    pub resource_cleanups: u64,
    /// Handle cleanups
    pub handle_cleanups: u64,
    /// Cancellation cleanups
    pub cancellation_cleanups: u64,
}

/// Context for post-return execution with async support
pub struct PostReturnContext {
    /// Instance being cleaned up
    pub instance_id: ComponentInstanceId,
    /// Cleanup tasks to execute
    #[cfg(feature = "std")]
    pub tasks: Vec<CleanupTask>,
    #[cfg(not(any(feature = "std",)))]
    pub tasks: BoundedVec<CleanupTask, MAX_CLEANUP_TASKS_NO_STD>,
    /// Binary std/no_std choice
    pub realloc_manager: Option<Arc<ReallocManager>>,
    /// Custom cleanup handlers
    #[cfg(feature = "std")]
    pub custom_handlers: BTreeMap<String, Box<dyn Fn(&CleanupData) -> Result<()> + Send + Sync>>,
    #[cfg(not(any(feature = "std",)))]
    pub custom_handlers: BoundedVec<
        (
            BoundedString<64>,
            fn(&CleanupData) -> core::result::Result<(), wrt_error::Error>,
        ),
        MAX_CLEANUP_HANDLERS,
    >,
    /// Async canonical ABI for async cleanup
    pub async_abi: Option<Arc<AsyncCanonicalAbi>>,
    /// Component ID for this context
    pub component_id: ComponentId,
    /// Current task ID
    pub task_id: TaskId,
}

impl PostReturnRegistry {
    pub fn new(max_cleanup_tasks: usize) -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            functions: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            functions: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|| {
                    Error::resource_exhausted(
                        "Failed to create bounded vector for post-return functions",
                    )
                })?
            },
            #[cfg(feature = "std")]
            pending_cleanups: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            pending_cleanups: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|| {
                    Error::resource_exhausted(
                        "Failed to create bounded vector for pending cleanups",
                    )
                })?
            },
            async_engine: None,
            cancellation_manager: None,
            handle_tracker: None,
            resource_manager: None,
            representation_manager: None,
            metrics: PostReturnMetrics::default(),
            max_cleanup_tasks,
            #[cfg(feature = "std")]
            cleanup_callback: None,
            #[cfg(feature = "std")]
            realloc_callback: None,
        })
    }

    /// Set the callback for invoking WebAssembly functions during cleanup
    ///
    /// The callback should:
    /// 1. Find the function in the component instance
    /// 2. Call it with the provided arguments
    /// 3. Return the result (if any)
    #[cfg(feature = "std")]
    pub fn set_cleanup_callback(&mut self, callback: CleanupCallback) {
        self.cleanup_callback = Some(callback);
    }

    /// Set the callback for invoking cabi_realloc for memory deallocation
    ///
    /// The callback should call the component's cabi_realloc export with:
    /// - old_ptr: The pointer to free
    /// - old_size: The size of the allocation
    /// - align: Alignment (typically 1)
    /// - new_size: 0 (to indicate free operation)
    #[cfg(feature = "std")]
    pub fn set_realloc_callback(&mut self, callback: ReallocCleanupCallback) {
        self.realloc_callback = Some(callback);
    }

    /// Check if cleanup callbacks are registered
    #[cfg(feature = "std")]
    pub fn has_callbacks(&self) -> bool {
        self.cleanup_callback.is_some() || self.realloc_callback.is_some()
    }

    /// Create new registry with async support
    pub fn new_with_async(
        max_cleanup_tasks: usize,
        async_engine: Option<Arc<AsyncExecutionEngine>>,
        cancellation_manager: Option<Arc<SubtaskManager>>,
        handle_tracker: Option<Arc<HandleLifetimeTracker>>,
        resource_manager: Option<Arc<ResourceLifecycleManager>>,
        representation_manager: Option<Arc<HandleRepresentationManager>>,
    ) -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            functions: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            functions: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|| {
                    Error::resource_exhausted(
                        "Failed to create bounded vector for async post-return functions",
                    )
                })?
            },
            #[cfg(feature = "std")]
            pending_cleanups: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            pending_cleanups: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|| {
                    Error::resource_exhausted(
                        "Failed to create bounded vector for async pending cleanups",
                    )
                })?
            },
            async_engine,
            cancellation_manager,
            handle_tracker,
            resource_manager,
            representation_manager,
            metrics: PostReturnMetrics::default(),
            max_cleanup_tasks,
            #[cfg(feature = "std")]
            cleanup_callback: None,
            #[cfg(feature = "std")]
            realloc_callback: None,
        })
    }

    /// Register a post-return function for an instance
    pub fn register_post_return(
        &mut self,
        instance_id: ComponentInstanceId,
        func_index: u32,
        cancellation_token: Option<CancellationToken>,
    ) -> Result<()> {
        let post_return_fn = PostReturnFunction {
            func_index,
            #[cfg(feature = "std")]
            func_ref: None,
            executing: false,
            cancellation_token,
        };

        #[cfg(feature = "std")]
        {
            self.functions.insert(instance_id, post_return_fn);
            self.pending_cleanups.insert(instance_id, Vec::new());
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.functions.push((instance_id, post_return_fn)).map_err(|_| {
                Error::resource_exhausted(
                    "Failed to register post-return function, capacity exceeded",
                )
            })?;
            let provider = safe_managed_alloc!(65536, CrateId::Component).map_err(|_| {
                Error::resource_exhausted("Failed to allocate memory for cleanup tasks")
            })?;
            let cleanup_vec = BoundedVec::new().map_err(|| {
                Error::resource_exhausted("Failed to create bounded vector for cleanup tasks")
            })?;
            self.pending_cleanups.push((instance_id, cleanup_vec)).map_err(|_| {
                Error::resource_exhausted(
                    "Failed to register cleanup tasks for instance, capacity exceeded",
                )
            })?;
        }

        Ok(())
    }

    /// Schedule a cleanup task to be executed during post-return
    pub fn schedule_cleanup(
        &mut self,
        instance_id: ComponentInstanceId,
        task: CleanupTask,
    ) -> Result<()> {
        #[cfg(feature = "std")]
        {
            let cleanup_tasks = self.pending_cleanups.get_mut(&instance_id).ok_or_else(|| {
                Error::runtime_execution_error("Component instance not found in cleanup registry")
            })?;

            if cleanup_tasks.len() >= self.max_cleanup_tasks {
                return Err(Error::resource_exhausted(
                    "Maximum cleanup tasks limit reached for component instance",
                ));
            }

            cleanup_tasks.push(task);

            // Update peak tasks metric
            let total_pending: usize =
                self.pending_cleanups.values().map(|tasks| tasks.len()).sum();
            if total_pending > self.metrics.peak_pending_tasks {
                self.metrics.peak_pending_tasks = total_pending;
            }

            return Ok(());
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (id, cleanup_tasks) in &mut self.pending_cleanups {
                if *id == instance_id {
                    cleanup_tasks.push(task).map_err(|_| {
                        Error::resource_exhausted("Failed to add cleanup task, capacity exceeded")
                    })?;

                    // Update peak tasks metric
                    let total_pending: usize =
                        self.pending_cleanups.iter().map(|(_, tasks)| tasks.len()).sum();
                    if total_pending > self.metrics.peak_pending_tasks {
                        self.metrics.peak_pending_tasks = total_pending;
                    }

                    return Ok(());
                }
            }

            return Err(Error::runtime_execution_error(
                "Component instance not found for cleanup task scheduling",
            ));
        }
    }

    /// Execute post-return cleanup for an instance
    pub fn execute_post_return(
        &mut self,
        instance_id: ComponentInstanceId,
        context: PostReturnContext,
    ) -> Result<()> {
        // Check if post-return function exists and isn't already executing
        #[cfg(feature = "std")]
        let post_return_fn = self.functions.get_mut(&instance_id).ok_or_else(|| {
            Error::runtime_execution_error("Post-return function not found for component instance")
        })?;

        #[cfg(not(any(feature = "std",)))]
        {
            // Check if function is already executing
            let is_executing = {
                let mut found = false;
                for (id, func) in &self.functions {
                    if *id == instance_id {
                        found = func.executing;
                        break;
                    }
                }
                found
            };

            if is_executing {
                return Err(Error::runtime_execution_error(
                    "Post-return function is already executing",
                ));
            }

            // Set executing flag
            for (id, func) in &mut self.functions {
                if *id == instance_id {
                    func.executing = true;
                    break;
                }
            }
        }

        // Simple timing implementation for no_std
        let result = self.execute_cleanup_tasks(instance_id, context);

        // Update metrics
        self.metrics.total_executions += 1;
        if result.is_err() {
            self.metrics.failed_cleanups += 1;
        }

        // Clear executing flag
        #[cfg(not(any(feature = "std",)))]
        {
            for (id, func) in &mut self.functions {
                if *id == instance_id {
                    func.executing = false;
                    break;
                }
            }
        }

        // Clear pending cleanups
        #[cfg(feature = "std")]
        {
            if let Some(cleanup_tasks) = self.pending_cleanups.get_mut(&instance_id) {
                cleanup_tasks.clear();
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (id, cleanup_tasks) in &mut self.pending_cleanups {
                if *id == instance_id {
                    cleanup_tasks.clear();
                    break;
                }
            }
        }

        result
    }

    /// Execute all pending cleanup tasks
    fn execute_cleanup_tasks(
        &mut self,
        instance_id: ComponentInstanceId,
        context: PostReturnContext,
    ) -> Result<()> {
        // Get all pending cleanup tasks - take ownership to avoid borrow issues
        #[cfg(feature = "std")]
        let mut all_tasks = context.tasks;
        #[cfg(not(any(feature = "std",)))]
        let mut all_tasks = context.tasks;

        #[cfg(feature = "std")]
        {
            if let Some(pending) = self.pending_cleanups.get(&instance_id) {
                all_tasks.extend(pending.iter().cloned());
            }
            all_tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (id, pending) in &self.pending_cleanups {
                if *id == instance_id {
                    for task in pending {
                        if all_tasks.push(task.clone()).is_err() {
                            break; // Skip if no more space
                        }
                    }
                    break;
                }
            }
            // Simple bubble sort for no_std
            for i in 0..all_tasks.len() {
                for j in 0..(all_tasks.len() - 1 - i) {
                    if all_tasks[j].priority < all_tasks[j + 1].priority {
                        let temp = all_tasks[j].clone();
                        all_tasks[j] = all_tasks[j + 1].clone();
                        all_tasks[j + 1] = temp;
                    }
                }
            }
        }

        // Execute each task - we need to clone context for each task since execute_single_cleanup_task borrows it mutably
        // Actually, we can't do that - let's restructure to avoid the borrow issue
        // Create a new context with empty task list
        #[cfg(feature = "std")]
        let mut exec_context = PostReturnContext {
            instance_id: context.instance_id,
            tasks: Vec::new(),
            realloc_manager: context.realloc_manager,
            custom_handlers: context.custom_handlers,
            async_abi: context.async_abi,
            component_id: context.component_id,
            task_id: context.task_id,
        };
        #[cfg(not(any(feature = "std",)))]
        let mut exec_context = PostReturnContext {
            instance_id: context.instance_id,
            tasks: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            realloc_manager: context.realloc_manager,
            custom_handlers: context.custom_handlers,
            async_abi: context.async_abi,
            component_id: context.component_id,
            task_id: context.task_id,
        };

        // Execute each task
        for task in &all_tasks {
            self.execute_single_cleanup_task(task, &mut exec_context)?;
            self.metrics.total_cleanup_tasks += 1;
        }

        Ok(())
    }

    /// Execute a single cleanup task
    fn execute_single_cleanup_task(
        &mut self,
        task: &CleanupTask,
        context: &mut PostReturnContext,
    ) -> Result<()> {
        match task.task_type {
            CleanupTaskType::DeallocateMemory => self.cleanup_memory(task, context),
            CleanupTaskType::CloseResource => self.cleanup_resource(task, context),
            CleanupTaskType::ReleaseReference => self.cleanup_reference(task, context),
            CleanupTaskType::Custom => self.cleanup_custom(task, context),
            CleanupTaskType::AsyncCleanup => self.cleanup_async(task, context),
            CleanupTaskType::CancelAsyncExecution => {
                self.cleanup_cancel_async_execution(task, context)
            },
            CleanupTaskType::DropBorrowedHandles => {
                self.cleanup_drop_borrowed_handles(task, context)
            },
            CleanupTaskType::EndLifetimeScope => self.cleanup_end_lifetime_scope(task, context),
            CleanupTaskType::ReleaseResourceRepresentation => {
                self.cleanup_release_resource_representation(task, context)
            },
            CleanupTaskType::FinalizeSubtask => self.cleanup_finalize_subtask(task, context),
        }
    }

    /// Free memory by calling cabi_realloc(ptr, size, align, 0)
    ///
    /// Per the Component Model spec, calling cabi_realloc with new_size=0
    /// frees the previously allocated memory.
    fn cleanup_memory(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::Memory { ptr, size, align } = &task.data {
            #[cfg(feature = "std")]
            {
                // Use the realloc callback to actually free memory
                if let Some(ref mut callback) = self.realloc_callback {
                    // Call cabi_realloc(ptr, size, align, 0) to free
                    // The instance_id comes from the task's source_instance
                    let instance_id = task.source_instance.0;
                    let _ = callback(instance_id, *ptr, *size, *align, 0)?;
                }
                // If no callback registered, we can't actually free memory.
                // This is fine for stub implementations but should be logged.
            }

            #[cfg(not(feature = "std"))]
            {
                // In no_std mode, memory management is more limited
                // For now, we just acknowledge the cleanup task
            }
        }
        Ok(())
    }

    /// Clean up resource handle by invoking its destructor
    ///
    /// If a cleanup callback is registered, this will invoke the resource's
    /// destructor function (if any) via the callback.
    fn cleanup_resource(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::Resource {
            handle,
            resource_type,
        } = &task.data
        {
            self.metrics.resource_cleanups += 1;

            #[cfg(feature = "std")]
            {
                // Use the cleanup callback to invoke the destructor
                // The destructor function index would typically be stored in the resource type
                // For now, we use a convention where the destructor is at a known function index
                // In a full implementation, this would look up the destructor from resource metadata
                if let Some(ref mut callback) = self.cleanup_callback {
                    let instance_id = task.source_instance.0;
                    // Call the resource destructor with the handle as the argument
                    // The destructor signature is: (handle: i32) -> ()
                    let _ = callback(instance_id, resource_type.0, &[*handle as i32])?;
                }
            }

            // Use resource manager if available for additional cleanup
            if let Some(_resource_manager) = &self.resource_manager {
                // The resource manager might need to update its internal state
                // after the destructor has been called
            }
        }
        Ok(())
    }

    /// Clean up reference count
    fn cleanup_reference(
        &self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::Reference {
            ref_id: _,
            ref_count: _,
        } = &task.data
        {
            // Binary std/no_std choice
            // Implementation would depend on reference counting system
        }
        Ok(())
    }

    /// Execute custom cleanup
    fn cleanup_custom(&self, task: &CleanupTask, context: &mut PostReturnContext) -> Result<()> {
        match &task.data {
            #[cfg(feature = "std")]
            CleanupData::Custom {
                cleanup_id,
                parameters: _,
            } => {
                if let Some(handler) = context.custom_handlers.get(cleanup_id) {
                    handler(&task.data)?;
                }
            },
            #[cfg(not(any(feature = "std",)))]
            CleanupData::Custom {
                cleanup_id,
                parameters: _,
            } => {
                for (id, handler) in &context.custom_handlers {
                    if id.as_str() == cleanup_id.as_str() {
                        handler(&task.data)?;
                        break;
                    }
                }
            },
            _ => {},
        }
        Ok(())
    }

    /// Clean up async resources (streams, futures, etc.)
    fn cleanup_async(&mut self, task: &CleanupTask, context: &mut PostReturnContext) -> Result<()> {
        if let CleanupData::Async {
            stream_handle,
            future_handle,
            error_context_handle,
            task_id: _,
            execution_id: _,
            cancellation_token,
        } = &task.data
        {
            self.metrics.async_cleanups += 1;

            // Cancel operations if cancellation token is available
            if let Some(token) = cancellation_token {
                let _ = token.cancel();
            }

            // Clean up async ABI resources
            // TODO: These methods require &mut self but async_abi is Arc<AsyncCanonicalAbi>
            // Either the methods need to use interior mutability (&self with Mutex/RefCell)
            // or the architecture needs to be redesigned to allow mutable access
            if let Some(_async_abi) = &context.async_abi {
                if let Some(_stream) = stream_handle {
                    // FIXME: Cannot call &mut methods on Arc-wrapped async_abi
                    // let _ = async_abi.stream_close_readable(*stream);
                    // let _ = async_abi.stream_close_writable(*stream);
                }

                if let Some(_future) = future_handle {
                    // Future cleanup would be handled by the async ABI
                }

                if let Some(_error_ctx) = error_context_handle {
                    // FIXME: Cannot call &mut methods on Arc-wrapped async_abi
                    // let _ = async_abi.error_context_drop(*error_ctx);
                }
            }
        }
        Ok(())
    }

    /// Cancel async execution
    fn cleanup_cancel_async_execution(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::AsyncExecution {
            execution_id,
            force_cancel,
        } = &task.data
        {
            self.metrics.cancellation_cleanups += 1;

            if let Some(async_engine) = &self.async_engine {
                // In a real implementation, this would cancel the execution
                if *force_cancel {
                    // Force cancel the execution
                } else {
                    // Graceful cancellation
                }
            }
        }
        Ok(())
    }

    /// Drop borrowed handles
    fn cleanup_drop_borrowed_handles(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::BorrowedHandle {
            borrow_handle: _,
            lifetime_scope,
            source_component: _,
        } = &task.data
        {
            self.metrics.handle_cleanups += 1;

            if let Some(handle_tracker) = &self.handle_tracker {
                // In a real implementation, this would invalidate the borrow
                // For now, we just acknowledge the cleanup
            }
        }
        Ok(())
    }

    /// End lifetime scope
    fn cleanup_end_lifetime_scope(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::LifetimeScope {
            scope,
            component: _,
            task: _,
        } = &task.data
        {
            if let Some(handle_tracker) = &self.handle_tracker {
                // In a real implementation, this would end the scope
                // For now, we just acknowledge the cleanup
            }
        }
        Ok(())
    }

    /// Release resource representation
    fn cleanup_release_resource_representation(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::ResourceRepresentation {
            handle,
            resource_id: _,
            component: _,
        } = &task.data
        {
            self.metrics.resource_cleanups += 1;

            if let Some(repr_manager) = &self.representation_manager {
                // In a real implementation, this would drop the resource representation
                // let _ = canon_resource_drop(repr_manager, *handle);
            }
        }
        Ok(())
    }

    /// Finalize subtask
    fn cleanup_finalize_subtask(
        &mut self,
        task: &CleanupTask,
        _context: &mut PostReturnContext,
    ) -> Result<()> {
        if let CleanupData::Subtask {
            execution_id,
            task_id: _,
            result: _,
            force_cleanup,
        } = &task.data
        {
            if let Some(cancellation_manager) = &self.cancellation_manager {
                if *force_cleanup {
                    // Force cleanup the subtask
                } else {
                    // Graceful finalization
                }
            }
        }
        Ok(())
    }

    /// Remove all cleanup tasks for an instance
    pub fn cleanup_instance(&mut self, instance_id: ComponentInstanceId) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.functions.remove(&instance_id);
            self.pending_cleanups.remove(&instance_id);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // Remove from functions
            let mut i = 0;
            while i < self.functions.len() {
                if self.functions[i].0 == instance_id {
                    self.functions.remove(i);
                    break;
                } else {
                    i += 1;
                }
            }

            // Remove from pending cleanups
            let mut i = 0;
            while i < self.pending_cleanups.len() {
                if self.pending_cleanups[i].0 == instance_id {
                    self.pending_cleanups.remove(i);
                    break;
                } else {
                    i += 1;
                }
            }
        }
        Ok(())
    }

    /// Get execution metrics
    pub fn metrics(&self) -> &PostReturnMetrics {
        &self.metrics
    }

    /// Reset metrics
    pub fn reset_metrics(&mut self) {
        self.metrics = PostReturnMetrics::default();
    }
}

/// Helper functions for common cleanup scenarios
pub mod helpers {
    use super::*;

    /// Create a memory cleanup task
    pub fn memory_cleanup_task(
        instance_id: ComponentInstanceId,
        ptr: i32,
        size: i32,
        align: i32,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::DeallocateMemory,
            source_instance: instance_id,
            priority,
            data: CleanupData::Memory { ptr, size, align },
        }
    }

    /// Create a resource cleanup task
    pub fn resource_cleanup_task(
        instance_id: ComponentInstanceId,
        handle: u32,
        resource_type: TypeId,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::CloseResource,
            source_instance: instance_id,
            priority,
            data: CleanupData::Resource {
                handle,
                resource_type,
            },
        }
    }

    /// Create an async cleanup task for streams, futures, and async operations
    pub fn async_cleanup_task(
        instance_id: ComponentInstanceId,
        stream_handle: Option<StreamHandle>,
        future_handle: Option<FutureHandle>,
        error_context_handle: Option<ErrorContextHandle>,
        task_id: Option<TaskId>,
        execution_id: Option<ExecutionId>,
        cancellation_token: Option<CancellationToken>,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::AsyncCleanup,
            source_instance: instance_id,
            priority,
            data: CleanupData::Async {
                stream_handle,
                future_handle,
                error_context_handle,
                task_id,
                execution_id,
                cancellation_token,
            },
        }
    }

    /// Create an async execution cancellation task
    pub fn async_execution_cleanup_task(
        instance_id: ComponentInstanceId,
        execution_id: ExecutionId,
        force_cancel: bool,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::CancelAsyncExecution,
            source_instance: instance_id,
            priority,
            data: CleanupData::AsyncExecution {
                execution_id,
                force_cancel,
            },
        }
    }

    /// Create a borrowed handle cleanup task
    pub fn borrowed_handle_cleanup_task(
        instance_id: ComponentInstanceId,
        borrow_handle: u32,
        lifetime_scope: LifetimeScope,
        source_component: ComponentId,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::DropBorrowedHandles,
            source_instance: instance_id,
            priority,
            data: CleanupData::BorrowedHandle {
                borrow_handle,
                lifetime_scope,
                source_component,
            },
        }
    }

    /// Create a lifetime scope cleanup task
    pub fn lifetime_scope_cleanup_task(
        instance_id: ComponentInstanceId,
        scope: LifetimeScope,
        component: ComponentId,
        task: TaskId,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::EndLifetimeScope,
            source_instance: instance_id,
            priority,
            data: CleanupData::LifetimeScope {
                scope,
                component,
                task,
            },
        }
    }

    /// Create a resource representation cleanup task
    pub fn resource_representation_cleanup_task(
        instance_id: ComponentInstanceId,
        handle: u32,
        resource_id: ResourceId,
        component: ComponentId,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::ReleaseResourceRepresentation,
            source_instance: instance_id,
            priority,
            data: CleanupData::ResourceRepresentation {
                handle,
                resource_id,
                component,
            },
        }
    }

    /// Create a subtask finalization cleanup task
    pub fn subtask_cleanup_task(
        instance_id: ComponentInstanceId,
        execution_id: ExecutionId,
        task_id: TaskId,
        result: Option<SubtaskResult>,
        force_cleanup: bool,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::FinalizeSubtask,
            source_instance: instance_id,
            priority,
            data: CleanupData::Subtask {
                execution_id,
                task_id,
                result,
                force_cleanup,
            },
        }
    }

    /// Create a custom cleanup task
    #[cfg(feature = "std")]
    pub fn custom_cleanup_task(
        instance_id: ComponentInstanceId,
        cleanup_id: &str,
        parameters: Vec<ComponentValue<ComponentProvider>>,
        priority: u8,
    ) -> CleanupTask {
        CleanupTask {
            task_type: CleanupTaskType::Custom,
            source_instance: instance_id,
            priority,
            data: CleanupData::Custom {
                cleanup_id: cleanup_id.to_string(),
                parameters,
            },
        }
    }

    /// Create a custom cleanup task (no_std version)
    #[cfg(not(any(feature = "std",)))]
    pub fn custom_cleanup_task(
        instance_id: ComponentInstanceId,
        cleanup_id: &str,
        parameters: BoundedVec<ComponentValue<ComponentProvider>, 16>,
        priority: u8,
    ) -> Result<CleanupTask> {
        use wrt_foundation::safe_memory::NoStdProvider;

        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let cleanup_id = BoundedString::try_from_str(cleanup_id).map_err(|_| {
            Error::runtime_execution_error("Failed to create cleanup task ID as bounded string")
        })?;

        // Convert parameters to use NoStdProvider<2048>
        let mut param_vec: BoundedVec<ComponentValue<NoStdProvider<2048>>, 16> = BoundedVec::new();
        for param in parameters.iter() {
            // Convert each parameter - this is a simplified conversion
            // In practice, you may need to deep-copy the ComponentValue with a new provider
            let converted_param = match param {
                ComponentValue::Bool(b) => ComponentValue::Bool(*b),
                ComponentValue::U8(v) => ComponentValue::U8(*v),
                ComponentValue::U16(v) => ComponentValue::U16(*v),
                ComponentValue::U32(v) => ComponentValue::U32(*v),
                ComponentValue::U64(v) => ComponentValue::U64(*v),
                ComponentValue::S8(v) => ComponentValue::S8(*v),
                ComponentValue::S16(v) => ComponentValue::S16(*v),
                ComponentValue::S32(v) => ComponentValue::S32(*v),
                ComponentValue::S64(v) => ComponentValue::S64(*v),
                ComponentValue::F32(v) => ComponentValue::F32(*v),
                ComponentValue::F64(v) => ComponentValue::F64(*v),
                // For complex types, just skip for now
                _ => continue,
            };
            param_vec.push(converted_param).map_err(|_| {
                Error::runtime_execution_error("Failed to push parameter to cleanup task")
            })?;
        }

        Ok(CleanupTask {
            task_type: CleanupTaskType::Custom,
            source_instance: instance_id,
            priority,
            data: CleanupData::Custom {
                cleanup_id,
                parameters: param_vec,
            },
        })
    }
}

impl Default for PostReturnRegistry {
    fn default() -> Self {
        Self::new(1024).unwrap_or_else(|_| {
            // Fallback on allocation failure - this should not happen in practice
            // but satisfies the Default trait requirement
            panic!("Failed to allocate memory for PostReturnRegistry")
        })
    }
}
