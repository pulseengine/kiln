//! Priority inheritance protocol for fuel-based async tasks
//!
//! This module implements priority inheritance to prevent priority inversion
//! in async task execution, critical for ASIL-B functional safety requirements.

use core::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

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
    async_::fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum number of tasks in priority inheritance chains
const MAX_INHERITANCE_CHAIN_LENGTH: usize = 32;

/// Maximum number of priority inheritance protocols active simultaneously
const MAX_ACTIVE_PROTOCOLS: usize = 128;

/// Fuel costs for priority inheritance operations
const PRIORITY_INHERITANCE_SETUP_FUEL: u64 = 15;
const PRIORITY_INHERITANCE_RESOLVE_FUEL: u64 = 10;
const PRIORITY_DONATION_FUEL: u64 = 5;
const PRIORITY_RESTORATION_FUEL: u64 = 8;

/// Priority inheritance protocol manager
pub struct FuelPriorityInheritanceProtocol {
    /// Active inheritance chains indexed by resource ID
    inheritance_chains: BoundedMap<ResourceId, InheritanceChain, MAX_ACTIVE_PROTOCOLS>,
    /// Task priority donations tracking
    priority_donations: BoundedMap<TaskId, PriorityDonation, MAX_ACTIVE_PROTOCOLS>,
    /// Original priorities before inheritance
    original_priorities: BoundedMap<TaskId, Priority, MAX_ACTIVE_PROTOCOLS>,
    /// Resource blocking relationships
    blocking_relationships: BoundedMap<TaskId, BlockingInfo, MAX_ACTIVE_PROTOCOLS>,
    /// Global protocol statistics
    protocol_stats: ProtocolStatistics,
    /// Verification level for fuel tracking
    verification_level: VerificationLevel,
}

/// Resource identifier for blocking relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ResourceId(pub u64);

impl ResourceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn from_task(task_id: TaskId) -> Self {
        Self(task_id as u64)
    }
}

impl wrt_foundation::traits::Checksummable for ResourceId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for ResourceId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for ResourceId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u64::from_bytes_with_provider(reader, provider)?))
    }
}

/// Priority inheritance chain tracking blocked tasks
#[derive(Debug)]
pub struct InheritanceChain {
    /// Resource being contended for
    pub resource_id: ResourceId,
    /// Task currently holding the resource
    pub holder: TaskId,
    /// Tasks waiting for the resource (highest priority first)
    pub waiters: BoundedVec<TaskId, MAX_INHERITANCE_CHAIN_LENGTH>,
    /// Current inherited priority level
    pub inherited_priority: Priority,
    /// Original priority of the holder
    pub holder_original_priority: Priority,
    /// Timestamp when inheritance started
    pub inheritance_start_time: AtomicU64,
    /// Fuel consumed by inheritance operations
    pub fuel_consumed: AtomicU64,
}

impl Default for InheritanceChain {
    fn default() -> Self {
        Self {
            resource_id: ResourceId::default(),
            holder: TaskId::default(),
            waiters: BoundedVec::new(),
            inherited_priority: Priority::default(),
            holder_original_priority: Priority::default(),
            inheritance_start_time: AtomicU64::new(0),
            fuel_consumed: AtomicU64::new(0),
        }
    }
}

impl PartialEq for InheritanceChain {
    fn eq(&self, other: &Self) -> bool {
        self.resource_id == other.resource_id
            && self.holder == other.holder
            && self.inherited_priority == other.inherited_priority
            && self.holder_original_priority == other.holder_original_priority
    }
}

impl Eq for InheritanceChain {}

impl Clone for InheritanceChain {
    fn clone(&self) -> Self {
        Self {
            resource_id: self.resource_id,
            holder: self.holder,
            waiters: self.waiters.clone(),
            inherited_priority: self.inherited_priority,
            holder_original_priority: self.holder_original_priority,
            inheritance_start_time: AtomicU64::new(
                self.inheritance_start_time.load(Ordering::Relaxed),
            ),
            fuel_consumed: AtomicU64::new(self.fuel_consumed.load(Ordering::Relaxed)),
        }
    }
}

impl wrt_foundation::traits::Checksummable for InheritanceChain {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.resource_id.update_checksum(checksum);
        self.holder.update_checksum(checksum);
        self.inherited_priority.update_checksum(checksum);
        self.holder_original_priority.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for InheritanceChain {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.resource_id.to_bytes_with_provider(writer, provider)?;
        self.holder.to_bytes_with_provider(writer, provider)?;
        self.inherited_priority.to_bytes_with_provider(writer, provider)?;
        self.holder_original_priority.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for InheritanceChain {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            resource_id: ResourceId::from_bytes_with_provider(reader, provider)?,
            holder: TaskId::from_bytes_with_provider(reader, provider)?,
            waiters: BoundedVec::new(),
            inherited_priority: Priority::from_bytes_with_provider(reader, provider)?,
            holder_original_priority: Priority::from_bytes_with_provider(reader, provider)?,
            inheritance_start_time: AtomicU64::new(0),
            fuel_consumed: AtomicU64::new(0),
        })
    }
}

/// Priority donation tracking
#[derive(Debug)]
pub struct PriorityDonation {
    /// Task receiving the priority donation
    pub recipient: TaskId,
    /// Task donating the priority
    pub donor: TaskId,
    /// Donated priority level
    pub donated_priority: Priority,
    /// Resource causing the donation
    pub resource_id: ResourceId,
    /// When the donation was made
    pub donation_time: AtomicU64,
    /// Whether the donation is still active
    pub active: bool,
}

impl Default for PriorityDonation {
    fn default() -> Self {
        Self {
            recipient: TaskId::default(),
            donor: TaskId::default(),
            donated_priority: Priority::default(),
            resource_id: ResourceId::default(),
            donation_time: AtomicU64::new(0),
            active: false,
        }
    }
}

impl PartialEq for PriorityDonation {
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
            && self.donor == other.donor
            && self.donated_priority == other.donated_priority
            && self.resource_id == other.resource_id
            && self.active == other.active
    }
}

impl Eq for PriorityDonation {}

impl Clone for PriorityDonation {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient,
            donor: self.donor,
            donated_priority: self.donated_priority,
            resource_id: self.resource_id,
            donation_time: AtomicU64::new(self.donation_time.load(Ordering::Relaxed)),
            active: self.active,
        }
    }
}

impl wrt_foundation::traits::Checksummable for PriorityDonation {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.recipient.update_checksum(checksum);
        self.donor.update_checksum(checksum);
        self.donated_priority.update_checksum(checksum);
        self.resource_id.update_checksum(checksum);
        self.active.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for PriorityDonation {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.recipient.to_bytes_with_provider(writer, provider)?;
        self.donor.to_bytes_with_provider(writer, provider)?;
        self.donated_priority.to_bytes_with_provider(writer, provider)?;
        self.resource_id.to_bytes_with_provider(writer, provider)?;
        self.active.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for PriorityDonation {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            recipient: TaskId::from_bytes_with_provider(reader, provider)?,
            donor: TaskId::from_bytes_with_provider(reader, provider)?,
            donated_priority: Priority::from_bytes_with_provider(reader, provider)?,
            resource_id: ResourceId::from_bytes_with_provider(reader, provider)?,
            donation_time: AtomicU64::new(0),
            active: bool::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Information about task blocking relationships
#[derive(Debug)]
pub struct BlockingInfo {
    /// Task that is blocked
    pub blocked_task: TaskId,
    /// Resource being waited for
    pub blocked_on_resource: ResourceId,
    /// Task holding the resource
    pub blocked_by_task: Option<TaskId>,
    /// Priority of the blocked task
    pub blocked_task_priority: Priority,
    /// When the blocking started
    pub blocking_start_time: AtomicU64,
    /// Maximum blocking time allowed
    pub max_blocking_time: Option<Duration>,
}

impl Default for BlockingInfo {
    fn default() -> Self {
        Self {
            blocked_task: TaskId::default(),
            blocked_on_resource: ResourceId::default(),
            blocked_by_task: None,
            blocked_task_priority: Priority::default(),
            blocking_start_time: AtomicU64::new(0),
            max_blocking_time: None,
        }
    }
}

impl PartialEq for BlockingInfo {
    fn eq(&self, other: &Self) -> bool {
        self.blocked_task == other.blocked_task
            && self.blocked_on_resource == other.blocked_on_resource
            && self.blocked_by_task == other.blocked_by_task
            && self.blocked_task_priority == other.blocked_task_priority
            && self.max_blocking_time == other.max_blocking_time
    }
}

impl Eq for BlockingInfo {}

impl Clone for BlockingInfo {
    fn clone(&self) -> Self {
        Self {
            blocked_task: self.blocked_task,
            blocked_on_resource: self.blocked_on_resource,
            blocked_by_task: self.blocked_by_task,
            blocked_task_priority: self.blocked_task_priority,
            blocking_start_time: AtomicU64::new(self.blocking_start_time.load(Ordering::Relaxed)),
            max_blocking_time: self.max_blocking_time,
        }
    }
}

impl wrt_foundation::traits::Checksummable for BlockingInfo {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.blocked_task.update_checksum(checksum);
        self.blocked_on_resource.update_checksum(checksum);
        if let Some(task) = self.blocked_by_task {
            task.update_checksum(checksum);
        }
        self.blocked_task_priority.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for BlockingInfo {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.blocked_task.to_bytes_with_provider(writer, provider)?;
        self.blocked_on_resource.to_bytes_with_provider(writer, provider)?;
        self.blocked_by_task.to_bytes_with_provider(writer, provider)?;
        self.blocked_task_priority.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for BlockingInfo {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            blocked_task: TaskId::from_bytes_with_provider(reader, provider)?,
            blocked_on_resource: ResourceId::from_bytes_with_provider(reader, provider)?,
            blocked_by_task: Option::<TaskId>::from_bytes_with_provider(reader, provider)?,
            blocked_task_priority: Priority::from_bytes_with_provider(reader, provider)?,
            blocking_start_time: AtomicU64::new(0),
            max_blocking_time: None,
        })
    }
}

/// Protocol performance statistics
#[derive(Debug)]
pub struct ProtocolStatistics {
    /// Total number of priority inheritances performed
    pub total_inheritances: AtomicUsize,
    /// Total number of priority donations
    pub total_donations: AtomicUsize,
    /// Total number of priority inversions prevented
    pub inversions_prevented: AtomicUsize,
    /// Total fuel consumed by inheritance protocol
    pub total_fuel_consumed: AtomicU64,
    /// Maximum inheritance chain length observed
    pub max_chain_length: AtomicUsize,
    /// Number of active inheritance chains
    pub active_chains: AtomicUsize,
    /// Average resolution time in fuel units
    pub average_resolution_fuel: AtomicU64,
}

impl FuelPriorityInheritanceProtocol {
    /// Create a new priority inheritance protocol manager
    pub fn new(verification_level: VerificationLevel) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        Ok(Self {
            inheritance_chains: BoundedMap::new(),
            priority_donations: BoundedMap::new(),
            original_priorities: BoundedMap::new(),
            blocking_relationships: BoundedMap::new(),
            protocol_stats: ProtocolStatistics {
                total_inheritances: AtomicUsize::new(0),
                total_donations: AtomicUsize::new(0),
                inversions_prevented: AtomicUsize::new(0),
                total_fuel_consumed: AtomicU64::new(0),
                max_chain_length: AtomicUsize::new(0),
                active_chains: AtomicUsize::new(0),
                average_resolution_fuel: AtomicU64::new(0),
            },
            verification_level,
        })
    }

    /// Register a task blocking on a resource
    pub fn register_blocking(
        &mut self,
        blocked_task: TaskId,
        blocked_task_priority: Priority,
        resource_id: ResourceId,
        holder_task: Option<TaskId>,
        max_blocking_time: Option<Duration>,
    ) -> Result<()> {
        record_global_operation(OperationType::CollectionInsert, self.verification_level);
        self.consume_protocol_fuel(PRIORITY_INHERITANCE_SETUP_FUEL)?;

        // Record the blocking relationship
        let blocking_info = BlockingInfo {
            blocked_task,
            blocked_on_resource: resource_id,
            blocked_by_task: holder_task,
            blocked_task_priority,
            blocking_start_time: AtomicU64::new(self.get_current_fuel_time()),
            max_blocking_time,
        };

        self.blocking_relationships.insert(blocked_task, blocking_info).map_err(|_| {
            Error::resource_limit_exceeded("Too many blocking relationships tracked")
        })?;

        // If there's a holder, initiate priority inheritance
        if let Some(holder) = holder_task {
            self.initiate_priority_inheritance(
                blocked_task,
                blocked_task_priority,
                resource_id,
                holder,
            )?;
        }

        Ok(())
    }

    /// Initiate priority inheritance when a high-priority task is blocked
    pub fn initiate_priority_inheritance(
        &mut self,
        blocked_task: TaskId,
        blocked_priority: Priority,
        resource_id: ResourceId,
        holder_task: TaskId,
    ) -> Result<()> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_protocol_fuel(PRIORITY_INHERITANCE_SETUP_FUEL)?;

        // Get or create inheritance chain for this resource
        let chain_exists = self.inheritance_chains.contains_key(&resource_id);

        if chain_exists {
            // Add to existing chain
            if let Some(existing_chain) = self.inheritance_chains.get_mut(&resource_id) {
                existing_chain
                    .waiters
                    .push(blocked_task)
                    .map_err(|_| Error::resource_limit_exceeded("Inheritance chain too long"))?;

                // Update inherited priority if this waiter has higher priority
                if blocked_priority > existing_chain.inherited_priority {
                    existing_chain.inherited_priority = blocked_priority;
                }
            }

            // Now sort waiters - extract and reinsert to avoid borrow conflict
            let mut waiters_to_sort =
                if let Some(chain) = self.inheritance_chains.get_mut(&resource_id) {
                    chain.waiters.clone()
                } else {
                    return Ok(());
                };

            self.sort_waiters_by_priority(&mut waiters_to_sort)?;

            if let Some(chain) = self.inheritance_chains.get_mut(&resource_id) {
                chain.waiters = waiters_to_sort;
            }
        } else {
            // Create new inheritance chain
            let provider = safe_managed_alloc!(1024, CrateId::Component)?;
            let mut waiters = BoundedVec::new().unwrap();
            waiters
                .push(blocked_task)
                .map_err(|_| Error::resource_limit_exceeded("Failed to add waiter to new chain"))?;

            let new_chain = InheritanceChain {
                resource_id,
                holder: holder_task,
                waiters,
                inherited_priority: blocked_priority,
                holder_original_priority: 128, // Normal priority // Will be updated below
                inheritance_start_time: AtomicU64::new(self.get_current_fuel_time()),
                fuel_consumed: AtomicU64::new(PRIORITY_INHERITANCE_SETUP_FUEL),
            };

            self.inheritance_chains
                .insert(resource_id, new_chain)
                .map_err(|_| Error::resource_limit_exceeded("Too many inheritance chains"))?;

            self.protocol_stats.active_chains.fetch_add(1, Ordering::AcqRel);
        }

        // Store original priority if not already stored
        if !self.original_priorities.contains_key(&holder_task) {
            // For this example, we'll assume Normal priority as default
            // In real implementation, this would query the actual task priority
            if let Some(chain) = self.inheritance_chains.get_mut(&resource_id) {
                chain.holder_original_priority = 128; // Normal priority
            }
            self.original_priorities
                .insert(holder_task, 128 /* Normal priority */)
                .map_err(|_| {
                    Error::resource_limit_exceeded("Too many original priorities tracked")
                })?;
        }

        // Create priority donation record
        let donation = PriorityDonation {
            recipient: holder_task,
            donor: blocked_task,
            donated_priority: blocked_priority,
            resource_id,
            donation_time: AtomicU64::new(self.get_current_fuel_time()),
            active: true,
        };

        self.priority_donations
            .insert(blocked_task, donation)
            .map_err(|_| Error::resource_limit_exceeded("Too many priority donations tracked"))?;

        // Update statistics
        self.protocol_stats.total_inheritances.fetch_add(1, Ordering::AcqRel);
        self.protocol_stats.total_donations.fetch_add(1, Ordering::AcqRel);
        self.protocol_stats.inversions_prevented.fetch_add(1, Ordering::AcqRel);

        // Update max chain length
        if let Some(chain) = self.inheritance_chains.get(&resource_id) {
            let chain_length = chain.waiters.len();
            let current_max = self.protocol_stats.max_chain_length.load(Ordering::Acquire);
            if chain_length > current_max {
                self.protocol_stats.max_chain_length.store(chain_length, Ordering::Release);
            }
        }

        Ok(())
    }

    /// Release a resource and restore original priorities
    pub fn release_resource(
        &mut self,
        resource_id: ResourceId,
        releasing_task: TaskId,
    ) -> Result<Option<TaskId>> {
        record_global_operation(OperationType::CollectionRemove, self.verification_level);
        self.consume_protocol_fuel(PRIORITY_RESTORATION_FUEL)?;

        // Get the inheritance chain for this resource
        let chain = match self.inheritance_chains.remove(&resource_id) {
            Some(chain) => chain,
            None => return Ok(None), // No inheritance for this resource
        };

        self.protocol_stats.active_chains.fetch_sub(1, Ordering::AcqRel);

        // Restore the original priority of the releasing task
        if let Some(original_priority) = self.original_priorities.remove(&releasing_task) {
            // In a real implementation, this would call into the scheduler to update
            // priority For now, we just track the restoration
            self.consume_protocol_fuel(PRIORITY_RESTORATION_FUEL)?;
        }

        // Deactivate priority donations from this chain
        for &waiter in chain.waiters.iter() {
            if let Some(donation) = self.priority_donations.get_mut(&waiter) {
                donation.active = false;
            }
        }

        // Determine the next task to get the resource (highest priority waiter)
        let next_holder = chain.waiters.first().copied();

        // Remove blocking relationships for all waiters
        for &waiter in chain.waiters.iter() {
            self.blocking_relationships.remove(&waiter);
        }

        // Calculate resolution fuel for statistics
        let resolution_fuel = chain.fuel_consumed.load(Ordering::Acquire);
        let current_avg = self.protocol_stats.average_resolution_fuel.load(Ordering::Acquire);
        let new_avg = if current_avg == 0 {
            resolution_fuel
        } else {
            (current_avg + resolution_fuel) / 2
        };
        self.protocol_stats.average_resolution_fuel.store(new_avg, Ordering::Release);

        Ok(next_holder)
    }

    /// Check for and resolve potential priority inversions
    pub fn check_priority_inversion(
        &mut self,
        task_id: TaskId,
        task_priority: Priority,
    ) -> Result<bool> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_protocol_fuel(PRIORITY_INHERITANCE_RESOLVE_FUEL)?;

        // Check if this task is being blocked by a lower priority task
        if let Some(blocking_info) = self.blocking_relationships.get(&task_id) {
            if let Some(blocking_task) = blocking_info.blocked_by_task {
                // Check if blocking task has lower priority than blocked task
                if blocking_info.blocked_task_priority > task_priority {
                    // Potential priority inversion detected
                    self.initiate_priority_inheritance(
                        task_id,
                        blocking_info.blocked_task_priority,
                        blocking_info.blocked_on_resource,
                        blocking_task,
                    )?;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get priority inheritance statistics
    pub fn get_statistics(&self) -> ProtocolStatistics {
        ProtocolStatistics {
            total_inheritances: AtomicUsize::new(
                self.protocol_stats.total_inheritances.load(Ordering::Acquire),
            ),
            total_donations: AtomicUsize::new(
                self.protocol_stats.total_donations.load(Ordering::Acquire),
            ),
            inversions_prevented: AtomicUsize::new(
                self.protocol_stats.inversions_prevented.load(Ordering::Acquire),
            ),
            total_fuel_consumed: AtomicU64::new(
                self.protocol_stats.total_fuel_consumed.load(Ordering::Acquire),
            ),
            max_chain_length: AtomicUsize::new(
                self.protocol_stats.max_chain_length.load(Ordering::Acquire),
            ),
            active_chains: AtomicUsize::new(
                self.protocol_stats.active_chains.load(Ordering::Acquire),
            ),
            average_resolution_fuel: AtomicU64::new(
                self.protocol_stats.average_resolution_fuel.load(Ordering::Acquire),
            ),
        }
    }

    /// Get the effective priority for a task (considering donations)
    pub fn get_effective_priority(&self, task_id: TaskId, base_priority: Priority) -> Priority {
        // Check if this task has received any priority donations
        for donation in self.priority_donations.values() {
            if donation.recipient == task_id
                && donation.active
                && donation.donated_priority > base_priority
            {
                return donation.donated_priority;
            }
        }
        base_priority
    }

    /// Clean up expired or resolved inheritance relationships
    pub fn cleanup_expired_inheritances(&mut self, current_fuel_time: u64) -> Result<usize> {
        record_global_operation(OperationType::CollectionMutate, self.verification_level);

        let mut cleaned_count = 0;
        let mut expired_resources = Vec::new();

        // Find expired inheritance chains
        for (resource_id, chain) in self.inheritance_chains.iter() {
            let start_time = chain.inheritance_start_time.load(Ordering::Acquire);
            let elapsed_fuel = current_fuel_time.saturating_sub(start_time);

            // Check if any blocking relationships have expired
            let mut has_expired = false;
            for &waiter in chain.waiters.iter() {
                if let Some(blocking_info) = self.blocking_relationships.get(&waiter) {
                    if let Some(max_time) = blocking_info.max_blocking_time {
                        let max_fuel_time = max_time.as_millis() as u64; // 1ms = 1 fuel
                        if elapsed_fuel > max_fuel_time {
                            has_expired = true;
                            break;
                        }
                    }
                }
            }

            if has_expired {
                expired_resources.push(*resource_id);
            }
        }

        // Clean up expired resources
        for resource_id in expired_resources {
            if let Some(chain) = self.inheritance_chains.remove(&resource_id) {
                // Restore priorities and clean up relationships
                for &waiter in chain.waiters.iter() {
                    self.blocking_relationships.remove(&waiter);
                    if let Some(donation) = self.priority_donations.get_mut(&waiter) {
                        donation.active = false;
                    }
                }

                // Restore holder's original priority
                if let Some(original_priority) = self.original_priorities.remove(&chain.holder) {
                    // Priority restoration would happen here in real implementation
                    self.consume_protocol_fuel(PRIORITY_RESTORATION_FUEL)?;
                }

                cleaned_count += 1;
                self.protocol_stats.active_chains.fetch_sub(1, Ordering::AcqRel);
            }
        }

        Ok(cleaned_count)
    }

    // Private helper methods

    fn get_current_fuel_time(&self) -> u64 {
        // In a real implementation, this would get the current fuel time from the
        // system For now, we'll use a simple counter
        self.protocol_stats.total_fuel_consumed.load(Ordering::Acquire)
    }

    fn consume_protocol_fuel(&self, amount: u64) -> Result<()> {
        self.protocol_stats.total_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
        Ok(())
    }

    fn sort_waiters_by_priority(
        &self,
        waiters: &mut BoundedVec<TaskId, MAX_INHERITANCE_CHAIN_LENGTH>,
    ) -> Result<()> {
        // Simple bubble sort for small collections
        let len = waiters.len();
        for i in 0..len {
            for j in 0..len.saturating_sub(1 + i) {
                if self.should_swap_by_priority(waiters[j], waiters[j + 1])? {
                    // Swap tasks
                    let temp = waiters[j];
                    waiters[j] = waiters[j + 1];
                    waiters[j + 1] = temp;
                }
            }
        }
        Ok(())
    }

    fn should_swap_by_priority(&self, task_a: TaskId, task_b: TaskId) -> Result<bool> {
        // Get priority for each task from blocking relationships
        let priority_a = self
            .blocking_relationships
            .get(&task_a)
            .map(|info| info.blocked_task_priority)
            .unwrap_or(128); // Normal priority

        let priority_b = self
            .blocking_relationships
            .get(&task_b)
            .map(|info| info.blocked_task_priority)
            .unwrap_or(128); // Normal priority

        // Higher priority tasks should come first (task_a > task_b means swap)
        Ok(priority_a < priority_b)
    }
}

impl Default for FuelPriorityInheritanceProtocol {
    fn default() -> Self {
        Self::new(VerificationLevel::Standard)
            .expect("Failed to create default priority inheritance protocol")
    }
}
