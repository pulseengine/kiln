//! Bridge between Component Model async and fuel-based executor
//!
//! This module provides integration between the Component Model's async
//! requirements and our fuel-based executor implementation.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Weak;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};
#[cfg(feature = "std")]
use std::sync::Weak;

use wrt_foundation::{
    Arc, CrateId, Mutex, collections::StaticMap as BoundedMap, safe_managed_alloc,
    verification::VerificationLevel,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::{
    task_manager::{TaskId as ComponentTaskId, TaskManager, TaskState, TaskType},
    thread_spawn_fuel::{FuelTrackedThreadManager, ThreadFuelStatus},
};

// Placeholder types when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type ComponentTaskId = u32;
#[cfg(not(feature = "component-model-threading"))]
pub type TaskManager = ();
#[cfg(not(feature = "component-model-threading"))]
pub type FuelTrackedThreadManager = ();
#[cfg(not(feature = "component-model-threading"))]
pub enum TaskType {
    AsyncOperation,
}
#[cfg(not(feature = "component-model-threading"))]
pub enum TaskState {
    Completed,
}
use crate::{
    ComponentInstanceId,
    async_::{
        fuel_async_executor::{AsyncTaskState, FuelAsyncExecutor},
        fuel_async_scheduler::SchedulingPolicy,
    },
    prelude::*,
};

/// Maximum concurrent async operations per component
const MAX_ASYNC_OPS_PER_COMPONENT: usize = 64;

/// Bridge between Component Model async and fuel executor
pub struct ComponentAsyncBridge {
    /// Fuel-based async executor
    executor: Arc<Mutex<FuelAsyncExecutor>>,
    /// Component task manager for async integration
    task_manager: Arc<Mutex<TaskManager>>,
    /// Thread manager for fuel tracking
    thread_manager: Arc<Mutex<FuelTrackedThreadManager>>,
    /// Mapping from component tasks to executor tasks
    task_mapping: BoundedMap<ComponentTaskId, u32, 1024>, /* Simple TaskId fallback */
    /// Per-component async operation limits
    component_limits: BoundedMap<ComponentInstanceId, AsyncComponentLimits, 256>,
    /// Global async fuel budget
    global_async_fuel_budget: AtomicU64,
    /// Verification level for async operations
    verification_level: VerificationLevel,
}

/// Per-component async operation limits
#[derive(Debug)]
struct AsyncComponentLimits {
    max_concurrent_tasks: usize,
    active_tasks: AtomicU64,
    fuel_budget: u64,
    fuel_consumed: AtomicU64,
    priority: Priority,
}

impl ComponentAsyncBridge {
    /// Create a new component async bridge
    pub fn new(
        task_manager: Arc<Mutex<TaskManager>>,
        thread_manager: Arc<Mutex<FuelTrackedThreadManager>>,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        let executor = Arc::new(Mutex::new(FuelAsyncExecutor::new()?));

        // Set up executor self-reference for proper waker creation
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            let weak_executor = Arc::downgrade(&executor);
            let mut exec = executor.lock();
            exec.set_self_ref(weak_executor);
        }

        Ok(Self {
            executor,
            task_manager,
            thread_manager,
            task_mapping: BoundedMap::new(),
            component_limits: BoundedMap::new(),
            global_async_fuel_budget: AtomicU64::new(u64::MAX),
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Register a component with async limits
    pub fn register_component(
        &mut self,
        component_id: ComponentInstanceId,
        max_concurrent_tasks: usize,
        fuel_budget: u64,
        priority: Priority,
    ) -> Result<()> {
        let limits = AsyncComponentLimits {
            max_concurrent_tasks,
            active_tasks: AtomicU64::new(0),
            fuel_budget,
            fuel_consumed: AtomicU64::new(0),
            priority,
        };

        let _ = self
            .component_limits
            .insert(component_id, limits)
            .map_err(|_| Error::resource_limit_exceeded("Too many registered components"))?;

        Ok(())
    }

    /// Spawn an async task for a component
    pub fn spawn_component_async<F>(
        &mut self,
        component_id: ComponentInstanceId,
        future: F,
        fuel_budget: Option<u64>,
    ) -> Result<ComponentTaskId>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        // Check component limits
        let limits = self
            .component_limits
            .get(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not registered for async"))?;

        let active = limits.active_tasks.load(Ordering::Acquire);
        if active >= limits.max_concurrent_tasks as u64 {
            return Err(Error::resource_limit_exceeded(
                "Component async task limit exceeded",
            ));
        }

        // Determine fuel budget
        let task_fuel =
            fuel_budget.unwrap_or(limits.fuel_budget / limits.max_concurrent_tasks as u64);

        // Check fuel availability
        let consumed = limits.fuel_consumed.load(Ordering::Acquire);
        if consumed + task_fuel > limits.fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Component fuel budget exceeded",
            ));
        }

        // Create component task
        let component_task_id = {
            #[cfg(feature = "component-model-threading")]
            {
                let mut tm = self.task_manager.lock();
                tm.spawn_task(TaskType::AsyncOperation, component_id.0, None)?
            }
            #[cfg(not(feature = "component-model-threading"))]
            {
                // Without threading support, use a simple counter
                component_id.0
            }
        };

        // Spawn in fuel executor
        let executor_task_id = {
            let mut exec = self.executor.lock();
            exec.spawn_task(component_id, task_fuel, limits.priority, future)?
        };

        // Update tracking - convert executor TaskId to u32 for mapping
        let executor_task_id_u32 = executor_task_id.into_inner();
        self.task_mapping
            .insert(component_task_id, executor_task_id_u32)
            .map_err(|_| Error::resource_limit_exceeded("Task mapping table full"))?;

        limits.active_tasks.fetch_add(1, Ordering::AcqRel);
        limits.fuel_consumed.fetch_add(task_fuel, Ordering::AcqRel);

        Ok(component_task_id)
    }

    /// Poll async tasks and advance execution
    pub fn poll_async_tasks(&mut self) -> Result<PollResult> {
        let mut result = PollResult::default();

        // Poll the fuel executor
        let tasks_polled = {
            let mut exec = self.executor.lock();
            exec.poll_tasks()?
        };
        result.tasks_polled = tasks_polled;

        // Update component task states based on executor state
        let mut completed_tasks = Vec::new();
        for (comp_task_id, exec_task_id) in self.task_mapping.iter() {
            let exec = self.executor.lock();
            // Convert u32 back to executor TaskId
            let exec_task_id_full = crate::async_::fuel_async_executor::TaskId::new(*exec_task_id);
            if let Some(status) = exec.get_task_status(exec_task_id_full) {
                match status.state {
                    AsyncTaskState::Completed => {
                        completed_tasks.push(*comp_task_id);
                        result.tasks_completed += 1;
                    },
                    AsyncTaskState::Failed => {
                        completed_tasks.push(*comp_task_id);
                        result.tasks_failed += 1;
                    },
                    AsyncTaskState::Waiting => {
                        result.tasks_waiting += 1;
                    },
                    _ => {},
                }
            }
        }

        // Clean up completed tasks
        for task_id in completed_tasks {
            self.cleanup_component_task(task_id)?;
        }

        // Collect fuel statistics
        let exec = self.executor.lock();
        let fuel_status = exec.get_global_fuel_status();
        result.total_fuel_consumed = fuel_status.consumed;
        result.fuel_remaining = fuel_status.remaining();

        Ok(result)
    }

    /// Check if a component task is ready
    pub fn is_task_ready(&self, task_id: ComponentTaskId) -> Result<bool> {
        if let Some(exec_task_id) = self.task_mapping.get(&task_id) {
            let exec = self.executor.lock();
            // Convert u32 back to executor TaskId
            let exec_task_id_full = crate::async_::fuel_async_executor::TaskId::new(*exec_task_id);
            if let Some(status) = exec.get_task_status(exec_task_id_full) {
                Ok(status.state == AsyncTaskState::Ready)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Get component async statistics
    pub fn get_component_stats(
        &self,
        component_id: ComponentInstanceId,
    ) -> Result<ComponentAsyncStats> {
        let limits = self
            .component_limits
            .get(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not registered"))?;

        Ok(ComponentAsyncStats {
            component_id,
            active_tasks: limits.active_tasks.load(Ordering::Acquire),
            max_tasks: limits.max_concurrent_tasks as u64,
            fuel_consumed: limits.fuel_consumed.load(Ordering::Acquire),
            fuel_budget: limits.fuel_budget,
        })
    }

    /// Clean up a completed component task
    fn cleanup_component_task(&mut self, task_id: ComponentTaskId) -> Result<()> {
        // Remove from mapping
        if let Some(exec_task_id) = self.task_mapping.remove(&task_id) {
            // Get component ID from task manager
            let component_id = {
                #[cfg(feature = "component-model-threading")]
                {
                    let tm = self.task_manager.lock();
                    if let Some(task) = tm.get_task(task_id) {
                        ComponentInstanceId::new(task.context.component_instance)
                    } else {
                        return Ok(());
                    }
                }
                #[cfg(not(feature = "component-model-threading"))]
                {
                    // Without threading support, derive from task_id
                    ComponentInstanceId::new(task_id)
                }
            };

            // Update component limits
            if let Some(limits) = self.component_limits.get(&component_id) {
                limits.active_tasks.fetch_sub(1, Ordering::AcqRel);

                // Return unused fuel
                let exec = self.executor.lock();
                // Convert u32 back to executor TaskId
                let exec_task_id_full =
                    crate::async_::fuel_async_executor::TaskId::new(exec_task_id);
                if let Some(status) = exec.get_task_status(exec_task_id_full) {
                    let unused_fuel = status.fuel_budget - status.fuel_consumed;
                    if unused_fuel > 0 {
                        limits.fuel_consumed.fetch_sub(unused_fuel, Ordering::AcqRel);
                    }
                }
            }

            // Update task manager state
            #[cfg(feature = "component-model-threading")]
            {
                let mut tm = self.task_manager.lock();
                if let Some(task) = tm.get_task_mut(task_id) {
                    task.state = TaskState::Completed;
                }
            }
        }

        Ok(())
    }

    /// Set global async fuel budget
    pub fn set_global_fuel_budget(&mut self, budget: u64) -> Result<()> {
        self.global_async_fuel_budget.store(budget, Ordering::SeqCst);
        let exec = self.executor.lock();
        exec.set_global_fuel_limit(budget);
        Ok(())
    }

    /// Get executor polling statistics
    pub fn get_polling_stats(
        &self,
    ) -> Result<crate::async_::fuel_async_executor::PollingStatistics> {
        let exec = self.executor.lock();
        Ok(exec.get_polling_stats())
    }
}

/// Result of polling async tasks
#[derive(Debug, Default)]
pub struct PollResult {
    pub tasks_polled: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub tasks_waiting: usize,
    pub total_fuel_consumed: u64,
    pub fuel_remaining: u64,
}

/// Component async statistics
#[derive(Debug, Clone)]
pub struct ComponentAsyncStats {
    pub component_id: ComponentInstanceId,
    pub active_tasks: u64,
    pub max_tasks: u64,
    pub fuel_consumed: u64,
    pub fuel_budget: u64,
}

impl ComponentAsyncStats {
    pub fn utilization_percentage(&self) -> f64 {
        if self.max_tasks == 0 {
            0.0
        } else {
            (self.active_tasks as f64 / self.max_tasks as f64) * 100.0
        }
    }

    pub fn fuel_usage_percentage(&self) -> f64 {
        if self.fuel_budget == 0 {
            0.0
        } else {
            (self.fuel_consumed as f64 / self.fuel_budget as f64) * 100.0
        }
    }
}
