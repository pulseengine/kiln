//! Timer and timeout integration for async operations
//!
//! This module provides timer functionality and timeout handling
//! integrated with the fuel-based async executor.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Weak;
#[cfg(not(any(feature = "std", feature = "alloc")))]
use core::mem::ManuallyDrop as Weak; // Placeholder for no_std
use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};
#[cfg(feature = "std")]
use std::sync::Weak;

use wrt_foundation::{
    Arc,
    CrateId,
    Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    // BoundedBinaryHeap, // Not available
    component_value::ComponentValue,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    // sync::Mutex, // Import from local instead
    traits::{Checksummable, FromBytes, ToBytes},
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

// Placeholder types for missing imports
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;
pub type BoundedBinaryHeap<T, const N: usize> = BoundedVec<T, N>;

/// Maximum timers per component
const MAX_TIMERS_PER_COMPONENT: usize = 128;

/// Maximum global timers
const MAX_GLOBAL_TIMERS: usize = 1024;

/// Fuel costs for timer operations
const TIMER_CREATE_FUEL: u64 = 30;
const TIMER_CANCEL_FUEL: u64 = 10;
const TIMER_FIRE_FUEL: u64 = 15;
const TIMEOUT_FUEL: u64 = 25;

/// Timer and timeout manager
pub struct TimerIntegration {
    /// Bridge for task management
    bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
    /// Active timers
    timers: BoundedMap<TimerId, Timer, MAX_GLOBAL_TIMERS>,
    /// Timer queue ordered by expiration time
    timer_queue: BoundedBinaryHeap<TimerEntry, MAX_GLOBAL_TIMERS>,
    /// Component timer contexts
    component_contexts: BoundedMap<ComponentInstanceId, ComponentTimerContext, 128>,
    /// Next timer ID
    next_timer_id: AtomicU64,
    /// Current time (simulated)
    current_time: AtomicU64,
    /// Timer statistics
    timer_stats: TimerStatistics,
    /// Timer configuration
    timer_config: TimerConfiguration,
}

/// Timer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimerId(u64);

impl Checksummable for TimerId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for TimerId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for TimerId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u64::from_bytes_with_provider(reader, provider)?))
    }
}

/// Timer implementation
#[derive(Debug)]
struct Timer {
    id: TimerId,
    component_id: ComponentInstanceId,
    timer_type: TimerType,
    expiration_time: u64,
    interval: Option<u64>,
    waker: Option<Waker>,
    cancelled: AtomicBool,
    fired_count: AtomicU32,
    fuel_consumed: AtomicU64,
    created_at: u64,
}

/// Type of timer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerType {
    /// One-shot timer
    Oneshot,
    /// Repeating interval timer
    Interval(u64),
    /// Deadline timer (absolute time)
    Deadline(u64),
    /// Timeout for an operation
    Timeout {
        operation_id: u64,
        timeout_duration: u64,
    },
    /// Rate-limited timer
    RateLimit {
        max_fires_per_period: u32,
        period_ms: u64,
    },
}

impl Default for TimerType {
    fn default() -> Self {
        Self::Oneshot
    }
}

impl Checksummable for TimerType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            TimerType::Oneshot => 0u8.update_checksum(checksum),
            TimerType::Interval(dur) => {
                1u8.update_checksum(checksum);
                dur.update_checksum(checksum);
            },
            TimerType::Deadline(time) => {
                2u8.update_checksum(checksum);
                time.update_checksum(checksum);
            },
            TimerType::Timeout {
                operation_id,
                timeout_duration,
            } => {
                3u8.update_checksum(checksum);
                operation_id.update_checksum(checksum);
                timeout_duration.update_checksum(checksum);
            },
            TimerType::RateLimit {
                max_fires_per_period,
                period_ms,
            } => {
                4u8.update_checksum(checksum);
                max_fires_per_period.update_checksum(checksum);
                period_ms.update_checksum(checksum);
            },
        }
    }
}

impl ToBytes for TimerType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            TimerType::Oneshot => 0u8.to_bytes_with_provider(writer, provider),
            TimerType::Interval(dur) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                dur.to_bytes_with_provider(writer, provider)
            },
            TimerType::Deadline(time) => {
                2u8.to_bytes_with_provider(writer, provider)?;
                time.to_bytes_with_provider(writer, provider)
            },
            TimerType::Timeout {
                operation_id,
                timeout_duration,
            } => {
                3u8.to_bytes_with_provider(writer, provider)?;
                operation_id.to_bytes_with_provider(writer, provider)?;
                timeout_duration.to_bytes_with_provider(writer, provider)
            },
            TimerType::RateLimit {
                max_fires_per_period,
                period_ms,
            } => {
                4u8.to_bytes_with_provider(writer, provider)?;
                max_fires_per_period.to_bytes_with_provider(writer, provider)?;
                period_ms.to_bytes_with_provider(writer, provider)
            },
        }
    }
}

impl FromBytes for TimerType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(TimerType::Oneshot),
            1 => {
                let dur = u64::from_bytes_with_provider(reader, provider)?;
                Ok(TimerType::Interval(dur))
            },
            2 => {
                let time = u64::from_bytes_with_provider(reader, provider)?;
                Ok(TimerType::Deadline(time))
            },
            3 => {
                let operation_id = u64::from_bytes_with_provider(reader, provider)?;
                let timeout_duration = u64::from_bytes_with_provider(reader, provider)?;
                Ok(TimerType::Timeout {
                    operation_id,
                    timeout_duration,
                })
            },
            4 => {
                let max_fires_per_period = u32::from_bytes_with_provider(reader, provider)?;
                let period_ms = u64::from_bytes_with_provider(reader, provider)?;
                Ok(TimerType::RateLimit {
                    max_fires_per_period,
                    period_ms,
                })
            },
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid TimerType discriminant",
            )),
        }
    }
}

/// Timer queue entry
#[derive(Debug, Clone, Default)]
struct TimerEntry {
    timer_id: TimerId,
    expiration_time: u64,
    sequence: u64, // For stable sorting
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // Earlier times have higher priority (reversed for min-heap)
        other
            .expiration_time
            .cmp(&self.expiration_time)
            .then(other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.timer_id == other.timer_id
    }
}

impl Eq for TimerEntry {}

impl Checksummable for TimerEntry {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.timer_id.update_checksum(checksum);
        self.expiration_time.update_checksum(checksum);
        self.sequence.update_checksum(checksum);
    }
}

impl ToBytes for TimerEntry {
    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        provider: &P,
    ) -> core::result::Result<(), wrt_error::Error> {
        self.timer_id.to_bytes_with_provider(writer, provider)?;
        self.expiration_time.to_bytes_with_provider(writer, provider)?;
        self.sequence.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for TimerEntry {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        provider: &P,
    ) -> core::result::Result<Self, wrt_error::Error> {
        Ok(Self {
            timer_id: TimerId::from_bytes_with_provider(reader, provider)?,
            expiration_time: u64::from_bytes_with_provider(reader, provider)?,
            sequence: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Component timer context
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ComponentTimerContext {
    component_id: ComponentInstanceId,
    /// Timers owned by this component
    owned_timers: BoundedVec<TimerId, MAX_TIMERS_PER_COMPONENT>,
    /// Active timeouts
    active_timeouts: BoundedMap<u64, TimerId, 64>, // operation_id -> timer_id
    /// Timer limits
    timer_limits: TimerLimits,
    /// Rate limiting state
    rate_limit_state: RateLimitState,
}

impl wrt_foundation::traits::Checksummable for ComponentTimerContext {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.component_id.update_checksum(checksum);
        self.owned_timers.update_checksum(checksum);
        self.active_timeouts.update_checksum(checksum);
        self.timer_limits.update_checksum(checksum);
        self.rate_limit_state.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for ComponentTimerContext {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.owned_timers.to_bytes_with_provider(writer, provider)?;
        self.active_timeouts.to_bytes_with_provider(writer, provider)?;
        self.timer_limits.to_bytes_with_provider(writer, provider)?;
        self.rate_limit_state.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ComponentTimerContext {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            component_id: ComponentInstanceId::new(u32::from_bytes_with_provider(
                reader, provider,
            )?),
            owned_timers: BoundedVec::from_bytes_with_provider(reader, provider)?,
            active_timeouts: BoundedMap::from_bytes_with_provider(reader, provider)?,
            timer_limits: TimerLimits::from_bytes_with_provider(reader, provider)?,
            rate_limit_state: RateLimitState::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Timer limits per component
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimerLimits {
    max_timers: usize,
    max_timeout_duration_ms: u64,
    max_interval_duration_ms: u64,
    fuel_budget: u64,
}

impl wrt_foundation::traits::Checksummable for TimerLimits {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.max_timers.update_checksum(checksum);
        self.max_timeout_duration_ms.update_checksum(checksum);
        self.max_interval_duration_ms.update_checksum(checksum);
        self.fuel_budget.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for TimerLimits {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.max_timers.to_bytes_with_provider(writer, provider)?;
        self.max_timeout_duration_ms.to_bytes_with_provider(writer, provider)?;
        self.max_interval_duration_ms.to_bytes_with_provider(writer, provider)?;
        self.fuel_budget.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for TimerLimits {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            max_timers: usize::from_bytes_with_provider(reader, provider)?,
            max_timeout_duration_ms: u64::from_bytes_with_provider(reader, provider)?,
            max_interval_duration_ms: u64::from_bytes_with_provider(reader, provider)?,
            fuel_budget: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Rate limiting state
#[derive(Debug)]
struct RateLimitState {
    /// Number of timers fired in current period
    fires_this_period: AtomicU32,
    /// Start of current period
    period_start: AtomicU64,
    /// Maximum fires allowed per period
    max_fires_per_period: u32,
    /// Period duration in ms
    period_duration_ms: u64,
}

impl Clone for RateLimitState {
    fn clone(&self) -> Self {
        Self {
            fires_this_period: AtomicU32::new(self.fires_this_period.load(Ordering::Relaxed)),
            period_start: AtomicU64::new(self.period_start.load(Ordering::Relaxed)),
            max_fires_per_period: self.max_fires_per_period,
            period_duration_ms: self.period_duration_ms,
        }
    }
}

impl PartialEq for RateLimitState {
    fn eq(&self, other: &Self) -> bool {
        self.fires_this_period.load(Ordering::Relaxed)
            == other.fires_this_period.load(Ordering::Relaxed)
            && self.period_start.load(Ordering::Relaxed)
                == other.period_start.load(Ordering::Relaxed)
            && self.max_fires_per_period == other.max_fires_per_period
            && self.period_duration_ms == other.period_duration_ms
    }
}

impl Eq for RateLimitState {}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            fires_this_period: AtomicU32::new(0),
            period_start: AtomicU64::new(0),
            max_fires_per_period: 0,
            period_duration_ms: 0,
        }
    }
}

impl wrt_foundation::traits::Checksummable for RateLimitState {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        use wrt_runtime::Checksummable;
        self.fires_this_period.load(Ordering::Relaxed).update_checksum(checksum);
        self.period_start.load(Ordering::Relaxed).update_checksum(checksum);
        self.max_fires_per_period.update_checksum(checksum);
        self.period_duration_ms.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for RateLimitState {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        use wrt_runtime::ToBytes;
        self.fires_this_period
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.period_start
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.max_fires_per_period.to_bytes_with_provider(writer, provider)?;
        self.period_duration_ms.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for RateLimitState {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_runtime::FromBytes;
        Ok(Self {
            fires_this_period: AtomicU32::new(u32::from_bytes_with_provider(reader, provider)?),
            period_start: AtomicU64::new(u64::from_bytes_with_provider(reader, provider)?),
            max_fires_per_period: u32::from_bytes_with_provider(reader, provider)?,
            period_duration_ms: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Timer configuration
#[derive(Debug, Clone)]
pub struct TimerConfiguration {
    pub enable_high_precision: bool,
    pub min_timer_resolution_ms: u64,
    pub max_timer_duration_ms: u64,
    pub enable_rate_limiting: bool,
    pub default_rate_limit: u32,
    pub enable_fuel_tracking: bool,
}

impl Default for TimerConfiguration {
    fn default() -> Self {
        Self {
            enable_high_precision: false,
            min_timer_resolution_ms: 1,
            max_timer_duration_ms: 24 * 60 * 60 * 1000, // 24 hours
            enable_rate_limiting: true,
            default_rate_limit: 100, // 100 timers per second
            enable_fuel_tracking: true,
        }
    }
}

/// Timer statistics
#[derive(Debug, Default)]
struct TimerStatistics {
    total_timers_created: AtomicU64,
    total_timers_fired: AtomicU64,
    total_timers_cancelled: AtomicU64,
    total_timeouts_created: AtomicU64,
    total_timeouts_expired: AtomicU64,
    total_fuel_consumed: AtomicU64,
    max_concurrent_timers: AtomicU32,
}

impl TimerIntegration {
    /// Create new timer integration
    pub fn new(
        bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
        config: Option<TimerConfiguration>,
    ) -> Self {
        Self {
            bridge,
            timers: BoundedMap::new(),
            timer_queue: BoundedBinaryHeap::new().unwrap(),
            component_contexts: BoundedMap::new(),
            next_timer_id: AtomicU64::new(1),
            current_time: AtomicU64::new(0),
            timer_stats: TimerStatistics::default(),
            timer_config: config.unwrap_or_default(),
        }
    }

    /// Initialize component for timer operations
    pub fn initialize_component_timers(
        &mut self,
        component_id: ComponentInstanceId,
        limits: Option<TimerLimits>,
    ) -> Result<()> {
        let limits = limits.unwrap_or(TimerLimits {
            max_timers: MAX_TIMERS_PER_COMPONENT,
            max_timeout_duration_ms: self.timer_config.max_timer_duration_ms,
            max_interval_duration_ms: self.timer_config.max_timer_duration_ms,
            fuel_budget: 50_000,
        });

        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        let context = ComponentTimerContext {
            component_id,
            owned_timers: BoundedVec::new().unwrap(),
            active_timeouts: BoundedMap::new(),
            timer_limits: limits,
            rate_limit_state: RateLimitState {
                fires_this_period: AtomicU32::new(0),
                period_start: AtomicU64::new(self.get_current_time()),
                max_fires_per_period: self.timer_config.default_rate_limit,
                period_duration_ms: 1000, // 1 second
            },
        };

        self.component_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component timer contexts"))?;

        Ok(())
    }

    /// Create a new timer
    pub fn create_timer(
        &mut self,
        component_id: ComponentInstanceId,
        timer_type: TimerType,
        duration_ms: u64,
    ) -> Result<TimerId> {
        // Extract values before mutable borrows to avoid borrow conflicts
        let timer_id = TimerId(self.next_timer_id.fetch_add(1, Ordering::AcqRel));
        let current_time = self.get_current_time();
        let expiration_time = current_time + duration_ms;
        let timer_sequence = self.timer_stats.total_timers_created.load(Ordering::Relaxed);

        let context = self.component_contexts.get_mut(&component_id).ok_or_else(|| {
            Error::validation_invalid_input("Component not initialized for timers")
        })?;

        // Check limits
        if context.owned_timers.len() >= context.timer_limits.max_timers {
            return Err(Error::resource_limit_exceeded(
                "Component timer limit exceeded",
            ));
        }

        // Validate duration
        if duration_ms > context.timer_limits.max_timeout_duration_ms {
            return Err(Error::validation_invalid_input(
                "Timer duration exceeds maximum",
            ));
        }

        if duration_ms < self.timer_config.min_timer_resolution_ms {
            return Err(Error::validation_invalid_input(
                "Timer duration below minimum resolution",
            ));
        }

        let timer = Timer {
            id: timer_id,
            component_id,
            timer_type: timer_type.clone(),
            expiration_time,
            interval: match &timer_type {
                TimerType::Interval(interval) => Some(*interval),
                _ => None,
            },
            waker: None,
            cancelled: AtomicBool::new(false),
            fired_count: AtomicU32::new(0),
            fuel_consumed: AtomicU64::new(TIMER_CREATE_FUEL),
            created_at: current_time,
        };

        // Add to timer queue
        let timer_entry = TimerEntry {
            timer_id,
            expiration_time,
            sequence: timer_sequence,
        };

        self.timer_queue
            .push(timer_entry)
            .map_err(|_| Error::resource_limit_exceeded("Timer queue full"))?;

        // Store timer
        self.timers
            .insert(timer_id, timer)
            .map_err(|_| Error::resource_limit_exceeded("Too many active timers"))?;

        // Add to component context
        context
            .owned_timers
            .push(timer_id)
            .map_err(|_| Error::resource_limit_exceeded("Component timer list full"))?;

        // Update statistics
        self.timer_stats.total_timers_created.fetch_add(1, Ordering::Relaxed);
        let current_count = self.timers.len() as u32;
        let max_count = self.timer_stats.max_concurrent_timers.load(Ordering::Relaxed);
        if current_count > max_count {
            self.timer_stats.max_concurrent_timers.store(current_count, Ordering::Relaxed);
        }

        Ok(timer_id)
    }

    /// Create a timeout for an operation
    pub fn create_timeout(
        &mut self,
        component_id: ComponentInstanceId,
        operation_id: u64,
        timeout_duration_ms: u64,
    ) -> Result<TimerId> {
        let timer_type = TimerType::Timeout {
            operation_id,
            timeout_duration: timeout_duration_ms,
        };

        let timer_id = self.create_timer(component_id, timer_type, timeout_duration_ms)?;

        // Add to component timeout tracking
        if let Some(context) = self.component_contexts.get_mut(&component_id) {
            context.active_timeouts.insert(operation_id, timer_id).ok();
        }

        self.timer_stats.total_timeouts_created.fetch_add(1, Ordering::Relaxed);

        Ok(timer_id)
    }

    /// Cancel a timer
    pub fn cancel_timer(&mut self, timer_id: TimerId) -> Result<bool> {
        let timer = self
            .timers
            .get_mut(&timer_id)
            .ok_or_else(|| Error::validation_invalid_input("Timer not found"))?;

        let was_cancelled = timer
            .cancelled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();

        if was_cancelled {
            timer.fuel_consumed.fetch_add(TIMER_CANCEL_FUEL, Ordering::Relaxed);
            self.timer_stats.total_timers_cancelled.fetch_add(1, Ordering::Relaxed);

            // Remove from component timeout tracking if applicable
            if let TimerType::Timeout { operation_id, .. } = &timer.timer_type {
                if let Some(context) = self.component_contexts.get_mut(&timer.component_id) {
                    context.active_timeouts.remove(operation_id);
                }
            }
        }

        Ok(was_cancelled)
    }

    /// Process expired timers
    pub fn process_timers(&mut self) -> Result<TimerProcessResult> {
        let current_time = self.get_current_time();
        let mut fired_timers = Vec::new();
        let mut timers_to_reschedule = Vec::new();

        // Process expired timers
        while let Some(timer_entry) = self.timer_queue.peek() {
            if timer_entry.expiration_time > current_time {
                break; // No more expired timers
            }

            let timer_entry = self.timer_queue.pop().unwrap();
            let timer_id = timer_entry.timer_id;

            // Extract timer data first to avoid borrow conflicts
            let (cancelled, component_id) = if let Some(timer) = self.timers.get(&timer_id) {
                (timer.cancelled.load(Ordering::Acquire), timer.component_id)
            } else {
                continue;
            };

            if cancelled {
                // Timer was cancelled, remove it
                self.cleanup_timer(timer_id)?;
                continue;
            }

            // Check rate limiting before getting mutable borrow
            if self.timer_config.enable_rate_limiting && !self.check_rate_limit(component_id)? {
                // Rate limit exceeded, reschedule for later
                let new_entry = TimerEntry {
                    timer_id,
                    expiration_time: current_time + 100, // Delay 100ms
                    sequence: timer_entry.sequence,
                };
                timers_to_reschedule.push(new_entry);
                continue;
            }

            // Now safely get mutable borrow
            if let Some(timer) = self.timers.get_mut(&timer_id) {
                // Fire the timer
                timer.fired_count.fetch_add(1, Ordering::AcqRel);
                timer.fuel_consumed.fetch_add(TIMER_FIRE_FUEL, Ordering::Relaxed);
                fired_timers.push(timer_id);

                // Wake the timer's waker if available
                if let Some(waker) = timer.waker.take() {
                    waker.wake();
                }

                // Handle repeating timers
                match &timer.timer_type {
                    TimerType::Interval(interval) => {
                        // Reschedule interval timer
                        timer.expiration_time = current_time + interval;
                        let new_entry = TimerEntry {
                            timer_id,
                            expiration_time: timer.expiration_time,
                            sequence: self.timer_stats.total_timers_fired.load(Ordering::Relaxed),
                        };
                        timers_to_reschedule.push(new_entry);
                    },
                    TimerType::Timeout { operation_id, .. } => {
                        // Timeout expired
                        self.timer_stats.total_timeouts_expired.fetch_add(1, Ordering::Relaxed);

                        // Remove from component timeout tracking
                        if let Some(context) = self.component_contexts.get_mut(&timer.component_id)
                        {
                            context.active_timeouts.remove(operation_id);
                        }

                        // Mark for cleanup
                        self.cleanup_timer(timer_id)?;
                    },
                    _ => {
                        // One-shot timer, mark for cleanup
                        self.cleanup_timer(timer_id)?;
                    },
                }
            }
        }

        // Reschedule timers that need it
        for timer_entry in timers_to_reschedule {
            self.timer_queue.push(timer_entry).ok();
        }

        // Update statistics
        let fired_count = fired_timers.len();
        self.timer_stats
            .total_timers_fired
            .fetch_add(fired_count as u64, Ordering::Relaxed);

        Ok(TimerProcessResult {
            fired_timers,
            expired_timeouts: 0, // Would count timeout expirations
            processed_count: fired_count,
        })
    }

    /// Advance time (for simulation/testing)
    pub fn advance_time(&mut self, duration_ms: u64) {
        self.current_time.fetch_add(duration_ms, Ordering::AcqRel);
    }

    /// Get timer status
    pub fn get_timer_status(&self, timer_id: TimerId) -> Result<TimerStatus> {
        let timer = self
            .timers
            .get(&timer_id)
            .ok_or_else(|| Error::validation_invalid_input("Timer not found"))?;

        let current_time = self.get_current_time();
        let time_remaining = if timer.expiration_time > current_time {
            Some(timer.expiration_time - current_time)
        } else {
            None
        };

        Ok(TimerStatus {
            timer_id,
            component_id: timer.component_id,
            timer_type: timer.timer_type.clone(),
            expiration_time: timer.expiration_time,
            time_remaining,
            fired_count: timer.fired_count.load(Ordering::Acquire),
            cancelled: timer.cancelled.load(Ordering::Acquire),
            fuel_consumed: timer.fuel_consumed.load(Ordering::Acquire),
        })
    }

    /// Get timer statistics
    pub fn get_timer_statistics(&self) -> TimerStats {
        TimerStats {
            total_timers_created: self.timer_stats.total_timers_created.load(Ordering::Relaxed),
            total_timers_fired: self.timer_stats.total_timers_fired.load(Ordering::Relaxed),
            total_timers_cancelled: self.timer_stats.total_timers_cancelled.load(Ordering::Relaxed),
            total_timeouts_created: self.timer_stats.total_timeouts_created.load(Ordering::Relaxed),
            total_timeouts_expired: self.timer_stats.total_timeouts_expired.load(Ordering::Relaxed),
            active_timers: self.timers.len() as u64,
            max_concurrent_timers: self.timer_stats.max_concurrent_timers.load(Ordering::Relaxed)
                as u64,
            total_fuel_consumed: self.timer_stats.total_fuel_consumed.load(Ordering::Relaxed),
        }
    }

    // Private helper methods

    fn get_current_time(&self) -> u64 {
        self.current_time.load(Ordering::Acquire)
    }

    fn check_rate_limit(&mut self, component_id: ComponentInstanceId) -> Result<bool> {
        // Get current time first to avoid borrow conflicts
        let current_time = self.get_current_time();

        let context = self
            .component_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not found"))?;

        let period_start = context.rate_limit_state.period_start.load(Ordering::Acquire);

        // Check if we need to reset the period
        if current_time - period_start >= context.rate_limit_state.period_duration_ms {
            context.rate_limit_state.period_start.store(current_time, Ordering::Release);
            context.rate_limit_state.fires_this_period.store(0, Ordering::Release);
        }

        let fires_this_period = context.rate_limit_state.fires_this_period.load(Ordering::Acquire);
        if fires_this_period >= context.rate_limit_state.max_fires_per_period {
            return Ok(false); // Rate limit exceeded
        }

        context.rate_limit_state.fires_this_period.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }

    fn cleanup_timer(&mut self, timer_id: TimerId) -> Result<()> {
        if let Some(timer) = self.timers.remove(&timer_id) {
            // Remove from component context
            if let Some(context) = self.component_contexts.get_mut(&timer.component_id) {
                context.owned_timers.retain(|&id| id != timer_id);
            }

            // Add fuel to total consumption
            let fuel_consumed = timer.fuel_consumed.load(Ordering::Acquire);
            self.timer_stats.total_fuel_consumed.fetch_add(fuel_consumed, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Timer processing result
#[derive(Debug, Clone)]
pub struct TimerProcessResult {
    pub fired_timers: Vec<TimerId>,
    pub expired_timeouts: usize,
    pub processed_count: usize,
}

/// Timer status
#[derive(Debug, Clone)]
pub struct TimerStatus {
    pub timer_id: TimerId,
    pub component_id: ComponentInstanceId,
    pub timer_type: TimerType,
    pub expiration_time: u64,
    pub time_remaining: Option<u64>,
    pub fired_count: u32,
    pub cancelled: bool,
    pub fuel_consumed: u64,
}

/// Timer statistics
#[derive(Debug, Clone)]
pub struct TimerStats {
    pub total_timers_created: u64,
    pub total_timers_fired: u64,
    pub total_timers_cancelled: u64,
    pub total_timeouts_created: u64,
    pub total_timeouts_expired: u64,
    pub active_timers: u64,
    pub max_concurrent_timers: u64,
    pub total_fuel_consumed: u64,
}

/// Timer future for async operations
pub struct TimerFuture {
    timer_id: TimerId,
    timer_integration: Weak<Mutex<TimerIntegration>>,
}

impl CoreFuture for TimerFuture {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // ManuallyDrop doesn't have upgrade - timer_integration is not a Weak reference in no_std
        #[cfg(any(feature = "std", feature = "alloc"))]
        let timer_opt = self.timer_integration.upgrade();
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        let timer_opt = None::<Arc<Mutex<TimerIntegration>>>;

        if let Some(timer_integration) = timer_opt {
            let mut timers = timer_integration.lock();

            // Extract data before mutable borrow
            let (cancelled, expiration_time) =
                if let Some(timer) = timers.timers.get(&self.timer_id) {
                    (
                        timer.cancelled.load(Ordering::Acquire),
                        timer.expiration_time,
                    )
                } else {
                    return Poll::Ready(Err(Error::validation_invalid_input(
                        "Timer operation failed",
                    )));
                };

            if cancelled {
                return Poll::Ready(Err(Error::runtime_execution_error("Timer cancelled")));
            }

            let current_time = timers.get_current_time();
            if current_time >= expiration_time {
                Poll::Ready(Ok(()))
            } else {
                // Store waker for when timer fires
                if let Some(timer) = timers.timers.get_mut(&self.timer_id) {
                    timer.waker = Some(cx.waker().clone());
                }
                Poll::Pending
            }
        } else {
            Poll::Ready(Err(Error::invalid_state_error("Timer manager dropped")))
        }
    }
}

/// Helper function to create a sleep future
pub fn sleep(duration_ms: u64, timer_integration: Weak<Mutex<TimerIntegration>>) -> TimerFuture {
    // In real implementation, would create timer and return future
    TimerFuture {
        timer_id: TimerId(0), // Would be real timer ID
        timer_integration,
    }
}

/// Helper function to create a timeout future
pub async fn timeout<F>(
    future: F,
    duration_ms: u64,
    timer_integration: Weak<Mutex<TimerIntegration>>,
) -> Result<F::Output>
where
    F: CoreFuture,
{
    // In real implementation, would race future against timer
    // For now, just return the future result
    Ok(future.await)
}
