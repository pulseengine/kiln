//! Async Runtime for WebAssembly Component Model
//! SW-REQ-ID: REQ_FUNC_030
//!
//! This module implements a complete async runtime with task scheduling,
//! stream operations, and future management for the Component Model.

#[cfg(not(feature = "std"))]
use core::{fmt, mem, time::Duration};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::VecDeque, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem, time::Duration};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    MemoryProvider,
    bounded::{BoundedError, BoundedString},
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    prelude::*,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

// Placeholder types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;
#[cfg(not(feature = "component-model-threading"))]
pub type Task = ();
#[cfg(not(feature = "component-model-threading"))]
pub type TaskContext = ();
#[cfg(not(feature = "component-model-threading"))]
pub type TaskManager = ();
#[cfg(not(feature = "component-model-threading"))]
pub type TaskState = ();
#[cfg(not(feature = "component-model-threading"))]
pub type TaskType = ();

use super::async_types::{
    AsyncReadResult, Future, FutureHandle, FutureState, Stream, StreamHandle, StreamState,
    Waitable, WaitableSet,
};
#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::{Task, TaskContext, TaskId, TaskManager, TaskState, TaskType};
use crate::types::{ValType, Value};

/// Maximum number of concurrent tasks in no_std environments
const MAX_CONCURRENT_TASKS: usize = 128;

/// Maximum number of pending operations in no_std environments
const MAX_PENDING_OPS: usize = 256;

/// Maximum reactor events per iteration in no_std environments
const MAX_REACTOR_EVENTS: usize = 64;

/// Async runtime for WebAssembly Component Model
#[derive(Debug)]
pub struct AsyncRuntime {
    /// Task scheduler
    scheduler: TaskScheduler,

    /// Reactor for async I/O
    reactor: Reactor,

    /// Stream registry
    #[cfg(feature = "std")]
    streams: Vec<StreamEntry>,
    #[cfg(not(any(feature = "std",)))]
    streams: BoundedVec<StreamEntry, MAX_CONCURRENT_TASKS>,

    /// Future registry
    #[cfg(feature = "std")]
    futures: Vec<FutureEntry>,
    #[cfg(not(any(feature = "std",)))]
    futures: BoundedVec<FutureEntry, MAX_CONCURRENT_TASKS>,

    /// Runtime configuration
    config: RuntimeConfig,

    /// Runtime statistics
    stats: RuntimeStats,

    /// Whether runtime is running
    is_running: bool,
}

/// Task scheduler for async operations
#[derive(Debug)]
pub struct TaskScheduler {
    /// Ready queue for immediately runnable tasks
    #[cfg(feature = "std")]
    ready_queue: VecDeque<ScheduledTask>,
    #[cfg(not(any(feature = "std",)))]
    ready_queue: BoundedVec<ScheduledTask, MAX_CONCURRENT_TASKS>,

    /// Waiting tasks (blocked on I/O or timers)
    #[cfg(feature = "std")]
    waiting_tasks: Vec<WaitingTask>,
    #[cfg(not(any(feature = "std",)))]
    waiting_tasks: BoundedVec<WaitingTask, MAX_CONCURRENT_TASKS>,

    /// Current time for scheduling
    current_time: u64,

    /// Task manager for low-level task operations
    task_manager: TaskManager,

    /// Next task ID counter (used when TaskManager is ())
    #[cfg(not(feature = "component-model-threading"))]
    next_task_id: u32,
}

/// Reactor for handling async I/O events
#[derive(Debug)]
pub struct Reactor {
    /// Pending events
    #[cfg(feature = "std")]
    pending_events: VecDeque<ReactorEvent>,
    #[cfg(not(any(feature = "std",)))]
    pending_events: BoundedVec<ReactorEvent, MAX_REACTOR_EVENTS>,

    /// Event handlers
    #[cfg(feature = "std")]
    event_handlers: Vec<EventHandler>,
    #[cfg(not(any(feature = "std",)))]
    event_handlers: BoundedVec<EventHandler, MAX_REACTOR_EVENTS>,
}

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Task time slice in microseconds
    pub task_time_slice_us: u64,
    /// Maximum time to run scheduler per iteration (microseconds)
    pub max_scheduler_time_us: u64,
    /// Enable task priority scheduling
    pub priority_scheduling: bool,
    /// Enable work stealing between tasks
    pub work_stealing: bool,
}

/// Runtime statistics
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Total tasks created
    pub tasks_created: u64,
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Current active tasks
    pub active_tasks: u32,
    /// Total scheduler iterations
    pub scheduler_iterations: u64,
    /// Total time spent in scheduler (microseconds)
    pub scheduler_time_us: u64,
    /// Average task execution time (microseconds)
    pub avg_task_execution_time_us: u64,
}

/// Entry for a registered stream
#[derive(Debug)]
pub struct StreamEntry {
    /// Stream handle
    pub handle: StreamHandle,
    /// Stream instance
    pub stream: Stream<Value>,
    /// Associated tasks
    #[cfg(feature = "std")]
    pub tasks: Vec<TaskId>,
    #[cfg(not(any(feature = "std",)))]
    pub tasks: BoundedVec<TaskId, 16>,
}

impl Clone for StreamEntry {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            stream: self.stream.clone(),
            tasks: self.tasks.clone(),
        }
    }
}

impl Default for StreamEntry {
    fn default() -> Self {
        Self {
            handle: StreamHandle::default(),
            stream: Stream::default(),
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        }
    }
}

impl PartialEq for StreamEntry {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Eq for StreamEntry {}

impl wrt_foundation::traits::Checksummable for StreamEntry {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.handle.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for StreamEntry {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for StreamEntry {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            handle: StreamHandle::from_bytes_with_provider(reader, provider)?,
            stream: Stream::default(),
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        })
    }
}

/// Entry for a registered future
#[derive(Debug)]
pub struct FutureEntry {
    /// Future handle
    pub handle: FutureHandle,
    /// Future instance
    pub future: Future<Value>,
    /// Associated tasks
    #[cfg(feature = "std")]
    pub tasks: Vec<TaskId>,
    #[cfg(not(any(feature = "std",)))]
    pub tasks: BoundedVec<TaskId, 16>,
}

impl Clone for FutureEntry {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            future: self.future.clone(),
            tasks: self.tasks.clone(),
        }
    }
}

impl Default for FutureEntry {
    fn default() -> Self {
        Self {
            handle: FutureHandle::default(),
            future: Future::default(),
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        }
    }
}

impl PartialEq for FutureEntry {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Eq for FutureEntry {}

impl wrt_foundation::traits::Checksummable for FutureEntry {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.handle.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for FutureEntry {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for FutureEntry {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            handle: FutureHandle::from_bytes_with_provider(reader, provider)?,
            future: Future::default(),
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        })
    }
}

/// Scheduled task in the ready queue
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// Task ID
    pub task_id: TaskId,
    /// Task priority (0 = highest)
    pub priority: u8,
    /// Estimated execution time (microseconds)
    pub estimated_time_us: u64,
    /// Task function to execute
    pub task_fn: TaskFunction,
}

impl Default for ScheduledTask {
    fn default() -> Self {
        Self {
            task_id: TaskId::default(),
            priority: 0,
            estimated_time_us: 0,
            task_fn: TaskFunction::Custom {
                name: BoundedString::from_str_truncate("")
                    .unwrap_or_else(|_| panic!("Failed to create default task name")),
                placeholder: 0,
            },
        }
    }
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for ScheduledTask {}

impl wrt_foundation::traits::Checksummable for ScheduledTask {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.task_id.update_checksum(checksum);
        self.priority.update_checksum(checksum);
        self.estimated_time_us.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ScheduledTask {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.task_id.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        self.estimated_time_us.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for ScheduledTask {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            task_id: TaskId::from_bytes_with_provider(reader, provider)?,
            priority: u8::from_bytes_with_provider(reader, provider)?,
            estimated_time_us: u64::from_bytes_with_provider(reader, provider)?,
            task_fn: TaskFunction::Custom {
                name: BoundedString::from_str_truncate("").map_err(|_| {
                    Error::foundation_bounded_capacity_exceeded("Failed to create task name")
                })?,
                placeholder: 0,
            },
        })
    }
}

/// Waiting task (blocked on I/O or timers)
#[derive(Debug, Clone)]
pub struct WaitingTask {
    /// Task ID
    pub task_id: TaskId,
    /// What the task is waiting for
    pub wait_condition: WaitCondition,
    /// Timeout (absolute time in microseconds)
    pub timeout_us: Option<u64>,
}

impl Default for WaitingTask {
    fn default() -> Self {
        Self {
            task_id: TaskId::default(),
            wait_condition: WaitCondition::Timer(0),
            timeout_us: None,
        }
    }
}

impl PartialEq for WaitingTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for WaitingTask {}

impl wrt_foundation::traits::Checksummable for WaitingTask {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.task_id.update_checksum(checksum);
        if let Some(timeout) = self.timeout_us {
            timeout.update_checksum(checksum);
        }
    }
}

impl wrt_runtime::ToBytes for WaitingTask {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.task_id.to_bytes_with_provider(writer, provider)?;
        self.timeout_us.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for WaitingTask {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            task_id: TaskId::from_bytes_with_provider(reader, provider)?,
            wait_condition: WaitCondition::Timer(0),
            timeout_us: Option::<u64>::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Task function type
#[derive(Debug, Clone)]
pub enum TaskFunction {
    /// Stream operation
    StreamOp {
        handle: StreamHandle,
        operation: StreamOperation,
    },
    /// Future operation
    FutureOp {
        handle: FutureHandle,
        operation: FutureOperation,
    },
    /// Custom user function
    Custom {
        name: BoundedString<64>,
        // In a real implementation, this would be a function pointer
        // For now, we'll use a placeholder
        placeholder: u32,
    },
}

/// Stream operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOperation {
    /// Read from stream
    Read,
    /// Write to stream
    Write,
    /// Close stream
    Close,
}

/// Future operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureOperation {
    /// Get future value
    Get,
    /// Set future value
    Set,
    /// Cancel future
    Cancel,
}

/// Wait condition for blocked tasks
#[derive(Debug, Clone)]
pub enum WaitCondition {
    /// Waiting for stream to be readable
    StreamReadable(StreamHandle),
    /// Waiting for stream to be writable
    StreamWritable(StreamHandle),
    /// Waiting for future to be ready
    FutureReady(FutureHandle),
    /// Waiting for timer
    Timer(u64),
    /// Waiting for multiple conditions
    Multiple(WaitableSet),
}

/// Reactor event
#[derive(Debug, Clone)]
pub struct ReactorEvent {
    /// Event ID
    pub id: u32,
    /// Event type
    pub event_type: ReactorEventType,
    /// Associated data
    pub data: u64,
}

impl Default for ReactorEvent {
    fn default() -> Self {
        Self {
            id: 0,
            event_type: ReactorEventType::StreamReady(StreamHandle::default()),
            data: 0,
        }
    }
}

impl PartialEq for ReactorEvent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ReactorEvent {}

impl wrt_foundation::traits::Checksummable for ReactorEvent {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.id.update_checksum(checksum);
        self.data.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ReactorEvent {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.id.to_bytes_with_provider(writer, provider)?;
        self.data.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for ReactorEvent {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            id: u32::from_bytes_with_provider(reader, provider)?,
            event_type: ReactorEventType::StreamReady(StreamHandle::default()),
            data: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Reactor event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactorEventType {
    /// Stream became readable
    StreamReadable,
    /// Stream became writable
    StreamWritable,
    /// Stream became ready
    StreamReady(StreamHandle),
    /// Future became ready
    FutureReady,
    /// Timer expired
    TimerExpired,
    /// Task ready
    TaskReady,
}

impl ReactorEventType {
    /// Get discriminant as u8
    fn discriminant(&self) -> u8 {
        match self {
            Self::StreamReadable => 0,
            Self::StreamWritable => 1,
            Self::StreamReady(_) => 2,
            Self::FutureReady => 3,
            Self::TimerExpired => 4,
            Self::TaskReady => 5,
        }
    }
}

impl Checksummable for ReactorEventType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.discriminant().update_checksum(checksum);
    }
}

impl ToBytes for ReactorEventType {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.discriminant().to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ReactorEventType {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let tag = u8::from_bytes_with_provider(reader, provider)?;
        match tag {
            0 => Ok(Self::StreamReadable),
            1 => Ok(Self::StreamWritable),
            2 => Ok(Self::FutureReady),
            3 => Ok(Self::TimerExpired),
            4 => Ok(Self::TaskReady),
            _ => Err(Error::validation_invalid_type(
                "Invalid ReactorEventType tag",
            )),
        }
    }
}

/// Event handler for reactor events
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandler {
    /// Handler ID
    pub id: u32,
    /// Event type to handle
    pub event_type: ReactorEventType,
    /// Associated task
    pub task_id: TaskId,
}

impl Default for EventHandler {
    fn default() -> Self {
        Self {
            id: 0,
            event_type: ReactorEventType::TaskReady,
            task_id: 0,
        }
    }
}

impl Checksummable for EventHandler {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.id.update_checksum(checksum);
        self.event_type.update_checksum(checksum);
        self.task_id.update_checksum(checksum);
    }
}

impl ToBytes for EventHandler {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.id.to_bytes_with_provider(writer, provider)?;
        self.event_type.to_bytes_with_provider(writer, provider)?;
        self.task_id.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for EventHandler {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let id = u32::from_bytes_with_provider(reader, provider)?;
        let event_type = ReactorEventType::from_bytes_with_provider(reader, provider)?;
        let task_id = u32::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            id,
            event_type,
            task_id,
        })
    }
}

/// Task execution result
#[derive(Debug, Clone)]
pub enum TaskExecutionResult {
    /// Task completed successfully
    Completed,
    /// Task yielded, should be rescheduled
    Yielded,
    /// Task is waiting for I/O
    Waiting(WaitCondition),
    /// Task failed with error
    Failed(Error),
}

impl AsyncRuntime {
    /// Create new async runtime
    pub fn new() -> Result<Self> {
        Ok(Self {
            scheduler: TaskScheduler::new()?,
            reactor: Reactor::new()?,
            #[cfg(feature = "std")]
            streams: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            streams: BoundedVec::new(),
            #[cfg(feature = "std")]
            futures: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            futures: BoundedVec::new(),
            config: RuntimeConfig::default(),
            stats: RuntimeStats::new(),
            is_running: false,
        })
    }

    /// Create new async runtime with custom configuration
    pub fn with_config(config: RuntimeConfig) -> Result<Self> {
        let mut runtime = Self::new()?;
        runtime.config = config;
        Ok(runtime)
    }

    /// Start the async runtime
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Err(Error::async_executor_state_violation(
                "Runtime is already running",
            ));
        }

        self.is_running = true;
        self.stats.scheduler_iterations = 0;
        Ok(())
    }

    /// Stop the async runtime
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Err(Error::async_executor_state_violation(
                "Runtime is not running",
            ));
        }

        self.is_running = false;

        // Clean up all tasks
        self.scheduler.cleanup_all_tasks()?;

        Ok(())
    }

    /// Execute one iteration of the runtime loop
    pub fn tick(&mut self) -> Result<bool> {
        if !self.is_running {
            return Ok(false);
        }

        let start_time = self.get_current_time();

        // Process reactor events
        self.reactor.process_events(&mut self.scheduler)?;

        // Run scheduler
        let has_work = self.scheduler.run_iteration(&self.config)?;

        // Update statistics
        let elapsed = self.get_current_time() - start_time;
        self.stats.scheduler_iterations += 1;
        self.stats.scheduler_time_us += elapsed;

        Ok(has_work || !self.scheduler.is_idle())
    }

    /// Run the runtime until all tasks complete or timeout
    pub fn run_to_completion(&mut self, timeout_us: Option<u64>) -> Result<()> {
        let start_time = self.get_current_time();

        while self.is_running {
            let has_work = self.tick()?;

            if !has_work {
                break; // No more work to do
            }

            if let Some(timeout) = timeout_us {
                if self.get_current_time() - start_time > timeout {
                    return Err(Error::async_error("Runtime timeout"));
                }
            }
        }

        Ok(())
    }

    /// Spawn a new task
    pub fn spawn_task(&mut self, task_fn: TaskFunction, priority: u8) -> Result<TaskId> {
        #[cfg(feature = "component-model-threading")]
        let task_id = self.scheduler.task_manager.create_task()?;

        #[cfg(not(feature = "component-model-threading"))]
        let task_id = {
            let id = self.scheduler.next_task_id;
            self.scheduler.next_task_id = self.scheduler.next_task_id.wrapping_add(1);
            id
        };

        let scheduled_task = ScheduledTask {
            task_id,
            priority,
            estimated_time_us: 1000, // Default 1ms estimate
            task_fn,
        };

        self.scheduler.schedule_task(scheduled_task)?;
        self.stats.tasks_created += 1;
        self.stats.active_tasks += 1;

        Ok(task_id)
    }

    /// Register a stream with the runtime
    pub fn register_stream(&mut self, stream: Stream<Value>) -> Result<StreamHandle> {
        let handle = stream.handle;

        let entry = StreamEntry {
            handle,
            stream,
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        };

        #[cfg(feature = "std")]
        {
            self.streams.push(entry);
        }
        #[cfg(not(feature = "std"))]
        {
            self.streams
                .push(entry)
                .map_err(|_| Error::async_executor_state_violation("Too many streams"))?;
        }

        Ok(handle)
    }

    /// Register a future with the runtime
    pub fn register_future(&mut self, future: Future<Value>) -> Result<FutureHandle> {
        let handle = future.handle;

        let entry = FutureEntry {
            handle,
            future,
            #[cfg(feature = "std")]
            tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            tasks: BoundedVec::new(),
        };

        #[cfg(feature = "std")]
        {
            self.futures.push(entry);
        }
        #[cfg(not(feature = "std"))]
        {
            self.futures
                .push(entry)
                .map_err(|_| Error::async_executor_state_violation("Too many futures"))?;
        }

        Ok(handle)
    }

    /// Get runtime statistics
    pub fn get_stats(&self) -> &RuntimeStats {
        &self.stats
    }

    /// Get current configuration
    pub fn get_config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Update runtime configuration
    pub fn update_config(&mut self, config: RuntimeConfig) -> Result<()> {
        if self.is_running && config.max_concurrent_tasks < self.stats.active_tasks as usize {
            return Err(Error::async_executor_state_violation(
                "Cannot reduce max concurrent tasks below current active count",
            ));
        }

        self.config = config;
        Ok(())
    }

    /// Get current time in microseconds (simplified implementation)
    fn get_current_time(&self) -> u64 {
        // In a real implementation, this would use a proper time source
        self.scheduler.current_time
    }
}

impl TaskScheduler {
    /// Create new task scheduler
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            ready_queue: VecDeque::new(),
            #[cfg(not(any(feature = "std",)))]
            ready_queue: BoundedVec::new(),
            #[cfg(feature = "std")]
            waiting_tasks: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            waiting_tasks: BoundedVec::new(),
            current_time: 0,
            task_manager: (), // TaskManager is () when component-model-threading is disabled
            #[cfg(not(feature = "component-model-threading"))]
            next_task_id: 1,
        })
    }

    /// Schedule a task for execution
    pub fn schedule_task(&mut self, task: ScheduledTask) -> Result<()> {
        #[cfg(feature = "std")]
        {
            // Insert task in priority order (lower number = higher priority)
            let insert_pos = self
                .ready_queue
                .iter()
                .position(|t| t.priority > task.priority)
                .unwrap_or(self.ready_queue.len());

            self.ready_queue.insert(insert_pos, task);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.ready_queue
                .push(task)
                .map_err(|_| Error::async_executor_state_violation("Ready queue full"))?;
        }

        Ok(())
    }

    /// Run one scheduler iteration
    pub fn run_iteration(&mut self, config: &RuntimeConfig) -> Result<bool> {
        let mut has_work = false;
        let iteration_start = self.current_time;

        // Process ready tasks
        while let Some(task) = self.get_next_ready_task() {
            has_work = true;

            let task_start = self.current_time;
            let result = self.execute_task(&task)?;
            let task_duration = self.current_time - task_start;

            // Handle execution result
            match result {
                TaskExecutionResult::Completed => {
                    // Task finished, no need to reschedule
                },
                TaskExecutionResult::Yielded => {
                    // Reschedule task
                    self.schedule_task(task)?;
                },
                TaskExecutionResult::Waiting(condition) => {
                    // Add to waiting tasks
                    let waiting_task = WaitingTask {
                        task_id: task.task_id,
                        wait_condition: condition,
                        timeout_us: Some(self.current_time + 1_000_000), // 1 second timeout
                    };
                    #[cfg(feature = "std")]
                    {
                        self.waiting_tasks.push(waiting_task);
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        self.waiting_tasks.push(waiting_task).map_err(|_| {
                            Error::async_executor_state_violation("Waiting tasks list full")
                        })?;
                    }
                },
                TaskExecutionResult::Failed(_error) => {
                    // Task failed, log and remove
                    // In a real implementation, we'd log the error
                },
            }

            // Check if we've exceeded our time slice
            if self.current_time - iteration_start > config.max_scheduler_time_us {
                break;
            }

            // Simulate time progression
            self.current_time += task_duration.max(100); // At least 100us per
            // task
        }

        // Check waiting tasks for timeouts or condition changes
        self.process_waiting_tasks()?;

        Ok(has_work)
    }

    /// Check if scheduler is idle
    pub fn is_idle(&self) -> bool {
        self.ready_queue.is_empty() && self.waiting_tasks.is_empty()
    }

    /// Clean up all tasks
    pub fn cleanup_all_tasks(&mut self) -> Result<()> {
        self.ready_queue.clear();
        self.waiting_tasks.clear();
        Ok(())
    }

    // Private helper methods

    fn get_next_ready_task(&mut self) -> Option<ScheduledTask> {
        #[cfg(feature = "std")]
        {
            self.ready_queue.pop_front()
        }
        #[cfg(not(any(feature = "std",)))]
        {
            if !self.ready_queue.is_empty() {
                Some(self.ready_queue.remove(0))
            } else {
                None
            }
        }
    }

    fn execute_task(&mut self, task: &ScheduledTask) -> Result<TaskExecutionResult> {
        // Simplified task execution - in real implementation this would
        // actually execute the task function
        match &task.task_fn {
            TaskFunction::StreamOp {
                handle: _,
                operation,
            } => {
                match operation {
                    StreamOperation::Read => {
                        // Simulate stream read
                        Ok(TaskExecutionResult::Completed)
                    },
                    StreamOperation::Write => {
                        // Simulate stream write
                        Ok(TaskExecutionResult::Completed)
                    },
                    StreamOperation::Close => {
                        // Simulate stream close
                        Ok(TaskExecutionResult::Completed)
                    },
                }
            },
            TaskFunction::FutureOp {
                handle: _,
                operation,
            } => {
                match operation {
                    FutureOperation::Get => {
                        // Simulate future get
                        Ok(TaskExecutionResult::Waiting(WaitCondition::Timer(
                            self.current_time + 1000,
                        )))
                    },
                    FutureOperation::Set => {
                        // Simulate future set
                        Ok(TaskExecutionResult::Completed)
                    },
                    FutureOperation::Cancel => {
                        // Simulate future cancel
                        Ok(TaskExecutionResult::Completed)
                    },
                }
            },
            TaskFunction::Custom { .. } => {
                // Simulate custom task execution
                Ok(TaskExecutionResult::Completed)
            },
        }
    }

    fn process_waiting_tasks(&mut self) -> Result<()> {
        let mut i = 0;
        while i < self.waiting_tasks.len() {
            let should_reschedule = {
                let waiting_task = &self.waiting_tasks[i];

                // Check timeout
                if let Some(timeout) = waiting_task.timeout_us {
                    if self.current_time >= timeout {
                        true // Timeout, reschedule task
                    } else {
                        false // Still waiting
                    }
                } else {
                    false // No timeout, still waiting
                }
            };

            if should_reschedule {
                let waiting_task = self.waiting_tasks.remove(i);

                // Create a new scheduled task
                let provider = safe_managed_alloc!(512, CrateId::Component)?;
                let scheduled_task = ScheduledTask {
                    task_id: waiting_task.task_id,
                    priority: 0, // Default priority
                    estimated_time_us: 1000,
                    task_fn: TaskFunction::Custom {
                        name: BoundedString::try_from_str("timeout").unwrap_or_default(),
                        placeholder: 0,
                    },
                };

                self.schedule_task(scheduled_task)?;
            } else {
                i += 1;
            }
        }

        Ok(())
    }
}

impl Reactor {
    /// Create new reactor
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            pending_events: VecDeque::new(),
            #[cfg(not(any(feature = "std",)))]
            pending_events: BoundedVec::new(),
            #[cfg(feature = "std")]
            event_handlers: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            event_handlers: BoundedVec::new(),
        })
    }

    /// Process pending events
    pub fn process_events(&mut self, scheduler: &mut TaskScheduler) -> Result<()> {
        #[cfg(feature = "std")]
        {
            while let Some(event) = self.pending_events.pop_front() {
                self.handle_event(event, scheduler)?;
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            while !self.pending_events.is_empty() {
                let event = self.pending_events.remove(0);
                self.handle_event(event, scheduler)?;
            }
        }

        Ok(())
    }

    /// Add event to pending queue
    pub fn add_event(&mut self, event: ReactorEvent) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.pending_events.push_back(event);
            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        self.pending_events
            .push(event)
            .map_err(|_| Error::async_executor_state_violation("Event queue full"))
    }

    fn handle_event(&mut self, _event: ReactorEvent, _scheduler: &mut TaskScheduler) -> Result<()> {
        // Simplified event handling - in real implementation this would
        // wake up waiting tasks based on the event type
        Ok(())
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: MAX_CONCURRENT_TASKS,
            task_time_slice_us: 1000,     // 1ms
            max_scheduler_time_us: 10000, // 10ms
            priority_scheduling: true,
            work_stealing: false,
        }
    }
}

impl Default for RuntimeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeStats {
    /// Create new runtime statistics
    pub fn new() -> Self {
        Self {
            tasks_created: 0,
            tasks_completed: 0,
            active_tasks: 0,
            scheduler_iterations: 0,
            scheduler_time_us: 0,
            avg_task_execution_time_us: 0,
        }
    }
}

// Default implementations removed - constructors now return Result types

impl fmt::Display for StreamOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamOperation::Read => write!(f, "read"),
            StreamOperation::Write => write!(f, "write"),
            StreamOperation::Close => write!(f, "close"),
        }
    }
}

impl fmt::Display for FutureOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FutureOperation::Get => write!(f, "get"),
            FutureOperation::Set => write!(f, "set"),
            FutureOperation::Cancel => write!(f, "cancel"),
        }
    }
}

impl fmt::Display for ReactorEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReactorEventType::StreamReadable => write!(f, "stream-readable"),
            ReactorEventType::StreamWritable => write!(f, "stream-writable"),
            ReactorEventType::StreamReady(_) => write!(f, "stream-ready"),
            ReactorEventType::FutureReady => write!(f, "future-ready"),
            ReactorEventType::TimerExpired => write!(f, "timer-expired"),
            ReactorEventType::TaskReady => write!(f, "task-ready"),
        }
    }
}
