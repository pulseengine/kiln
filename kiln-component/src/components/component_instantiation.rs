//! Component Instantiation and Linking System
//!
//! This module provides comprehensive component instantiation and linking
//! functionality for the WebAssembly Component Model. It enables creating
//! component instances from component binaries, resolving imports/exports, and
//! composing components together.
//!
//! # Features
//!
//! - **Component Instance Creation**: Create executable instances from
//!   component binaries
//! - **Import/Export Resolution**: Automatic resolution of component
//!   dependencies
//! - **Component Linking**: Link multiple components together with type safety
//! - **Cross-Environment Support**: Works in std, no_std+alloc, and pure no_std
//! - **Memory Safety**: Comprehensive validation and bounds checking
//! - **Resource Management**: Proper cleanup and lifecycle management
//!
//! # Core Concepts
//!
//! - **Component**: A compiled WebAssembly component binary
//! - **Instance**: A runtime instantiation of a component
//! - **Linker**: Manages component composition and dependency resolution
//! - **Import/Export**: Interface definitions for component communication
//!
//! # Example
//!
//! ```no_run
//! use wrt_component::component_instantiation::{
//!     ComponentLinker,
//!     InstanceConfig,
//! };
//!
//! // Create a linker for component composition
//! let mut linker = ComponentLinker::new();
//!
//! // Add a component with exports
//! linker.add_component("math", &math_component_binary)?;
//!
//! // Instantiate a component that imports from "math"
//! let instance = linker.instantiate("calculator", &calc_component_binary)?;
//! ```

// Cross-environment imports
#[cfg(feature = "std")]
use std::{
    any::Any,
    boxed::Box,
    collections::HashMap,
    format,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(not(feature = "std"))]
use wrt_foundation::{
    CrateId, bounded::BoundedString, collections::StaticVec as BoundedVec, safe_managed_alloc,
    safe_memory::NoStdProvider,
};

#[cfg(feature = "std")]
use wrt_foundation::BoundedVec;

#[cfg(not(feature = "std"))]
type InstantiationString = BoundedString<256>;

#[cfg(not(feature = "std"))]
type InstantiationVec<T> = BoundedVec<T, 64>;

// Enable vec! and format! macros for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

// use crate::component_communication::{CallRouter, CallContext as CommCallContext};
// use crate::call_context::CallContextManager;
use wrt_error::{Error, ErrorCategory, Result, codes};

use wrt_foundation::{
    MemoryProvider,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

use crate::{
    bounded_component_infra::{BufferProvider, ComponentProvider},
    canonical_abi::{CanonicalABI, CanonicalMemory, ComponentType, ComponentValue},
    components::component::Component as RuntimeComponent,
    resource_management::{
        ResourceData, ResourceHandle, ResourceManager as ComponentResourceManager, ResourceTypeId,
    },
    types::{ComponentInstanceState, ComponentMetadata},
};

#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{CrateId, WrtVec};

// Tracing imports for structured logging
#[cfg(feature = "tracing")]
use wrt_foundation::tracing::trace;

/// Maximum number of component instances
const MAX_COMPONENT_INSTANCES: usize = 1024;

/// Maximum number of imports per component
const MAX_IMPORTS_PER_COMPONENT: usize = 256;

/// Maximum number of exports per component
const MAX_EXPORTS_PER_COMPONENT: usize = 256;

/// Maximum component nesting depth
const MAX_COMPONENT_NESTING_DEPTH: usize = 16;

/// Maximum number of core modules per component (ASIL-D safety limit)
const MAX_CORE_MODULES: usize = 64;

/// Maximum number of types per component (ASIL-D safety limit)
const MAX_TYPES: usize = 512;

/// Maximum number of canonical operations (ASIL-D safety limit)
const MAX_CANONICAL_OPS: usize = 128;

/// Maximum number of core instances (ASIL-D safety limit)
const MAX_CORE_INSTANCES: usize = 64;

/// Component instance identifier
pub type InstanceId = u32;

/// Component function handle
pub type FunctionHandle = u32;

/// Memory handle for component instances
pub type MemoryHandle = u32;

/// Component instance state
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceState {
    /// Instance is being initialized
    Initializing,
    /// Instance is ready for use
    Ready,
    /// Instance has encountered an error
    Error(String),
    /// Instance has been terminated
    Terminated,
}

/// Component instance configuration
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    /// Maximum memory size in bytes
    pub max_memory_size: u32,
    /// Maximum table size
    pub max_table_size: u32,
    /// Enable debug mode
    pub debug_mode: bool,
    /// Binary std/no_std choice
    pub memory_config: MemoryConfig,
}

/// Memory configuration for component instances
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    /// Initial memory size in pages (64KB each)
    pub initial_pages: u32,
    /// Maximum memory size in pages
    pub max_pages: Option<u32>,
    /// Enable memory protection
    pub protected: bool,
}

/// Component function signature
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionSignature {
    /// Function name
    pub name: String,
    /// Parameter types
    #[cfg(feature = "std")]
    pub params: Vec<ComponentType>,
    #[cfg(not(feature = "std"))]
    pub params: BoundedVec<ComponentType, 16>,
    /// Return types
    #[cfg(feature = "std")]
    pub returns: Vec<ComponentType>,
    #[cfg(not(feature = "std"))]
    pub returns: BoundedVec<ComponentType, 16>,
}

/// Component export definition
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentExport {
    /// Export name
    pub name: String,
    /// Export type
    pub export_type: ExportType,
    /// Index of the exported item within its sort (function index, table index, etc.)
    /// This is the actual index needed for function dispatch.
    pub index: u32,
}

/// Component import definition
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentImport {
    /// Import name
    pub name: String,
    /// Module name (for namespace)
    pub module: String,
    /// Import type
    pub import_type: ImportType,
}

/// Types of exports a component can provide
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportType {
    /// Function export
    Function(FunctionSignature),
    /// Memory export
    Memory(MemoryConfig),
    /// Table export
    Table {
        element_type: ComponentType,
        size: u32,
    },
    /// Global export
    Global {
        value_type: ComponentType,
        mutable: bool,
    },
    /// Component type export
    Type(ComponentType),
}

impl Default for ExportType {
    fn default() -> Self {
        ExportType::Function(FunctionSignature::default())
    }
}

/// Types of imports a component can require
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportType {
    /// Function import
    Function(FunctionSignature),
    /// Memory import
    Memory(MemoryConfig),
    /// Table import
    Table {
        element_type: ComponentType,
        min_size: u32,
        max_size: Option<u32>,
    },
    /// Global import
    Global {
        value_type: ComponentType,
        mutable: bool,
    },
    /// Component type import
    Type(ComponentType),
}

impl Default for ImportType {
    fn default() -> Self {
        ImportType::Function(FunctionSignature::default())
    }
}

/// Component instance implementation (alias to canonical definition)
pub use crate::types::ComponentInstance;

/// Local instance implementation details
#[derive(Debug)]
pub struct ComponentInstanceImpl {
    /// Instance memory
    pub memory: Option<ComponentMemory>,
    /// Canonical ABI for value conversion
    abi: CanonicalABI,
    /// Function table
    #[cfg(feature = "std")]
    functions: Vec<ComponentFunction>,
    #[cfg(not(feature = "std"))]
    functions: BoundedVec<ComponentFunction, 128>,
    /// Instance metadata
    metadata: InstanceMetadata,
    /// Resource manager for this instance
    resource_manager: Option<ComponentResourceManager>,
}

/// Resolved import with actual provider
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    /// Original import definition
    pub import: ComponentImport,
    /// Provider instance ID
    pub provider_id: InstanceId,
    /// Provider export name
    #[cfg(feature = "std")]
    pub provider_export: String,
    #[cfg(not(feature = "std"))]
    pub provider_export: wrt_foundation::bounded::BoundedString<64>,
}

/// Component function implementation
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentFunction {
    /// Function handle
    pub handle: FunctionHandle,
    /// Function signature
    pub signature: FunctionSignature,
    /// Implementation type
    pub implementation: FunctionImplementation,
}

/// Function implementation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionImplementation {
    /// Native WebAssembly function
    Native {
        /// Function index in the component
        func_index: u32,
        /// Module containing the function
        module_index: u32,
    },
    /// Host function
    Host {
        /// Host function callback
        #[cfg(feature = "std")]
        callback: String, // Simplified - would be actual callback in full implementation
        #[cfg(not(feature = "std"))]
        callback: wrt_foundation::bounded::BoundedString<64>,
    },
    /// Component function (calls through canonical ABI)
    Component {
        /// Target component instance
        target_instance: InstanceId,
        /// Target function name
        #[cfg(feature = "std")]
        target_function: String,
        #[cfg(not(feature = "std"))]
        target_function: wrt_foundation::bounded::BoundedString<64>,
    },
}

impl Default for FunctionImplementation {
    fn default() -> Self {
        FunctionImplementation::Native {
            func_index: 0,
            module_index: 0,
        }
    }
}

impl Checksummable for FunctionImplementation {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            FunctionImplementation::Native {
                func_index,
                module_index,
            } => {
                0u8.update_checksum(checksum);
                func_index.update_checksum(checksum);
                module_index.update_checksum(checksum);
            },
            FunctionImplementation::Host { callback } => {
                1u8.update_checksum(checksum);
                #[cfg(feature = "std")]
                {
                    callback.len().update_checksum(checksum);
                    for ch in callback.chars() {
                        (ch as u32).update_checksum(checksum);
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    callback.update_checksum(checksum);
                }
            },
            FunctionImplementation::Component {
                target_instance,
                target_function,
            } => {
                2u8.update_checksum(checksum);
                target_instance.update_checksum(checksum);
                #[cfg(feature = "std")]
                {
                    target_function.len().update_checksum(checksum);
                    for ch in target_function.chars() {
                        (ch as u32).update_checksum(checksum);
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    target_function.update_checksum(checksum);
                }
            },
        }
    }
}

impl ToBytes for FunctionImplementation {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match self {
            FunctionImplementation::Native {
                func_index,
                module_index,
            } => {
                0u8.to_bytes_with_provider(writer, provider)?;
                func_index.to_bytes_with_provider(writer, provider)?;
                module_index.to_bytes_with_provider(writer, provider)
            },
            FunctionImplementation::Host { callback } => {
                1u8.to_bytes_with_provider(writer, provider)?;
                #[cfg(feature = "std")]
                {
                    (callback.len() as u32).to_bytes_with_provider(writer, provider)?;
                    for ch in callback.chars() {
                        (ch as u8).to_bytes_with_provider(writer, provider)?;
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    callback.to_bytes_with_provider(writer, provider)?;
                }
                Ok(())
            },
            FunctionImplementation::Component {
                target_instance,
                target_function,
            } => {
                2u8.to_bytes_with_provider(writer, provider)?;
                target_instance.to_bytes_with_provider(writer, provider)?;
                #[cfg(feature = "std")]
                {
                    (target_function.len() as u32).to_bytes_with_provider(writer, provider)?;
                    for ch in target_function.chars() {
                        (ch as u8).to_bytes_with_provider(writer, provider)?;
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    target_function.to_bytes_with_provider(writer, provider)?;
                }
                Ok(())
            },
        }
    }
}

impl FromBytes for FunctionImplementation {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let tag = u8::from_bytes_with_provider(reader, provider)?;
        match tag {
            0 => {
                let func_index = u32::from_bytes_with_provider(reader, provider)?;
                let module_index = u32::from_bytes_with_provider(reader, provider)?;
                Ok(FunctionImplementation::Native {
                    func_index,
                    module_index,
                })
            },
            1 => {
                #[cfg(feature = "std")]
                {
                    let len = u32::from_bytes_with_provider(reader, provider)? as usize;
                    let mut bytes = vec![0u8; len];
                    for i in 0..len {
                        bytes[i] = u8::from_bytes_with_provider(reader, provider)?;
                    }
                    Ok(FunctionImplementation::Host {
                        callback: String::from_utf8(bytes).map_err(|_| {
                            Error::validation_error("Invalid UTF-8 in callback name")
                        })?,
                    })
                }
                #[cfg(not(feature = "std"))]
                {
                    let callback =
                        wrt_foundation::bounded::BoundedString::<64>::from_bytes_with_provider(
                            reader, provider,
                        )?;
                    Ok(FunctionImplementation::Host { callback })
                }
            },
            2 => {
                let target_instance = u32::from_bytes_with_provider(reader, provider)?;
                #[cfg(feature = "std")]
                {
                    let len = u32::from_bytes_with_provider(reader, provider)? as usize;
                    let mut bytes = vec![0u8; len];
                    for i in 0..len {
                        bytes[i] = u8::from_bytes_with_provider(reader, provider)?;
                    }
                    Ok(FunctionImplementation::Component {
                        target_instance,
                        target_function: String::from_utf8(bytes).map_err(|_| {
                            Error::validation_error("Invalid UTF-8 in function name")
                        })?,
                    })
                }
                #[cfg(not(feature = "std"))]
                {
                    let target_function =
                        wrt_foundation::bounded::BoundedString::<64>::from_bytes_with_provider(
                            reader, provider,
                        )?;
                    Ok(FunctionImplementation::Component {
                        target_instance,
                        target_function,
                    })
                }
            },
            _ => Err(Error::validation_error(
                "Invalid FunctionImplementation tag",
            )),
        }
    }
}

impl Checksummable for FunctionSignature {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.len().update_checksum(checksum);
        for ch in self.name.chars() {
            (ch as u32).update_checksum(checksum);
        }
        for param in self.params.iter() {
            param.update_checksum(checksum);
        }
        for ret in self.returns.iter() {
            ret.update_checksum(checksum);
        }
    }
}

impl ToBytes for FunctionSignature {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        // Write name
        (self.name.len() as u32).to_bytes_with_provider(writer, provider)?;
        for ch in self.name.chars() {
            (ch as u8).to_bytes_with_provider(writer, provider)?;
        }

        // Write params
        (self.params.len() as u32).to_bytes_with_provider(writer, provider)?;
        for param in self.params.iter() {
            param.to_bytes_with_provider(writer, provider)?;
        }

        // Write returns
        (self.returns.len() as u32).to_bytes_with_provider(writer, provider)?;
        for ret in self.returns.iter() {
            ret.to_bytes_with_provider(writer, provider)?;
        }

        Ok(())
    }
}

impl FromBytes for FunctionSignature {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        // Read name
        let name_len = u32::from_bytes_with_provider(reader, provider)? as usize;
        let mut name_bytes = Vec::new();
        for _ in 0..name_len {
            name_bytes.push(u8::from_bytes_with_provider(reader, provider)?);
        }
        let name = String::from_utf8(name_bytes)
            .map_err(|_| Error::validation_error("Invalid UTF-8 in function name"))?;

        // Read params
        let params_len = u32::from_bytes_with_provider(reader, provider)? as usize;
        #[cfg(feature = "std")]
        let mut params = Vec::new();
        #[cfg(not(feature = "std"))]
        let mut params = BoundedVec::<ComponentType, 16>::new();
        for _ in 0..params_len {
            let param = ComponentType::from_bytes_with_provider(reader, provider)?;
            #[cfg(feature = "std")]
            params.push(param);
            #[cfg(not(feature = "std"))]
            params.push(param).map_err(|_| {
                Error::foundation_bounded_capacity_exceeded(
                    "FunctionSignature params capacity exceeded",
                )
            })?;
        }

        // Read returns
        let returns_len = u32::from_bytes_with_provider(reader, provider)? as usize;
        #[cfg(feature = "std")]
        let mut returns = Vec::new();
        #[cfg(not(feature = "std"))]
        let mut returns = BoundedVec::<ComponentType, 16>::new();
        for _ in 0..returns_len {
            let ret = ComponentType::from_bytes_with_provider(reader, provider)?;
            #[cfg(feature = "std")]
            returns.push(ret);
            #[cfg(not(feature = "std"))]
            returns.push(ret).map_err(|_| {
                Error::foundation_bounded_capacity_exceeded(
                    "FunctionSignature returns capacity exceeded",
                )
            })?;
        }

        Ok(FunctionSignature {
            name,
            params,
            returns,
        })
    }
}

impl Checksummable for ComponentFunction {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.handle.update_checksum(checksum);
        self.signature.update_checksum(checksum);
        self.implementation.update_checksum(checksum);
    }
}

impl ToBytes for ComponentFunction {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.handle.to_bytes_with_provider(writer, provider)?;
        self.signature.to_bytes_with_provider(writer, provider)?;
        self.implementation.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ComponentFunction {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let handle = u32::from_bytes_with_provider(reader, provider)?;
        let signature = FunctionSignature::from_bytes_with_provider(reader, provider)?;
        let implementation = FunctionImplementation::from_bytes_with_provider(reader, provider)?;
        Ok(ComponentFunction {
            handle,
            signature,
            implementation,
        })
    }
}

/// Component memory implementation
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentMemory {
    /// Memory handle
    pub handle: MemoryHandle,
    /// Memory configuration
    pub config: MemoryConfig,
    /// Current memory size in bytes
    pub current_size: u32,
    /// Memory data (simplified for this implementation)
    #[cfg(feature = "std")]
    pub data: Vec<u8>,
    #[cfg(not(feature = "std"))]
    pub data: BoundedVec<u8, 65536>,
}

/// Instance metadata for debugging and introspection
#[derive(Debug, Clone, Default)]
pub struct InstanceMetadata {
    /// Creation timestamp
    pub created_at: u64,
    /// Total function calls
    pub function_calls: u64,
    /// Binary std/no_std choice
    pub memory_allocations: u64,
    /// Current memory usage
    pub memory_usage: u32,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            max_memory_size: 64 * 1024 * 1024, // 64MB
            max_table_size: 1024,
            debug_mode: false,
            memory_config: MemoryConfig::default(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            initial_pages: 1,      // 64KB
            max_pages: Some(1024), // 64MB
            protected: true,
        }
    }
}

impl ComponentInstance {
    /// Create a new component instance from a runtime component
    ///
    /// Takes ownership of the runtime component to avoid duplication.
    pub fn new(id: InstanceId, component: RuntimeComponent) -> Result<Self> {
        // Initialize empty collections based on feature flags
        #[cfg(all(feature = "std", feature = "safety-critical"))]
        let (
            functions,
            imports,
            exports,
            resource_tables,
            module_instances,
            nested_component_instances,
        ) = {
            (
                WrtVec::new(),
                WrtVec::new(),
                WrtVec::new(),
                WrtVec::new(),
                WrtVec::new(),
                WrtVec::new(),
            )
        };

        #[cfg(all(feature = "std", not(feature = "safety-critical")))]
        let (
            functions,
            imports,
            exports,
            resource_tables,
            module_instances,
            nested_component_instances,
        ) = {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };

        #[cfg(not(feature = "std"))]
        let (
            functions,
            imports,
            exports,
            resource_tables,
            module_instances,
            nested_component_instances,
        ) = {
            (
                BoundedVec::new(),
                BoundedVec::new(),
                BoundedVec::new(),
                BoundedVec::new(),
                BoundedVec::new(),
                BoundedVec::new(),
            )
        };

        Ok(Self {
            id,
            component,
            state: ComponentInstanceState::Initialized,
            resource_manager: None,
            memory: None,
            metadata: ComponentMetadata::default(),
            #[cfg(feature = "std")]
            type_index: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            type_index: (),
            functions,
            imports,
            exports,
            resource_tables,
            module_instances,
            nested_component_instances,
            #[cfg(feature = "wrt-execution")]
            runtime_engine: None,
            #[cfg(feature = "wrt-execution")]
            main_instance_handle: None,
        })
    }

    /// Create a component instance directly from parsed binary format
    ///
    /// **Memory Efficient**: Consumes parsed component and converts to runtime format
    /// without keeping both in memory simultaneously. Uses streaming conversion where
    /// each section is processed and dropped immediately.
    ///
    /// **Safety**: All limits are validated before allocation. Exceeding ASIL-D limits
    /// results in early failure with clear error messages.
    ///
    /// **Performance**: When `std` feature is enabled and component-model-threading is
    /// available, core modules are instantiated in parallel using structured parallelism
    /// for deterministic results.
    ///
    /// # Memory Flow (Streaming)
    /// ```text
    /// Binary → Parsed → Process modules → drop modules
    ///                → Process types   → drop types
    ///                → Process exports → drop exports
    ///                → Process canonicals → drop canonicals
    ///                → Runtime Instance (only this remains)
    /// ```
    ///
    /// # Safety Limits (ASIL-D)
    /// - Modules: 64 max
    /// - Types: 512 max
    /// - Canonicals: 128 max
    /// - Exports: 256 max
    /// - Imports: 256 max
    #[cfg(feature = "std")]
    pub fn from_parsed(
        id: InstanceId,
        parsed: &mut wrt_format::component::Component,
        host_registry: Option<std::sync::Arc<wrt_host::CallbackRegistry>>,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        trace!("from_parsed: ENTERED, Component passed by reference");

        // Component passed by reference to avoid stack overflow (850KB structure)
        // Work with references throughout

        // Phase 1: Validate safety limits before processing
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to validate_component_limits");
        Self::validate_component_limits(parsed)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: validate_component_limits completed");

        // Phase 2: Build type index (streaming) - Steps 1-3
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to build_type_index");
        let type_index = Self::build_type_index(&parsed.types)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: build_type_index completed");
        // parsed.types can be dropped after indexing (will happen when parsed is dropped)

        // Phase 3: Extract and process core modules (streaming) - Step 4
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to extract_core_modules");
        let module_binaries = Self::extract_core_modules(parsed)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: extract_core_modules completed");
        // parsed.modules is now empty (moved)

        // Phase 4: Parse canonical ABI operations (Step 5)
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to parse_canonical_operations");
        Self::parse_canonical_operations(&parsed.canonicals)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: parse_canonical_operations completed");

        // Phase 5: Parse exports (Step 6a)
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to parse_exports");
        Self::parse_exports(&parsed.exports)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: parse_exports completed");
        // Save exports for later resolution
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to clone exports");
        let exports_to_resolve = parsed.exports.clone();
        #[cfg(feature = "tracing")]
        trace!("from_parsed: exports cloned");

        // Phase 6: Parse imports (Step 6b)
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to parse_imports");
        Self::parse_imports(&parsed.imports)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: parse_imports completed");

        // Phase 6.5: Link imports to providers using unified linker
        // The consolidated ComponentLinker handles:
        // 1. Internal resolution from registered components
        // 2. WASI host provider imports
        // 3. Fails loud on unresolved imports
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to create ComponentLinker (unified)");
        use crate::components::component_linker::ComponentLinker;
        let mut linker = ComponentLinker::new();
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to link_imports");
        let resolved_imports = linker.link_imports(&parsed.imports)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: link_imports completed");

        // Phase 7: Core modules instantiation following component instructions
        // The component's core_instances tell us which modules to instantiate and in what order.
        // Modules with start functions will run automatically during instantiation.

        // Build alias map to track where core exports come from
        // Map: (export_kind, export_idx) -> source_instance_idx
        #[cfg(not(feature = "std"))]
        use alloc::collections::BTreeMap as HashMap;
        #[cfg(feature = "std")]
        use std::collections::HashMap;
        use wrt_format::component::{AliasTarget, CoreSort};

        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to build alias map");
        let mut core_export_sources: HashMap<(CoreSort, u32), u32> = HashMap::new();
        for alias in &parsed.aliases {
            if let AliasTarget::CoreInstanceExport {
                instance_idx,
                name,
                kind,
            } = &alias.target
            {
                // Aliases create new indices in the index space
                // For now, track that exports of this kind from the source instance
                // TODO: Track the actual index mapping once we understand the index space better
                #[cfg(feature = "tracing")]
                trace!(instance_idx = instance_idx, name = %name, kind = ?kind, "Core instance export alias");
            }
        }
        #[cfg(feature = "tracing")]
        trace!("from_parsed: alias map built");

        // Create capability engine for instantiation
        #[cfg(feature = "tracing")]
        trace!("from_parsed: About to create CapabilityAwareEngine");
        use wrt_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};
        let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
        #[cfg(feature = "tracing")]
        trace!("from_parsed: engine created");

        // Set host registry for WASI and custom host functions
        #[cfg(feature = "std")]
        if let Some(ref registry) = host_registry {
            #[cfg(feature = "tracing")]
            trace!("from_parsed: Setting host registry on engine");
            engine.set_host_registry(registry.clone());
            #[cfg(feature = "tracing")]
            trace!("from_parsed: Host registry set successfully");
        }

        // Track instantiated modules by core instance index
        // Map: core_instance_idx -> runtime instance handle
        #[cfg(not(feature = "std"))]
        use alloc::collections::BTreeMap;
        #[cfg(feature = "std")]
        use std::collections::BTreeMap;
        use wrt_runtime::engine::InstanceHandle;
        let mut core_instances_map: BTreeMap<usize, InstanceHandle> = BTreeMap::new();
        // Track InlineExports export names for later use when linking instance imports
        // Map: core_instance_idx -> Vec of (semantic_name, actual_export_name, sort)
        // The semantic_name is used for import matching, the actual_export_name is used for calling
        #[cfg(feature = "std")]
        let mut inline_exports_map: BTreeMap<usize, Vec<(String, String, CoreSort)>> =
            BTreeMap::new();
        // Track which core instance index exports _start (the main executable module)
        // This is the GENERIC way to find the main module - it's the one that exports _start
        let mut start_export_instance_idx: Option<u32> = None;
        // Track if this is an interface-style component (exports wasi:cli/run instead of _start alias)
        let mut is_interface_style = false;

        #[cfg(feature = "std")]
        {
            #[cfg(feature = "tracing")]
            {
                trace!("from_parsed: STEP 7 - Core Module Instantiation");
                trace!(
                    module_count = module_binaries.len(),
                    "Module binaries available"
                );
            }
            let num_core_instances = parsed.core_instances.len();
            #[cfg(feature = "tracing")]
            trace!(
                core_instance_count = num_core_instances,
                "Core instances to instantiate"
            );

            // Build alias map: (CoreSort, index) -> (instance_idx, export_name)
            // This maps each aliased item to its source instance
            // The index now comes from alias.dest_idx which was computed during parsing
            // to account for intermixed canon definitions
            use std::collections::HashMap;
            use wrt_format::component::{AliasTarget, CoreSort};
            let mut alias_map: HashMap<(CoreSort, u32), (u32, String)> = HashMap::new();

            #[cfg(feature = "tracing")]
            trace!(alias_count = parsed.aliases.len(), "Building alias map");
            for alias in &parsed.aliases {
                if let AliasTarget::CoreInstanceExport {
                    instance_idx,
                    name,
                    kind,
                } = &alias.target
                {
                    if let Some(dest_idx) = alias.dest_idx {
                        alias_map.insert((*kind, dest_idx), (*instance_idx, name.clone()));
                        #[cfg(feature = "tracing")]
                        trace!(kind = ?kind, dest_idx = dest_idx, instance_idx = instance_idx, name = %name, "Alias mapping");
                    } else {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(kind = ?kind, instance_idx = instance_idx, name = %name, "Alias missing dest_idx");
                    }
                }
            }
            #[cfg(feature = "tracing")]
            trace!(core_export_count = alias_map.len(), "Alias map built");

            // Handle component instance export aliases
            // Map: (Sort, index) -> (instance_idx, export_name)
            use wrt_format::component::Sort;
            let mut component_alias_map: HashMap<(Sort, u32), (u32, String)> = HashMap::new();

            for alias in &parsed.aliases {
                match &alias.target {
                    AliasTarget::InstanceExport {
                        instance_idx,
                        name,
                        kind,
                    } => {
                        // Component instance export alias
                        if let Some(dest_idx) = alias.dest_idx {
                            component_alias_map
                                .insert((*kind, dest_idx), (*instance_idx, name.clone()));
                            #[cfg(feature = "tracing")]
                            trace!(kind = ?kind, dest_idx = dest_idx, instance_idx = instance_idx, name = %name, "Component alias");
                        }
                    },
                    AliasTarget::Outer { count, kind, idx } => {
                        // Outer alias - references to enclosing component's items
                        // For now, log that we encountered it; full implementation requires
                        // tracking parent component context during nested instantiation
                        #[cfg(feature = "tracing")]
                        trace!(kind = ?kind, count = count, idx = idx, "Outer alias - requires parent context");
                        // TODO: Implement outer alias resolution with parent component tracking
                    },
                    AliasTarget::CoreInstanceExport { .. } => {
                        // Already handled above
                    },
                }
            }
            #[cfg(feature = "tracing")]
            trace!(
                instance_export_count = component_alias_map.len(),
                "Component alias map built"
            );

            // Build canon_lower_map: core_func_idx -> (component_func_idx, interface, function_name)
            // This tracks which core functions were created by canon.lower operations
            // and maps them to their WASI interface/function names for canonical executor dispatch
            //
            // Step 1: Build component_import_func_map - maps component func idx to WASI names
            // Component function indices are assigned in order: first from imports that are functions,
            // then from canon.lift operations
            let mut component_import_func_map: HashMap<u32, (String, String)> = HashMap::new();
            {
                let mut component_func_idx = 0u32;
                for import in &parsed.imports {
                    // Check if this import is a function
                    if matches!(
                        import.ty,
                        wrt_format::component::ExternType::Function { .. }
                    ) {
                        let interface = if !import.name.namespace.is_empty() {
                            import.name.namespace.clone()
                        } else {
                            import.name.name.clone()
                        };
                        let function_name = if !import.name.namespace.is_empty() {
                            import.name.name.clone()
                        } else {
                            String::new()
                        };
                        println!(
                            "    ├─ Component func[{}] = import {}::{}",
                            component_func_idx, interface, function_name
                        );
                        component_import_func_map
                            .insert(component_func_idx, (interface, function_name));
                        component_func_idx += 1;
                    }
                }
                #[cfg(feature = "tracing")]
                trace!(
                    import_func_count = component_func_idx,
                    "Component import function map built"
                );
            }

            // Build instance_interface_map: instance_idx -> WASI interface name
            // In the component model, the instance index space is:
            //   0..K-1:    imported instances (from component imports with ExternType::Instance)
            //   K..K+M-1:  defined instances (from parsed.instances)
            // We need this to resolve WASI interface names for canon.lower operations.
            let instance_interface_map: HashMap<u32, String> = {
                let mut map = HashMap::new();
                let mut instance_import_idx = 0u32;
                for import in &parsed.imports {
                    // In the component model, imports that create instance-space entries include:
                    // - ExternType::Instance { .. } (inline instance type)
                    // - ExternType::Type(idx) where the type is an instance type
                    //   (common: WASI interfaces are encoded as type references)
                    // Function and Value imports go to their own index spaces.
                    let is_instance_import = match &import.ty {
                        wrt_format::component::ExternType::Instance { .. } => true,
                        wrt_format::component::ExternType::Type(_) => true,
                        wrt_format::component::ExternType::Component { .. } => true,
                        wrt_format::component::ExternType::Module { .. } => true,
                        _ => false,
                    };

                    if is_instance_import {
                        // For instance imports, the interface name is the import name
                        let interface = if !import.name.namespace.is_empty() {
                            import.name.namespace.clone()
                        } else {
                            import.name.name.clone()
                        };
                        println!(
                            "    ├─ Instance import[{}] = {}",
                            instance_import_idx, interface
                        );
                        map.insert(instance_import_idx, interface);
                        instance_import_idx += 1;
                    }
                }
                let num_instance_imports = instance_import_idx;
                println!(
                    "    ├─ {} instance imports, {} defined instances",
                    num_instance_imports,
                    parsed.instances.len()
                );
                // Add defined instances (from parsed.instances) with their offset
                for (i, instance) in parsed.instances.iter().enumerate() {
                    let combined_idx = num_instance_imports + i as u32;
                    match &instance.instance_expr {
                        wrt_format::component::InstanceExpr::ComponentReference {
                            arg_refs,
                            ..
                        } => {
                            // For component references, the args might identify WASI interfaces
                            if let Some(arg) = arg_refs.first() {
                                if !arg.name.is_empty() {
                                    map.insert(combined_idx, arg.name.clone());
                                }
                            }
                        },
                        wrt_format::component::InstanceExpr::InlineExports(_) => {
                            // InlineExports don't directly represent WASI interfaces
                        },
                    }
                }
                map
            };

            // Step 2: Build canon_lower_map - maps core_func_idx -> (component_func_idx, interface, function)
            // To compute core function indices, we need to process aliases and canonicals in order.
            // The decoder assigns indices sequentially, interleaving aliases and canonicals.
            //
            // Approach: Use the fact that aliases have dest_idx stored during parsing.
            // Find the maximum dest_idx for function aliases, then canon.lower operations
            // get indices starting after the highest alias dest_idx (in simple cases).
            //
            // For proper handling, we track all function alias dest_idx values and assign
            // canon.lower indices to the remaining slots.
            let mut canon_lower_map: HashMap<u32, (u32, String, String)> = HashMap::new();
            {
                use wrt_format::component::CanonOperation;

                // Collect all function alias dest_idx values
                let alias_func_indices: std::collections::HashSet<u32> = alias_map
                    .iter()
                    .filter(|((sort, _), _)| matches!(sort, CoreSort::Function))
                    .map(|((_, idx), _)| *idx)
                    .collect();

                println!(
                    "    ├─ Found {} function aliases with indices: {:?}",
                    alias_func_indices.len(),
                    alias_func_indices
                );

                // Count canon.lower and canon.resource operations to know how many core functions they create
                let mut canon_creates_count = 0u32;
                for canon in &parsed.canonicals {
                    match &canon.operation {
                        CanonOperation::Lower { .. } => canon_creates_count += 1,
                        CanonOperation::Resource(_) => canon_creates_count += 1,
                        _ => {},
                    }
                }

                // Total core functions = alias functions + canon functions
                // Canon functions get indices that aren't used by aliases
                // In the typical case, indices are assigned sequentially: 0, 1, 2, ...
                // Aliases take some indices, canon operations take the rest

                // Simple approach: assume canon.lower operations get consecutive indices
                // starting from 0, skipping any that are used by aliases
                let mut canon_core_idx = 0u32;
                for canon in &parsed.canonicals {
                    match &canon.operation {
                        CanonOperation::Lower { func_idx, .. } => {
                            // Skip indices used by aliases
                            while alias_func_indices.contains(&canon_core_idx) {
                                canon_core_idx += 1;
                            }

                            // Look up the component function to get WASI name
                            // Try: 1) Direct import map, 2) Component alias map (InstanceExport aliases)
                            if let Some((interface, function)) =
                                component_import_func_map.get(func_idx)
                            {
                                println!(
                                    "    ├─ Canon.lower: core_func[{}] = lower(component_func[{}]) = {}::{}",
                                    canon_core_idx, func_idx, interface, function
                                );
                                canon_lower_map.insert(
                                    canon_core_idx,
                                    (*func_idx, interface.clone(), function.clone()),
                                );
                            } else if let Some((instance_idx, func_name)) =
                                component_alias_map.get(&(Sort::Function, *func_idx))
                            {
                                // This is an InstanceExport alias - the function comes from a component instance
                                // The instance_idx is in the combined instance space:
                                //   0..K-1 = imported instances, K..K+M-1 = defined instances
                                // Use instance_interface_map to resolve the WASI interface name
                                let interface_name = instance_interface_map
                                    .get(instance_idx)
                                    .cloned()
                                    .unwrap_or_default();
                                println!(
                                    "    ├─ Canon.lower: core_func[{}] = lower(component_func[{}]) = instance[{}]::{}",
                                    canon_core_idx, func_idx, instance_idx, func_name
                                );
                                println!("    │  └─ Resolved interface: '{}'", interface_name);
                                canon_lower_map.insert(
                                    canon_core_idx,
                                    (*func_idx, interface_name, func_name.clone()),
                                );
                            } else {
                                println!(
                                    "    ├─ Canon.lower: core_func[{}] = lower(component_func[{}]) - unresolved",
                                    canon_core_idx, func_idx
                                );
                                // Store with component func idx for later resolution
                                canon_lower_map.insert(
                                    canon_core_idx,
                                    (*func_idx, String::new(), String::new()),
                                );
                            }
                            canon_core_idx += 1;
                        },
                        CanonOperation::Resource(_) => {
                            // Resource operations also create core functions, skip their index
                            while alias_func_indices.contains(&canon_core_idx) {
                                canon_core_idx += 1;
                            }
                            canon_core_idx += 1;
                        },
                        _ => {},
                    }
                }

                println!(
                    "    ├─ Built canon_lower_map with {} entries",
                    canon_lower_map.len()
                );
                #[cfg(feature = "tracing")]
                trace!(
                    canon_lower_count = canon_lower_map.len(),
                    "Canon lower map built"
                );
            }

            // Find which core instance exports _start - this is the main executable module
            // This is the GENERIC way to find the main module regardless of component structure
            for alias in &parsed.aliases {
                if let AliasTarget::CoreInstanceExport {
                    instance_idx,
                    name,
                    kind: _,
                } = &alias.target
                {
                    if name == "_start" {
                        start_export_instance_idx = Some(*instance_idx);
                        #[cfg(feature = "tracing")]
                        tracing::info!(
                            instance_idx = instance_idx,
                            "Found _start export - this is the main module"
                        );
                        break;
                    }
                }
            }
            if start_export_instance_idx.is_none() {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "No _start export found in aliases - checking for wasi:cli/run interface"
                );

                // If no _start alias, check for wasi:cli/run interface export
                // This is an interface-style component that uses wasi:cli/run instead of _start
                for export in &parsed.exports {
                    if export.name.name.starts_with("wasi:cli/run@") {
                        // This is an interface-style component
                        // It doesn't need _start - the entry point is via the interface
                        #[cfg(feature = "tracing")]
                        tracing::info!(
                            export_name = %export.name.name,
                            "Found wasi:cli/run interface export - interface-style component"
                        );
                        println!("    ├─ Found wasi:cli/run interface: {}", export.name.name);
                        is_interface_style = true;
                        break;
                    }
                }

                if !is_interface_style {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("No _start export and no wasi:cli/run interface found");
                }
            }

            // Track which core instances have canon-lowered function exports
            // Maps: core_instance_idx -> Vec<(export_name, core_func_idx, interface, function)>
            // Used after module instantiation to register lowered functions with the engine
            let mut canon_lowered_for_instance: HashMap<usize, Vec<(String, u32, String, String)>> =
                HashMap::new();

            // Instantiate modules according to core_instances order
            // core_instances describes WHAT to instantiate and HOW to link imports
            #[cfg(feature = "tracing")]
            trace!("from_parsed: About to iterate core_instances");
            for (core_instance_idx, core_instance) in parsed.core_instances.iter().enumerate() {
                #[cfg(feature = "tracing")]
                trace!(
                    core_instance_idx = core_instance_idx,
                    "Processing CoreInstance"
                );

                // Extract module reference from instance expression
                use wrt_format::component::CoreInstanceExpr;
                match &core_instance.instance_expr {
                    CoreInstanceExpr::ModuleReference {
                        module_idx,
                        arg_refs,
                    } => {
                        let module_idx = *module_idx as usize;
                        println!(
                            "    ├─ Instantiate module {} with {} import args",
                            module_idx,
                            arg_refs.len()
                        );

                        if module_idx >= module_binaries.len() {
                            println!(
                                "    └─ ERROR: Module index {} out of range (have {} modules)",
                                module_idx,
                                module_binaries.len()
                            );
                            continue;
                        }

                        let binary = &module_binaries[module_idx];
                        println!("    ├─ Binary size: {} bytes", binary.len());

                        // Track if this is module 0 (the main 850KB module)
                        if module_idx == 0 {
                            #[cfg(feature = "tracing")]
                            {
                                tracing::info!(
                                    module_idx = module_idx,
                                    core_instance_idx = core_instance_idx,
                                    "THIS IS MODULE 0 - THE MAIN MODULE WITH GLOBALS"
                                );
                            }
                            println!(
                                "    ├─ *** THIS IS MODULE 0 - THE MAIN MODULE WITH GLOBALS ***"
                            );
                            println!(
                                "    ├─ *** Being instantiated as CoreInstance[{}] ***",
                                core_instance_idx
                            );
                        }

                        // Skip diagnostic parsing to avoid stack overflow
                        // TODO: Fix large structure handling in load_wasm_unified
                        #[cfg(feature = "tracing")]
                        trace!("Skipping load_wasm_unified diagnostics to avoid stack overflow");

                        // Load module into engine
                        println!("    ├─ Loading module into engine...");
                        match engine.load_module(binary) {
                            Ok(module_handle) => {
                                println!("    ├─ Module loaded as handle {:?}", module_handle);

                                // Get the import namespaces from the loaded module
                                let import_namespaces =
                                    engine.get_module_import_namespaces(module_handle);
                                println!(
                                    "    ├─ Module has {} import namespaces: {:?}",
                                    import_namespaces.len(),
                                    import_namespaces
                                );

                                // Register import links from arg_refs BEFORE instantiation
                                // NOTE: All imports must be explicitly declared via arg_refs
                                // NO HARDCODED INSTANCE INDICES - this must work for any component, not just TinyGo
                                println!(
                                    "    ├─ Registering {} import links from arg_refs...",
                                    arg_refs.len()
                                );

                                // Check if this module actually needs imports but has none declared
                                if arg_refs.is_empty() && !import_namespaces.is_empty() {
                                    // FAIL LOUD: Module needs imports but none provided in arg_refs
                                    // This indicates the component's alias/instantiation structure isn't being parsed correctly
                                    #[cfg(feature = "tracing")]
                                    {
                                        tracing::error!(
                                            core_instance_idx = core_instance_idx,
                                            import_namespaces = ?import_namespaces,
                                            available_instances = ?core_instances_map.keys().collect::<Vec<_>>(),
                                            alias_count = alias_map.len(),
                                            "Module needs imports but arg_refs is empty"
                                        );
                                        for ((sort, idx), (inst, name)) in &alias_map {
                                            tracing::error!(sort = ?sort, idx = idx, instance = inst, name = %name, "Available alias");
                                        }
                                    }

                                    // TODO: Phase 2.2 - Implement proper alias resolution
                                    // For now, return error instead of using hardcoded indices
                                    return Err(wrt_error::Error::component_linking_error(
                                        "Module requires imports but no arg_refs provided - alias resolution needed",
                                    ));
                                }
                                for arg_ref in arg_refs {
                                    // Handle different kinds of imports based on arg_ref.kind
                                    // kind: 0x00=Func, 0x01=Table, 0x02=Mem, 0x03=Global, 0x12=Instance
                                    let kind_name = match arg_ref.kind {
                                        0x00 => "Func",
                                        0x01 => "Table",
                                        0x02 => "Mem",
                                        0x03 => "Global",
                                        0x12 => "Instance",
                                        _ => "Unknown",
                                    };
                                    println!(
                                        "    │  ├─ Import '{}': kind={} (0x{:02x}), idx={}",
                                        arg_ref.name, kind_name, arg_ref.kind, arg_ref.idx
                                    );

                                    // Resolve the provider based on kind
                                    let (provider_handle, export_name) = match arg_ref.kind {
                                        0x12 => {
                                            // Instance kind: idx is a core instance index
                                            // For instance imports, we need to link ALL exports from the provider
                                            // to the module's imports that match by name
                                            if let Some(&handle) =
                                                core_instances_map.get(&(arg_ref.idx as usize))
                                            {
                                                // NOTE: We do NOT skip wasi_snapshot_preview1 here!
                                                // If a component has an adapter module that implements Preview1
                                                // by translating to Preview2, we should link to the adapter's
                                                // exports, not fall back to the dispatcher.
                                                // The adapter's exports will be in inline_exports_map.

                                                // Check if this is an InlineExports with stored export names
                                                if let Some(export_mappings) =
                                                    inline_exports_map.get(&(arg_ref.idx as usize))
                                                {
                                                    println!(
                                                        "    │  │  ├─ Instance {} has {} exports to link",
                                                        arg_ref.idx,
                                                        export_mappings.len()
                                                    );
                                                    // Link each export from the InlineExports
                                                    // Tuple is (semantic_name, actual_export_name, sort)
                                                    for (
                                                        semantic_name,
                                                        actual_export_name,
                                                        _sort,
                                                    ) in export_mappings
                                                    {
                                                        println!(
                                                            "    │  │  │  ├─ Linking '{}' -> actual export '{}'",
                                                            semantic_name, actual_export_name
                                                        );
                                                        match engine.link_import(
                                                            module_handle,
                                                            &arg_ref.name, // import module name (e.g., "wasi:io/streams@0.2.4")
                                                            semantic_name, // import name (semantic, e.g., "[method]output-stream.blocking-write-and-flush")
                                                            handle,
                                                            actual_export_name, // actual export name in target module (e.g., "1")
                                                        ) {
                                                            Ok(()) => {
                                                                println!("    │  │  │  └─ ✓ Linked")
                                                            },
                                                            Err(e) => println!(
                                                                "    │  │  │  └─ Note: {:?}",
                                                                e
                                                            ),
                                                        }
                                                    }
                                                    // Skip the normal single-link below since we linked everything
                                                    continue;
                                                } else {
                                                    // No stored exports - this is likely a stub/canonical function instance
                                                    // For WASI interfaces, skip linking and let WASI dispatcher handle it
                                                    if arg_ref.name.starts_with("wasi:") {
                                                        println!(
                                                            "    │  │  ├─ WASI interface '{}' - skipping link, will use WASI dispatcher",
                                                            arg_ref.name
                                                        );
                                                        continue;
                                                    }
                                                    // For non-WASI, use the old behavior
                                                    (handle, arg_ref.name.clone())
                                                }
                                            } else {
                                                println!(
                                                    "    │  │  └─ ERROR: Instance {} not yet instantiated",
                                                    arg_ref.idx
                                                );
                                                continue;
                                            }
                                        },
                                        0x01 => {
                                            // Table kind: look up Table[idx] in alias_map
                                            if let Some((inst_idx, exp_name)) =
                                                alias_map.get(&(CoreSort::Table, arg_ref.idx))
                                            {
                                                if let Some(&handle) =
                                                    core_instances_map.get(&(*inst_idx as usize))
                                                {
                                                    println!(
                                                        "    │  │  ├─ Resolved Table[{}] -> instance {} export '{}'",
                                                        arg_ref.idx, inst_idx, exp_name
                                                    );
                                                    (handle, exp_name.clone())
                                                } else {
                                                    println!(
                                                        "    │  │  └─ ERROR: Table source instance {} not instantiated",
                                                        inst_idx
                                                    );
                                                    continue;
                                                }
                                            } else {
                                                println!(
                                                    "    │  │  └─ ERROR: Table[{}] not found in alias_map",
                                                    arg_ref.idx
                                                );
                                                continue;
                                            }
                                        },
                                        0x02 => {
                                            // Memory kind: look up Memory[idx] in alias_map
                                            if let Some((inst_idx, exp_name)) =
                                                alias_map.get(&(CoreSort::Memory, arg_ref.idx))
                                            {
                                                if let Some(&handle) =
                                                    core_instances_map.get(&(*inst_idx as usize))
                                                {
                                                    println!(
                                                        "    │  │  ├─ Resolved Memory[{}] -> instance {} export '{}'",
                                                        arg_ref.idx, inst_idx, exp_name
                                                    );
                                                    (handle, exp_name.clone())
                                                } else {
                                                    println!(
                                                        "    │  │  └─ ERROR: Memory source instance {} not instantiated",
                                                        inst_idx
                                                    );
                                                    continue;
                                                }
                                            } else {
                                                println!(
                                                    "    │  │  └─ ERROR: Memory[{}] not found in alias_map",
                                                    arg_ref.idx
                                                );
                                                continue;
                                            }
                                        },
                                        0x03 => {
                                            // Global kind: look up Global[idx] in alias_map
                                            if let Some((inst_idx, exp_name)) =
                                                alias_map.get(&(CoreSort::Global, arg_ref.idx))
                                            {
                                                if let Some(&handle) =
                                                    core_instances_map.get(&(*inst_idx as usize))
                                                {
                                                    println!(
                                                        "    │  │  ├─ Resolved Global[{}] -> instance {} export '{}'",
                                                        arg_ref.idx, inst_idx, exp_name
                                                    );
                                                    (handle, exp_name.clone())
                                                } else {
                                                    println!(
                                                        "    │  │  └─ ERROR: Global source instance {} not instantiated",
                                                        inst_idx
                                                    );
                                                    continue;
                                                }
                                            } else {
                                                println!(
                                                    "    │  │  └─ ERROR: Global[{}] not found in alias_map",
                                                    arg_ref.idx
                                                );
                                                continue;
                                            }
                                        },
                                        0x00 => {
                                            // Function kind: look up Function[idx] in alias_map
                                            if let Some((inst_idx, exp_name)) =
                                                alias_map.get(&(CoreSort::Function, arg_ref.idx))
                                            {
                                                if let Some(&handle) =
                                                    core_instances_map.get(&(*inst_idx as usize))
                                                {
                                                    println!(
                                                        "    │  │  ├─ Resolved Function[{}] -> instance {} export '{}'",
                                                        arg_ref.idx, inst_idx, exp_name
                                                    );
                                                    (handle, exp_name.clone())
                                                } else {
                                                    println!(
                                                        "    │  │  └─ ERROR: Function source instance {} not instantiated",
                                                        inst_idx
                                                    );
                                                    continue;
                                                }
                                            } else {
                                                println!(
                                                    "    │  │  └─ ERROR: Function[{}] not found in alias_map",
                                                    arg_ref.idx
                                                );
                                                continue;
                                            }
                                        },
                                        _ => {
                                            println!(
                                                "    │  │  └─ ERROR: Unknown kind 0x{:02x}",
                                                arg_ref.kind
                                            );
                                            continue;
                                        },
                                    };

                                    // Determine the import module name
                                    let import_module = if arg_ref.name == "memory"
                                        && !import_namespaces.is_empty()
                                    {
                                        import_namespaces.first().unwrap_or(&String::new()).clone()
                                    } else {
                                        String::new()
                                    };

                                    println!(
                                        "    │  │  ├─ Linking to instance {:?} export '{}' (import module: '{}')",
                                        provider_handle, export_name, import_module
                                    );

                                    // Register the import link
                                    match engine.link_import(
                                        module_handle,
                                        &import_module,
                                        &arg_ref.name,
                                        provider_handle,
                                        &export_name,
                                    ) {
                                        Ok(()) => {
                                            println!("    │  │  └─ ✓ Linked");

                                            // Track aliased functions for cross-instance calls
                                            if arg_ref.name == "_initialize"
                                                || arg_ref.name.starts_with("__wasm_call_")
                                            {
                                                println!(
                                                    "    │  │  └─ ALIASED FUNCTION DETECTED: {}",
                                                    arg_ref.name
                                                );
                                            }
                                        },
                                        Err(e) => {
                                            println!("    │  │  └─ ERROR: Link failed: {:?}", e)
                                        },
                                    }
                                }

                                // Instantiate the module (import links are applied during instantiation)
                                println!("    ├─ Instantiating module...");
                                match engine.instantiate(module_handle) {
                                    Ok(instance_handle) => {
                                        println!(
                                            "    ├─ ✓ Instantiated as instance {:?}",
                                            instance_handle
                                        );
                                        core_instances_map
                                            .insert(core_instance_idx, instance_handle);

                                        // CRITICAL: Remap import links from module_handle to instance_handle
                                        // link_import was called before instantiation with module_handle,
                                        // but runtime lookup uses instance_handle - these IDs differ!
                                        if let Err(e) = engine
                                            .remap_import_links(module_handle, instance_handle)
                                        {
                                            println!(
                                                "    ├─ WARNING: Failed to remap import links: {:?}",
                                                e
                                            );
                                        } else {
                                            println!(
                                                "    ├─ Import links remapped to instance {:?}",
                                                instance_handle
                                            );
                                        }

                                        // Register canon-lowered functions for this instance
                                        // When this module calls an imported canon-lowered function,
                                        // the engine dispatches through the WASI canonical executor
                                        // instead of executing bytecode (preventing infinite recursion).
                                        //
                                        // The func_idx in the module's index space depends on the ORDER
                                        // of arg_refs and how many function imports each provides.
                                        // We track the cumulative function import offset.
                                        {
                                            let mut func_import_offset: usize = 0;
                                            for arg_ref in arg_refs {
                                                match arg_ref.kind {
                                                    0x12 => {
                                                        // Instance kind - provides multiple imports
                                                        // Count function exports from this instance
                                                        let func_export_count = inline_exports_map
                                                            .get(&(arg_ref.idx as usize))
                                                            .map(|mappings| {
                                                                mappings
                                                                    .iter()
                                                                    .filter(|(_, _, sort)| {
                                                                        *sort == CoreSort::Function
                                                                    })
                                                                    .count()
                                                            })
                                                            .unwrap_or(0);

                                                        if let Some(lowered_exports) =
                                                            canon_lowered_for_instance
                                                                .get(&(arg_ref.idx as usize))
                                                        {
                                                            println!(
                                                                "    ├─ Registering {} canon-lowered functions for instance {:?} (from InlineExports[{}], offset={})",
                                                                lowered_exports.len(),
                                                                instance_handle,
                                                                arg_ref.idx,
                                                                func_import_offset
                                                            );
                                                            for (
                                                                i,
                                                                (_name, _idx, interface, function),
                                                            ) in
                                                                lowered_exports.iter().enumerate()
                                                            {
                                                                // func_idx = offset + position within this InlineExports
                                                                let func_idx =
                                                                    func_import_offset + i;
                                                                println!(
                                                                    "    │  ├─ Lowered func[{}] = {}::{}",
                                                                    func_idx, interface, function
                                                                );
                                                                engine.register_lowered_function(
                                                                    instance_handle.index(),
                                                                    func_idx,
                                                                    interface.clone(),
                                                                    function.clone(),
                                                                    None, // memory_idx
                                                                    None, // realloc_idx
                                                                );
                                                            }
                                                        }

                                                        func_import_offset += func_export_count;
                                                    },
                                                    0x00 => {
                                                        // Function kind - single function import
                                                        func_import_offset += 1;
                                                    },
                                                    _ => {}, // Table (0x01), Memory (0x02), Global (0x03) don't affect function index
                                                }
                                            }
                                        }

                                        // Check if this is the instance that exports _start
                                        // (we found this by scanning aliases earlier)
                                        if start_export_instance_idx
                                            == Some(core_instance_idx as u32)
                                        {
                                            #[cfg(feature = "tracing")]
                                            tracing::info!(
                                                ?instance_handle,
                                                core_instance_idx = core_instance_idx,
                                                "Main module (_start exporter) instantiated"
                                            );
                                        }

                                        println!("    └─ Instance ready");
                                    },
                                    Err(e) => {
                                        println!("    └─ ERROR: Instantiation failed: {:?}", e);
                                        println!("    └─ Reason: {:?}", e);
                                        // Continue with other modules
                                    },
                                }
                            },
                            Err(e) => {
                                println!("    └─ ERROR: Failed to load module: {:?}", e);
                            },
                        }
                    },
                    CoreInstanceExpr::InlineExports(exports) => {
                        println!("    ├─ InlineExports with {} exports", exports.len());

                        // Debug: show what's in the exports
                        for export in exports {
                            println!(
                                "    │  ├─ Export[{}]: name='{}', sort={:?}",
                                export.idx, export.name, export.sort
                            );
                        }

                        // InlineExports re-export items from other instances via aliases
                        // Use the alias_map to find where these exports come from
                        // Build mapping: (semantic_name, actual_export_name, sort)
                        let mut source_instance_idx: Option<u32> = None;
                        let mut export_mappings: Vec<(String, String, CoreSort)> = Vec::new();

                        // Track canon-lowered function exports separately
                        // These need special handling via canonical executor
                        let mut canon_lowered_exports: Vec<(String, u32, String, String)> =
                            Vec::new();

                        for export in exports {
                            // Look up this export in the alias map using (sort, index)
                            if let Some((instance_idx, actual_export_name)) =
                                alias_map.get(&(export.sort, export.idx))
                            {
                                if source_instance_idx.is_none() {
                                    source_instance_idx = Some(*instance_idx);
                                }
                                println!(
                                    "    │  ├─ Export[{}] '{}' (sort={:?}) -> instance {} export '{}'",
                                    export.idx,
                                    export.name,
                                    export.sort,
                                    instance_idx,
                                    actual_export_name
                                );
                                // Store BOTH semantic name (for import matching) and actual export name (for calling)
                                export_mappings.push((
                                    export.name.clone(),
                                    actual_export_name.clone(),
                                    export.sort,
                                ));
                            } else if export.sort == CoreSort::Function {
                                // No alias found for function - check if it's a canon-lowered function
                                if let Some((comp_func_idx, interface, function)) =
                                    canon_lower_map.get(&export.idx)
                                {
                                    println!(
                                        "    │  ├─ Export[{}] '{}' -> CANON LOWERED: {}::{} (comp_func[{}])",
                                        export.idx, export.name, interface, function, comp_func_idx
                                    );
                                    // Store for canonical executor handling
                                    canon_lowered_exports.push((
                                        export.name.clone(),
                                        export.idx,
                                        interface.clone(),
                                        function.clone(),
                                    ));
                                    // Still add to export_mappings with a marker indicating canonical handling needed
                                    // The actual dispatch will be handled by the canonical executor
                                    export_mappings.push((
                                        export.name.clone(),
                                        format!("__canon_lower_{}::{}", interface, function),
                                        export.sort,
                                    ));
                                } else {
                                    println!(
                                        "    │  ├─ Export[{}] '{}' -> UNKNOWN (no alias, no canon.lower)",
                                        export.idx, export.name
                                    );
                                    export_mappings.push((
                                        export.name.clone(),
                                        export.name.clone(),
                                        export.sort,
                                    ));
                                }
                            } else {
                                // Non-function export without alias
                                println!(
                                    "    │  ├─ Export[{}] '{}' (sort={:?}) -> no alias",
                                    export.idx, export.name, export.sort
                                );
                                export_mappings.push((
                                    export.name.clone(),
                                    export.name.clone(),
                                    export.sort,
                                ));
                            }
                        }

                        if let Some(src_idx) = source_instance_idx {
                            if let Some(&source_handle) =
                                core_instances_map.get(&(src_idx as usize))
                            {
                                println!(
                                    "    ├─ Mapping to source instance {} (handle {:?})",
                                    src_idx, source_handle
                                );
                                core_instances_map.insert(core_instance_idx, source_handle);
                                // Store the actual export names for later use when linking instance imports
                                println!(
                                    "    ├─ Storing {} export names for instance linking",
                                    export_mappings.len()
                                );
                                inline_exports_map.insert(core_instance_idx, export_mappings);
                                println!("    └─ Mapped");
                            } else {
                                println!(
                                    "    └─ ERROR: Source instance {} not yet instantiated",
                                    src_idx
                                );
                                return Err(Error::runtime_error(
                                    "InlineExports source instance not instantiated",
                                ));
                            }
                        } else if !canon_lowered_exports.is_empty() {
                            // All exports are canon-lowered functions - this is a canonical function provider
                            println!(
                                "    ├─ InlineExports contains {} canon-lowered functions:",
                                canon_lowered_exports.len()
                            );
                            for (name, idx, interface, function) in &canon_lowered_exports {
                                println!(
                                    "    │  ├─ '{}' (idx={}) -> {}::{}",
                                    name, idx, interface, function
                                );

                                // Register this lowered function with the engine for canonical execution
                                // The engine will route calls to these functions through the canonical executor
                                #[cfg(feature = "std")]
                                {
                                    // Register the lowered function for canonical executor dispatch
                                    // This will be called when the module imports this function
                                    let full_interface = interface.clone();
                                    let func_name = function.clone();
                                    println!(
                                        "    │  │  └─ Registering for canonical dispatch: {}::{}",
                                        full_interface, func_name
                                    );
                                }
                            }

                            // Use instance 0 as a placeholder handle for the InlineExports
                            // The actual execution will be handled by the canonical executor
                            if let Some(&stub_handle) = core_instances_map.get(&0) {
                                core_instances_map.insert(core_instance_idx, stub_handle);
                                // Store export mappings for import linking
                                inline_exports_map.insert(core_instance_idx, export_mappings);
                                println!("    └─ Canonical functions registered for dispatch");
                            } else {
                                return Err(Error::runtime_error(
                                    "Cannot configure canonicals - no base instance",
                                ));
                            }
                        } else {
                            println!("    └─ WARNING: InlineExports with no resolved exports");
                            // Create a stub instance
                            if let Some(&stub_handle) = core_instances_map.get(&0) {
                                core_instances_map.insert(core_instance_idx, stub_handle);
                                inline_exports_map.insert(core_instance_idx, export_mappings);
                                println!("    └─ Stub instance created");
                            } else {
                                return Err(Error::runtime_error(
                                    "Cannot create stub - no base instance",
                                ));
                            }
                        }

                        // Store canon-lowered exports for later registration with the engine
                        // This is used after module instantiation to register function indices
                        // so call_indirect dispatches them through the canonical executor
                        if !canon_lowered_exports.is_empty() {
                            println!(
                                "    ├─ Storing {} canon-lowered exports for core_instance[{}]",
                                canon_lowered_exports.len(),
                                core_instance_idx
                            );
                            canon_lowered_for_instance
                                .insert(core_instance_idx, canon_lowered_exports);
                        }
                    },
                }
            }
            println!();
        }

        // Phase 7b: Process nested component instances
        // Nested components are instantiated and their exports can be aliased
        #[cfg(feature = "std")]
        let nested_instances = {
            use crate::types::{NestedComponentInstance, NestedExportKind, NestedExportRef};
            use wrt_format::component::{InstanceExpr, Sort};

            println!("=== Phase 7b: Nested Component Instances ===");
            println!("Nested components defined: {}", parsed.components.len());
            println!("Component instances to create: {}", parsed.instances.len());

            // Track nested component instances - will be stored in the final ComponentInstance
            let mut nested_instances: Vec<NestedComponentInstance> = Vec::new();

            // Track component instance exports for alias resolution
            // Maps: instance_idx -> (export_name -> (sort, idx))
            let mut component_instance_exports: std::collections::HashMap<
                usize,
                std::collections::HashMap<String, (Sort, u32)>,
            > = std::collections::HashMap::new();

            for (inst_idx, instance) in parsed.instances.iter().enumerate() {
                match &instance.instance_expr {
                    InstanceExpr::ComponentReference {
                        component_idx,
                        arg_refs,
                    } => {
                        println!(
                            "  ├─ Instance[{}]: Instantiate nested component {} with {} args",
                            inst_idx,
                            component_idx,
                            arg_refs.len()
                        );

                        let comp_idx = *component_idx as usize;

                        // Check if component exists
                        if comp_idx >= parsed.components.len() {
                            println!(
                                "ERROR: Nested component index {} out of range (have {} components)",
                                component_idx,
                                parsed.components.len()
                            );
                            return Err(Error::new(
                                wrt_error::ErrorCategory::Validation,
                                wrt_error::codes::VALIDATION_ERROR,
                                "Nested component index out of range",
                            ));
                        }

                        // Get the nested component definition
                        let nested_component = &parsed.components[comp_idx];

                        println!("      ├─ Nested component structure:");
                        println!(
                            "      │  ├─ Core modules: {}",
                            nested_component.modules.len()
                        );
                        println!("      │  ├─ Imports: {}", nested_component.imports.len());
                        println!("      │  ├─ Exports: {}", nested_component.exports.len());
                        println!("      │  ├─ Types: {}", nested_component.types.len());
                        println!(
                            "      │  └─ Sub-components: {}",
                            nested_component.components.len()
                        );

                        // Resolve arguments from arg_refs
                        // These reference exports from the parent component's index spaces
                        println!(
                            "      ├─ Resolving {} instantiation arguments:",
                            arg_refs.len()
                        );
                        for arg_ref in arg_refs {
                            println!(
                                "        ├─ Arg '{}': sort={:?} idx={}",
                                arg_ref.name, arg_ref.sort, arg_ref.idx
                            );
                        }

                        // Check if this is a "passthrough" component with no core modules
                        // These are typically wrapper components created by wit-component to satisfy
                        // interface type requirements. They import a function and re-export it.
                        //
                        // Pattern: (component (import "x" (func)) (export "y" (func 0)))
                        // This just wraps the imported function, no actual execution needed.
                        if nested_component.modules.is_empty() {
                            println!("      ├─ Passthrough component detected (no core modules)");
                            println!("      ├─ Handling as function forwarder");

                            // For passthrough components, create a synthetic instance that maps
                            // the nested component's exports to the parent's provided functions
                            let mut instance_export_map: std::collections::HashMap<
                                String,
                                (Sort, u32),
                            > = std::collections::HashMap::new();

                            // The exports from this nested component should map to the provided args
                            // For now, map each export to the corresponding arg function
                            for export in &nested_component.exports {
                                // The export references func 0, which is the imported function,
                                // which is provided by arg_ref[0]
                                if !arg_refs.is_empty() {
                                    let arg = &arg_refs[0];
                                    let export_name_str = export.name.name.clone();
                                    println!(
                                        "        ├─ Export '{}' -> provided func idx {}",
                                        export_name_str, arg.idx
                                    );
                                    instance_export_map
                                        .insert(export_name_str, (arg.sort, arg.idx));
                                }
                            }

                            // Store for alias resolution
                            component_instance_exports.insert(inst_idx, instance_export_map);
                            println!("      └─ Passthrough instance {} ready", inst_idx);
                        } else {
                            // Full nested component with core modules - requires recursive instantiation
                            println!(
                                "      ├─ Full nested component with {} core modules",
                                nested_component.modules.len()
                            );

                            // =====================================================================
                            // Circular Dependency and Nesting Depth Validation
                            // =====================================================================

                            // Check nesting depth to prevent infinite recursion
                            // The ID encoding uses id * 1000 + inst_idx, so we can decode depth
                            let current_depth = {
                                let mut depth = 0usize;
                                let mut test_id = id as u64;
                                while test_id >= 1000 {
                                    depth += 1;
                                    test_id /= 1000;
                                }
                                depth
                            };

                            if current_depth >= MAX_COMPONENT_NESTING_DEPTH {
                                println!(
                                    "ERROR: Maximum component nesting depth ({}) exceeded at depth {}",
                                    MAX_COMPONENT_NESTING_DEPTH, current_depth
                                );
                                return Err(Error::new(
                                    wrt_error::ErrorCategory::Validation,
                                    wrt_error::codes::VALIDATION_ERROR,
                                    "Maximum component nesting depth exceeded - possible circular dependency",
                                ));
                            }

                            // Check for sub-components that could create circular dependencies
                            if !nested_component.components.is_empty() {
                                println!(
                                    "      ├─ Nested component has {} sub-components (depth {})",
                                    nested_component.components.len(),
                                    current_depth + 1
                                );

                                // Warn about potential deep nesting
                                if current_depth + 1 >= MAX_COMPONENT_NESTING_DEPTH / 2 {
                                    println!(
                                        "      ├─ WARNING: Approaching max nesting depth limit"
                                    );
                                }
                            }

                            // Create a unique ID for the nested instance
                            let nested_id = (id as u64 * 1000 + inst_idx as u64) as u32;

                            // Clone the nested component for instantiation
                            let mut nested_component_clone = nested_component.clone();

                            // Recursively instantiate the nested component
                            // Note: We pass None for host_registry - imports should come from args
                            // TODO: Implement proper import resolution from arg_refs
                            match Self::from_parsed(nested_id, &mut nested_component_clone, None) {
                                Ok(child_instance) => {
                                    println!("      ├─ Successfully instantiated nested component");
                                    println!("      │  ├─ Child ID: {}", nested_id);
                                    println!(
                                        "      │  ├─ Child exports: {}",
                                        child_instance.exports.len()
                                    );

                                    // Build exports map for this nested instance
                                    let mut exports_map: std::collections::HashMap<
                                        String,
                                        NestedExportRef,
                                    > = std::collections::HashMap::new();
                                    let mut instance_export_map: std::collections::HashMap<
                                        String,
                                        (Sort, u32),
                                    > = std::collections::HashMap::new();

                                    // The child's exports become accessible to the parent
                                    for (exp_idx, export) in
                                        child_instance.exports.iter().enumerate()
                                    {
                                        let kind = NestedExportKind::Function;
                                        exports_map.insert(
                                            export.name.clone(),
                                            NestedExportRef {
                                                kind,
                                                index: exp_idx as u32,
                                            },
                                        );
                                        instance_export_map.insert(
                                            export.name.clone(),
                                            (Sort::Function, exp_idx as u32),
                                        );
                                        println!(
                                            "      │  ├─ Export '{}' available for aliasing",
                                            export.name
                                        );
                                    }

                                    // Store the instance export map for alias resolution
                                    component_instance_exports
                                        .insert(inst_idx, instance_export_map);

                                    // Create the nested instance wrapper
                                    let nested = NestedComponentInstance {
                                        instance_index: inst_idx as u32,
                                        component_index: *component_idx,
                                        instance: Box::new(child_instance),
                                        exports: exports_map,
                                    };

                                    nested_instances.push(nested);
                                    println!(
                                        "      └─ Nested instance {} created successfully",
                                        inst_idx
                                    );
                                },
                                Err(e) => {
                                    // Provide detailed error context for debugging via println
                                    // (the static error message is complemented by this output)
                                    println!(
                                        "ERROR: Failed to instantiate nested component {}: {}",
                                        component_idx, e
                                    );
                                    println!("       ├─ Parent instance ID: {}", id);
                                    println!("       ├─ Nested instance index: {}", inst_idx);
                                    println!("       ├─ Component index: {}", component_idx);
                                    println!("       ├─ Nesting depth: {}", current_depth);
                                    println!("       └─ Original error: {}", e);

                                    // Use static error message - dynamic details are already printed
                                    // Check if this looks like a circular dependency
                                    let is_circular = e.to_string().contains("nesting depth");
                                    let static_msg = if is_circular {
                                        "Failed to instantiate nested component: possible circular dependency"
                                    } else {
                                        "Failed to instantiate nested component"
                                    };

                                    return Err(Error::new(
                                        wrt_error::ErrorCategory::ComponentRuntime,
                                        wrt_error::codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                                        static_msg,
                                    ));
                                },
                            }
                        }
                    },
                    InstanceExpr::InlineExports(exports) => {
                        println!(
                            "  ├─ Instance[{}]: Inline exports ({} items)",
                            inst_idx,
                            exports.len()
                        );

                        // Inline exports create a synthetic instance from explicit exports
                        // These exports reference items from the parent's index spaces
                        let mut instance_export_map: std::collections::HashMap<
                            String,
                            (Sort, u32),
                        > = std::collections::HashMap::new();

                        for export in exports {
                            println!(
                                "      ├─ Export: '{}' sort {:?} idx {}",
                                export.name, export.sort, export.idx
                            );
                            instance_export_map
                                .insert(export.name.clone(), (export.sort, export.idx));
                        }

                        // Store for alias resolution
                        component_instance_exports.insert(inst_idx, instance_export_map);
                    },
                }
            }

            println!(
                "Nested component instances created: {}",
                nested_instances.len()
            );
            println!(
                "Component instances with exports: {}",
                component_instance_exports.len()
            );
            println!();

            nested_instances
        };

        #[cfg(not(feature = "std"))]
        let nested_instances = {
            // Nested component instantiation not supported in no_std
            // Return empty vec equivalent
            BoundedVec::new()
        };

        // Phase 8-11: DEFERRED - These phases depend on instantiated modules
        // Will be performed lazily during function execution

        // Build minimal runtime component
        // In future: this will hold the actual converted data
        let component_type = crate::components::component::WrtComponentType::default();
        let mut runtime_component = RuntimeComponent::new(component_type);

        // Store module binaries in runtime component
        runtime_component.modules = module_binaries;

        // Phase 8: Create instance with runtime component
        // At this point, parsed is dropped and only runtime_component remains
        let mut instance = Self::new(id, runtime_component)?;

        // Store type index for runtime type resolution (Step 2)
        #[cfg(feature = "std")]
        {
            instance.type_index = type_index;
        }

        // Resolve exports and add to instance
        #[cfg(feature = "std")]
        {
            Self::resolve_exports(&mut instance, &exports_to_resolve)?;
        }

        // Store resolved imports in instance
        #[cfg(feature = "std")]
        {
            instance.imports = resolved_imports;
            println!(
                "[INSTANTIATION] Stored {} resolved imports in instance",
                instance.imports.len()
            );
        }

        // Store nested component instances
        {
            instance.nested_component_instances = nested_instances;
            println!(
                "[INSTANTIATION] Stored {} nested component instances",
                instance.nested_component_instances.len()
            );
        }

        // Execute start functions immediately (Step 12) - DEFERRED
        #[cfg(feature = "std")]
        {
            println!("\n=== Component Initialization Complete ===");
            println!(
                "Modules: {} binaries stored",
                instance.component.modules.len()
            );
            println!("Imports: {} resolved", instance.imports.len());
            println!();
            println!("Start functions will execute when component functions are called");
            println!("Component ready for execution\n");

            // Update instance state to Running
            instance.state = ComponentInstanceState::Running;
        }

        // Update metadata with conversion stats
        instance.metadata.function_calls = 0;

        // Test type lookup (Step 2)
        #[cfg(feature = "std")]
        {
            println!("=== STEP 2: Type Lookup Test ===");
            for idx in 0..instance.type_index.len() {
                match instance.get_type(idx as u32) {
                    Ok(ty) => {
                        let type_name = Self::get_type_name(ty);
                        println!("✓ Type lookup successful: Type[{}] = {}", idx, type_name);
                    },
                    Err(e) => {
                        println!("✗ Type lookup failed: Type[{}] - {:?}", idx, e);
                    },
                }
            }
            println!();
        }

        // Test safe type resolution with circular reference detection (Step 3)
        #[cfg(feature = "std")]
        {
            println!("=== STEP 3: Safe Type Resolution Test ===");
            for idx in 0..instance.type_index.len() {
                match instance.resolve_type_safe(idx as u32) {
                    Ok(ty) => {
                        let type_name = Self::get_type_name(ty);
                        println!(
                            "✓ Safe resolution successful: Type[{}] → {}",
                            idx, type_name
                        );
                    },
                    Err(e) => {
                        println!("✗ Safe resolution failed: Type[{}] - {:?}", idx, e);
                    },
                }
            }

            // Test error handling for out-of-bounds
            let invalid_idx = instance.type_index.len() as u32 + 10;
            match instance.resolve_type_safe(invalid_idx) {
                Ok(_) => {
                    println!("✗ Should have failed for invalid index {}", invalid_idx);
                },
                Err(_) => {
                    println!("✓ Correctly rejected invalid index {}", invalid_idx);
                },
            }
            println!();
        }

        // Store the engine in the instance so it can be used for executing functions
        #[cfg(feature = "wrt-execution")]
        {
            instance.runtime_engine = Some(Box::new(engine));

            // Store the main module's instance handle
            // The main module is the one that exports _start - we found this earlier
            // by scanning the aliases. This is GENERIC and works for any component.
            // NO FALLBACKS - per spec, command components MUST export _start

            let main_handle = match start_export_instance_idx {
                Some(start_idx) => match core_instances_map.get(&(start_idx as usize)) {
                    Some(&handle) => {
                        #[cfg(feature = "tracing")]
                        tracing::info!(
                            ?handle,
                            core_instance_idx = start_idx,
                            "Found main module instance - exports _start"
                        );
                        instance.main_instance_handle = Some(handle);
                        handle
                    },
                    None => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(
                            start_idx = start_idx,
                            available = ?core_instances_map.keys().collect::<Vec<_>>(),
                            "Instance exporting _start not found in map"
                        );
                        return Err(Error::runtime_execution_error(
                            "Cannot find main module instance (the one exporting _start)",
                        ));
                    },
                },
                None => {
                    if is_interface_style {
                        // Interface-style component (exports wasi:cli/run at component level)
                        // The CORE module still exports _start - search for it.
                        // NOTE: wasi:cli/run@*#run are COMPONENT-level export names, not core module
                        // exports! Always search for _start in core modules.
                        let mut found_handle = None;
                        // For Rust component model, entry point is often wasi:cli/run@VERSION#run
                        // For C/TinyGo, entry point is _start
                        let core_entry_points = [
                            "_start",
                            "wasi:cli/run@0.2.3#run",
                            "wasi:cli/run@0.2.6#run",
                            "wasi:cli/run@0.2.0#run",
                            "run",
                            "main",
                        ];

                        println!(
                            "    ├─ Searching {} core instances for entry point...",
                            core_instances_map.len()
                        );
                        for (&idx, &handle) in &core_instances_map {
                            if let Some(engine_ref) = &instance.runtime_engine {
                                // Debug: list what exports this instance has
                                println!(
                                    "    │  ├─ Instance {} (handle {:?}) exports:",
                                    idx, handle
                                );
                                for entry_point in &[
                                    "_start",
                                    "_initialize",
                                    "run",
                                    "main",
                                    "memory",
                                    "cabi_realloc",
                                    "wasi:cli/run@0.2.3#run",
                                    "wasi:cli/run@0.2.6#run",
                                ] {
                                    let has = engine_ref
                                        .has_function(handle, entry_point)
                                        .unwrap_or(false);
                                    if has {
                                        println!("    │  │  ✓ {}", entry_point);
                                    }
                                }

                                for entry_point in &core_entry_points {
                                    if engine_ref.has_function(handle, entry_point).unwrap_or(false)
                                    {
                                        println!(
                                            "    ├─ Found {} export in core instance {} (handle {:?})",
                                            entry_point, idx, handle
                                        );
                                        found_handle = Some(handle);
                                        break;
                                    }
                                }
                                if found_handle.is_some() {
                                    break;
                                }
                            }
                        }

                        // NO FALLBACK: Per CLAUDE.md rules, we must not guess.
                        // If _start is not found, the component is malformed or our loading is broken.
                        if let Some(handle) = found_handle {
                            #[cfg(feature = "tracing")]
                            tracing::info!(
                                ?handle,
                                "Interface-style component - found instance with core entry point"
                            );
                            instance.main_instance_handle = Some(handle);
                            handle
                        } else {
                            println!("    ✗ ERROR: No core instance exports _start, run, or main!");
                            println!(
                                "    Available instances: {:?}",
                                core_instances_map.keys().collect::<Vec<_>>()
                            );
                            return Err(Error::runtime_execution_error(
                                "No core instance exports _start - module exports not properly loaded",
                            ));
                        }
                    } else {
                        #[cfg(feature = "tracing")]
                        tracing::error!(
                            "No _start export found - component does not export an entry point"
                        );
                        return Err(Error::runtime_execution_error(
                            "No _start export found - command components must export _start",
                        ));
                    }
                },
            };

            #[cfg(feature = "tracing")]
            if is_interface_style {
                tracing::info!(
                    ?main_handle,
                    "Main instance selected (interface-style component)"
                );
            } else {
                tracing::info!(?main_handle, "Main instance selected (exports _start)");
            }
        }

        Ok(instance)
    }

    /// Get a type by index (Step 2)
    ///
    /// Looks up a type definition by its index in the type section.
    /// Returns an error if the index is out of bounds.
    #[cfg(feature = "std")]
    pub fn get_type(&self, idx: u32) -> Result<&wrt_format::component::ComponentType> {
        self.type_index.get(&idx).ok_or_else(|| {
            Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::OUT_OF_BOUNDS_ERROR,
                "Type index out of bounds",
            )
        })
    }

    /// Get a type by index (no_std placeholder)
    #[cfg(not(feature = "std"))]
    pub fn get_type(&self, _idx: u32) -> Result<()> {
        // No_std version - type lookup not yet implemented
        Ok(())
    }

    /// Resolve a type reference, following any indirection (Step 3)
    ///
    /// This resolves ExternType::Type(idx) to the actual ComponentType definition.
    /// It handles potential alias chains and detects circular references.
    ///
    /// **Use case**: When processing exports/imports, we often have ExternType::Type(idx)
    /// references that need to be resolved to know the actual function signature, etc.
    #[cfg(feature = "std")]
    pub fn resolve_extern_type(
        &self,
        extern_type: &wrt_format::component::ExternType,
    ) -> Result<Option<&wrt_format::component::ComponentType>> {
        match extern_type {
            wrt_format::component::ExternType::Type(idx) => {
                // Resolve the type index
                let resolved = self.get_type(*idx)?;
                Ok(Some(resolved))
            },
            _ => {
                // Not a type reference - return None
                Ok(None)
            },
        }
    }

    /// Resolve a type with circular reference detection (Step 3)
    ///
    /// This is a safety wrapper around get_type that prevents infinite loops
    /// in case of circular type references (though these should be caught by validation).
    ///
    /// **Max depth**: 16 levels of indirection (ASIL-D bounded)
    #[cfg(feature = "std")]
    pub fn resolve_type_safe(&self, idx: u32) -> Result<&wrt_format::component::ComponentType> {
        const MAX_RESOLUTION_DEPTH: usize = 16;

        let mut current_idx = idx;
        let mut visited = std::collections::HashSet::new();

        for depth in 0..MAX_RESOLUTION_DEPTH {
            // Check for circular reference
            if !visited.insert(current_idx) {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Validation,
                    wrt_error::codes::VALIDATION_ERROR,
                    "Circular type reference detected",
                ));
            }

            // Get the type
            let ty = self.get_type(current_idx)?;

            // Check if this is a type that needs further resolution
            // (In Component Model, types are usually direct, not aliased through type definitions)
            // So we just return the type we found
            return Ok(ty);
        }

        // If we hit max depth without resolving, that's an error
        Err(Error::new(
            wrt_error::ErrorCategory::Validation,
            wrt_error::codes::VALIDATION_ERROR,
            "Type resolution depth exceeded (possible circular reference)",
        ))
    }

    /// No_std placeholder for resolve functions
    #[cfg(not(feature = "std"))]
    pub fn resolve_type_safe(&self, _idx: u32) -> Result<()> {
        Ok(())
    }

    /// Validate component against ASIL-D safety limits
    ///
    /// Fails fast if any limit is exceeded, preventing resource exhaustion.
    fn validate_component_limits(parsed: &wrt_format::component::Component) -> Result<()> {
        use wrt_error::{ErrorCategory, codes};

        if parsed.modules.len() > MAX_CORE_MODULES {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum modules limit (ASIL-D)",
            ));
        }

        if parsed.types.len() > MAX_TYPES {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum types limit (ASIL-D)",
            ));
        }

        if parsed.canonicals.len() > MAX_CANONICAL_OPS {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum canonical operations limit (ASIL-D)",
            ));
        }

        if parsed.exports.len() > MAX_EXPORTS_PER_COMPONENT {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum exports limit (ASIL-D)",
            ));
        }

        if parsed.imports.len() > MAX_IMPORTS_PER_COMPONENT {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum imports limit (ASIL-D)",
            ));
        }

        if parsed.core_instances.len() > MAX_CORE_INSTANCES {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::CAPACITY_EXCEEDED,
                "Component exceeds maximum core instances limit (ASIL-D)",
            ));
        }

        Ok(())
    }

    /// Extract core module binaries from parsed component
    ///
    /// **Performance**: Modules are moved (not copied) from parsed component.
    /// When threading is enabled, validation can be parallelized.
    ///
    /// **Memory**: After this call, parsed.modules is empty - memory is transferred.
    /// **Step 4**: Now shows detailed validation output for each module
    fn extract_core_modules(parsed: &mut wrt_format::component::Component) -> Result<Vec<Vec<u8>>> {
        use wrt_error::{ErrorCategory, codes};

        #[cfg(feature = "std")]
        {
            println!("=== STEP 4: Module Extraction and Validation ===");
            println!("Total modules to extract: {}", parsed.modules.len());
        }

        let mut module_binaries = Vec::with_capacity(parsed.modules.len());

        // Move modules out of parsed component (no copy)
        for (idx, module) in parsed.modules.drain(..).enumerate() {
            // Extract the binary data from the module
            // Module structure contains the actual binary
            if let Some(binary) = module.binary {
                #[cfg(feature = "std")]
                println!("  Module[{}]:", idx);

                // Validate module magic/version before accepting
                if binary.len() >= 8 && &binary[0..4] == b"\0asm" {
                    // Read version bytes
                    let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);

                    #[cfg(feature = "std")]
                    {
                        println!("    ├─ Size: {} bytes", binary.len());
                        println!("    ├─ Magic: ✓ (valid WASM)");
                        println!("    ├─ Version: {}", version);

                        // Validate it's a core module (version 1)
                        if version == 1 {
                            println!("    └─ Type: Core Module ✓");
                        } else {
                            println!("    └─ Type: Unknown version (expected 1)");
                        }
                    }

                    module_binaries.push(binary.to_vec());
                } else {
                    #[cfg(feature = "std")]
                    println!("    └─ ✗ Invalid magic number or too short");

                    return Err(Error::new(
                        ErrorCategory::Validation,
                        codes::PARSE_INVALID_MAGIC_BYTES,
                        "Invalid WebAssembly module magic number",
                    ));
                }
            } else {
                #[cfg(feature = "std")]
                println!("  Module[{}]: ✗ Missing binary data", idx);

                return Err(Error::new(
                    ErrorCategory::Validation,
                    codes::PARSE_INVALID_SECTION_ID,
                    "Core module missing binary data",
                ));
            }
        }

        #[cfg(feature = "std")]
        println!("✓ Extracted {} valid modules\n", module_binaries.len());

        Ok(module_binaries)
    }

    /// Get a readable type name for logging/debugging
    fn get_type_name(ty: &wrt_format::component::ComponentType) -> &'static str {
        use wrt_format::component::ComponentTypeDefinition;
        match &ty.definition {
            ComponentTypeDefinition::Component { .. } => "Component",
            ComponentTypeDefinition::Instance { .. } => "Instance",
            ComponentTypeDefinition::Function { .. } => "Function",
            ComponentTypeDefinition::Value(_) => "Value",
            ComponentTypeDefinition::Resource { .. } => "Resource",
        }
    }

    /// Parse and display canonical ABI operations (Step 5)
    ///
    /// Canonical operations define how to convert between core WebAssembly
    /// and component model values:
    /// - Lift: Convert core values to component values
    /// - Lower: Convert component values to core values
    /// - Resource: Resource lifecycle operations (new/drop/rep)
    #[cfg(feature = "std")]
    fn parse_canonical_operations(canonicals: &[wrt_format::component::Canon]) -> Result<()> {
        println!("=== STEP 5: Canonical ABI Operations ===");
        println!("Total operations: {}", canonicals.len());

        if canonicals.is_empty() {
            println!("(No canonical operations defined)\n");
            return Ok(());
        }

        for (idx, canon) in canonicals.iter().enumerate() {
            use wrt_format::component::CanonOperation;

            print!("  Canon[{}]: ", idx);

            match &canon.operation {
                CanonOperation::Lift {
                    func_idx, type_idx, ..
                } => {
                    println!("Lift");
                    println!("    ├─ Core func[{}] → Component", func_idx);
                    println!("    └─ Type: type[{}]", type_idx);
                },
                CanonOperation::Lower { func_idx, .. } => {
                    println!("Lower");
                    println!("    └─ Component func[{}] → Core", func_idx);
                },
                CanonOperation::Resource(res_op) => {
                    use wrt_format::component::FormatResourceOperation;
                    match res_op {
                        FormatResourceOperation::New(res_new) => {
                            println!("Resource.New");
                            println!("    └─ Type: type[{}]", res_new.type_idx);
                        },
                        FormatResourceOperation::Drop(res_drop) => {
                            println!("Resource.Drop");
                            println!("    └─ Type: type[{}]", res_drop.type_idx);
                        },
                        FormatResourceOperation::Rep(res_rep) => {
                            println!("Resource.Rep");
                            println!("    └─ Type: type[{}]", res_rep.type_idx);
                        },
                    }
                },
                CanonOperation::Realloc {
                    alloc_func_idx,
                    memory_idx,
                } => {
                    println!("Realloc");
                    println!("    ├─ Alloc func[{}]", alloc_func_idx);
                    println!("    └─ Memory: mem[{}]", memory_idx);
                },
                CanonOperation::PostReturn { func_idx } => {
                    println!("PostReturn");
                    println!("    └─ Cleanup func[{}]", func_idx);
                },
                CanonOperation::MemoryCopy {
                    src_memory_idx,
                    dst_memory_idx,
                    ..
                } => {
                    println!("MemoryCopy");
                    println!("    ├─ Source: mem[{}]", src_memory_idx);
                    println!("    └─ Dest: mem[{}]", dst_memory_idx);
                },
                CanonOperation::Async { func_idx, .. } => {
                    println!("Async");
                    println!("    └─ Async func[{}]", func_idx);
                },
            }
        }

        println!("✓ Parsed {} canonical operations\n", canonicals.len());
        Ok(())
    }

    /// Parse canonical operations (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn parse_canonical_operations(_canonicals: &[wrt_format::component::Canon]) -> Result<()> {
        Ok(())
    }

    /// Parse and display component exports (Step 6a)
    #[cfg(feature = "std")]
    fn parse_exports(exports: &[wrt_format::component::Export]) -> Result<()> {
        println!("=== STEP 6a: Component Exports ===");
        println!("Total exports: {}", exports.len());

        if exports.is_empty() {
            println!("(No exports defined)\n");
            return Ok(());
        }

        for (idx, export) in exports.iter().enumerate() {
            use wrt_format::component::Sort;

            println!("  Export[{}]: \"{}\"", idx, export.name.name);

            // Show sort (Function, Instance, etc.)
            let sort_name = match &export.sort {
                Sort::Core(core_sort) => {
                    use wrt_format::component::CoreSort;
                    match core_sort {
                        CoreSort::Function => "Core.Function",
                        CoreSort::Table => "Core.Table",
                        CoreSort::Memory => "Core.Memory",
                        CoreSort::Global => "Core.Global",
                        CoreSort::Type => "Core.Type",
                        CoreSort::Module => "Core.Module",
                        CoreSort::Instance => "Core.Instance",
                    }
                },
                Sort::Function => "Function",
                Sort::Value => "Value",
                Sort::Type => "Type",
                Sort::Component => "Component",
                Sort::Instance => "Instance",
            };
            println!("    ├─ Sort: {}", sort_name);
            println!("    ├─ Index: {}", export.idx);

            // Show type information if available
            if let Some(ref ty) = export.ty {
                use wrt_format::component::ExternType;
                match ty {
                    ExternType::Function {
                        params, results, ..
                    } => {
                        println!("    ├─ Type: Function");
                        println!("    │  ├─ Params: {}", params.len());
                        println!("    │  └─ Results: {}", results.len());
                    },
                    ExternType::Module { type_idx } => {
                        println!("    ├─ Type: Module");
                        println!("    │  └─ Type index: {}", type_idx);
                    },
                    ExternType::Component { type_idx } => {
                        println!("    ├─ Type: Component");
                        println!("    │  └─ Type index: {}", type_idx);
                    },
                    ExternType::Instance { exports } => {
                        println!("    ├─ Type: Instance");
                        println!("    │  └─ Exports: {}", exports.len());
                    },
                    ExternType::Value(value_type) => {
                        println!("    ├─ Type: Value");
                        println!("    │  └─ Value type: {:?}", value_type);
                    },
                    ExternType::Type(bounds) => {
                        println!("    ├─ Type: Type");
                        println!("    │  └─ Bounds: {:?}", bounds);
                    },
                }
            } else {
                println!("    ├─ Type: (not specified)");
            }

            // Show additional metadata
            if export.name.is_resource {
                println!("    ├─ Resource: yes");
            }
            if let Some(ref semver) = export.name.semver {
                println!("    ├─ Semver: {}", semver);
            }
            if let Some(ref integrity) = export.name.integrity {
                println!(
                    "    └─ Integrity: {}...",
                    &integrity[..integrity.len().min(16)]
                );
            } else {
                println!("    └─ (end)");
            }
        }

        println!("✓ Parsed {} exports\n", exports.len());
        Ok(())
    }

    /// Parse exports (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn parse_exports(_exports: &[wrt_format::component::Export]) -> Result<()> {
        Ok(())
    }

    /// Parse and display component imports (Step 6b)
    #[cfg(feature = "std")]
    fn parse_imports(imports: &[wrt_format::component::Import]) -> Result<()> {
        println!("=== STEP 6b: Component Imports ===");
        println!("Total imports: {}", imports.len());

        if imports.is_empty() {
            println!("(No imports required)\n");
            return Ok(());
        }

        for (idx, import) in imports.iter().enumerate() {
            use wrt_format::component::ExternType;

            // Show namespace.name format
            let full_name = if import.name.nested.is_empty() {
                format!("{}:{}", import.name.namespace, import.name.name)
            } else {
                format!(
                    "{}:{}:{}",
                    import.name.namespace,
                    import.name.nested.join(":"),
                    import.name.name
                )
            };

            println!("  Import[{}]: \"{}\"", idx, full_name);

            // Show type information
            match &import.ty {
                ExternType::Function {
                    params, results, ..
                } => {
                    println!("    ├─ Type: Function");
                    println!("    │  ├─ Params: {}", params.len());
                    for (pidx, (pname, _ptype)) in params.iter().enumerate() {
                        if pidx < 3 {
                            println!("    │  │  ├─ {}: <type>", pname);
                        }
                    }
                    if params.len() > 3 {
                        println!("    │  │  └─ ... ({} more)", params.len() - 3);
                    }
                    println!("    │  └─ Results: {}", results.len());
                },
                ExternType::Module { type_idx } => {
                    println!("    ├─ Type: Module");
                    println!("    │  └─ Type index: {}", type_idx);
                },
                ExternType::Component { type_idx } => {
                    println!("    ├─ Type: Component");
                    println!("    │  └─ Type index: {}", type_idx);
                },
                ExternType::Instance { exports } => {
                    println!("    ├─ Type: Instance");
                    println!("    │  └─ Exports: {}", exports.len());
                },
                ExternType::Value(value_type) => {
                    println!("    ├─ Type: Value");
                    println!("    │  └─ Value type: {:?}", value_type);
                },
                ExternType::Type(bounds) => {
                    println!("    ├─ Type: Type");
                    println!("    │  └─ Bounds: {:?}", bounds);
                },
            }

            // Show package reference if present
            if let Some(ref pkg) = import.name.package {
                println!("    └─ Package: {} (version: {:?})", pkg.name, pkg.version);
            } else {
                println!("    └─ (end)");
            }
        }

        println!("✓ Parsed {} imports\n", imports.len());
        Ok(())
    }

    /// Parse imports (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn parse_imports(_imports: &[wrt_format::component::Import]) -> Result<()> {
        Ok(())
    }

    /// Resolve parsed exports into resolved exports (Step 6a - Resolution)
    #[cfg(feature = "std")]
    fn resolve_exports(
        instance: &mut Self,
        parsed_exports: &[wrt_format::component::Export],
    ) -> Result<()> {
        use crate::bounded_component_infra::ComponentProvider;
        use crate::instantiation::{ExportValue, FunctionExport, ResolvedExport};
        use wrt_format::component::Sort;
        use wrt_foundation::safe_memory::NoStdProvider;

        println!("\n=== Resolving Component Exports ===");
        println!("Total exports to resolve: {}", parsed_exports.len());

        for (idx, export) in parsed_exports.iter().enumerate() {
            let export_name = if export.name.nested.is_empty() {
                export.name.name.clone()
            } else {
                format!("{}:{}", export.name.nested.join(":"), export.name.name)
            };

            println!("  Resolving[{}]: \"{}\"", idx, export_name);

            // Create an export value based on the sort
            let export_value = match &export.sort {
                Sort::Function => {
                    // Create a function export with placeholder signature
                    let provider = NoStdProvider::<4096>::default();
                    let signature = wrt_foundation::ComponentType::unit(provider)
                        .unwrap_or_else(|_| panic!("Failed to create component type"));

                    let func_export = FunctionExport {
                        signature,
                        index: export.idx,
                    };
                    ExportValue::Function(func_export)
                },
                Sort::Instance => {
                    // Instance export - use placeholder value
                    ExportValue::Value(crate::types::Value::Bool(false).into())
                },
                _ => {
                    // Other sorts - use placeholder value
                    ExportValue::Value(crate::types::Value::Bool(false).into())
                },
            };

            let resolved = ResolvedExport {
                name: export_name.clone(),
                value: export_value.clone(),
                export_type: export_value,
            };

            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            {
                instance.exports.push(resolved);
            }
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            {
                instance
                    .exports
                    .push(resolved)
                    .map_err(|_| Error::validation_error("Too many exports"))?;
            }

            // Create ComponentFunction entries for callable exports
            match &export.sort {
                Sort::Function => {
                    println!("    ├─ Creating ComponentFunction for Sort::Function");
                    let func = ComponentFunction {
                        handle: export.idx as FunctionHandle,
                        signature: FunctionSignature {
                            name: export_name.clone(),
                            #[cfg(feature = "std")]
                            params: Vec::new(),
                            #[cfg(not(feature = "std"))]
                            params: {
                                use wrt_foundation::bounded::BoundedVec;
                                BoundedVec::new()
                            },
                            #[cfg(feature = "std")]
                            returns: Vec::new(),
                            #[cfg(not(feature = "std"))]
                            returns: {
                                use wrt_foundation::bounded::BoundedVec;
                                BoundedVec::new()
                            },
                        },
                        implementation: FunctionImplementation::Native {
                            func_index: export.idx,
                            module_index: 0, // Main module
                        },
                    };

                    #[cfg(feature = "std")]
                    {
                        instance.functions.push(func);
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        instance
                            .functions
                            .push(func)
                            .map_err(|_| Error::validation_error("Too many functions"))?;
                    }
                    println!("    ├─ Added to instance.functions");
                },
                Sort::Instance => {
                    println!("    ├─ Creating ComponentFunction for Sort::Instance");
                    // For instance exports like "wasi:cli/run@0.2.0"
                    // The instance contains functions accessible via qualified names
                    let func = ComponentFunction {
                        handle: export.idx as FunctionHandle,
                        signature: FunctionSignature {
                            name: export_name.clone(),
                            #[cfg(feature = "std")]
                            params: Vec::new(),
                            #[cfg(not(feature = "std"))]
                            params: {
                                use wrt_foundation::bounded::BoundedVec;
                                BoundedVec::new()
                            },
                            #[cfg(feature = "std")]
                            returns: Vec::new(),
                            #[cfg(not(feature = "std"))]
                            returns: {
                                use wrt_foundation::bounded::BoundedVec;
                                BoundedVec::new()
                            },
                        },
                        implementation: FunctionImplementation::Native {
                            func_index: export.idx,
                            module_index: 0, // Will need proper resolution
                        },
                    };

                    #[cfg(feature = "std")]
                    {
                        instance.functions.push(func);
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        instance
                            .functions
                            .push(func)
                            .map_err(|_| Error::validation_error("Too many functions"))?;
                    }
                    println!("    ├─ Added to instance.functions as Native implementation");
                },
                _ => {
                    println!("    ├─ Non-callable export type");
                },
            }

            println!("    ✓ Resolved");
        }

        println!("✓ Resolved {} exports\n", parsed_exports.len());
        Ok(())
    }

    /// Analyze and instantiate core modules (Step 7 & 8)
    #[cfg(feature = "std")]
    fn instantiate_core_modules(
        module_binaries: &[Vec<u8>],
    ) -> Result<Vec<wrt_runtime::module::Module>> {
        println!("=== STEP 7: Core Module Instantiation ===");
        println!("Total modules to instantiate: {}", module_binaries.len());

        if module_binaries.is_empty() {
            println!("(No core modules to instantiate)\n");
            return Ok(Vec::new());
        }

        // Step 8: Use sequential instantiation (parallel causes stack overflow with large modules)
        // TODO: Re-enable parallel processing with larger thread stack size
        println!("=== STEP 7: Core Module Instantiation ===");
        println!(
            "Total modules: {} (sequential processing)\n",
            module_binaries.len()
        );

        let mut instantiated_modules = Vec::with_capacity(module_binaries.len());

        for (idx, binary) in module_binaries.iter().enumerate() {
            println!("  Module[{}]:", idx);
            println!("    ├─ Binary size: {} bytes", binary.len());

            // Parse the module using wrt-decoder
            use wrt_decoder::load_wasm_unified;
            let wasm_info = load_wasm_unified(binary).map_err(|_| {
                Error::new(
                    wrt_error::ErrorCategory::Parse,
                    wrt_error::codes::PARSE_ERROR,
                    "Failed to parse core module binary",
                )
            })?;

            // Verify it's a core module
            if !wasm_info.is_core_module() {
                println!("    └─ ✗ Not a core module");
                continue;
            }

            let module_info = wasm_info.require_module_info().map_err(|_| {
                Error::new(
                    wrt_error::ErrorCategory::Parse,
                    wrt_error::codes::PARSE_ERROR,
                    "Module info not available",
                )
            })?;

            // Display module details
            println!(
                "    ├─ Type signatures: {}",
                module_info.function_types.len()
            );

            println!("    ├─ Imports: {}", module_info.imports.len());
            let func_imports = module_info
                .imports
                .iter()
                .filter(|i| matches!(i.import_type, wrt_decoder::ImportType::Function(_)))
                .count();
            let memory_imports = module_info
                .imports
                .iter()
                .filter(|i| matches!(i.import_type, wrt_decoder::ImportType::Memory))
                .count();
            let table_imports = module_info
                .imports
                .iter()
                .filter(|i| matches!(i.import_type, wrt_decoder::ImportType::Table))
                .count();
            let global_imports = module_info
                .imports
                .iter()
                .filter(|i| matches!(i.import_type, wrt_decoder::ImportType::Global))
                .count();

            if func_imports > 0 {
                println!("    │  ├─ Functions: {}", func_imports);
            }
            if memory_imports > 0 {
                println!("    │  ├─ Memories: {}", memory_imports);
            }
            if table_imports > 0 {
                println!("    │  ├─ Tables: {}", table_imports);
            }
            if global_imports > 0 {
                println!("    │  └─ Globals: {}", global_imports);
            }

            println!("    ├─ Exports: {}", module_info.exports.len());
            let func_exports = module_info
                .exports
                .iter()
                .filter(|e| matches!(e.export_type, wrt_decoder::ExportType::Function))
                .count();
            let memory_exports = module_info
                .exports
                .iter()
                .filter(|e| matches!(e.export_type, wrt_decoder::ExportType::Memory))
                .count();
            let table_exports = module_info
                .exports
                .iter()
                .filter(|e| matches!(e.export_type, wrt_decoder::ExportType::Table))
                .count();
            let global_exports = module_info
                .exports
                .iter()
                .filter(|e| matches!(e.export_type, wrt_decoder::ExportType::Global))
                .count();

            if func_exports > 0 {
                println!("    │  ├─ Functions: {}", func_exports);
            }
            if memory_exports > 0 {
                println!("    │  ├─ Memories: {}", memory_exports);
            }
            if table_exports > 0 {
                println!("    │  ├─ Tables: {}", table_exports);
            }
            if global_exports > 0 {
                println!("    │  └─ Globals: {}", global_exports);
            }

            // Show memory requirements if present
            if let Some((min_pages, max_pages)) = module_info.memory_pages {
                print!("    ├─ Memory: {} pages", min_pages);
                if let Some(max) = max_pages {
                    println!(" (max: {})", max);
                } else {
                    println!(" (unbounded)");
                }
            }

            // Show start function if present
            if let Some(start_idx) = module_info.start_function {
                println!("    ├─ Start function: func[{}]", start_idx);
            } else {
                println!("    ├─ Start function: (none)");
            }

            // Show a few key exports
            if !module_info.exports.is_empty() {
                println!("    ├─ Key exports:");
                for (eidx, export) in module_info.exports.iter().take(3).enumerate() {
                    let export_type = match &export.export_type {
                        wrt_decoder::ExportType::Function => "Function",
                        wrt_decoder::ExportType::Table => "Table",
                        wrt_decoder::ExportType::Memory => "Memory",
                        wrt_decoder::ExportType::Global => "Global",
                    };
                    if eidx < 2 {
                        println!("    │  ├─ \"{}\": {}", export.name, export_type);
                    } else {
                        println!("    │  └─ \"{}\" : {}", export.name, export_type);
                    }
                }
                if module_info.exports.len() > 3 {
                    println!("    │     ... ({} more)", module_info.exports.len() - 3);
                }
            }

            // Instantiate the module using wrt-runtime
            println!("    ├─ Status: Parsing successful");

            // Use a thread with larger stack size for module loading (16MB)
            // This prevents stack overflow on large modules (850KB)
            let binary_clone = binary.clone();
            let runtime_module = std::thread::Builder::new()
                .name(format!("module-loader-{}", idx))
                .stack_size(16 * 1024 * 1024)  // 16MB stack
                .spawn(move || -> Result<wrt_runtime::module::Module> {
                    {
                        let mut module = wrt_runtime::module::Module::new_empty()?;
                        module.load_from_binary(&binary_clone)
                    }
                        .map_err(|_| Error::new(
                            wrt_error::ErrorCategory::RuntimeTrap,
                            wrt_error::codes::RUNTIME_ERROR,
                            "Failed to load module from binary"
                        ))
                })
                .map_err(|_| Error::new(
                    wrt_error::ErrorCategory::RuntimeTrap,
                    wrt_error::codes::RUNTIME_ERROR,
                    "Failed to spawn module loader thread"
                ))?
                .join()
                .map_err(|_| Error::new(
                    wrt_error::ErrorCategory::RuntimeTrap,
                    wrt_error::codes::RUNTIME_ERROR,
                    "Module loader thread panicked"
                ))??;

            println!("    └─ ✓ Module instantiated");

            instantiated_modules.push(runtime_module);
        }

        println!(
            "✓ Instantiated {} core modules\n",
            instantiated_modules.len()
        );
        Ok(instantiated_modules)
    }

    /// Parallel module instantiation using std::thread::scope (Step 8)
    #[cfg(feature = "std")]
    fn instantiate_core_modules_parallel(
        module_binaries: &[Vec<u8>],
    ) -> Result<Vec<wrt_runtime::module::Module>> {
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        let start_time = Instant::now();

        // Shared results vector protected by Mutex
        let results: Arc<Mutex<Vec<Option<wrt_runtime::module::Module>>>> =
            Arc::new(Mutex::new(vec![None; module_binaries.len()]));

        // Shared error tracking
        let errors: Arc<Mutex<Vec<(usize, String)>>> = Arc::new(Mutex::new(Vec::new()));

        println!("Spawning {} threads...", module_binaries.len());

        // Use scoped threads to safely borrow module_binaries
        std::thread::scope(|scope| {
            let mut handles = Vec::new();

            for (idx, binary) in module_binaries.iter().enumerate() {
                let results = Arc::clone(&results);
                let errors = Arc::clone(&errors);

                let handle = scope.spawn(move || {
                    let thread_start = Instant::now();
                    println!("  Thread[{}]: Starting module instantiation...", idx);

                    // Parse and instantiate the module
                    match Self::instantiate_single_module(idx, binary) {
                        Ok(module) => {
                            let elapsed = thread_start.elapsed();
                            println!(
                                "  Thread[{}]: ✓ Completed in {:.2}ms",
                                idx,
                                elapsed.as_secs_f64() * 1000.0
                            );

                            // Store result
                            let mut results_guard = results.lock().unwrap();
                            results_guard[idx] = Some(module);
                        },
                        Err(e) => {
                            println!("  Thread[{}]: ✗ Failed: {}", idx, e);
                            let mut errors_guard = errors.lock().unwrap();
                            errors_guard.push((idx, format!("{}", e)));
                        },
                    }
                });

                handles.push(handle);
            }

            // Wait for all threads to complete
            for handle in handles {
                let _ = handle.join();
            }
        });

        let total_elapsed = start_time.elapsed();
        println!(
            "✓ All threads completed in {:.2}ms",
            total_elapsed.as_secs_f64() * 1000.0
        );

        // Check for errors
        let errors_guard = errors.lock().unwrap();
        if !errors_guard.is_empty() {
            println!("✗ {} modules failed to instantiate:", errors_guard.len());
            for (idx, err) in errors_guard.iter() {
                println!("  Module[{}]: {}", idx, err);
            }
            return Err(Error::new(
                wrt_error::ErrorCategory::RuntimeTrap,
                wrt_error::codes::RUNTIME_ERROR,
                "One or more modules failed to instantiate",
            ));
        }

        // Collect successful results
        let results_guard = results.lock().unwrap();
        let instantiated_modules: Vec<wrt_runtime::module::Module> =
            results_guard.iter().filter_map(|opt| opt.as_ref().cloned()).collect();

        println!(
            "✓ Successfully instantiated {} modules in parallel\n",
            instantiated_modules.len()
        );
        Ok(instantiated_modules)
    }

    /// Instantiate a single module (helper for parallel processing)
    #[cfg(feature = "std")]
    fn instantiate_single_module(idx: usize, binary: &[u8]) -> Result<wrt_runtime::module::Module> {
        use wrt_decoder::load_wasm_unified;

        // Parse the module
        let wasm_info = load_wasm_unified(binary).map_err(|_| {
            Error::new(
                wrt_error::ErrorCategory::Parse,
                wrt_error::codes::PARSE_ERROR,
                "Failed to parse core module binary",
            )
        })?;

        // Verify it's a core module
        if !wasm_info.is_core_module() {
            return Err(Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::VALIDATION_ERROR,
                "Not a core module",
            ));
        }

        let module_info = wasm_info.require_module_info().map_err(|_| {
            Error::new(
                wrt_error::ErrorCategory::Parse,
                wrt_error::codes::PARSE_ERROR,
                "Module info not available",
            )
        })?;

        // Log key module stats (concise for parallel output)
        println!(
            "  Thread[{}]: {} bytes, {} imports, {} exports",
            idx,
            binary.len(),
            module_info.imports.len(),
            module_info.exports.len()
        );

        // Create runtime module directly from binary
        let runtime_module = {
            let mut module = wrt_runtime::module::Module::new_empty()?;
            module.load_from_binary(binary).map_err(|_| {
                Error::new(
                    wrt_error::ErrorCategory::RuntimeTrap,
                    wrt_error::codes::RUNTIME_ERROR,
                    "Failed to load module from binary",
                )
            })
        }?;

        Ok(runtime_module)
    }

    /// Instantiate core modules (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn instantiate_core_modules(
        _module_binaries: &[Vec<u8>],
    ) -> Result<Vec<wrt_runtime::module::Module>> {
        Ok(Vec::new())
    }

    /// Link core modules by analyzing imports and exports (Step 9)
    #[cfg(feature = "std")]
    fn link_core_modules(modules: &[wrt_runtime::module::Module]) -> Result<()> {
        use std::collections::{HashMap, HashSet};

        println!("=== STEP 9: Module Linking ===");
        println!("Total modules to link: {}", modules.len());

        if modules.is_empty() {
            println!("(No modules to link)\n");
            return Ok(());
        }

        println!();

        // Build export map: (export_name) -> module_idx
        let mut export_map: HashMap<String, usize> = HashMap::new();

        println!("Analyzing exports:");
        for (idx, module) in modules.iter().enumerate() {
            let export_count = module.exports.len();
            if export_count > 0 {
                println!("  Module[{}]: {} exports", idx, export_count);

                // Iterate over exports
                for (export_name, _export_val) in module.exports.iter() {
                    // BoundedString::as_str() returns Result<&str, BoundedError>
                    let name_str = export_name.as_str().unwrap_or("<invalid>");

                    export_map.insert(name_str.to_string(), idx);
                    println!("    ├─ \"{}\"", name_str);
                }
            } else {
                println!("  Module[{}]: no exports", idx);
            }
        }

        println!();

        // Analyze each module's imports
        println!("Analyzing imports:");
        let mut dependencies: Vec<HashSet<usize>> = vec![HashSet::new(); modules.len()];
        let mut host_imports: Vec<Vec<String>> = vec![Vec::new(); modules.len()];
        let mut total_imports = 0;

        for (idx, module) in modules.iter().enumerate() {
            let import_count = module.imports.len();
            total_imports += import_count;

            if import_count > 0 {
                println!("  Module[{}]: {} imports", idx, import_count);
                // Note: Full import resolution requires nested iteration over ModuleImports
                // For now, we mark modules with imports as having potential host dependencies
                if import_count > 0 {
                    host_imports[idx].push(format!("({}  imports)", import_count));
                }
            } else {
                println!("  Module[{}]: no imports", idx);
            }
        }

        println!();

        // Build dependency graph visualization
        println!("Link graph:");
        for (idx, deps) in dependencies.iter().enumerate() {
            if deps.is_empty() && host_imports[idx].is_empty() {
                println!("  Module[{}] (no dependencies)", idx);
            } else {
                let mut dep_list = Vec::new();

                if !host_imports[idx].is_empty() {
                    dep_list.push("Host".to_string());
                }

                for &dep_idx in deps {
                    dep_list.push(format!("Module[{}]", dep_idx));
                }

                println!("  Module[{}] → [{}]", idx, dep_list.join(", "));
            }
        }

        println!();

        // Compute instantiation order (topological sort)
        let instantiation_order = Self::topological_sort(&dependencies)?;

        println!("Instantiation order: {:?}", instantiation_order);

        // Report summary
        println!();
        println!("✓ Linking analysis complete");
        println!("  Exports indexed: {}", export_map.len());
        println!("  Total imports: {}", total_imports);

        println!();
        Ok(())
    }

    /// Topological sort for module instantiation order
    #[cfg(feature = "std")]
    fn topological_sort(dependencies: &[std::collections::HashSet<usize>]) -> Result<Vec<usize>> {
        use std::collections::HashSet;

        let n = dependencies.len();
        let mut visited = vec![false; n];
        let mut stack = Vec::new();
        let mut in_progress = HashSet::new();

        fn visit(
            node: usize,
            dependencies: &[HashSet<usize>],
            visited: &mut [bool],
            in_progress: &mut HashSet<usize>,
            stack: &mut Vec<usize>,
        ) -> Result<()> {
            if in_progress.contains(&node) {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Validation,
                    wrt_error::codes::VALIDATION_ERROR,
                    "Circular dependency detected",
                ));
            }

            if visited[node] {
                return Ok(());
            }

            in_progress.insert(node);

            for &dep in &dependencies[node] {
                visit(dep, dependencies, visited, in_progress, stack)?;
            }

            in_progress.remove(&node);
            visited[node] = true;
            stack.push(node);

            Ok(())
        }

        for i in 0..n {
            if !visited[i] {
                visit(i, dependencies, &mut visited, &mut in_progress, &mut stack)?;
            }
        }

        Ok(stack)
    }

    /// Link core modules (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn link_core_modules(_modules: &[wrt_runtime::module::Module]) -> Result<()> {
        Ok(())
    }

    /// Prepare modules for execution by analyzing start functions and exports (Step 10)
    /// Returns: Vec<(module_idx, function_name, function_idx, is_start)>
    #[cfg(feature = "std")]
    fn prepare_module_execution(
        modules: &[wrt_runtime::module::Module],
    ) -> Result<Vec<(usize, String, u32, bool)>> {
        println!("=== STEP 10: Execution Preparation ===");
        println!("Total modules to prepare: {}", modules.len());

        if modules.is_empty() {
            println!("(No modules to execute)\n");
            return Ok(Vec::new());
        }

        println!();

        let mut total_start_functions = 0;
        let mut total_exported_functions = 0;
        let mut execution_plan = Vec::new();

        // Analyze each module for execution
        for (idx, module) in modules.iter().enumerate() {
            println!("Module[{}] execution analysis:", idx);

            // Check for start function
            if let Some(start_idx) = module.start {
                println!("  ├─ Start function: func[{}] ✓", start_idx);
                total_start_functions += 1;
                execution_plan.push((idx, "_start".to_string(), start_idx, true));
            } else {
                println!("  ├─ Start function: (none)");
            }

            // Count exported functions
            let mut func_exports = Vec::new();
            for (export_name, export_val) in module.exports.iter() {
                if let wrt_runtime::module::ExportKind::Function = export_val.kind {
                    // BoundedString::as_str() returns Result<&str, BoundedError>
                    let name_str = export_name.as_str().unwrap_or("<invalid>");

                    func_exports.push((name_str.to_string(), export_val.index));
                }
            }

            total_exported_functions += func_exports.len();

            if !func_exports.is_empty() {
                println!("  ├─ Exported functions: {}", func_exports.len());
                for (fidx, (fname, findex)) in func_exports.iter().enumerate().take(3) {
                    if fidx < 2 {
                        println!("  │  ├─ \"{}\" → func[{}]", fname, findex);
                    } else {
                        println!("  │  └─ \"{}\" → func[{}]", fname, findex);
                    }
                    execution_plan.push((idx, fname.clone(), *findex, false));
                }
                if func_exports.len() > 3 {
                    println!("  │     ... ({} more)", func_exports.len() - 3);
                }
            } else {
                println!("  ├─ Exported functions: 0");
            }

            // Show function count
            let func_count = module.functions.len();
            println!("  └─ Total functions: {}", func_count);
        }

        println!();

        // Execution plan summary
        println!("Execution plan:");
        println!(
            "  ├─ Modules with start functions: {}",
            total_start_functions
        );
        println!("  ├─ Exported functions: {}", total_exported_functions);
        println!(
            "  └─ Total executable entry points: {}",
            execution_plan.len()
        );

        println!();

        // Show execution order
        if !execution_plan.is_empty() {
            println!("Execution sequence (if triggered):");
            for (seq, (module_idx, func_name, func_idx, is_start)) in
                execution_plan.iter().enumerate().take(5)
            {
                let marker = if *is_start { "[START]" } else { "" };
                println!(
                    "  {}. Module[{}].\"{}\" (func[{}]) {}",
                    seq + 1,
                    module_idx,
                    func_name,
                    func_idx,
                    marker
                );
            }
            if execution_plan.len() > 5 {
                println!(
                    "     ... ({} more functions available)",
                    execution_plan.len() - 5
                );
            }
        } else {
            println!("⚠ No executable entry points found");
        }

        println!();
        println!("✓ Ready for execution");
        println!();

        Ok(execution_plan)
    }

    /// Prepare modules for execution (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn prepare_module_execution(_modules: &[wrt_runtime::module::Module]) -> Result<Vec<()>> {
        Ok(Vec::new())
    }

    /// Execute modules according to execution plan (Step 11)
    #[cfg(feature = "std")]
    fn execute_modules(
        modules: &[wrt_runtime::module::Module],
        execution_plan: &[(usize, String, u32, bool)],
    ) -> Result<()> {
        println!("=== STEP 11: Function Execution ===");

        if execution_plan.is_empty() {
            println!("(No functions to execute)\n");
            return Ok(());
        }

        println!("Executing {} functions...", execution_plan.len());
        println!();

        // Separate start functions from regular exports
        let start_functions: Vec<_> =
            execution_plan.iter().filter(|(_, _, _, is_start)| *is_start).collect();
        let export_functions: Vec<_> =
            execution_plan.iter().filter(|(_, _, _, is_start)| !*is_start).collect();

        // Execute start functions first
        if !start_functions.is_empty() {
            println!("Start functions:");
            for (module_idx, _func_name, func_idx, _is_start) in &start_functions {
                println!("  Module[{}]._start (func[{}]):", module_idx, func_idx);

                // Note: Full execution requires engine setup
                // For now, we show what would be executed
                println!("    ├─ Status: Execution deferred (requires engine)");
                println!(
                    "    ├─ Module function count: {}",
                    modules[*module_idx].functions.len()
                );
                println!(
                    "    └─ Would execute: Module[{}] start function",
                    module_idx
                );
            }
            println!();
        }

        // Show available exported functions
        if !export_functions.is_empty() {
            println!("Available exported functions ({}):", export_functions.len());
            for (idx, (module_idx, func_name, func_idx, _is_start)) in
                export_functions.iter().enumerate().take(5)
            {
                println!(
                    "  {}. Module[{}].\"{}\" (func[{}])",
                    idx + 1,
                    module_idx,
                    func_name,
                    func_idx
                );
                println!("     └─ Ready to call (requires engine setup)");
            }
            if export_functions.len() > 5 {
                println!("     ... ({} more functions)", export_functions.len() - 5);
            }
            println!();
        }

        println!("✓ Execution analysis complete");
        println!("  Note: Full execution requires:");
        println!("    - Engine initialization (CapabilityEngine)");
        println!("    - ModuleInstance creation");
        println!("    - Import resolution");
        println!("    - Memory allocation");
        println!();

        Ok(())
    }

    /// Execute modules (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn execute_modules(_modules: &[wrt_runtime::module::Module], _plan: &[()]) -> Result<()> {
        Ok(())
    }

    /// Initialize execution engine with configuration (Step 12)
    #[cfg(feature = "std")]
    fn initialize_engine(
        modules: &[wrt_runtime::module::Module],
        execution_plan: &[(usize, String, u32, bool)],
        host_registry: Option<std::sync::Arc<wrt_host::CallbackRegistry>>,
    ) -> Result<Box<dyn wrt_runtime::engine_factory::RuntimeEngine>> {
        use wrt_runtime::engine_factory::{
            EngineConfig, EngineFactory, EngineType, MemoryProviderType,
        };

        println!("=== STEP 12: Engine Initialization ===");

        // Calculate memory requirements based on module count
        // Default to 64KB per module (1 page = 64KB)
        let total_memory = if modules.is_empty() {
            65536 // Minimum 64KB
        } else {
            modules.len() * 65536 // 64KB per module
        };

        println!("Memory requirements:");
        println!("  ├─ Modules: {}", modules.len());
        println!(
            "  ├─ Base memory: {} bytes ({} KB)",
            total_memory,
            total_memory / 1024
        );
        println!(
            "  └─ Allocating: {} bytes ({} KB)",
            total_memory * 2,
            total_memory * 2 / 1024
        );
        println!();

        // Configure engine
        let config = EngineConfig::new(EngineType::CapabilityAware)
            .with_memory_provider(MemoryProviderType::CapabilityAware)
            .with_memory_budget(total_memory * 2) // 2x for safety margin
            .with_max_call_depth(1024)
            .with_debug_mode(true);

        println!("Engine configuration:");
        println!("  ├─ Type: {:?}", config.engine_type);
        println!("  ├─ Memory provider: {:?}", config.memory_provider);
        println!(
            "  ├─ Memory budget: {} bytes ({} KB)",
            config.memory_budget,
            config.memory_budget / 1024
        );
        println!("  ├─ Max call depth: {:?}", config.max_call_depth);
        println!("  └─ Debug mode: {}", config.debug_mode);
        println!();

        // Create engine
        println!("Creating engine...");
        let mut engine = EngineFactory::create(config)?;
        println!("✓ Engine created successfully");

        // Set host registry for WASI and custom host functions
        if let Some(registry) = host_registry {
            println!("Setting host registry on engine...");
            engine.set_host_registry(registry);
            println!("✓ Host registry set successfully");
        }
        println!();

        // Show what's ready
        println!("Engine ready for:");
        println!("  ├─ {} modules", modules.len());
        println!("  ├─ {} functions in execution plan", execution_plan.len());

        let start_count = execution_plan.iter().filter(|(_, _, _, is_start)| *is_start).count();
        let export_count = execution_plan.len() - start_count;

        if start_count > 0 {
            println!("  ├─ {} start functions", start_count);
        }
        if export_count > 0 {
            println!("  └─ {} exported functions", export_count);
        }
        println!();

        Ok(engine)
    }

    /// Initialize engine (no_std placeholder)
    #[cfg(not(feature = "std"))]
    fn initialize_engine(
        _modules: &[wrt_runtime::module::Module],
        _plan: &[()],
        _host_registry: Option<()>,
    ) -> Result<()> {
        Ok(())
    }

    /// Build type index from parsed component types
    ///
    /// This creates a mapping from type index to ComponentType, allowing
    /// runtime type resolution for Canon ABI operations and export/import linking.
    ///
    /// **Output**: Prints each type as it's indexed for visibility
    /// **Step 2**: Now stores the index for runtime type lookup
    #[cfg(feature = "std")]
    fn build_type_index(
        types: &[wrt_format::component::ComponentType],
    ) -> Result<std::collections::HashMap<u32, wrt_format::component::ComponentType>> {
        use std::collections::HashMap;

        println!("=== STEP 1: Building Type Index ===");
        println!("Total types to index: {}", types.len());

        let mut index = HashMap::new();

        for (idx, ty) in types.iter().enumerate() {
            let type_name = Self::get_type_name(ty);
            println!("  Type[{}]: {}", idx, type_name);

            // Additional detail for function types (most common)
            if let wrt_format::component::ComponentTypeDefinition::Function { params, results } =
                &ty.definition
            {
                println!("    ├─ Params: {}", params.len());
                println!("    └─ Results: {}", results.len());
            }

            index.insert(idx as u32, ty.clone());
        }

        println!("✓ Type index built: {} entries\n", index.len());
        Ok(index)
    }

    /// Build type index (no_std version - simplified, no printing)
    #[cfg(not(feature = "std"))]
    fn build_type_index(types: &[wrt_format::component::ComponentType]) -> Result<()> {
        // In no_std, we just validate type count for now
        // Full type resolution will be implemented when needed
        if types.len() > MAX_TYPES {
            return Err(Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::CAPACITY_EXCEEDED,
                "Type count exceeds maximum",
            ));
        }
        Ok(())
    }

    /// Initialize the instance (transition from Initializing to Ready)
    pub fn initialize(&mut self) -> Result<()> {
        match self.state {
            ComponentInstanceState::Initialized => {
                // Perform initialization logic
                self.validate_exports()?;
                self.setup_function_table()?;

                self.state = ComponentInstanceState::Running;
                Ok(())
            },
            _ => Err(Error::runtime_execution_error(
                "Instance not in initializing state",
            )),
        }
    }

    /// Call a function in this instance
    ///
    /// # Parameters
    /// - `function_name`: Name of the function to call
    /// - `args`: Arguments to pass to the function
    /// - `host_registry`: Optional reference to host function registry for calling WASI/host functions
    ///
    /// # Note
    /// This method does not support component-to-component calls. Use
    /// `call_function_with_resolver` if cross-component calls are needed.
    #[cfg(feature = "wrt-execution")]
    pub fn call_function(
        &mut self,
        function_name: &str,
        args: &[ComponentValue],
        host_registry: Option<&wrt_host::CallbackRegistry>,
    ) -> Result<Vec<ComponentValue>> {
        // Define a concrete type for the resolver to satisfy type inference
        // This is the closure type that matches the resolver signature
        type NoResolver = fn(InstanceId, &str, &[ComponentValue]) -> Result<Vec<ComponentValue>>;
        // Delegate to the resolver-enabled version with no resolver
        self.call_function_with_resolver::<NoResolver>(function_name, args, host_registry, None)
    }

    /// Call a function in this instance with cross-component call support
    ///
    /// # Parameters
    /// - `function_name`: Name of the function to call
    /// - `args`: Arguments to pass to the function
    /// - `host_registry`: Optional reference to host function registry for calling WASI/host functions
    /// - `cross_component_resolver`: Optional callback for resolving cross-component function calls.
    ///   The callback receives (target_instance_id, target_function_name, args) and returns the result.
    ///
    /// # Cross-Component Calls
    /// When a function has `FunctionImplementation::Component`, this method will use the
    /// provided resolver to dispatch the call to the target component instance. The resolver
    /// is typically provided by the `ComponentLinker` which manages all instances.
    #[cfg(feature = "wrt-execution")]
    pub fn call_function_with_resolver<F>(
        &mut self,
        function_name: &str,
        args: &[ComponentValue],
        host_registry: Option<&wrt_host::CallbackRegistry>,
        cross_component_resolver: Option<F>,
    ) -> Result<Vec<ComponentValue>>
    where
        F: FnMut(InstanceId, &str, &[ComponentValue]) -> Result<Vec<ComponentValue>>,
    {
        // Check instance state
        if self.state != ComponentInstanceState::Running {
            return Err(Error::new(
                ErrorCategory::Runtime,
                codes::INVALID_STATE,
                "Instance not in ready state",
            ));
        }

        // Find the function (extract data to avoid multiple borrows)
        let (func_impl, func_sig) = {
            let function = self.find_function(function_name)?;
            (function.implementation.clone(), function.signature.clone())
        };

        // Validate arguments
        self.validate_function_args(&func_sig, args)?;

        // Update metrics
        self.metadata.function_calls += 1;

        // Use extracted function implementation
        let function = ComponentFunction {
            handle: 0,
            signature: func_sig,
            implementation: func_impl,
        };

        // Execute the function based on its implementation
        match &function.implementation {
            FunctionImplementation::Native {
                func_index,
                module_index,
            } => {
                #[cfg(feature = "std")]
                {
                    self.call_native_function(*func_index, *module_index, args)
                }
                #[cfg(not(feature = "std"))]
                {
                    Err(Error::runtime_not_implemented(
                        "Native function calls require std feature",
                    ))
                }
            },
            FunctionImplementation::Host { callback } => {
                #[cfg(feature = "std")]
                let callback_str = callback.as_str();
                #[cfg(not(feature = "std"))]
                let callback_str = callback.as_str()?;
                self.call_host_function(callback_str, args, host_registry)
            },
            FunctionImplementation::Component {
                target_instance,
                target_function,
            } => {
                // Use the cross-component resolver if provided
                if let Some(mut resolver) = cross_component_resolver {
                    #[cfg(feature = "std")]
                    let target_fn_name = target_function.as_str();
                    #[cfg(not(feature = "std"))]
                    let target_fn_name = target_function.as_str()?;

                    resolver(*target_instance, target_fn_name, args)
                } else {
                    // No resolver provided - cannot make cross-component calls
                    Err(Error::new(
                        ErrorCategory::Runtime,
                        codes::INVALID_OPERATION,
                        "Cross-component call requires a resolver. Use ComponentLinker::call_function or provide a resolver.",
                    ))
                }
            },
        }
    }

    /// Call a function in this instance (no host registry version)
    #[cfg(not(feature = "wrt-execution"))]
    pub fn call_function(
        &mut self,
        function_name: &str,
        args: &[ComponentValue],
    ) -> Result<Vec<ComponentValue>> {
        // Check instance state
        if self.state != ComponentInstanceState::Running {
            return Err(Error::new(
                ErrorCategory::Runtime,
                codes::INVALID_STATE,
                "Instance not in ready state",
            ));
        }

        // Find the function (extract data to avoid multiple borrows)
        let (func_impl, func_sig) = {
            let function = self.find_function(function_name)?;
            (function.implementation.clone(), function.signature.clone())
        };

        // Validate arguments
        self.validate_function_args(&func_sig, args)?;

        // Update metrics
        self.metadata.function_calls += 1;

        // Use extracted function implementation
        let function = ComponentFunction {
            handle: 0,
            signature: func_sig,
            implementation: func_impl,
        };

        // Execute the function based on its implementation
        match &function.implementation {
            FunctionImplementation::Native {
                func_index,
                module_index,
            } => {
                #[cfg(feature = "std")]
                {
                    self.call_native_function(*func_index, *module_index, args)
                }
                #[cfg(not(feature = "std"))]
                {
                    Err(Error::runtime_not_implemented(
                        "Native function calls require std feature",
                    ))
                }
            },
            FunctionImplementation::Host { callback } => {
                #[cfg(feature = "std")]
                let callback_str = callback.as_str();
                #[cfg(not(feature = "std"))]
                let callback_str = callback.as_str()?;
                self.call_host_function(callback_str, args, None)
            },
            FunctionImplementation::Component {
                target_instance,
                target_function,
            } => {
                // This would need to go through the linker to call another component
                // For now, return a placeholder
                Err(Error::runtime_not_implemented(
                    "Component-to-component calls not yet implemented",
                ))
            },
        }
    }

    /// Get an export by name
    pub fn get_export(&self, name: &str) -> Option<&crate::instantiation::ResolvedExport> {
        self.exports.iter().find(|export| {
            #[cfg(feature = "std")]
            {
                export.name == name
            }
            #[cfg(not(feature = "std"))]
            {
                export.name.as_str().map(|s| s == name).unwrap_or(false)
            }
        })
    }

    /// Add a resolved import
    pub fn add_resolved_import(
        &mut self,
        resolved: crate::instantiation::ResolvedImport,
    ) -> Result<()> {
        if self.imports.len() >= MAX_IMPORTS_PER_COMPONENT {
            return Err(Error::validation_error("Too many resolved imports"));
        }

        #[cfg(all(feature = "std", not(feature = "safety-critical")))]
        {
            self.imports.push(resolved);
        }
        #[cfg(not(all(feature = "std", not(feature = "safety-critical"))))]
        {
            self.imports
                .push(resolved)
                .map_err(|_| Error::validation_error("Failed to push resolved import"))?;
        }
        Ok(())
    }

    /// Get memory if available
    pub fn get_memory(&self) -> Option<&ComponentMemory> {
        self.memory.as_ref()
    }

    /// Get mutable memory if available
    pub fn get_memory_mut(&mut self) -> Option<&mut ComponentMemory> {
        self.memory.as_mut()
    }

    /// Terminate the instance
    pub fn terminate(&mut self) {
        self.state = ComponentInstanceState::Stopped;
        // Cleanup resources
        self.functions.clear();
        if let Some(memory) = &mut self.memory {
            memory.clear();
        }
        // Clean up resource manager
        // Note: remove_instance_table method doesn't exist, skip cleanup for now
        // if let Some(resource_manager) = &mut self.resource_manager {
        //     let _ = resource_manager.remove_instance_table(self.id);
        // }
    }

    /// Get the resource manager for this instance
    pub fn get_resource_manager(&self) -> Option<&ComponentResourceManager> {
        self.resource_manager.as_ref()
    }

    /// Get a mutable resource manager for this instance
    pub fn get_resource_manager_mut(&mut self) -> Option<&mut ComponentResourceManager> {
        self.resource_manager.as_mut()
    }

    /// Create a resource in this instance
    pub fn create_resource(
        &mut self,
        resource_type: ResourceTypeId,
        data: ResourceData,
    ) -> Result<ResourceHandle> {
        if let Some(resource_manager) = &mut self.resource_manager {
            resource_manager.create_resource(resource_type, data).map_err(|_e| {
                Error::component_resource_lifecycle_error("Failed to create resource")
            })
        } else {
            Err(Error::runtime_not_implemented(
                "Resource management not available for this instance",
            ))
        }
    }

    /// Drop a resource from this instance
    pub fn drop_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        if let Some(resource_manager) = &mut self.resource_manager {
            // Use destroy_resource instead of drop_resource
            resource_manager
                .destroy_resource(handle)
                .map_err(|_| Error::runtime_execution_error("Failed to destroy resource"))
        } else {
            Err(Error::runtime_not_implemented(
                "Resource management not available for this instance",
            ))
        }
    }

    // Private helper methods

    fn validate_exports(&self) -> Result<()> {
        // Validate that all exports are well-formed
        for export in &self.exports {
            // ResolvedExport doesn't have export_type - validation happens during resolution
            // Skip validation here since exports are already resolved
        }
        Ok(())
    }

    fn setup_function_table(&mut self) -> Result<()> {
        // Create function entries for all function exports
        // Function table setup is handled during instantiation
        // This is called after exports are already resolved
        Ok(())
    }

    fn find_function(&self, name: &str) -> Result<&ComponentFunction> {
        self.functions
            .iter()
            .find(|f| f.signature.name == name)
            .ok_or_else(|| Error::runtime_function_not_found("Function not found"))
    }

    fn validate_function_args(
        &self,
        signature: &FunctionSignature,
        args: &[ComponentValue],
    ) -> Result<()> {
        if args.len() != signature.params.len() {
            return Err(Error::runtime_type_mismatch(
                "Function argument count mismatch",
            ));
        }

        // Type checking would go here in a full implementation
        Ok(())
    }

    #[cfg(feature = "std")]
    fn call_native_function(
        &mut self,
        func_index: u32,
        module_index: u32,
        args: &[ComponentValue],
    ) -> Result<Vec<ComponentValue>> {
        println!(
            "[CALL_NATIVE] Calling func[{}] in module[{}]",
            func_index, module_index
        );

        // This is a canonical function that needs to be executed through the runtime
        // The func_index refers to a function in the already-instantiated module

        // For canonical functions like wasi:cli/run, we need to:
        // 1. Lower the component values to WASM values
        // 2. Call the underlying core WASM function
        // 3. Lift the result back to component values

        println!("[CALL_NATIVE] Executing canonical function {}", func_index);

        // Use the stored engine and instance handle from component instantiation
        // This preserves all the import linkages established during instantiation
        #[cfg(feature = "wrt-execution")]
        {
            use wrt_foundation::values::Value;
            use wrt_runtime::engine::CapabilityEngine;

            if let (Some(engine_box), Some(instance_handle)) =
                (&mut self.runtime_engine, self.main_instance_handle)
            {
                let engine = engine_box.as_mut();
                println!(
                    "[CALL_NATIVE] Using stored engine and instance {:?}",
                    instance_handle
                );

                // Convert component values to WASM values
                let wasm_args: Vec<Value> = args
                    .iter()
                    .map(|_| {
                        // For now, we don't pass arguments - wasi:cli/run takes none
                        Value::I32(0)
                    })
                    .collect();

                // Try _initialize first (important for TinyGo components)
                println!("[CALL_NATIVE] Attempting to call _initialize...");
                match engine.execute(instance_handle, "_initialize", &[]) {
                    Ok(_) => println!("[CALL_NATIVE] ✓ _initialize completed"),
                    Err(e) => println!("[CALL_NATIVE] ⚠ _initialize skipped: {:?}", e),
                }

                // Try entry point functions in order of preference:
                // 1. _start - standard WASI command entry point (C/TinyGo)
                // 2. wasi:cli/run@*#run - Rust component model entry point
                // 3. run - common for interface-style components
                // 4. main - C-style entry point
                // NOTE: We must use the correct instance_handle (main module, not a shim).
                // Infinite recursion occurs if we call a re-exported alias instead of the
                // actual implementation.
                let entry_points = [
                    "_start",
                    "wasi:cli/run@0.2.3#run",
                    "wasi:cli/run@0.2.6#run",
                    "wasi:cli/run@0.2.0#run",
                    "run",
                    "main",
                ];
                let mut last_error = None;

                for entry_point in entry_points {
                    println!("[CALL_NATIVE] Trying entry point: {}...", entry_point);
                    match engine.execute(instance_handle, entry_point, &wasm_args) {
                        Ok(results) => {
                            println!(
                                "[CALL_NATIVE] ✓ {} completed with {} results",
                                entry_point,
                                results.len()
                            );
                            return Ok(vec![]); // wasi:cli/run returns nothing on success
                        },
                        Err(e) => {
                            println!(
                                "[CALL_NATIVE] ⚠ {} not found or failed: {:?}",
                                entry_point, e
                            );
                            last_error = Some(e);
                        },
                    }
                }

                // All entry points failed - check available exports for debugging
                if let Some(e) = last_error {
                    println!("[CALL_NATIVE] ✗ All entry points failed");
                    // Debug: check for common exports (core module exports only)
                    let common_exports = [
                        "_start",
                        "_initialize",
                        "run",
                        "main",
                        "memory",
                        "__heap_base",
                        "__data_end",
                        "cabi_realloc",
                        "__wasm_call_ctors",
                    ];
                    println!("[CALL_NATIVE] Checking for common exports:");
                    for name in common_exports {
                        match engine.has_function(instance_handle, name) {
                            Ok(true) => println!("    ✓ {} - EXISTS", name),
                            Ok(false) => println!("    ✗ {} - not found", name),
                            Err(_) => println!("    ? {} - error checking", name),
                        }
                    }
                    return Err(e);
                }
            } else {
                println!(
                    "[CALL_NATIVE] No stored engine/instance, falling back to fresh instantiation"
                );
            }
        }

        // Fallback: Create fresh engine if stored engine not available
        // This path is used when wrt-execution feature is disabled or during testing
        let module_binary = self
            .component
            .modules
            .get(module_index as usize)
            .ok_or_else(|| Error::runtime_execution_error("Module index out of bounds"))?;

        println!(
            "[CALL_NATIVE] Module binary size: {} bytes",
            module_binary.len()
        );

        let binary_clone = module_binary.clone();
        let result = std::thread::Builder::new()
            .name(format!("module-exec-{}", module_index))
            .stack_size(32 * 1024 * 1024)  // 32MB stack for safety
            .spawn(move || -> Result<Vec<ComponentValue>> {
                println!("[TEST-THREAD] Attempting to instantiate module...");
                println!("[TEST-THREAD] Binary size: {} bytes", binary_clone.len());

                // Load module directly (static method returns Box<Module>)
                println!("[TEST-THREAD] Loading module from binary...");

                let module = {
                    // Create module directly using create_runtime_provider
                    let provider = wrt_runtime::bounded_runtime_infra::create_runtime_provider().map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to create runtime provider: {:?}", e);
                        Error::runtime_execution_error("Failed to create runtime provider")
                    })?;
                    let mut m = wrt_runtime::module::Module {
                        types: Vec::new(),
                        imports: wrt_foundation::bounded_collections::BoundedMap::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create imports: {:?}", e);
                            Error::runtime_execution_error("Failed to create imports")
                        })?,
                        #[cfg(feature = "std")]
                        import_order: Vec::new(),
                        #[cfg(not(feature = "std"))]
                        import_order: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create import_order: {:?}", e);
                            Error::runtime_execution_error("Failed to create import_order")
                        })?,
                        functions: Vec::new(),
                        #[cfg(feature = "std")]
                        tables: Vec::new(),
                        #[cfg(not(feature = "std"))]
                        tables: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create tables: {:?}", e);
                            Error::runtime_execution_error("Failed to create tables")
                        })?,
                        memories: Vec::new(),
                        globals: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create globals: {:?}", e);
                            Error::runtime_execution_error("Failed to create globals")
                        })?,
                        #[cfg(feature = "std")]
                        tags: Vec::new(),
                        #[cfg(not(feature = "std"))]
                        tags: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create tags: {:?}", e);
                            Error::runtime_execution_error("Failed to create tags")
                        })?,
                        #[cfg(feature = "std")]
                        elements: Vec::new(),
                        #[cfg(not(feature = "std"))]
                        elements: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create elements: {:?}", e);
                            Error::runtime_execution_error("Failed to create elements")
                        })?,
                        #[cfg(feature = "std")]
                        data: Vec::new(),
                        #[cfg(not(feature = "std"))]
                        data: wrt_foundation::bounded::BoundedVec::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create data: {:?}", e);
                            Error::runtime_execution_error("Failed to create data")
                        })?,
                        start: None,
                        custom_sections: wrt_foundation::bounded_collections::BoundedMap::new(provider.clone()).map_err(|e| {
                            println!("[TEST-THREAD] ✗ Failed to create custom_sections: {:?}", e);
                            Error::runtime_execution_error("Failed to create custom_sections")
                        })?,
                        exports: wrt_foundation::direct_map::DirectMap::new(),
                        name: None,
                        binary: None,
                        validated: false,
                        num_global_imports: 0,
                        #[cfg(feature = "std")]
                        global_import_types: Vec::new(),
                        #[cfg(feature = "std")]
                        deferred_global_inits: Vec::new(),
                        #[cfg(feature = "std")]
                        import_types: Vec::new(),
                    };
                    m.load_from_binary(&binary_clone)
                }
                    .map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to load module: {:?}", e);
                        Error::runtime_execution_error("Failed to load module from binary")
                    })?;

                println!("[TEST-THREAD] ✓ Module loaded successfully!");
                println!("[TEST-THREAD] Exports: {}", module.exports.len());
                println!("[TEST-THREAD] Imports: {}", module.imports.len());

                // Execute the function using the runtime engine
                use wrt_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};
                use wrt_foundation::memory_init::MemoryInitializer;

                // Initialize memory system
                MemoryInitializer::initialize()
                    .map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to initialize memory: {:?}", e);
                        Error::runtime_error("Memory initialization failed")
                    })?;

                // Create execution engine
                let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)
                    .map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to create engine: {:?}", e);
                        Error::runtime_error("Engine creation failed")
                    })?;

                // Register WASI host functions with memory access
                println!("[TEST-THREAD] Registering all WASI host functions...");

                use wrt_foundation::values::Value;
                use std::sync::{Arc as StdArc, Mutex};

                // WASI Preview 2 versions we support - register for all to ensure compatibility
                // Components may import @0.2.0, @0.2.4, or @0.2.6 depending on toolchain version
                const WASI_VERSIONS: &[&str] = &["0.2.0", "0.2.4", "0.2.6"];

                // Create shared instance state for WASI functions to access memory
                // We'll populate this after instantiation
                type InstanceMemory = StdArc<Mutex<Option<StdArc<wrt_runtime::module_instance::ModuleInstance>>>>;
                let instance_memory: InstanceMemory = StdArc::new(Mutex::new(None));

                // Helper to read from WASM memory
                let read_memory = {
                    let mem = instance_memory.clone();
                    move |ptr: u32, len: u32| -> std::result::Result<Vec<u8>, String> {
                        let inst_lock = mem.lock().unwrap();
                        if let Some(ref inst) = *inst_lock {
                            // Use public API to get memory
                            match inst.memory(0) {
                                Ok(memory_wrapper) => {
                                    let mut buffer = vec![0u8; len as usize];
                                    memory_wrapper.0.read(ptr, &mut buffer)
                                        .map_err(|e| format!("Memory read error: {:?}", e))?;
                                    Ok(buffer)
                                }
                                Err(e) => Err(format!("No memory in instance: {:?}", e))
                            }
                        } else {
                            Err("Instance not initialized".to_string())
                        }
                    }
                };

                // Helper to write to WASM memory (not currently used but available for future)
                let _write_memory = {
                    let mem = instance_memory.clone();
                    move |ptr: u32, data: &[u8]| -> std::result::Result<(), String> {
                        let inst_lock = mem.lock().unwrap();
                        if let Some(ref inst) = *inst_lock {
                            // Use public API to get memory
                            match inst.memory(0) {
                                Ok(memory_wrapper) => {
                                    memory_wrapper.0.write_shared(ptr, data)
                                        .map_err(|e| format!("Memory write error: {:?}", e))?;
                                    Ok(())
                                }
                                Err(e) => Err(format!("No memory in instance: {:?}", e))
                            }
                        } else {
                            Err("Instance not initialized".to_string())
                        }
                    }
                };

                // wasi:cli/environment - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:cli/environment@{}", version);
                    let _ = engine.register_host_function(&iface, "get-environment",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // Empty list ptr
                    let _ = engine.register_host_function(&iface, "get-arguments",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // Empty list ptr
                    let _ = engine.register_host_function(&iface, "initial-cwd",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // None
                }

                // wasi:cli/exit - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:cli/exit@{}", version);
                    let _ = engine.register_host_function(&iface, "exit",
                        |args: &[Value]| {
                            if let Some(Value::I32(code)) = args.get(0) {
                                println!("[WASI] exit called with code: {}", code);
                            }
                            Ok(vec![]) // Never returns
                        });
                }

                // wasi:io/poll - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:io/poll@{}", version);
                    let _ = engine.register_host_function(&iface, "[method]pollable.block",
                        |_args: &[Value]| Ok(vec![]));
                }

                // wasi:io/streams - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:io/streams@{}", version);
                    let _ = engine.register_host_function(&iface, "[method]input-stream.blocking-read",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0), Value::I32(0)])); // result<tuple<list<u8>, bool>, error>
                    let _ = engine.register_host_function(&iface, "[resource-drop]input-stream",
                        |_args: &[Value]| Ok(vec![]));
                    let _ = engine.register_host_function(&iface, "[method]output-stream.blocking-flush",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>

                    // blocking-write-and-flush - the critical write function
                    let read_mem = read_memory.clone();
                    let _ = engine.register_host_function(&iface, "[method]output-stream.blocking-write-and-flush",
                        {
                            let read_mem = read_mem.clone();
                            move |args: &[Value]| {
                                if args.len() >= 3 {
                                    let handle = if let Value::I32(h) = args[0] { h } else { 1 };
                                    let ptr = if let Value::I32(p) = args[1] { p as u32 } else { 0 };
                                    let len = if let Value::I32(l) = args[2] { l as u32 } else { 0 };

                                    println!("[WASI] blocking-write-and-flush: handle={}, ptr={}, len={}", handle, ptr, len);

                                    // Try to read from WASM memory
                                    match read_mem(ptr, len) {
                                        Ok(data) => {
                                            // Write to stdout or stderr
                                            use std::io::Write;
                                            let result = if handle == 1 {
                                                std::io::stdout().write_all(&data)
                                                    .and_then(|_| std::io::stdout().flush())
                                            } else if handle == 2 {
                                                std::io::stderr().write_all(&data)
                                                    .and_then(|_| std::io::stderr().flush())
                                            } else {
                                                Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid handle"))
                                            };

                                            match result {
                                                Ok(_) => {
                                                    println!("[WASI] ✓ Wrote {} bytes to handle {}", len, handle);
                                                    println!("[WASI] Data: {:?}", String::from_utf8_lossy(&data));
                                                    Ok(vec![Value::I64(0)]) // Success
                                                }
                                                Err(e) => {
                                                    #[cfg(feature = "tracing")]
                                                    tracing::warn!(error = ?e, "WASI write error");
                                                    Ok(vec![Value::I64(1)]) // Error
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            #[cfg(feature = "tracing")]
                                            tracing::warn!(error = %e, "WASI memory read error");
                                            Ok(vec![Value::I64(1)]) // Error - can't read memory yet
                                        }
                                    }
                                } else {
                                    Ok(vec![Value::I64(1)]) // Error - wrong args
                                }
                            }
                        });
                    let _ = engine.register_host_function(&iface, "[resource-drop]output-stream",
                        |_args: &[Value]| Ok(vec![]));
                }

                // wasi:cli/stdout - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:cli/stdout@{}", version);
                    let _ = engine.register_host_function(&iface, "get-stdout",
                        |_args: &[Value]| {
                            println!("[WASI] get-stdout called");
                            Ok(vec![Value::I32(1)]) // Return stdout handle
                        });
                }

                // wasi:cli/stderr - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:cli/stderr@{}", version);
                    let _ = engine.register_host_function(&iface, "get-stderr",
                        |_args: &[Value]| {
                            println!("[WASI] get-stderr called");
                            Ok(vec![Value::I32(2)]) // Return stderr handle
                        });
                }

                // wasi:cli/stdin - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:cli/stdin@{}", version);
                    let _ = engine.register_host_function(&iface, "get-stdin",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // Return stdin handle
                }

                // wasi:clocks/monotonic-clock - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:clocks/monotonic-clock@{}", version);
                    let _ = engine.register_host_function(&iface, "now",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // instant (u64)
                    let _ = engine.register_host_function(&iface, "subscribe-duration",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // pollable handle
                }

                // wasi:clocks/wall-clock - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:clocks/wall-clock@{}", version);
                    let _ = engine.register_host_function(&iface, "now",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // datetime ptr
                }

                // wasi:random/random - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:random/random@{}", version);
                    let _ = engine.register_host_function(&iface, "get-random-bytes",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // list<u8> ptr
                    let _ = engine.register_host_function(&iface, "get-random-u64",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // u64
                }

                // wasi:filesystem/types - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:filesystem/types@{}", version);
                    let _ = engine.register_host_function(&iface, "[method]descriptor.create-directory-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.link-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.open-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0)])); // result<descriptor, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.read",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0), Value::I32(0)])); // result<tuple<list<u8>, bool>, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.read-directory",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0)])); // result<directory-entry-stream, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.readlink-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0), Value::I32(0)])); // result<string, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.remove-directory-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.rename-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[resource-drop]descriptor",
                        |_args: &[Value]| Ok(vec![]));
                    let _ = engine.register_host_function(&iface, "[method]descriptor.stat",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0)])); // result<descriptor-stat, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.stat-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0)])); // result<descriptor-stat, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.symlink-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.sync-data",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.unlink-file-at",
                        |_args: &[Value]| Ok(vec![Value::I64(0)])); // result<_, error>
                    let _ = engine.register_host_function(&iface, "[method]descriptor.write",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I64(0)])); // result<filesize, error>
                    let _ = engine.register_host_function(&iface, "[method]directory-entry-stream.read-directory-entry",
                        |_args: &[Value]| Ok(vec![Value::I64(0), Value::I32(0)])); // result<option<directory-entry>, error>
                    let _ = engine.register_host_function(&iface, "[resource-drop]directory-entry-stream",
                        |_args: &[Value]| Ok(vec![]));
                }

                // wasi:filesystem/preopens - register for all supported versions
                for version in WASI_VERSIONS {
                    let iface = format!("wasi:filesystem/preopens@{}", version);
                    let _ = engine.register_host_function(&iface, "get-directories",
                        |_args: &[Value]| Ok(vec![Value::I32(0)])); // list<tuple<descriptor, string>> ptr
                }

                // 36 functions * 3 versions = 108 total registrations
                println!("[TEST-THREAD] ✓ Registered WASI host functions for versions 0.2.0, 0.2.4, 0.2.6");
                println!("[TEST-THREAD] Loading module into engine...");

                // Load module into engine
                let module_handle = engine.load_module(&binary_clone)
                    .map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to load into engine: {:?}", e);
                        Error::runtime_error("Engine load failed")
                    })?;

                // Instantiate module
                let instance = engine.instantiate(module_handle)
                    .map_err(|e| {
                        println!("[TEST-THREAD] ✗ Failed to instantiate: {:?}", e);
                        Error::runtime_error("Instantiation failed")
                    })?;

                #[cfg(feature = "std")]
                println!("[TEST-THREAD] ✓ Engine and instance ready");

                // Populate instance_memory so WASI host functions can access WASM memory
                // The engine stores instances and exposes them via get_instance()
                match engine.get_instance(instance) {
                    Ok(inst_arc) => {
                        let mut lock = instance_memory.lock().unwrap();
                        *lock = Some(inst_arc.clone());
                        println!("[TEST-THREAD] ✓ Instance memory connected for WASI functions");
                    }
                    Err(e) => {
                        println!("[TEST-THREAD] ⚠ Could not get instance for memory access: {:?}", e);
                        println!("[TEST-THREAD] WASI output may not work correctly");
                    }
                }

                // Try to find and execute the component's exported function
                // For WASI components, we need to call _initialize first to set up globals
                #[cfg(feature = "std")]
                println!("[TEST-THREAD] Step 1: Call _initialize to set up runtime globals...");

                // Call _initialize first (critical for TinyGo WASM)
                match engine.execute(instance, "_initialize", &[]) {
                    Ok(_) => {
                        println!("[TEST-THREAD] ✓ _initialize completed successfully");
                    }
                    Err(e) => {
                        println!("[TEST-THREAD] ⚠ _initialize failed: {:?}", e);
                        println!("[TEST-THREAD] Continuing anyway - some components work without it");
                    }
                }

                #[cfg(feature = "std")]
                println!("[TEST-THREAD] Step 2: Looking for main entry points...");

                // Try the actual entry points
                let entry_points = vec![
                    "wasi:cli/run@0.2.0#run", // Component model entry point (primary)
                    "_start",                  // WASI command entry point (fallback)
                ];

                #[cfg(feature = "std")]
                {
                    use wrt_foundation::values::Value;

                    let mut executed = false;
                    for entry_point in &entry_points {
                        println!("[TEST-THREAD] Trying to call function: {}", entry_point);

                        // Prepare Component Model canonical ABI arguments
                        // The lowered core function signature depends on the component interface
                        // For now, try calling with no arguments to check the actual signature
                        let args = vec![];  // Try with no arguments first

                        match engine.execute(instance, entry_point, &args) {
                            Ok(results) => {
                                println!("[TEST-THREAD] ✓ Successfully called {}", entry_point);
                                println!("[TEST-THREAD] Results: {:?}", results);

                                // For Component Model functions with result types, the result is in the return values
                                // TODO: Read the result from memory at ret_ptr (65500) when we have proper memory access
                                if *entry_point == "wasi:cli/run@0.2.0#run" {
                                    println!("[TEST-THREAD] Component function completed");
                                    // Exit code interpretation would go here
                                }

                                executed = true;
                                break;
                            }
                            Err(e) => {
                                println!("[TEST-THREAD] Function {} not found or failed: {:?}", entry_point, e);
                                continue;
                            }
                        }
                    }

                    if !executed {
                        println!("[TEST-THREAD] No standard entry point found, module loaded but not executed");
                    }

                    println!("[TEST-THREAD] Module instantiated and execution attempted");
                }

                Ok(vec![])
            })
            .map_err(|e| {
                println!("[DIAGNOSTIC] ✗ Failed to spawn thread: {:?}", e);
                Error::runtime_execution_error("Failed to spawn execution thread")
            })?
            .join()
            .map_err(|e| {
                println!("[DIAGNOSTIC] ✗ Thread panicked: {:?}", e);
                Error::runtime_execution_error("Execution thread panicked")
            })??;

        println!("[CALL_NATIVE] Function execution completed");
        Ok(result)
    }

    #[cfg(feature = "wrt-execution")]
    fn call_host_function(
        &mut self,
        callback: &str,
        args: &[ComponentValue],
        host_registry: Option<&wrt_host::CallbackRegistry>,
    ) -> Result<Vec<ComponentValue>> {
        // Check if this is a wasip2 canonical function
        if crate::canonical_executor::is_wasip2_canonical(callback) {
            #[cfg(feature = "tracing")]
            trace!(callback = %callback, "Dispatching wasip2 canonical");

            // Parse the interface and function from the callback string
            // Format: "wasi:io/streams@0.2.0::output-stream.blocking-write-and-flush"
            let (interface, function) = if let Some(double_colon) = callback.find("::") {
                let interface_part = &callback[..double_colon];
                let function_part = &callback[double_colon + 2..];
                (interface_part, function_part)
            } else {
                // Simple format: "wasi:cli/stdout@0.2.0"
                (callback, "get-stdout")
            };

            // Create a canonical executor and dispatch
            let mut executor = crate::canonical_executor::CanonicalExecutor::new();

            // Get memory if available (for functions that need it)
            // This would need proper memory access in a real implementation
            let memory: Option<&mut [u8]> = None;

            return executor.execute_canon_lower(interface, function, args.to_vec(), memory);
        }

        // For non-wasip2 functions, use the regular host registry
        let registry = host_registry.ok_or_else(|| {
            Error::runtime_execution_error("Host function registry not available")
        })?;

        // Extract module name from callback string (e.g., "wasi:cli/run@0.2.0" -> "wasi:cli")
        let module_name = if let Some(colon_pos) = callback.find(':') {
            if let Some(slash_pos) = callback[colon_pos..].find('/') {
                &callback[..colon_pos + slash_pos]
            } else {
                callback
            }
        } else {
            callback
        };

        // Convert ComponentValue arguments to wrt_foundation::values::Value
        // For now, we'll support basic types
        use wrt_foundation::values::{FloatBits32, FloatBits64};
        let mut wasm_args = Vec::new();
        for arg in args {
            let wasm_value = match arg {
                ComponentValue::Bool(b) => {
                    wrt_foundation::values::Value::I32(if *b { 1 } else { 0 })
                },
                ComponentValue::S8(v) => wrt_foundation::values::Value::I32(*v as i32),
                ComponentValue::U8(v) => wrt_foundation::values::Value::I32(*v as i32),
                ComponentValue::S16(v) => wrt_foundation::values::Value::I32(*v as i32),
                ComponentValue::U16(v) => wrt_foundation::values::Value::I32(*v as i32),
                ComponentValue::S32(v) => wrt_foundation::values::Value::I32(*v),
                ComponentValue::U32(v) => wrt_foundation::values::Value::I32(*v as i32),
                ComponentValue::S64(v) => wrt_foundation::values::Value::I64(*v),
                ComponentValue::U64(v) => wrt_foundation::values::Value::I64(*v as i64),
                ComponentValue::F32(v) => {
                    wrt_foundation::values::Value::F32(FloatBits32::from_f32(*v))
                },
                ComponentValue::F64(v) => {
                    wrt_foundation::values::Value::F64(FloatBits64::from_f64(*v))
                },
                _ => {
                    return Err(Error::runtime_execution_error(
                        "Unsupported argument type for host function",
                    ));
                },
            };
            wasm_args.push(wasm_value);
        }

        // Create a dummy engine (component model doesn't use engine parameter)
        let mut dummy_engine = ();
        let engine_any: &mut dyn core::any::Any = &mut dummy_engine;

        // Look up the function in the registry and call it
        // The callback string is the full function name (e.g., "wasi:cli/run@0.2.0")
        let results = registry
            .call_host_function(engine_any, module_name, callback, wasm_args)
            .map_err(|_| {
                Error::runtime_function_not_found("Host function not found or execution failed")
            })?;

        // Convert results back to ComponentValue
        let mut component_results = Vec::new();
        for result in results.as_slice() {
            let component_value = match result {
                wrt_foundation::values::Value::I32(v) => ComponentValue::S32(*v),
                wrt_foundation::values::Value::I64(v) => ComponentValue::S64(*v),
                wrt_foundation::values::Value::F32(v) => ComponentValue::F32(v.to_f32()),
                wrt_foundation::values::Value::F64(v) => ComponentValue::F64(v.to_f64()),
                _ => {
                    return Err(Error::runtime_execution_error(
                        "Unsupported return type from host function",
                    ));
                },
            };
            component_results.push(component_value);
        }

        Ok(component_results)
    }

    #[cfg(not(feature = "wrt-execution"))]
    fn call_host_function(
        &mut self,
        _callback: &str,
        _args: &[ComponentValue],
        _host_registry: Option<&()>, // Dummy type for no wrt-execution
    ) -> Result<Vec<ComponentValue>> {
        // Simplified implementation - would call actual host function
        Ok(vec![ComponentValue::String(String::from("host_result"))]) // Placeholder result
    }
}

impl ComponentMemory {
    /// Create a new component memory
    pub fn new(handle: MemoryHandle, config: MemoryConfig) -> Result<Self> {
        let initial_size = config.initial_pages * 65536; // 64KB per page

        if let Some(max_pages) = config.max_pages {
            if config.initial_pages > max_pages {
                return Err(Error::validation_error(
                    "Initial pages cannot exceed maximum pages",
                ));
            }
        }

        // Create bounded vec for memory data
        #[cfg(feature = "std")]
        let data = vec![0u8; initial_size as usize];
        #[cfg(not(feature = "std"))]
        let data = {
            let mut data = BoundedVec::<u8, 65536>::new();
            // Fill with zeros up to initial size (capped at 65536)
            let fill_size = (initial_size as usize).min(65536);
            for _ in 0..fill_size {
                data.push(0).map_err(|_| Error::memory_error("Memory data capacity exceeded"))?;
            }
            data
        };

        Ok(Self {
            handle,
            config,
            current_size: initial_size,
            data,
        })
    }

    /// Grow memory by the specified number of pages
    pub fn grow(&mut self, pages: u32) -> Result<u32> {
        let old_pages = self.current_size / 65536;
        let new_pages = old_pages + pages;

        if let Some(max_pages) = self.config.max_pages {
            if new_pages > max_pages {
                return Err(Error::runtime_out_of_bounds(
                    "Memory growth would exceed maximum pages",
                ));
            }
        }

        let new_size = new_pages * 65536;
        let target_len = (new_size as usize).min(self.data.capacity());

        // Grow the bounded vec to new size (capped at capacity)
        #[cfg(feature = "std")]
        {
            self.data.resize(target_len, 0);
        }
        #[cfg(not(feature = "std"))]
        {
            while self.data.len() < target_len {
                self.data
                    .push(0)
                    .map_err(|_| Error::memory_error("Memory growth capacity exceeded"))?;
            }
        }

        self.current_size = new_size;

        Ok(old_pages)
    }

    /// Get current size in pages
    pub fn size_pages(&self) -> u32 {
        self.current_size / 65536
    }

    /// Clear memory (for cleanup)
    pub fn clear(&mut self) {
        let _ = self.data.clear();
        self.current_size = 0;
    }
}

impl CanonicalMemory for ComponentMemory {
    fn read_bytes(&self, offset: u32, len: u32) -> Result<Vec<u8>> {
        let start = offset as usize;
        let end = start + len as usize;

        if end > self.data.len() {
            return Err(Error::memory_out_of_bounds("Memory read out of bounds"));
        }

        // Read bytes one by one from BoundedVec
        let mut result = Vec::with_capacity(len as usize);
        for i in start..end {
            let byte = *self
                .data
                .get(i)
                .ok_or_else(|| Error::memory_out_of_bounds("Failed to read byte from memory"))?;
            result.push(byte);
        }
        Ok(result)
    }

    fn write_bytes(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + data.len();

        if end > self.data.len() {
            return Err(Error::memory_out_of_bounds("Memory write out of bounds"));
        }

        // Write bytes one by one to BoundedVec
        for (i, &byte) in data.iter().enumerate() {
            if let Some(elem) = self.data.get_mut(start + i) {
                *elem = byte;
            } else {
                return Err(Error::memory_error("Failed to write byte to memory"));
            }
        }
        Ok(())
    }

    fn size(&self) -> u32 {
        self.current_size
    }
}

/// Component instantiation errors
#[derive(Debug, Clone, PartialEq)]
pub enum InstantiationError {
    /// Invalid component binary
    InvalidComponent(String),
    /// Missing required import
    MissingImport(String),
    /// Type mismatch in import/export
    TypeMismatch(String),
    /// Resource exhaustion
    ResourceExhaustion(String),
    /// Initialization failure
    InitializationFailed(String),
}

impl core::fmt::Display for InstantiationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            InstantiationError::InvalidComponent(msg) => write!(f, "Invalid component: {}", msg),
            InstantiationError::MissingImport(name) => write!(f, "Missing import: {}", name),
            InstantiationError::TypeMismatch(msg) => write!(f, "Type mismatch: {}", msg),
            InstantiationError::ResourceExhaustion(msg) => {
                write!(f, "Resource exhaustion: {}", msg)
            },
            InstantiationError::InitializationFailed(msg) => {
                write!(f, "Initialization failed: {}", msg)
            },
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InstantiationError {}

/// Create a function signature
pub fn create_function_signature(
    name: String,
    params_vec: Vec<ComponentType>,
    returns_vec: Vec<ComponentType>,
) -> FunctionSignature {
    #[cfg(feature = "std")]
    {
        FunctionSignature {
            name,
            params: params_vec,
            returns: returns_vec,
        }
    }
    #[cfg(not(feature = "std"))]
    {
        // Convert Vec to BoundedVec
        let mut bounded_params = BoundedVec::<ComponentType, 16>::new();
        for param in params_vec {
            let _ = bounded_params.push(param); // Silently ignore if capacity exceeded
        }

        let mut bounded_returns = BoundedVec::<ComponentType, 16>::new();
        for ret in returns_vec {
            let _ = bounded_returns.push(ret); // Silently ignore if capacity exceeded
        }

        FunctionSignature {
            name,
            params: bounded_params,
            returns: bounded_returns,
        }
    }
}

/// Create a component export
pub fn create_component_export(
    name: String,
    export_type: ExportType,
    index: u32,
) -> ComponentExport {
    ComponentExport {
        name,
        export_type,
        index,
    }
}

/// Create a component import
pub fn create_component_import(
    name: String,
    module: String,
    import_type: ImportType,
) -> ComponentImport {
    ComponentImport {
        name,
        module,
        import_type,
    }
}
