//! Dynamic fuel management for async tasks
//!
//! This module provides adaptive fuel allocation based on task behavior,
//! system load, and priority requirements.

use core::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use wrt_foundation::{
    CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    safe_managed_alloc,
    verification::VerificationLevel,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{
    ComponentInstanceId,
    async_::fuel_async_executor::{AsyncTaskState, AsyncTaskStatus},
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum history entries for task behavior analysis
const MAX_HISTORY_ENTRIES: usize = 1024;

/// Fuel adjustment factors
const FUEL_INCREASE_FACTOR: f64 = 1.5;
const FUEL_DECREASE_FACTOR: f64 = 0.8;
const MIN_FUEL_ALLOCATION: u64 = 100;
const MAX_FUEL_ALLOCATION: u64 = 100_000;

/// Dynamic fuel manager for adaptive allocation
pub struct FuelDynamicManager {
    /// Task execution history for behavior analysis
    task_history: BoundedMap<TaskId, TaskExecutionHistory, MAX_HISTORY_ENTRIES>,
    /// Component fuel quotas
    component_quotas: BoundedMap<ComponentInstanceId, ComponentFuelQuota, 256>,
    /// System load metrics
    system_load: SystemLoadMetrics,
    /// Fuel allocation policy
    allocation_policy: FuelAllocationPolicy,
    /// Global fuel reserve for emergency allocations
    fuel_reserve: AtomicU64,
    /// Minimum reserve threshold
    min_reserve_threshold: u64,
}

/// Task execution history for adaptive allocation
#[derive(Debug, Clone)]
struct TaskExecutionHistory {
    task_id: TaskId,
    /// Recent fuel consumption samples
    fuel_samples: BoundedVec<FuelSample, 32>,
    /// Average fuel consumption per poll
    avg_fuel_per_poll: f64,
    /// Task completion rate
    completion_rate: f64,
    /// Number of times task exhausted fuel
    exhaustion_count: u32,
    /// Priority boost factor
    priority_boost: f64,
}

/// Fuel consumption sample
#[derive(Debug, Clone, Copy)]
struct FuelSample {
    fuel_consumed: u64,
    poll_count: u32,
    completed: bool,
    timestamp: u64,
}

/// Component fuel quota management
#[derive(Debug)]
struct ComponentFuelQuota {
    component_id: ComponentInstanceId,
    /// Base fuel allocation
    base_quota: u64,
    /// Current dynamic quota
    current_quota: AtomicU64,
    /// Quota utilization (0.0 - 1.0)
    utilization: AtomicU32, // Stored as percentage * 100
    /// Number of active tasks
    active_tasks: AtomicU32,
    /// Priority level
    priority: Priority,
}

/// System-wide load metrics
#[derive(Debug)]
struct SystemLoadMetrics {
    /// Total active tasks
    total_active_tasks: AtomicU32,
    /// Average fuel consumption rate
    avg_fuel_rate: AtomicU64,
    /// System fuel pressure (0-100)
    fuel_pressure: AtomicU32,
    /// Peak fuel usage in recent window
    peak_fuel_usage: AtomicU64,
}

/// Fuel allocation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelAllocationPolicy {
    /// Fixed allocation based on initial values
    Fixed,
    /// Dynamic allocation based on task behavior
    Adaptive,
    /// Priority-based with dynamic adjustment
    PriorityAdaptive,
    /// Fairness-oriented equal distribution
    FairShare,
    /// Performance-optimized allocation
    PerformanceOptimized,
}

impl FuelDynamicManager {
    /// Create a new dynamic fuel manager
    pub fn new(allocation_policy: FuelAllocationPolicy, fuel_reserve: u64) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        Ok(Self {
            task_history: BoundedMap::new(),
            component_quotas: BoundedMap::new(),
            system_load: SystemLoadMetrics {
                total_active_tasks: AtomicU32::new(0),
                avg_fuel_rate: AtomicU64::new(0),
                fuel_pressure: AtomicU32::new(0),
                peak_fuel_usage: AtomicU64::new(0),
            },
            allocation_policy,
            fuel_reserve: AtomicU64::new(fuel_reserve),
            min_reserve_threshold: fuel_reserve / 10, // 10% minimum reserve
        })
    }

    /// Register a component with fuel quota
    pub fn register_component(
        &mut self,
        component_id: ComponentInstanceId,
        base_quota: u64,
        priority: Priority,
    ) -> Result<()> {
        let quota = ComponentFuelQuota {
            component_id,
            base_quota,
            current_quota: AtomicU64::new(base_quota),
            utilization: AtomicU32::new(0),
            active_tasks: AtomicU32::new(0),
            priority,
        };

        self.component_quotas
            .insert(component_id, quota)
            .map_err(|_| Error::resource_limit_exceeded("Too many registered components"))?;

        Ok(())
    }

    /// Calculate dynamic fuel allocation for a task
    pub fn calculate_fuel_allocation(
        &mut self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        base_fuel: u64,
        priority: Priority,
    ) -> Result<u64> {
        match self.allocation_policy {
            FuelAllocationPolicy::Fixed => Ok(base_fuel),
            FuelAllocationPolicy::Adaptive => {
                self.calculate_adaptive_allocation(task_id, component_id, base_fuel)
            },
            FuelAllocationPolicy::PriorityAdaptive => self.calculate_priority_adaptive_allocation(
                task_id,
                component_id,
                base_fuel,
                priority,
            ),
            FuelAllocationPolicy::FairShare => {
                self.calculate_fair_share_allocation(component_id, base_fuel)
            },
            FuelAllocationPolicy::PerformanceOptimized => {
                self.calculate_performance_optimized_allocation(task_id, component_id, base_fuel)
            },
        }
    }

    /// Update task execution history
    pub fn update_task_history(
        &mut self,
        task_id: TaskId,
        fuel_consumed: u64,
        poll_count: u32,
        completed: bool,
    ) -> Result<()> {
        let sample = FuelSample {
            fuel_consumed,
            poll_count,
            completed,
            timestamp: self.get_timestamp(),
        };

        if let Some(history) = self.task_history.get_mut(&task_id) {
            // Add sample to history
            if history.fuel_samples.len() >= 32 {
                history.fuel_samples.remove(0);
            }
            history.fuel_samples.push(sample).ok();
        }

        // Update statistics (extract data to avoid borrow conflict)
        if let Some(history) = self.task_history.get_mut(&task_id) {
            let total_fuel: u64 = history.fuel_samples.iter().map(|s| s.fuel_consumed).sum();
            let total_polls: u32 = history.fuel_samples.iter().map(|s| s.poll_count).sum();
            let completed_count = history.fuel_samples.iter().filter(|s| s.completed).count();

            if total_polls > 0 {
                history.avg_fuel_per_poll = total_fuel as f64 / total_polls as f64;
            }

            if !history.fuel_samples.is_empty() {
                history.completion_rate =
                    completed_count as f64 / history.fuel_samples.len() as f64;
            }
        } else {
            // Create new history entry
            let provider = safe_managed_alloc!(512, CrateId::Component)?;
            let mut fuel_samples = BoundedVec::new().unwrap();
            fuel_samples.push(sample)?;

            let history = TaskExecutionHistory {
                task_id,
                fuel_samples,
                avg_fuel_per_poll: fuel_consumed as f64 / poll_count.max(1) as f64,
                completion_rate: if completed { 1.0 } else { 0.0 },
                exhaustion_count: 0,
                priority_boost: 1.0,
            };

            self.task_history
                .insert(task_id, history)
                .map_err(|_| Error::resource_limit_exceeded("Task history table full"))?;
        }

        // Update system load metrics
        self.update_system_load_metrics(fuel_consumed);

        Ok(())
    }

    /// Handle fuel exhaustion event
    pub fn handle_fuel_exhaustion(&mut self, task_id: TaskId) -> Result<u64> {
        // Extract data then drop borrow
        let emergency_fuel = if let Some(history) = self.task_history.get_mut(&task_id) {
            history.exhaustion_count += 1;

            // Calculate emergency fuel allocation (inline to avoid borrow conflict)
            let emergency = (history.avg_fuel_per_poll * 2.0 * history.priority_boost) as u64;
            emergency.clamp(MIN_FUEL_ALLOCATION, MAX_FUEL_ALLOCATION / 2)
        } else {
            return Err(Error::validation_invalid_input("Unknown task"));
        };

        // Check reserve availability
        let current_reserve = self.fuel_reserve.load(Ordering::Acquire);
        if current_reserve >= emergency_fuel
            && current_reserve - emergency_fuel >= self.min_reserve_threshold
        {
            self.fuel_reserve.fetch_sub(emergency_fuel, Ordering::AcqRel);
            Ok(emergency_fuel)
        } else {
            Err(Error::resource_exhausted("Insufficient fuel reserve"))
        }
    }

    /// Rebalance fuel allocations across components
    pub fn rebalance_allocations(&mut self) -> Result<()> {
        let total_active_tasks = self.system_load.total_active_tasks.load(Ordering::Acquire);
        if total_active_tasks == 0 {
            return Ok(());
        }

        // Calculate total available fuel
        let reserve = self.fuel_reserve.load(Ordering::Acquire);
        let total_available = reserve.saturating_sub(self.min_reserve_threshold);

        // Rebalance based on policy
        match self.allocation_policy {
            FuelAllocationPolicy::FairShare => {
                self.rebalance_fair_share(total_available, total_active_tasks)
            },
            FuelAllocationPolicy::PriorityAdaptive => {
                self.rebalance_priority_adaptive(total_available)
            },
            _ => Ok(()), // Other policies don't require periodic rebalancing
        }
    }

    /// Get fuel allocation statistics
    pub fn get_allocation_stats(&self) -> FuelAllocationStats {
        FuelAllocationStats {
            total_active_tasks: self.system_load.total_active_tasks.load(Ordering::Acquire),
            avg_fuel_rate: self.system_load.avg_fuel_rate.load(Ordering::Acquire),
            fuel_pressure: self.system_load.fuel_pressure.load(Ordering::Acquire),
            reserve_fuel: self.fuel_reserve.load(Ordering::Acquire),
            policy: self.allocation_policy,
        }
    }

    // Private helper methods

    fn calculate_adaptive_allocation(
        &self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        base_fuel: u64,
    ) -> Result<u64> {
        if let Some(history) = self.task_history.get(&task_id) {
            // Adjust based on historical consumption
            let adjustment_factor = if history.exhaustion_count > 0 {
                FUEL_INCREASE_FACTOR
            } else if history.completion_rate > 0.8 {
                FUEL_DECREASE_FACTOR
            } else {
                1.0
            };

            let adjusted = (base_fuel as f64 * adjustment_factor) as u64;
            Ok(adjusted.clamp(MIN_FUEL_ALLOCATION, MAX_FUEL_ALLOCATION))
        } else {
            Ok(base_fuel)
        }
    }

    fn calculate_priority_adaptive_allocation(
        &self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        base_fuel: u64,
        priority: Priority,
    ) -> Result<u64> {
        let priority_multiplier = match priority {
            0..=96 => 0.5,    // Low priority (0-96)
            97..=160 => 1.0,  // Normal priority (97-160)
            161..=224 => 2.0, // High priority (161-224)
            225..=255 => 4.0, // Critical priority (225-255)
        };

        let adaptive_base = self.calculate_adaptive_allocation(task_id, component_id, base_fuel)?;
        let priority_adjusted = (adaptive_base as f64 * priority_multiplier) as u64;

        Ok(priority_adjusted.clamp(MIN_FUEL_ALLOCATION, MAX_FUEL_ALLOCATION))
    }

    fn calculate_fair_share_allocation(
        &self,
        component_id: ComponentInstanceId,
        base_fuel: u64,
    ) -> Result<u64> {
        if let Some(quota) = self.component_quotas.get(&component_id) {
            let active_tasks = quota.active_tasks.load(Ordering::Acquire);
            if active_tasks > 0 {
                let fair_share = quota.current_quota.load(Ordering::Acquire) / active_tasks as u64;
                Ok(fair_share.max(MIN_FUEL_ALLOCATION))
            } else {
                Ok(base_fuel)
            }
        } else {
            Ok(base_fuel)
        }
    }

    fn calculate_performance_optimized_allocation(
        &self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        base_fuel: u64,
    ) -> Result<u64> {
        if let Some(history) = self.task_history.get(&task_id) {
            // Optimize for throughput
            let optimal = (history.avg_fuel_per_poll * 1.2) as u64;
            Ok(optimal.clamp(MIN_FUEL_ALLOCATION, MAX_FUEL_ALLOCATION))
        } else {
            Ok(base_fuel)
        }
    }

    fn calculate_emergency_fuel(&self, history: &TaskExecutionHistory) -> u64 {
        // Emergency fuel is based on average consumption with boost
        let emergency = (history.avg_fuel_per_poll * 2.0 * history.priority_boost) as u64;
        emergency.clamp(MIN_FUEL_ALLOCATION, MAX_FUEL_ALLOCATION / 2)
    }

    fn update_task_statistics(&self, history: &mut TaskExecutionHistory) {
        let total_fuel: u64 = history.fuel_samples.iter().map(|s| s.fuel_consumed).sum();
        let total_polls: u32 = history.fuel_samples.iter().map(|s| s.poll_count).sum();
        let completed_count = history.fuel_samples.iter().filter(|s| s.completed).count();

        if total_polls > 0 {
            history.avg_fuel_per_poll = total_fuel as f64 / total_polls as f64;
        }

        if !history.fuel_samples.is_empty() {
            history.completion_rate = completed_count as f64 / history.fuel_samples.len() as f64;
        }
    }

    fn update_system_load_metrics(&self, fuel_consumed: u64) {
        // Update average fuel rate (simple moving average)
        let current_rate = self.system_load.avg_fuel_rate.load(Ordering::Acquire);
        let new_rate = (current_rate * 9 + fuel_consumed) / 10;
        self.system_load.avg_fuel_rate.store(new_rate, Ordering::Release);

        // Update peak usage
        let peak = self.system_load.peak_fuel_usage.load(Ordering::Acquire);
        if fuel_consumed > peak {
            self.system_load.peak_fuel_usage.store(fuel_consumed, Ordering::Release);
        }

        // Calculate fuel pressure (0-100)
        let reserve = self.fuel_reserve.load(Ordering::Acquire);
        let pressure = if reserve > 0 {
            ((1.0 - (reserve as f64 / self.min_reserve_threshold as f64 / 10.0)) * 100.0) as u32
        } else {
            100
        };
        self.system_load.fuel_pressure.store(pressure.min(100), Ordering::Release);
    }

    fn rebalance_fair_share(&mut self, total_available: u64, total_tasks: u32) -> Result<()> {
        if total_tasks == 0 {
            return Ok(());
        }

        let per_task_allocation = total_available / total_tasks as u64;

        for (_, quota) in self.component_quotas.iter() {
            let active = quota.active_tasks.load(Ordering::Acquire);
            if active > 0 {
                let new_quota = per_task_allocation * active as u64;
                quota.current_quota.store(new_quota, Ordering::Release);
            }
        }

        Ok(())
    }

    fn rebalance_priority_adaptive(&mut self, total_available: u64) -> Result<()> {
        // Calculate priority weights
        let mut total_weight = 0.0;
        let mut weights = Vec::new();

        for (id, quota) in self.component_quotas.iter() {
            let active = quota.active_tasks.load(Ordering::Acquire) as f64;
            let priority_weight = match quota.priority {
                0..=96 => 0.5,    // Low priority (0-96)
                97..=160 => 1.0,  // Normal priority (97-160)
                161..=224 => 2.0, // High priority (161-224)
                225..=255 => 4.0, // Critical priority (225-255)
            };
            let weight = active * priority_weight;
            weights.push((*id, weight));
            total_weight += weight;
        }

        // Distribute fuel based on weights
        if total_weight > 0.0 {
            for (id, weight) in weights {
                if let Some(quota) = self.component_quotas.get(&id) {
                    let allocation = (total_available as f64 * weight / total_weight) as u64;
                    quota.current_quota.store(allocation, Ordering::Release);
                }
            }
        }

        Ok(())
    }

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }
}

/// Fuel allocation statistics
#[derive(Debug, Clone)]
pub struct FuelAllocationStats {
    pub total_active_tasks: u32,
    pub avg_fuel_rate: u64,
    pub fuel_pressure: u32,
    pub reserve_fuel: u64,
    pub policy: FuelAllocationPolicy,
}

impl Default for FuelDynamicManager {
    fn default() -> Self {
        Self::new(FuelAllocationPolicy::Adaptive, 1_000_000)
            .expect("Failed to create default manager")
    }
}
