//! Handle tables with fuel tracking for Component Model resources
//!
//! This module provides fuel-aware handle table management for tracking
//! component resources with deterministic performance characteristics.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use wrt_foundation::{
    CrateId, MemoryProvider,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::{Checksum, VerificationLevel},
};

use crate::{
    async_::{
        fuel_error_context::{AsyncErrorKind, async_error},
        fuel_resource_lifetime::{ResourceHandle, ResourceState},
    },
    prelude::*,
};

/// Maximum handles per table
const MAX_HANDLES_PER_TABLE: usize = 1024;

/// Maximum handle tables per component
const MAX_HANDLE_TABLES: usize = 32;

/// Fuel costs for handle operations
const HANDLE_ALLOCATE_FUEL: u64 = 3;
const HANDLE_LOOKUP_FUEL: u64 = 1;
const HANDLE_UPDATE_FUEL: u64 = 2;
const HANDLE_DEALLOCATE_FUEL: u64 = 3;
const TABLE_CREATE_FUEL: u64 = 20;
const TABLE_RESIZE_FUEL: u64 = 50;

/// Handle entry in the table
#[derive(Debug)]
pub struct HandleEntry<T> {
    /// The actual data
    pub data: Option<T>,
    /// Generation counter for ABA problem prevention
    pub generation: u32,
    /// Resource state
    pub state: ResourceState,
    /// Last access timestamp (in fuel units)
    pub last_accessed: AtomicU64,
    /// Access count
    pub access_count: AtomicU32,
}

impl<T> HandleEntry<T> {
    /// Create a new handle entry
    pub fn new(data: T) -> Self {
        Self {
            data: Some(data),
            generation: 0,
            state: ResourceState::Available,
            last_accessed: AtomicU64::new(wrt_foundation::operations::global_fuel_consumed()),
            access_count: AtomicU32::new(0),
        }
    }

    /// Update last accessed time
    pub fn touch(&self) {
        self.last_accessed.store(
            wrt_foundation::operations::global_fuel_consumed(),
            Ordering::Release,
        );
        self.access_count.fetch_add(1, Ordering::AcqRel);
    }
}

/// Handle with generation for ABA prevention
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GenerationalHandle {
    /// Index in the handle table
    pub index: u32,
    /// Generation counter
    pub generation: u32,
}

impl GenerationalHandle {
    /// Create a new generational handle
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Convert to ResourceHandle
    pub fn to_resource_handle(self) -> ResourceHandle {
        ResourceHandle(((self.generation as u64) << 32) | (self.index as u64))
    }

    /// Create from ResourceHandle
    pub fn from_resource_handle(handle: ResourceHandle) -> Self {
        let value = handle.0;
        Self {
            index: value as u32,
            generation: (value >> 32) as u32,
        }
    }
}

impl Checksummable for GenerationalHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.index.update_checksum(checksum);
        self.generation.update_checksum(checksum);
    }
}

impl ToBytes for GenerationalHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        self.generation.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for GenerationalHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let generation = u32::from_bytes_with_provider(reader, provider)?;
        Ok(Self { index, generation })
    }
}

/// Handle table with fuel tracking
pub struct FuelHandleTable<T> {
    /// Table identifier
    pub table_id: u64,
    /// Entries in the table
    entries: BoundedVec<HandleEntry<T>, MAX_HANDLES_PER_TABLE>,
    /// Free list for handle reuse
    free_list: BoundedVec<u32, MAX_HANDLES_PER_TABLE>,
    /// Next generation counter
    next_generation: AtomicU32,
    /// Total fuel consumed
    fuel_consumed: AtomicU64,
    /// Fuel budget for this table
    fuel_budget: u64,
    /// Verification level
    verification_level: VerificationLevel,
    /// Statistics
    stats: HandleTableStats,
}

/// Statistics for handle table operations
#[derive(Debug, Default)]
pub struct HandleTableStats {
    /// Total allocations
    pub total_allocations: AtomicU64,
    /// Total deallocations
    pub total_deallocations: AtomicU64,
    /// Total lookups
    pub total_lookups: AtomicU64,
    /// Cache hits (fast path lookups)
    pub cache_hits: AtomicU64,
    /// Cache misses
    pub cache_misses: AtomicU64,
}

impl<T> FuelHandleTable<T> {
    /// Create a new handle table
    pub fn new(
        table_id: u64,
        initial_capacity: usize,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;

        let mut entries = BoundedVec::new().unwrap();
        let mut free_list = BoundedVec::new().unwrap();

        // Pre-allocate entries
        for i in (0..initial_capacity).rev() {
            free_list.push(i as u32)?;
        }

        // Record table creation
        record_global_operation(OperationType::CollectionCreate, verification_level);

        Ok(Self {
            table_id,
            entries,
            free_list,
            next_generation: AtomicU32::new(1),
            fuel_consumed: AtomicU64::new(TABLE_CREATE_FUEL),
            fuel_budget,
            verification_level,
            stats: HandleTableStats::default(),
        })
    }

    /// Allocate a new handle
    pub fn allocate(&mut self, data: T) -> Result<GenerationalHandle> {
        // Check fuel budget
        if !self.check_fuel(HANDLE_ALLOCATE_FUEL)? {
            return Err(Error::resource_limit_exceeded(
                "Handle table fuel budget exceeded",
            ));
        }

        // Get generation first
        let generation = self.next_generation.fetch_add(1, Ordering::AcqRel);

        // Get index from free list or extend table
        let index = if let Some(index) = self.free_list.pop() {
            // Update existing entry
            if let Some(entry) = self.entries.get_mut(index as usize) {
                entry.data = Some(data);
                entry.generation = generation;
                entry.state = ResourceState::Available;
                entry.touch();
            } else {
                return Err(Error::resource_error("Failed to update handle entry"));
            }
            index
        } else {
            // Need to extend the table
            if self.entries.len() >= MAX_HANDLES_PER_TABLE {
                return Err(Error::resource_limit_exceeded(
                    "Handle table capacity exceeded",
                ));
            }

            let new_index = self.entries.len() as u32;
            let mut entry = HandleEntry::new(data);
            entry.generation = generation;
            self.entries.push(entry)?;
            new_index
        };

        // Update stats
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.consume_fuel(HANDLE_ALLOCATE_FUEL)?;

        Ok(GenerationalHandle::new(index, generation))
    }

    /// Look up a handle
    pub fn lookup(&self, handle: GenerationalHandle) -> Result<&T> {
        // Check fuel
        if !self.check_fuel(HANDLE_LOOKUP_FUEL)? {
            return Err(Error::resource_limit_exceeded(
                "Handle table fuel budget exceeded",
            ));
        }

        // Validate index
        let entry = self.entries.get(handle.index as usize).ok_or_else(|| {
            self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
            Error::resource_not_found("Invalid handle index")
        })?;

        // Validate generation
        if entry.generation != handle.generation {
            self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
            return Err(Error::runtime_execution_error("Generation mismatch"));
        }

        // Check state
        if entry.state != ResourceState::Available && entry.state != ResourceState::InUse {
            return Err(Error::new(
                ErrorCategory::Resource,
                codes::RESOURCE_ACCESS_ERROR,
                "Handle not found",
            ));
        }

        // Get data
        let data = entry
            .data
            .as_ref()
            .ok_or_else(|| Error::resource_not_found("Handle data not found"))?;

        // Update access tracking
        entry.touch();
        self.stats.total_lookups.fetch_add(1, Ordering::Relaxed);
        self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
        self.consume_fuel(HANDLE_LOOKUP_FUEL)?;

        Ok(data)
    }

    /// Look up a handle mutably
    pub fn lookup_mut(&mut self, handle: GenerationalHandle) -> Result<&mut T> {
        // Check fuel
        if !self.check_fuel(HANDLE_UPDATE_FUEL)? {
            return Err(Error::resource_limit_exceeded(
                "Handle table fuel budget exceeded",
            ));
        }

        // Consume fuel before borrowing to avoid borrow conflict
        self.consume_fuel(HANDLE_UPDATE_FUEL)?;

        // Validate index
        let entry = self
            .entries
            .get_mut(handle.index as usize)
            .ok_or_else(|| Error::resource_not_found("Invalid handle index"))?;

        // Validate generation
        if entry.generation != handle.generation {
            return Err(Error::runtime_execution_error("Generation mismatch"));
        }

        // Update state
        entry.state = ResourceState::InUse;
        entry.touch();

        // Get data
        let data = entry
            .data
            .as_mut()
            .ok_or_else(|| Error::resource_not_found("Handle data not found"))?;

        Ok(data)
    }

    /// Deallocate a handle
    pub fn deallocate(&mut self, handle: GenerationalHandle) -> Result<T> {
        // Check fuel
        if !self.check_fuel(HANDLE_DEALLOCATE_FUEL)? {
            return Err(Error::resource_limit_exceeded(
                "Handle table fuel budget exceeded",
            ));
        }

        // Validate and remove
        let entry = self
            .entries
            .get_mut(handle.index as usize)
            .ok_or_else(|| Error::resource_not_found("Invalid handle index"))?;

        // Validate generation
        if entry.generation != handle.generation {
            return Err(Error::runtime_execution_error("Generation mismatch"));
        }

        // Take data
        let data = entry
            .data
            .take()
            .ok_or_else(|| Error::resource_not_found("Handle data not found"))?;

        // Update state
        entry.state = ResourceState::Dropped;
        entry.generation = entry.generation.wrapping_add(1);

        // Add to free list
        self.free_list.push(handle.index)?;

        // Update stats
        self.stats.total_deallocations.fetch_add(1, Ordering::Relaxed);
        self.consume_fuel(HANDLE_DEALLOCATE_FUEL)?;

        Ok(data)
    }

    /// Check if we have enough fuel
    fn check_fuel(&self, required: u64) -> Result<bool> {
        let current = self.fuel_consumed.load(Ordering::Acquire);
        Ok(current.saturating_add(required) <= self.fuel_budget)
    }

    /// Consume fuel
    fn consume_fuel(&self, amount: u64) -> Result<()> {
        let adjusted =
            OperationType::fuel_cost_for_operation(OperationType::Other, self.verification_level)?;

        let total = amount.saturating_add(adjusted);
        self.fuel_consumed.fetch_add(total, Ordering::AcqRel);
        record_global_operation(OperationType::Other, self.verification_level);

        Ok(())
    }

    /// Get table statistics
    pub fn stats(&self) -> &HandleTableStats {
        &self.stats
    }

    /// Get current capacity
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Get number of active handles
    pub fn active_handles(&self) -> usize {
        self.entries.len() - self.free_list.len()
    }
}

/// Handle table manager for multiple tables
pub struct HandleTableManager {
    /// Tables by ID
    tables: BoundedMap<u64, Box<dyn core::any::Any + Send + Sync>, MAX_HANDLE_TABLES>,
    /// Next table ID
    next_table_id: AtomicU64,
    /// Global fuel budget
    global_fuel_budget: u64,
    /// Total fuel consumed across all tables
    total_fuel_consumed: AtomicU64,
}

impl HandleTableManager {
    /// Create a new handle table manager
    pub fn new(global_fuel_budget: u64) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        let tables = BoundedMap::new();

        Ok(Self {
            tables,
            next_table_id: AtomicU64::new(1),
            global_fuel_budget,
            total_fuel_consumed: AtomicU64::new(0),
        })
    }

    /// Create a new handle table
    pub fn create_table<T: Send + Sync + 'static>(
        &mut self,
        initial_capacity: usize,
        verification_level: VerificationLevel,
    ) -> Result<u64> {
        let table_id = self.next_table_id.fetch_add(1, Ordering::AcqRel);

        // Allocate fuel budget for table (10% of remaining budget)
        let remaining_budget = self
            .global_fuel_budget
            .saturating_sub(self.total_fuel_consumed.load(Ordering::Acquire));
        let table_budget = remaining_budget / 10;

        let table = FuelHandleTable::<T>::new(
            table_id,
            initial_capacity,
            table_budget,
            verification_level,
        )?;

        self.tables.insert(table_id, Box::new(table))?;
        self.total_fuel_consumed.fetch_add(TABLE_CREATE_FUEL, Ordering::AcqRel);

        Ok(table_id)
    }

    /// Get a handle table
    pub fn get_table<T: Send + Sync + 'static>(
        &self,
        table_id: u64,
    ) -> Result<&FuelHandleTable<T>> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| Error::resource_not_found("Handle table not found"))?;

        // Downcast to specific type
        table
            .downcast_ref::<FuelHandleTable<T>>()
            .ok_or_else(|| Error::type_error("Handle table type mismatch"))
    }

    /// Get a handle table mutably
    pub fn get_table_mut<T: Send + Sync + 'static>(
        &mut self,
        table_id: u64,
    ) -> Result<&mut FuelHandleTable<T>> {
        let table = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| Error::resource_not_found("Handle table not found"))?;

        // Downcast to specific type
        table
            .downcast_mut::<FuelHandleTable<T>>()
            .ok_or_else(|| Error::type_error("Handle table type mismatch"))
    }

    /// Drop a table
    pub fn drop_table(&mut self, table_id: u64) -> Result<()> {
        self.tables
            .remove(&table_id)
            .ok_or_else(|| Error::resource_not_found("Handle table not found"))?;

        Ok(())
    }
}
