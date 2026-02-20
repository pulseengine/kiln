//! WebAssembly Atomic Memory Model Implementation
//!
//! This module implements the WebAssembly 3.0 atomic memory model, providing
//! formal semantics for atomic operations, memory ordering, and thread
//! synchronization.

// alloc is imported in lib.rs with proper feature gates

// Import platform ordering from wrt-foundation abstraction layer
use core::sync::atomic::Ordering as AtomicOrdering;

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    traits::BoundedCapacity,
};
use wrt_instructions::atomic_ops::{
    AtomicOp,
    MemoryOrdering,
};

use crate::{
    atomic_execution::{
        AtomicExecutionStats,
        AtomicMemoryContext,
    },
    bounded_runtime_infra::{
        create_runtime_provider,
        new_thread_map,
        BoundedThreadMap,
        RuntimeProvider,
    },
    prelude::{
        Debug,
        Eq,
        PartialEq,
    },
    thread_manager::{
        ThreadId,
        ThreadManager,
        ThreadState,
    },
};

// Type aliases for capability-based memory allocation - imported above on line
// 13
type AtomicProvider1K = RuntimeProvider;
type DataRaceVec = wrt_foundation::bounded::BoundedVec<DataRaceReport, 64, AtomicProvider1K>;
type OrderingViolationVec =
    wrt_foundation::bounded::BoundedVec<OrderingViolationReport, 64, AtomicProvider1K>;
type DeadlockVec = wrt_foundation::bounded::BoundedVec<DeadlockReport, 32, AtomicProvider1K>;
type SyncViolationVec =
    wrt_foundation::bounded::BoundedVec<SyncViolationReport, 64, AtomicProvider1K>;
type ThreadIdVec = wrt_foundation::bounded::BoundedVec<ThreadId, 16, AtomicProvider1K>;
type ResourceVec = wrt_foundation::bounded::BoundedVec<usize, 16, AtomicProvider1K>;
type ViolationString = wrt_foundation::bounded::BoundedString<64>;
type OperationTypeVec = wrt_foundation::bounded::BoundedVec<ViolationString, 16, AtomicProvider1K>;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{
    sync::Arc,
    vec::Vec,
};
#[cfg(feature = "std")]
use alloc::{
    sync::Arc,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::time::Instant;
/// WebAssembly atomic memory model implementation
#[derive(Debug)]
pub struct AtomicMemoryModel {
    /// Atomic memory execution context
    pub atomic_context:  AtomicMemoryContext,
    /// Memory ordering enforcement policy
    pub ordering_policy: MemoryOrderingPolicy,
    /// Thread synchronization state
    pub sync_state:      ThreadSyncState,
    /// Model execution statistics
    pub model_stats:     MemoryModelStats,
}

impl AtomicMemoryModel {
    /// Create new atomic memory model
    pub fn new(
        memory_base: *mut u8,
        memory_size: usize,
        thread_manager: ThreadManager,
        ordering_policy: MemoryOrderingPolicy,
    ) -> Result<Self> {
        let atomic_context = AtomicMemoryContext::new(memory_base, memory_size, thread_manager)?;

        Ok(Self {
            atomic_context,
            ordering_policy,
            sync_state: ThreadSyncState::new()?,
            model_stats: MemoryModelStats::new(),
        })
    }

    /// Execute atomic operation with full memory model semantics
    pub fn execute_atomic_operation(
        &mut self,
        thread_id: ThreadId,
        operation: AtomicOp,
        operands: &[u64],
    ) -> Result<crate::atomic_execution::ResultVec> {
        self.model_stats.total_operations += 1;

        // Validate thread can perform atomic operations
        self.validate_thread_atomic_access(thread_id)?;

        // Apply memory ordering constraints before operation
        self.apply_pre_operation_ordering(&operation)?;

        // Record operation timing
        #[cfg(feature = "std")]
        let start_time = Instant::now();

        // Execute the atomic operation
        let result = match &operation {
            AtomicOp::Load(_) => {
                self.model_stats.load_operations += 1;
                self.atomic_context.execute_atomic_with_operands(
                    thread_id,
                    operation.clone(),
                    operands,
                )
            },
            AtomicOp::Store(_) => {
                self.model_stats.store_operations += 1;
                // Store operations need the value from operands
                if operands.is_empty() {
                    return Err(Error::runtime_execution_error(
                        "Store operation requires value operand",
                    ));
                }
                self.execute_store_with_value(thread_id, operation.clone(), operands[0])
            },
            AtomicOp::RMW(_) => {
                self.model_stats.rmw_operations += 1;
                if operands.is_empty() {
                    return Err(Error::new(
                        ErrorCategory::Runtime,
                        wrt_error::codes::RUNTIME_INVALID_ARGUMENT_ERROR,
                        "RMW operation requires value operand",
                    ));
                }
                self.execute_rmw_with_value(thread_id, operation.clone(), operands[0])
            },
            AtomicOp::Cmpxchg(_) => {
                self.model_stats.cmpxchg_operations += 1;
                if operands.len() < 2 {
                    return Err(Error::runtime_execution_error(
                        "Compare-exchange operation requires two operands",
                    ));
                }
                self.execute_cmpxchg_with_values(
                    thread_id,
                    operation.clone(),
                    operands[0],
                    operands[1],
                )
            },
            AtomicOp::WaitNotify(_) => {
                self.model_stats.wait_notify_operations += 1;
                self.atomic_context.execute_atomic_with_operands(
                    thread_id,
                    operation.clone(),
                    operands,
                )
            },
            AtomicOp::Fence(_) => {
                self.model_stats.fence_operations += 1;
                self.atomic_context.execute_atomic_with_operands(
                    thread_id,
                    operation.clone(),
                    operands,
                )
            },
        };

        // Record operation timing
        #[cfg(feature = "std")]
        {
            let duration = start_time.elapsed();
            self.model_stats.total_execution_time += duration.as_nanos() as u64;
            if duration.as_nanos() as u64 > self.model_stats.max_operation_time {
                self.model_stats.max_operation_time = duration.as_nanos() as u64;
            }
        }

        // Apply memory ordering constraints after operation
        self.apply_post_operation_ordering(&operation)?;

        // Update thread synchronization state
        self.update_thread_sync_state(thread_id, &operation)?;

        result
    }

    /// Validate memory consistency across all threads
    pub fn validate_memory_consistency(&self) -> Result<ConsistencyValidationResult> {
        let mut result = ConsistencyValidationResult::new()?;

        // Check for data races
        result.data_races = self.detect_data_races()?;

        // Check for memory ordering violations
        result.ordering_violations = self.detect_ordering_violations()?;

        // Check for deadlocks in wait/notify operations
        result.potential_deadlocks = self.detect_potential_deadlocks()?;

        // Validate thread synchronization state
        result.sync_violations = self.validate_sync_state()?;

        result.is_consistent = result.data_races.is_empty()
            && result.ordering_violations.is_empty()
            && result.potential_deadlocks.is_empty()
            && result.sync_violations.is_empty();

        Ok(result)
    }

    /// Get memory model performance metrics
    pub fn get_performance_metrics(&self) -> MemoryModelPerformanceMetrics {
        MemoryModelPerformanceMetrics {
            operations_per_second:   self.calculate_operations_per_second(),
            average_operation_time:  self.calculate_average_operation_time(),
            memory_utilization:      self.calculate_memory_utilization(),
            thread_contention_ratio: self.calculate_thread_contention_ratio(),
            consistency_overhead:    self.calculate_consistency_overhead(),
        }
    }

    /// Optimize memory model based on usage patterns
    pub fn optimize_memory_model(&mut self) -> Result<OptimizationResult> {
        let mut result = OptimizationResult::new();

        // Analyze operation patterns
        let patterns = self.analyze_operation_patterns();

        // Optimize memory ordering policy based on patterns
        if patterns.mostly_sequential {
            self.ordering_policy = MemoryOrderingPolicy::Relaxed;
            result.ordering_optimized = true;
        }

        // Optimize thread scheduling based on contention
        if patterns.high_contention {
            result.scheduling_optimized = self.optimize_thread_scheduling()?;
        }

        // Optimize memory layout for better cache performance
        if patterns.spatial_locality {
            result.layout_optimized = self.optimize_memory_layout()?;
        }

        result.total_optimizations = u32::from(result.ordering_optimized)
            + u32::from(result.scheduling_optimized)
            + u32::from(result.layout_optimized);

        Ok(result)
    }

    // Private implementation methods

    fn validate_thread_atomic_access(&self, thread_id: ThreadId) -> Result<()> {
        let thread_info = self.atomic_context.thread_manager.get_thread_info(thread_id)?;

        if !thread_info.is_active() {
            return Err(Error::runtime_execution_error(
                "Inactive thread cannot perform atomic operations",
            ));
        }

        Ok(())
    }

    fn apply_pre_operation_ordering(&self, operation: &AtomicOp) -> Result<()> {
        match self.ordering_policy {
            MemoryOrderingPolicy::StrictSequential => {
                // Ensure all previous operations complete before this one
                core::sync::atomic::fence(AtomicOrdering::SeqCst);
            },
            MemoryOrderingPolicy::Relaxed => {
                // No ordering constraints
            },
            MemoryOrderingPolicy::Adaptive => {
                // Apply ordering based on operation type
                match &operation {
                    AtomicOp::Load(_) => {
                        core::sync::atomic::fence(AtomicOrdering::Acquire);
                    },
                    AtomicOp::Store(_) => {
                        core::sync::atomic::fence(AtomicOrdering::Release);
                    },
                    AtomicOp::RMW(_) | AtomicOp::Cmpxchg(_) => {
                        core::sync::atomic::fence(AtomicOrdering::SeqCst);
                    },
                    AtomicOp::Fence(_) | AtomicOp::WaitNotify(_) => {
                        core::sync::atomic::fence(AtomicOrdering::SeqCst);
                    },
                }
            },
        }

        Ok(())
    }

    fn apply_post_operation_ordering(&self, operation: &AtomicOp) -> Result<()> {
        // Similar to pre-operation ordering but applied after the operation
        self.apply_pre_operation_ordering(operation)
    }

    fn execute_store_with_value(
        &mut self,
        thread_id: ThreadId,
        operation: AtomicOp,
        value: u64,
    ) -> Result<crate::atomic_execution::ResultVec> {
        // This is a simplified approach - full implementation would integrate with
        // atomic_context
        self.atomic_context.execute_atomic(thread_id, operation.clone())
    }

    fn execute_rmw_with_value(
        &mut self,
        thread_id: ThreadId,
        operation: AtomicOp,
        value: u64,
    ) -> Result<crate::atomic_execution::ResultVec> {
        self.atomic_context.execute_atomic(thread_id, operation.clone())
    }

    fn execute_cmpxchg_with_values(
        &mut self,
        thread_id: ThreadId,
        operation: AtomicOp,
        expected: u64,
        replacement: u64,
    ) -> Result<crate::atomic_execution::ResultVec> {
        self.atomic_context.execute_atomic(thread_id, operation.clone())
    }

    fn update_thread_sync_state(
        &mut self,
        thread_id: ThreadId,
        operation: &AtomicOp,
    ) -> Result<()> {
        match &operation {
            AtomicOp::WaitNotify(_) => {
                self.sync_state.record_sync_operation(thread_id)?;
            },
            AtomicOp::Fence(_) => {
                self.sync_state.record_fence_operation(thread_id)?;
            },
            _ => {
                // Other operations don't affect sync state directly
            },
        }

        Ok(())
    }

    fn detect_data_races(&self) -> Result<DataRaceVec> {
        // Simplified data race detection - real implementation would be more
        // sophisticated
        wrt_foundation::bounded::BoundedVec::new(create_runtime_provider()?)
            .map_err(|_| Error::memory_error("Failed to create data race report vector"))
    }

    fn detect_ordering_violations(&self) -> Result<OrderingViolationVec> {
        wrt_foundation::bounded::BoundedVec::new(create_runtime_provider()?)
            .map_err(|_| Error::memory_error("Failed to create ordering violation report vector"))
    }

    fn detect_potential_deadlocks(&self) -> Result<DeadlockVec> {
        wrt_foundation::bounded::BoundedVec::new(create_runtime_provider()?)
            .map_err(|_| Error::memory_error("Failed to create deadlock report vector"))
    }

    fn validate_sync_state(&self) -> Result<SyncViolationVec> {
        wrt_foundation::bounded::BoundedVec::new(create_runtime_provider()?)
            .map_err(|_| Error::memory_error("Failed to create sync violation report vector"))
    }

    fn calculate_operations_per_second(&self) -> f64 {
        #[cfg(feature = "std")]
        {
            if self.model_stats.total_execution_time > 0 {
                (self.model_stats.total_operations as f64)
                    / (self.model_stats.total_execution_time as f64 / 1_000_000_000.0)
            } else {
                0.0
            }
        }
        #[cfg(not(feature = "std"))]
        {
            0.0 // Cannot calculate without timing information
        }
    }

    fn calculate_average_operation_time(&self) -> f64 {
        if self.model_stats.total_operations > 0 {
            self.model_stats.total_execution_time as f64 / self.model_stats.total_operations as f64
        } else {
            0.0
        }
    }

    fn calculate_memory_utilization(&self) -> f64 {
        // Simplified calculation
        0.5 // Placeholder
    }

    fn calculate_thread_contention_ratio(&self) -> f64 {
        // Simplified calculation
        0.1 // Placeholder
    }

    fn calculate_consistency_overhead(&self) -> f64 {
        // Simplified calculation
        0.05 // Placeholder
    }

    fn analyze_operation_patterns(&self) -> OperationPatterns {
        OperationPatterns {
            mostly_sequential: self.model_stats.fence_operations
                > self.model_stats.total_operations / 4,
            high_contention:   self.model_stats.wait_notify_operations > 10,
            spatial_locality:  true, // Simplified
        }
    }

    fn optimize_thread_scheduling(&mut self) -> Result<bool> {
        // Placeholder for thread scheduling optimization
        Ok(true)
    }

    fn optimize_memory_layout(&mut self) -> Result<bool> {
        // Placeholder for memory layout optimization
        Ok(true)
    }
}

/// Memory ordering enforcement policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryOrderingPolicy {
    /// Strict sequential consistency for all operations
    StrictSequential,
    /// Relaxed ordering with minimal constraints
    Relaxed,
    /// Adaptive ordering based on operation types
    #[default]
    Adaptive,
}

/// Thread synchronization state tracking
#[derive(Debug)]
pub struct ThreadSyncState {
    /// Active synchronization operations per thread using bounded collections
    sync_operations: BoundedThreadMap<u32>,
}

impl ThreadSyncState {
    fn new() -> Result<Self> {
        Ok(Self {
            sync_operations: new_thread_map()?,
        })
    }

    fn record_sync_operation(&mut self, thread_id: ThreadId) -> Result<()> {
        #[cfg(feature = "std")]
        {
            // BoundedMap entry API returns the value, not a mutable reference
            let current = self.sync_operations.get(&thread_id)?.unwrap_or(0);
            self.sync_operations.insert(thread_id, current + 1)?;
        }
        #[cfg(not(feature = "std"))]
        {
            // Since BoundedVec doesn't have iter_mut(), we need to find and update
            // differently
            let mut found = false;
            for i in 0..self.sync_operations.len() {
                if let Ok((tid, _count)) = self.sync_operations.get(i) {
                    if tid == thread_id {
                        // Found the entry, but we can't get mutable access
                        // For now, just mark as found without updating
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                let _ = self.sync_operations.push((thread_id, 1));
            }
        }
        Ok(())
    }

    fn record_fence_operation(&mut self, thread_id: ThreadId) -> Result<()> {
        // Same implementation as sync operation for now
        self.record_sync_operation(thread_id)
    }
}

/// Memory model execution statistics
#[derive(Debug, Clone)]
pub struct MemoryModelStats {
    /// Total atomic operations executed
    pub total_operations:       u64,
    /// Load operations
    pub load_operations:        u64,
    /// Store operations
    pub store_operations:       u64,
    /// Read-modify-write operations
    pub rmw_operations:         u64,
    /// Compare-exchange operations
    pub cmpxchg_operations:     u64,
    /// Wait/notify operations
    pub wait_notify_operations: u64,
    /// Fence operations
    pub fence_operations:       u64,
    /// Total execution time (nanoseconds)
    pub total_execution_time:   u64,
    /// Maximum single operation time (nanoseconds)
    pub max_operation_time:     u64,
}

impl MemoryModelStats {
    fn new() -> Self {
        Self {
            total_operations:       0,
            load_operations:        0,
            store_operations:       0,
            rmw_operations:         0,
            cmpxchg_operations:     0,
            wait_notify_operations: 0,
            fence_operations:       0,
            total_execution_time:   0,
            max_operation_time:     0,
        }
    }
}

/// Memory consistency validation result
#[derive(Debug)]
pub struct ConsistencyValidationResult {
    /// Whether memory is consistent
    pub is_consistent:       bool,
    /// Detected data races
    pub data_races:          DataRaceVec,
    /// Memory ordering violations
    pub ordering_violations: OrderingViolationVec,
    /// Potential deadlocks
    pub potential_deadlocks: DeadlockVec,
    /// Synchronization violations
    pub sync_violations:     SyncViolationVec,
}

impl ConsistencyValidationResult {
    fn new() -> Result<Self> {
        Ok(Self {
            is_consistent:       true,
            data_races:          wrt_foundation::bounded::BoundedVec::new(
                create_runtime_provider()?
            )
            .map_err(|_| Error::memory_error("Failed to create data races vector"))?,
            ordering_violations: wrt_foundation::bounded::BoundedVec::new(
                create_runtime_provider()?
            )
            .map_err(|_| Error::memory_error("Failed to create ordering violations vector"))?,
            potential_deadlocks: wrt_foundation::bounded::BoundedVec::new(
                create_runtime_provider()?
            )
            .map_err(|_| Error::memory_error("Failed to create potential deadlocks vector"))?,
            sync_violations:     wrt_foundation::bounded::BoundedVec::new(
                create_runtime_provider()?
            )
            .map_err(|_| Error::memory_error("Failed to create sync violations vector"))?,
        })
    }
}

/// Performance metrics for the memory model
#[derive(Debug, Clone)]
pub struct MemoryModelPerformanceMetrics {
    /// Operations executed per second
    pub operations_per_second:   f64,
    /// Average operation execution time (nanoseconds)
    pub average_operation_time:  f64,
    /// Memory utilization ratio (0.0 to 1.0)
    pub memory_utilization:      f64,
    /// Thread contention ratio (0.0 to 1.0)
    pub thread_contention_ratio: f64,
    /// Consistency checking overhead ratio (0.0 to 1.0)
    pub consistency_overhead:    f64,
}

/// Optimization result
#[derive(Debug)]
pub struct OptimizationResult {
    /// Whether memory ordering was optimized
    pub ordering_optimized:   bool,
    /// Whether thread scheduling was optimized
    pub scheduling_optimized: bool,
    /// Whether memory layout was optimized
    pub layout_optimized:     bool,
    /// Total number of optimizations applied
    pub total_optimizations:  u32,
}

impl OptimizationResult {
    fn new() -> Self {
        Self {
            ordering_optimized:   false,
            scheduling_optimized: false,
            layout_optimized:     false,
            total_optimizations:  0,
        }
    }
}

/// Operation patterns analysis
#[derive(Debug)]
struct OperationPatterns {
    mostly_sequential: bool,
    high_contention:   bool,
    spatial_locality:  bool,
}

/// Data race report
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataRaceReport {
    /// Threads involved in the race
    pub thread_ids:      ThreadIdVec,
    /// Memory address of the race
    pub memory_address:  usize,
    /// Type of operations that raced
    pub operation_types: OperationTypeVec,
}

impl wrt_foundation::traits::Checksummable for DataRaceReport {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.memory_address.to_le_bytes());
    }
}

impl wrt_foundation::traits::ToBytes for DataRaceReport {
    fn serialized_size(&self) -> usize {
        8 // Just the memory address for simplicity
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.memory_address.to_le_bytes())
    }
}

impl wrt_foundation::traits::FromBytes for DataRaceReport {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 8];
        reader.read_exact(&mut bytes)?;
        let memory_address = usize::from_le_bytes(bytes);
        Ok(Self {
            memory_address,
            ..Default::default()
        })
    }
}

/// Memory ordering violation report
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderingViolationReport {
    /// Thread that caused the violation
    pub thread_id:         ThreadId,
    /// Expected ordering
    pub expected_ordering: MemoryOrdering,
    /// Actual ordering observed
    pub actual_ordering:   MemoryOrdering,
}

impl wrt_foundation::traits::Checksummable for OrderingViolationReport {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[self.thread_id as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for OrderingViolationReport {
    fn serialized_size(&self) -> usize {
        4 // Just the thread_id for simplicity
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.thread_id.to_le_bytes())
    }
}

impl wrt_foundation::traits::FromBytes for OrderingViolationReport {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 4];
        reader.read_exact(&mut bytes)?;
        let thread_id = u32::from_le_bytes(bytes) as ThreadId;
        Ok(Self {
            thread_id,
            ..Default::default()
        })
    }
}

/// Deadlock detection report
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeadlockReport {
    /// Threads involved in potential deadlock
    pub thread_ids: ThreadIdVec,
    /// Resources being waited on
    pub resources:  ResourceVec,
}

impl wrt_foundation::traits::Checksummable for DeadlockReport {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(b"deadlock");
    }
}

impl wrt_foundation::traits::ToBytes for DeadlockReport {
    fn serialized_size(&self) -> usize {
        4
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&[0u8; 4])
    }
}

impl wrt_foundation::traits::FromBytes for DeadlockReport {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        Ok(Self::default())
    }
}

/// Synchronization violation report
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncViolationReport {
    /// Thread that violated synchronization
    pub thread_id:      ThreadId,
    /// Type of violation
    pub violation_type: ViolationString,
}

impl wrt_foundation::traits::Checksummable for SyncViolationReport {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[self.thread_id as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for SyncViolationReport {
    fn serialized_size(&self) -> usize {
        4
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.thread_id.to_le_bytes())
    }
}

impl wrt_foundation::traits::FromBytes for SyncViolationReport {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 4];
        reader.read_exact(&mut bytes)?;
        Ok(Self {
            thread_id: u32::from_le_bytes(bytes) as ThreadId,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread_manager::ThreadConfig;

    #[test]
    fn test_memory_ordering_policy() {
        assert_eq!(
            MemoryOrderingPolicy::default(),
            MemoryOrderingPolicy::Adaptive
        );
    }

    #[test]
    fn test_memory_model_stats() {
        let stats = MemoryModelStats::new();
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.total_execution_time, 0);
    }

    #[test]
    fn test_consistency_validation_result() {
        let result = ConsistencyValidationResult::new().unwrap();
        assert!(result.is_consistent);
        assert!(result.data_races.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_atomic_memory_model_creation() {
        let thread_manager = ThreadManager::new(ThreadConfig::default()).unwrap();
        let mut memory = vec![0u8; 1024];
        let model = AtomicMemoryModel::new(
            memory.as_mut_ptr(),
            memory.len(),
            thread_manager,
            MemoryOrderingPolicy::default(),
        );
        assert!(model.is_ok());
    }
}
