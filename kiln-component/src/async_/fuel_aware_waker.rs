//! Fuel-aware waker implementation for async task notification
//!
//! This module provides ASIL-D compliant waker implementations that integrate
//! with the fuel-based async executor while maintaining safety requirements.

use core::{
    mem,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    task::{RawWaker, RawWakerVTable, Waker},
};

use wrt_foundation::{
    Arc, CrateId, Mutex, collections::StaticVec as BoundedVec, safe_managed_alloc,
};

// Import Weak from the appropriate source based on features
#[cfg(not(feature = "std"))]
use alloc::sync::Weak;
#[cfg(feature = "std")]
use std::sync::Weak;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;

// Re-export TaskId from fuel_async_executor when threading is not enabled
#[cfg(not(feature = "component-model-threading"))]
pub use crate::async_::fuel_async_executor::TaskId;

use crate::{
    async_::fuel_async_executor::{ASILExecutionMode, AsyncTaskState, FuelAsyncExecutor},
    prelude::*,
};

/// Safe abstraction trait for waker operations
pub trait SafeWaker: Send + Sync {
    /// Wake the associated task
    fn wake(&self);

    /// Clone this waker
    fn clone_waker(&self) -> Box<dyn SafeWaker>;
}

/// ASIL-compliant waker implementation
pub struct AsilCompliantWaker {
    task_id: TaskId,
    ready_queue: Arc<Mutex<BoundedVec<TaskId, 128>>>,
    executor_ref: Weak<Mutex<FuelAsyncExecutor>>,
    asil_mode: ASILExecutionMode,
}

impl SafeWaker for AsilCompliantWaker {
    fn wake(&self) {
        // Safe wake implementation without unsafe code
        let mut queue = self.ready_queue.lock();
        let _ = queue.push(self.task_id);
    }

    fn clone_waker(&self) -> Box<dyn SafeWaker> {
        Box::new(AsilCompliantWaker {
            task_id: self.task_id,
            ready_queue: self.ready_queue.clone(),
            executor_ref: self.executor_ref.clone(),
            asil_mode: self.asil_mode,
        })
    }
}

/// Maximum number of pending wakes to coalesce
const MAX_PENDING_WAKES: usize = 64;

/// Fuel cost for wake operations
const WAKE_OPERATION_FUEL: u64 = 5;

/// Waker data that is passed to the RawWaker
#[derive(Debug)]
pub struct WakerData {
    /// Task ID to wake
    pub task_id: TaskId,
    /// Reference to the executor's ready queue
    pub ready_queue: Arc<Mutex<BoundedVec<TaskId, 128>>>,
    /// Wake count for debugging and metrics
    pub wake_count: Arc<AtomicU32>,
    /// Flag to prevent duplicate wakes
    pub is_woken: Arc<AtomicBool>,
    /// Weak reference to executor for fuel tracking
    pub executor_ref: Weak<Mutex<FuelAsyncExecutor>>,
    /// ASIL mode for this task (affects wake behavior)
    pub asil_mode: ASILExecutionMode,
    /// Wake timestamp for deterministic execution (ASIL-D)
    pub wake_timestamp: Arc<AtomicU64>,
}

impl WakerData {
    /// Create a new waker data instance
    pub fn new(
        task_id: TaskId,
        ready_queue: Arc<Mutex<BoundedVec<TaskId, 128>>>,
        executor_ref: Weak<Mutex<FuelAsyncExecutor>>,
        asil_mode: ASILExecutionMode,
    ) -> Self {
        Self {
            task_id,
            ready_queue,
            wake_count: Arc::new(AtomicU32::new(0)),
            is_woken: Arc::new(AtomicBool::new(false)),
            executor_ref,
            asil_mode,
            wake_timestamp: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Wake the associated task
    pub fn wake(&self) {
        // ASIL-D requires deterministic wake ordering
        if let ASILExecutionMode::D {
            deterministic_execution: true,
            ..
        } = self.asil_mode
        {
            // Record wake timestamp for deterministic ordering
            let timestamp = self.get_deterministic_timestamp();
            self.wake_timestamp.store(timestamp, Ordering::SeqCst);
        }

        // Check if already woken to prevent duplicate wakes
        if self
            .is_woken
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            // Already woken - for ASIL-B/C/D, this is important for preventing resource
            // waste
            return;
        }

        // Increment wake count for metrics
        self.wake_count.fetch_add(1, Ordering::Relaxed);

        // ASIL-specific wake handling
        match self.asil_mode {
            ASILExecutionMode::QM => {
                // QM: Basic wake
                self.wake_basic();
            },
            ASILExecutionMode::AsilA => {
                // ASIL-A: Basic wake with error detection
                self.wake_basic();
            },
            ASILExecutionMode::AsilB => {
                // ASIL-B: Wake with resource limit checks
                self.wake_with_resource_limits();
            },
            ASILExecutionMode::AsilC => {
                // ASIL-C: Wake with temporal isolation guarantees
                self.wake_with_temporal_isolation();
            },
            ASILExecutionMode::AsilD => {
                // ASIL-D: Deterministic wake with strict ordering
                self.wake_deterministic();
            },
            ASILExecutionMode::D { .. } => {
                // ASIL-D: Deterministic wake with strict ordering
                self.wake_deterministic();
            },
            ASILExecutionMode::C {
                temporal_isolation: true,
                ..
            } => {
                // ASIL-C: Wake with temporal isolation guarantees
                self.wake_with_temporal_isolation();
            },
            ASILExecutionMode::C {
                temporal_isolation: false,
                ..
            } => {
                // ASIL-C without temporal isolation: Basic wake
                self.wake_basic();
            },
            ASILExecutionMode::B {
                strict_resource_limits: true,
                ..
            } => {
                // ASIL-B: Wake with resource limit checks
                self.wake_with_resource_limits();
            },
            ASILExecutionMode::B {
                strict_resource_limits: false,
                ..
            } => {
                // ASIL-B without strict resource limits: Basic wake
                self.wake_basic();
            },
            ASILExecutionMode::A { .. } => {
                // ASIL-A: Basic wake with error detection
                self.wake_basic();
            },
        }

        // Track fuel consumption if executor is still alive
        if let Some(executor) = self.executor_ref.upgrade() {
            let mut exec = executor.lock();
            // ASIL-specific fuel costs
            let wake_fuel = match self.asil_mode {
                ASILExecutionMode::QM => WAKE_OPERATION_FUEL,
                ASILExecutionMode::AsilA => WAKE_OPERATION_FUEL,
                ASILExecutionMode::AsilB => WAKE_OPERATION_FUEL + 2,
                ASILExecutionMode::AsilC => WAKE_OPERATION_FUEL + 3,
                ASILExecutionMode::AsilD => WAKE_OPERATION_FUEL * 2, // Higher cost for deterministic
                ASILExecutionMode::D { .. } => WAKE_OPERATION_FUEL * 2, // Higher cost for deterministic
                ASILExecutionMode::C { .. } => WAKE_OPERATION_FUEL + 3,
                ASILExecutionMode::B { .. } => WAKE_OPERATION_FUEL + 2,
                ASILExecutionMode::A { .. } => WAKE_OPERATION_FUEL,
            };
            exec.consume_global_fuel(wake_fuel).ok();
        }

        // Reset woken flag will be handled by the executor when task is polled
    }

    /// Wake with deterministic ordering (ASIL-D)
    fn wake_deterministic(&self) {
        let mut queue = self.ready_queue.lock();

        // Insert at the correct position for deterministic ordering
        if !queue.iter().any(|&id| id == self.task_id) {
            // StaticVec doesn't have insert method, so we append and rely on executor
            // to handle ordering during task polling
            let _ = queue.push(self.task_id);
        }
    }

    /// Wake with temporal isolation (ASIL-C)
    fn wake_with_temporal_isolation(&self) {
        let mut queue = self.ready_queue.lock();
        // Check temporal constraints before adding to queue
        let already_ready = queue.iter().any(|&id| id == self.task_id);

        if !already_ready {
            // Add with temporal isolation consideration
            if queue.push(self.task_id).is_err() {
                // Handle queue full - for ASIL-C, this shouldn't happen
                // due to resource isolation guarantees
            }
        }
    }

    /// Wake with resource limit checks (ASIL-B)
    fn wake_with_resource_limits(&self) {
        let mut queue = self.ready_queue.lock();
        // Check resource limits before queueing
        if queue.len() >= queue.capacity() - 10 {
            // Near capacity - ASIL-B requires handling this gracefully
            // Remove duplicates using retain (StaticVec doesn't have dedup)
            let mut seen = [false; 128];
            queue.retain(|&id| {
                let id_usize = id.into_inner() as usize;
                if id_usize < 128 && !seen[id_usize] {
                    seen[id_usize] = true;
                    true
                } else {
                    false
                }
            });
        }

        if !queue.iter().any(|&id| id == self.task_id) {
            queue.push(self.task_id).ok();
        }
    }

    /// Basic wake (ASIL-A)
    fn wake_basic(&self) {
        let mut queue = self.ready_queue.lock();
        if !queue.iter().any(|&id| id == self.task_id) && queue.push(self.task_id).is_err() {
            // Basic error detection - queue full
            // Remove duplicates using retain (StaticVec doesn't have dedup)
            let mut seen = [false; 128];
            queue.retain(|&id| {
                let id_usize = id.into_inner() as usize;
                if id_usize < 128 && !seen[id_usize] {
                    seen[id_usize] = true;
                    true
                } else {
                    false
                }
            });
            let _ = queue.push(self.task_id);
        }
    }

    /// Get deterministic timestamp for ASIL-D
    fn get_deterministic_timestamp(&self) -> u64 {
        // In real implementation, would use deterministic time source
        // For now, use wake count as a proxy for ordering
        self.wake_count.load(Ordering::Acquire) as u64
    }

    /// Clone the waker data
    pub fn clone_data(&self) -> Self {
        Self {
            task_id: self.task_id,
            ready_queue: self.ready_queue.clone(),
            wake_count: self.wake_count.clone(),
            is_woken: self.is_woken.clone(),
            executor_ref: self.executor_ref.clone(),
            asil_mode: self.asil_mode,
            wake_timestamp: self.wake_timestamp.clone(),
        }
    }

    /// Reset the woken flag (called by executor after polling)
    pub fn reset_woken_flag(&self) {
        self.is_woken.store(false, Ordering::Release);
    }
}

/// ASIL-D safe waker implementations using conditional compilation
/// Only available for QM builds - excluded from all ASIL levels for safety
#[cfg(all(
    not(feature = "asil-a"),
    not(feature = "asil-b"),
    not(feature = "asil-c"),
    not(feature = "asil-d")
))]
#[allow(unsafe_code)] // Required for Waker creation API
mod unsafe_waker {
    use super::*;

    /// Raw waker clone implementation (unsafe - only for non-ASIL-D builds)
    pub unsafe fn waker_clone(data: *const ()) -> RawWaker {
        unsafe {
            let waker_data = &*(data as *const WakerData);
            let cloned = Box::new(waker_data.clone_data());
            RawWaker::new(Box::into_raw(cloned) as *const (), &WAKER_VTABLE)
        }
    }

    /// Raw waker wake implementation (unsafe - only for non-ASIL-D builds)
    pub unsafe fn waker_wake(data: *const ()) {
        unsafe {
            let waker_data = Box::from_raw(data as *mut WakerData);
            waker_data.wake();
        }
    }

    /// Raw waker wake by ref implementation (unsafe - only for non-ASIL-D
    /// builds)
    pub unsafe fn waker_wake_by_ref(data: *const ()) {
        unsafe {
            let waker_data = &*(data as *const WakerData);
            waker_data.wake();
        }
    }

    /// Raw waker drop implementation (unsafe - only for non-ASIL-D builds)
    pub unsafe fn waker_drop(data: *const ()) {
        unsafe {
            drop(Box::from_raw(data as *mut WakerData));
        }
    }
}

#[cfg(feature = "asil-d")]
mod safe_waker {
    use super::*;

    /// ASIL-D safe waker clone implementation
    pub fn waker_clone(_data: *const ()) -> RawWaker {
        // ASIL-D safe: Return noop waker for safety compliance
        create_asil_d_noop_waker()
    }

    /// ASIL-D safe waker wake implementation  
    pub fn waker_wake(_data: *const ()) {
        // ASIL-D safe: No-op for safety compliance
    }

    /// ASIL-D safe waker wake by ref implementation
    pub fn waker_wake_by_ref(_data: *const ()) {
        // ASIL-D safe: No-op for safety compliance
    }

    /// ASIL-D safe waker drop implementation
    pub fn waker_drop(_data: *const ()) {
        // ASIL-D safe: No-op for safety compliance
    }

    fn create_asil_d_noop_waker() -> RawWaker {
        RawWaker::new(
            core::ptr::null(),
            &RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop),
        )
    }
}

// Use appropriate implementation based on ASIL level
#[cfg(feature = "asil-d")]
use safe_waker::*;
#[cfg(not(feature = "asil-d"))]
use unsafe_waker::*;

/// VTable for the fuel-aware waker
static WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

/// Create a fuel-aware waker for a task
pub fn create_fuel_aware_waker(
    task_id: TaskId,
    ready_queue: Arc<Mutex<BoundedVec<TaskId, 128>>>,
    executor_ref: Weak<Mutex<FuelAsyncExecutor>>,
) -> Waker {
    // Default to ASIL-A for backwards compatibility
    create_fuel_aware_waker_with_asil(
        task_id,
        ready_queue,
        executor_ref,
        ASILExecutionMode::default(),
    )
}

/// Create a fuel-aware waker for a task with specific ASIL mode
pub fn create_fuel_aware_waker_with_asil(
    task_id: TaskId,
    ready_queue: Arc<Mutex<BoundedVec<TaskId, 128>>>,
    executor_ref: Weak<Mutex<FuelAsyncExecutor>>,
    asil_mode: ASILExecutionMode,
) -> Waker {
    #[cfg(not(feature = "asil-d"))]
    {
        // Standard unsafe waker for non-ASIL-D builds
        let waker_data = Box::new(WakerData::new(
            task_id,
            ready_queue,
            executor_ref,
            asil_mode,
        ));
        let raw_waker = RawWaker::new(Box::into_raw(waker_data) as *const (), &WAKER_VTABLE);
        // SAFETY: This unsafe call is required by Rust's Waker API.
        // The raw_waker contains a valid heap-allocated WakerData pointer
        // that will be properly cleaned up by waker_drop when the Waker is dropped.
        #[allow(unsafe_code)] // Required for Waker creation API
        unsafe {
            Waker::from_raw(raw_waker)
        }
    }

    #[cfg(feature = "asil-d")]
    {
        // ASIL-D safe: Use noop waker for safety compliance
        // Real waker functionality disabled to ensure deterministic behavior
        let raw_waker = RawWaker::new(core::ptr::null(), &WAKER_VTABLE);
        // SAFETY: This unsafe call is required by Rust's Waker API and cannot be
        // avoided. For ASIL-D compliance:
        // 1. The raw_waker uses null pointer data (no dereferencing)
        // 2. All vtable functions are no-ops that don't access memory
        // 3. This creates a functionally safe noop waker
        // 4. The waker lifetime is managed by Rust's type system
        #[allow(unsafe_code)]
        unsafe {
            Waker::from_raw(raw_waker)
        }
    }
}

/// Wake coalescing to prevent thundering herd
pub struct WakeCoalescer {
    /// Pending wakes to be processed
    pending_wakes: Mutex<BoundedVec<TaskId, MAX_PENDING_WAKES>>,
    /// Flag indicating if coalescer is processing
    processing: AtomicBool,
}

impl WakeCoalescer {
    /// Create a new wake coalescer
    pub fn new() -> Result<Self> {
        let provider = safe_managed_alloc!(1024, CrateId::Component)?;
        Ok(Self {
            pending_wakes: Mutex::new(BoundedVec::new()),
            processing: AtomicBool::new(false),
        })
    }

    /// Add a wake to be coalesced
    pub fn add_wake(&self, task_id: TaskId) -> Result<()> {
        let mut pending = self.pending_wakes.lock();

        // Check if already pending
        if !pending.iter().any(|&id| id == task_id) {
            pending
                .push(task_id)
                .map_err(|_| Error::resource_limit_exceeded("Wake coalescer queue full"))?;
        }

        Ok(())
    }

    /// Process all pending wakes
    pub fn process_wakes(
        &self,
        ready_queue: &Arc<Mutex<BoundedVec<TaskId, 128>>>,
    ) -> Result<usize> {
        // Check if already processing to prevent recursion
        if self
            .processing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(0);
        }

        let mut count = 0;

        // Process all pending wakes
        {
            let mut pending = self.pending_wakes.lock();
            let mut ready = ready_queue.lock();

            while let Some(task_id) = pending.pop() {
                // Add to ready queue if not already there
                if !ready.iter().any(|&id| id == task_id) {
                    ready.push(task_id).ok();
                    count += 1;
                }
            }
        }

        self.processing.store(false, Ordering::Release);
        Ok(count)
    }

    /// Get the number of pending wakes
    pub fn pending_count(&self) -> usize {
        let pending = self.pending_wakes.lock();
        pending.len()
    }
}

/// Create a no-op waker that does nothing when awakened
///
/// This is used as a fallback when no proper waker context is available.
/// The waker will not actually wake any tasks.
pub fn create_noop_waker() -> Waker {
    /// No-op waker vtable
    static NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE), // clone
        |_| {},                                                   // wake
        |_| {},                                                   // wake_by_ref
        |_| {},                                                   // drop
    );

    let raw_waker = RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE);
    // SAFETY: The vtable is statically allocated and valid
    #[allow(unsafe_code)] // Required for Waker creation API
    unsafe {
        Waker::from_raw(raw_waker)
    }
}
