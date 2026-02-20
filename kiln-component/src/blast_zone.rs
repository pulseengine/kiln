//! Blast Zone Isolation for Enhanced Component Safety
//!
//! This module implements blast zone isolation mechanisms that contain failures
//! and prevent them from propagating across component boundaries. Blast zones
//! provide hierarchical isolation with different containment levels.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    // component::WrtComponentType, // Not available
    component_value::ComponentValue,
    prelude::*,
    // resource::ResourceHandle, // Not available
    safe_managed_alloc,
};

use crate::{
    // resource_lifecycle::ResourceLifecycleManager, // Module not available
    types::{ComponentInstance, Value},
};

// Placeholder types for missing imports
// WrtComponentType now exported from crate root
// ResourceHandle now exported from crate root
pub type ResourceLifecycleManager = ();

/// Maximum number of blast zones in no_std environments
const MAX_BLAST_ZONES: usize = 64;

/// Maximum number of components per blast zone
const MAX_COMPONENTS_PER_ZONE: usize = 32;

/// Maximum number of isolation policies
const MAX_ISOLATION_POLICIES: usize = 16;

/// Blast zone isolation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IsolationLevel {
    /// No isolation - failures can propagate freely
    None,
    /// Memory isolation - separate memory spaces
    Memory,
    /// Resource isolation - separate resource tables
    Resource,
    /// Capability isolation - restricted capability access
    Capability,
    /// Full isolation - complete containment with no shared state
    Full,
}

/// Blast zone containment policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentPolicy {
    /// Terminate only the failing component
    TerminateComponent,
    /// Terminate the entire blast zone
    TerminateZone,
    /// Attempt recovery with fallback
    RecoveryFallback,
    /// Quarantine zone and continue with degraded service
    QuarantineZone,
}

/// Blast zone recovery strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// No recovery - fail fast
    None,
    /// Restart component with clean state
    RestartComponent,
    /// Restart entire blast zone
    RestartZone,
    /// Migrate to backup zone
    MigrateToBackup,
    /// Graceful degradation
    GracefulDegradation,
}

/// Blast zone health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneHealth {
    /// Zone is healthy and operating normally
    Healthy,
    /// Zone is experiencing minor issues but functional
    Degraded,
    /// Zone is in recovery mode
    Recovering,
    /// Zone is quarantined due to failures
    Quarantined,
    /// Zone has failed and is terminated
    Failed,
}

/// Blast zone configuration
#[derive(Debug, Clone)]
pub struct BlastZoneConfig {
    /// Unique zone identifier
    pub zone_id: u32,
    /// Zone name for debugging
    pub zone_name: String,
    /// Isolation level for this zone
    pub isolation_level: IsolationLevel,
    /// Containment policy for failures
    pub containment_policy: ContainmentPolicy,
    /// Recovery strategy
    pub recovery_strategy: RecoveryStrategy,
    /// Maximum number of components allowed in this zone
    pub max_components: usize,
    /// Memory budget for this zone (bytes)
    pub memory_budget: usize,
    /// Maximum number of resources
    pub max_resources: u32,
    /// Failure threshold before triggering containment
    pub failure_threshold: u32,
    /// Recovery timeout in milliseconds
    pub recovery_timeout_ms: u64,
}

/// Blast zone runtime state
#[derive(Debug)]
pub struct BlastZone {
    /// Zone configuration
    config: BlastZoneConfig,
    /// Current health status
    health: ZoneHealth,
    /// Components assigned to this zone
    #[cfg(feature = "std")]
    components: Vec<u32>,
    #[cfg(not(any(feature = "std",)))]
    components: BoundedVec<u32, MAX_COMPONENTS_PER_ZONE>,
    /// Current failure count
    failure_count: u32,
    /// Last failure timestamp
    last_failure_time: u64,
    /// Memory usage tracking
    memory_used: usize,
    /// Resource usage tracking
    resources_used: u32,
    /// Zone-specific resource manager
    resource_manager: Option<ResourceLifecycleManager>,
    /// Recovery attempt count
    recovery_attempts: u32,
}

/// Isolation policy for cross-zone interactions
#[derive(Debug, Clone)]
pub struct IsolationPolicy {
    /// Policy identifier
    pub policy_id: u32,
    /// Source zone pattern (None = any zone)
    pub source_zone: Option<u32>,
    /// Target zone pattern (None = any zone)
    pub target_zone: Option<u32>,
    /// Whether interaction is allowed
    pub allowed: bool,
    /// Required capabilities for interaction
    #[cfg(feature = "std")]
    pub required_capabilities: Vec<String>,
    #[cfg(not(any(feature = "std",)))]
    pub required_capabilities: BoundedVec<String, 8>,
    /// Maximum data transfer size
    pub max_transfer_size: usize,
    /// Whether resource sharing is allowed
    pub allow_resource_sharing: bool,
}

/// Blast zone failure information
#[derive(Debug, Clone)]
pub struct ZoneFailure {
    /// Zone that failed
    pub zone_id: u32,
    /// Component that triggered the failure
    pub component_id: u32,
    /// Failure timestamp
    pub timestamp: u64,
    /// Failure reason
    pub reason: String,
    /// Stack trace or additional context
    pub context: String,
    /// Whether the failure was contained
    pub contained: bool,
}

/// Blast zone isolation manager
pub struct BlastZoneManager {
    /// All blast zones
    #[cfg(feature = "std")]
    zones: HashMap<u32, BlastZone>,
    #[cfg(not(any(feature = "std",)))]
    zones: BoundedVec<(u32, BlastZone), MAX_BLAST_ZONES>,

    /// Isolation policies
    #[cfg(feature = "std")]
    policies: Vec<IsolationPolicy>,
    #[cfg(not(any(feature = "std",)))]
    policies: BoundedVec<IsolationPolicy, MAX_ISOLATION_POLICIES>,

    /// Component to zone mapping
    #[cfg(feature = "std")]
    component_zones: HashMap<u32, u32>,
    #[cfg(not(any(feature = "std",)))]
    component_zones: BoundedVec<(u32, u32), 256>,

    /// Recent failures for analysis
    #[cfg(feature = "std")]
    failure_history: Vec<ZoneFailure>,
    #[cfg(not(any(feature = "std",)))]
    failure_history: BoundedVec<ZoneFailure, 64>,

    /// Global failure threshold
    global_failure_threshold: u32,
    /// Current global failure count
    global_failure_count: u32,
}

impl BlastZoneConfig {
    /// Create a new blast zone configuration
    pub fn new(zone_id: u32, zone_name: &str) -> Self {
        Self {
            zone_id,
            zone_name: zone_name.to_string(),
            isolation_level: IsolationLevel::Resource,
            containment_policy: ContainmentPolicy::TerminateComponent,
            recovery_strategy: RecoveryStrategy::RestartComponent,
            max_components: MAX_COMPONENTS_PER_ZONE,
            memory_budget: 16 * 1024 * 1024, // 16MB default
            max_resources: 1000,
            failure_threshold: 3,
            recovery_timeout_ms: 5000,
        }
    }

    /// Set isolation level
    pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Set containment policy
    pub fn with_containment_policy(mut self, policy: ContainmentPolicy) -> Self {
        self.containment_policy = policy;
        self
    }

    /// Set recovery strategy
    pub fn with_recovery_strategy(mut self, strategy: RecoveryStrategy) -> Self {
        self.recovery_strategy = strategy;
        self
    }

    /// Set memory budget
    pub fn with_memory_budget(mut self, budget: usize) -> Self {
        self.memory_budget = budget;
        self
    }

    /// Set failure threshold
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }
}

impl BlastZone {
    /// Create a new blast zone from configuration
    pub fn new(config: BlastZoneConfig) -> wrt_error::Result<Self> {
        Ok(Self {
            config,
            health: ZoneHealth::Healthy,
            #[cfg(feature = "std")]
            components: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            components: BoundedVec::new(),
            failure_count: 0,
            last_failure_time: 0,
            memory_used: 0,
            resources_used: 0,
            resource_manager: Some(()),
            recovery_attempts: 0,
        })
    }

    /// Add a component to this blast zone
    pub fn add_component(&mut self, component_id: u32) -> wrt_error::Result<()> {
        if self.components.len() >= self.config.max_components {
            return Err(wrt_error::Error::resource_exhausted(
                "Blast zone at capacity",
            ));
        }

        #[cfg(feature = "std")]
        {
            self.components.push(component_id);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.components.push(component_id).map_err(|_| {
                wrt_error::Error::resource_exhausted("Failed to add component to zone")
            })?;
        }

        Ok(())
    }

    /// Remove a component from this blast zone
    pub fn remove_component(&mut self, component_id: u32) -> bool {
        #[cfg(feature = "std")]
        {
            if let Some(pos) = self.components.iter().position(|&id| id == component_id) {
                self.components.remove(pos);
                return true;
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (i, &id) in self.components.iter().enumerate() {
                if id == component_id {
                    let _ = self.components.remove(i);
                    return true;
                }
            }
        }
        false
    }

    /// Record a failure in this zone
    pub fn record_failure(&mut self, component_id: u32, reason: &str, timestamp: u64) -> bool {
        self.failure_count += 1;
        self.last_failure_time = timestamp;

        // Check if we've exceeded the failure threshold
        if self.failure_count >= self.config.failure_threshold {
            self.health = ZoneHealth::Failed;
            return true; // Trigger containment
        }

        // Update health status based on failure pattern
        match self.failure_count {
            1..=2 => self.health = ZoneHealth::Degraded,
            _ => self.health = ZoneHealth::Quarantined,
        }

        false
    }

    /// Attempt recovery of this blast zone
    pub fn attempt_recovery(&mut self) -> wrt_error::Result<bool> {
        self.recovery_attempts += 1;
        self.health = ZoneHealth::Recovering;

        match self.config.recovery_strategy {
            RecoveryStrategy::None => {
                self.health = ZoneHealth::Failed;
                Ok(false)
            },
            RecoveryStrategy::RestartComponent => {
                // Would restart individual components
                self.health = ZoneHealth::Healthy;
                self.failure_count = 0;
                Ok(true)
            },
            RecoveryStrategy::RestartZone => {
                // Would restart entire zone
                self.health = ZoneHealth::Healthy;
                self.failure_count = 0;
                self.memory_used = 0;
                self.resources_used = 0;
                Ok(true)
            },
            RecoveryStrategy::MigrateToBackup => {
                // Would migrate to backup zone
                self.health = ZoneHealth::Healthy;
                Ok(true)
            },
            RecoveryStrategy::GracefulDegradation => {
                // Operate with reduced functionality
                self.health = ZoneHealth::Degraded;
                Ok(true)
            },
        }
    }

    /// Update resource usage
    pub fn update_resource_usage(
        &mut self,
        memory_delta: isize,
        resource_delta: i32,
    ) -> wrt_error::Result<()> {
        // Update memory usage
        if memory_delta < 0 {
            let decrease = (-memory_delta) as usize;
            self.memory_used = self.memory_used.saturating_sub(decrease);
        } else {
            let increase = memory_delta as usize;
            if self.memory_used + increase > self.config.memory_budget {
                return Err(wrt_error::Error::resource_exhausted(
                    "Memory budget exceeded",
                ));
            }
            self.memory_used += increase;
        }

        // Update resource usage
        if resource_delta < 0 {
            let decrease = (-resource_delta) as u32;
            self.resources_used = self.resources_used.saturating_sub(decrease);
        } else {
            let increase = resource_delta as u32;
            if self.resources_used + increase > self.config.max_resources {
                return Err(wrt_error::Error::resource_exhausted(
                    "Resource limit exceeded",
                ));
            }
            self.resources_used += increase;
        }

        Ok(())
    }

    /// Get zone health status
    pub fn health(&self) -> ZoneHealth {
        self.health
    }

    /// Get component count
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Check if zone contains component
    pub fn contains_component(&self, component_id: u32) -> bool {
        self.components.iter().any(|&id| id == component_id)
    }

    /// Get memory utilization (0.0 to 1.0)
    pub fn memory_utilization(&self) -> f64 {
        if self.config.memory_budget == 0 {
            0.0
        } else {
            self.memory_used as f64 / self.config.memory_budget as f64
        }
    }

    /// Get resource utilization (0.0 to 1.0)
    pub fn resource_utilization(&self) -> f64 {
        if self.config.max_resources == 0 {
            0.0
        } else {
            self.resources_used as f64 / self.config.max_resources as f64
        }
    }
}

impl BlastZoneManager {
    /// Create a new blast zone manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            zones: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            zones: BoundedVec::new(),
            #[cfg(feature = "std")]
            policies: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            policies: BoundedVec::new(),
            #[cfg(feature = "std")]
            component_zones: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            component_zones: BoundedVec::new(),
            #[cfg(feature = "std")]
            failure_history: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            failure_history: BoundedVec::new(),
            global_failure_threshold: 10,
            global_failure_count: 0,
        })
    }

    /// Create a new blast zone
    pub fn create_zone(&mut self, config: BlastZoneConfig) -> wrt_error::Result<u32> {
        let zone_id = config.zone_id;
        let zone = BlastZone::new(config)?;

        #[cfg(feature = "std")]
        {
            self.zones.insert(zone_id, zone);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.zones
                .push((zone_id, zone))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many blast zones"))?;
        }

        Ok(zone_id)
    }

    /// Assign a component to a blast zone
    pub fn assign_component(&mut self, component_id: u32, zone_id: u32) -> wrt_error::Result<()> {
        // Find and update the zone
        #[cfg(feature = "std")]
        {
            let zone = self
                .zones
                .get_mut(&zone_id)
                .ok_or_else(|| wrt_error::Error::invalid_value("Zone not found"))?;
            zone.add_component(component_id)?;
            self.component_zones.insert(component_id, zone_id);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let mut zone_found = false;
            for (zid, zone) in &mut self.zones {
                if *zid == zone_id {
                    zone.add_component(component_id)?;
                    zone_found = true;
                    break;
                }
            }
            if !zone_found {
                return Err(wrt_error::Error::invalid_value("Zone not found"));
            }
            self.component_zones
                .push((component_id, zone_id))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many component mappings"))?;
        }

        Ok(())
    }

    /// Handle a component failure with blast zone isolation
    pub fn handle_failure(
        &mut self,
        component_id: u32,
        reason: &str,
        timestamp: u64,
    ) -> wrt_error::Result<ContainmentPolicy> {
        self.global_failure_count += 1;

        // Find the zone containing this component
        let zone_id = self
            .get_component_zone(component_id)
            .ok_or_else(|| wrt_error::Error::invalid_value("Component not in any zone"))?;

        // Record the failure
        let failure = ZoneFailure {
            zone_id,
            component_id,
            timestamp,
            reason: reason.to_string(),
            context: format!("Global failure count: {}", self.global_failure_count),
            contained: false,
        };

        #[cfg(feature = "std")]
        {
            self.failure_history.push(failure);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let _ = self.failure_history.push(failure);
        }

        // Update zone failure state
        let containment_policy = {
            #[cfg(feature = "std")]
            {
                let zone = self.zones.get_mut(&zone_id).unwrap();
                let should_contain = zone.record_failure(component_id, reason, timestamp);
                if should_contain {
                    zone.config.containment_policy
                } else {
                    ContainmentPolicy::TerminateComponent
                }
            }
            #[cfg(not(any(feature = "std",)))]
            {
                let mut policy = ContainmentPolicy::TerminateComponent;
                for (zid, zone) in &mut self.zones {
                    if *zid == zone_id {
                        let should_contain = zone.record_failure(component_id, reason, timestamp);
                        if should_contain {
                            policy = zone.config.containment_policy;
                        }
                        break;
                    }
                }
                policy
            }
        };

        // Check global failure threshold
        if self.global_failure_count >= self.global_failure_threshold {
            return Ok(ContainmentPolicy::QuarantineZone);
        }

        Ok(containment_policy)
    }

    /// Check if interaction between zones is allowed
    pub fn is_interaction_allowed(&self, source_zone: u32, target_zone: u32) -> bool {
        #[cfg(feature = "std")]
        {
            for policy in &self.policies {
                if self.policy_matches(&policy, source_zone, target_zone) {
                    return policy.allowed;
                }
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for policy in &self.policies {
                if self.policy_matches(policy, source_zone, target_zone) {
                    return policy.allowed;
                }
            }
        }

        // Default: allow interaction between zones at same or lower isolation level
        self.get_zone_isolation_level(source_zone) <= self.get_zone_isolation_level(target_zone)
    }

    /// Get the zone containing a component
    pub fn get_component_zone(&self, component_id: u32) -> Option<u32> {
        #[cfg(feature = "std")]
        {
            self.component_zones.get(&component_id).copied()
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (cid, zone_id) in &self.component_zones {
                if *cid == component_id {
                    return Some(*zone_id);
                }
            }
            None
        }
    }

    /// Add an isolation policy
    pub fn add_policy(&mut self, policy: IsolationPolicy) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            self.policies.push(policy);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.policies
                .push(policy)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many policies"))?;
        }
        Ok(())
    }

    /// Get zone health status
    pub fn get_zone_health(&self, zone_id: u32) -> Option<ZoneHealth> {
        #[cfg(feature = "std")]
        {
            self.zones.get(&zone_id).map(|z| z.health())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (zid, zone) in &self.zones {
                if *zid == zone_id {
                    return Some(zone.health);
                }
            }
            None
        }
    }

    /// Attempt recovery of a failed zone
    pub fn recover_zone(&mut self, zone_id: u32) -> wrt_error::Result<bool> {
        #[cfg(feature = "std")]
        {
            let zone = self
                .zones
                .get_mut(&zone_id)
                .ok_or_else(|| wrt_error::Error::invalid_value("Zone not found"))?;
            zone.attempt_recovery()
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (zid, zone) in &mut self.zones {
                if *zid == zone_id {
                    return zone.attempt_recovery();
                }
            }
            Err(wrt_error::Error::invalid_value("Zone not found"))
        }
    }

    /// Get zone isolation level
    fn get_zone_isolation_level(&self, zone_id: u32) -> IsolationLevel {
        #[cfg(feature = "std")]
        {
            self.zones
                .get(&zone_id)
                .map(|z| z.config.isolation_level)
                .unwrap_or(IsolationLevel::None)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (zid, zone) in &self.zones {
                if *zid == zone_id {
                    return zone.config.isolation_level;
                }
            }
            IsolationLevel::None
        }
    }

    /// Check if a policy matches the interaction
    fn policy_matches(&self, policy: &IsolationPolicy, source_zone: u32, target_zone: u32) -> bool {
        let source_matches = policy.source_zone.is_none_or(|z| z == source_zone);
        let target_matches = policy.target_zone.is_none_or(|z| z == target_zone);
        source_matches && target_matches
    }
}

impl Default for BlastZoneManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to create default BlastZoneManager"))
    }
}

impl fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsolationLevel::None => write!(f, "none"),
            IsolationLevel::Memory => write!(f, "memory"),
            IsolationLevel::Resource => write!(f, "resource"),
            IsolationLevel::Capability => write!(f, "capability"),
            IsolationLevel::Full => write!(f, "full"),
        }
    }
}

impl fmt::Display for ZoneHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneHealth::Healthy => write!(f, "healthy"),
            ZoneHealth::Degraded => write!(f, "degraded"),
            ZoneHealth::Recovering => write!(f, "recovering"),
            ZoneHealth::Quarantined => write!(f, "quarantined"),
            ZoneHealth::Failed => write!(f, "failed"),
        }
    }
}
