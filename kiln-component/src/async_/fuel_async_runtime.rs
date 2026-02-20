//! Main async runtime for fuel-based WebAssembly execution
//!
//! This module provides the top-level runtime that orchestrates async task
//! execution with fuel tracking and ASIL compliance.

use crate::{
    async_::{
        fuel_async_executor::{FuelAsyncExecutor, FuelAsyncTask, AsyncTaskState, ExecutionContext, ASILExecutionMode},
        fuel_resource_cleanup::{GlobalCleanupManager, TaskCleanupGuard},
        fuel_error_context::{AsyncErrorKind, async_error, ContextualError},
    },
    types::{ComponentInstance, ComponentInstanceId},
    task_manager::TaskId,
    prelude::*,
};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};
use wrt_foundation::{
    BoundedVec,
    bounded_collections::BoundedMap,
    operations::{record_global_operation, Type as OperationType, global_fuel_consumed},
    verification::VerificationLevel,
    safe_managed_alloc, CrateId,
    Arc, sync::Mutex,
};

/// Maximum number of components in the runtime
const MAX_COMPONENTS: usize = 64;

/// Maximum number of concurrent tasks across all components
const MAX_RUNTIME_TASKS: usize = 512;

/// Runtime polling batch size
const RUNTIME_POLLING_BATCH: usize = 32;

/// Fuel costs for runtime operations
const RUNTIME_SPAWN_FUEL: u64 = 50;
const RUNTIME_POLL_FUEL: u64 = 10;
const RUNTIME_CLEANUP_FUEL: u64 = 20;

/// Main async runtime for WebAssembly components
pub struct FuelAsyncRuntime {
    /// Core async executor
    executor: Arc<Mutex<FuelAsyncExecutor>>,
    /// Component registry
    components: BoundedMap<ComponentInstanceId, Arc<ComponentInstance>, MAX_COMPONENTS>,
    /// Global fuel budget for the runtime
    global_fuel_budget: u64,
    /// Total fuel consumed across all operations
    total_fuel_consumed: u64,
    /// Runtime state
    state: RuntimeState,
    /// Global cleanup manager
    cleanup_manager: Arc<Mutex<GlobalCleanupManager>>,
    /// Runtime statistics
    stats: RuntimeStatistics,
    /// Verification level for runtime operations
    verification_level: VerificationLevel,
    /// Task completion results
    task_results: BoundedMap<TaskId, TaskResult, MAX_RUNTIME_TASKS>,
}

/// Runtime execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Runtime is initializing
    Initializing,
    /// Runtime is running and processing tasks
    Running,
    /// Runtime is shutting down gracefully
    ShuttingDown,
    /// Runtime has stopped
    Stopped,
    /// Runtime encountered an error
    Error,
}

/// Task execution result
#[derive(Debug, Clone)]
pub enum TaskResult {
    /// Task completed successfully with return values
    Completed(Vec<u8>),
    /// Task failed with error
    Failed(ContextualError),
    /// Task was cancelled
    Cancelled,
}

/// Runtime execution statistics
#[derive(Debug, Default)]
pub struct RuntimeStatistics {
    /// Total tasks spawned
    pub total_spawned: u64,
    /// Total tasks completed
    pub total_completed: u64,
    /// Total tasks failed
    pub total_failed: u64,
    /// Total tasks cancelled
    pub total_cancelled: u64,
    /// Total polling cycles
    pub polling_cycles: u64,
    /// Total fuel consumed
    pub total_fuel_consumed: u64,
    /// Components registered
    pub components_registered: u32,
}

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Global fuel budget
    pub global_fuel_budget: u64,
    /// Default verification level
    pub verification_level: VerificationLevel,
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Polling batch size
    pub polling_batch_size: usize,
    /// Enable cleanup on shutdown
    pub enable_cleanup: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            global_fuel_budget: 1_000_000, // 1M fuel units
            verification_level: VerificationLevel::Basic,
            max_concurrent_tasks: MAX_RUNTIME_TASKS,
            polling_batch_size: RUNTIME_POLLING_BATCH,
            enable_cleanup: true,
        }
    }
}

impl FuelAsyncRuntime {
    /// Create a new async runtime with configuration
    pub fn new(config: RuntimeConfig) -> Result<Self> {
        let provider = safe_managed_alloc!(16384, CrateId::Component)?;
        
        let executor = Arc::new(Mutex::new(FuelAsyncExecutor::new(
            config.global_fuel_budget,
            config.verification_level,
        )?));
        
        let components = BoundedMap::new();
        let task_results = BoundedMap::new();
        
        let cleanup_manager = Arc::new(Mutex::new(
            GlobalCleanupManager::new(config.global_fuel_budget / 10)?
        ));
        
        Ok(Self {
            executor,
            components,
            global_fuel_budget: config.global_fuel_budget,
            total_fuel_consumed: 0,
            state: RuntimeState::Initializing,
            cleanup_manager,
            stats: RuntimeStatistics::default(),
            verification_level: config.verification_level,
            task_results,
        })
    }
    
    /// Register a component instance with the runtime
    pub fn register_component(
        &mut self,
        component_id: ComponentInstanceId,
        component: Arc<ComponentInstance>,
    ) -> Result<()> {
        // Check fuel budget
        self.consume_runtime_fuel(RUNTIME_SPAWN_FUEL)?;
        
        // Register component
        self.components.insert(component_id, component)?;
        self.stats.components_registered += 1;
        
        // Register with cleanup manager
        self.cleanup_manager.lock().get_or_create_manager(component_id)?;
        
        record_global_operation(OperationType::Other)?;
        Ok(())
    }
    
    /// Spawn an async task to execute a component function
    pub fn spawn_task(
        &mut self,
        component_id: ComponentInstanceId,
        function_name: &str,
        params: Vec<wrt_foundation::Value>,
        fuel_budget: u64,
        asil_mode: ASILExecutionMode,
    ) -> Result<TaskId> {
        // Check runtime state
        if self.state != RuntimeState::Running && self.state != RuntimeState::Initializing {
            return Err(Error::async_executor_state_violation("Runtime not in running state"));
        }
        
        // Check fuel budget
        self.consume_runtime_fuel(RUNTIME_SPAWN_FUEL)?;
        
        // Get component instance
        let component = self.components.get(&component_id).ok_or_else(|| {
            Error::component_resource_lifecycle_error("Component not registered")
        })?.clone();
        
        // Resolve function export
        let function_index = self.resolve_function_export(&component, function_name)?;
        
        // Create execution context
        let mut execution_context = ExecutionContext::new(asil_mode);
        execution_context.set_component_instance(component.clone());
        execution_context.current_function_index = function_index;
        execution_context.function_params = params;
        
        // Create and spawn task
        let task_id = self.executor.lock().spawn_async_task(
            component_id,
            fuel_budget,
            self.verification_level,
            execution_context,
        )?;
        
        // Register task for cleanup
        self.cleanup_manager.lock().register_task(
            task_id,
            component_id,
            self.verification_level,
        )?;
        
        self.stats.total_spawned += 1;
        Ok(task_id)
    }
    
    /// Run the runtime until all tasks complete or fuel is exhausted
    pub fn run(&mut self) -> Result<RuntimeExecutionSummary> {
        self.state = RuntimeState::Running;
        let start_fuel = global_fuel_consumed);
        
        while self.has_active_tasks()? {
            // Check global fuel budget
            if self.total_fuel_consumed >= self.global_fuel_budget {
                self.state = RuntimeState::Error;
                return Err(Error::async_fuel_exhausted("Global runtime fuel budget exhausted";
"            }
            
            // Poll tasks
            let polled = self.poll_tasks()?;
            self.stats.polling_cycles += 1;
            
            // Update fuel consumption
            let current_fuel = global_fuel_consumed);
            self.total_fuel_consumed = current_fuel.saturating_sub(start_fuel;
            self.stats.total_fuel_consumed = self.total_fuel_consumed;
            
            // If no tasks were polled and we have active tasks, wait briefly
            if polled == 0 && self.has_active_tasks()? {
                self.wait_for_tasks()?;
            }
        }
        
        self.state = RuntimeState::Stopped;
        
        Ok(RuntimeExecutionSummary {
            tasks_spawned: self.stats.total_spawned,
            tasks_completed: self.stats.total_completed,
            tasks_failed: self.stats.total_failed,
            tasks_cancelled: self.stats.total_cancelled,
            total_fuel_consumed: self.total_fuel_consumed,
            polling_cycles: self.stats.polling_cycles,
        })
    }
    
    /// Run until a specific task completes
    pub fn run_until_task_complete(&mut self, task_id: TaskId) -> Result<TaskResult> {
        self.state = RuntimeState::Running;
        
        while !self.is_task_complete(task_id)? {
            // Check if task still exists
            if !self.executor.lock().has_task(task_id) {
                return Ok(TaskResult::Failed(async_error(
                    AsyncErrorKind::TaskCancelled,
                    0,
                    Some(task_id),
                    "Task not found during execution",
                )?;
            }
            
            // Poll tasks
            self.poll_tasks()?;
            
            // Check global fuel budget
            if self.total_fuel_consumed >= self.global_fuel_budget {
                return Ok(TaskResult::Failed(async_error(
                    AsyncErrorKind::FuelExhausted,
                    0,
                    Some(task_id),
                    "Global fuel budget exhausted",
                )?;
            }
        }
        
        // Get task result
        self.get_task_result(task_id)
    }
    
    /// Poll ready tasks and process results
    fn poll_tasks(&mut self) -> Result<u32> {
        self.consume_runtime_fuel(RUNTIME_POLL_FUEL)?;
        
        let polled = self.executor.lock().poll_ready_tasks(RUNTIME_POLLING_BATCH as u32)?;
        
        // Collect completed task results
        self.collect_completed_tasks()?;
        
        Ok(polled)
    }
    
    /// Collect results from completed tasks
    fn collect_completed_tasks(&mut self) -> Result<()> {
        let mut completed_tasks = Vec::new();
        
        // Get completed tasks from executor
        {
            let executor = self.executor.lock();
            for (task_id, task) in executor.get_tasks() {
                match task.state {
                    AsyncTaskState::Completed => {
                        completed_tasks.push((*task_id, TaskResult::Completed(vec![])));
                        self.stats.total_completed += 1;
                    },
                    AsyncTaskState::Failed => {
                        let error = async_error(
                            AsyncErrorKind::TaskCancelled,
                            task.component_id,
                            Some(*task_id),
                            "Task execution failed",
                        )?;
                        completed_tasks.push((*task_id, TaskResult::Failed(error)));
                        self.stats.total_failed += 1;
                    },
                    AsyncTaskState::Cancelled => {
                        completed_tasks.push((*task_id, TaskResult::Cancelled));
                        self.stats.total_cancelled += 1;
                    },
                    _ => {} // Task still running
                }
            }
        }
        
        // Store results and cleanup
        for (task_id, result) in completed_tasks {
            self.task_results.insert(task_id, result)?;
            
            // Run cleanup for completed task
            let _cleanup_errors = self.cleanup_manager.lock().cancel_task(task_id)?;
            
            // Remove task from executor
            self.executor.lock().remove_task(task_id)?;
        }
        
        Ok(())
    }
    
    /// Wait for tasks to become ready
    fn wait_for_tasks(&mut self) -> Result<()> {
        // In a real implementation, this would use proper async waiting
        // For now, just consume a small amount of fuel to simulate waiting
        self.consume_runtime_fuel(1)?;
        Ok(())
    }
    
    /// Check if there are active tasks
    fn has_active_tasks(&self) -> Result<bool> {
        Ok(self.executor.lock().has_tasks())
    }
    
    /// Check if a specific task is complete
    fn is_task_complete(&self, task_id: TaskId) -> Result<bool> {
        if let Some(_result) = self.task_results.get(&task_id) {
            return Ok(true);
        }
        
        if let Some(task) = self.executor.lock().get_task(task_id) {
            match task.state {
                AsyncTaskState::Completed | AsyncTaskState::Failed | AsyncTaskState::Cancelled => {
                    Ok(true)
                },
                _ => Ok(false),
            }
        } else {
            Ok(true) // Task doesn't exist, consider it complete
        }
    }
    
    /// Get the result of a completed task
    pub fn get_task_result(&self, task_id: TaskId) -> Result<TaskResult> {
        if let Some(result) = self.task_results.get(&task_id) {
            Ok(result.clone())
        } else {
            Err(Error::async_task_execution_failed("Task result not available"))
        }
    }
    
    /// Cancel a task
    pub fn cancel_task(&mut self, task_id: TaskId) -> Result<()> {
        self.consume_runtime_fuel(RUNTIME_CLEANUP_FUEL)?;
        
        // Cancel in executor
        self.executor.lock().cancel_task(task_id)?;
        
        // Run cleanup
        self.cleanup_manager.lock().cancel_task(task_id)?;
        
        // Store cancelled result
        self.task_results.insert(task_id, TaskResult::Cancelled)?;
        self.stats.total_cancelled += 1;
        
        Ok(())
    }
    
    /// Shutdown the runtime gracefully
    pub fn shutdown(&mut self) -> Result<()> {
        self.state = RuntimeState::ShuttingDown;
        
        // Cancel all active tasks
        let active_tasks: Vec<TaskId> = self.executor.lock().get_active_task_ids();
        for task_id in active_tasks {
            let _ = self.cancel_task(task_id);
        }
        
        // Run global cleanup
        self.cleanup_manager.lock().run_cleanup()?;
        
        self.state = RuntimeState::Stopped;
        Ok(())
    }
    
    /// Resolve function export to function index
    fn resolve_function_export(
        &self,
        component: &ComponentInstance,
        function_name: &str,
    ) -> Result<u32> {
        // Check if the function export exists
        if !component.has_function_export(function_name) {
            return Err(Error::component_instantiation_runtime_error("Function not found in component exports"));
        }

        // Get the function index for the export
        match component.get_function_index(function_name) {
            Some(function_index) => {
                // Validate that the function index is reasonable
                if function_index >= 65536 {
                    return Err(Error::component_instantiation_runtime_error("Function index out of range"));
"                }
                Ok(function_index)
            },
            None => Err(Error::component_instantiation_runtime_error("Could not resolve function index")),
        }
    }
    
    /// Consume runtime-level fuel
    fn consume_runtime_fuel(&mut self, amount: u64) -> Result<()> {
        let adjusted = OperationType::fuel_cost_for_operation(
            OperationType::Other,
            self.verification_level,
        )?;
        
        let total_cost = amount.saturating_add(adjusted);
        
        if self.total_fuel_consumed.saturating_add(total_cost) > self.global_fuel_budget {
            return Err(Error::async_fuel_exhausted("Runtime fuel budget exceeded"));
        }
        
        self.total_fuel_consumed = self.total_fuel_consumed.saturating_add(total_cost);
        record_global_operation(OperationType::Other)?;
        
        Ok(())
    }
    
    /// Get runtime statistics
    pub fn stats(&self) -> &RuntimeStatistics {
        &self.stats
    }
    
    /// Get current runtime state
    pub fn state(&self) -> RuntimeState {
        self.state
    }
}

/// Summary of runtime execution
#[derive(Debug, Clone)]
pub struct RuntimeExecutionSummary {
    /// Total tasks spawned
    pub tasks_spawned: u64,
    /// Total tasks completed successfully
    pub tasks_completed: u64,
    /// Total tasks that failed
    pub tasks_failed: u64,
    /// Total tasks cancelled
    pub tasks_cancelled: u64,
    /// Total fuel consumed during execution
    pub total_fuel_consumed: u64,
    /// Total polling cycles performed
    pub polling_cycles: u64,
}

/// Convenience wrapper for simple async execution
pub struct SimpleAsyncExecutor {
    runtime: FuelAsyncRuntime,
}

impl SimpleAsyncExecutor {
    /// Create a simple executor with default configuration
    pub fn new() -> Result<Self> {
        let runtime = FuelAsyncRuntime::new(RuntimeConfig::default())?;
        Ok(Self { runtime })
    }
    
    /// Execute a single async function and return the result
    pub fn execute_async_function(
        &mut self,
        component: Arc<ComponentInstance>,
        function_name: &str,
        params: Vec<wrt_foundation::Value>,
        fuel_budget: u64,
    ) -> Result<Vec<u8>> {
        // Register component
        let component_id = 1; // Simple ID for single component
        self.runtime.register_component(component_id, component)?;
        
        // Spawn task
        let task_id = self.runtime.spawn_task(
            component_id,
            function_name,
            params,
            fuel_budget,
            ASILExecutionMode::A { error_detection: true },
        )?;
        
        // Run until complete
        match self.runtime.run_until_task_complete(task_id)? {
            TaskResult::Completed(result) => Ok(result),
            TaskResult::Failed(error) => Err(Error::from(error)),
            TaskResult::Cancelled => Err(Error::async_task_execution_failed("Task was cancelled")),
        }

}
