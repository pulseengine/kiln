//! Preemption support for fuel-based async execution
//!
//! This module provides preemption capabilities for async tasks based on
//! fuel consumption, priority, and deadline requirements.

use core::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use wrt_foundation::{
    // Note: Using BoundedVec instead of BoundedBinaryHeap
    // Priority ordering will need to be handled manually
    Arc,
    CrateId,
    Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    safe_managed_alloc,
    verification::VerificationLevel,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{
    ComponentInstanceId,
    async_::{
        fuel_async_executor::{AsyncTaskState, FuelAsyncExecutor},
        fuel_dynamic_manager::FuelDynamicManager,
    },
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum preemption points per task
const MAX_PREEMPTION_POINTS: usize = 64;

/// Fuel threshold for preemption check
const PREEMPTION_CHECK_FUEL: u64 = 100;

/// Preemption manager for async tasks
pub struct FuelPreemptionManager {
    /// Preemption policies
    preemption_policy: PreemptionPolicy,
    /// Task preemption state
    task_states: BoundedMap<TaskId, PreemptionState, 1024>,
    /// Preemption queue ordered by priority
    preemption_queue: BoundedVec<PreemptionRequest, 256>,
    /// Active preemption points
    preemption_points: BoundedMap<TaskId, BoundedVec<PreemptionPoint, MAX_PREEMPTION_POINTS>, 512>,
    /// Global preemption enabled flag
    preemption_enabled: AtomicBool,
    /// Preemption statistics
    stats: PreemptionStatistics,
}

/// Preemption policy configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionPolicy {
    /// No preemption
    Disabled,
    /// Cooperative preemption at yield points
    Cooperative,
    /// Priority-based preemption
    PriorityBased,
    /// Deadline-driven preemption
    DeadlineDriven,
    /// Hybrid policy combining priority and deadline
    Hybrid,
}

/// Task preemption state
#[derive(Debug)]
struct PreemptionState {
    task_id: TaskId,
    /// Can this task be preempted
    preemptible: bool,
    /// Current preemption priority
    preemption_priority: AtomicU32,
    /// Fuel consumed since last preemption check
    fuel_since_check: AtomicU64,
    /// Number of times preempted
    preemption_count: AtomicU32,
    /// Currently preempted
    is_preempted: AtomicBool,
    /// Time quantum remaining
    quantum_remaining: AtomicU64,
}

/// Preemption request
#[derive(Debug, Clone, Eq, PartialEq)]
struct PreemptionRequest {
    requesting_task: TaskId,
    target_task: TaskId,
    priority: Priority,
    reason: PreemptionReason,
    timestamp: u64,
}

impl Ord for PreemptionRequest {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.priority.cmp(&other.priority).then(self.timestamp.cmp(&other.timestamp))
    }
}

impl PartialOrd for PreemptionRequest {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Reason for preemption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionReason {
    /// Higher priority task needs to run
    Priority,
    /// Deadline approaching
    Deadline,
    /// Fuel quantum exhausted
    FuelQuantum,
    /// System-level preemption
    System,
    /// Voluntary yield
    Voluntary,
}

/// Preemption point in task execution
#[derive(Debug, Clone)]
pub struct PreemptionPoint {
    /// Location identifier
    location_id: u64,
    /// Fuel consumed at this point
    fuel_consumed: u64,
    /// Can safely preempt here
    safe_to_preempt: bool,
    /// State checkpoint data
    checkpoint_data: Option<StateCheckpoint>,
}

/// State checkpoint for resumption
#[derive(Debug, Clone)]
pub struct StateCheckpoint {
    /// Saved fuel state
    fuel_state: u64,
    /// Task-specific state
    task_state: Vec<u8>,
    /// Timestamp of checkpoint
    timestamp: u64,
}

/// Preemption statistics
#[derive(Debug, Default)]
struct PreemptionStatistics {
    total_preemptions: AtomicU64,
    voluntary_yields: AtomicU64,
    priority_preemptions: AtomicU64,
    deadline_preemptions: AtomicU64,
    fuel_preemptions: AtomicU64,
    failed_preemptions: AtomicU64,
    avg_preemption_latency: AtomicU64,
}

impl FuelPreemptionManager {
    /// Create a new preemption manager
    pub fn new(policy: PreemptionPolicy) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;

        Ok(Self {
            preemption_policy: policy,
            task_states: BoundedMap::new(),
            preemption_queue: BoundedVec::new(),
            preemption_points: BoundedMap::new(),
            preemption_enabled: AtomicBool::new(policy != PreemptionPolicy::Disabled),
            stats: PreemptionStatistics::default(),
        })
    }

    /// Register a task for preemption management
    pub fn register_task(
        &mut self,
        task_id: TaskId,
        priority: Priority,
        preemptible: bool,
        quantum: u64,
    ) -> Result<()> {
        let state = PreemptionState {
            task_id,
            preemptible,
            preemption_priority: AtomicU32::new(priority as u32),
            fuel_since_check: AtomicU64::new(0),
            preemption_count: AtomicU32::new(0),
            is_preempted: AtomicBool::new(false),
            quantum_remaining: AtomicU64::new(quantum),
        };

        self.task_states.insert(task_id, state).map_err(|_| {
            Error::resource_limit_exceeded("Too many tasks registered for preemption")
        })?;

        // Initialize preemption points
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;
        let points = BoundedVec::new().unwrap();
        self.preemption_points.insert(task_id, points).ok();

        Ok(())
    }

    /// Check if preemption is needed for current task
    pub fn check_preemption(
        &mut self,
        current_task: TaskId,
        fuel_consumed: u64,
        executor: &FuelAsyncExecutor,
    ) -> Result<PreemptionDecision> {
        if !self.preemption_enabled.load(Ordering::Acquire) {
            return Ok(PreemptionDecision::Continue);
        }

        // First, get state and update fuel tracking
        {
            let state = self.task_states.get(&current_task).ok_or_else(|| {
                Error::validation_invalid_input("Task not registered for preemption")
            })?;

            // Update fuel tracking
            state.fuel_since_check.fetch_add(fuel_consumed, Ordering::AcqRel);
            let fuel_since = state.fuel_since_check.load(Ordering::Acquire);

            // Check if we should evaluate preemption
            if fuel_since < PREEMPTION_CHECK_FUEL {
                return Ok(PreemptionDecision::Continue);
            }

            // Reset fuel counter
            state.fuel_since_check.store(0, Ordering::Release);
        } // Drop the immutable borrow here

        // Now get state again for policy check (needs mutable self in some cases)
        let state = self
            .task_states
            .get(&current_task)
            .ok_or_else(|| Error::validation_invalid_input("Task not registered for preemption"))?;

        // Evaluate preemption based on policy (clone state data needed for methods)
        let preemptible = state.preemptible;
        let preemption_priority = state.preemption_priority.load(Ordering::Acquire);
        let quantum_remaining = state.quantum_remaining.load(Ordering::Acquire);

        match self.preemption_policy {
            PreemptionPolicy::Disabled => Ok(PreemptionDecision::Continue),
            PreemptionPolicy::Cooperative => {
                // Check quantum
                if quantum_remaining == 0 {
                    self.stats.fuel_preemptions.fetch_add(1, Ordering::Relaxed);
                    return Ok(PreemptionDecision::Preempt(PreemptionReason::FuelQuantum));
                }
                // Check if there are pending higher priority tasks
                if !self.preemption_queue.is_empty() {
                    return Ok(PreemptionDecision::YieldPoint);
                }
                Ok(PreemptionDecision::Continue)
            },
            PreemptionPolicy::PriorityBased => {
                // Check if higher priority task is waiting
                if let Some(request) = self.preemption_queue.first() {
                    if request.priority as u32 > preemption_priority && preemptible {
                        self.stats.priority_preemptions.fetch_add(1, Ordering::Relaxed);
                        return Ok(PreemptionDecision::Preempt(PreemptionReason::Priority));
                    }
                }
                Ok(PreemptionDecision::Continue)
            },
            PreemptionPolicy::DeadlineDriven => {
                if quantum_remaining < PREEMPTION_CHECK_FUEL {
                    self.stats.deadline_preemptions.fetch_add(1, Ordering::Relaxed);
                    return Ok(PreemptionDecision::Preempt(PreemptionReason::Deadline));
                }
                Ok(PreemptionDecision::Continue)
            },
            PreemptionPolicy::Hybrid => {
                // Check deadline first
                if quantum_remaining < PREEMPTION_CHECK_FUEL {
                    self.stats.deadline_preemptions.fetch_add(1, Ordering::Relaxed);
                    return Ok(PreemptionDecision::Preempt(PreemptionReason::Deadline));
                }
                // Then check priority
                if let Some(request) = self.preemption_queue.first() {
                    if request.priority as u32 > preemption_priority && preemptible {
                        self.stats.priority_preemptions.fetch_add(1, Ordering::Relaxed);
                        return Ok(PreemptionDecision::Preempt(PreemptionReason::Priority));
                    }
                }
                Ok(PreemptionDecision::Continue)
            },
        }
    }

    /// Add a preemption point for a task
    pub fn add_preemption_point(
        &mut self,
        task_id: TaskId,
        location_id: u64,
        fuel_consumed: u64,
        safe_to_preempt: bool,
    ) -> Result<()> {
        let points = self
            .preemption_points
            .get_mut(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not registered"))?;

        let point = PreemptionPoint {
            location_id,
            fuel_consumed,
            safe_to_preempt,
            checkpoint_data: None,
        };

        points
            .push(point)
            .map_err(|_| Error::resource_limit_exceeded("Too many preemption points"))?;

        Ok(())
    }

    /// Handle voluntary yield
    pub fn voluntary_yield(&mut self, task_id: TaskId) -> Result<()> {
        if let Some(state) = self.task_states.get(&task_id) {
            state.is_preempted.store(true, Ordering::Release);
            self.stats.voluntary_yields.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Resume a preempted task
    pub fn resume_task(&mut self, task_id: TaskId) -> Result<Option<StateCheckpoint>> {
        let state = self
            .task_states
            .get(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not registered"))?;

        state.is_preempted.store(false, Ordering::Release);

        // Find nearest safe preemption point with checkpoint
        if let Some(points) = self.preemption_points.get(&task_id) {
            for point in points.iter().rev() {
                if let Some(ref checkpoint) = point.checkpoint_data {
                    return Ok(Some(checkpoint.clone()));
                }
            }
        }

        Ok(None)
    }

    /// Get preemption statistics
    pub fn get_statistics(&self) -> PreemptionStats {
        PreemptionStats {
            total_preemptions: self.stats.total_preemptions.load(Ordering::Relaxed),
            voluntary_yields: self.stats.voluntary_yields.load(Ordering::Relaxed),
            priority_preemptions: self.stats.priority_preemptions.load(Ordering::Relaxed),
            deadline_preemptions: self.stats.deadline_preemptions.load(Ordering::Relaxed),
            fuel_preemptions: self.stats.fuel_preemptions.load(Ordering::Relaxed),
            failed_preemptions: self.stats.failed_preemptions.load(Ordering::Relaxed),
            avg_preemption_latency: self.stats.avg_preemption_latency.load(Ordering::Relaxed),
        }
    }

    // Private helper methods

    fn check_cooperative_preemption(
        &self,
        current_task: TaskId,
        state: &PreemptionState,
    ) -> Result<PreemptionDecision> {
        // Check quantum
        let quantum = state.quantum_remaining.load(Ordering::Acquire);
        if quantum == 0 {
            self.stats.fuel_preemptions.fetch_add(1, Ordering::Relaxed);
            return Ok(PreemptionDecision::Preempt(PreemptionReason::FuelQuantum));
        }

        // Check if there are pending higher priority tasks
        if !self.preemption_queue.is_empty() {
            return Ok(PreemptionDecision::YieldPoint);
        }

        Ok(PreemptionDecision::Continue)
    }

    fn check_priority_preemption(
        &mut self,
        current_task: TaskId,
        state: &PreemptionState,
        executor: &FuelAsyncExecutor,
    ) -> Result<PreemptionDecision> {
        let current_priority = state.preemption_priority.load(Ordering::Acquire);

        // Check if higher priority task is waiting
        if let Some(request) = self.preemption_queue.peek() {
            if request.priority as u32 > current_priority && state.preemptible {
                self.stats.priority_preemptions.fetch_add(1, Ordering::Relaxed);
                return Ok(PreemptionDecision::Preempt(PreemptionReason::Priority));
            }
        }

        Ok(PreemptionDecision::Continue)
    }

    fn check_deadline_preemption(
        &self,
        current_task: TaskId,
        state: &PreemptionState,
        executor: &FuelAsyncExecutor,
    ) -> Result<PreemptionDecision> {
        // In real implementation, would check task deadlines
        // For now, just check quantum
        let quantum = state.quantum_remaining.load(Ordering::Acquire);
        if quantum < PREEMPTION_CHECK_FUEL {
            self.stats.deadline_preemptions.fetch_add(1, Ordering::Relaxed);
            return Ok(PreemptionDecision::Preempt(PreemptionReason::Deadline));
        }

        Ok(PreemptionDecision::Continue)
    }

    fn check_hybrid_preemption(
        &mut self,
        current_task: TaskId,
        state: &PreemptionState,
        executor: &FuelAsyncExecutor,
    ) -> Result<PreemptionDecision> {
        // First check deadline
        if let Ok(PreemptionDecision::Preempt(reason)) =
            self.check_deadline_preemption(current_task, state, executor)
        {
            return Ok(PreemptionDecision::Preempt(reason));
        }

        // Then check priority
        self.check_priority_preemption(current_task, state, executor)
    }

    /// Update quantum for a task
    pub fn update_quantum(&self, task_id: TaskId, fuel_consumed: u64) -> Result<()> {
        if let Some(state) = self.task_states.get(&task_id) {
            let current = state.quantum_remaining.load(Ordering::Acquire);
            if current > fuel_consumed {
                state.quantum_remaining.fetch_sub(fuel_consumed, Ordering::AcqRel);
            } else {
                state.quantum_remaining.store(0, Ordering::Release);
            }
        }
        Ok(())
    }

    /// Refill quantum for all tasks
    pub fn refill_quantums(&self, default_quantum: u64) {
        for (_, state) in self.task_states.iter() {
            state.quantum_remaining.store(default_quantum, Ordering::Release);
        }
    }
}

/// Preemption decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionDecision {
    /// Continue execution
    Continue,
    /// Yield at next safe point
    YieldPoint,
    /// Preempt immediately
    Preempt(PreemptionReason),
}

/// Preemption statistics
#[derive(Debug, Clone)]
pub struct PreemptionStats {
    pub total_preemptions: u64,
    pub voluntary_yields: u64,
    pub priority_preemptions: u64,
    pub deadline_preemptions: u64,
    pub fuel_preemptions: u64,
    pub failed_preemptions: u64,
    pub avg_preemption_latency: u64,
}

/// Integration with fuel async executor
pub trait PreemptibleExecutor {
    /// Check and handle preemption for current task
    fn check_and_handle_preemption(&mut self, fuel_consumed: u64) -> Result<bool>;

    /// Save task state at preemption point
    fn save_preemption_state(&mut self, task_id: TaskId) -> Result<()>;

    /// Restore task state after preemption
    fn restore_preemption_state(
        &mut self,
        task_id: TaskId,
        checkpoint: StateCheckpoint,
    ) -> Result<()>;
}
