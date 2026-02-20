//! Fuel-aware constrained deadline scheduler for ASIL-C compliance
//!
//! This module implements a deadline-based scheduler that combines Rate
//! Monotonic scheduling with EDF optimization within priority bands, providing
//! WCET guarantees through fuel budget enforcement for safety-critical systems.

use core::{
    cmp::Ordering as CmpOrdering,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

#[cfg(feature = "std")]
use log::{debug, error, info, trace, warn};
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
        fuel_priority_inheritance::{FuelPriorityInheritanceProtocol, ResourceId},
    },
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum number of deadline-constrained tasks
const MAX_DEADLINE_TASKS: usize = 256;

/// Maximum number of criticality levels
const MAX_CRITICALITY_LEVELS: usize = 4;

/// Maximum tasks per criticality level
const MAX_TASKS_PER_LEVEL: usize = 64;

/// Fuel costs for deadline scheduling operations
const DEADLINE_ANALYSIS_FUEL: u64 = 25;
const WCET_VERIFICATION_FUEL: u64 = 15;
const SCHEDULABILITY_TEST_FUEL: u64 = 20;
const DEADLINE_MISS_PENALTY: u64 = 100;
const CRITICALITY_SWITCH_FUEL: u64 = 50;

/// ASIL criticality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AsilLevel {
    /// Quality Management (QM) - No safety relevance
    QM = 0,
    /// ASIL-A - Lowest safety integrity
    A = 1,
    /// ASIL-B - Medium safety integrity  
    B = 2,
    /// ASIL-C - High safety integrity
    C = 3,
    /// ASIL-D - Highest safety integrity
    D = 4,
}

/// Criticality mode for mixed-criticality scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalityMode {
    /// Normal operation - all tasks active
    Low,
    /// High criticality mode - only high criticality tasks
    High,
    /// Critical mode - only ASIL-C/D tasks
    Critical,
}

/// Deadline-constrained task information
#[derive(Debug)]
pub struct DeadlineConstrainedTask {
    /// Task identifier
    pub task_id: TaskId,
    /// Component owning the task
    pub component_id: ComponentInstanceId,
    /// ASIL criticality level
    pub asil_level: AsilLevel,
    /// Base priority for rate monotonic ordering
    pub base_priority: Priority,
    /// Task period (rate monotonic scheduling basis)
    pub period: Duration,
    /// Relative deadline (constrained: deadline ≤ period)
    pub deadline: Duration,
    /// Worst-Case Execution Time fuel budget
    pub wcet_fuel: u64,
    /// Best-Case Execution Time fuel estimate
    pub bcet_fuel: u64,
    /// Current fuel consumption in this instance
    pub current_fuel_consumed: u64,
    /// Release time of current job (fuel time)
    pub release_time: u64,
    /// Absolute deadline of current job (fuel time)
    pub absolute_deadline: u64,
    /// Number of deadline misses
    pub deadline_misses: AtomicUsize,
    /// Task state
    pub state: AsyncTaskState,
    /// Whether task is active in current criticality mode
    pub active_in_mode: bool,
    /// Utilization factor (WCET/Period)
    pub utilization: f64,
}

/// Criticality level queue for hierarchical scheduling
#[derive(Debug)]
pub struct CriticalityLevelQueue {
    /// ASIL level for this queue
    pub asil_level: AsilLevel,
    /// Tasks at this criticality level, sorted by Rate Monotonic order
    pub rm_tasks: BoundedVec<TaskId, MAX_TASKS_PER_LEVEL>,
    /// EDF queue for tasks with same period (within RM band)
    pub edf_ready_queue: BoundedVec<TaskId, MAX_TASKS_PER_LEVEL>,
    /// Total utilization at this criticality level
    pub total_utilization: f64,
    /// Fuel consumed by this criticality level
    pub fuel_consumed: AtomicU64,
    /// Number of deadline misses at this level
    pub deadline_misses: AtomicUsize,
}

/// Fuel-aware constrained deadline scheduler
pub struct FuelDeadlineScheduler {
    /// Tasks indexed by TaskId
    task_info: BoundedMap<TaskId, DeadlineConstrainedTask, MAX_DEADLINE_TASKS>,
    /// Criticality level queues (highest first)
    criticality_queues: BoundedVec<CriticalityLevelQueue, MAX_CRITICALITY_LEVELS>,
    /// Current criticality mode
    current_mode: CriticalityMode,
    /// Priority inheritance protocol
    priority_protocol: FuelPriorityInheritanceProtocol,
    /// Scheduler configuration
    config: DeadlineSchedulerConfig,
    /// Performance statistics
    stats: DeadlineSchedulerStats,
    /// Current fuel time
    current_fuel_time: AtomicU64,
    /// Whether scheduler is in overload condition
    overload_detected: AtomicBool,
    /// Verification level for fuel tracking
    verification_level: VerificationLevel,
}

/// Deadline scheduler configuration
#[derive(Debug, Clone)]
pub struct DeadlineSchedulerConfig {
    /// Enable Rate Monotonic + EDF hybrid scheduling
    pub enable_hybrid_scheduling: bool,
    /// Enable criticality mode switching
    pub enable_criticality_switching: bool,
    /// Enable WCET enforcement
    pub enable_wcet_enforcement: bool,
    /// Enable deadline miss detection
    pub enable_deadline_monitoring: bool,
    /// Maximum allowed utilization per criticality level
    pub max_utilization_per_level: f64,
    /// Global utilization bound for schedulability
    pub global_utilization_bound: f64,
    /// Deadline miss threshold for mode switching
    pub deadline_miss_threshold: usize,
    /// Fuel overhead factor for scheduling operations
    pub scheduling_overhead_factor: f64,
}

/// Scheduler performance statistics
#[derive(Debug)]
pub struct DeadlineSchedulerStats {
    /// Total deadline-constrained tasks
    pub total_tasks: AtomicUsize,
    /// Tasks currently active
    pub active_tasks: AtomicUsize,
    /// Total deadline misses across all tasks
    pub total_deadline_misses: AtomicUsize,
    /// Total successful deadline meets
    pub successful_deadlines: AtomicUsize,
    /// Total fuel consumed by scheduler
    pub scheduler_fuel_consumed: AtomicU64,
    /// Average response time in fuel units
    pub average_response_time: AtomicU64,
    /// Number of criticality mode switches
    pub criticality_switches: AtomicUsize,
    /// Tasks dropped due to overload
    pub tasks_dropped: AtomicUsize,
    /// Current global utilization
    pub current_utilization: AtomicU64, // Fixed point: utilization * 1000
    /// WCET violations detected
    pub wcet_violations: AtomicUsize,
}

/// Schedulability analysis result
#[derive(Debug, Clone)]
pub struct SchedulabilityResult {
    /// Whether the task set is schedulable
    pub schedulable: bool,
    /// Total utilization factor
    pub total_utilization: f64,
    /// Utilization bound for this configuration
    pub utilization_bound: f64,
    /// Critical path fuel time
    pub critical_path_fuel: u64,
    /// Maximum response time for any task
    pub max_response_time: u64,
    /// Tasks that would miss deadlines
    pub problematic_tasks: BoundedVec<TaskId, MAX_DEADLINE_TASKS>,
}

impl Default for DeadlineSchedulerConfig {
    fn default() -> Self {
        Self {
            enable_hybrid_scheduling: true,
            enable_criticality_switching: true,
            enable_wcet_enforcement: true,
            enable_deadline_monitoring: true,
            max_utilization_per_level: 0.7, // Conservative for safety
            global_utilization_bound: 0.69, // Rate Monotonic bound
            deadline_miss_threshold: 3,
            scheduling_overhead_factor: 1.1, // 10% overhead
        }
    }
}

impl FuelDeadlineScheduler {
    /// Create a new deadline scheduler
    pub fn new(
        config: DeadlineSchedulerConfig,
        verification_level: VerificationLevel,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(16384, CrateId::Component)?;

        // Initialize criticality level queues
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        let mut criticality_queues = BoundedVec::new();
        let asil_levels = [
            AsilLevel::QM,
            AsilLevel::A,
            AsilLevel::B,
            AsilLevel::C,
            AsilLevel::D,
        ];

        for &asil_level in &asil_levels {
            let queue = CriticalityLevelQueue {
                asil_level,
                rm_tasks: BoundedVec::new(),
                edf_ready_queue: BoundedVec::new(),
                total_utilization: 0.0,
                fuel_consumed: AtomicU64::new(0),
                deadline_misses: AtomicUsize::new(0),
            };
            criticality_queues.push(queue).map_err(|_| {
                Error::resource_limit_exceeded("Failed to initialize criticality queues")
            })?;
        }

        let priority_protocol = FuelPriorityInheritanceProtocol::new(verification_level)?;

        Ok(Self {
            task_info: BoundedMap::new(),
            criticality_queues,
            current_mode: CriticalityMode::Low,
            priority_protocol,
            config,
            stats: DeadlineSchedulerStats {
                total_tasks: AtomicUsize::new(0),
                active_tasks: AtomicUsize::new(0),
                total_deadline_misses: AtomicUsize::new(0),
                successful_deadlines: AtomicUsize::new(0),
                scheduler_fuel_consumed: AtomicU64::new(0),
                average_response_time: AtomicU64::new(0),
                criticality_switches: AtomicUsize::new(0),
                tasks_dropped: AtomicUsize::new(0),
                current_utilization: AtomicU64::new(0),
                wcet_violations: AtomicUsize::new(0),
            },
            current_fuel_time: AtomicU64::new(0),
            overload_detected: AtomicBool::new(false),
            verification_level,
        })
    }

    /// Add a deadline-constrained task with WCET analysis
    pub fn add_deadline_task(
        &mut self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        asil_level: AsilLevel,
        period: Duration,
        deadline: Duration,
        wcet_fuel: u64,
        bcet_fuel: u64,
    ) -> Result<()> {
        record_global_operation(OperationType::CollectionInsert, self.verification_level);
        self.consume_scheduler_fuel(DEADLINE_ANALYSIS_FUEL)?;

        // Validate constrained deadline (deadline ≤ period)
        if deadline > period {
            return Err(Error::runtime_execution_error(
                "Deadline must be less than or equal to period",
            ));
        }

        // Validate WCET/BCET relationship
        if wcet_fuel < bcet_fuel {
            return Err(Error::new(
                ErrorCategory::InvalidInput,
                codes::INVALID_ARGUMENT,
                "WCET must be greater than or equal to BCET",
            ));
        }

        // Calculate utilization
        let period_ms = period.as_millis() as u64;
        let utilization = (wcet_fuel as f64) / (period_ms as f64);

        // Determine base priority using Rate Monotonic (shorter period = higher
        // priority)
        let base_priority = self.calculate_rm_priority(period)?;

        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        let task = DeadlineConstrainedTask {
            task_id,
            component_id,
            asil_level,
            base_priority,
            period,
            deadline,
            wcet_fuel,
            bcet_fuel,
            current_fuel_consumed: 0,
            release_time: current_time,
            absolute_deadline: current_time + deadline.as_millis() as u64,
            deadline_misses: AtomicUsize::new(0),
            state: AsyncTaskState::Ready,
            active_in_mode: self.is_task_active_in_mode(asil_level),
            utilization,
        };

        // Perform schedulability analysis
        let schedulability = self.analyze_schedulability_with_new_task(&task)?;
        if !schedulability.schedulable {
            return Err(Error::runtime_execution_error(
                "Task would make system unschedulable",
            ));
        }

        // Add task to system
        self.task_info
            .insert(task_id, task)
            .map_err(|_| Error::resource_limit_exceeded("Too many deadline tasks"))?;

        // Add to appropriate criticality queue
        self.add_task_to_criticality_queue(task_id, asil_level)?;

        // Update statistics
        self.stats.total_tasks.fetch_add(1, Ordering::AcqRel);
        self.stats.active_tasks.fetch_add(1, Ordering::AcqRel);

        // Update utilization
        let new_util = (schedulability.total_utilization * 1000.0) as u64;
        self.stats.current_utilization.store(new_util, Ordering::Release);

        Ok(())
    }

    /// Schedule next task using hybrid RM+EDF approach
    pub fn schedule_next_task(&mut self) -> Result<Option<TaskId>> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_scheduler_fuel(DEADLINE_ANALYSIS_FUEL)?;

        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        // Check for deadline misses and mode switches
        self.check_deadline_misses(current_time)?;
        self.check_criticality_mode_switch(current_time)?;

        // Process task releases (new job arrivals)
        self.process_task_releases(current_time)?;

        // Select task using criticality-aware hybrid scheduling
        let selected_task = self.select_highest_criticality_task()?;

        if let Some(task_id) = selected_task {
            // Verify WCET budget before execution
            if self.config.enable_wcet_enforcement {
                self.verify_wcet_budget(task_id, current_time)?;
            }
        }

        Ok(selected_task)
    }

    /// Update task execution progress and check for WCET violations
    pub fn update_task_execution(
        &mut self,
        task_id: TaskId,
        fuel_consumed: u64,
        new_state: AsyncTaskState,
    ) -> Result<()> {
        record_global_operation(OperationType::CollectionMutate, self.verification_level);
        self.consume_scheduler_fuel(WCET_VERIFICATION_FUEL)?;

        let current_time = self.current_fuel_time.fetch_add(fuel_consumed, Ordering::AcqRel);

        // Extract task data needed for checks before doing mutable operations
        let (current_fuel_consumed, wcet_fuel, absolute_deadline, asil_level) = {
            if let Some(task) = self.task_info.get_mut(&task_id) {
                task.current_fuel_consumed += fuel_consumed;
                task.state = new_state;
                (
                    task.current_fuel_consumed,
                    task.wcet_fuel,
                    task.absolute_deadline,
                    task.asil_level,
                )
            } else {
                return Ok(());
            }
        };

        // Check for WCET violation
        if current_fuel_consumed > wcet_fuel {
            self.handle_wcet_violation(task_id, current_fuel_consumed, wcet_fuel)?;
        }

        // Check for deadline miss
        if current_time > absolute_deadline && new_state != AsyncTaskState::Completed {
            self.handle_deadline_miss(task_id, current_time)?;
        }

        // Handle task completion
        if new_state == AsyncTaskState::Completed {
            self.handle_task_completion(task_id, current_time)?;
        }

        // Update criticality queue fuel consumption
        self.update_criticality_fuel_consumption(asil_level, fuel_consumed)?;

        Ok(())
    }

    /// Perform offline schedulability analysis
    pub fn analyze_schedulability(&self) -> Result<SchedulabilityResult> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_scheduler_fuel(SCHEDULABILITY_TEST_FUEL)?;

        let mut total_utilization = 0.0;
        let mut max_response_time = 0u64;
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;
        let mut problematic_tasks = BoundedVec::new();

        // Rate Monotonic schedulability test for each criticality level
        for queue in self.criticality_queues.iter() {
            let level_utilization = queue.total_utilization;
            total_utilization += level_utilization;

            // Apply Rate Monotonic bound with overheads
            let rm_bound = self.calculate_rm_bound(queue.rm_tasks.len())?;
            let adjusted_bound = rm_bound * (1.0 - self.config.scheduling_overhead_factor);

            if level_utilization > adjusted_bound {
                // Collect tasks that exceed the bound
                for &task_id in queue.rm_tasks.iter() {
                    if let Some(task) = self.task_info.get(&task_id) {
                        if task.utilization > self.config.max_utilization_per_level {
                            problematic_tasks.push(task_id).map_err(|_| {
                                Error::resource_limit_exceeded("Too many problematic tasks")
                            })?;
                        }
                    }
                }
            }

            // Calculate worst-case response time for this level
            let level_response_time = self.calculate_worst_case_response_time(queue)?;
            max_response_time = max_response_time.max(level_response_time);
        }

        let schedulable = total_utilization <= self.config.global_utilization_bound
            && problematic_tasks.is_empty();

        Ok(SchedulabilityResult {
            schedulable,
            total_utilization,
            utilization_bound: self.config.global_utilization_bound,
            critical_path_fuel: max_response_time,
            max_response_time,
            problematic_tasks,
        })
    }

    /// Get current scheduler statistics
    pub fn get_statistics(&self) -> DeadlineSchedulerStats {
        DeadlineSchedulerStats {
            total_tasks: AtomicUsize::new(self.stats.total_tasks.load(Ordering::Acquire)),
            active_tasks: AtomicUsize::new(self.stats.active_tasks.load(Ordering::Acquire)),
            total_deadline_misses: AtomicUsize::new(
                self.stats.total_deadline_misses.load(Ordering::Acquire),
            ),
            successful_deadlines: AtomicUsize::new(
                self.stats.successful_deadlines.load(Ordering::Acquire),
            ),
            scheduler_fuel_consumed: AtomicU64::new(
                self.stats.scheduler_fuel_consumed.load(Ordering::Acquire),
            ),
            average_response_time: AtomicU64::new(
                self.stats.average_response_time.load(Ordering::Acquire),
            ),
            criticality_switches: AtomicUsize::new(
                self.stats.criticality_switches.load(Ordering::Acquire),
            ),
            tasks_dropped: AtomicUsize::new(self.stats.tasks_dropped.load(Ordering::Acquire)),
            current_utilization: AtomicU64::new(
                self.stats.current_utilization.load(Ordering::Acquire),
            ),
            wcet_violations: AtomicUsize::new(self.stats.wcet_violations.load(Ordering::Acquire)),
        }
    }

    /// Get task deadline information
    pub fn get_task_deadline_info(&self, task_id: TaskId) -> Option<&DeadlineConstrainedTask> {
        self.task_info.get(&task_id)
    }

    /// Switch criticality mode (for emergency/degraded operation)
    pub fn switch_criticality_mode(&mut self, new_mode: CriticalityMode) -> Result<()> {
        record_global_operation(OperationType::ControlFlow, self.verification_level);
        self.consume_scheduler_fuel(CRITICALITY_SWITCH_FUEL)?;

        if self.current_mode != new_mode {
            self.current_mode = new_mode;

            // Collect task updates to avoid borrow conflict
            let task_updates: Vec<_> = self
                .task_info
                .iter()
                .map(|(task_id, task)| {
                    let was_active = task.active_in_mode;
                    let should_be_active = self.is_task_active_in_mode(task.asil_level);
                    (*task_id, task.asil_level, was_active, should_be_active)
                })
                .collect();

            // Apply task updates
            for (task_id, asil_level, was_active, should_be_active) in task_updates {
                // Update task active status
                if let Some(task) = self.task_info.get_mut(&task_id) {
                    task.active_in_mode = should_be_active;
                }

                // If task becomes inactive, remove from queues
                if was_active && !should_be_active {
                    self.remove_task_from_criticality_queues(task_id)?;
                    self.stats.tasks_dropped.fetch_add(1, Ordering::AcqRel);
                }
                // If task becomes active, add to queues
                else if !was_active && should_be_active {
                    self.add_task_to_criticality_queue(task_id, asil_level)?;
                }
            }

            self.stats.criticality_switches.fetch_add(1, Ordering::AcqRel);
        }

        Ok(())
    }

    // Private helper methods

    fn calculate_rm_priority(&self, period: Duration) -> Result<Priority> {
        // Rate Monotonic: shorter period = higher priority
        // Priority values: 255 (critical), 192 (high), 128 (normal), 64 (low)
        let period_ms = period.as_millis() as u64;

        match period_ms {
            0..=10 => Ok(255),   // ≤ 10ms - critical priority
            11..=50 => Ok(192),  // 11-50ms - high priority
            51..=200 => Ok(128), // 51-200ms - normal priority
            _ => Ok(64),         // > 200ms - low priority
        }
    }

    fn is_task_active_in_mode(&self, asil_level: AsilLevel) -> bool {
        match self.current_mode {
            CriticalityMode::Low => true,                        // All tasks active
            CriticalityMode::High => asil_level >= AsilLevel::B, // ASIL-B and above
            CriticalityMode::Critical => asil_level >= AsilLevel::C, // ASIL-C and above
        }
    }

    fn analyze_schedulability_with_new_task(
        &self,
        new_task: &DeadlineConstrainedTask,
    ) -> Result<SchedulabilityResult> {
        let mut total_utilization = new_task.utilization;

        // Add existing utilization
        for task in self.task_info.values() {
            if task.active_in_mode {
                total_utilization += task.utilization;
            }
        }

        let schedulable = total_utilization <= self.config.global_utilization_bound;
        let provider = safe_managed_alloc!(256, CrateId::Component)?;

        Ok(SchedulabilityResult {
            schedulable,
            total_utilization,
            utilization_bound: self.config.global_utilization_bound,
            critical_path_fuel: if schedulable { 0 } else { new_task.wcet_fuel },
            max_response_time: 0, // Would need complex analysis
            problematic_tasks: {
                let provider = safe_managed_alloc!(1024, CrateId::Component)?;
                BoundedVec::new()
            },
        })
    }

    fn add_task_to_criticality_queue(
        &mut self,
        task_id: TaskId,
        asil_level: AsilLevel,
    ) -> Result<()> {
        // Extract task data before mutable borrow of queues
        let task = self
            .task_info
            .get(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;
        let task_period = task.period;
        let task_utilization = task.utilization;

        // Find insert position before mutable borrow
        let insert_pos_and_index = self
            .criticality_queues
            .iter()
            .enumerate()
            .find(|(_, queue)| queue.asil_level == asil_level)
            .map(|(idx, queue)| {
                let pos = self
                    .find_rm_insert_position(&queue.rm_tasks, task_period)
                    .unwrap_or(queue.rm_tasks.len());
                (idx, pos)
            });

        if let Some((queue_idx, insert_pos)) = insert_pos_and_index {
            let queue = &mut self.criticality_queues[queue_idx];

            // StaticVec doesn't have insert method, so we manually shift elements
            if insert_pos >= queue.rm_tasks.len() {
                // Insert at end
                queue
                    .rm_tasks
                    .push(task_id)
                    .map_err(|_| Error::resource_limit_exceeded("Criticality queue is full"))?;
            } else {
                // Insert in middle - need to shift elements
                let mut temp_vec = BoundedVec::new();
                for (i, &id) in queue.rm_tasks.iter().enumerate() {
                    if i == insert_pos {
                        temp_vec.push(task_id).map_err(|_| {
                            Error::resource_limit_exceeded("Criticality queue is full")
                        })?;
                    }
                    temp_vec
                        .push(id)
                        .map_err(|_| Error::resource_limit_exceeded("Criticality queue is full"))?;
                }
                queue.rm_tasks = temp_vec;
            }

            queue.total_utilization += task_utilization;
            Ok(())
        } else {
            Err(Error::resource_not_found("Criticality level not found"))
        }
    }

    fn find_rm_insert_position(
        &self,
        rm_tasks: &BoundedVec<TaskId, MAX_TASKS_PER_LEVEL>,
        period: Duration,
    ) -> Result<usize> {
        for (i, &existing_task_id) in rm_tasks.iter().enumerate() {
            if let Some(existing_task) = self.task_info.get(&existing_task_id) {
                if period < existing_task.period {
                    return Ok(i);
                }
            }
        }
        Ok(rm_tasks.len())
    }

    fn select_highest_criticality_task(&mut self) -> Result<Option<TaskId>> {
        // Process criticality levels from highest to lowest (D, C, B, A, QM)
        // Extract queue information first to avoid double borrow
        for i in (0..self.criticality_queues.len()).rev() {
            // Check if queue has ready tasks before calling method
            let has_ready_tasks = if let Some(queue) = self.criticality_queues.get(i) {
                !queue.rm_tasks.is_empty()
            } else {
                false
            };

            if has_ready_tasks {
                // Now select task using queue index
                if let Some(task_id) = self.select_task_from_criticality_level(i)? {
                    return Ok(Some(task_id));
                }
            }
        }
        Ok(None)
    }

    fn select_task_from_criticality_level(&mut self, queue_index: usize) -> Result<Option<TaskId>> {
        if self.config.enable_hybrid_scheduling {
            // Use EDF within Rate Monotonic priority bands
            self.select_edf_within_rm_band(queue_index)
        } else {
            // Pure Rate Monotonic
            self.select_pure_rm_task(queue_index)
        }
    }

    fn select_edf_within_rm_band(&mut self, queue_index: usize) -> Result<Option<TaskId>> {
        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        // Build EDF ready queue
        {
            let queue = &mut self.criticality_queues[queue_index];

            // Find tasks ready for execution
            queue.edf_ready_queue.clear();

            for &task_id in queue.rm_tasks.iter() {
                if let Some(task) = self.task_info.get(&task_id) {
                    if task.state == AsyncTaskState::Ready
                        && task.active_in_mode
                        && current_time >= task.release_time
                    {
                        queue.edf_ready_queue.push(task_id).map_err(|_| {
                            Error::resource_limit_exceeded("EDF ready queue is full")
                        })?;
                    }
                }
            }
        }

        // Sort the queue using a separate mutable access with immutable self
        self.sort_edf_queue_at_index(queue_index, current_time)?;

        // Get result after sorting
        Ok(self.criticality_queues[queue_index].edf_ready_queue.first().copied())
    }

    fn sort_edf_queue_at_index(&mut self, queue_index: usize, _current_time: u64) -> Result<()> {
        // Inline bubble sort to avoid borrow conflicts
        let edf_queue = &mut self.criticality_queues[queue_index].edf_ready_queue;
        let len = edf_queue.len();
        for i in 0..len {
            for j in 0..len.saturating_sub(1 + i) {
                let should_swap = {
                    if let (Some(task_a), Some(task_b)) = (
                        self.task_info.get(&edf_queue[j]),
                        self.task_info.get(&edf_queue[j + 1]),
                    ) {
                        task_a.absolute_deadline > task_b.absolute_deadline
                    } else {
                        false
                    }
                };

                if should_swap {
                    let temp = edf_queue[j];
                    edf_queue[j] = edf_queue[j + 1];
                    edf_queue[j + 1] = temp;
                }
            }
        }
        Ok(())
    }

    fn select_pure_rm_task(&self, queue_index: usize) -> Result<Option<TaskId>> {
        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        // Get reference to queue
        let queue = &self.criticality_queues[queue_index];

        // Return first ready task in RM order
        for &task_id in queue.rm_tasks.iter() {
            if let Some(task) = self.task_info.get(&task_id) {
                if task.state == AsyncTaskState::Ready
                    && task.active_in_mode
                    && current_time >= task.release_time
                {
                    return Ok(Some(task_id));
                }
            }
        }
        Ok(None)
    }

    fn sort_edf_queue(
        &self,
        edf_queue: &mut BoundedVec<TaskId, MAX_TASKS_PER_LEVEL>,
        _current_time: u64,
    ) -> Result<()> {
        // Simple bubble sort for EDF ordering
        let len = edf_queue.len();
        for i in 0..len {
            for j in 0..len.saturating_sub(1 + i) {
                let should_swap = {
                    if let (Some(task_a), Some(task_b)) = (
                        self.task_info.get(&edf_queue[j]),
                        self.task_info.get(&edf_queue[j + 1]),
                    ) {
                        task_a.absolute_deadline > task_b.absolute_deadline
                    } else {
                        false
                    }
                };

                if should_swap {
                    let temp = edf_queue[j];
                    edf_queue[j] = edf_queue[j + 1];
                    edf_queue[j + 1] = temp;
                }
            }
        }
        Ok(())
    }

    fn process_task_releases(&mut self, current_time: u64) -> Result<()> {
        // Check for new job releases (periodic tasks)
        for task in self.task_info.values_mut() {
            let period_ms = task.period.as_millis() as u64;
            let time_since_release = current_time.saturating_sub(task.release_time);

            if time_since_release >= period_ms && task.state == AsyncTaskState::Completed {
                // Release new job
                task.release_time = current_time;
                task.absolute_deadline = current_time + task.deadline.as_millis() as u64;
                task.current_fuel_consumed = 0;
                task.state = AsyncTaskState::Ready;
            }
        }
        Ok(())
    }

    fn verify_wcet_budget(&self, task_id: TaskId, _current_time: u64) -> Result<()> {
        if let Some(task) = self.task_info.get(&task_id) {
            if task.current_fuel_consumed >= task.wcet_fuel {
                return Err(Error::runtime_execution_error("WCET budget exceeded"));
            }
        }
        Ok(())
    }

    fn handle_wcet_violation(&mut self, task_id: TaskId, consumed: u64, wcet: u64) -> Result<()> {
        self.stats.wcet_violations.fetch_add(1, Ordering::AcqRel);
        self.consume_scheduler_fuel(DEADLINE_MISS_PENALTY)?;

        // In a real system, this might trigger a safety response
        #[cfg(feature = "std")]
        warn!("Task deadline violation detected");

        Ok(())
    }

    fn handle_deadline_miss(&mut self, task_id: TaskId, current_time: u64) -> Result<()> {
        // Extract task data before consuming fuel (which borrows self)
        #[cfg(feature = "component-model-threading")]
        let (task_id_value, absolute_deadline) = if let Some(task) = self.task_info.get(&task_id) {
            (task_id.0, task.absolute_deadline)
        } else {
            return Ok(());
        };

        #[cfg(not(feature = "component-model-threading"))]
        let absolute_deadline = if let Some(task) = self.task_info.get(&task_id) {
            task.absolute_deadline
        } else {
            return Ok(());
        };

        // Consume fuel before mutable borrow
        self.stats.total_deadline_misses.fetch_add(1, Ordering::AcqRel);
        self.consume_scheduler_fuel(DEADLINE_MISS_PENALTY)?;

        // Now update the task
        if let Some(task) = self.task_info.get_mut(&task_id) {
            task.deadline_misses.fetch_add(1, Ordering::AcqRel);

            let miss_count = task.deadline_misses.load(Ordering::Acquire);
            if miss_count >= self.config.deadline_miss_threshold {
                // Consider criticality mode switch
                #[cfg(all(feature = "std", feature = "component-model-threading"))]
                error!(
                    "Task {} missed {} deadlines, considering mode switch",
                    task_id_value, miss_count
                );
            }

            let lateness = current_time.saturating_sub(absolute_deadline);
            #[cfg(all(feature = "std", feature = "component-model-threading"))]
            warn!(
                "Deadline miss: Task {} late by {} fuel units",
                task_id_value, lateness
            );
        }
        Ok(())
    }

    fn handle_task_completion(&mut self, task_id: TaskId, current_time: u64) -> Result<()> {
        if let Some(task) = self.task_info.get(&task_id) {
            if current_time <= task.absolute_deadline {
                self.stats.successful_deadlines.fetch_add(1, Ordering::AcqRel);
            }

            // Update average response time
            let response_time = current_time.saturating_sub(task.release_time);
            let current_avg = self.stats.average_response_time.load(Ordering::Acquire);
            let new_avg =
                if current_avg == 0 { response_time } else { (current_avg + response_time) / 2 };
            self.stats.average_response_time.store(new_avg, Ordering::Release);
        }
        Ok(())
    }

    fn check_deadline_misses(&mut self, current_time: u64) -> Result<()> {
        if !self.config.enable_deadline_monitoring {
            return Ok(());
        }

        let mut total_misses = 0;
        for task in self.task_info.values() {
            if current_time > task.absolute_deadline && task.state != AsyncTaskState::Completed {
                total_misses += 1;
            }
        }

        if total_misses > self.config.deadline_miss_threshold {
            self.overload_detected.store(true, Ordering::Release);
        }

        Ok(())
    }

    fn check_criticality_mode_switch(&mut self, _current_time: u64) -> Result<()> {
        if !self.config.enable_criticality_switching {
            return Ok(());
        }

        let total_misses = self.stats.total_deadline_misses.load(Ordering::Acquire);
        let overload = self.overload_detected.load(Ordering::Acquire);

        // Switch to higher criticality mode if too many deadline misses
        let new_mode = match self.current_mode {
            CriticalityMode::Low
                if total_misses > self.config.deadline_miss_threshold || overload =>
            {
                CriticalityMode::High
            },
            CriticalityMode::High if total_misses > self.config.deadline_miss_threshold * 2 => {
                CriticalityMode::Critical
            },
            _ => return Ok(()), // No mode switch needed
        };

        self.switch_criticality_mode(new_mode)?;
        Ok(())
    }

    fn update_criticality_fuel_consumption(
        &mut self,
        asil_level: AsilLevel,
        fuel: u64,
    ) -> Result<()> {
        for queue in self.criticality_queues.iter_mut() {
            if queue.asil_level == asil_level {
                queue.fuel_consumed.fetch_add(fuel, Ordering::AcqRel);
                break;
            }
        }
        Ok(())
    }

    fn remove_task_from_criticality_queues(&mut self, task_id: TaskId) -> Result<()> {
        for queue in self.criticality_queues.iter_mut() {
            queue.rm_tasks.retain(|&id| id != task_id);
            queue.edf_ready_queue.retain(|&id| id != task_id);
        }
        Ok(())
    }

    fn calculate_rm_bound(&self, n: usize) -> Result<f64> {
        if n == 0 {
            return Ok(1.0);
        }

        // Rate Monotonic bound: n * (2^(1/n) - 1)
        let n_f = n as f64;
        let bound = n_f * (2.0_f64.powf(1.0 / n_f) - 1.0);
        Ok(bound)
    }

    fn calculate_worst_case_response_time(&self, queue: &CriticalityLevelQueue) -> Result<u64> {
        let mut max_response_time = 0u64;

        for &task_id in queue.rm_tasks.iter() {
            if let Some(task) = self.task_info.get(&task_id) {
                // Simple approximation: WCET + interference from higher priority tasks
                let mut response_time = task.wcet_fuel;

                // Add interference from higher priority tasks in this and higher criticality
                // levels
                for higher_queue in self.criticality_queues.iter().rev() {
                    if higher_queue.asil_level >= queue.asil_level {
                        for &higher_task_id in higher_queue.rm_tasks.iter() {
                            if let Some(higher_task) = self.task_info.get(&higher_task_id) {
                                if higher_task.period < task.period {
                                    // This is a simplified interference calculation
                                    let interference = (task.deadline.as_millis() as u64)
                                        / (higher_task.period.as_millis() as u64);
                                    response_time += interference * higher_task.wcet_fuel;
                                }
                            }
                        }
                    }
                    if higher_queue.asil_level == queue.asil_level {
                        break;
                    }
                }

                max_response_time = max_response_time.max(response_time);
            }
        }

        Ok(max_response_time)
    }

    fn consume_scheduler_fuel(&self, amount: u64) -> Result<()> {
        self.stats.scheduler_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
        Ok(())
    }
}
