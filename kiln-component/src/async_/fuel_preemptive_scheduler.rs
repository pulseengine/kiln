//! Preemptive fuel-based scheduler for async tasks
//!
//! This module provides advanced scheduling with preemption based on fuel
//! consumption and priority levels, essential for ASIL-B real-time guarantees.

use core::{
    cmp::Ordering as CmpOrdering,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

use wrt_foundation::{
    CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
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
        fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
        fuel_async_scheduler::{ScheduledTask, SchedulingPolicy},
        fuel_priority_inheritance::{FuelPriorityInheritanceProtocol, ResourceId},
    },
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum number of tasks in preemptive scheduler
const MAX_PREEMPTIVE_TASKS: usize = 128;

/// Maximum number of priority levels
const MAX_PRIORITY_LEVELS: usize = 8;

/// Fuel quantum for time slicing
const DEFAULT_FUEL_QUANTUM: u64 = 1000;

/// Fuel costs for preemptive operations
const PREEMPTION_FUEL: u64 = 20;
const CONTEXT_SWITCH_FUEL: u64 = 15;
const PRIORITY_BOOST_FUEL: u64 = 10;
const AGING_CHECK_FUEL: u64 = 5;

/// Preemptive scheduler with fuel-based time slicing
pub struct FuelPreemptiveScheduler {
    /// Tasks organized by priority level
    priority_queues: BoundedVec<TaskPriorityQueue, MAX_PRIORITY_LEVELS>,
    /// Currently running task
    current_task: Option<RunningTaskContext>,
    /// Task information indexed by TaskId
    task_info: BoundedMap<TaskId, PreemptiveTaskInfo, MAX_PREEMPTIVE_TASKS>,
    /// Scheduling configuration
    config: PreemptiveSchedulerConfig,
    /// Priority inheritance protocol
    priority_protocol: FuelPriorityInheritanceProtocol,
    /// Scheduler statistics
    stats: PreemptiveSchedulerStats,
    /// Current fuel time for scheduling decisions
    current_fuel_time: AtomicU64,
    /// Whether preemption is enabled
    preemption_enabled: AtomicBool,
    /// Verification level for fuel tracking
    verification_level: VerificationLevel,
}

/// Priority queue for a specific priority level
#[derive(Debug)]
pub struct TaskPriorityQueue {
    /// Priority level of this queue
    pub priority: Priority,
    /// Tasks at this priority level
    pub tasks: BoundedVec<TaskId, MAX_PREEMPTIVE_TASKS>,
    /// Round-robin position for this priority level
    pub round_robin_position: AtomicUsize,
    /// Total fuel consumed by tasks at this priority
    pub fuel_consumed: AtomicU64,
    /// Number of context switches for this priority
    pub context_switches: AtomicUsize,
}

impl Default for TaskPriorityQueue {
    fn default() -> Self {
        Self {
            priority: 128, // Normal priority
            tasks: BoundedVec::new(),
            round_robin_position: AtomicUsize::new(0),
            fuel_consumed: AtomicU64::new(0),
            context_switches: AtomicUsize::new(0),
        }
    }
}

impl Clone for TaskPriorityQueue {
    fn clone(&self) -> Self {
        Self {
            priority: self.priority,
            tasks: self.tasks.clone(),
            round_robin_position: AtomicUsize::new(
                self.round_robin_position.load(Ordering::Relaxed),
            ),
            fuel_consumed: AtomicU64::new(self.fuel_consumed.load(Ordering::Relaxed)),
            context_switches: AtomicUsize::new(self.context_switches.load(Ordering::Relaxed)),
        }
    }
}

impl PartialEq for TaskPriorityQueue {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for TaskPriorityQueue {}

impl wrt_foundation::traits::Checksummable for TaskPriorityQueue {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.priority.update_checksum(checksum);
        self.tasks.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for TaskPriorityQueue {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.priority.to_bytes_with_provider(writer, provider)?;
        self.tasks.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for TaskPriorityQueue {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            priority: Priority::from_bytes_with_provider(reader, provider)?,
            tasks: BoundedVec::from_bytes_with_provider(reader, provider)?,
            round_robin_position: AtomicUsize::new(0),
            fuel_consumed: AtomicU64::new(0),
            context_switches: AtomicUsize::new(0),
        })
    }
}

/// Information about a task in the preemptive scheduler
#[derive(Debug, Clone)]
pub struct PreemptiveTaskInfo {
    /// Task identifier
    pub task_id: TaskId,
    /// Component owning the task
    pub component_id: ComponentInstanceId,
    /// Base priority of the task
    pub base_priority: Priority,
    /// Current effective priority (may be boosted)
    pub effective_priority: Priority,
    /// Fuel budget for the task
    pub fuel_budget: u64,
    /// Fuel consumed by the task
    pub fuel_consumed: u64,
    /// Fuel quantum for time slicing
    pub fuel_quantum: u64,
    /// Current state of the task
    pub state: AsyncTaskState,
    /// Time when task last ran
    pub last_run_time: u64,
    /// Total time the task has been running
    pub total_run_time: u64,
    /// Number of times the task has been preempted
    pub preemption_count: usize,
    /// Priority boost level (0 = no boost)
    pub priority_boost: u32,
    /// Deadline for the task (if any)
    pub deadline: Option<Duration>,
    /// Whether the task can be preempted
    pub preemptible: bool,
}

impl Default for PreemptiveTaskInfo {
    fn default() -> Self {
        Self {
            task_id: TaskId::default(),
            component_id: ComponentInstanceId::default(),
            base_priority: Priority::default(),
            effective_priority: Priority::default(),
            fuel_budget: 0,
            fuel_consumed: 0,
            fuel_quantum: DEFAULT_FUEL_QUANTUM,
            state: AsyncTaskState::Waiting,
            last_run_time: 0,
            total_run_time: 0,
            preemption_count: 0,
            priority_boost: 0,
            deadline: None,
            preemptible: true,
        }
    }
}

impl PartialEq for PreemptiveTaskInfo {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for PreemptiveTaskInfo {}

impl wrt_foundation::traits::Checksummable for PreemptiveTaskInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.task_id.update_checksum(checksum);
        self.component_id.update_checksum(checksum);
        self.base_priority.update_checksum(checksum);
        self.effective_priority.update_checksum(checksum);
        self.fuel_budget.update_checksum(checksum);
        self.fuel_consumed.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for PreemptiveTaskInfo {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.task_id.to_bytes_with_provider(writer, provider)?;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.base_priority.to_bytes_with_provider(writer, provider)?;
        self.effective_priority.to_bytes_with_provider(writer, provider)?;
        self.fuel_budget.to_bytes_with_provider(writer, provider)?;
        self.fuel_consumed.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for PreemptiveTaskInfo {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            task_id: TaskId::from_bytes_with_provider(reader, provider)?,
            component_id: ComponentInstanceId::from_bytes_with_provider(reader, provider)?,
            base_priority: Priority::from_bytes_with_provider(reader, provider)?,
            effective_priority: Priority::from_bytes_with_provider(reader, provider)?,
            fuel_budget: u64::from_bytes_with_provider(reader, provider)?,
            fuel_consumed: u64::from_bytes_with_provider(reader, provider)?,
            fuel_quantum: DEFAULT_FUEL_QUANTUM,
            state: AsyncTaskState::Waiting,
            last_run_time: 0,
            total_run_time: 0,
            preemption_count: 0,
            priority_boost: 0,
            deadline: None,
            preemptible: true,
        })
    }
}

/// Currently running task context
#[derive(Debug, Clone)]
pub struct RunningTaskContext {
    /// Task identifier
    pub task_id: TaskId,
    /// When the task started running (fuel time)
    pub start_time: u64,
    /// Fuel quantum allocated for this run
    pub allocated_quantum: u64,
    /// Fuel consumed in current run
    pub consumed_quantum: u64,
    /// Priority when task started running
    pub run_priority: Priority,
    /// Whether task can be preempted
    pub preemptible: bool,
}

/// Configuration for the preemptive scheduler
#[derive(Debug, Clone)]
pub struct PreemptiveSchedulerConfig {
    /// Default fuel quantum for time slicing
    pub default_fuel_quantum: u64,
    /// Whether to enable priority aging
    pub enable_priority_aging: bool,
    /// Fuel threshold for priority aging
    pub aging_fuel_threshold: u64,
    /// Maximum priority boost level
    pub max_priority_boost: u32,
    /// Whether to enable deadline scheduling
    pub enable_deadline_scheduling: bool,
    /// Whether to enable priority inheritance
    pub enable_priority_inheritance: bool,
    /// Minimum fuel quantum (prevents starvation)
    pub min_fuel_quantum: u64,
    /// Maximum fuel quantum
    pub max_fuel_quantum: u64,
}

/// Scheduler performance statistics
#[derive(Debug)]
pub struct PreemptiveSchedulerStats {
    /// Total number of preemptions
    pub total_preemptions: AtomicUsize,
    /// Total number of context switches
    pub total_context_switches: AtomicUsize,
    /// Total number of priority boosts
    pub total_priority_boosts: AtomicUsize,
    /// Total fuel consumed by scheduler overhead
    pub scheduler_fuel_consumed: AtomicU64,
    /// Average task run time in fuel units
    pub average_task_run_time: AtomicU64,
    /// Number of deadline misses
    pub deadline_misses: AtomicUsize,
    /// Total tasks scheduled
    pub total_tasks_scheduled: AtomicUsize,
    /// Current active tasks
    pub active_tasks: AtomicUsize,
}

impl Default for PreemptiveSchedulerConfig {
    fn default() -> Self {
        Self {
            default_fuel_quantum: DEFAULT_FUEL_QUANTUM,
            enable_priority_aging: true,
            aging_fuel_threshold: 5000,
            max_priority_boost: 3,
            enable_deadline_scheduling: true,
            enable_priority_inheritance: true,
            min_fuel_quantum: 100,
            max_fuel_quantum: 10000,
        }
    }
}

impl FuelPreemptiveScheduler {
    /// Create a new preemptive scheduler
    pub fn new(
        config: PreemptiveSchedulerConfig,
        verification_level: VerificationLevel,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;

        // Initialize priority queues for each priority level
        let mut priority_queues = BoundedVec::new().unwrap();
        let priorities = [
            64,  // Low priority
            128, // Normal priority
            192, // High priority
            255, // Critical priority
        ];

        for &priority in &priorities {
            let queue = TaskPriorityQueue {
                priority,
                tasks: BoundedVec::new().unwrap(),
                round_robin_position: AtomicUsize::new(0),
                fuel_consumed: AtomicU64::new(0),
                context_switches: AtomicUsize::new(0),
            };
            priority_queues.push(queue).map_err(|_| {
                Error::resource_limit_exceeded("Failed to initialize priority queues")
            })?;
        }

        let priority_protocol = FuelPriorityInheritanceProtocol::new(verification_level)?;

        Ok(Self {
            priority_queues,
            current_task: None,
            task_info: BoundedMap::new(),
            config,
            priority_protocol,
            stats: PreemptiveSchedulerStats {
                total_preemptions: AtomicUsize::new(0),
                total_context_switches: AtomicUsize::new(0),
                total_priority_boosts: AtomicUsize::new(0),
                scheduler_fuel_consumed: AtomicU64::new(0),
                average_task_run_time: AtomicU64::new(0),
                deadline_misses: AtomicUsize::new(0),
                total_tasks_scheduled: AtomicUsize::new(0),
                active_tasks: AtomicUsize::new(0),
            },
            current_fuel_time: AtomicU64::new(0),
            preemption_enabled: AtomicBool::new(true),
            verification_level,
        })
    }

    /// Add a task to the preemptive scheduler
    pub fn add_task(
        &mut self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        base_priority: Priority,
        fuel_budget: u64,
        deadline: Option<Duration>,
        preemptible: bool,
    ) -> Result<()> {
        record_global_operation(OperationType::CollectionInsert, self.verification_level);
        self.consume_scheduler_fuel(10)?;

        let fuel_quantum = self.calculate_fuel_quantum(base_priority, fuel_budget);

        let task_info = PreemptiveTaskInfo {
            task_id,
            component_id,
            base_priority,
            effective_priority: base_priority,
            fuel_budget,
            fuel_consumed: 0,
            fuel_quantum,
            state: AsyncTaskState::Ready,
            last_run_time: 0,
            total_run_time: 0,
            preemption_count: 0,
            priority_boost: 0,
            deadline,
            preemptible,
        };

        self.task_info
            .insert(task_id, task_info)
            .map_err(|_| Error::resource_limit_exceeded("Too many tasks in scheduler"))?;

        // Add to appropriate priority queue
        self.add_task_to_priority_queue(task_id, base_priority)?;

        self.stats.total_tasks_scheduled.fetch_add(1, Ordering::AcqRel);
        self.stats.active_tasks.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Select the next task to run (with preemption logic)
    pub fn schedule_next_task(&mut self) -> Result<Option<TaskId>> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_scheduler_fuel(15)?;

        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        // Check if current task should be preempted
        if let Some(ref current) = self.current_task {
            if self.should_preempt_current_task(current, current_time)? {
                self.preempt_current_task(current_time)?;
            } else {
                // Current task continues running
                return Ok(Some(current.task_id));
            }
        }

        // Select the highest priority ready task
        let next_task = self.select_highest_priority_task()?;

        if let Some(task_id) = next_task {
            self.start_task_execution(task_id, current_time)?;
        }

        Ok(next_task)
    }

    /// Update task state and handle completion
    pub fn update_task_state(
        &mut self,
        task_id: TaskId,
        new_state: AsyncTaskState,
        fuel_consumed: u64,
    ) -> Result<()> {
        record_global_operation(OperationType::CollectionMutate, self.verification_level);
        self.consume_scheduler_fuel(8)?;

        let current_time = self.current_fuel_time.fetch_add(fuel_consumed, Ordering::AcqRel);

        if let Some(task_info) = self.task_info.get_mut(&task_id) {
            task_info.fuel_consumed += fuel_consumed;
            task_info.state = new_state;

            // Update current task context if this is the running task
            if let Some(ref mut current) = self.current_task {
                if current.task_id == task_id {
                    current.consumed_quantum += fuel_consumed;
                    task_info.total_run_time += fuel_consumed;
                }
            }

            // Handle task completion or blocking
            match new_state {
                AsyncTaskState::Completed | AsyncTaskState::Failed | AsyncTaskState::Cancelled => {
                    self.remove_task_from_scheduler(task_id)?;
                },
                AsyncTaskState::Waiting => {
                    // Task is blocked, remove from ready queues
                    self.remove_task_from_priority_queues(task_id)?;
                    if let Some(ref current) = self.current_task {
                        if current.task_id == task_id {
                            self.current_task = None;
                        }
                    }
                },
                AsyncTaskState::Ready => {
                    // Task is ready, ensure it's in the right priority queue
                    self.update_task_priority_queue(task_id)?;
                },
                _ => {},
            }
        }

        // Check for priority aging
        if self.config.enable_priority_aging {
            self.check_priority_aging(current_time)?;
        }

        Ok(())
    }

    /// Check if a task should be preempted
    pub fn should_preempt_current_task(
        &self,
        current: &RunningTaskContext,
        current_time: u64,
    ) -> Result<bool> {
        // Don't preempt non-preemptible tasks
        if !current.preemptible {
            return Ok(false);
        }

        // Check if quantum is exhausted
        if current.consumed_quantum >= current.allocated_quantum {
            return Ok(true);
        }

        // Check if a higher priority task is ready
        if let Some(task_info) = self.task_info.get(&current.task_id) {
            let current_priority = task_info.effective_priority;

            // Look for higher priority ready tasks
            for queue in self.priority_queues.iter() {
                if queue.priority > current_priority && !queue.tasks.is_empty() {
                    // Check if any task in this higher priority queue is ready
                    for &task_id in queue.tasks.iter() {
                        if let Some(info) = self.task_info.get(&task_id) {
                            if info.state == AsyncTaskState::Ready {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // Check deadline violations
        if self.config.enable_deadline_scheduling {
            if let Some(task_info) = self.task_info.get(&current.task_id) {
                if let Some(deadline) = task_info.deadline {
                    let deadline_fuel = deadline.as_millis() as u64;
                    let elapsed = current_time.saturating_sub(task_info.last_run_time);
                    if elapsed > deadline_fuel {
                        self.stats.deadline_misses.fetch_add(1, Ordering::AcqRel);
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Preempt the currently running task
    pub fn preempt_current_task(&mut self, current_time: u64) -> Result<()> {
        if let Some(current) = self.current_task.take() {
            record_global_operation(OperationType::ControlFlow, self.verification_level);
            self.consume_scheduler_fuel(PREEMPTION_FUEL)?;

            // Extract task state and priority before second mutable borrow
            let (should_readd, effective_priority) = {
                if let Some(task_info) = self.task_info.get_mut(&current.task_id) {
                    task_info.preemption_count += 1;
                    task_info.total_run_time += current_time.saturating_sub(current.start_time);
                    (
                        task_info.state == AsyncTaskState::Ready,
                        task_info.effective_priority,
                    )
                } else {
                    (false, 0)
                }
            };

            // Add back to appropriate priority queue if still ready
            if should_readd {
                self.add_task_to_priority_queue(current.task_id, effective_priority)?;
            }

            self.stats.total_preemptions.fetch_add(1, Ordering::AcqRel);
        }

        Ok(())
    }

    /// Start executing a task
    pub fn start_task_execution(&mut self, task_id: TaskId, current_time: u64) -> Result<()> {
        record_global_operation(OperationType::ControlFlow, self.verification_level);
        self.consume_scheduler_fuel(CONTEXT_SWITCH_FUEL)?;

        if let Some(task_info) = self.task_info.get_mut(&task_id) {
            let quantum = task_info.fuel_quantum;
            let preemptible = task_info.preemptible;
            let priority = task_info.effective_priority;

            task_info.last_run_time = current_time;

            // Remove from priority queue
            self.remove_task_from_priority_queues(task_id)?;

            // Set as current task
            self.current_task = Some(RunningTaskContext {
                task_id,
                start_time: current_time,
                allocated_quantum: quantum,
                consumed_quantum: 0,
                run_priority: priority,
                preemptible,
            });

            self.stats.total_context_switches.fetch_add(1, Ordering::AcqRel);
        }

        Ok(())
    }

    /// Check for priority aging and boost old tasks
    pub fn check_priority_aging(&mut self, current_time: u64) -> Result<()> {
        record_global_operation(OperationType::CollectionIterate, self.verification_level);
        self.consume_scheduler_fuel(AGING_CHECK_FUEL)?;

        // Collect tasks that need priority boosts to avoid double borrow
        let mut tasks_to_boost =
            BoundedVec::<(TaskId, Priority, Priority), MAX_PREEMPTIVE_TASKS>::new();

        for (task_id, task_info) in self.task_info.iter() {
            if task_info.state == AsyncTaskState::Ready {
                let wait_time = current_time.saturating_sub(task_info.last_run_time);

                if wait_time > self.config.aging_fuel_threshold
                    && task_info.priority_boost < self.config.max_priority_boost
                {
                    let old_priority = task_info.effective_priority;
                    let new_boost = task_info.priority_boost + 1;
                    let new_priority = self.boost_priority(task_info.base_priority, new_boost);

                    if new_priority != old_priority {
                        tasks_to_boost.push((*task_id, old_priority, new_priority)).ok();
                    }
                }
            }
        }

        // Now apply the boosts
        for (task_id, old_priority, new_priority) in tasks_to_boost.iter() {
            if let Some(task_info) = self.task_info.get_mut(task_id) {
                task_info.priority_boost += 1;
                task_info.effective_priority = *new_priority;

                // Move task to new priority queue
                self.remove_task_from_priority_queues(*task_id)?;
                self.add_task_to_priority_queue(*task_id, *new_priority)?;

                self.stats.total_priority_boosts.fetch_add(1, Ordering::AcqRel);
                self.consume_scheduler_fuel(PRIORITY_BOOST_FUEL)?;
            }
        }

        Ok(())
    }

    /// Get scheduler statistics
    pub fn get_statistics(&self) -> PreemptiveSchedulerStats {
        PreemptiveSchedulerStats {
            total_preemptions: AtomicUsize::new(
                self.stats.total_preemptions.load(Ordering::Acquire),
            ),
            total_context_switches: AtomicUsize::new(
                self.stats.total_context_switches.load(Ordering::Acquire),
            ),
            total_priority_boosts: AtomicUsize::new(
                self.stats.total_priority_boosts.load(Ordering::Acquire),
            ),
            scheduler_fuel_consumed: AtomicU64::new(
                self.stats.scheduler_fuel_consumed.load(Ordering::Acquire),
            ),
            average_task_run_time: AtomicU64::new(
                self.stats.average_task_run_time.load(Ordering::Acquire),
            ),
            deadline_misses: AtomicUsize::new(self.stats.deadline_misses.load(Ordering::Acquire)),
            total_tasks_scheduled: AtomicUsize::new(
                self.stats.total_tasks_scheduled.load(Ordering::Acquire),
            ),
            active_tasks: AtomicUsize::new(self.stats.active_tasks.load(Ordering::Acquire)),
        }
    }

    /// Get information about a specific task
    pub fn get_task_info(&self, task_id: TaskId) -> Option<&PreemptiveTaskInfo> {
        self.task_info.get(&task_id)
    }

    /// Enable or disable preemption
    pub fn set_preemption_enabled(&self, enabled: bool) {
        self.preemption_enabled.store(enabled, Ordering::SeqCst);
    }

    // Private helper methods

    fn calculate_fuel_quantum(&self, priority: Priority, fuel_budget: u64) -> u64 {
        let base_quantum = match priority {
            225..=255 => self.config.max_fuel_quantum, // Critical priority
            161..=224 => self.config.default_fuel_quantum * 2, // High priority
            97..=160 => self.config.default_fuel_quantum, // Normal priority
            0..=96 => self.config.default_fuel_quantum / 2, // Low priority
        };

        // Ensure quantum is within bounds and doesn't exceed budget
        base_quantum
            .max(self.config.min_fuel_quantum)
            .min(self.config.max_fuel_quantum)
            .min(fuel_budget)
    }

    fn boost_priority(&self, base_priority: Priority, boost_level: u32) -> Priority {
        match (base_priority, boost_level) {
            (0..=96, 1..=2) => 128,   // Low -> Normal
            (0..=96, 3..) => 192,     // Low -> High
            (97..=160, 1..=2) => 192, // Normal -> High
            (97..=160, 3..) => 255,   // Normal -> Critical
            (161..=224, 1..) => 255,  // High -> Critical
            _ => base_priority,
        }
    }

    fn add_task_to_priority_queue(&mut self, task_id: TaskId, priority: Priority) -> Result<()> {
        for queue in self.priority_queues.iter_mut() {
            if queue.priority == priority {
                queue
                    .tasks
                    .push(task_id)
                    .map_err(|_| Error::resource_limit_exceeded("Priority queue is full"))?;
                return Ok(());
            }
        }
        Err(Error::resource_not_found("Priority queue not found"))
    }

    fn remove_task_from_priority_queues(&mut self, task_id: TaskId) -> Result<()> {
        for queue in self.priority_queues.iter_mut() {
            queue.tasks.retain(|&id| id != task_id);
        }
        Ok(())
    }

    fn update_task_priority_queue(&mut self, task_id: TaskId) -> Result<()> {
        if let Some(task_info) = self.task_info.get(&task_id) {
            let priority = task_info.effective_priority;
            self.remove_task_from_priority_queues(task_id)?;
            self.add_task_to_priority_queue(task_id, priority)?;
        }
        Ok(())
    }

    fn select_highest_priority_task(&mut self) -> Result<Option<TaskId>> {
        // Start from highest priority and work down
        for queue in self.priority_queues.iter_mut().rev() {
            if !queue.tasks.is_empty() {
                // Round-robin within the same priority level
                let position = queue.round_robin_position.load(Ordering::Acquire);
                let queue_len = queue.tasks.len();

                for i in 0..queue_len {
                    let index = (position + i) % queue_len;
                    if let Some(&task_id) = queue.tasks.get(index) {
                        if let Some(task_info) = self.task_info.get(&task_id) {
                            if task_info.state == AsyncTaskState::Ready {
                                // Update round-robin position
                                queue
                                    .round_robin_position
                                    .store((index + 1) % queue_len, Ordering::Release);
                                return Ok(Some(task_id));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn remove_task_from_scheduler(&mut self, task_id: TaskId) -> Result<()> {
        self.task_info.remove(&task_id);
        self.remove_task_from_priority_queues(task_id)?;

        if let Some(ref current) = self.current_task {
            if current.task_id == task_id {
                self.current_task = None;
            }
        }

        self.stats.active_tasks.fetch_sub(1, Ordering::AcqRel);
        Ok(())
    }

    fn consume_scheduler_fuel(&self, amount: u64) -> Result<()> {
        self.stats.scheduler_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
        Ok(())
    }
}
