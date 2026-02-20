//! Resource cleanup on task cancellation
//!
//! This module provides automatic resource cleanup when async tasks are
//! cancelled, ensuring no resource leaks with fuel tracking.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Weak;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(feature = "std")]
use std::sync::Weak;

use wrt_foundation::{
    Arc, CrateId, MemoryProvider, Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::{Checksum, VerificationLevel},
};

use crate::{
    async_::{
        fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
        fuel_error_context::{AsyncErrorKind, ContextualError, async_error},
        fuel_handle_table::{GenerationalHandle, HandleTableManager},
        fuel_resource_lifetime::{
            ComponentResourceTracker, ResourceHandle, ResourceLifetimeManager,
        },
        fuel_stream_handler::FuelStreamManager,
    },
    prelude::*,
};

/// Maximum cleanup callbacks per task
const MAX_CLEANUP_CALLBACKS: usize = 64;

/// Maximum tasks tracked for cleanup
const MAX_TRACKED_TASKS: usize = 256;

/// Fuel costs for cleanup operations
const CLEANUP_REGISTER_FUEL: u64 = 2;
const CLEANUP_EXECUTE_FUEL: u64 = 5;
const CLEANUP_CANCEL_FUEL: u64 = 10;
const CLEANUP_FINALIZE_FUEL: u64 = 8;

/// Cleanup action type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupAction {
    /// Drop a resource handle
    DropResource(ResourceHandle),
    /// Close a stream
    CloseStream(u64),
    /// Release a handle from table
    ReleaseHandle(u64, GenerationalHandle),
    /// Custom cleanup callback
    Custom(String),
}

impl Checksummable for CleanupAction {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            Self::DropResource(h) => {
                0u8.update_checksum(checksum);
                h.0.update_checksum(checksum);
            },
            Self::CloseStream(s) => {
                1u8.update_checksum(checksum);
                s.update_checksum(checksum);
            },
            Self::ReleaseHandle(t, h) => {
                2u8.update_checksum(checksum);
                t.update_checksum(checksum);
                h.index.update_checksum(checksum);
            },
            Self::Custom(s) => {
                3u8.update_checksum(checksum);
                s.as_bytes().update_checksum(checksum);
            },
        }
    }
}

impl ToBytes for CleanupAction {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::DropResource(h) => {
                0u8.to_bytes_with_provider(writer, provider)?;
                h.0.to_bytes_with_provider(writer, provider)
            },
            Self::CloseStream(s) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                s.to_bytes_with_provider(writer, provider)
            },
            Self::ReleaseHandle(t, h) => {
                2u8.to_bytes_with_provider(writer, provider)?;
                t.to_bytes_with_provider(writer, provider)?;
                h.index.to_bytes_with_provider(writer, provider)
            },
            Self::Custom(s) => {
                3u8.to_bytes_with_provider(writer, provider)?;
                (s.len() as u32).to_bytes_with_provider(writer, provider)?;
                // Write each byte individually
                for byte in s.as_bytes() {
                    byte.to_bytes_with_provider(writer, provider)?;
                }
                Ok(())
            },
        }
    }
}

impl FromBytes for CleanupAction {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::CloseStream(0))
    }
}

/// Cleanup callback registration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupCallback {
    /// Action to perform
    pub action: CleanupAction,
    /// Priority (higher executes first)
    pub priority: u32,
    /// Fuel cost for this cleanup
    pub fuel_cost: u64,
    /// Whether cleanup is critical (must not fail)
    pub is_critical: bool,
}

impl Default for CleanupCallback {
    fn default() -> Self {
        Self {
            action: CleanupAction::CloseStream(0),
            priority: 0,
            fuel_cost: 0,
            is_critical: false,
        }
    }
}

impl CleanupCallback {
    /// Create a new cleanup callback
    pub fn new(action: CleanupAction, priority: u32, fuel_cost: u64, is_critical: bool) -> Self {
        Self {
            action,
            priority,
            fuel_cost,
            is_critical,
        }
    }
}

impl Checksummable for CleanupCallback {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.action.update_checksum(checksum);
        self.priority.update_checksum(checksum);
        self.fuel_cost.update_checksum(checksum);
        self.is_critical.update_checksum(checksum);
    }
}

impl ToBytes for CleanupCallback {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.action.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        self.fuel_cost.to_bytes_with_provider(writer, provider)?;
        self.is_critical.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for CleanupCallback {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            action: CleanupAction::from_bytes_with_provider(reader, provider)?,
            priority: u32::from_bytes_with_provider(reader, provider)?,
            fuel_cost: u64::from_bytes_with_provider(reader, provider)?,
            is_critical: bool::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Task cleanup context
pub struct TaskCleanupContext {
    /// Task ID
    pub task_id: u64,
    /// Component ID
    pub component_id: u64,
    /// Cleanup callbacks
    callbacks: BoundedVec<CleanupCallback, MAX_CLEANUP_CALLBACKS>,
    /// Resources owned by this task
    owned_resources: BoundedVec<ResourceHandle, MAX_CLEANUP_CALLBACKS>,
    /// Streams owned by this task
    owned_streams: BoundedVec<u64, MAX_CLEANUP_CALLBACKS>,
    /// Handle table entries
    handle_entries: BoundedVec<(u64, GenerationalHandle), MAX_CLEANUP_CALLBACKS>,
    /// Cleanup executed flag
    cleanup_executed: AtomicBool,
    /// Total fuel consumed during cleanup
    cleanup_fuel_consumed: AtomicU64,
    /// Verification level
    verification_level: VerificationLevel,
}

impl TaskCleanupContext {
    /// Create a new task cleanup context
    pub fn new(
        task_id: u64,
        component_id: u64,
        verification_level: VerificationLevel,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        Ok(Self {
            task_id,
            component_id,
            callbacks: {
                let provider = safe_managed_alloc!(2048, CrateId::Component)?;
                BoundedVec::new()
            },
            owned_resources: {
                let provider = safe_managed_alloc!(2048, CrateId::Component)?;
                BoundedVec::new()
            },
            owned_streams: {
                let provider = safe_managed_alloc!(2048, CrateId::Component)?;
                BoundedVec::new()
            },
            handle_entries: {
                let provider = safe_managed_alloc!(2048, CrateId::Component)?;
                BoundedVec::new()
            },
            cleanup_executed: AtomicBool::new(false),
            cleanup_fuel_consumed: AtomicU64::new(0),
            verification_level,
        })
    }

    /// Register a cleanup callback
    pub fn register_callback(&mut self, callback: CleanupCallback) -> Result<()> {
        self.consume_fuel(CLEANUP_REGISTER_FUEL)?;
        self.callbacks.push(callback)?;

        // Sort by priority (highest first)
        self.callbacks.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(())
    }

    /// Register resource ownership
    pub fn register_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        self.consume_fuel(CLEANUP_REGISTER_FUEL)?;
        self.owned_resources.push(handle)?;

        // Also register cleanup callback
        self.register_callback(CleanupCallback::new(
            CleanupAction::DropResource(handle),
            50, // Medium priority
            CLEANUP_EXECUTE_FUEL,
            true, // Critical
        ))?;
        Ok(())
    }

    /// Register stream ownership
    pub fn register_stream(&mut self, stream_id: u64) -> Result<()> {
        self.consume_fuel(CLEANUP_REGISTER_FUEL)?;
        self.owned_streams.push(stream_id)?;

        // Also register cleanup callback
        self.register_callback(CleanupCallback::new(
            CleanupAction::CloseStream(stream_id),
            60, // Higher priority than resources
            CLEANUP_EXECUTE_FUEL,
            false, // Non-critical
        ))?;
        Ok(())
    }

    /// Register handle table entry
    pub fn register_handle(&mut self, table_id: u64, handle: GenerationalHandle) -> Result<()> {
        self.consume_fuel(CLEANUP_REGISTER_FUEL)?;
        self.handle_entries.push((table_id, handle))?;

        // Also register cleanup callback
        self.register_callback(CleanupCallback::new(
            CleanupAction::ReleaseHandle(table_id, handle),
            40, // Lower priority
            CLEANUP_EXECUTE_FUEL,
            false, // Non-critical
        ))?;
        Ok(())
    }

    /// Execute cleanup
    pub fn execute_cleanup(
        &mut self,
        resource_tracker: &mut ComponentResourceTracker,
        stream_manager: &mut FuelStreamManager,
        handle_manager: &mut HandleTableManager,
    ) -> Result<Vec<ContextualError>> {
        // Check if already executed
        if self.cleanup_executed.swap(true, Ordering::AcqRel) {
            return Ok(Vec::new());
        }

        let mut errors = Vec::new();

        // Execute callbacks in priority order (drain not available on StaticVec, so iterate and clear)
        let callbacks_to_process = self.callbacks.clone();
        self.callbacks.clear();

        for callback in callbacks_to_process {
            // Check fuel
            if let Err(e) = self.consume_fuel(callback.fuel_cost) {
                if callback.is_critical {
                    match self.create_error(
                        AsyncErrorKind::FuelExhausted,
                        "Critical cleanup failed due to fuel exhaustion",
                    ) {
                        Ok(err) => errors.push(err),
                        Err(_) => {
                            // If we can't even create an error, skip this callback
                            continue;
                        },
                    }
                }
                continue;
            }

            // Execute action
            let result = match callback.action {
                CleanupAction::DropResource(handle) => {
                    self.cleanup_resource(resource_tracker, handle)
                },
                CleanupAction::CloseStream(stream_id) => {
                    self.cleanup_stream(stream_manager, stream_id)
                },
                CleanupAction::ReleaseHandle(table_id, handle) => {
                    self.cleanup_handle(handle_manager, table_id, handle)
                },
                CleanupAction::Custom(description) => {
                    // Custom callbacks would be executed here
                    Ok(())
                },
            };

            // Handle errors
            if let Err(e) = result {
                if callback.is_critical {
                    match self.create_error(
                        AsyncErrorKind::TaskCancelled,
                        &format!("Cleanup failed: {}", e.message()),
                    ) {
                        Ok(err) => errors.push(err),
                        Err(_) => {
                            // Continue processing other callbacks even if error creation fails
                        },
                    }
                }
            }
        }

        // Final cleanup
        self.consume_fuel(CLEANUP_FINALIZE_FUEL)?;

        Ok(errors)
    }

    /// Cleanup a resource
    fn cleanup_resource(
        &self,
        resource_tracker: &mut ComponentResourceTracker,
        handle: ResourceHandle,
    ) -> Result<()> {
        if let Ok(manager) = resource_tracker.get_or_create_manager(self.component_id) {
            let mut manager = manager.lock();
            manager.drop_resource(handle)?;
        }
        Ok(())
    }

    /// Cleanup a stream
    fn cleanup_stream(&self, stream_manager: &mut FuelStreamManager, stream_id: u64) -> Result<()> {
        stream_manager.close_stream(stream_id)
    }

    /// Cleanup a handle
    fn cleanup_handle(
        &self,
        handle_manager: &mut HandleTableManager,
        table_id: u64,
        handle: GenerationalHandle,
    ) -> Result<()> {
        // We can't deallocate without knowing the type, so we just log it
        // In a real implementation, we'd need type information
        Ok(())
    }

    /// Consume fuel for cleanup operation
    fn consume_fuel(&self, amount: u64) -> Result<()> {
        let adjusted =
            OperationType::fuel_cost_for_operation(OperationType::Other, self.verification_level)?;

        let total = amount.saturating_add(adjusted);
        self.cleanup_fuel_consumed.fetch_add(total, Ordering::AcqRel);
        record_global_operation(OperationType::Other, self.verification_level);

        Ok(())
    }

    /// Create an error with context
    fn create_error(&self, kind: AsyncErrorKind, context: &str) -> Result<ContextualError> {
        async_error(kind, self.component_id, Some(self.task_id), context)
    }
}

impl Default for TaskCleanupContext {
    fn default() -> Self {
        Self {
            task_id: 0,
            component_id: 0,
            callbacks: BoundedVec::new(),
            owned_resources: BoundedVec::new(),
            owned_streams: BoundedVec::new(),
            handle_entries: BoundedVec::new(),
            cleanup_executed: AtomicBool::new(false),
            cleanup_fuel_consumed: AtomicU64::new(0),
            verification_level: VerificationLevel::Off,
        }
    }
}

impl Clone for TaskCleanupContext {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id,
            component_id: self.component_id,
            callbacks: self.callbacks.clone(),
            owned_resources: self.owned_resources.clone(),
            owned_streams: self.owned_streams.clone(),
            handle_entries: self.handle_entries.clone(),
            cleanup_executed: AtomicBool::new(self.cleanup_executed.load(Ordering::Relaxed)),
            cleanup_fuel_consumed: AtomicU64::new(
                self.cleanup_fuel_consumed.load(Ordering::Relaxed),
            ),
            verification_level: self.verification_level,
        }
    }
}

impl PartialEq for TaskCleanupContext {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
            && self.component_id == other.component_id
            && self.verification_level == other.verification_level
    }
}

impl Eq for TaskCleanupContext {}

impl Checksummable for TaskCleanupContext {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.task_id.update_checksum(checksum);
        self.component_id.update_checksum(checksum);
        // Note: Atomic fields are not checksummed as they represent runtime state
        self.cleanup_executed.load(Ordering::Relaxed).update_checksum(checksum);
        self.cleanup_fuel_consumed.load(Ordering::Relaxed).update_checksum(checksum);
        // Note: callbacks, resources, streams, handles are not checksummed as they contain
        // complex types that don't all implement Checksummable
    }
}

impl ToBytes for TaskCleanupContext {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.task_id.to_bytes_with_provider(writer, provider)?;
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.cleanup_executed
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.cleanup_fuel_consumed
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        // Note: Complex fields are not serialized
        Ok(())
    }
}

impl FromBytes for TaskCleanupContext {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let task_id = u64::from_bytes_with_provider(reader, provider)?;
        let component_id = u64::from_bytes_with_provider(reader, provider)?;
        let cleanup_executed_val = bool::from_bytes_with_provider(reader, provider)?;
        let cleanup_fuel_consumed_val = u64::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            task_id,
            component_id,
            callbacks: BoundedVec::new(),
            owned_resources: BoundedVec::new(),
            owned_streams: BoundedVec::new(),
            handle_entries: BoundedVec::new(),
            cleanup_executed: AtomicBool::new(cleanup_executed_val),
            cleanup_fuel_consumed: AtomicU64::new(cleanup_fuel_consumed_val),
            verification_level: VerificationLevel::Off,
        })
    }
}

/// Global cleanup manager
pub struct GlobalCleanupManager {
    /// Cleanup contexts by task ID
    contexts: BoundedMap<u64, TaskCleanupContext, MAX_TRACKED_TASKS>,
    /// Component resource tracker
    resource_tracker: Arc<Mutex<ComponentResourceTracker>>,
    /// Stream manager
    stream_manager: Arc<Mutex<FuelStreamManager>>,
    /// Handle table manager
    handle_manager: Arc<Mutex<HandleTableManager>>,
    /// Total cleanup operations
    total_cleanups: AtomicU64,
    /// Failed cleanups
    failed_cleanups: AtomicU64,
}

impl GlobalCleanupManager {
    /// Create a new global cleanup manager
    pub fn new(global_fuel_budget: u64) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        let contexts = BoundedMap::new();

        let resource_tracker = Arc::new(Mutex::new(ComponentResourceTracker::new(
            global_fuel_budget / 3,
        )?));
        let stream_manager = Arc::new(Mutex::new(FuelStreamManager::new(global_fuel_budget / 3)?));
        let handle_manager = Arc::new(Mutex::new(HandleTableManager::new(global_fuel_budget / 3)?));

        Ok(Self {
            contexts,
            resource_tracker,
            stream_manager,
            handle_manager,
            total_cleanups: AtomicU64::new(0),
            failed_cleanups: AtomicU64::new(0),
        })
    }

    /// Register a task for cleanup tracking
    pub fn register_task(
        &mut self,
        task_id: u64,
        component_id: u64,
        verification_level: VerificationLevel,
    ) -> Result<()> {
        let context = TaskCleanupContext::new(task_id, component_id, verification_level)?;
        self.contexts.insert(task_id, context)?;
        Ok(())
    }

    /// Get cleanup context for a task
    pub fn get_context_mut(&mut self, task_id: u64) -> Result<&mut TaskCleanupContext> {
        self.contexts
            .get_mut(&task_id)
            .ok_or_else(|| Error::async_error("Task cleanup context not found"))
    }

    /// Cancel a task and run cleanup
    pub fn cancel_task(&mut self, task_id: u64) -> Result<Vec<ContextualError>> {
        // Remove context
        let mut context = self
            .contexts
            .remove(&task_id)
            .ok_or_else(|| Error::async_error("Task not found for cancellation"))?;

        // Consume cancellation fuel
        context.consume_fuel(CLEANUP_CANCEL_FUEL)?;

        // Execute cleanup
        let errors = context.execute_cleanup(
            &mut self.resource_tracker.lock(),
            &mut self.stream_manager.lock(),
            &mut self.handle_manager.lock(),
        )?;

        // Update stats
        self.total_cleanups.fetch_add(1, Ordering::Relaxed);
        if !errors.is_empty() {
            self.failed_cleanups.fetch_add(1, Ordering::Relaxed);
        }

        Ok(errors)
    }

    /// Cancel all tasks for a component
    pub fn cancel_component_tasks(&mut self, component_id: u64) -> Result<Vec<ContextualError>> {
        let task_ids: Vec<u64> = self
            .contexts
            .iter()
            .filter(|(_, ctx)| ctx.component_id == component_id)
            .map(|(id, _)| *id)
            .collect();

        let mut all_errors = Vec::new();

        for task_id in task_ids {
            match self.cancel_task(task_id) {
                Ok(errors) => all_errors.extend(errors),
                Err(e) => {
                    let error = async_error(
                        AsyncErrorKind::TaskCancelled,
                        component_id,
                        Some(task_id),
                        &format!("Failed to cancel task: {}", e.message()),
                    )?;
                    all_errors.push(error);
                },
            }
        }

        Ok(all_errors)
    }

    /// Get cleanup statistics
    pub fn stats(&self) -> CleanupStats {
        CleanupStats {
            total_cleanups: self.total_cleanups.load(Ordering::Relaxed),
            failed_cleanups: self.failed_cleanups.load(Ordering::Relaxed),
            active_tasks: self.contexts.len(),
        }
    }
}

/// Cleanup statistics
#[derive(Debug, Clone)]
pub struct CleanupStats {
    /// Total cleanup operations performed
    pub total_cleanups: u64,
    /// Failed cleanup operations
    pub failed_cleanups: u64,
    /// Currently active tasks
    pub active_tasks: usize,
}

/// RAII guard for automatic task cleanup
pub struct TaskCleanupGuard {
    task_id: u64,
    manager: Arc<Mutex<GlobalCleanupManager>>,
    cancelled: AtomicBool,
}

impl TaskCleanupGuard {
    /// Create a new cleanup guard
    pub fn new(task_id: u64, manager: Arc<Mutex<GlobalCleanupManager>>) -> Self {
        Self {
            task_id,
            manager,
            cancelled: AtomicBool::new(false),
        }
    }

    /// Cancel the task early
    pub fn cancel(&self) -> Result<Vec<ContextualError>> {
        if self.cancelled.swap(true, Ordering::AcqRel) {
            return Ok(Vec::new());
        }

        self.manager.lock().cancel_task(self.task_id)
    }
}

impl Drop for TaskCleanupGuard {
    fn drop(&mut self) {
        if !self.cancelled.load(Ordering::Acquire) {
            let _ = self.cancel();
        }
    }
}
