//! Dynamic Resource Quota Management
//!
//! This module extends the existing resource management system with
//! hierarchical quota management that integrates directly with the memory
//! provider system. It provides dynamic quota adjustment based on runtime
//! conditions while maintaining ASIL compliance.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_foundation::{
    MemoryProvider,
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    // component::WrtComponentType, // Not available
    component_value::ComponentValue,
    prelude::*,
    // resource::ResourceHandle, // Not available
    safe_managed_alloc,
    safe_memory::{CapabilityAwareProvider, NoStdProvider},
};

// Placeholder types for missing imports
pub type WrtComponentType = u32;
pub type ResourceHandle = u32;

use crate::{
    blast_zone::{BlastZoneManager, IsolationLevel},
    resources::{
        MemoryStrategy, ResourceManager, VerificationLevel,
        resource_lifecycle::ResourceLifecycleManager,
    },
    types::{ComponentInstance, Value},
};

/// Maximum number of quota nodes in no_std environments
const MAX_QUOTA_NODES: usize = 128;

/// Maximum number of quota policies
const MAX_QUOTA_POLICIES: usize = 64;

/// Maximum number of quota watchers
const MAX_QUOTA_WATCHERS: usize = 32;

/// Quota node types in the hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuotaNodeType {
    /// Global system quota (root node)
    Global,
    /// Blast zone quota
    BlastZone,
    /// Component instance quota
    Component,
    /// Resource type quota
    ResourceType,
    /// Custom quota node
    Custom,
}

/// Resource types for quota management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// Memory allocation quota
    Memory,
    /// Resource handle quota
    Handles,
    /// File descriptor quota
    Files,
    /// Network connection quota
    Network,
    /// Compute time quota (CPU cycles)
    Compute,
    /// Custom resource type
    Custom(u32),
}

/// Quota enforcement policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaPolicy {
    /// Hard limit - fail allocation when exceeded
    Hard,
    /// Soft limit - allow temporary overflow with warning
    Soft,
    /// Elastic limit - adjust based on system load
    Elastic,
    /// Adaptive limit - learn from usage patterns
    Adaptive,
}

/// Quota adjustment strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaStrategy {
    /// Static quota - never changes
    Static,
    /// Linear growth based on utilization
    Linear,
    /// Exponential backoff on failures
    Exponential,
    /// Proportional to available resources
    Proportional,
    /// AI-based prediction (future extension)
    Predictive,
}

/// Quota status information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaStatus {
    /// Normal operation
    Normal,
    /// Approaching limit (warning threshold)
    Warning,
    /// At limit (critical threshold)
    Critical,
    /// Over limit (emergency mode)
    Emergency,
    /// Disabled quota checking
    Disabled,
}

/// Quota node in the hierarchy
#[derive(Debug, Clone)]
pub struct QuotaNode {
    /// Unique node identifier
    pub node_id: u32,
    /// Node type
    pub node_type: QuotaNodeType,
    /// Associated entity ID (component ID, blast zone ID, etc.)
    pub entity_id: u32,
    /// Parent node ID (None for root)
    pub parent_id: Option<u32>,
    /// Resource type this quota applies to
    pub resource_type: ResourceType,
    /// Maximum allowed allocation
    pub max_quota: u64,
    /// Currently allocated amount
    pub current_usage: u64,
    /// Peak usage seen
    pub peak_usage: u64,
    /// Enforcement policy
    pub policy: QuotaPolicy,
    /// Adjustment strategy
    pub strategy: QuotaStrategy,
    /// Warning threshold (percentage of max_quota)
    pub warning_threshold: u8,
    /// Critical threshold (percentage of max_quota)
    pub critical_threshold: u8,
    /// Current status
    pub status: QuotaStatus,
    /// Last update timestamp
    pub last_updated: u64,
    /// Number of allocation failures
    pub failure_count: u32,
    /// Total allocations served
    pub allocation_count: u64,
}

/// Quota allocation request
#[derive(Debug, Clone)]
pub struct QuotaRequest {
    /// Requesting entity ID
    pub entity_id: u32,
    /// Entity type
    pub entity_type: QuotaNodeType,
    /// Resource type requested
    pub resource_type: ResourceType,
    /// Amount requested
    pub amount: u64,
    /// Whether this is a temporary allocation
    pub temporary: bool,
    /// Priority level (0 = highest)
    pub priority: u8,
}

/// Quota allocation response
#[derive(Debug, Clone)]
pub struct QuotaResponse {
    /// Whether allocation was granted
    pub granted: bool,
    /// Actual amount granted (may be less than requested)
    pub amount_granted: u64,
    /// Quota reservation ID for deallocation
    pub reservation_id: Option<u32>,
    /// Reason for denial or partial grant
    pub reason: Option<String>,
    /// Suggested retry time if denied
    pub retry_after_ms: Option<u64>,
}

/// Quota watcher for notifications
pub trait QuotaWatcher {
    /// Called when quota status changes
    fn on_quota_status_change(
        &self,
        node_id: u32,
        old_status: QuotaStatus,
        new_status: QuotaStatus,
    );

    /// Called when allocation fails
    fn on_allocation_failure(&self, node_id: u32, request: &QuotaRequest);

    /// Called when quota is adjusted
    fn on_quota_adjustment(&self, node_id: u32, old_quota: u64, new_quota: u64);
}

/// Dynamic quota manager that integrates with existing resource management
pub struct DynamicQuotaManager {
    /// Quota hierarchy nodes
    #[cfg(feature = "std")]
    nodes: HashMap<u32, QuotaNode>,
    #[cfg(not(any(feature = "std",)))]
    nodes: BoundedVec<(u32, QuotaNode), MAX_QUOTA_NODES>,

    /// Active reservations
    #[cfg(feature = "std")]
    reservations: HashMap<u32, (u32, u64)>, // reservation_id -> (node_id, amount)
    #[cfg(not(any(feature = "std",)))]
    reservations: BoundedVec<(u32, (u32, u64)), 256>,

    /// Quota policies
    #[cfg(feature = "std")]
    policies: Vec<Box<dyn QuotaWatcher + Send + Sync>>,
    #[cfg(not(any(feature = "std",)))]
    policies: BoundedVec<u32, MAX_QUOTA_POLICIES>, // Simplified for no_std

    /// Integration with existing resource manager
    resource_manager: Option<ResourceManager>,

    /// Integration with blast zone manager
    blast_zone_manager: Option<BlastZoneManager>,

    /// Next node ID
    next_node_id: u32,

    /// Next reservation ID
    next_reservation_id: u32,

    /// Global memory provider for quota enforcement
    memory_provider: Option<CapabilityAwareProvider<NoStdProvider<4096>>>,
}

impl QuotaNode {
    /// Create a new quota node
    pub fn new(
        node_id: u32,
        node_type: QuotaNodeType,
        entity_id: u32,
        parent_id: Option<u32>,
        resource_type: ResourceType,
        max_quota: u64,
    ) -> Self {
        Self {
            node_id,
            node_type,
            entity_id,
            parent_id,
            resource_type,
            max_quota,
            current_usage: 0,
            peak_usage: 0,
            policy: QuotaPolicy::Hard,
            strategy: QuotaStrategy::Static,
            warning_threshold: 80,
            critical_threshold: 95,
            status: QuotaStatus::Normal,
            last_updated: 0,
            failure_count: 0,
            allocation_count: 0,
        }
    }

    /// Check if allocation would exceed quota
    pub fn can_allocate(&self, amount: u64) -> bool {
        match self.policy {
            QuotaPolicy::Hard => self.current_usage + amount <= self.max_quota,
            QuotaPolicy::Soft => true, // Allow soft overruns
            QuotaPolicy::Elastic => self.current_usage + amount <= self.max_quota * 120 / 100, /* 20% elastic */
            QuotaPolicy::Adaptive => {
                // Adaptive based on failure rate
                let failure_rate = if self.allocation_count > 0 {
                    self.failure_count as f64 / self.allocation_count as f64
                } else {
                    0.0
                };

                let adjusted_quota = if failure_rate > 0.1 {
                    self.max_quota * 90 / 100 // Be more conservative
                } else {
                    self.max_quota * 110 / 100 // Be more permissive
                };

                self.current_usage + amount <= adjusted_quota
            },
        }
    }

    /// Allocate from this quota
    pub fn allocate(&mut self, amount: u64, timestamp: u64) -> bool {
        if !self.can_allocate(amount) {
            self.failure_count += 1;
            return false;
        }

        self.current_usage += amount;
        self.allocation_count += 1;
        self.last_updated = timestamp;

        if self.current_usage > self.peak_usage {
            self.peak_usage = self.current_usage;
        }

        // Update status based on usage
        let usage_percent = (self.current_usage * 100 / self.max_quota) as u8;
        self.status = if usage_percent >= self.critical_threshold {
            QuotaStatus::Critical
        } else if usage_percent >= self.warning_threshold {
            QuotaStatus::Warning
        } else {
            QuotaStatus::Normal
        };

        true
    }

    /// Deallocate from this quota
    pub fn deallocate(&mut self, amount: u64, timestamp: u64) {
        self.current_usage = self.current_usage.saturating_sub(amount);
        self.last_updated = timestamp;

        // Update status based on new usage
        let usage_percent = if self.max_quota > 0 {
            (self.current_usage * 100 / self.max_quota) as u8
        } else {
            0
        };

        self.status = if usage_percent >= self.critical_threshold {
            QuotaStatus::Critical
        } else if usage_percent >= self.warning_threshold {
            QuotaStatus::Warning
        } else {
            QuotaStatus::Normal
        };
    }

    /// Adjust quota based on strategy
    pub fn adjust_quota(&mut self, system_load: f64, available_resources: u64) {
        match self.strategy {
            QuotaStrategy::Static => {
                // No adjustment
            },
            QuotaStrategy::Linear => {
                // Linear adjustment based on utilization
                let utilization = self.current_usage as f64 / self.max_quota as f64;
                if utilization > 0.9 {
                    self.max_quota = (self.max_quota as f64 * 1.1) as u64;
                } else if utilization < 0.5 {
                    self.max_quota = (self.max_quota as f64 * 0.95) as u64;
                }
            },
            QuotaStrategy::Exponential => {
                // Exponential backoff based on failures
                if self.failure_count > 10 {
                    self.max_quota = (self.max_quota as f64 * 0.8) as u64;
                    self.failure_count = 0; // Reset counter
                }
            },
            QuotaStrategy::Proportional => {
                // Adjust proportional to available system resources
                let total_capacity = available_resources;
                let fair_share = total_capacity / 10; // Assume 10 components max
                if self.max_quota < fair_share {
                    self.max_quota = core::cmp::min(self.max_quota * 2, fair_share);
                }
            },
            QuotaStrategy::Predictive => {
                // Future: ML-based prediction
                // For now, use simple heuristics
                let trend = if self.allocation_count > 100 {
                    self.peak_usage as f64 / self.allocation_count as f64
                } else {
                    self.current_usage as f64
                };

                if trend > self.max_quota as f64 * 0.8 {
                    self.max_quota = (trend * 1.2) as u64;
                }
            },
        }
    }

    /// Get utilization percentage
    pub fn utilization_percent(&self) -> u8 {
        if self.max_quota == 0 {
            0
        } else {
            (self.current_usage * 100 / self.max_quota) as u8
        }
    }
}

impl DynamicQuotaManager {
    /// Create a new dynamic quota manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            nodes: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            nodes: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            reservations: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            reservations: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            policies: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            policies: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            resource_manager: None,
            blast_zone_manager: None,
            next_node_id: 1,
            next_reservation_id: 1,
            memory_provider: None,
        })
    }

    /// Create a new quota manager with resource manager integration
    pub fn with_resource_manager(resource_manager: ResourceManager) -> wrt_error::Result<Self> {
        let mut manager = Self::new()?;
        manager.resource_manager = Some(resource_manager);
        Ok(manager)
    }

    /// Create a new quota manager with blast zone integration
    pub fn with_blast_zone_manager(
        blast_zone_manager: BlastZoneManager,
    ) -> wrt_error::Result<Self> {
        let mut manager = Self::new()?;
        manager.blast_zone_manager = Some(blast_zone_manager);
        Ok(manager)
    }

    /// Set memory provider for quota enforcement
    pub fn set_memory_provider(&mut self, provider: CapabilityAwareProvider<NoStdProvider<4096>>) {
        self.memory_provider = Some(provider);
    }

    /// Create a quota node in the hierarchy
    pub fn create_quota_node(
        &mut self,
        node_type: QuotaNodeType,
        entity_id: u32,
        parent_id: Option<u32>,
        resource_type: ResourceType,
        max_quota: u64,
    ) -> wrt_error::Result<u32> {
        let node_id = self.next_node_id;
        self.next_node_id += 1;

        let node = QuotaNode::new(
            node_id,
            node_type,
            entity_id,
            parent_id,
            resource_type,
            max_quota,
        );

        #[cfg(feature = "std")]
        {
            self.nodes.insert(node_id, node);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.nodes
                .push((node_id, node))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many quota nodes"))?;
        }

        Ok(node_id)
    }

    /// Request quota allocation
    pub fn request_quota(&mut self, request: &QuotaRequest) -> wrt_error::Result<QuotaResponse> {
        // Find the appropriate quota node
        let node_id = self.find_quota_node(
            request.entity_id,
            request.entity_type,
            request.resource_type,
        )?;

        // Check hierarchical constraints
        if !self.check_hierarchical_quota(node_id, request.amount)? {
            return Ok(QuotaResponse {
                granted: false,
                amount_granted: 0,
                reservation_id: None,
                reason: Some(String::from("Hierarchical quota exceeded")),
                retry_after_ms: Some(1000),
            });
        }

        // Apply allocation to the node and ancestors
        let timestamp = self.get_current_time();
        if self.allocate_hierarchical(node_id, request.amount, timestamp)? {
            let reservation_id = self.next_reservation_id;
            self.next_reservation_id += 1;

            #[cfg(feature = "std")]
            {
                self.reservations.insert(reservation_id, (node_id, request.amount));
            }
            #[cfg(not(any(feature = "std",)))]
            {
                self.reservations
                    .push((reservation_id, (node_id, request.amount)))
                    .map_err(|_| wrt_error::Error::resource_exhausted("Too many reservations"))?;
            }

            // Integrate with memory provider if available
            if let Some(ref provider) = self.memory_provider {
                // Check if memory provider has enough capacity
                if provider.capacity() < request.amount as usize {
                    // Rollback allocation
                    self.deallocate_hierarchical(node_id, request.amount, timestamp)?;
                    return Ok(QuotaResponse {
                        granted: false,
                        amount_granted: 0,
                        reservation_id: None,
                        reason: Some(String::from("Memory provider capacity exceeded")),
                        retry_after_ms: Some(5000),
                    });
                }
            }

            Ok(QuotaResponse {
                granted: true,
                amount_granted: request.amount,
                reservation_id: Some(reservation_id),
                reason: None,
                retry_after_ms: None,
            })
        } else {
            Ok(QuotaResponse {
                granted: false,
                amount_granted: 0,
                reservation_id: None,
                reason: Some(String::from("Quota allocation failed")),
                retry_after_ms: Some(2000),
            })
        }
    }

    /// Release quota allocation
    pub fn release_quota(&mut self, reservation_id: u32) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some((node_id, amount)) = self.reservations.remove(&reservation_id) {
                let timestamp = self.get_current_time();
                self.deallocate_hierarchical(node_id, amount, timestamp)?;
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let mut found = false;
            let mut node_id = 0;
            let mut amount = 0;

            for (i, (rid, (nid, amt))) in self.reservations.iter().enumerate() {
                if *rid == reservation_id {
                    node_id = *nid;
                    amount = *amt;
                    let _ = self.reservations.remove(i);
                    found = true;
                    break;
                }
            }

            if found {
                let timestamp = self.get_current_time();
                self.deallocate_hierarchical(node_id, amount, timestamp)?;
            }
        }

        Ok(())
    }

    /// Update quota based on system conditions
    pub fn update_quotas(
        &mut self,
        system_load: f64,
        available_resources: u64,
    ) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            for node in self.nodes.values_mut() {
                node.adjust_quota(system_load, available_resources);
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (_, node) in &mut self.nodes {
                node.adjust_quota(system_load, available_resources);
            }
        }

        Ok(())
    }

    /// Get quota status for a node
    pub fn get_quota_status(&self, node_id: u32) -> Option<&QuotaNode> {
        #[cfg(feature = "std")]
        {
            self.nodes.get(&node_id)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (nid, node) in &self.nodes {
                if *nid == node_id {
                    return Some(node);
                }
            }
            None
        }
    }

    /// Find quota node for entity and resource type
    fn find_quota_node(
        &self,
        entity_id: u32,
        entity_type: QuotaNodeType,
        resource_type: ResourceType,
    ) -> wrt_error::Result<u32> {
        #[cfg(feature = "std")]
        {
            for (node_id, node) in &self.nodes {
                if node.entity_id == entity_id
                    && node.node_type == entity_type
                    && node.resource_type == resource_type
                {
                    return Ok(*node_id);
                }
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (node_id, node) in &self.nodes {
                if node.entity_id == entity_id
                    && node.node_type == entity_type
                    && node.resource_type == resource_type
                {
                    return Ok(*node_id);
                }
            }
        }

        Err(wrt_error::Error::invalid_value("Quota node not found"))
    }

    /// Check hierarchical quota constraints
    fn check_hierarchical_quota(&self, node_id: u32, amount: u64) -> wrt_error::Result<bool> {
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            let node = self
                .get_quota_status(id)
                .ok_or_else(|| wrt_error::Error::invalid_value("Invalid node ID"))?;

            if !node.can_allocate(amount) {
                return Ok(false);
            }

            current_id = node.parent_id;
        }

        Ok(true)
    }

    /// Allocate quota hierarchically up the tree
    fn allocate_hierarchical(
        &mut self,
        node_id: u32,
        amount: u64,
        timestamp: u64,
    ) -> wrt_error::Result<bool> {
        let mut current_id = Some(node_id);
        #[cfg(feature = "std")]
        let mut allocated_nodes = Vec::new();
        #[cfg(not(feature = "std"))]
        let mut allocated_nodes = { BoundedVec::<u32, 64>::new().unwrap() };

        // First pass: check if all nodes can allocate
        while let Some(id) = current_id {
            #[cfg(feature = "std")]
            {
                if let Some(node) = self.nodes.get(&id) {
                    if !node.can_allocate(amount) {
                        return Ok(false);
                    }
                    allocated_nodes.push(id);
                    current_id = node.parent_id;
                } else {
                    break;
                }
            }
            #[cfg(not(any(feature = "std",)))]
            {
                let mut found = false;
                for (nid, node) in &self.nodes {
                    if *nid == id {
                        if !node.can_allocate(amount) {
                            return Ok(false);
                        }
                        let _ = allocated_nodes.push(id);
                        current_id = node.parent_id;
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }
        }

        // Second pass: actually allocate
        for id in allocated_nodes {
            #[cfg(feature = "std")]
            {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.allocate(amount, timestamp);
                }
            }
            #[cfg(not(any(feature = "std",)))]
            {
                for (nid, node) in &mut self.nodes {
                    if *nid == id {
                        node.allocate(amount, timestamp);
                        break;
                    }
                }
            }
        }

        Ok(true)
    }

    /// Deallocate quota hierarchically down the tree
    fn deallocate_hierarchical(
        &mut self,
        node_id: u32,
        amount: u64,
        timestamp: u64,
    ) -> wrt_error::Result<()> {
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            #[cfg(feature = "std")]
            {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.deallocate(amount, timestamp);
                    current_id = node.parent_id;
                } else {
                    break;
                }
            }
            #[cfg(not(any(feature = "std",)))]
            {
                let mut found = false;
                for (nid, node) in &mut self.nodes {
                    if *nid == id {
                        node.deallocate(amount, timestamp);
                        current_id = node.parent_id;
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get current time (simplified)
    fn get_current_time(&self) -> u64 {
        // In a real implementation, would use proper time measurement
        0
    }
}

impl Default for DynamicQuotaManager {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Memory => write!(f, "memory"),
            ResourceType::Handles => write!(f, "handles"),
            ResourceType::Files => write!(f, "files"),
            ResourceType::Network => write!(f, "network"),
            ResourceType::Compute => write!(f, "compute"),
            ResourceType::Custom(id) => write!(f, "custom_{}", id),
        }
    }
}

impl fmt::Display for QuotaStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaStatus::Normal => write!(f, "normal"),
            QuotaStatus::Warning => write!(f, "warning"),
            QuotaStatus::Critical => write!(f, "critical"),
            QuotaStatus::Emergency => write!(f, "emergency"),
            QuotaStatus::Disabled => write!(f, "disabled"),
        }
    }
}
