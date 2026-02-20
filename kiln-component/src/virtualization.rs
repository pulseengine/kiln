#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;

// Placeholder types
pub use crate::types::ComponentInstanceId;
use crate::{
    canonical_abi::{canonical_options::CanonicalOptions, post_return::PostReturnRegistry},
    components::component_instantiation::ComponentInstance,
};
pub type ResourceHandle = u32;
pub type ValType = u32;
use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::StaticMap as BoundedMap,
    collections::StaticVec as BoundedVec,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ToBytes},
};

use crate::bounded_component_infra::ComponentProvider;
use crate::prelude::WrtComponentValue;

const MAX_VIRTUAL_COMPONENTS: usize = 256;
const MAX_VIRTUAL_IMPORTS: usize = 1024;
const MAX_VIRTUAL_EXPORTS: usize = 1024;
const MAX_CAPABILITY_GRANTS: usize = 512;
const MAX_VIRTUAL_MEMORY_REGIONS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizationError {
    pub kind: VirtualizationErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VirtualizationErrorKind {
    CapabilityDenied,
    ResourceExhaustion,
    InvalidVirtualComponent,
    MemoryViolation,
    ImportNotFound,
    ExportConflict,
    VirtualizationNotSupported,
}

impl fmt::Display for VirtualizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VirtualizationError {}

// Conversion to wrt_error::Error for unified error handling
impl From<VirtualizationError> for wrt_error::Error {
    fn from(err: VirtualizationError) -> Self {
        use wrt_error::{ErrorCategory, codes};
        match err.kind {
            VirtualizationErrorKind::CapabilityDenied => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_CAPABILITY_DENIED,
                "Virtualization capability denied",
            ),
            VirtualizationErrorKind::ResourceExhaustion => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_RESOURCE_LIFECYCLE_ERROR,
                "Virtualization resource exhausted",
            ),
            VirtualizationErrorKind::InvalidVirtualComponent => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_VIRTUALIZATION_ERROR,
                "Invalid virtual component",
            ),
            VirtualizationErrorKind::MemoryViolation => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_VIRTUALIZATION_ERROR,
                "Virtualization memory violation",
            ),
            VirtualizationErrorKind::ImportNotFound => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                "Virtual import not found",
            ),
            VirtualizationErrorKind::ExportConflict => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                "Virtual export conflict",
            ),
            VirtualizationErrorKind::VirtualizationNotSupported => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_VIRTUALIZATION_ERROR,
                "Virtualization not supported",
            ),
        }
    }
}

pub type VirtualizationResult<T> = wrt_error::Result<T>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    Memory {
        max_size: usize,
    },
    FileSystem {
        read_only: bool,
        path_prefix: Option<String>,
    },
    Network {
        allowed_hosts: BoundedVec<String, 32>,
    },
    Time {
        precision_ms: u64,
    },
    Random,
    Threading {
        max_threads: u32,
    },
    Logging {
        max_level: LogLevel,
    },
    Custom {
        name: String,
        data: BoundedVec<u8, 256>,
    },
}

impl Default for Capability {
    fn default() -> Self {
        Self::Random
    }
}

impl Checksummable for Capability {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::Memory { max_size } => {
                0u8.update_checksum(checksum);
                max_size.update_checksum(checksum);
            },
            Self::FileSystem { .. } => {
                1u8.update_checksum(checksum);
            },
            Self::Network { .. } => {
                2u8.update_checksum(checksum);
            },
            Self::Time { precision_ms } => {
                3u8.update_checksum(checksum);
                precision_ms.update_checksum(checksum);
            },
            Self::Random => {
                4u8.update_checksum(checksum);
            },
            Self::Threading { max_threads } => {
                5u8.update_checksum(checksum);
                max_threads.update_checksum(checksum);
            },
            Self::Logging { .. } => {
                6u8.update_checksum(checksum);
            },
            Self::Custom { .. } => {
                7u8.update_checksum(checksum);
            },
        }
    }
}

impl ToBytes for Capability {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::Random => 0u8.to_bytes_with_provider(writer, provider),
            _ => 1u8.to_bytes_with_provider(writer, provider),
        }
    }
}

impl FromBytes for Capability {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityGrant {
    pub capability: Capability,
    pub granted_to: ComponentInstanceId,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    pub revocable: bool,
}

impl Checksummable for CapabilityGrant {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.capability.update_checksum(checksum);
        self.granted_to.update_checksum(checksum);
        self.granted_at.update_checksum(checksum);
        if let Some(expires_at) = self.expires_at {
            1u8.update_checksum(checksum);
            expires_at.update_checksum(checksum);
        } else {
            0u8.update_checksum(checksum);
        }
        self.revocable.update_checksum(checksum);
    }
}

impl ToBytes for CapabilityGrant {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.capability.to_bytes_with_provider(writer, provider)?;
        self.granted_to.to_bytes_with_provider(writer, provider)?;
        self.granted_at.to_bytes_with_provider(writer, provider)?;
        match self.expires_at {
            Some(expires_at) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                expires_at.to_bytes_with_provider(writer, provider)?;
            },
            None => {
                0u8.to_bytes_with_provider(writer, provider)?;
            },
        }
        self.revocable.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for CapabilityGrant {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let capability = Capability::from_bytes_with_provider(reader, provider)?;
        let granted_to = ComponentInstanceId::from_bytes_with_provider(reader, provider)?;
        let granted_at = u64::from_bytes_with_provider(reader, provider)?;
        let has_expiry = u8::from_bytes_with_provider(reader, provider)?;
        let expires_at = if has_expiry == 1 {
            Some(u64::from_bytes_with_provider(reader, provider)?)
        } else {
            None
        };
        let revocable = bool::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            capability,
            granted_to,
            granted_at,
            expires_at,
            revocable,
        })
    }
}

#[derive(Debug, Clone)]
pub struct VirtualComponent {
    pub instance_id: ComponentInstanceId,
    pub name: String,
    pub parent: Option<ComponentInstanceId>,
    pub children: BoundedVec<ComponentInstanceId, MAX_VIRTUAL_COMPONENTS>,
    pub capabilities: BoundedVec<Capability, MAX_CAPABILITY_GRANTS>,
    pub virtual_imports: BoundedMap<String, VirtualImport, MAX_VIRTUAL_IMPORTS>,
    pub virtual_exports: BoundedMap<String, VirtualExport, MAX_VIRTUAL_EXPORTS>,
    pub memory_regions: BoundedVec<VirtualMemoryRegion, MAX_VIRTUAL_MEMORY_REGIONS>,
    pub isolation_level: IsolationLevel,
    pub resource_limits: ResourceLimits,
    pub is_sandboxed: bool,
}

#[derive(Debug, Clone)]
pub struct VirtualImport {
    pub name: String,
    pub val_type: ValType,
    pub required: bool,
    pub virtual_source: Option<VirtualSource>,
    pub capability_required: Option<Capability>,
}

#[derive(Debug, Clone)]
pub struct VirtualExport {
    pub name: String,
    pub val_type: ValType,
    pub visibility: ExportVisibility,
    pub capability_required: Option<Capability>,
}

#[derive(Debug, Clone)]
pub enum VirtualSource {
    HostFunction {
        name: String,
    },
    ParentComponent {
        export_name: String,
    },
    SiblingComponent {
        instance_id: ComponentInstanceId,
        export_name: String,
    },
    VirtualProvider {
        provider_id: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportVisibility {
    Public,
    Parent,
    Children,
    Siblings,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VirtualMemoryRegion {
    pub start_addr: usize,
    pub size: usize,
    pub permissions: MemoryPermissions,
    pub shared: bool,
    pub mapped_to: Option<ComponentInstanceId>,
}

impl Checksummable for VirtualMemoryRegion {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.start_addr.update_checksum(checksum);
        self.size.update_checksum(checksum);
        self.permissions.update_checksum(checksum);
        self.shared.update_checksum(checksum);
        if let Some(id) = &self.mapped_to {
            id.0.update_checksum(checksum);
        }
    }
}

impl ToBytes for VirtualMemoryRegion {
    fn serialized_size(&self) -> usize {
        let ptr_size = core::mem::size_of::<usize>();
        let base_size = ptr_size * 2 + 4 + 2; // start_addr + size + permissions + shared + has_mapped
        if self.mapped_to.is_some() {
            base_size + 4 // Add space for ComponentInstanceId
        } else {
            base_size
        }
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_usize_le(self.start_addr)?;
        writer.write_usize_le(self.size)?;
        self.permissions.to_bytes_with_provider(writer, provider)?;
        writer.write_u8(self.shared as u8)?;
        writer.write_u8(self.mapped_to.is_some() as u8)?;
        if let Some(id) = &self.mapped_to {
            writer.write_u32_le(id.0)?;
        }
        Ok(())
    }
}

impl FromBytes for VirtualMemoryRegion {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let start_addr = reader.read_usize_le()?;
        let size = reader.read_usize_le()?;
        let permissions = MemoryPermissions::from_bytes_with_provider(reader, provider)?;
        let shared = reader.read_u8()? != 0;
        let has_mapped = reader.read_u8()? != 0;

        let mapped_to = if has_mapped {
            let id = reader.read_u32_le()?;
            Some(ComponentInstanceId(id))
        } else {
            None
        };

        Ok(Self {
            start_addr,
            size,
            permissions,
            shared,
            mapped_to,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Checksummable for MemoryPermissions {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.read.update_checksum(checksum);
        self.write.update_checksum(checksum);
        self.execute.update_checksum(checksum);
    }
}

impl ToBytes for MemoryPermissions {
    fn serialized_size(&self) -> usize {
        4 // 4 bytes for flags
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        let flags = (self.read as u32) | ((self.write as u32) << 1) | ((self.execute as u32) << 2);
        writer.write_u32_le(flags)?;
        Ok(())
    }
}

impl FromBytes for MemoryPermissions {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let flags = reader.read_u32_le()?;
        Ok(Self {
            read: (flags & 1) != 0,
            write: (flags & 2) != 0,
            execute: (flags & 4) != 0,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    #[default]
    None,
    Basic,
    Strong,
    Complete,
}

impl Checksummable for IsolationLevel {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::None => 0u8.update_checksum(checksum),
            Self::Basic => 1u8.update_checksum(checksum),
            Self::Strong => 2u8.update_checksum(checksum),
            Self::Complete => 3u8.update_checksum(checksum),
        }
    }
}

impl ToBytes for IsolationLevel {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let val = match self {
            Self::None => 0u8,
            Self::Basic => 1u8,
            Self::Strong => 2u8,
            Self::Complete => 3u8,
        };
        val.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for IsolationLevel {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let val = u8::from_bytes_with_provider(reader, provider)?;
        Ok(match val {
            0 => Self::None,
            1 => Self::Basic,
            2 => Self::Strong,
            3 => Self::Complete,
            _ => Self::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_memory: usize,
    pub max_cpu_time_ms: u64,
    pub max_file_handles: u32,
    pub max_network_connections: u32,
    pub max_threads: u32,
    pub max_recursive_calls: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 1024 * 1024,
            max_cpu_time_ms: 5000,
            max_file_handles: 10,
            max_network_connections: 5,
            max_threads: 1,
            max_recursive_calls: 100,
        }
    }
}

impl Checksummable for ResourceLimits {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.max_memory.update_checksum(checksum);
        self.max_cpu_time_ms.update_checksum(checksum);
        self.max_file_handles.update_checksum(checksum);
        self.max_network_connections.update_checksum(checksum);
        self.max_threads.update_checksum(checksum);
        self.max_recursive_calls.update_checksum(checksum);
    }
}

impl ToBytes for ResourceLimits {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.max_memory.to_bytes_with_provider(writer, provider)?;
        self.max_cpu_time_ms.to_bytes_with_provider(writer, provider)?;
        self.max_file_handles.to_bytes_with_provider(writer, provider)?;
        self.max_network_connections.to_bytes_with_provider(writer, provider)?;
        self.max_threads.to_bytes_with_provider(writer, provider)?;
        self.max_recursive_calls.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ResourceLimits {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            max_memory: usize::from_bytes_with_provider(reader, provider)?,
            max_cpu_time_ms: u64::from_bytes_with_provider(reader, provider)?,
            max_file_handles: u32::from_bytes_with_provider(reader, provider)?,
            max_network_connections: u32::from_bytes_with_provider(reader, provider)?,
            max_threads: u32::from_bytes_with_provider(reader, provider)?,
            max_recursive_calls: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

#[derive(Debug)]
pub struct VirtualizationManager {
    virtual_components: BoundedMap<ComponentInstanceId, VirtualComponent, MAX_VIRTUAL_COMPONENTS>,
    capability_grants: BoundedVec<CapabilityGrant, MAX_CAPABILITY_GRANTS>,
    host_exports: BoundedMap<String, HostExport, MAX_VIRTUAL_EXPORTS>,
    sandbox_registry: BoundedMap<ComponentInstanceId, SandboxState, MAX_VIRTUAL_COMPONENTS>,
    next_virtual_id: AtomicU32,
    virtualization_enabled: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct HostExport {
    pub name: String,
    pub val_type: ValType,
    pub handler: HostExportHandler,
    pub required_capability: Option<Capability>,
}

#[derive(Debug, Clone)]
pub enum HostExportHandler {
    Memory { allocator: String },
    FileSystem { base_path: String },
    Network { endpoint: String },
    Time,
    Random,
    Logging { destination: String },
    Custom { handler_id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxState {
    pub instance_id: ComponentInstanceId,
    pub active: bool,
    pub resource_usage: ResourceUsage,
    pub violation_count: u32,
    pub last_violation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceUsage {
    pub memory_used: usize,
    pub cpu_time_used_ms: u64,
    pub file_handles_used: u32,
    pub network_connections_used: u32,
    pub threads_used: u32,
    pub recursive_calls_depth: u32,
}

impl Checksummable for ResourceUsage {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.memory_used.update_checksum(checksum);
        self.cpu_time_used_ms.update_checksum(checksum);
        self.file_handles_used.update_checksum(checksum);
        self.network_connections_used.update_checksum(checksum);
        self.threads_used.update_checksum(checksum);
        self.recursive_calls_depth.update_checksum(checksum);
    }
}

impl ToBytes for ResourceUsage {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.memory_used.to_bytes_with_provider(writer, provider)?;
        self.cpu_time_used_ms.to_bytes_with_provider(writer, provider)?;
        self.file_handles_used.to_bytes_with_provider(writer, provider)?;
        self.network_connections_used.to_bytes_with_provider(writer, provider)?;
        self.threads_used.to_bytes_with_provider(writer, provider)?;
        self.recursive_calls_depth.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ResourceUsage {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            memory_used: usize::from_bytes_with_provider(reader, provider)?,
            cpu_time_used_ms: u64::from_bytes_with_provider(reader, provider)?,
            file_handles_used: u32::from_bytes_with_provider(reader, provider)?,
            network_connections_used: u32::from_bytes_with_provider(reader, provider)?,
            threads_used: u32::from_bytes_with_provider(reader, provider)?,
            recursive_calls_depth: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

impl VirtualizationManager {
    pub fn new() -> VirtualizationResult<Self> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let capability_grants = BoundedVec::new();

        Ok(Self {
            virtual_components: BoundedMap::new(),
            capability_grants,
            host_exports: BoundedMap::new(),
            sandbox_registry: BoundedMap::new(),
            next_virtual_id: AtomicU32::new(1000),
            virtualization_enabled: AtomicBool::new(true),
        })
    }

    pub fn enable_virtualization(&self) {
        self.virtualization_enabled.store(true, Ordering::SeqCst);
    }

    pub fn disable_virtualization(&self) {
        self.virtualization_enabled.store(false, Ordering::SeqCst);
    }

    pub fn is_virtualization_enabled(&self) -> bool {
        self.virtualization_enabled.load(Ordering::SeqCst)
    }

    pub fn create_virtual_component(
        &mut self,
        name: &str,
        parent: Option<ComponentInstanceId>,
        isolation_level: IsolationLevel,
    ) -> VirtualizationResult<ComponentInstanceId> {
        if !self.is_virtualization_enabled() {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::VirtualizationNotSupported,
                message: String::from("Virtualization is disabled"),
            }
            .into());
        }

        let instance_id =
            ComponentInstanceId::new(self.next_virtual_id.fetch_add(1, Ordering::SeqCst));

        let virtual_component = VirtualComponent {
            instance_id,
            name: String::from(name),
            parent,
            children: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            capabilities: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            virtual_imports: BoundedMap::new(),
            virtual_exports: BoundedMap::new(),
            memory_regions: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            isolation_level,
            resource_limits: ResourceLimits::default(),
            is_sandboxed: isolation_level != IsolationLevel::None,
        };

        if let Some(parent_id) = parent {
            if let Some(parent_component) = self.virtual_components.get_mut(&parent_id) {
                parent_component.children.push(instance_id).map_err(|_| VirtualizationError {
                    kind: VirtualizationErrorKind::ResourceExhaustion,
                    message: String::from("Parent component has too many children"),
                })?;
            }
        }

        self.virtual_components.insert(instance_id, virtual_component).map_err(|_| {
            VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Too many virtual components"),
            }
        })?;

        if isolation_level != IsolationLevel::None {
            let sandbox_state = SandboxState {
                instance_id,
                active: true,
                resource_usage: ResourceUsage::default(),
                violation_count: 0,
                last_violation: None,
            };
            self.sandbox_registry.insert(instance_id, sandbox_state).map_err(|_| {
                VirtualizationError {
                    kind: VirtualizationErrorKind::ResourceExhaustion,
                    message: String::from("Too many sandboxed components"),
                }
            })?;
        }

        Ok(instance_id)
    }

    pub fn grant_capability(
        &mut self,
        instance_id: ComponentInstanceId,
        capability: Capability,
        expires_at: Option<u64>,
        revocable: bool,
    ) -> VirtualizationResult<()> {
        if !self.virtual_components.contains_key(&instance_id) {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::InvalidVirtualComponent,
                message: String::from("Component not found"),
            }
            .into());
        }

        let grant = CapabilityGrant {
            capability: capability.clone(),
            granted_to: instance_id,
            granted_at: self.get_current_time(),
            expires_at,
            revocable,
        };

        self.capability_grants.push(grant).map_err(|_| VirtualizationError {
            kind: VirtualizationErrorKind::ResourceExhaustion,
            message: String::from("Too many capability grants"),
        })?;

        if let Some(component) = self.virtual_components.get_mut(&instance_id) {
            component.capabilities.push(capability).map_err(|_| VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Component has too many capabilities"),
            })?;
        }

        Ok(())
    }

    pub fn check_capability(
        &self,
        instance_id: ComponentInstanceId,
        capability: &Capability,
    ) -> bool {
        if let Some(component) = self.virtual_components.get(&instance_id) {
            component
                .capabilities
                .iter()
                .any(|cap| self.capability_matches(cap, capability))
        } else {
            false
        }
    }

    pub fn add_virtual_import(
        &mut self,
        instance_id: ComponentInstanceId,
        import: VirtualImport,
    ) -> VirtualizationResult<()> {
        let component =
            self.virtual_components
                .get_mut(&instance_id)
                .ok_or_else(|| VirtualizationError {
                    kind: VirtualizationErrorKind::InvalidVirtualComponent,
                    message: String::from("Component not found"),
                })?;

        let import_name = import.name.clone();
        component
            .virtual_imports
            .insert(import_name, import)
            .map_err(|_| VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Too many virtual imports"),
            })?;

        Ok(())
    }

    pub fn add_virtual_export(
        &mut self,
        instance_id: ComponentInstanceId,
        export: VirtualExport,
    ) -> VirtualizationResult<()> {
        let component =
            self.virtual_components
                .get_mut(&instance_id)
                .ok_or_else(|| VirtualizationError {
                    kind: VirtualizationErrorKind::InvalidVirtualComponent,
                    message: String::from("Component not found"),
                })?;

        let export_name = export.name.clone();
        component
            .virtual_exports
            .insert(export_name, export)
            .map_err(|_| VirtualizationError {
                kind: VirtualizationErrorKind::ExportConflict,
                message: String::from("Export already exists or too many exports"),
            })?;

        Ok(())
    }

    pub fn allocate_virtual_memory(
        &mut self,
        instance_id: ComponentInstanceId,
        size: usize,
        permissions: MemoryPermissions,
    ) -> VirtualizationResult<usize> {
        // Extract isolation level and limits before mutable borrow
        let (isolation_level, max_memory, current_usage) = {
            let component =
                self.virtual_components.get(&instance_id).ok_or_else(|| VirtualizationError {
                    kind: VirtualizationErrorKind::InvalidVirtualComponent,
                    message: String::from("Component not found"),
                })?;

            let current_usage =
                component.memory_regions.iter().map(|region| region.size).sum::<usize>();
            (
                component.isolation_level,
                component.resource_limits.max_memory,
                current_usage,
            )
        };

        if isolation_level == IsolationLevel::None {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::MemoryViolation,
                message: String::from("Virtual memory not available for non-isolated components"),
            }
            .into());
        }

        if !self.check_capability(instance_id, &Capability::Memory { max_size: size }) {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::CapabilityDenied,
                message: String::from("Insufficient memory capability"),
            }
            .into());
        }

        if current_usage + size > max_memory {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Memory limit exceeded"),
            }
            .into());
        }

        let start_addr = self.find_virtual_address_space(size)?;

        let memory_region = VirtualMemoryRegion {
            start_addr,
            size,
            permissions,
            shared: false,
            mapped_to: None,
        };

        // Now safely get mutable borrow
        let component =
            self.virtual_components
                .get_mut(&instance_id)
                .ok_or_else(|| VirtualizationError {
                    kind: VirtualizationErrorKind::InvalidVirtualComponent,
                    message: String::from("Component not found"),
                })?;

        component.memory_regions.push(memory_region).map_err(|_| VirtualizationError {
            kind: VirtualizationErrorKind::ResourceExhaustion,
            message: String::from("Too many memory regions"),
        })?;

        Ok(start_addr)
    }

    pub fn resolve_virtual_import(
        &self,
        instance_id: ComponentInstanceId,
        import_name: &str,
    ) -> VirtualizationResult<Option<WrtComponentValue<ComponentProvider>>> {
        let component =
            self.virtual_components.get(&instance_id).ok_or_else(|| VirtualizationError {
                kind: VirtualizationErrorKind::InvalidVirtualComponent,
                message: String::from("Component not found"),
            })?;

        let import_name_string = String::from(import_name);
        let import = component.virtual_imports.get(&import_name_string).ok_or_else(|| {
            wrt_error::Error::from(VirtualizationError {
                kind: VirtualizationErrorKind::ImportNotFound,
                message: String::from("Component not found"),
            })
        })?;

        if let Some(ref capability) = import.capability_required {
            if !self.check_capability(instance_id, capability) {
                return Err(VirtualizationError {
                    kind: VirtualizationErrorKind::CapabilityDenied,
                    message: String::from("Component not found"),
                }
                .into());
            }
        }

        match &import.virtual_source {
            Some(VirtualSource::HostFunction { name }) => self.resolve_host_function(name),
            Some(VirtualSource::ParentComponent { export_name }) => {
                if let Some(parent_id) = component.parent {
                    self.resolve_parent_export(parent_id, export_name)
                } else {
                    Ok(None)
                }
            },
            Some(VirtualSource::SiblingComponent {
                instance_id: sibling_id,
                export_name,
            }) => self.resolve_sibling_export(*sibling_id, export_name),
            Some(VirtualSource::VirtualProvider { provider_id }) => {
                self.resolve_virtual_provider(provider_id)
            },
            None => Ok(None),
        }
    }

    pub fn update_resource_usage(
        &mut self,
        instance_id: ComponentInstanceId,
        usage_update: ResourceUsage,
    ) -> VirtualizationResult<()> {
        // Update sandbox state first
        if let Some(sandbox_state) = self.sandbox_registry.get_mut(&instance_id) {
            sandbox_state.resource_usage = usage_update.clone();
        }

        // Check limits separately to avoid borrow conflict
        if let Some(component) = self.virtual_components.get(&instance_id) {
            self.check_resource_limits(component, &usage_update)?;
        }

        Ok(())
    }

    fn capability_matches(&self, granted: &Capability, requested: &Capability) -> bool {
        match (granted, requested) {
            (
                Capability::Memory {
                    max_size: granted_size,
                },
                Capability::Memory {
                    max_size: requested_size,
                },
            ) => granted_size >= requested_size,
            (
                Capability::Threading {
                    max_threads: granted,
                },
                Capability::Threading {
                    max_threads: requested,
                },
            ) => granted >= requested,
            (Capability::Random, Capability::Random) => true,
            (Capability::Time { .. }, Capability::Time { .. }) => true,
            (a, b) => a == b,
        }
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

    fn find_virtual_address_space(&self, size: usize) -> VirtualizationResult<usize> {
        let base_addr = 0x10000000;
        Ok(base_addr)
    }

    fn resolve_host_function(
        &self,
        name: &str,
    ) -> VirtualizationResult<Option<WrtComponentValue<ComponentProvider>>> {
        let name_string = String::from(name);
        if let Some(export) = self.host_exports.get(&name_string) {
            match &export.handler {
                HostExportHandler::Memory { .. } => {
                    Ok(Some(WrtComponentValue::<ComponentProvider>::U32(0)))
                },
                HostExportHandler::Time => Ok(Some(WrtComponentValue::<ComponentProvider>::U64(
                    self.get_current_time(),
                ))),
                HostExportHandler::Random => {
                    Ok(Some(WrtComponentValue::<ComponentProvider>::U32(42)))
                },
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn resolve_parent_export(
        &self,
        parent_id: ComponentInstanceId,
        export_name: &str,
    ) -> VirtualizationResult<Option<WrtComponentValue<ComponentProvider>>> {
        if let Some(parent) = self.virtual_components.get(&parent_id) {
            let export_name_string = String::from(export_name);
            if let Some(export) = parent.virtual_exports.get(&export_name_string) {
                match export.visibility {
                    ExportVisibility::Public | ExportVisibility::Children => Ok(None),
                    _ => Err(VirtualizationError {
                        kind: VirtualizationErrorKind::CapabilityDenied,
                        message: String::from("Export not visible to children"),
                    }
                    .into()),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn resolve_sibling_export(
        &self,
        sibling_id: ComponentInstanceId,
        export_name: &str,
    ) -> VirtualizationResult<Option<WrtComponentValue<ComponentProvider>>> {
        if let Some(sibling) = self.virtual_components.get(&sibling_id) {
            let export_name_string = String::from(export_name);
            if let Some(export) = sibling.virtual_exports.get(&export_name_string) {
                match export.visibility {
                    ExportVisibility::Public | ExportVisibility::Siblings => Ok(None),
                    _ => Err(VirtualizationError {
                        kind: VirtualizationErrorKind::CapabilityDenied,
                        message: String::from("Export not visible to siblings"),
                    }
                    .into()),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn resolve_virtual_provider(
        &self,
        provider_id: &str,
    ) -> VirtualizationResult<Option<WrtComponentValue<ComponentProvider>>> {
        Ok(None)
    }

    fn check_resource_limits(
        &self,
        component: &VirtualComponent,
        usage: &ResourceUsage,
    ) -> VirtualizationResult<()> {
        let limits = &component.resource_limits;

        if usage.memory_used > limits.max_memory {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Memory limit exceeded"),
            }
            .into());
        }

        if usage.cpu_time_used_ms > limits.max_cpu_time_ms {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("CPU time limit exceeded"),
            }
            .into());
        }

        if usage.threads_used > limits.max_threads {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Thread limit exceeded"),
            }
            .into());
        }

        if usage.recursive_calls_depth > limits.max_recursive_calls {
            return Err(VirtualizationError {
                kind: VirtualizationErrorKind::ResourceExhaustion,
                message: String::from("Recursion limit exceeded"),
            }
            .into());
        }

        Ok(())
    }
}

impl Default for VirtualizationManager {
    fn default() -> Self {
        // Use new() which properly handles allocation or panic in development
        Self::new()
            .expect("VirtualizationManager allocation should not fail in default construction")
    }
}

pub fn create_memory_capability(max_size: usize) -> Capability {
    Capability::Memory { max_size }
}

pub fn create_network_capability(allowed_hosts: &[&str]) -> VirtualizationResult<Capability> {
    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
    let mut hosts = BoundedVec::new();
    for host in allowed_hosts {
        hosts.push(String::from(*host)).map_err(|_| VirtualizationError {
            kind: VirtualizationErrorKind::ResourceExhaustion,
            message: String::from("Too many allowed hosts"),
        })?;
    }
    Ok(Capability::Network {
        allowed_hosts: hosts,
    })
}

pub fn create_threading_capability(max_threads: u32) -> Capability {
    Capability::Threading { max_threads }
}
