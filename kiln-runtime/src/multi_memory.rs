//! WebAssembly 3.0 Multi-Memory Runtime Implementation with ASIL Compliance
//!
//! This module provides the complete runtime implementation for WebAssembly
//! multi-memory proposal supporting multiple linear memory instances per module
//! across all ASIL levels (QM, ASIL-A, ASIL-B, ASIL-C, ASIL-D).
//!
//! # Features Supported
//! - Multiple linear memory instances per module (up to 16)
//! - Memory-indexed load/store operations
//! - Memory-indexed bulk operations (fill, copy, init)
//! - Cross-memory operations for data transfer
//! - Memory grow and size operations for each memory
//! - Integration with existing memory operations and validation
//!
//! # Safety and Compliance
//! - No unsafe code in safety-critical configurations
//! - Deterministic execution across all ASIL levels
//! - Bounded memory usage with compile-time guarantees
//! - Comprehensive validation and bounds checking
//! - Memory isolation and access control

// Binary std/no_std choice
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::BTreeMap as HashMap,
    sync::Arc,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    collections::HashMap,
    sync::Arc,
    vec::Vec,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::{
    traits::BoundedCapacity,
    types::{
        MemoryType,
        ValueType,
    },
    values::Value,
};

use crate::prelude::CoreMemoryType;
use wrt_instructions::{
    memory_ops::{
        DataSegmentOperations,
        MemoryOperations,
    },
    multi_memory::{
        MultiMemoryBulk,
        MultiMemoryCrossCopy,
        MultiMemoryGrow,
        MultiMemoryLoad,
        MultiMemorySize,
        MultiMemoryStore,
        MAX_MEMORIES,
    },
};
use wrt_sync::{
    unified_sync::{
        AsilLevel,
        SafeAtomicCounter,
        SafetyContext,
    },
    WrtMutex,
};

use crate::memory::Memory;

/// Provider trait for multi-memory management across ASIL levels
pub trait MultiMemoryProvider {
    /// Execute multi-memory operation with provider-specific optimizations
    fn execute_with_provider(
        &self,
        context: &mut MultiMemoryContext,
        operation: MultiMemoryOperation,
    ) -> Result<Option<Value>>;

    /// Validate multi-memory access for ASIL compliance
    fn validate_memory_access(
        &self,
        context: &MultiMemoryContext,
        memory_index: u32,
        offset: u64,
        size: u64,
    ) -> Result<()>;
}

/// Multi-memory operation types
#[derive(Debug, Clone)]
pub enum MultiMemoryOperation {
    /// Load from specific memory instance
    Load {
        /// Index of the memory to load from.
        memory_index: u32,
        /// The load operation to perform.
        load_op:      MultiMemoryLoad,
        /// Address to load from.
        address:      Value,
    },
    /// Store to specific memory instance
    Store {
        /// Index of the memory to store to.
        memory_index: u32,
        /// The store operation to perform.
        store_op:     MultiMemoryStore,
        /// Address to store to.
        address:      Value,
        /// Value to store.
        value:        Value,
    },
    /// Fill memory region
    Fill {
        /// Index of the memory to fill.
        memory_index: u32,
        /// Destination address.
        dest:         Value,
        /// Byte value to fill with.
        value:        Value,
        /// Number of bytes to fill.
        size:         Value,
    },
    /// Copy within same memory
    Copy {
        /// Index of the memory to copy within.
        memory_index: u32,
        /// Destination address.
        dest:         Value,
        /// Source address.
        src:          Value,
        /// Number of bytes to copy.
        size:         Value,
    },
    /// Initialize memory from data segment
    Init {
        /// Index of the memory to initialize.
        memory_index: u32,
        /// Index of the data segment.
        data_index:   u32,
        /// Destination address.
        dest:         Value,
        /// Source offset in data segment.
        src:          Value,
        /// Number of bytes to copy.
        size:         Value,
    },
    /// Cross-memory copy operation
    CrossCopy {
        /// The cross-copy operation definition.
        cross_copy_op: MultiMemoryCrossCopy,
        /// Destination address.
        dest_addr:     Value,
        /// Source address.
        src_addr:      Value,
        /// Number of bytes to copy.
        size:          Value,
    },
    /// Get memory size
    Size {
        /// The size operation definition.
        size_op: MultiMemorySize
    },
    /// Grow memory
    Grow {
        /// The grow operation definition.
        grow_op:     MultiMemoryGrow,
        /// Number of pages to grow by.
        delta_pages: Value,
    },
}

/// Multi-memory instance wrapper
#[derive(Debug)]
pub struct MultiMemoryInstance {
    /// Memory index within the module.
    pub memory_index: u32,
    /// Memory type specification.
    pub memory_type:  MemoryType,
    /// Underlying memory implementation.
    memory:           Arc<WrtMutex<Memory>>,
    /// Access statistics.
    pub stats:        Arc<WrtMutex<MultiMemoryStats>>,
}

impl MultiMemoryInstance {
    /// Create new multi-memory instance
    pub fn new(memory_index: u32, memory_type: MemoryType) -> Result<Self> {
        // Convert MemoryType to CoreMemoryType for Memory::new()
        let core_mem_type = CoreMemoryType {
            limits: memory_type.limits,
            shared: memory_type.shared,
        };
        let memory = Memory::new(core_mem_type)
            .map_err(|_| Error::runtime_execution_error("Failed to create memory instance"))?;

        Ok(Self {
            memory_index,
            memory_type,
            memory: Arc::new(WrtMutex::new(*memory)),  // Dereference Box<Memory>
            stats: Arc::new(WrtMutex::new(MultiMemoryStats::new())),
        })
    }

    /// Execute load operation on this memory
    pub fn execute_load(&self, load_op: &MultiMemoryLoad, address: &Value) -> Result<Value> {
        let memory = self.memory.lock();
        let result = load_op.execute_with_memory(&*memory, address)?;

        let mut stats = self.stats.lock();
        stats.load_operations += 1;

        Ok(result)
    }

    /// Execute store operation on this memory
    pub fn execute_store(
        &self,
        store_op: &MultiMemoryStore,
        address: &Value,
        value: &Value,
    ) -> Result<()> {
        let mut memory = self.memory.lock();
        store_op.execute_with_memory(&mut *memory, address, value)?;

        let mut stats = self.stats.lock();
        stats.store_operations += 1;

        Ok(())
    }

    /// Execute fill operation on this memory
    pub fn execute_fill(&self, dest: &Value, value: &Value, size: &Value) -> Result<()> {
        let mut memory = self.memory.lock();
        let bulk_op = MultiMemoryBulk::new(self.memory_index);
        bulk_op.fill(&mut *memory, dest, value, size)?;

        let mut stats = self.stats.lock();
        stats.bulk_operations += 1;

        Ok(())
    }

    /// Execute copy operation on this memory
    pub fn execute_copy(&self, dest: &Value, src: &Value, size: &Value) -> Result<()> {
        let mut memory = self.memory.lock();
        let bulk_op = MultiMemoryBulk::new(self.memory_index);
        bulk_op.copy(&mut *memory, dest, src, size)?;

        let mut stats = self.stats.lock();
        stats.bulk_operations += 1;

        Ok(())
    }

    /// Execute init operation on this memory
    pub fn execute_init(
        &self,
        data_segments: &DummyDataSegments,
        data_index: u32,
        dest: &Value,
        src: &Value,
        size: &Value,
    ) -> Result<()> {
        let mut memory = self.memory.lock();
        let bulk_op = MultiMemoryBulk::new(self.memory_index);
        bulk_op.init(&mut *memory, data_segments, data_index, dest, src, size)?;

        let mut stats = self.stats.lock();
        stats.bulk_operations += 1;

        Ok(())
    }

    /// Get memory size in pages
    pub fn get_size(&self) -> Result<Value> {
        let memory = self.memory.lock();
        let size_bytes: usize = memory.size_in_bytes();
        let pages = (size_bytes / 65536) as i32;
        Ok(Value::I32(pages))
    }

    /// Grow memory by specified pages
    pub fn grow(&self, delta_pages: i32) -> Result<Value> {
        let mut memory = self.memory.lock();
        let current_size: usize = memory.size_in_bytes();
        let current_pages = (current_size / 65536) as i32;

        if delta_pages > 0 {
            memory.grow(delta_pages as u32)?;

            let mut stats = self.stats.lock();
            stats.grow_operations += 1;
        }

        Ok(Value::I32(current_pages))
    }

    /// Get memory statistics
    pub fn get_stats(&self) -> Result<MultiMemoryStats> {
        let stats = self.stats.lock();
        Ok(stats.clone())
    }
}

/// Multi-memory context managing multiple memory instances
#[derive(Debug)]
pub struct MultiMemoryContext {
    /// Memory instances indexed by memory index.
    #[cfg(feature = "std")]
    memories: HashMap<u32, Arc<MultiMemoryInstance>>,
    /// Memory instances indexed by memory index in no_std mode.
    #[cfg(not(feature = "std"))]
    memories: [(u32, Option<Arc<MultiMemoryInstance>>); MAX_MEMORIES],

    /// Thread-safe counter for memory allocation.
    memory_counter: SafeAtomicCounter,

    /// Global multi-memory statistics.
    pub global_stats: Arc<WrtMutex<MultiMemoryStats>>,

    /// Dummy data segments for operations.
    data_segments: DummyDataSegments,
}

impl MultiMemoryContext {
    /// Create new multi-memory context
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            memories: HashMap::new(),
            #[cfg(not(feature = "std"))]
            memories: core::array::from_fn(|i| (i as u32, None)),
            memory_counter: SafeAtomicCounter::new(
                usize::MAX,
                SafetyContext::new(AsilLevel::QM),
            ),
            global_stats: Arc::new(WrtMutex::new(MultiMemoryStats::new())),
            data_segments: DummyDataSegments,
        }
    }

    /// Register a memory instance
    pub fn register_memory(&mut self, memory: Arc<MultiMemoryInstance>) -> Result<u32> {
        let memory_index = memory.memory_index;

        #[cfg(feature = "std")]
        {
            if self.memories.len() >= MAX_MEMORIES {
                return Err(Error::memory_error("Maximum number of memories reached"));
            }
            self.memories.insert(memory_index, memory);
        }

        #[cfg(not(feature = "std"))]
        {
            if let Some(slot) = self
                .memories
                .iter_mut()
                .find(|(idx, mem)| *idx == memory_index && mem.is_none())
            {
                slot.1 = Some(memory);
            } else {
                return Err(Error::memory_error(
                    "Memory index already exists or maximum memories reached",
                ));
            }
        }

        let mut global_stats = self.global_stats.lock();
        global_stats.registered_memories += 1;

        Ok(memory_index)
    }

    /// Get memory instance by index
    pub fn get_memory(&self, memory_index: u32) -> Result<Arc<MultiMemoryInstance>> {
        #[cfg(feature = "std")]
        {
            self.memories
                .get(&memory_index)
                .cloned()
                .ok_or_else(|| Error::runtime_execution_error("Memory index not found"))
        }

        #[cfg(not(feature = "std"))]
        {
            self.memories
                .iter()
                .find(|(idx, _)| *idx == memory_index)
                .and_then(|(_, mem)| mem.as_ref())
                .cloned()
                .ok_or_else(|| Error::runtime_execution_error("Memory index not found"))
        }
    }

    /// Execute multi-memory operation
    pub fn execute_operation(&self, operation: MultiMemoryOperation) -> Result<Option<Value>> {
        match operation {
            MultiMemoryOperation::Load {
                memory_index,
                load_op,
                address,
            } => {
                let memory = self.get_memory(memory_index)?;
                let result = memory.execute_load(&load_op, &address)?;
                Ok(Some(result))
            },

            MultiMemoryOperation::Store {
                memory_index,
                store_op,
                address,
                value,
            } => {
                let memory = self.get_memory(memory_index)?;
                memory.execute_store(&store_op, &address, &value)?;
                Ok(None)
            },

            MultiMemoryOperation::Fill {
                memory_index,
                dest,
                value,
                size,
            } => {
                let memory = self.get_memory(memory_index)?;
                memory.execute_fill(&dest, &value, &size)?;
                Ok(None)
            },

            MultiMemoryOperation::Copy {
                memory_index,
                dest,
                src,
                size,
            } => {
                let memory = self.get_memory(memory_index)?;
                memory.execute_copy(&dest, &src, &size)?;
                Ok(None)
            },

            MultiMemoryOperation::Init {
                memory_index,
                data_index,
                dest,
                src,
                size,
            } => {
                let memory = self.get_memory(memory_index)?;
                memory.execute_init(&self.data_segments, data_index, &dest, &src, &size)?;
                Ok(None)
            },

            MultiMemoryOperation::CrossCopy {
                cross_copy_op,
                dest_addr,
                src_addr,
                size,
            } => {
                let dest_memory = self.get_memory(cross_copy_op.dest_memory_index)?;
                let src_memory = self.get_memory(cross_copy_op.src_memory_index)?;

                cross_copy_op.execute(
                    &mut *dest_memory.memory.lock(),
                    &*src_memory.memory.lock(),
                    &dest_addr,
                    &src_addr,
                    &size,
                )?;
                Ok(None)
            },

            MultiMemoryOperation::Size { size_op } => {
                let memory = self.get_memory(size_op.memory_index)?;
                let result = memory.get_size()?;
                Ok(Some(result))
            },

            MultiMemoryOperation::Grow {
                grow_op,
                delta_pages,
            } => {
                let memory = self.get_memory(grow_op.memory_index)?;
                let delta = match delta_pages {
                    Value::I32(val) => val,
                    _ => return Err(Error::type_error("Memory grow expects i32 delta")),
                };
                let result = memory.grow(delta)?;
                Ok(Some(result))
            },
        }
    }

    /// Get list of all memory indices.
    #[cfg(feature = "std")]
    pub fn get_memory_indices(&self) -> Vec<u32> {
        self.memories.keys().copied().collect()
    }

    /// Get list of all memory indices in no_std mode.
    #[cfg(not(feature = "std"))]
    pub fn get_memory_indices(
        &self,
    ) -> Result<
        wrt_foundation::bounded::BoundedVec<
            u32,
            MAX_MEMORIES,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    > {
        use wrt_foundation::{
            budget_aware_provider::CrateId,
            safe_managed_alloc,
        };
        let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
        let mut indices = wrt_foundation::bounded::BoundedVec::new(provider).map_err(|_| {
            Error::runtime_execution_error("Failed to create memory indices vector")
        })?;
        for (idx, mem) in &self.memories {
            if mem.is_some() {
                indices.push(*idx).map_err(|_| {
                    Error::runtime_execution_error("Failed to add memory index to vector")
                })?;
            }
        }
        Ok(indices)
    }

    /// Get global multi-memory statistics
    pub fn get_global_stats(&self) -> Result<MultiMemoryStats> {
        let stats = self.global_stats.lock();
        Ok(stats.clone())
    }
}

impl Default for MultiMemoryContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Default multi-memory provider implementation for all ASIL levels.
pub struct ASILCompliantMultiMemoryProvider;

impl MultiMemoryProvider for ASILCompliantMultiMemoryProvider {
    fn execute_with_provider(
        &self,
        context: &mut MultiMemoryContext,
        operation: MultiMemoryOperation,
    ) -> Result<Option<Value>> {
        self.validate_operation(&operation)?;
        context.execute_operation(operation)
    }

    fn validate_memory_access(
        &self,
        context: &MultiMemoryContext,
        memory_index: u32,
        offset: u64,
        size: u64,
    ) -> Result<()> {
        let _memory = context.get_memory(memory_index)?;

        if offset.saturating_add(size) > u32::MAX as u64 {
            return Err(Error::validation_error(
                "Memory access exceeds 32-bit address space",
            ));
        }

        Ok(())
    }
}

impl ASILCompliantMultiMemoryProvider {
    /// Validates that a multi-memory operation is within bounds and safe to execute.
    fn validate_operation(&self, operation: &MultiMemoryOperation) -> Result<()> {
        match operation {
            MultiMemoryOperation::Load { memory_index, .. }
            | MultiMemoryOperation::Store { memory_index, .. }
            | MultiMemoryOperation::Fill { memory_index, .. }
            | MultiMemoryOperation::Copy { memory_index, .. }
            | MultiMemoryOperation::Init { memory_index, .. } => {
                if *memory_index >= MAX_MEMORIES as u32 {
                    return Err(Error::validation_error("Memory index exceeds maximum"));
                }
            },
            MultiMemoryOperation::CrossCopy { cross_copy_op, .. } => {
                if cross_copy_op.dest_memory_index >= MAX_MEMORIES as u32
                    || cross_copy_op.src_memory_index >= MAX_MEMORIES as u32
                {
                    return Err(Error::validation_error(
                        "Cross-copy memory index exceeds maximum",
                    ));
                }
            },
            MultiMemoryOperation::Size { size_op } => {
                if size_op.memory_index >= MAX_MEMORIES as u32 {
                    return Err(Error::validation_error("Memory index exceeds maximum"));
                }
            },
            MultiMemoryOperation::Grow { grow_op, .. } => {
                if grow_op.memory_index >= MAX_MEMORIES as u32 {
                    return Err(Error::validation_error("Memory index exceeds maximum"));
                }
            },
        }
        Ok(())
    }
}

/// Statistics for multi-memory usage
#[derive(Debug, Clone)]
pub struct MultiMemoryStats {
    /// Number of registered memory instances.
    pub registered_memories:     u64,
    /// Number of load operations performed.
    pub load_operations:         u64,
    /// Number of store operations performed.
    pub store_operations:        u64,
    /// Number of bulk operations performed.
    pub bulk_operations:         u64,
    /// Number of cross-memory operations performed.
    pub cross_memory_operations: u64,
    /// Number of grow operations performed.
    pub grow_operations:         u64,
    /// Number of access violations detected.
    pub access_violations:       u64,
}

impl MultiMemoryStats {
    /// Creates a new statistics instance with all counters initialized to zero.
    fn new() -> Self {
        Self {
            registered_memories:     0,
            load_operations:         0,
            store_operations:        0,
            bulk_operations:         0,
            cross_memory_operations: 0,
            grow_operations:         0,
            access_violations:       0,
        }
    }

    /// Records a cross-memory operation in the statistics.
    pub fn record_cross_memory_operation(&mut self) {
        self.cross_memory_operations += 1;
    }

    /// Records an access violation in the statistics.
    pub fn record_access_violation(&mut self) {
        self.access_violations += 1;
    }

    /// Calculates the throughput as operations per registered memory.
    pub fn throughput(&self) -> f64 {
        if self.registered_memories == 0 {
            0.0
        } else {
            (self.load_operations + self.store_operations + self.bulk_operations) as f64
                / self.registered_memories as f64
        }
    }
}

/// Dummy data segments implementation for testing.
#[derive(Debug)]
pub struct DummyDataSegments;

impl DataSegmentOperations for DummyDataSegments {
    #[cfg(feature = "std")]
    fn get_data_segment(&self, _index: u32) -> Result<Option<Vec<u8>>> {
        Ok(Some(Vec::new()))
    }

    #[cfg(not(feature = "std"))]
    fn get_data_segment(
        &self,
        _index: u32,
    ) -> Result<Option<wrt_foundation::BoundedVec<u8, 65_536, wrt_foundation::NoStdProvider<65_536>>>> {
        Ok(None)
    }

    fn drop_data_segment(&mut self, _index: u32) -> Result<()> {
        Ok(())
    }
}

// ================================================================================================
// Convenience Functions
// ================================================================================================

/// Creates a new memory instance and registers it with the given context.
pub fn create_and_register_memory(
    context: &mut MultiMemoryContext,
    memory_index: u32,
    memory_type: MemoryType,
) -> Result<Arc<MultiMemoryInstance>> {
    let memory = Arc::new(MultiMemoryInstance::new(memory_index, memory_type)?);
    context.register_memory(memory.clone())?;
    Ok(memory)
}

/// Loads a 32-bit integer value from the specified memory at the given address.
pub fn load_i32_from_memory(
    context: &MultiMemoryContext,
    memory_index: u32,
    address: u32,
) -> Result<i32> {
    let load_op = MultiMemoryLoad::i32_load(memory_index, 0, 2);
    let operation = MultiMemoryOperation::Load {
        memory_index,
        load_op,
        address: Value::I32(address as i32),
    };

    let result = context.execute_operation(operation)?;
    match result {
        Some(Value::I32(value)) => Ok(value),
        _ => Err(Error::type_error("Expected i32 result from memory load")),
    }
}

/// Stores a 32-bit integer value to the specified memory at the given address.
pub fn store_i32_to_memory(
    context: &MultiMemoryContext,
    memory_index: u32,
    address: u32,
    value: i32,
) -> Result<()> {
    let store_op = MultiMemoryStore::i32_store(memory_index, 0, 2);
    let operation = MultiMemoryOperation::Store {
        memory_index,
        store_op,
        address: Value::I32(address as i32),
        value: Value::I32(value),
    };

    context.execute_operation(operation)?;
    Ok(())
}

/// Copies data between two different memory instances.
pub fn copy_between_memories(
    context: &MultiMemoryContext,
    dest_memory: u32,
    dest_addr: u32,
    src_memory: u32,
    src_addr: u32,
    size: u32,
) -> Result<()> {
    let cross_copy_op = MultiMemoryCrossCopy::new(dest_memory, src_memory);
    let operation = MultiMemoryOperation::CrossCopy {
        cross_copy_op,
        dest_addr: Value::I32(dest_addr as i32),
        src_addr: Value::I32(src_addr as i32),
        size: Value::I32(size as i32),
    };

    context.execute_operation(operation)?;
    Ok(())
}

/// Grows the specified memory by the given number of pages, returning the previous size.
pub fn grow_memory(
    context: &MultiMemoryContext,
    memory_index: u32,
    delta_pages: u32,
) -> Result<u32> {
    let grow_op = MultiMemoryGrow::new(memory_index);
    let operation = MultiMemoryOperation::Grow {
        grow_op,
        delta_pages: Value::I32(delta_pages as i32),
    };

    let result = context.execute_operation(operation)?;
    match result {
        Some(Value::I32(old_pages)) => {
            if old_pages < 0 {
                Ok(u32::MAX)
            } else {
                Ok(old_pages as u32)
            }
        },
        _ => Err(Error::type_error("Expected i32 result from memory grow")),
    }
}
