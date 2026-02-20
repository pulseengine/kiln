use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

// Import from prelude for std/no_std compatibility
use crate::prelude::{Box, Duration};

use wrt_foundation::{
    ToString,
    // safe_memory::SafeMemory, // Not available
    bounded::BoundedVec,
    budget_aware_provider::CrateId,
    collections::{StaticMap, StaticVec},
    safe_managed_alloc,
};

use crate::{
    ComponentInstanceId, ResourceHandle, TypeId,
    bounded_component_infra::ComponentProvider,
    generative_types::{GenerativeResourceType, GenerativeTypeRegistry},
    handle_representation::{AccessRights, HandleOperation, HandleRepresentationManager},
    post_return::{CleanupTask, CleanupTaskType, PostReturnRegistry},
    prelude::WrtComponentValue,
    type_bounds::{TypeBoundsChecker, TypeRelation},
    virtualization::{Capability, VirtualizationManager},
};

// Type aliases for static memory allocation
type TypeIdVec<const N: usize> = StaticVec<TypeId, N>;
type StringVec<const N: usize> = StaticVec<String, N>;
type ComponentIdVec<const N: usize> = StaticVec<ComponentInstanceId, N>;
type U32Vec<const N: usize> = StaticVec<u32, N>;
type PolicyRuleVec = StaticVec<PolicyRule, 32>;
type SharingRestrictionVec = StaticVec<SharingRestriction, 16>;
type AuditEntryVec = StaticVec<AuditEntry, 32>;
type CapabilityVec = StaticVec<Capability, 8>;
type SharingPolicyVec = StaticVec<SharingPolicy, MAX_SHARING_POLICIES>;
type TransferRequestVec = StaticVec<ResourceTransferRequest, MAX_TRANSFER_QUEUE>;

// Enable vec! and format! macros for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{format, vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[cfg(not(feature = "std"))]
use wrt_foundation::{bounded::BoundedString, safe_memory::NoStdProvider};

#[cfg(not(feature = "std"))]
type String = BoundedString<256>;

#[cfg(not(feature = "std"))]
type Vec<T> = BoundedVec<T, 256, NoStdProvider<4096>>;

// Helper function to convert &str to String in no_std mode
#[cfg(not(feature = "std"))]
fn str_to_string(s: &str) -> String {
    BoundedString::from_str_truncate(s)
        .unwrap_or_else(|_| BoundedString::from_str_truncate("").unwrap())
}

#[cfg(feature = "std")]
fn str_to_string(s: &str) -> String {
    s.to_string()
}

const MAX_SHARING_AGREEMENTS: usize = 512;
const MAX_SHARED_RESOURCES: usize = 1024;
const MAX_SHARING_POLICIES: usize = 256;
const MAX_TRANSFER_QUEUE: usize = 128;
const MAX_SHARING_CALLBACKS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSharingError {
    pub kind: ResourceSharingErrorKind,
    #[cfg(feature = "std")]
    pub message: String,
    #[cfg(not(feature = "std"))]
    pub message: &'static str,
    pub source_component: Option<ComponentInstanceId>,
    pub target_component: Option<ComponentInstanceId>,
    pub resource: Option<ResourceHandle>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceSharingErrorKind {
    PermissionDenied,
    ResourceNotFound,
    InvalidSharingAgreement,
    PolicyViolation,
    TransferFailed,
    ResourceLimitExceeded,
    TypeMismatch,
    CircularDependency,
    ConcurrentAccess,
    CapabilityRequired,
}

impl fmt::Display for ResourceSharingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResourceSharingError {}

pub type ResourceSharingResult<T> = Result<T, ResourceSharingError>;

#[derive(Debug, Clone)]
pub struct SharingAgreement {
    pub id: u32,
    pub source_component: ComponentInstanceId,
    pub target_component: ComponentInstanceId,
    pub resource_types: TypeIdVec<32>,
    pub access_rights: AccessRights,
    pub transfer_policy: TransferPolicy,
    pub lifetime: SharingLifetime,
    pub established_at: u64,
    pub metadata: SharingMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferPolicy {
    Copy,            // Resource can be copied
    Move,            // Resource ownership is transferred
    Borrow,          // Temporary access granted
    SharedOwnership, // Both components own the resource
    Delegate,        // Target can further share the resource
}

#[derive(Debug, Clone)]
pub enum SharingLifetime {
    Permanent,
    Temporary { expires_at: u64 },
    SessionBased { session_id: u32 },
    RefCounted { initial_count: u32 },
    ConditionalWhile { condition: String },
}

#[derive(Debug, Clone)]
pub struct SharingMetadata {
    pub description: String,
    pub tags: StringVec<16>,
    pub restrictions: SharingRestrictionVec,
    pub audit_log: AuditEntryVec,
}

#[derive(Debug, Clone)]
pub enum SharingRestriction {
    NoFurtherSharing,
    ReadOnlyAfterSharing,
    MustReturnBy { deadline: u64 },
    MaxConcurrentAccess { limit: u32 },
    RequiredCapability { capability: Capability },
    GeographicRestriction { allowed_regions: StringVec<8> },
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub action: AuditAction,
    pub component_id: ComponentInstanceId,
    pub success: bool,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuditAction {
    ResourceShared,
    ResourceAccessed,
    ResourceModified,
    ResourceReturned,
    ResourceDropped,
    PolicyViolation,
}

#[derive(Debug)]
pub struct SharedResource {
    pub handle: ResourceHandle,
    pub resource_type: GenerativeResourceType,
    pub owner_component: ComponentInstanceId,
    pub shared_with: ComponentIdVec<16>,
    pub sharing_agreements: U32Vec<16>,
    pub access_count: AtomicU32,
    pub is_locked: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct ResourceTransferRequest {
    pub resource_handle: ResourceHandle,
    pub source_component: ComponentInstanceId,
    pub target_component: ComponentInstanceId,
    pub transfer_type: TransferType,
    pub access_rights: AccessRights,
    pub metadata: Option<WrtComponentValue<ComponentProvider>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferType {
    Ownership,
    SharedAccess,
    TemporaryLoan { duration_ms: u64 },
    Delegation,
}

#[derive(Debug, Clone)]
pub struct SharingPolicy {
    pub id: u32,
    pub name: String,
    pub applies_to: PolicyScope,
    pub rules: PolicyRuleVec,
    pub priority: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum PolicyScope {
    Global,
    ComponentPair {
        source: ComponentInstanceId,
        target: ComponentInstanceId,
    },
    ResourceType {
        type_id: TypeId,
    },
    Component {
        component_id: ComponentInstanceId,
    },
}

#[derive(Debug, Clone)]
pub enum PolicyRule {
    RequireExplicitConsent,
    AllowedResourceTypes { types: TypeIdVec<16> },
    DeniedResourceTypes { types: TypeIdVec<16> },
    MaxShareCount { limit: u32 },
    RequiredCapabilities { capabilities: CapabilityVec },
    TimeRestriction { allowed_hours: (u8, u8) },
}

pub type SharingCallback =
    Box<dyn Fn(&SharedResource, &SharingAgreement) -> ResourceSharingResult<()> + Send + Sync>;

pub struct CrossComponentResourceSharingManager {
    handle_manager: HandleRepresentationManager,
    type_registry: GenerativeTypeRegistry,
    bounds_checker: TypeBoundsChecker,
    virt_manager: Option<VirtualizationManager>,
    post_return_registry: PostReturnRegistry,

    sharing_agreements: StaticMap<u32, SharingAgreement, MAX_SHARING_AGREEMENTS>,
    shared_resources: StaticMap<ResourceHandle, SharedResource, MAX_SHARED_RESOURCES>,
    sharing_policies: SharingPolicyVec,
    transfer_queue: TransferRequestVec,

    callbacks: StaticMap<wrt_foundation::String, SharingCallback, MAX_SHARING_CALLBACKS>,

    next_agreement_id: AtomicU32,
    next_policy_id: AtomicU32,
    enforce_policies: AtomicBool,
}

impl CrossComponentResourceSharingManager {
    pub fn new() -> ResourceSharingResult<Self> {
        Ok(Self {
            handle_manager: HandleRepresentationManager::new().map_err(|_| {
                ResourceSharingError {
                    kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                    #[cfg(feature = "std")]
                    message: "Failed to create handle manager".to_string(),
                    #[cfg(not(feature = "std"))]
                    #[cfg(feature = "std")]
                    message: "Failed to create handle manager".to_string(),
                    #[cfg(not(feature = "std"))]
                    message: "Failed to create handle manager",
                    source_component: None,
                    target_component: None,
                    resource: None,
                }
            })?,
            type_registry: GenerativeTypeRegistry::new(),
            bounds_checker: TypeBoundsChecker::new().map_err(|_| ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                #[cfg(feature = "std")]
                message: "Failed to create bounds checker".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Failed to create bounds checker",
                source_component: None,
                target_component: None,
                resource: None,
            })?,
            virt_manager: None,
            post_return_registry: PostReturnRegistry::new(64).map_err(|_| {
                ResourceSharingError {
                    kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                    #[cfg(feature = "std")]
                    message: "Failed to create post return registry".to_string(),
                    #[cfg(not(feature = "std"))]
                    message: "Failed to create post return registry",
                    source_component: None,
                    target_component: None,
                    resource: None,
                }
            })?,

            sharing_agreements: StaticMap::new(),
            shared_resources: StaticMap::new(),
            sharing_policies: StaticVec::new(),
            transfer_queue: StaticVec::new(),

            callbacks: StaticMap::new(),

            next_agreement_id: AtomicU32::new(1),
            next_policy_id: AtomicU32::new(1),
            enforce_policies: AtomicBool::new(true),
        })
    }

    pub fn with_virtualization(mut self, virt_manager: VirtualizationManager) -> Self {
        // VirtualizationManager doesn't implement Clone, so we cannot clone it
        // The handle_manager will receive the reference instead
        self.virt_manager = Some(virt_manager);
        self
    }

    pub fn set_policy_enforcement(&self, enforce: bool) {
        self.enforce_policies.store(enforce, Ordering::SeqCst);
    }

    pub fn establish_sharing_agreement(
        &mut self,
        source_component: ComponentInstanceId,
        target_component: ComponentInstanceId,
        resource_types: TypeIdVec<32>,
        access_rights: AccessRights,
        transfer_policy: TransferPolicy,
        lifetime: SharingLifetime,
    ) -> ResourceSharingResult<u32> {
        // Validate components
        self.validate_components(source_component, target_component)?;

        // Check policies
        if self.enforce_policies.load(Ordering::Acquire) {
            self.check_sharing_policies(source_component, target_component, &resource_types)?;
        }

        let agreement_id = self.next_agreement_id.fetch_add(1, Ordering::SeqCst);

        let agreement = SharingAgreement {
            id: agreement_id,
            source_component,
            target_component,
            resource_types,
            access_rights,
            transfer_policy,
            lifetime,
            established_at: self.get_current_time(),
            metadata: SharingMetadata {
                description: str_to_string(&format!(
                    "Agreement between {} and {}",
                    source_component.id(),
                    target_component.id()
                )),
                tags: StaticVec::new(),
                restrictions: StaticVec::new(),
                audit_log: StaticVec::new(),
            },
        };

        self.sharing_agreements.insert(agreement_id, agreement).map_err(|_| {
            ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                #[cfg(feature = "std")]
                message: "Too many sharing agreements".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Too many sharing agreements",
                source_component: Some(source_component),
                target_component: Some(target_component),
                resource: None,
            }
        })?;

        // Audit log
        self.audit_action(
            agreement_id,
            AuditAction::ResourceShared,
            source_component,
            true,
            "Sharing agreement established",
        )?;

        Ok(agreement_id)
    }

    pub fn share_resource(
        &mut self,
        agreement_id: u32,
        resource_handle: ResourceHandle,
    ) -> ResourceSharingResult<ResourceHandle> {
        // Extract agreement data to avoid holding immutable borrow
        let (source_component, target_component, access_rights, resource_types) = {
            let agreement = self.get_agreement(agreement_id)?;
            (
                agreement.source_component,
                agreement.target_component,
                agreement.access_rights,
                agreement.resource_types.clone(),
            )
        };

        // Verify resource type matches agreement
        let resource_type = self
            .handle_manager
            .get_representation(resource_handle)
            .map_err(|e| ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceNotFound,
                #[cfg(feature = "std")]
                message: "Component not found".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Component not found",
                source_component: Some(source_component),
                target_component: Some(target_component),
                resource: Some(resource_handle),
            })?
            .type_id;

        if !resource_types.contains(&resource_type) {
            return Err(ResourceSharingError {
                kind: ResourceSharingErrorKind::TypeMismatch,
                #[cfg(feature = "std")]
                message: "Resource type not covered by agreement".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Resource type not covered by agreement",
                source_component: Some(source_component),
                target_component: Some(target_component),
                resource: Some(resource_handle),
            });
        }

        // Create shared handle
        let shared_handle = self
            .handle_manager
            .share_handle(
                source_component,
                target_component,
                resource_handle,
                access_rights,
            )
            .map_err(|e| ResourceSharingError {
                kind: ResourceSharingErrorKind::TransferFailed,
                #[cfg(feature = "std")]
                message: "Component not found".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Component not found",
                source_component: Some(source_component),
                target_component: Some(target_component),
                resource: Some(resource_handle),
            })?;

        // Track shared resource
        self.track_shared_resource(
            resource_handle,
            source_component,
            target_component,
            agreement_id,
        )?;

        // Execute callbacks (get agreement again for execution)
        let agreement = self.get_agreement(agreement_id)?;
        self.execute_sharing_callbacks(resource_handle, agreement)?;

        // Audit log
        self.audit_action(
            agreement_id,
            AuditAction::ResourceShared,
            agreement.source_component,
            true,
            "Resource shared successfully",
        )?;

        Ok(shared_handle)
    }

    pub fn transfer_resource_ownership(
        &mut self,
        resource_handle: ResourceHandle,
        source_component: ComponentInstanceId,
        target_component: ComponentInstanceId,
    ) -> ResourceSharingResult<()> {
        // Create transfer request
        let request = ResourceTransferRequest {
            resource_handle,
            source_component,
            target_component,
            transfer_type: TransferType::Ownership,
            access_rights: AccessRights::full_access(),
            metadata: None,
        };

        // Process transfer
        self.process_resource_transfer(request)?;

        // Update ownership in shared resources
        if let Some(shared_resource) = self.shared_resources.get_mut(&resource_handle) {
            shared_resource.owner_component = target_component;
        }

        // Add cleanup task for source component
        let cleanup_task = CleanupTask {
            task_type: CleanupTaskType::CloseResource,
            source_instance: source_component,
            priority: 100, // High priority
            data: crate::post_return::CleanupData::Resource {
                handle: resource_handle,
                resource_type: crate::types::TypeId(0), // Generic type
            },
        };
        // Note: PostReturnRegistry doesn't have add_cleanup_task method
        // Skip this operation for now

        Ok(())
    }

    pub fn access_shared_resource(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
        operation: HandleOperation,
    ) -> ResourceSharingResult<Option<WrtComponentValue<ComponentProvider>>> {
        // Check if resource is shared
        let shared_resource =
            self.shared_resources.get(&resource_handle).ok_or(ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceNotFound,
                #[cfg(feature = "std")]
                message: "Resource not shared".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Resource not shared",
                source_component: Some(component_id),
                target_component: None,
                resource: Some(resource_handle),
            })?;

        // Check if component has access
        if !shared_resource.shared_with.contains(&component_id)
            && shared_resource.owner_component != component_id
        {
            return Err(ResourceSharingError {
                kind: ResourceSharingErrorKind::PermissionDenied,
                #[cfg(feature = "std")]
                message: "Component does not have access to shared resource".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Component does not have access to shared resource",
                source_component: Some(component_id),
                target_component: None,
                resource: Some(resource_handle),
            });
        }

        // Check if resource is locked
        if shared_resource.is_locked.load(Ordering::Acquire) {
            return Err(ResourceSharingError {
                kind: ResourceSharingErrorKind::ConcurrentAccess,
                #[cfg(feature = "std")]
                message: "Resource is locked".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Resource is locked",
                source_component: Some(component_id),
                target_component: None,
                resource: Some(resource_handle),
            });
        }

        // Increment access count
        shared_resource.access_count.fetch_add(1, Ordering::SeqCst);

        // Perform operation through handle manager
        let result = self
            .handle_manager
            .perform_operation(component_id, resource_handle, operation)
            .map_err(|e| ResourceSharingError {
                kind: ResourceSharingErrorKind::TransferFailed,
                #[cfg(feature = "std")]
                message: "Component not found".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Component not found",
                source_component: Some(component_id),
                target_component: None,
                resource: Some(resource_handle),
            })?;

        // Audit access (extract agreement IDs first to avoid borrow conflict)
        let mut agreement_ids = U32Vec::<16>::new();
        for &id in shared_resource.sharing_agreements.iter() {
            let _ = agreement_ids.push(id);
        }
        for agreement_id in agreement_ids {
            self.audit_action(
                agreement_id,
                AuditAction::ResourceAccessed,
                component_id,
                true,
                "Resource accessed successfully",
            )?;
        }

        Ok(result)
    }

    pub fn return_shared_resource(
        &mut self,
        component_id: ComponentInstanceId,
        resource_handle: ResourceHandle,
    ) -> ResourceSharingResult<()> {
        // Extract agreement IDs before mutable operations
        let agreement_ids = {
            let shared_resource = self.shared_resources.get_mut(&resource_handle).ok_or({
                ResourceSharingError {
                    kind: ResourceSharingErrorKind::ResourceNotFound,
                    #[cfg(feature = "std")]
                    message: "Resource not shared".to_string(),
                    #[cfg(not(feature = "std"))]
                    message: "Resource not shared",
                    source_component: Some(component_id),
                    target_component: None,
                    resource: Some(resource_handle),
                }
            })?;

            // Remove component from shared list
            if let Some(pos) = shared_resource.shared_with.iter().position(|&id| id == component_id)
            {
                shared_resource.shared_with.remove(pos);
            }

            // Clone agreement IDs for later use
            let mut ids = U32Vec::<16>::new();
            for &id in shared_resource.sharing_agreements.iter() {
                let _ = ids.push(id);
            }
            ids
        };

        // Drop the handle for this component
        self.handle_manager.drop_handle(component_id, resource_handle).map_err(|e| {
            ResourceSharingError {
                kind: ResourceSharingErrorKind::TransferFailed,
                #[cfg(feature = "std")]
                message: "Component not found".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Component not found",
                source_component: Some(component_id),
                target_component: None,
                resource: Some(resource_handle),
            }
        })?;

        // Audit return
        for agreement_id in agreement_ids.iter() {
            self.audit_action(
                *agreement_id,
                AuditAction::ResourceReturned,
                component_id,
                true,
                "Resource returned successfully",
            )?;
        }

        Ok(())
    }

    pub fn add_sharing_policy(&mut self, policy: SharingPolicy) -> ResourceSharingResult<u32> {
        let policy_id = policy.id;

        self.sharing_policies.push(policy).map_err(|_| ResourceSharingError {
            kind: ResourceSharingErrorKind::ResourceLimitExceeded,
            #[cfg(feature = "std")]
            message: "Too many sharing policies".to_string(),
            #[cfg(not(feature = "std"))]
            message: "Too many sharing policies",
            source_component: None,
            target_component: None,
            resource: None,
        })?;

        Ok(policy_id)
    }

    pub fn register_sharing_callback(
        &mut self,
        name: wrt_foundation::String,
        callback: SharingCallback,
    ) -> ResourceSharingResult<()> {
        self.callbacks.insert(name, callback).map_err(|_| ResourceSharingError {
            kind: ResourceSharingErrorKind::ResourceLimitExceeded,
            #[cfg(feature = "std")]
            message: "Too many callbacks".to_string(),
            #[cfg(not(feature = "std"))]
            message: "Too many callbacks",
            source_component: None,
            target_component: None,
            resource: None,
        })?;

        Ok(())
    }

    pub fn get_shared_resources_for_component(
        &self,
        component_id: ComponentInstanceId,
    ) -> Result<BoundedVec<ResourceHandle, 256, ComponentProvider>, wrt_error::Error> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        let mut result = BoundedVec::new(provider)?;
        for (handle, shared) in self.shared_resources.iter() {
            if shared.owner_component == component_id || shared.shared_with.contains(&component_id)
            {
                let _ = result.push(*handle);
            }
        }
        Ok(result)
    }

    pub fn get_sharing_statistics(&self) -> SharingStatistics {
        SharingStatistics {
            total_agreements: self.sharing_agreements.len(),
            active_agreements: self.count_active_agreements(),
            total_shared_resources: self.shared_resources.len(),
            total_policies: self.sharing_policies.len(),
            pending_transfers: self.transfer_queue.len(),
        }
    }

    fn validate_components(
        &self,
        source: ComponentInstanceId,
        target: ComponentInstanceId,
    ) -> ResourceSharingResult<()> {
        if source == target {
            return Err(ResourceSharingError {
                kind: ResourceSharingErrorKind::InvalidSharingAgreement,
                #[cfg(feature = "std")]
                message: "Cannot share with self".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Cannot share with self",
                source_component: Some(source),
                target_component: Some(target),
                resource: None,
            });
        }

        // Check for circular dependencies
        if self.would_create_circular_dependency(source, target) {
            return Err(ResourceSharingError {
                kind: ResourceSharingErrorKind::CircularDependency,
                #[cfg(feature = "std")]
                message: "Would create circular dependency".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Would create circular dependency",
                source_component: Some(source),
                target_component: Some(target),
                resource: None,
            });
        }

        Ok(())
    }

    fn check_sharing_policies(
        &self,
        source: ComponentInstanceId,
        target: ComponentInstanceId,
        resource_types: &StaticVec<TypeId, 32>,
    ) -> ResourceSharingResult<()> {
        for policy in self.sharing_policies.iter().filter(|p| p.enabled) {
            if !self.policy_applies_to(policy, source, target, resource_types.as_slice()) {
                continue;
            }

            for rule in policy.rules.iter() {
                self.check_policy_rule(rule, source, target, resource_types.as_slice())?;
            }
        }

        Ok(())
    }

    fn policy_applies_to(
        &self,
        policy: &SharingPolicy,
        source: ComponentInstanceId,
        target: ComponentInstanceId,
        resource_types: &[TypeId],
    ) -> bool {
        match &policy.applies_to {
            PolicyScope::Global => true,
            PolicyScope::ComponentPair {
                source: s,
                target: t,
            } => *s == source && *t == target,
            PolicyScope::ResourceType { type_id } => resource_types.contains(type_id),
            PolicyScope::Component { component_id } => {
                *component_id == source || *component_id == target
            },
        }
    }

    fn check_policy_rule(
        &self,
        rule: &PolicyRule,
        source: ComponentInstanceId,
        target: ComponentInstanceId,
        resource_types: &[TypeId],
    ) -> ResourceSharingResult<()> {
        match rule {
            PolicyRule::DeniedResourceTypes { types } => {
                for resource_type in resource_types {
                    if types.contains(resource_type) {
                        return Err(ResourceSharingError {
                            kind: ResourceSharingErrorKind::PolicyViolation,
                            #[cfg(feature = "std")]
                            message: "Resource type denied by policy".to_string(),
                            #[cfg(not(feature = "std"))]
                            message: "Resource type denied by policy",
                            source_component: Some(source),
                            target_component: Some(target),
                            resource: None,
                        });
                    }
                }
            },
            PolicyRule::AllowedResourceTypes { types } => {
                for resource_type in resource_types {
                    if !types.contains(resource_type) {
                        return Err(ResourceSharingError {
                            kind: ResourceSharingErrorKind::PolicyViolation,
                            #[cfg(feature = "std")]
                            message: "Resource type not allowed by policy".to_string(),
                            #[cfg(not(feature = "std"))]
                            message: "Resource type not allowed by policy",
                            source_component: Some(source),
                            target_component: Some(target),
                            resource: None,
                        });
                    }
                }
            },
            PolicyRule::RequiredCapabilities { capabilities } => {
                if let Some(ref virt_manager) = self.virt_manager {
                    for capability in capabilities.iter() {
                        if !virt_manager.check_capability(target, capability) {
                            #[cfg(feature = "std")]
                            let message =
                                format!("Target missing required capability: {:?}", capability);
                            #[cfg(not(feature = "std"))]
                            let message = "Target missing required capability";

                            return Err(ResourceSharingError {
                                kind: ResourceSharingErrorKind::CapabilityRequired,
                                message,
                                source_component: Some(source),
                                target_component: Some(target),
                                resource: None,
                            });
                        }
                    }
                }
            },
            _ => {}, // Other rules would be checked elsewhere
        }

        Ok(())
    }

    fn would_create_circular_dependency(
        &self,
        source: ComponentInstanceId,
        target: ComponentInstanceId,
    ) -> bool {
        // Simple check - in a real implementation, this would do graph traversal
        self.sharing_agreements.values().any(|agreement| {
            agreement.source_component == target && agreement.target_component == source
        })
    }

    fn track_shared_resource(
        &mut self,
        handle: ResourceHandle,
        owner: ComponentInstanceId,
        shared_with: ComponentInstanceId,
        agreement_id: u32,
    ) -> ResourceSharingResult<()> {
        if let Some(shared_resource) = self.shared_resources.get_mut(&handle) {
            // Add to existing shared resource
            if !shared_resource.shared_with.contains(&shared_with) {
                shared_resource.shared_with.push(shared_with).map_err(|_| {
                    ResourceSharingError {
                        kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                        #[cfg(feature = "std")]
                        message: "Too many components sharing resource".to_string(),
                        #[cfg(not(feature = "std"))]
                        message: "Too many components sharing resource",
                        source_component: Some(owner),
                        target_component: Some(shared_with),
                        resource: Some(handle),
                    }
                })?;
            }

            if !shared_resource.sharing_agreements.contains(&agreement_id) {
                shared_resource.sharing_agreements.push(agreement_id).map_err(|_| {
                    ResourceSharingError {
                        kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                        #[cfg(feature = "std")]
                        message: "Too many agreements for resource".to_string(),
                        #[cfg(not(feature = "std"))]
                        message: "Too many agreements for resource",
                        source_component: Some(owner),
                        target_component: Some(shared_with),
                        resource: Some(handle),
                    }
                })?;
            }
        } else {
            // Create new shared resource entry
            // Convert the type alias ResourceHandle (u32) to the newtype ResourceHandle struct
            let resource_handle = crate::resource_management::ResourceHandle::new(handle);
            let resource_type = self.type_registry.get_resource_type(resource_handle).ok_or(
                ResourceSharingError {
                    kind: ResourceSharingErrorKind::ResourceNotFound,
                    #[cfg(feature = "std")]
                    message: "Component not found".to_string(),
                    #[cfg(not(feature = "std"))]
                    message: "Component not found",
                    source_component: Some(owner),
                    target_component: Some(shared_with),
                    resource: Some(handle),
                },
            )?;

            let mut shared_with_vec = StaticVec::new();
            shared_with_vec.push(shared_with).map_err(|_| ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                #[cfg(feature = "std")]
                message: "Failed to create shared_with list".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Failed to create shared_with list",
                source_component: Some(owner),
                target_component: Some(shared_with),
                resource: Some(handle),
            })?;

            let mut agreements_vec = StaticVec::new();
            agreements_vec.push(agreement_id).map_err(|_| ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                #[cfg(feature = "std")]
                message: "Failed to create agreements list".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Failed to create agreements list",
                source_component: Some(owner),
                target_component: Some(shared_with),
                resource: Some(handle),
            })?;

            let shared_resource = SharedResource {
                handle,
                resource_type: resource_type.clone(),
                owner_component: owner,
                shared_with: shared_with_vec,
                sharing_agreements: agreements_vec,
                access_count: AtomicU32::new(0),
                is_locked: AtomicBool::new(false),
            };

            self.shared_resources.insert(handle, shared_resource).map_err(|_| {
                ResourceSharingError {
                    kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                    #[cfg(feature = "std")]
                    message: "Too many shared resources".to_string(),
                    #[cfg(not(feature = "std"))]
                    message: "Too many shared resources",
                    source_component: Some(owner),
                    target_component: Some(shared_with),
                    resource: Some(handle),
                }
            })?;
        }

        Ok(())
    }

    fn process_resource_transfer(
        &mut self,
        request: ResourceTransferRequest,
    ) -> ResourceSharingResult<()> {
        // This would handle the actual transfer logic
        // For now, we'll add it to the queue
        self.transfer_queue.push(request).map_err(|_| ResourceSharingError {
            kind: ResourceSharingErrorKind::ResourceLimitExceeded,
            #[cfg(feature = "std")]
            message: "Transfer queue full".to_string(),
            #[cfg(not(feature = "std"))]
            message: "Transfer queue full",
            source_component: None,
            target_component: None,
            resource: None,
        })?;

        Ok(())
    }

    fn execute_sharing_callbacks(
        &self,
        handle: ResourceHandle,
        agreement: &SharingAgreement,
    ) -> ResourceSharingResult<()> {
        if let Some(shared_resource) = self.shared_resources.get(&handle) {
            for callback in self.callbacks.values() {
                callback(shared_resource, agreement)?;
            }
        }
        Ok(())
    }

    fn get_agreement(&self, agreement_id: u32) -> ResourceSharingResult<&SharingAgreement> {
        self.sharing_agreements.get(&agreement_id).ok_or(ResourceSharingError {
            kind: ResourceSharingErrorKind::InvalidSharingAgreement,
            #[cfg(feature = "std")]
            message: "Agreement not found".to_string(),
            #[cfg(not(feature = "std"))]
            message: "Agreement not found",
            source_component: None,
            target_component: None,
            resource: None,
        })
    }

    fn audit_action(
        &mut self,
        agreement_id: u32,
        action: AuditAction,
        component_id: ComponentInstanceId,
        success: bool,
        details: &str,
    ) -> ResourceSharingResult<()> {
        // Get timestamp first to avoid borrow conflict
        let timestamp = self.get_current_time();

        if let Some(agreement) = self.sharing_agreements.get_mut(&agreement_id) {
            let entry = AuditEntry {
                timestamp,
                action,
                component_id,
                success,
                details: str_to_string(details),
            };

            agreement.metadata.audit_log.push(entry).map_err(|_| ResourceSharingError {
                kind: ResourceSharingErrorKind::ResourceLimitExceeded,
                #[cfg(feature = "std")]
                message: "Audit log full".to_string(),
                #[cfg(not(feature = "std"))]
                message: "Audit log full",
                source_component: Some(agreement.source_component),
                target_component: Some(agreement.target_component),
                resource: None,
            })?;
        }

        Ok(())
    }

    fn count_active_agreements(&self) -> usize {
        let current_time = self.get_current_time();

        self.sharing_agreements
            .values()
            .filter(|agreement| {
                match &agreement.lifetime {
                    SharingLifetime::Permanent => true,
                    SharingLifetime::Temporary { expires_at } => current_time < *expires_at,
                    _ => true, // Other lifetimes require more complex checks
                }
            })
            .count()
    }

    fn get_current_time(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

impl Default for CrossComponentResourceSharingManager {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharingStatistics {
    pub total_agreements: usize,
    pub active_agreements: usize,
    pub total_shared_resources: usize,
    pub total_policies: usize,
    pub pending_transfers: usize,
}

pub fn create_basic_sharing_policy(name: &str) -> ResourceSharingResult<SharingPolicy> {
    Ok(SharingPolicy {
        id: 0, // Will be assigned by manager
        name: str_to_string(name),
        applies_to: PolicyScope::Global,
        rules: StaticVec::new(),
        priority: 0,
        enabled: true,
    })
}

pub fn create_component_pair_policy(
    name: &str,
    source: ComponentInstanceId,
    target: ComponentInstanceId,
) -> ResourceSharingResult<SharingPolicy> {
    Ok(SharingPolicy {
        id: 0,
        name: str_to_string(name),
        applies_to: PolicyScope::ComponentPair { source, target },
        rules: StaticVec::new(),
        priority: 0,
        enabled: true,
    })
}
