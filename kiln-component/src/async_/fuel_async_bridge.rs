//! Async function bridge using TimeBoundedContext for fuel-based execution
//!
//! This module provides a bridge between async Component Model functions
//! and the existing fuel/time-bounded execution system.

use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};

use wrt_foundation::{
    CrateId,
    collections::StaticMap as BoundedMap,
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    verification::VerificationLevel,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{
    ComponentInstanceId,
    async_::{
        fuel_async_executor::{AsyncTaskState, AsyncTaskStatus, FuelAsyncExecutor},
        fuel_async_scheduler::{FuelAsyncScheduler, SchedulingPolicy},
    },
    execution::{TimeBoundedConfig, TimeBoundedContext, TimeBoundedOutcome, run_with_time_bounds},
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum number of concurrent async bridges
const MAX_ASYNC_BRIDGES: usize = 64;

/// Fuel costs for bridge operations
const ASYNC_BRIDGE_SETUP_FUEL: u64 = 25;
const ASYNC_BRIDGE_POLL_FUEL: u64 = 10;
const ASYNC_BRIDGE_CLEANUP_FUEL: u64 = 15;

/// Bridge between async functions and time-bounded execution
pub struct FuelAsyncBridge {
    /// Async executor for managing tasks
    executor: FuelAsyncExecutor,
    /// Scheduler for task ordering
    scheduler: FuelAsyncScheduler,
    /// Active bridges indexed by task ID
    active_bridges: BoundedMap<TaskId, AsyncBridgeContext, MAX_ASYNC_BRIDGES>,
    /// Global bridge configuration
    default_config: AsyncBridgeConfig,
    /// Verification level for bridge operations
    verification_level: VerificationLevel,
}

/// Configuration for async bridges
#[derive(Debug, Clone)]
pub struct AsyncBridgeConfig {
    /// Default fuel budget for async tasks
    pub default_fuel_budget: u64,
    /// Default time limit for async operations
    pub default_time_limit_ms: Option<u64>,
    /// Default priority for async tasks
    pub default_priority: Priority,
    /// Scheduling policy for async tasks
    pub scheduling_policy: SchedulingPolicy,
    /// Whether to allow fuel extension
    pub allow_fuel_extension: bool,
    /// Fuel check interval
    pub fuel_check_interval: u64,
}

impl Default for AsyncBridgeConfig {
    fn default() -> Self {
        Self {
            default_fuel_budget: 10000,
            default_time_limit_ms: Some(5000), // 5 seconds
            default_priority: 128,             // Normal priority
            scheduling_policy: SchedulingPolicy::Cooperative,
            allow_fuel_extension: false,
            fuel_check_interval: 1000,
        }
    }
}

/// Context for an individual async bridge
#[derive(Debug)]
pub struct AsyncBridgeContext {
    pub task_id: TaskId,
    pub component_id: ComponentInstanceId,
    pub time_bounded_context: TimeBoundedContext,
    pub fuel_consumed: AtomicU64,
    pub bridge_state: AsyncBridgeState,
    pub result_ready: AtomicBool,
}

/// State of an async bridge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncBridgeState {
    /// Bridge is initializing
    Initializing,
    /// Bridge is actively executing
    Executing,
    /// Bridge is waiting for async operation
    Waiting,
    /// Bridge has completed successfully
    Completed,
    /// Bridge encountered an error
    Failed,
    /// Bridge was cancelled
    Cancelled,
}

impl FuelAsyncBridge {
    /// Create a new async function bridge
    pub fn new(config: AsyncBridgeConfig, verification_level: VerificationLevel) -> Result<Self> {
        let executor = FuelAsyncExecutor::new()?;
        let scheduler = FuelAsyncScheduler::new(config.scheduling_policy, verification_level)?;
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        Ok(Self {
            executor,
            scheduler,
            active_bridges: BoundedMap::new(),
            default_config: config,
            verification_level,
        })
    }

    /// Execute an async function with fuel and time bounds
    pub fn execute_async_function<F, T>(
        &mut self,
        component_id: ComponentInstanceId,
        future: F,
        config: Option<AsyncBridgeConfig>,
    ) -> Result<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static + Default,
    {
        let bridge_config = config.unwrap_or_else(|| self.default_config.clone());

        record_global_operation(OperationType::FunctionCall, self.verification_level);

        // Create time-bounded configuration
        let time_config = TimeBoundedConfig {
            time_limit_ms: bridge_config.default_time_limit_ms,
            allow_extension: bridge_config.allow_fuel_extension,
            fuel_limit: Some(bridge_config.default_fuel_budget),
        };

        // Execute with time bounds
        let (result, outcome) = run_with_time_bounds(time_config, |time_context| {
            // Create async task
            let task_future = self.create_bridged_future(component_id, future, time_context)?;

            // Spawn task in executor
            let task_id = self.executor.spawn_task(
                component_id,
                bridge_config.default_fuel_budget,
                bridge_config.default_priority,
                task_future,
            )?;

            // Add to scheduler - convert TaskId to u32
            self.scheduler.add_task(
                task_id.0,
                component_id,
                bridge_config.default_priority,
                bridge_config.default_fuel_budget,
                bridge_config.default_time_limit_ms.map(Duration::from_millis),
            )?;

            // Create bridge context
            let bridge_context = AsyncBridgeContext {
                task_id: task_id.0,
                component_id,
                time_bounded_context: TimeBoundedContext::new(TimeBoundedConfig {
                    time_limit_ms: bridge_config.default_time_limit_ms,
                    allow_extension: bridge_config.allow_fuel_extension,
                    fuel_limit: Some(bridge_config.default_fuel_budget),
                }),
                fuel_consumed: AtomicU64::new(ASYNC_BRIDGE_SETUP_FUEL),
                bridge_state: AsyncBridgeState::Initializing,
                result_ready: AtomicBool::new(false),
            };

            self.active_bridges
                .insert(task_id.0, bridge_context)
                .map_err(|_| Error::resource_limit_exceeded("Too many active async bridges"))?;

            // Run the async execution loop
            self.run_async_execution_loop(task_id.0, time_context)
        });

        match outcome {
            TimeBoundedOutcome::Completed => result,
            TimeBoundedOutcome::TimedOut => {
                Err(Error::runtime_execution_error("Execution timed out"))
            },
            TimeBoundedOutcome::Terminated => Err(Error::new(
                ErrorCategory::Runtime,
                codes::EXECUTION_LIMIT_EXCEEDED,
                "Operation failed",
            )),
            TimeBoundedOutcome::Error(e) => Err(Error::runtime_execution_error(
                "Async function execution error",
            )),
        }
    }

    /// Execute multiple async functions concurrently
    pub fn execute_concurrent_async<F, T>(
        &mut self,
        component_id: ComponentInstanceId,
        futures: Vec<F>,
        config: Option<AsyncBridgeConfig>,
    ) -> Result<Vec<Result<T>>>
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static + Default,
    {
        let bridge_config = config.unwrap_or_else(|| self.default_config.clone());
        let mut task_ids = Vec::new();

        // Spawn all futures as tasks
        for future in futures {
            // Create a time bounded context for this task
            let mut time_context = TimeBoundedContext::new(TimeBoundedConfig {
                time_limit_ms: bridge_config.default_time_limit_ms,
                allow_extension: bridge_config.allow_fuel_extension,
                fuel_limit: Some(bridge_config.default_fuel_budget),
            });

            let task_id = self.executor.spawn_task(
                component_id,
                bridge_config.default_fuel_budget,
                bridge_config.default_priority,
                self.create_bridged_future(component_id, future, &mut time_context)?,
            )?;

            self.scheduler.add_task(
                task_id.0,
                component_id,
                bridge_config.default_priority,
                bridge_config.default_fuel_budget,
                bridge_config.default_time_limit_ms.map(Duration::from_millis),
            )?;

            task_ids.push(task_id);
        }

        // Execute all tasks concurrently
        let mut results = Vec::new();
        for task_id in task_ids {
            // This is a simplified version - real implementation would use proper
            // concurrent execution
            match self.executor.get_task_status(task_id) {
                Some(status) => match status.state {
                    AsyncTaskState::Completed => {
                        results.push(Ok(self.get_task_result(task_id.0)?));
                    },
                    AsyncTaskState::Failed => {
                        results.push(Err(Error::runtime_execution_error("Async task failed")));
                    },
                    _ => {
                        results.push(Err(Error::runtime_execution_error("Task execution error")));
                    },
                },
                None => {
                    results.push(Err(Error::new(
                        ErrorCategory::Runtime,
                        codes::TASK_NOT_FOUND,
                        "Operation failed",
                    )));
                },
            }
        }

        Ok(results)
    }

    /// Get bridge statistics
    pub fn get_bridge_statistics(&self) -> AsyncBridgeStatistics {
        let mut total_fuel_consumed = 0;
        let mut active_bridges = 0;
        let mut completed_bridges = 0;
        let mut failed_bridges = 0;

        for context in self.active_bridges.values() {
            total_fuel_consumed += context.fuel_consumed.load(Ordering::Acquire);
            match context.bridge_state {
                AsyncBridgeState::Executing | AsyncBridgeState::Waiting => active_bridges += 1,
                AsyncBridgeState::Completed => completed_bridges += 1,
                AsyncBridgeState::Failed => failed_bridges += 1,
                _ => {},
            }
        }

        let executor_stats = self.executor.get_global_fuel_status();
        let scheduler_stats = self.scheduler.get_statistics();

        AsyncBridgeStatistics {
            total_bridges: self.active_bridges.len(),
            active_bridges,
            completed_bridges,
            failed_bridges,
            total_fuel_consumed,
            executor_fuel_status: executor_stats,
            scheduler_statistics: scheduler_stats,
        }
    }

    /// Shutdown all async bridges gracefully
    pub fn shutdown(&mut self) -> Result<()> {
        // Cancel all active bridges
        for (task_id, context) in self.active_bridges.iter_mut() {
            if matches!(
                context.bridge_state,
                AsyncBridgeState::Executing | AsyncBridgeState::Waiting
            ) {
                context.bridge_state = AsyncBridgeState::Cancelled;
            }
        }

        // Shutdown executor and scheduler
        self.executor.shutdown()?;
        self.active_bridges.clear();

        Ok(())
    }

    // Private helper methods

    fn create_bridged_future<F, T>(
        &self,
        component_id: ComponentInstanceId,
        future: F,
        time_context: &mut TimeBoundedContext,
    ) -> Result<Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>>
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        // Consume fuel for bridge setup
        #[cfg(not(feature = "std"))]
        time_context.consume_fuel(ASYNC_BRIDGE_SETUP_FUEL);

        record_global_operation(OperationType::FunctionCall, self.verification_level);

        // Create a wrapper future that integrates with the bridge
        let bridged_future = async move {
            // Execute the original future
            match future.await {
                Ok(_) => {
                    record_global_operation(
                        OperationType::FunctionCall,
                        VerificationLevel::Standard,
                    );
                    Ok(())
                },
                Err(e) => {
                    record_global_operation(OperationType::Other, VerificationLevel::Standard);
                    Err(e)
                },
            }
        };

        Ok(Box::pin(bridged_future))
    }

    fn run_async_execution_loop<T>(
        &mut self,
        task_id: TaskId,
        time_context: &mut TimeBoundedContext,
    ) -> Result<T>
    where
        T: Default,
    {
        let mut poll_count = 0;
        const MAX_POLLS: usize = 1000; // Prevent infinite loops

        // Convert u32 TaskId to executor's TaskId struct
        let executor_task_id = crate::async_::fuel_async_executor::TaskId::new(task_id);

        loop {
            // Check time bounds
            time_context.check_time_bounds()?;

            // Consume fuel for polling
            #[cfg(not(feature = "std"))]
            time_context.consume_fuel(ASYNC_BRIDGE_POLL_FUEL);

            // Poll tasks
            let polled = self.executor.poll_tasks()?;
            poll_count += 1;

            // Check task status
            if let Some(status) = self.executor.get_task_status(executor_task_id) {
                match status.state {
                    AsyncTaskState::Completed => {
                        self.cleanup_bridge(task_id)?;
                        return Ok(T::default()); // Simplified return
                    },
                    AsyncTaskState::Failed => {
                        self.cleanup_bridge(task_id)?;
                        return Err(Error::runtime_execution_error(
                            "Async task failed during execution",
                        ));
                    },
                    AsyncTaskState::FuelExhausted => {
                        self.cleanup_bridge(task_id)?;
                        return Err(Error::runtime_execution_error("Fuel exhausted"));
                    },
                    _ => {
                        // Continue execution
                    },
                }
            }

            // Safety check to prevent infinite loops
            if poll_count >= MAX_POLLS {
                self.cleanup_bridge(task_id)?;
                return Err(Error::new(
                    ErrorCategory::Runtime,
                    codes::EXECUTION_LIMIT_EXCEEDED,
                    "Operation failed",
                ));
            }

            // If no tasks were polled, yield briefly
            if polled == 0 {
                #[cfg(not(feature = "std"))]
                time_context.consume_fuel(ASYNC_BRIDGE_POLL_FUEL);
            }
        }
    }

    fn cleanup_bridge(&mut self, task_id: TaskId) -> Result<()> {
        record_global_operation(OperationType::CollectionRemove, self.verification_level);

        // Remove from scheduler
        self.scheduler.remove_task(task_id)?;

        // Remove bridge context
        if let Some(mut context) = self.active_bridges.remove(&task_id) {
            context.fuel_consumed.fetch_add(ASYNC_BRIDGE_CLEANUP_FUEL, Ordering::AcqRel);
            context.bridge_state = AsyncBridgeState::Completed;
        }

        Ok(())
    }

    fn get_task_result<T>(&self, _task_id: TaskId) -> Result<T>
    where
        T: Default,
    {
        // Simplified result extraction - real implementation would store results
        Ok(T::default())
    }
}

/// Statistics for async bridges
#[derive(Debug, Clone)]
pub struct AsyncBridgeStatistics {
    pub total_bridges: usize,
    pub active_bridges: usize,
    pub completed_bridges: usize,
    pub failed_bridges: usize,
    pub total_fuel_consumed: u64,
    pub executor_fuel_status: super::fuel_async_executor::GlobalAsyncFuelStatus,
    pub scheduler_statistics: super::fuel_async_scheduler::SchedulingStatistics,
}

impl AsyncBridgeStatistics {
    pub fn success_rate(&self) -> f64 {
        if self.total_bridges > 0 {
            (self.completed_bridges as f64 / self.total_bridges as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn average_fuel_per_bridge(&self) -> f64 {
        if self.total_bridges > 0 {
            self.total_fuel_consumed as f64 / self.total_bridges as f64
        } else {
            0.0
        }
    }
}
