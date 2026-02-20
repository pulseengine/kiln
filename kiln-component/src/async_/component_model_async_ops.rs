//! Component Model async operations implementation
//!
//! This module implements the async operations defined in the WebAssembly
//! Component Model, including task.wait, task.yield, and task.poll.

use core::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use wrt_foundation::{
    Arc, CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    safe_managed_alloc,
};
use wrt_sync::Mutex;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::{TaskId, TaskManager};

// Fallback types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskManager = ();
#[cfg(not(feature = "component-model-threading"))]
use crate::async_::fuel_async_executor::TaskId;
use crate::{
    ComponentInstanceId,
    async_::{
        async_types::{FutureHandle, StreamHandle, Waitable, WaitableSet},
        fuel_async_executor::{
            AsyncTaskState, ComponentAsyncOperation, ExecutionContext, ExecutionStepResult,
            FuelAsyncExecutor, FuelAsyncTask,
        },
    },
    prelude::*,
    types::ComponentInstance,
};

/// Maximum number of waitables per task.wait call
const MAX_WAITABLES: usize = 64;

/// Fuel cost for async operations
const TASK_WAIT_FUEL: u64 = 50;
const TASK_YIELD_FUEL: u64 = 20;
const TASK_POLL_FUEL: u64 = 30;

/// Component Model async operations handler
pub struct ComponentModelAsyncOps {
    /// Reference to the fuel async executor
    executor: Arc<Mutex<FuelAsyncExecutor>>,
    /// Reference to the task manager
    task_manager: Arc<Mutex<TaskManager>>,
    /// Active wait operations
    active_waits: BoundedMap<TaskId, WaitOperation, 128>,
    /// Waitable registry
    waitable_registry: WaitableRegistry,
    /// Operation statistics
    stats: AsyncOpStats,
}

/// Active wait operation
#[derive(Debug)]
struct WaitOperation {
    /// Task that is waiting
    task_id: TaskId,
    /// Set of waitables
    waitables: WaitableSet,
    /// Which waitable became ready (if any)
    ready_index: Option<u32>,
    /// Timestamp when wait started
    start_time: u64,
    /// Timeout duration (0 = no timeout)
    timeout_ms: u64,
}

/// Registry for managing waitables
struct WaitableRegistry {
    /// Future handles (readable state)
    futures_readable: BoundedMap<FutureHandle, WaitableState, 256>,
    /// Future handles (writable state)
    futures_writable: BoundedMap<FutureHandle, WaitableState, 256>,
    /// Stream handles (readable state)
    streams_readable: BoundedMap<StreamHandle, WaitableState, 128>,
    /// Stream handles (writable state)
    streams_writable: BoundedMap<StreamHandle, WaitableState, 128>,
}

/// State of a waitable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitableState {
    /// Waitable is pending
    Pending,
    /// Waitable is ready
    Ready,
    /// Waitable has been consumed
    Consumed,
    /// Waitable encountered an error
    Error,
}

/// Async operation statistics
#[derive(Debug, Default)]
struct AsyncOpStats {
    total_waits: AtomicU64,
    total_yields: AtomicU64,
    total_polls: AtomicU64,
    wait_timeouts: AtomicU64,
    wait_successes: AtomicU64,
}

impl ComponentModelAsyncOps {
    /// Create new Component Model async operations handler
    pub fn new(
        executor: Arc<Mutex<FuelAsyncExecutor>>,
        task_manager: Arc<Mutex<TaskManager>>,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        Ok(Self {
            executor,
            task_manager,
            active_waits: BoundedMap::new(),
            waitable_registry: WaitableRegistry::new()?,
            stats: AsyncOpStats::default(),
        })
    }

    /// Implement task.wait - wait for one of multiple waitables
    pub fn task_wait(
        &mut self,
        current_task: TaskId,
        waitables: WaitableSet,
        timeout_ms: Option<u64>,
    ) -> Result<TaskWaitResult> {
        self.stats.total_waits.fetch_add(1, Ordering::Relaxed);

        // Validate waitables
        if waitables.waitables.is_empty() {
            return Err(Error::validation_invalid_input("Empty waitable set"));
        }

        if waitables.waitables.len() > MAX_WAITABLES {
            return Err(Error::runtime_execution_error(
                "Too many waitables exceeds limit",
            ));
        }

        // Consume fuel for the operation
        self.consume_fuel_for_task(current_task, TASK_WAIT_FUEL)?;

        // Check if any waitable is immediately ready
        if let Some(ready_index) = self.check_waitables_ready(&waitables)? {
            self.stats.wait_successes.fetch_add(1, Ordering::Relaxed);
            return Ok(TaskWaitResult::Ready { index: ready_index });
        }

        // Create wait operation
        let wait_op = WaitOperation {
            task_id: current_task,
            waitables: waitables.clone(),
            ready_index: None,
            start_time: self.get_current_time(),
            timeout_ms: timeout_ms.unwrap_or(0),
        };

        // Register wait operation
        self.active_waits
            .insert(current_task, wait_op)
            .map_err(|_| Error::resource_limit_exceeded("Too many active wait operations"))?;

        // Mark task as waiting
        self.mark_task_waiting(current_task)?;

        Ok(TaskWaitResult::Waiting)
    }

    /// Implement task.yield - yield execution to other tasks
    pub fn task_yield(&mut self, current_task: TaskId) -> Result<()> {
        self.stats.total_yields.fetch_add(1, Ordering::Relaxed);

        // Consume fuel for yielding
        self.consume_fuel_for_task(current_task, TASK_YIELD_FUEL)?;

        // Get execution context
        let mut executor = self.executor.lock();

        // Check task state
        let task_state = executor
            .get_task_state(current_task)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

        match task_state {
            AsyncTaskState::Ready => {
                // Create yield point in execution context
                if let Some(context) = executor.get_task_execution_context_mut(current_task) {
                    context.create_yield_point(
                        0,      // Would be real instruction pointer
                        vec![], // Would capture stack
                        vec![], // Would capture locals
                    )?;
                }

                // Mark as waiting (will be immediately re-queued)
                executor.set_task_state(current_task, AsyncTaskState::Waiting)?;

                // Immediately wake the task to re-queue it
                drop(executor); // Release lock before waking
                self.wake_task(current_task)?;
            },
            _ => {
                // Task not in ready state, can't yield
                return Err(Error::invalid_state_error("Task not in ready state"));
            },
        }

        Ok(())
    }

    /// Implement task.poll - check waitables without blocking
    pub fn task_poll(
        &mut self,
        current_task: TaskId,
        waitables: &WaitableSet,
    ) -> Result<TaskPollResult> {
        self.stats.total_polls.fetch_add(1, Ordering::Relaxed);

        // Consume fuel for polling
        self.consume_fuel_for_task(current_task, TASK_POLL_FUEL)?;

        // Check each waitable
        if let Some(ready_index) = self.check_waitables_ready(waitables)? {
            Ok(TaskPollResult::Ready { index: ready_index })
        } else {
            Ok(TaskPollResult::NotReady)
        }
    }

    /// Process wait operations and wake tasks when waitables are ready
    pub fn process_wait_operations(&mut self) -> Result<usize> {
        let mut woken_count = 0;
        let current_time = self.get_current_time();

        // Check all active wait operations
        let mut completed_waits = Vec::new();

        // Collect task IDs and waitables to check (to avoid borrowing conflict)
        let wait_checks: Vec<_> = self
            .active_waits
            .iter()
            .map(|(task_id, wait_op)| {
                (
                    *task_id,
                    wait_op.waitables.clone(),
                    wait_op.timeout_ms,
                    wait_op.start_time,
                )
            })
            .collect();

        // Process each wait operation
        for (task_id, waitables, timeout_ms, start_time) in wait_checks {
            // Check for timeout
            if timeout_ms > 0 {
                let elapsed = current_time.saturating_sub(start_time);
                if elapsed >= timeout_ms {
                    // Timeout occurred
                    self.stats.wait_timeouts.fetch_add(1, Ordering::Relaxed);
                    completed_waits.push((task_id, None));
                    continue;
                }
            }

            // Check if any waitable is ready
            if let Some(ready_index) = self.check_waitables_ready(&waitables)? {
                // Update the wait operation
                if let Some(wait_op) = self.active_waits.get_mut(&task_id) {
                    wait_op.ready_index = Some(ready_index);
                }
                self.stats.wait_successes.fetch_add(1, Ordering::Relaxed);
                completed_waits.push((task_id, Some(ready_index)));
            }
        }

        // Wake completed tasks
        for (task_id, ready_index) in completed_waits {
            self.active_waits.remove(&task_id);

            // Store the result for the task
            if let Some(index) = ready_index {
                self.set_task_wait_result(task_id, index)?;
            } else {
                // Timeout - set special result
                self.set_task_timeout_result(task_id)?;
            }

            // Wake the task
            self.wake_task(task_id)?;
            woken_count += 1;
        }

        Ok(woken_count)
    }

    /// Register a future as ready
    pub fn mark_future_ready(&mut self, handle: FutureHandle) -> Result<()> {
        self.waitable_registry.mark_future_ready(handle)
    }

    /// Register a stream as having data
    pub fn mark_stream_ready(&mut self, handle: StreamHandle) -> Result<()> {
        self.waitable_registry.mark_stream_ready(handle)
    }

    /// Register a stream as writable
    pub fn mark_stream_writable(&mut self, handle: StreamHandle) -> Result<()> {
        self.waitable_registry.mark_stream_writable(handle)
    }

    /// Register a future as writable
    pub fn mark_future_writable(&mut self, handle: FutureHandle) -> Result<()> {
        self.waitable_registry.mark_future_writable(handle)
    }

    // Private helper methods

    fn check_waitables_ready(&self, waitables: &WaitableSet) -> Result<Option<u32>> {
        for (index, waitable) in waitables.waitables.iter().enumerate() {
            match waitable {
                Waitable::FutureReadable(handle) => {
                    if self.waitable_registry.is_future_ready(*handle)? {
                        return Ok(Some(index as u32));
                    }
                },
                Waitable::StreamReadable(handle) => {
                    if self.waitable_registry.is_stream_ready(*handle)? {
                        return Ok(Some(index as u32));
                    }
                },
                Waitable::FutureWritable(handle) => {
                    // For writable futures, check if they can accept a value
                    if self.waitable_registry.is_future_writable(*handle)? {
                        return Ok(Some(index as u32));
                    }
                },
                Waitable::StreamWritable(handle) => {
                    // For writable streams, check if they have space
                    if self.waitable_registry.is_stream_writable(*handle)? {
                        return Ok(Some(index as u32));
                    }
                },
            }
        }
        Ok(None)
    }

    fn consume_fuel_for_task(&self, task_id: TaskId, fuel: u64) -> Result<()> {
        let executor = self.executor.lock();
        executor.consume_fuel_for_task(task_id, fuel)
    }

    fn mark_task_waiting(&self, task_id: TaskId) -> Result<()> {
        let mut executor = self.executor.lock();
        executor.set_task_state(task_id, AsyncTaskState::Waiting)
    }

    fn wake_task(&self, task_id: TaskId) -> Result<()> {
        let mut executor = self.executor.lock();
        executor.wake_task(task_id)
    }

    fn set_task_wait_result(&self, task_id: TaskId, ready_index: u32) -> Result<()> {
        // In real implementation, would store result in task context
        Ok(())
    }

    fn set_task_timeout_result(&self, task_id: TaskId) -> Result<()> {
        // In real implementation, would store timeout result in task context
        Ok(())
    }

    fn get_current_time(&self) -> u64 {
        // In real implementation, would use deterministic time source
        0
    }
}

impl WaitableRegistry {
    fn new() -> Result<Self> {
        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        Ok(Self {
            futures_readable: BoundedMap::new(),
            futures_writable: BoundedMap::new(),
            streams_readable: BoundedMap::new(),
            streams_writable: BoundedMap::new(),
        })
    }

    fn is_future_ready(&self, handle: FutureHandle) -> Result<bool> {
        Ok(matches!(
            self.futures_readable.get(&handle),
            Some(WaitableState::Ready)
        ))
    }

    fn is_future_writable(&self, handle: FutureHandle) -> Result<bool> {
        Ok(matches!(
            self.futures_writable.get(&handle),
            Some(WaitableState::Ready)
        ))
    }

    fn is_stream_ready(&self, handle: StreamHandle) -> Result<bool> {
        Ok(matches!(
            self.streams_readable.get(&handle),
            Some(WaitableState::Ready)
        ))
    }

    fn is_stream_writable(&self, handle: StreamHandle) -> Result<bool> {
        Ok(matches!(
            self.streams_writable.get(&handle),
            Some(WaitableState::Ready)
        ))
    }

    fn mark_future_ready(&mut self, handle: FutureHandle) -> Result<()> {
        self.futures_readable.insert(handle, WaitableState::Ready).ok();
        Ok(())
    }

    fn mark_future_writable(&mut self, handle: FutureHandle) -> Result<()> {
        self.futures_writable.insert(handle, WaitableState::Ready).ok();
        Ok(())
    }

    fn mark_stream_ready(&mut self, handle: StreamHandle) -> Result<()> {
        self.streams_readable.insert(handle, WaitableState::Ready).ok();
        Ok(())
    }

    fn mark_stream_writable(&mut self, handle: StreamHandle) -> Result<()> {
        self.streams_writable.insert(handle, WaitableState::Ready).ok();
        Ok(())
    }
}

/// Result of task.wait operation
#[derive(Debug, Clone)]
pub enum TaskWaitResult {
    /// One of the waitables is ready
    Ready { index: u32 },
    /// All waitables are still pending
    Waiting,
    /// Wait timed out
    Timeout,
}

/// Result of task.poll operation
#[derive(Debug, Clone)]
pub enum TaskPollResult {
    /// One of the waitables is ready
    Ready { index: u32 },
    /// No waitables are ready
    NotReady,
}

/// Component Model async operation
#[derive(Debug, Clone)]
pub enum ComponentModelAsyncOp {
    /// task.wait operation
    TaskWait {
        waitables: WaitableSet,
        timeout_ms: Option<u64>,
    },
    /// task.yield operation
    TaskYield,
    /// task.poll operation
    TaskPoll { waitables: WaitableSet },
}

impl ComponentModelAsyncOp {
    /// Get fuel cost for this operation
    pub fn fuel_cost(&self) -> u64 {
        match self {
            Self::TaskWait { .. } => TASK_WAIT_FUEL,
            Self::TaskYield => TASK_YIELD_FUEL,
            Self::TaskPoll { .. } => TASK_POLL_FUEL,
        }
    }

    /// Check if operation can block
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::TaskWait { .. })
    }
}
