use crate::{
    ComponentInstanceId, ResourceHandle, TypeId,
    bounded_component_infra::ComponentProvider,
    generative_types::{GenerativeResourceType, GenerativeTypeRegistry},
    type_bounds::{TypeBoundsChecker, TypeRelation},
    virtualization::{Capability, VirtualizationManager},
};
use core::{
    fmt,
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::StaticMap as BoundedMap,
    collections::StaticVec as BoundedVec,
    component_value::ComponentValue,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ToBytes},
};

// Enable vec! and format! macros for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{format, vec};

#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[cfg(not(feature = "std"))]
type String = wrt_foundation::BoundedString<256>;
#[cfg(not(feature = "std"))]
type Vec<T> = BoundedVec<T, 64>;

// Helper function to create error message strings
#[cfg(not(feature = "std"))]
fn error_msg(s: &str) -> String {
    let provider = NoStdProvider::<1024>::default();
    wrt_foundation::BoundedString::from_str_truncate(s).unwrap_or_default()
}

#[cfg(feature = "std")]
fn error_msg(s: &str) -> String {
    String::from(s)
}

const MAX_HANDLE_REPRESENTATIONS: usize = 1024;
const MAX_HANDLE_OPERATIONS: usize = 512;
const MAX_HANDLE_METADATA: usize = 256;
const MAX_ACCESS_POLICIES: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct HandleRepresentationError {
    pub kind: HandleRepresentationErrorKind,
    pub message: String,
    pub handle: Option<ResourceHandle>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HandleRepresentationErrorKind {
    InvalidHandle,
    TypeMismatch,
    AccessDenied,
    HandleNotFound,
    OperationNotSupported,
    ResourceExhausted,
    ValidationFailed,
    CapabilityRequired,
}

impl fmt::Display for HandleRepresentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            write!(f, "{:?}: {}", self.kind, self.message)
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std, BoundedString doesn't implement Display, so convert to str
            match self.message.as_str() {
                Ok(s) => write!(f, "{:?}: {}", self.kind, s),
                Err(_) => write!(f, "{:?}: [invalid utf8]", self.kind),
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HandleRepresentationError {}

// Conversion to wrt_error::Error for unified error handling
impl From<HandleRepresentationError> for wrt_error::Error {
    fn from(err: HandleRepresentationError) -> Self {
        use wrt_error::{ErrorCategory, codes};
        match err.kind {
            HandleRepresentationErrorKind::InvalidHandle => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_HANDLE_REPRESENTATION_ERROR,
                "Invalid resource handle",
            ),
            HandleRepresentationErrorKind::TypeMismatch => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_ABI_RUNTIME_ERROR,
                "Handle type mismatch",
            ),
            HandleRepresentationErrorKind::AccessDenied => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_CAPABILITY_DENIED,
                "Handle access denied",
            ),
            HandleRepresentationErrorKind::HandleNotFound => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_HANDLE_REPRESENTATION_ERROR,
                "Resource handle not found",
            ),
            HandleRepresentationErrorKind::OperationNotSupported => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_ABI_RUNTIME_ERROR,
                "Handle operation not supported",
            ),
            HandleRepresentationErrorKind::ResourceExhausted => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_RESOURCE_LIFECYCLE_ERROR,
                "Handle resource exhausted",
            ),
            HandleRepresentationErrorKind::ValidationFailed => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_CONFIGURATION_INVALID,
                "Handle validation failed",
            ),
            HandleRepresentationErrorKind::CapabilityRequired => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_CAPABILITY_DENIED,
                "Handle operation requires capability",
            ),
        }
    }
}

pub type HandleRepresentationResult<T> = wrt_error::Result<T>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandleRepresentation {
    pub handle: ResourceHandle,
    pub type_id: TypeId,
    pub component_id: ComponentInstanceId,
    pub access_rights: AccessRights,
    pub is_owned: bool,
    pub reference_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AccessRights {
    pub can_read: bool,
    pub can_write: bool,
    pub can_drop: bool,
    pub can_share: bool,
    pub can_borrow: bool,
}

impl AccessRights {
    pub fn read_only() -> Self {
        Self {
            can_read: true,
            can_write: false,
            can_drop: false,
            can_share: false,
            can_borrow: true,
        }
    }

    pub fn full_access() -> Self {
        Self {
            can_read: true,
            can_write: true,
            can_drop: true,
            can_share: true,
            can_borrow: true,
        }
    }

    pub fn no_access() -> Self {
        Self {
            can_read: false,
            can_write: false,
            can_drop: false,
            can_share: false,
            can_borrow: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HandleMetadata {
    pub created_at: u64,
    pub last_accessed: u64,
    pub access_count: u32,
    pub creator_component: ComponentInstanceId,
    pub tags: BoundedVec<String, 16>,
    pub custom_data: BoundedMap<String, ComponentValue<ComponentProvider>, 32>,
}

#[derive(Debug, Clone)]
pub enum HandleOperation {
    Read {
        fields: BoundedVec<String, 16>,
    },
    Write {
        fields: BoundedVec<(String, ComponentValue<ComponentProvider>), 16>,
    },
    Call {
        method: String,
        args: BoundedVec<ComponentValue<ComponentProvider>, 16>,
    },
    Drop,
    Share {
        target_component: ComponentInstanceId,
    },
    Borrow {
        mutable: bool,
    },
    Return {
        from_borrow: bool,
    },
}

#[derive(Debug, Clone)]
pub struct HandleAccessPolicy {
    pub component_id: ComponentInstanceId,
    pub resource_type: TypeId,
    pub allowed_operations: BoundedVec<HandleOperation, 32>,
    pub required_capability: Option<Capability>,
    pub expiry: Option<u64>,
}

#[derive(Debug)]
pub struct HandleRepresentationManager {
    representations: BoundedMap<ResourceHandle, HandleRepresentation, MAX_HANDLE_REPRESENTATIONS>,
    metadata: BoundedMap<ResourceHandle, HandleMetadata, MAX_HANDLE_METADATA>,
    access_policies: BoundedVec<HandleAccessPolicy, MAX_ACCESS_POLICIES>,
    type_registry: GenerativeTypeRegistry,
    bounds_checker: TypeBoundsChecker,
    virt_manager: Option<VirtualizationManager>,
    next_handle_id: AtomicU32,
    strict_type_checking: AtomicBool,
}

impl HandleRepresentationManager {
    pub fn new() -> HandleRepresentationResult<Self> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        Ok(Self {
            representations: BoundedMap::new(),
            metadata: BoundedMap::new(),
            access_policies: { BoundedVec::new().unwrap() },
            type_registry: GenerativeTypeRegistry::new(),
            bounds_checker: TypeBoundsChecker::new()?,
            virt_manager: None,
            next_handle_id: AtomicU32::new(1),
            strict_type_checking: AtomicBool::new(true),
        })
    }

    pub fn with_virtualization(mut self, virt_manager: VirtualizationManager) -> Self {
        self.virt_manager = Some(virt_manager);
        self
    }

    pub fn set_strict_type_checking(&self, strict: bool) {
        self.strict_type_checking.store(strict, Ordering::SeqCst);
    }

    pub fn create_handle(
        &mut self,
        component_id: ComponentInstanceId,
        resource_type: GenerativeResourceType,
        access_rights: AccessRights,
    ) -> HandleRepresentationResult<ResourceHandle> {
        let handle_id = self.next_handle_id.fetch_add(1, Ordering::SeqCst);
        let handle = handle_id; // ResourceHandle type alias for u32

        let representation = HandleRepresentation {
            handle,
            type_id: resource_type.unique_type_id.into_inner(),
            component_id,
            access_rights,
            is_owned: true,
            reference_count: 1,
        };

        self.representations.insert(handle, representation).map_err(|_| {
            HandleRepresentationError {
                kind: HandleRepresentationErrorKind::ResourceExhausted,
                message: error_msg("Too many handle representations"),
                handle: Some(handle),
            }
        })?;

        // Create metadata
        let metadata = HandleMetadata {
            created_at: self.get_current_time(),
            last_accessed: self.get_current_time(),
            access_count: 0,
            creator_component: component_id,
            tags: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            custom_data: BoundedMap::new(),
        };

        self.metadata.insert(handle, metadata).map_err(|_| HandleRepresentationError {
            kind: HandleRepresentationErrorKind::ResourceExhausted,
            message: {
                #[cfg(feature = "std")]
                {
                    String::from("Too many handle metadata entries")
                }
                #[cfg(not(feature = "std"))]
                {
                    error_msg("Too many handle metadata entries")
                }
            },
            handle: Some(handle),
        })?;

        // Map handle to resource type
        // Convert the type alias ResourceHandle (u32) to the newtype ResourceHandle struct
        let resource_handle = crate::resource_management::ResourceHandle::new(handle);
        self.type_registry
            .register_resource_handle(resource_handle, resource_type)
            .map_err(|_e| HandleRepresentationError {
                kind: HandleRepresentationErrorKind::ValidationFailed,
                message: {
                    #[cfg(feature = "std")]
                    {
                        String::from("Handle operation failed")
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        error_msg("Handle operation failed")
                    }
                },
                handle: Some(handle),
            })?;

        Ok(handle)
    }

    pub fn get_representation(
        &self,
        handle: ResourceHandle,
    ) -> HandleRepresentationResult<&HandleRepresentation> {
        self.representations.get(&handle).ok_or_else(|| {
            HandleRepresentationError {
                kind: HandleRepresentationErrorKind::HandleNotFound,
                message: {
                    #[cfg(feature = "std")]
                    {
                        String::from("Component operation error")
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        error_msg("Component operation error")
                    }
                },
                handle: Some(handle),
            }
            .into()
        })
    }

    pub fn perform_operation(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        operation: HandleOperation,
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // Check if handle exists and get representation
        let representation = self.get_representation(handle)?;

        // Verify component has access
        self.verify_access(component_id, handle, &operation)?;

        // Update metadata (extract time first to avoid borrow conflict)
        let current_time = self.get_current_time();
        if let Some(metadata) = self.metadata.get_mut(&handle) {
            metadata.last_accessed = current_time;
            metadata.access_count = metadata.access_count.saturating_add(1);
        }

        // Perform the operation
        match operation {
            HandleOperation::Read { fields } => {
                self.handle_read_operation(handle, fields.as_slice())
            },
            HandleOperation::Write { fields } => {
                self.handle_write_operation(handle, fields.as_slice())
            },
            HandleOperation::Call { method, args } => {
                #[cfg(feature = "std")]
                let method_str = method.as_str();
                #[cfg(not(feature = "std"))]
                let method_str = method.as_str()?;
                self.handle_call_operation(handle, method_str, args.as_slice())
            },
            HandleOperation::Drop => self.handle_drop_operation(component_id, handle),
            HandleOperation::Share { target_component } => {
                self.handle_share_operation(component_id, handle, target_component)
            },
            HandleOperation::Borrow { mutable } => {
                self.handle_borrow_operation(component_id, handle, mutable)
            },
            HandleOperation::Return { from_borrow } => {
                self.handle_return_operation(component_id, handle, from_borrow)
            },
        }
    }

    pub fn add_access_policy(
        &mut self,
        policy: HandleAccessPolicy,
    ) -> HandleRepresentationResult<()> {
        self.access_policies.push(policy).map_err(|_| HandleRepresentationError {
            kind: HandleRepresentationErrorKind::ResourceExhausted,
            message: {
                #[cfg(feature = "std")]
                {
                    String::from("Too many access policies")
                }
                #[cfg(not(feature = "std"))]
                {
                    error_msg("Too many access policies")
                }
            },
            handle: None,
        })?;
        Ok(())
    }

    pub fn share_handle(
        &mut self,
        source_component: ComponentInstanceId,
        target_component: ComponentInstanceId,
        handle: ResourceHandle,
        new_access_rights: AccessRights,
    ) -> HandleRepresentationResult<ResourceHandle> {
        // Verify source has share permission
        let operation = HandleOperation::Share { target_component };
        self.verify_access(source_component, handle, &operation)?;

        // Get original representation
        let original = *self.get_representation(handle)?;

        // Create new handle for target component
        let new_handle_id = self.next_handle_id.fetch_add(1, Ordering::SeqCst);
        let new_handle = new_handle_id; // ResourceHandle is u32

        // Create shared representation
        let shared_representation = HandleRepresentation {
            handle: new_handle,
            type_id: original.type_id,
            component_id: target_component,
            access_rights: new_access_rights,
            is_owned: false,
            reference_count: 1,
        };

        self.representations.insert(new_handle, shared_representation).map_err(|_| {
            HandleRepresentationError {
                kind: HandleRepresentationErrorKind::ResourceExhausted,
                message: {
                    #[cfg(feature = "std")]
                    {
                        String::from("Too many handle representations")
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        error_msg("Too many handle representations")
                    }
                },
                handle: Some(new_handle),
            }
        })?;

        // Copy metadata with updated info
        if let Some(original_metadata) = self.metadata.get(&handle) {
            let mut shared_metadata = original_metadata.clone();
            let tag = {
                #[cfg(feature = "std")]
                {
                    String::from("shared")
                }
                #[cfg(not(feature = "std"))]
                {
                    error_msg("shared")
                }
            };
            shared_metadata.tags.push(tag).ok();

            self.metadata.insert(new_handle, shared_metadata).map_err(|_| {
                HandleRepresentationError {
                    kind: HandleRepresentationErrorKind::ResourceExhausted,
                    message: {
                        #[cfg(feature = "std")]
                        {
                            String::from("Too many metadata entries")
                        }
                        #[cfg(not(feature = "std"))]
                        {
                            error_msg("Too many metadata entries")
                        }
                    },
                    handle: Some(new_handle),
                }
            })?;
        }

        // Increment reference count on original
        if let Some(original_repr) = self.representations.get_mut(&handle) {
            original_repr.reference_count = original_repr.reference_count.saturating_add(1);
        }

        Ok(new_handle)
    }

    pub fn drop_handle(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
    ) -> HandleRepresentationResult<()> {
        // Verify component can drop
        let operation = HandleOperation::Drop;
        self.verify_access(component_id, handle, &operation)?;

        // Get representation
        let representation =
            self.representations.get_mut(&handle).ok_or_else(|| HandleRepresentationError {
                kind: HandleRepresentationErrorKind::HandleNotFound,
                message: {
                    #[cfg(feature = "std")]
                    {
                        String::from("Component operation error")
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        error_msg("Component operation error")
                    }
                },
                handle: Some(handle),
            })?;

        // Decrement reference count
        representation.reference_count = representation.reference_count.saturating_sub(1);

        // If reference count reaches zero, actually drop
        if representation.reference_count == 0 {
            self.representations.remove(&handle);
            self.metadata.remove(&handle);

            // Unmap from type registry - note: there is no unmap method,
            // the registry handles cleanup via other means
            // Resource mappings persist until cleanup_instance is called
        }

        Ok(())
    }

    pub fn get_handle_metadata(&self, handle: ResourceHandle) -> Option<&HandleMetadata> {
        self.metadata.get(&handle)
    }

    pub fn update_handle_metadata<F>(
        &mut self,
        handle: ResourceHandle,
        updater: F,
    ) -> HandleRepresentationResult<()>
    where
        F: FnOnce(&mut HandleMetadata),
    {
        let metadata = self.metadata.get_mut(&handle).ok_or_else(|| HandleRepresentationError {
            kind: HandleRepresentationErrorKind::HandleNotFound,
            message: {
                #[cfg(feature = "std")]
                {
                    String::from("Component operation error")
                }
                #[cfg(not(feature = "std"))]
                {
                    error_msg("Component operation error")
                }
            },
            handle: Some(handle),
        })?;

        updater(metadata);
        Ok(())
    }

    pub fn validate_handle_type(
        &self,
        handle: ResourceHandle,
        expected_type: TypeId,
    ) -> HandleRepresentationResult<()> {
        let representation = self.get_representation(handle)?;

        if self.strict_type_checking.load(Ordering::Acquire) {
            // Strict checking - must be exact match or subtype
            if representation.type_id != expected_type {
                // Check if it's a valid subtype
                // Convert u32 type aliases to TypeId newtype structs for type_bounds checker
                let type_id = crate::types::TypeId(representation.type_id);
                let expected_type_id = crate::types::TypeId(expected_type);
                let is_subtype = self.bounds_checker.check_subtype(type_id, expected_type_id);
                if is_subtype != crate::type_bounds::RelationResult::Satisfied {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::TypeMismatch,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                format!(
                                    "Handle type {} does not match expected type {}",
                                    representation.type_id, expected_type
                                )
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Handle type mismatch")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            }
        }

        Ok(())
    }

    fn verify_access(
        &self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        operation: &HandleOperation,
    ) -> HandleRepresentationResult<()> {
        let representation = self.get_representation(handle)?;

        // Check if component owns or has access to the handle
        if representation.component_id != component_id
            && !self.has_shared_access(component_id, handle)
        {
            return Err(HandleRepresentationError {
                kind: HandleRepresentationErrorKind::AccessDenied,
                message: {
                    #[cfg(feature = "std")]
                    {
                        String::from("Component operation error")
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        error_msg("Component operation error")
                    }
                },
                handle: Some(handle),
            })?;
        }

        // Check specific operation permissions
        match operation {
            HandleOperation::Read { .. } => {
                if !representation.access_rights.can_read {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::AccessDenied,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Read access denied")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Read access denied")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            },
            HandleOperation::Write { .. } => {
                if !representation.access_rights.can_write {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::AccessDenied,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Write access denied")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Write access denied")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            },
            HandleOperation::Drop => {
                if !representation.access_rights.can_drop {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::AccessDenied,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Drop access denied")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Drop access denied")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            },
            HandleOperation::Share { .. } => {
                if !representation.access_rights.can_share {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::AccessDenied,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Share access denied")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Share access denied")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            },
            HandleOperation::Borrow { .. } => {
                if !representation.access_rights.can_borrow {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::AccessDenied,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Borrow access denied")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Borrow access denied")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            },
            _ => {},
        }

        // Check access policies
        self.check_access_policies(component_id, handle, operation)?;

        // Check virtualization capabilities if enabled
        if let Some(ref virt_manager) = self.virt_manager {
            self.check_virtualization_capabilities(component_id, operation, virt_manager)?;
        }

        Ok(())
    }

    fn has_shared_access(&self, component_id: ComponentInstanceId, handle: ResourceHandle) -> bool {
        // Check if there's any handle representation for this component that references the same resource
        self.representations.iter().any(|(h, repr)| {
            repr.component_id == component_id
                && repr.type_id
                    == self.get_representation(handle).map(|r| r.type_id).unwrap_or_default()
        })
    }

    fn check_access_policies(
        &self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        operation: &HandleOperation,
    ) -> HandleRepresentationResult<()> {
        let representation = self.get_representation(handle)?;
        let current_time = self.get_current_time();

        for policy in self.access_policies.iter() {
            if policy.component_id == component_id && policy.resource_type == representation.type_id
            {
                // Check expiry
                if let Some(expiry) = policy.expiry {
                    if current_time > expiry {
                        continue;
                    }
                }

                // Check if operation is allowed
                let operation_allowed = policy.allowed_operations.iter().any(|allowed_op| {
                    matches!(
                        (allowed_op, operation),
                        (HandleOperation::Read { .. }, HandleOperation::Read { .. })
                            | (HandleOperation::Write { .. }, HandleOperation::Write { .. })
                            | (HandleOperation::Drop, HandleOperation::Drop)
                            | (HandleOperation::Share { .. }, HandleOperation::Share { .. })
                            | (
                                HandleOperation::Borrow { .. },
                                HandleOperation::Borrow { .. }
                            )
                    )
                });

                if !operation_allowed {
                    return Err(HandleRepresentationError {
                        kind: HandleRepresentationErrorKind::OperationNotSupported,
                        message: {
                            #[cfg(feature = "std")]
                            {
                                String::from("Operation not allowed by policy")
                            }
                            #[cfg(not(feature = "std"))]
                            {
                                error_msg("Operation not allowed by policy")
                            }
                        },
                        handle: Some(handle),
                    })?;
                }
            }
        }

        Ok(())
    }

    fn check_virtualization_capabilities(
        &self,
        component_id: ComponentInstanceId,
        operation: &HandleOperation,
        virt_manager: &VirtualizationManager,
    ) -> HandleRepresentationResult<()> {
        // For certain operations, check if component has required capabilities
        match operation {
            HandleOperation::Call { .. } => {
                // May require specific capability for external calls
                // This is a simplified check - real implementation would be more sophisticated
                Ok(())
            },
            _ => Ok(()),
        }
    }

    fn handle_read_operation(
        &self,
        handle: ResourceHandle,
        fields: &[String],
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // This is a placeholder - actual implementation would read from the resource
        Ok(Some(ComponentValue::S32(42)))
    }

    fn handle_write_operation(
        &mut self,
        handle: ResourceHandle,
        fields: &[(String, ComponentValue<ComponentProvider>)],
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // This is a placeholder - actual implementation would write to the resource
        Ok(None)
    }

    fn handle_call_operation(
        &mut self,
        handle: ResourceHandle,
        method: &str,
        args: &[ComponentValue<ComponentProvider>],
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // This is a placeholder - actual implementation would call the method
        #[cfg(feature = "std")]
        {
            Ok(Some(ComponentValue::String(String::from("success"))))
        }
        #[cfg(not(feature = "std"))]
        {
            // Create a BoundedString with the correct provider type (ComponentProvider)
            // In no_std, convert directly to String (from alloc)
            #[cfg(not(feature = "std"))]
            use alloc::string::ToString;

            Ok(Some(ComponentValue::String("success".to_string())))
        }
    }

    fn handle_drop_operation(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        self.drop_handle(component_id, handle)?;
        Ok(None)
    }

    fn handle_share_operation(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        target_component: ComponentInstanceId,
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        let new_handle = self.share_handle(
            component_id,
            target_component,
            handle,
            AccessRights::read_only(),
        )?;

        // ResourceHandle is u32, so we can use it directly
        Ok(Some(ComponentValue::U32(new_handle)))
    }

    fn handle_borrow_operation(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        mutable: bool,
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // This is a placeholder - actual implementation would create a borrowed reference
        Ok(Some(ComponentValue::Bool(true)))
    }

    fn handle_return_operation(
        &mut self,
        component_id: ComponentInstanceId,
        handle: ResourceHandle,
        from_borrow: bool,
    ) -> HandleRepresentationResult<Option<ComponentValue<ComponentProvider>>> {
        // This is a placeholder - actual implementation would return a borrowed reference
        Ok(None)
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

impl Default for HandleRepresentationManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default HandleRepresentationManager")
    }
}

pub struct TypedHandle<T> {
    handle: ResourceHandle,
    type_id: TypeId,
    _phantom: PhantomData<T>,
}

impl<T> TypedHandle<T> {
    pub fn new(handle: ResourceHandle, type_id: TypeId) -> Self {
        Self {
            handle,
            type_id,
            _phantom: PhantomData,
        }
    }

    pub fn handle(&self) -> ResourceHandle {
        self.handle
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

impl<T> Clone for TypedHandle<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            type_id: self.type_id,
            _phantom: PhantomData,
        }
    }
}

impl<T> Copy for TypedHandle<T> {}

pub fn create_access_rights(
    read: bool,
    write: bool,
    drop: bool,
    share: bool,
    borrow: bool,
) -> AccessRights {
    AccessRights {
        can_read: read,
        can_write: write,
        can_drop: drop,
        can_share: share,
        can_borrow: borrow,
    }
}
