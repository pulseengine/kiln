//! Advanced synchronization primitives for async operations
//!
//! This module provides async-aware synchronization primitives including
//! mutexes, semaphores, barriers, and condition variables integrated with
//! the fuel-based async executor.
//!
//! **REQUIRES**: `std`, `bounded-allocation`, or `managed-allocation` feature
//! Async sync primitives need Arc/Weak support for reference counting

// Async sync primitives fundamentally require Arc/Weak from alloc
#[cfg(not(any(
    feature = "std",
    feature = "bounded-allocation",
    feature = "managed-allocation"
)))]
compile_error!(
    "advanced_sync_primitives requires 'std', 'bounded-allocation', or 'managed-allocation' feature - async operations need Arc/Weak support"
);

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::sync::Weak;
#[cfg(feature = "std")]
use std::sync::Weak;

use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};

use wrt_foundation::{
    CrateId, Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    component_value::ComponentValue,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{
    ComponentInstanceId,
    async_::{
        fuel_async_executor::AsyncTaskState,
        fuel_aware_waker::create_fuel_aware_waker,
        task_manager_async_bridge::{ComponentAsyncTaskType, TaskManagerAsyncBridge},
    },
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum waiters per synchronization primitive
const MAX_WAITERS_PER_PRIMITIVE: usize = 128;

/// Maximum sync primitives per component
const MAX_SYNC_PRIMITIVES_PER_COMPONENT: usize = 64;

/// Fuel costs for sync operations
const MUTEX_LOCK_FUEL: u64 = 25;
const MUTEX_UNLOCK_FUEL: u64 = 15;
const SEMAPHORE_ACQUIRE_FUEL: u64 = 20;
const SEMAPHORE_RELEASE_FUEL: u64 = 15;
const BARRIER_WAIT_FUEL: u64 = 30;
const CONDVAR_WAIT_FUEL: u64 = 25;
const CONDVAR_NOTIFY_FUEL: u64 = 20;

/// Advanced synchronization primitives manager
pub struct AdvancedSyncPrimitives {
    /// Bridge for task management
    bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
    /// Active sync primitives
    primitives: BoundedMap<SyncPrimitiveId, SyncPrimitive, 512>,
    /// Component sync contexts
    component_contexts: BoundedMap<ComponentInstanceId, ComponentSyncContext, 128>,
    /// Next primitive ID
    next_primitive_id: AtomicU64,
    /// Sync statistics
    sync_stats: SyncStatistics,
    /// Sync configuration
    sync_config: SyncConfiguration,
}

/// Synchronization primitive identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SyncPrimitiveId(u64);

/// Synchronization primitive
#[derive(Debug)]
struct SyncPrimitive {
    id: SyncPrimitiveId,
    component_id: ComponentInstanceId,
    primitive_type: SyncPrimitiveType,
    waiters: BoundedVec<WaiterInfo, MAX_WAITERS_PER_PRIMITIVE>,
    created_at: u64,
    fuel_consumed: AtomicU64,
}

/// Type of synchronization primitive
#[derive(Debug)]
pub enum SyncPrimitiveType {
    /// Async mutex
    AsyncMutex {
        locked: AtomicBool,
        owner: Option<TaskId>,
        lock_count: AtomicU32, // For reentrant mutexes
        is_reentrant: bool,
    },
    /// Async semaphore
    AsyncSemaphore {
        permits: AtomicU32,
        max_permits: u32,
        fair_scheduling: bool,
    },
    /// Async barrier
    AsyncBarrier {
        parties: u32,
        waiting: AtomicU32,
        generation: AtomicU64,
        broken: AtomicBool,
    },
    /// Async condition variable
    AsyncCondVar {
        associated_mutex: Option<SyncPrimitiveId>,
        notification_count: AtomicU64,
    },
    /// Async read-write lock
    AsyncRwLock {
        readers: AtomicU32,
        writer: AtomicBool,
        writer_waiting: AtomicBool,
        readers_waiting: AtomicU32,
        prefer_writers: bool,
    },
    /// Async latch (countdown latch)
    AsyncLatch {
        count: AtomicU32,
        initial_count: u32,
    },
}

/// Waiter information
#[derive(Debug, Clone)]
struct WaiterInfo {
    task_id: TaskId,
    component_id: ComponentInstanceId,
    waker: Option<Waker>,
    wait_type: WaitType,
    queued_at: u64,
    priority: Priority,
}

/// Type of wait operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitType {
    /// Waiting for mutex lock
    MutexLock,
    /// Waiting for semaphore permit
    SemaphorePermit,
    /// Waiting at barrier
    BarrierWait,
    /// Waiting on condition variable
    CondVarWait,
    /// Waiting for read lock
    ReadLock,
    /// Waiting for write lock
    WriteLock,
    /// Waiting for latch countdown
    LatchWait,
}

/// Component sync context
#[derive(Debug)]
struct ComponentSyncContext {
    component_id: ComponentInstanceId,
    /// Sync primitives owned by this component
    owned_primitives: BoundedVec<SyncPrimitiveId, MAX_SYNC_PRIMITIVES_PER_COMPONENT>,
    /// Sync limits
    sync_limits: SyncLimits,
}

/// Sync limits per component
#[derive(Debug, Clone)]
pub struct SyncLimits {
    pub max_mutexes: usize,
    pub max_semaphores: usize,
    pub max_barriers: usize,
    pub max_condvars: usize,
    pub max_rwlocks: usize,
    pub max_latches: usize,
    pub fuel_budget: u64,
}

/// Sync configuration
#[derive(Debug, Clone)]
pub struct SyncConfiguration {
    pub enable_priority_inheritance: bool,
    pub enable_deadlock_detection: bool,
    pub enable_fair_scheduling: bool,
    pub max_wait_time_ms: u64,
    pub enable_reentrant_mutexes: bool,
    pub fuel_tracking: bool,
}

impl Default for SyncConfiguration {
    fn default() -> Self {
        Self {
            enable_priority_inheritance: true,
            enable_deadlock_detection: true,
            enable_fair_scheduling: true,
            max_wait_time_ms: 30_000, // 30 seconds
            enable_reentrant_mutexes: false,
            fuel_tracking: true,
        }
    }
}

/// Sync statistics
#[derive(Debug, Default)]
struct SyncStatistics {
    total_mutexes_created: AtomicU64,
    total_mutex_locks: AtomicU64,
    total_mutex_unlocks: AtomicU64,
    total_semaphores_created: AtomicU64,
    total_semaphore_acquires: AtomicU64,
    total_semaphore_releases: AtomicU64,
    total_barriers_created: AtomicU64,
    total_barrier_waits: AtomicU64,
    total_condvars_created: AtomicU64,
    total_condvar_waits: AtomicU64,
    total_condvar_notifies: AtomicU64,
    deadlocks_detected: AtomicU64,
    priority_inversions: AtomicU64,
    total_fuel_consumed: AtomicU64,
}

impl AdvancedSyncPrimitives {
    /// Create new advanced sync primitives manager
    pub fn new(
        bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
        config: Option<SyncConfiguration>,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        Ok(Self {
            bridge,
            primitives: BoundedMap::new(),
            component_contexts: BoundedMap::new(),
            next_primitive_id: AtomicU64::new(1),
            sync_stats: SyncStatistics::default(),
            sync_config: config.unwrap_or_default(),
        })
    }

    /// Initialize component for sync operations
    pub fn initialize_component_sync(
        &mut self,
        component_id: ComponentInstanceId,
        limits: Option<SyncLimits>,
    ) -> Result<()> {
        let limits = limits.unwrap_or_else(|| SyncLimits {
            max_mutexes: 32,
            max_semaphores: 16,
            max_barriers: 8,
            max_condvars: 16,
            max_rwlocks: 16,
            max_latches: 8,
            fuel_budget: 50_000,
        });

        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        let context = ComponentSyncContext {
            component_id,
            owned_primitives: BoundedVec::new(),
            sync_limits: limits,
        };

        self.component_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component sync contexts"))?;

        Ok(())
    }

    /// Create an async mutex
    pub fn create_async_mutex(
        &mut self,
        component_id: ComponentInstanceId,
        is_reentrant: bool,
    ) -> Result<SyncPrimitiveId> {
        // Extract values before mutable borrow
        let max_mutexes = self
            .component_contexts
            .get(&component_id)
            .ok_or_else(|| {
                Error::validation_invalid_input("Component not initialized for sync operations")
            })?
            .sync_limits
            .max_mutexes;
        let timestamp = self.get_timestamp();

        let context = self.component_contexts.get_mut(&component_id).ok_or_else(|| {
            Error::validation_invalid_input("Component not initialized for sync operations")
        })?;

        // Check limits
        let current_mutexes = context
            .owned_primitives
            .iter()
            .filter(|&&id| {
                if let Some(primitive) = self.primitives.get(&id) {
                    matches!(
                        primitive.primitive_type,
                        SyncPrimitiveType::AsyncMutex { .. }
                    )
                } else {
                    false
                }
            })
            .count();

        if current_mutexes >= max_mutexes {
            return Err(Error::resource_limit_exceeded(
                "Component mutex limit exceeded",
            ));
        }

        let primitive_id = SyncPrimitiveId(self.next_primitive_id.fetch_add(1, Ordering::AcqRel));
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;

        let primitive = SyncPrimitive {
            id: primitive_id,
            component_id,
            primitive_type: SyncPrimitiveType::AsyncMutex {
                locked: AtomicBool::new(false),
                owner: None,
                lock_count: AtomicU32::new(0),
                is_reentrant,
            },
            waiters: BoundedVec::new(),
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
        };

        self.primitives
            .insert(primitive_id, primitive)
            .map_err(|_| Error::resource_limit_exceeded("Too many sync primitives"))?;

        context
            .owned_primitives
            .push(primitive_id)
            .map_err(|_| Error::resource_limit_exceeded("Component primitive list full"))?;

        self.sync_stats.total_mutexes_created.fetch_add(1, Ordering::Relaxed);

        Ok(primitive_id)
    }

    /// Create an async semaphore
    pub fn create_async_semaphore(
        &mut self,
        component_id: ComponentInstanceId,
        permits: u32,
        fair_scheduling: bool,
    ) -> Result<SyncPrimitiveId> {
        if permits == 0 {
            return Err(Error::validation_invalid_input(
                "Semaphore must have at least 1 permit",
            ));
        }

        // Check component exists before mutable borrow
        if !self.component_contexts.contains_key(&component_id) {
            return Err(Error::validation_invalid_input(
                "Component not initialized for sync operations",
            ));
        }

        let primitive_id = SyncPrimitiveId(self.next_primitive_id.fetch_add(1, Ordering::AcqRel));
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;
        let timestamp = self.get_timestamp();

        let primitive = SyncPrimitive {
            id: primitive_id,
            component_id,
            primitive_type: SyncPrimitiveType::AsyncSemaphore {
                permits: AtomicU32::new(permits),
                max_permits: permits,
                fair_scheduling,
            },
            waiters: BoundedVec::new(),
            created_at: timestamp,
            fuel_consumed: AtomicU64::new(0),
        };

        self.primitives
            .insert(primitive_id, primitive)
            .map_err(|_| Error::resource_limit_exceeded("Too many sync primitives"))?;

        if let Some(context) = self.component_contexts.get_mut(&component_id) {
            context.owned_primitives.push(primitive_id).ok();
        }

        self.sync_stats.total_semaphores_created.fetch_add(1, Ordering::Relaxed);

        Ok(primitive_id)
    }

    /// Create an async barrier
    pub fn create_async_barrier(
        &mut self,
        component_id: ComponentInstanceId,
        parties: u32,
    ) -> Result<SyncPrimitiveId> {
        if parties == 0 {
            return Err(Error::validation_invalid_input(
                "Barrier must have at least 1 party",
            ));
        }

        let primitive_id = SyncPrimitiveId(self.next_primitive_id.fetch_add(1, Ordering::AcqRel));
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;

        let primitive = SyncPrimitive {
            id: primitive_id,
            component_id,
            primitive_type: SyncPrimitiveType::AsyncBarrier {
                parties,
                waiting: AtomicU32::new(0),
                generation: AtomicU64::new(0),
                broken: AtomicBool::new(false),
            },
            waiters: BoundedVec::new(),
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        self.primitives.insert(primitive_id, primitive).ok();

        if let Some(context) = self.component_contexts.get_mut(&component_id) {
            context.owned_primitives.push(primitive_id).ok();
        }

        self.sync_stats.total_barriers_created.fetch_add(1, Ordering::Relaxed);

        Ok(primitive_id)
    }

    /// Create an async condition variable
    pub fn create_async_condvar(
        &mut self,
        component_id: ComponentInstanceId,
        associated_mutex: Option<SyncPrimitiveId>,
    ) -> Result<SyncPrimitiveId> {
        // Validate associated mutex if provided
        if let Some(mutex_id) = associated_mutex {
            let mutex = self
                .primitives
                .get(&mutex_id)
                .ok_or_else(|| Error::validation_invalid_input("Associated mutex not found"))?;

            if !matches!(mutex.primitive_type, SyncPrimitiveType::AsyncMutex { .. }) {
                return Err(Error::validation_invalid_input(
                    "Associated primitive is not a mutex",
                ));
            }
        }

        let primitive_id = SyncPrimitiveId(self.next_primitive_id.fetch_add(1, Ordering::AcqRel));
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;

        let primitive = SyncPrimitive {
            id: primitive_id,
            component_id,
            primitive_type: SyncPrimitiveType::AsyncCondVar {
                associated_mutex,
                notification_count: AtomicU64::new(0),
            },
            waiters: BoundedVec::new(),
            created_at: self.get_timestamp(),
            fuel_consumed: AtomicU64::new(0),
        };

        self.primitives.insert(primitive_id, primitive).ok();

        if let Some(context) = self.component_contexts.get_mut(&component_id) {
            context.owned_primitives.push(primitive_id).ok();
        }

        self.sync_stats.total_condvars_created.fetch_add(1, Ordering::Relaxed);

        Ok(primitive_id)
    }

    /// Lock an async mutex
    pub fn lock_async_mutex(
        &mut self,
        primitive_id: SyncPrimitiveId,
        task_id: TaskId,
        component_id: ComponentInstanceId,
    ) -> Result<MutexLockResult> {
        // Extract timestamp before mutable borrow
        let timestamp = self.get_timestamp();

        let primitive = self
            .primitives
            .get_mut(&primitive_id)
            .ok_or_else(|| Error::validation_invalid_input("Mutex not found"))?;

        match &mut primitive.primitive_type {
            SyncPrimitiveType::AsyncMutex {
                locked,
                owner,
                lock_count,
                is_reentrant,
            } => {
                // Check if already locked
                if locked.load(Ordering::Acquire) {
                    // Check for reentrant lock
                    if *is_reentrant && owner.as_ref() == Some(&task_id) {
                        lock_count.fetch_add(1, Ordering::AcqRel);
                        primitive.fuel_consumed.fetch_add(MUTEX_LOCK_FUEL, Ordering::Relaxed);
                        self.sync_stats.total_mutex_locks.fetch_add(1, Ordering::Relaxed);
                        return Ok(MutexLockResult::Acquired);
                    }

                    // Add to waiters
                    let waiter = WaiterInfo {
                        task_id,
                        component_id,
                        waker: None,
                        wait_type: WaitType::MutexLock,
                        queued_at: timestamp,
                        priority: 128, // Normal priority // Would get from task
                    };

                    primitive
                        .waiters
                        .push(waiter)
                        .map_err(|_| Error::resource_limit_exceeded("Mutex waiter queue full"))?;

                    return Ok(MutexLockResult::WouldBlock);
                }

                // Acquire the lock
                locked.store(true, Ordering::Release);
                *owner = Some(task_id);
                lock_count.store(1, Ordering::Release);

                primitive.fuel_consumed.fetch_add(MUTEX_LOCK_FUEL, Ordering::Relaxed);
                self.sync_stats.total_mutex_locks.fetch_add(1, Ordering::Relaxed);

                Ok(MutexLockResult::Acquired)
            },
            _ => Err(Error::validation_invalid_input("Primitive is not a mutex")),
        }
    }

    /// Unlock an async mutex
    pub fn unlock_async_mutex(
        &mut self,
        primitive_id: SyncPrimitiveId,
        task_id: TaskId,
    ) -> Result<()> {
        {
            let primitive = self
                .primitives
                .get_mut(&primitive_id)
                .ok_or_else(|| Error::validation_invalid_input("Mutex not found"))?;

            match &mut primitive.primitive_type {
                SyncPrimitiveType::AsyncMutex {
                    locked,
                    owner,
                    lock_count,
                    is_reentrant,
                } => {
                    // Verify ownership
                    if owner.as_ref() != Some(&task_id) {
                        return Err(Error::invalid_state_error("Task does not own mutex"));
                    }

                    let current_count = lock_count.load(Ordering::Acquire);
                    if current_count == 0 {
                        return Err(Error::invalid_state_error("Mutex is not locked"));
                    }

                    // Handle reentrant unlock
                    if *is_reentrant && current_count > 1 {
                        lock_count.fetch_sub(1, Ordering::AcqRel);
                        primitive.fuel_consumed.fetch_add(MUTEX_UNLOCK_FUEL, Ordering::Relaxed);
                        self.sync_stats.total_mutex_unlocks.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }

                    // Release the lock
                    *owner = None;
                    lock_count.store(0, Ordering::Release);
                    locked.store(false, Ordering::Release);

                    primitive.fuel_consumed.fetch_add(MUTEX_UNLOCK_FUEL, Ordering::Relaxed);
                    self.sync_stats.total_mutex_unlocks.fetch_add(1, Ordering::Relaxed);
                },
                _ => return Err(Error::validation_invalid_input("Primitive is not a mutex")),
            }
        }

        // Wake next waiter after releasing the primitive borrow
        let primitive = self
            .primitives
            .get_mut(&primitive_id)
            .ok_or_else(|| Error::validation_invalid_input("Mutex not found"))?;
        Self::wake_next_mutex_waiter(primitive)?;

        Ok(())
    }

    /// Acquire semaphore permit
    pub fn acquire_semaphore(
        &mut self,
        primitive_id: SyncPrimitiveId,
        task_id: TaskId,
        component_id: ComponentInstanceId,
    ) -> Result<SemaphoreAcquireResult> {
        // Extract timestamp before mutable borrow
        let timestamp = self.get_timestamp();

        let primitive = self
            .primitives
            .get_mut(&primitive_id)
            .ok_or_else(|| Error::validation_invalid_input("Semaphore not found"))?;

        match &mut primitive.primitive_type {
            SyncPrimitiveType::AsyncSemaphore {
                permits,
                fair_scheduling,
                ..
            } => {
                let current_permits = permits.load(Ordering::Acquire);

                if current_permits == 0 {
                    // Add to waiters
                    let waiter = WaiterInfo {
                        task_id,
                        component_id,
                        waker: None,
                        wait_type: WaitType::SemaphorePermit,
                        queued_at: timestamp,
                        priority: 128, // Normal priority
                    };

                    primitive.waiters.push(waiter).map_err(|_| {
                        Error::resource_limit_exceeded("Semaphore waiter queue full")
                    })?;

                    return Ok(SemaphoreAcquireResult::WouldBlock);
                }

                // Acquire permit
                permits.fetch_sub(1, Ordering::AcqRel);
                primitive.fuel_consumed.fetch_add(SEMAPHORE_ACQUIRE_FUEL, Ordering::Relaxed);
                self.sync_stats.total_semaphore_acquires.fetch_add(1, Ordering::Relaxed);

                Ok(SemaphoreAcquireResult::Acquired)
            },
            _ => Err(Error::validation_invalid_input(
                "Primitive is not a semaphore",
            )),
        }
    }

    /// Release semaphore permit
    pub fn release_semaphore(&mut self, primitive_id: SyncPrimitiveId) -> Result<()> {
        {
            let primitive = self
                .primitives
                .get_mut(&primitive_id)
                .ok_or_else(|| Error::validation_invalid_input("Semaphore not found"))?;

            match &mut primitive.primitive_type {
                SyncPrimitiveType::AsyncSemaphore {
                    permits,
                    max_permits,
                    ..
                } => {
                    let current_permits = permits.load(Ordering::Acquire);

                    if current_permits >= *max_permits {
                        return Err(Error::invalid_state_error(
                            "Semaphore already at maximum permits",
                        ));
                    }

                    // Release permit
                    permits.fetch_add(1, Ordering::AcqRel);
                    primitive.fuel_consumed.fetch_add(SEMAPHORE_RELEASE_FUEL, Ordering::Relaxed);
                    self.sync_stats.total_semaphore_releases.fetch_add(1, Ordering::Relaxed);
                },
                _ => {
                    return Err(Error::validation_invalid_input(
                        "Primitive is not a semaphore",
                    ));
                },
            }
        }

        // Wake next waiter after releasing the primitive borrow
        let primitive = self
            .primitives
            .get_mut(&primitive_id)
            .ok_or_else(|| Error::validation_invalid_input("Semaphore not found"))?;
        Self::wake_next_semaphore_waiter(primitive)?;

        Ok(())
    }

    /// Wait at async barrier
    pub fn wait_barrier(
        &mut self,
        primitive_id: SyncPrimitiveId,
        task_id: TaskId,
        component_id: ComponentInstanceId,
    ) -> Result<BarrierWaitResult> {
        // Extract timestamp before mutable borrow
        let timestamp = self.get_timestamp();

        let primitive = self
            .primitives
            .get_mut(&primitive_id)
            .ok_or_else(|| Error::validation_invalid_input("Barrier not found"))?;

        match &mut primitive.primitive_type {
            SyncPrimitiveType::AsyncBarrier {
                parties,
                waiting,
                generation,
                broken,
            } => {
                if broken.load(Ordering::Acquire) {
                    return Err(Error::invalid_state_error("Barrier is broken"));
                }

                let current_waiting = waiting.fetch_add(1, Ordering::AcqRel);

                if current_waiting + 1 == *parties {
                    // Last party arrived - release all
                    generation.fetch_add(1, Ordering::AcqRel);
                    waiting.store(0, Ordering::Release);

                    primitive.fuel_consumed.fetch_add(BARRIER_WAIT_FUEL, Ordering::Relaxed);
                    self.sync_stats.total_barrier_waits.fetch_add(1, Ordering::Relaxed);

                    // Need to wake waiters after releasing the primitive borrow
                    let result = BarrierWaitResult::Leader;
                    // primitive reference drops here automatically

                    let primitive = self
                        .primitives
                        .get_mut(&primitive_id)
                        .ok_or_else(|| Error::validation_invalid_input("Barrier not found"))?;
                    Self::wake_all_barrier_waiters(primitive)?;

                    Ok(result)
                } else {
                    // Not the last party - wait
                    let waiter = WaiterInfo {
                        task_id,
                        component_id,
                        waker: None,
                        wait_type: WaitType::BarrierWait,
                        queued_at: timestamp,
                        priority: 128, // Normal priority
                    };

                    primitive
                        .waiters
                        .push(waiter)
                        .map_err(|_| Error::resource_limit_exceeded("Barrier waiter queue full"))?;

                    Ok(BarrierWaitResult::WouldBlock)
                }
            },
            _ => Err(Error::validation_invalid_input(
                "Primitive is not a barrier",
            )),
        }
    }

    /// Get sync primitive statistics
    pub fn get_sync_statistics(&self) -> SyncStats {
        SyncStats {
            total_mutexes_created: self.sync_stats.total_mutexes_created.load(Ordering::Relaxed),
            total_mutex_locks: self.sync_stats.total_mutex_locks.load(Ordering::Relaxed),
            total_mutex_unlocks: self.sync_stats.total_mutex_unlocks.load(Ordering::Relaxed),
            total_semaphores_created: self
                .sync_stats
                .total_semaphores_created
                .load(Ordering::Relaxed),
            total_semaphore_acquires: self
                .sync_stats
                .total_semaphore_acquires
                .load(Ordering::Relaxed),
            total_semaphore_releases: self
                .sync_stats
                .total_semaphore_releases
                .load(Ordering::Relaxed),
            total_barriers_created: self.sync_stats.total_barriers_created.load(Ordering::Relaxed),
            total_barrier_waits: self.sync_stats.total_barrier_waits.load(Ordering::Relaxed),
            total_condvars_created: self.sync_stats.total_condvars_created.load(Ordering::Relaxed),
            total_condvar_waits: self.sync_stats.total_condvar_waits.load(Ordering::Relaxed),
            total_condvar_notifies: self.sync_stats.total_condvar_notifies.load(Ordering::Relaxed),
            active_primitives: self.primitives.len() as u64,
            deadlocks_detected: self.sync_stats.deadlocks_detected.load(Ordering::Relaxed),
            priority_inversions: self.sync_stats.priority_inversions.load(Ordering::Relaxed),
            total_fuel_consumed: self.sync_stats.total_fuel_consumed.load(Ordering::Relaxed),
        }
    }

    // Private helper methods

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }

    fn wake_next_mutex_waiter(primitive: &mut SyncPrimitive) -> Result<()> {
        // Find next waiter for mutex
        let mut waiter_index = None;
        for (i, waiter) in primitive.waiters.iter().enumerate() {
            if waiter.wait_type == WaitType::MutexLock {
                waiter_index = Some(i);
                break;
            }
        }

        if let Some(index) = waiter_index {
            if let Some(waiter) = primitive.waiters.get(index) {
                if let Some(waker) = &waiter.waker {
                    waker.wake_by_ref();
                }
            }
            primitive.waiters.remove(index);
        }

        Ok(())
    }

    fn wake_next_semaphore_waiter(primitive: &mut SyncPrimitive) -> Result<()> {
        // Find next waiter for semaphore
        let mut waiter_index = None;
        for (i, waiter) in primitive.waiters.iter().enumerate() {
            if waiter.wait_type == WaitType::SemaphorePermit {
                waiter_index = Some(i);
                break;
            }
        }

        if let Some(index) = waiter_index {
            if let Some(waiter) = primitive.waiters.get(index) {
                if let Some(waker) = &waiter.waker {
                    waker.wake_by_ref();
                }
            }
            primitive.waiters.remove(index);
        }

        Ok(())
    }

    fn wake_all_barrier_waiters(primitive: &mut SyncPrimitive) -> Result<()> {
        // Wake all barrier waiters
        for waiter in primitive.waiters.iter() {
            if waiter.wait_type == WaitType::BarrierWait {
                if let Some(waker) = &waiter.waker {
                    waker.wake_by_ref();
                }
            }
        }

        // Remove all barrier waiters
        primitive.waiters.retain(|waiter| waiter.wait_type != WaitType::BarrierWait);

        Ok(())
    }
}

/// Result of mutex lock operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutexLockResult {
    /// Lock acquired immediately
    Acquired,
    /// Would block, caller should wait
    WouldBlock,
}

/// Result of semaphore acquire operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemaphoreAcquireResult {
    /// Permit acquired immediately
    Acquired,
    /// Would block, caller should wait
    WouldBlock,
}

/// Result of barrier wait operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierWaitResult {
    /// This task is the leader (last to arrive)
    Leader,
    /// Would block, waiting for other parties
    WouldBlock,
    /// Barrier was broken
    Broken,
}

/// Sync statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    pub total_mutexes_created: u64,
    pub total_mutex_locks: u64,
    pub total_mutex_unlocks: u64,
    pub total_semaphores_created: u64,
    pub total_semaphore_acquires: u64,
    pub total_semaphore_releases: u64,
    pub total_barriers_created: u64,
    pub total_barrier_waits: u64,
    pub total_condvars_created: u64,
    pub total_condvar_waits: u64,
    pub total_condvar_notifies: u64,
    pub active_primitives: u64,
    pub deadlocks_detected: u64,
    pub priority_inversions: u64,
    pub total_fuel_consumed: u64,
}

/// Async mutex guard
pub struct AsyncMutexGuard {
    primitive_id: SyncPrimitiveId,
    task_id: TaskId,
    sync_primitives: Weak<Mutex<AdvancedSyncPrimitives>>,
}

impl Drop for AsyncMutexGuard {
    fn drop(&mut self) {
        if let Some(sync_primitives) = self.sync_primitives.upgrade() {
            let mut primitives = sync_primitives.lock();
            let _ = primitives.unlock_async_mutex(self.primitive_id, self.task_id);
        }
    }
}

/// Async semaphore permit
pub struct AsyncSemaphorePermit {
    primitive_id: SyncPrimitiveId,
    sync_primitives: Weak<Mutex<AdvancedSyncPrimitives>>,
}

impl Drop for AsyncSemaphorePermit {
    fn drop(&mut self) {
        if let Some(sync_primitives) = self.sync_primitives.upgrade() {
            let mut primitives = sync_primitives.lock();
            let _ = primitives.release_semaphore(self.primitive_id);
        }
    }
}

/// Future for mutex lock operation
pub struct MutexLockFuture {
    primitive_id: SyncPrimitiveId,
    task_id: TaskId,
    component_id: ComponentInstanceId,
    sync_primitives: Weak<Mutex<AdvancedSyncPrimitives>>,
}

impl CoreFuture for MutexLockFuture {
    type Output = Result<AsyncMutexGuard>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(sync_primitives) = self.sync_primitives.upgrade() {
            let mut primitives = sync_primitives.lock();
            match primitives.lock_async_mutex(self.primitive_id, self.task_id, self.component_id) {
                Ok(MutexLockResult::Acquired) => Poll::Ready(Ok(AsyncMutexGuard {
                    primitive_id: self.primitive_id,
                    task_id: self.task_id,
                    sync_primitives: self.sync_primitives.clone(),
                })),
                Ok(MutexLockResult::WouldBlock) => {
                    // Register waker
                    if let Some(primitive) = primitives.primitives.get_mut(&self.primitive_id) {
                        for waiter in primitive.waiters.iter_mut() {
                            if waiter.task_id == self.task_id {
                                waiter.waker = Some(cx.waker().clone());
                                break;
                            }
                        }
                    }
                    Poll::Pending
                },
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            Poll::Ready(Err(Error::invalid_state_error(
                "Sync primitives manager dropped",
            )))
        }
    }
}
