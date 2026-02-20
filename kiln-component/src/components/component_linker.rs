//! Component Linker and Import/Export Resolution System

// Cross-environment imports
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, format, string::String, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    bounded::BoundedString, budget_aware_provider::CrateId, collections::StaticVec as BoundedVec,
    safe_managed_alloc, safe_memory::NoStdProvider,
};

use crate::prelude::*;

// Tracing imports for structured logging
#[cfg(feature = "tracing")]
use wrt_foundation::tracing::{trace, warn as tracing_warn};

// Type aliases for no_std environment with proper generics
#[cfg(not(feature = "std"))]
type String = BoundedString<256>;
#[cfg(not(feature = "std"))]
type Vec<T> = BoundedVec<T, 256>;
#[cfg(not(feature = "std"))]
type HashMap<K, V> = wrt_foundation::collections::StaticMap<K, V, 64>;

use crate::{
    components::{
        component::Component,
        component_instantiation::{
            ComponentExport, ComponentImport, ExportType, FunctionSignature, ImportType,
            InstanceConfig, InstanceId, create_component_export, create_component_import,
        },
    },
    types::ComponentInstance,
};

/// Maximum number of components in linker
const MAX_LINKED_COMPONENTS: usize = 256;

/// Component identifier in the linker
pub type ComponentId = String;

/// Component linker for managing multiple components and their dependencies
///
/// This is the unified linker that handles:
/// 1. Internal resolution - imports from registered components
/// 2. Host providers - WASI imports from WasiInstanceProvider
/// 3. Component composition - dependency graphs and topological sorting
#[derive(Debug)]
pub struct ComponentLinker {
    /// Registered components
    components: HashMap<ComponentId, ComponentDefinition>,
    /// Active component instances
    instances: HashMap<InstanceId, ComponentInstance>,
    /// Dependency graph
    link_graph: LinkGraph,
    /// Next available instance ID
    next_instance_id: InstanceId,
    /// Linker configuration
    config: LinkerConfig,
    /// Resolution statistics
    stats: LinkingStats,
    /// WASI instance provider for host imports
    #[cfg(feature = "std")]
    wasi_provider: Option<crate::linker::WasiInstanceProvider>,
}

/// Component definition in the linker
#[derive(Debug, Clone)]
pub struct ComponentDefinition {
    /// Component ID
    pub id: ComponentId,
    /// Component binary (simplified as bytes)
    pub binary: Vec<u8>, // Use Vec for std, BoundedVec handled in no_std type alias
    /// Parsed exports
    pub exports: Vec<ComponentExport>,
    /// Parsed imports
    pub imports: Vec<ComponentImport>,
    /// Component metadata
    pub metadata: ComponentMetadata,
}

/// Component metadata for introspection
#[derive(Debug, Clone)]
pub struct ComponentMetadata {
    /// Component name
    pub name: String,
    /// Component version
    pub version: String,
    /// Component description
    pub description: String,
    /// Component author
    pub author: String,
    /// Compilation timestamp
    pub compiled_at: u64,
}

/// Dependency graph for component linking
#[derive(Debug, Clone)]
pub struct LinkGraph {
    /// Nodes (components)
    nodes: Vec<GraphNode>,
    /// Edges (dependencies)
    edges: Vec<GraphEdge>,
}

/// Graph node representing a component
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    /// Component ID
    pub component_id: ComponentId,
    /// Node index in graph
    pub index: usize,
    /// Dependencies (outgoing edges)
    pub dependencies: Vec<usize>,
    /// Dependents (incoming edges)
    pub dependents: Vec<usize>,
}

/// Graph edge representing a dependency relationship
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    /// Source node index
    pub from: usize,
    /// Target node index
    pub to: usize,
    /// Import that creates this dependency
    pub import: ComponentImport,
    /// Export that satisfies this dependency
    pub export: ComponentExport,
    /// Edge weight (for optimization)
    pub weight: u32,
}

/// Linker configuration
#[derive(Debug, Clone)]
pub struct LinkerConfig {
    /// Enable strict type checking
    pub strict_typing: bool,
    /// Allow hot swapping of components
    pub allow_hot_swap: bool,
    /// Maximum memory per instance
    pub max_instance_memory: u32,
    /// Enable dependency validation
    pub validate_dependencies: bool,
    /// Circular dependency handling
    pub circular_dependency_mode: CircularDependencyMode,
}

/// Circular dependency handling modes
#[derive(Debug, Clone, PartialEq)]
pub enum CircularDependencyMode {
    /// Reject circular dependencies
    Reject,
    /// Allow circular dependencies (with limitations)
    Allow,
    /// Warn about circular dependencies but allow them
    Warn,
}

/// Linking statistics
#[derive(Debug, Clone, Default)]
pub struct LinkingStats {
    /// Total components registered
    pub components_registered: u32,
    /// Total instances created
    pub instances_created: u32,
    /// Total links resolved
    pub links_resolved: u32,
    /// Resolution failures
    pub resolution_failures: u32,
    /// Last resolution time (microseconds)
    pub last_resolution_time: u64,
}

impl Default for LinkerConfig {
    fn default() -> Self {
        Self {
            strict_typing: true,
            allow_hot_swap: false,
            max_instance_memory: 64 * 1024 * 1024, // 64MB
            validate_dependencies: true,
            circular_dependency_mode: CircularDependencyMode::Reject,
        }
    }
}

impl Default for ComponentMetadata {
    fn default() -> Self {
        #[cfg(feature = "std")]
        {
            Self {
                name: String::new(),
                version: "1.0.0".to_owned(),
                description: String::new(),
                author: String::new(),
                compiled_at: 0,
            }
        }
        #[cfg(not(feature = "std"))]
        {
            use wrt_foundation::{budget_aware_provider::CrateId, safe_managed_alloc};

            let name_provider = safe_managed_alloc!(1024, CrateId::Component)
                .unwrap_or_else(|_| panic!("Failed to allocate memory for ComponentMetadata name"));
            let version_provider =
                safe_managed_alloc!(1024, CrateId::Component).unwrap_or_else(|_| {
                    panic!("Failed to allocate memory for ComponentMetadata version")
                });
            let description_provider = safe_managed_alloc!(1024, CrateId::Component)
                .unwrap_or_else(|_| {
                    panic!("Failed to allocate memory for ComponentMetadata description")
                });
            let author_provider =
                safe_managed_alloc!(1024, CrateId::Component).unwrap_or_else(|_| {
                    panic!("Failed to allocate memory for ComponentMetadata author")
                });

            Self {
                name: BoundedString::try_from_str("")
                    .unwrap_or_else(|_| panic!("Failed to create ComponentMetadata name")),
                version: BoundedString::try_from_str("1.0.0")
                    .unwrap_or_else(|_| panic!("Failed to create ComponentMetadata version")),
                description: BoundedString::try_from_str("")
                    .unwrap_or_else(|_| panic!("Failed to create ComponentMetadata description")),
                author: BoundedString::try_from_str("")
                    .unwrap_or_else(|_| panic!("Failed to create ComponentMetadata author")),
                compiled_at: 0,
            }
        }
    }
}

impl ComponentLinker {
    /// Create a new component linker
    pub fn new() -> Self {
        Self::with_config(LinkerConfig::default())
    }

    /// Create a new component linker with custom configuration
    pub fn with_config(config: LinkerConfig) -> Self {
        Self {
            components: HashMap::new(),
            instances: HashMap::new(),
            link_graph: LinkGraph::new(),
            next_instance_id: 1,
            config,
            stats: LinkingStats::default(),
            #[cfg(feature = "std")]
            wasi_provider: crate::linker::WasiInstanceProvider::new().ok(),
        }
    }

    /// Link component imports to providers using internal-first resolution
    ///
    /// Resolution order (per Component Model spec):
    /// 1. Try to resolve from registered internal components
    /// 2. Try to resolve from host providers (WASI)
    /// 3. Fail with clear error if no resolution found
    #[cfg(feature = "std")]
    pub fn link_imports(
        &mut self,
        imports: &[wrt_format::component::Import],
    ) -> Result<Vec<crate::instantiation::ResolvedImport>> {
        let mut resolved = Vec::with_capacity(imports.len());

        for import in imports {
            // Build full import name
            let name = if import.name.namespace.is_empty() {
                import.name.name.clone()
            } else {
                std::format!("{}:{}", import.name.namespace, import.name.name)
            };

            // STEP 1: Try internal resolution from registered components
            if let Some(internal_resolved) = self.resolve_from_internal(&name, import)? {
                resolved.push(internal_resolved);
                continue;
            }

            // STEP 2: Try WASI host provider
            if name.starts_with("wasi:") || name.contains("wasi:") {
                if let Some(ref mut wasi_provider) = self.wasi_provider {
                    let instance_import = wasi_provider.create_instance(&name)?;
                    resolved.push(crate::instantiation::ResolvedImport::Instance(
                        instance_import,
                    ));
                    continue;
                }
            }

            // STEP 3: FAIL LOUD - no provider found
            #[cfg(feature = "tracing")]
            tracing_warn!(import_name = %name, "Unresolved import - not found in internal components or host providers");
            return Err(Error::new(
                ErrorCategory::Core,
                codes::COMPONENT_LINKING_ERROR,
                "Unresolved import: no provider registered",
            ));
        }

        // Verify all imports were resolved
        if resolved.len() != imports.len() {
            #[cfg(feature = "tracing")]
            tracing_warn!(
                resolved = resolved.len(),
                total = imports.len(),
                "Not all imports resolved"
            );
            return Err(Error::new(
                ErrorCategory::Core,
                codes::COMPONENT_LINKING_ERROR,
                "Not all imports could be resolved",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(
            import_count = resolved.len(),
            "Successfully resolved imports"
        );
        self.stats.links_resolved += resolved.len() as u32;
        Ok(resolved)
    }

    /// Try to resolve an import from internal registered components
    ///
    /// Per Component Model spec, this performs:
    /// 1. Name matching between import and export
    /// 2. Type compatibility validation
    /// 3. Creation of proper ResolvedImport with export reference
    #[cfg(feature = "std")]
    fn resolve_from_internal(
        &self,
        name: &str,
        import: &wrt_format::component::Import,
    ) -> Result<Option<crate::instantiation::ResolvedImport>> {
        // Check if any registered component exports this interface
        for (component_id, component) in &self.components {
            for export in &component.exports {
                let export_name = &export.name;

                if export_name == name {
                    // Found a matching export from a registered component
                    #[cfg(feature = "tracing")]
                    trace!(export_name = %name, component_id = %component_id, "Found matching export from component");

                    // Validate type compatibility between import and export
                    if !self.check_import_export_compatibility(import, export) {
                        #[cfg(feature = "tracing")]
                        trace!(import_name = %name, "Type mismatch - import incompatible with export");
                        continue; // Try next matching export
                    }

                    // Convert ComponentExport to ExportValue for InstanceImport
                    let export_value = self.convert_export_to_value(export)?;

                    // Create InstanceImport with the export
                    let mut instance_import = crate::instantiation::InstanceImport {
                        exports: std::collections::BTreeMap::new(),
                    };
                    instance_import.exports.insert(name.to_string(), Box::new(export_value));

                    #[cfg(feature = "tracing")]
                    trace!(import_name = %name, component_id = %component_id, "Successfully resolved import from internal component");
                    return Ok(Some(crate::instantiation::ResolvedImport::Instance(
                        instance_import,
                    )));
                }
            }
        }
        Ok(None)
    }

    /// Check if an import is compatible with an export
    #[cfg(feature = "std")]
    fn check_import_export_compatibility(
        &self,
        import: &wrt_format::component::Import,
        export: &ComponentExport,
    ) -> bool {
        use wrt_format::component::ExternType;

        // Check kind compatibility first using the import's type (ty field)
        match (&import.ty, &export.export_type) {
            // Function import requires function export
            (ExternType::Function { .. }, ExportType::Function(_)) => true,
            // Value import requires memory export
            (ExternType::Value(_), ExportType::Memory(_)) => true,
            // Type imports can match type exports
            (ExternType::Type(_), ExportType::Type(_)) => true,
            // Instance imports can match component exports
            (ExternType::Instance { .. }, _) => true,
            // Component imports need special handling
            (ExternType::Component { .. }, _) => true,
            // Module imports
            (ExternType::Module { .. }, _) => true,
            // For now, allow mismatches to fall through (will be rejected if no match found)
            _ => {
                #[cfg(feature = "tracing")]
                trace!(
                    import_type = ?std::mem::discriminant(&import.ty),
                    export_type = ?std::mem::discriminant(&export.export_type),
                    "Import/export type comparison"
                );
                // Allow to continue searching for better matches
                false
            },
        }
    }

    /// Convert a ComponentExport to an instantiation ExportValue
    #[cfg(feature = "std")]
    fn convert_export_to_value(
        &self,
        export: &ComponentExport,
    ) -> Result<crate::instantiation::ExportValue> {
        use crate::bounded_component_infra::ComponentProvider;
        use crate::prelude::{WrtComponentType, WrtComponentValue};

        match &export.export_type {
            ExportType::Function(_signature) => {
                // Create a FunctionExport from the signature
                // Use the actual index from the parsed export
                let func_export = crate::instantiation::FunctionExport {
                    signature: WrtComponentType::unit(ComponentProvider::default())?,
                    index: export.index, // Use the actual function index from parsed export
                };
                Ok(crate::instantiation::ExportValue::Function(func_export))
            },
            ExportType::Memory(_mem_config) => {
                // Memory exports are handled differently - create a Value placeholder
                Ok(crate::instantiation::ExportValue::Value(
                    WrtComponentValue::Unit,
                ))
            },
            ExportType::Table { .. } => {
                // Table exports - placeholder for now
                Ok(crate::instantiation::ExportValue::Value(
                    WrtComponentValue::Unit,
                ))
            },
            ExportType::Global { .. } => {
                // Global exports - placeholder for now
                Ok(crate::instantiation::ExportValue::Value(
                    WrtComponentValue::Unit,
                ))
            },
            ExportType::Type(_) => {
                // Type exports
                Ok(crate::instantiation::ExportValue::Type(
                    WrtComponentType::unit(ComponentProvider::default())?,
                ))
            },
        }
    }

    /// Add a component to the linker
    pub fn add_component(&mut self, id: ComponentId, binary: &[u8]) -> Result<()> {
        if self.components.len() >= MAX_LINKED_COMPONENTS {
            return Err(Error::resource_exhausted(
                "Maximum number of components reached",
            ));
        }

        // Parse component binary (simplified)
        let (exports, imports, metadata) = self.parse_component_binary(binary)?;

        // Convert binary slice to Vec
        #[cfg(feature = "std")]
        let binary_vec = binary.to_vec();

        #[cfg(not(feature = "std"))]
        let binary_vec = {
            let mut vec = Vec::new();
            for &byte in binary {
                vec.push(byte).map_err(|_| {
                    Error::new(
                        ErrorCategory::Capacity,
                        codes::CAPACITY_EXCEEDED,
                        "Binary too large for component",
                    )
                })?;
            }
            vec
        };

        let definition = ComponentDefinition {
            id: id.clone(),
            binary: binary_vec,
            exports,
            imports,
            metadata,
        };

        // Add to components map
        let _ = self.components.insert(id.clone(), definition);

        // Update dependency graph
        self.link_graph.add_component(id)?;

        // Update statistics
        self.stats.components_registered += 1;

        Ok(())
    }

    /// Remove a component from the linker
    pub fn remove_component(&mut self, id: &ComponentId) -> Result<()> {
        // Check if component exists
        if !self.components.contains_key(id) {
            return Err(Error::component_not_found("Component not found"));
        }

        // Check if any instances are using this component
        #[cfg(feature = "std")]
        let dependent_instances: Vec<_> = self
            .instances
            .values()
            .filter(|instance| {
                instance.component.id.as_ref().map(|s| s.as_str()) == Some(id.as_str())
            })
            .map(|instance| instance.id)
            .collect();

        #[cfg(not(feature = "std"))]
        let dependent_instances = {
            let mut dependent_instances = Vec::new();
            if let Ok(linker_id_str) = id.as_str() {
                for instance in self.instances.values() {
                    if let Some(comp_id) = instance.component.id.as_ref() {
                        let comp_id_str = comp_id.as_str();
                        if comp_id_str == linker_id_str {
                            let _ = dependent_instances.push(instance.id);
                        }
                    }
                }
            }
            dependent_instances
        };

        if !dependent_instances.is_empty() {
            return Err(Error::runtime_execution_error(
                "Component has active instances and cannot be removed",
            ));
        }

        // Remove from components and graph
        self.components.remove(id);
        self.link_graph.remove_component(id)?;

        Ok(())
    }

    /// Instantiate a component with dependency resolution
    pub fn instantiate(
        &mut self,
        component_id: &ComponentId,
        config: Option<InstanceConfig>,
    ) -> Result<InstanceId> {
        // Find component definition and extract imports
        let imports = {
            let component = self
                .components
                .get(component_id)
                .ok_or_else(|| Error::component_not_found("Component not found"))?;
            component.imports.clone()
        };

        // Resolve dependencies
        #[cfg(feature = "std")]
        let resolved_imports = self.resolve_imports(component_id, &imports)?;
        #[cfg(not(feature = "std"))]
        let resolved_imports = {
            // Convert BoundedVec to slice for resolve_imports
            self.resolve_imports(component_id, imports.as_slice())?
        };

        // Get component again for instance creation
        let component = self
            .components
            .get(component_id)
            .ok_or_else(|| Error::component_not_found("Component not found"))?;

        // Create instance
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;

        // Create Component from definition (using new constructor)
        let mut comp = Component::new(crate::components::component::WrtComponentType::default());

        // Set the component ID
        #[cfg(feature = "std")]
        {
            comp.id = Some(component.id.clone());
        }
        #[cfg(not(feature = "std"))]
        {
            // Convert BoundedString to String for Component id
            if let Ok(id_str) = component.id.as_str() {
                comp.id = Some(alloc::string::String::from(id_str));
            }
        }

        // Create ComponentInstance
        let mut instance = ComponentInstance {
            #[cfg(feature = "wrt-execution")]
            runtime_engine: None,
            #[cfg(feature = "wrt-execution")]
            main_instance_handle: None,
            id: instance_id,
            component: comp,
            state: crate::types::ComponentInstanceState::Initialized,
            resource_manager: None,
            memory: None,
            metadata: crate::types::ComponentMetadata::default(),
            #[cfg(feature = "std")]
            type_index: std::collections::HashMap::new(),
            #[cfg(not(feature = "std"))]
            type_index: (),
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            functions: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            functions: Vec::new(),
            #[cfg(not(feature = "std"))]
            functions: BoundedVec::new(),
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            imports: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            imports: Vec::new(),
            #[cfg(not(feature = "std"))]
            imports: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            exports: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            exports: Vec::new(),
            #[cfg(not(feature = "std"))]
            exports: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            resource_tables: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            resource_tables: Vec::new(),
            #[cfg(not(feature = "std"))]
            resource_tables: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            module_instances: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            module_instances: Vec::new(),
            #[cfg(not(feature = "std"))]
            module_instances: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new()
            },
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            nested_component_instances: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            nested_component_instances: Vec::new(),
            #[cfg(not(feature = "std"))]
            nested_component_instances: { BoundedVec::new() },
        };

        // Add resolved imports from instantiation module
        #[cfg(feature = "std")]
        for resolved in resolved_imports {
            instance.imports.push(crate::instantiation::ResolvedImport::Value(
                crate::prelude::WrtComponentValue::Unit,
            ));
        }
        #[cfg(not(feature = "std"))]
        for resolved in resolved_imports {
            let _ = instance.imports.push(crate::instantiation::ResolvedImport::Value(
                crate::prelude::WrtComponentValue::Unit,
            ));
        }

        // Transition to running state
        instance.state = crate::types::ComponentInstanceState::Running;

        // Add to instances map
        let _ = self.instances.insert(instance_id, instance);

        // Update statistics
        self.stats.instances_created += 1;

        Ok(instance_id)
    }

    /// Link all components and create instances
    pub fn link_all(&mut self) -> Result<Vec<InstanceId>> {
        let mut instance_ids = Vec::new();

        // Topological sort to determine instantiation order
        let sorted_components = self.link_graph.topological_sort()?;

        // Instantiate components in dependency order
        for component_id in sorted_components {
            let instance_id = self.instantiate(&component_id, None)?;
            let _ = instance_ids.push(instance_id);
        }

        Ok(instance_ids)
    }

    /// Get a component instance by ID
    pub fn get_instance(&self, instance_id: InstanceId) -> Option<&ComponentInstance> {
        self.instances.get(&instance_id)
    }

    /// Get a mutable component instance by ID
    pub fn get_instance_mut(&mut self, instance_id: InstanceId) -> Option<&mut ComponentInstance> {
        self.instances.get_mut(&instance_id)
    }

    /// Get linking statistics
    pub fn get_stats(&self) -> &LinkingStats {
        &self.stats
    }

    // Private helper methods

    /// Parse a component binary to extract exports, imports, and metadata.
    ///
    /// In std mode with the decoder feature, this uses the real wrt-decoder to parse
    /// the component binary. In no_std mode, component parsing is not supported.
    fn parse_component_binary(
        &self,
        binary: &[u8],
    ) -> core::result::Result<
        (
            Vec<ComponentExport>,
            Vec<ComponentImport>,
            ComponentMetadata,
        ),
        Error,
    > {
        if binary.is_empty() {
            return Err(Error::runtime_execution_error("Empty component binary"));
        }

        // Use real decoder in std mode
        #[cfg(all(feature = "std", feature = "decoder"))]
        {
            self.parse_component_with_decoder(binary)
        }

        // Fallback for std without decoder feature
        #[cfg(all(feature = "std", not(feature = "decoder")))]
        {
            self.parse_component_fallback(binary)
        }

        // no_std mode - component parsing not supported
        #[cfg(not(feature = "std"))]
        {
            Err(Error::runtime_execution_error(
                "Component parsing requires std and decoder features",
            ))
        }
    }

    /// Parse component using the real wrt-decoder
    #[cfg(all(feature = "std", feature = "decoder"))]
    fn parse_component_with_decoder(
        &self,
        binary: &[u8],
    ) -> core::result::Result<
        (
            Vec<ComponentExport>,
            Vec<ComponentImport>,
            ComponentMetadata,
        ),
        Error,
    > {
        use wrt_format::component::{CoreSort, Sort};

        // Decode the component binary
        let decoded = wrt_decoder::component::decode_component(binary)?;

        // Convert decoded exports to ComponentExport
        let mut exports = Vec::with_capacity(decoded.exports.len());
        for export in &decoded.exports {
            let export_type = self.sort_to_export_type(&export.sort, &export.ty);
            let name = export.name.name.clone();
            let index = export.idx;

            #[cfg(feature = "tracing")]
            trace!(
                export_name = %name,
                export_index = index,
                "Parsed component export"
            );

            exports.push(create_component_export(name, export_type, index));
        }

        // Convert decoded imports to ComponentImport
        let mut imports = Vec::with_capacity(decoded.imports.len());
        for import in &decoded.imports {
            let import_type = self.extern_type_to_import_type(&import.ty);
            let name = import.name.name.clone();
            let module = import.name.namespace.clone();

            #[cfg(feature = "tracing")]
            trace!(
                import_name = %name,
                import_module = %module,
                "Parsed component import"
            );

            imports.push(create_component_import(name, module, import_type));
        }

        // Extract metadata from component
        let metadata = ComponentMetadata {
            name: decoded.name.unwrap_or_default(),
            version: "1.0.0".to_owned(),
            description: String::new(),
            author: String::new(),
            compiled_at: 0,
        };

        Ok((exports, imports, metadata))
    }

    /// Convert Sort to ExportType
    #[cfg(all(feature = "std", feature = "decoder"))]
    fn sort_to_export_type(
        &self,
        sort: &wrt_format::component::Sort,
        ty: &Option<wrt_format::component::ExternType>,
    ) -> ExportType {
        use wrt_format::component::{CoreSort, ExternType as FormatExternType, Sort};

        match sort {
            Sort::Function => {
                // Component-level function (canonical lift/lower)
                let signature = if let Some(FormatExternType::Function { params, results }) = ty {
                    self.convert_function_type(params, results)
                } else {
                    FunctionSignature::default()
                };
                ExportType::Function(signature)
            },
            Sort::Core(CoreSort::Function) => {
                // Core WebAssembly function
                let signature = if let Some(FormatExternType::Function { params, results }) = ty {
                    self.convert_function_type(params, results)
                } else {
                    FunctionSignature::default()
                };
                ExportType::Function(signature)
            },
            Sort::Core(CoreSort::Memory) => {
                ExportType::Memory(crate::component_instantiation::MemoryConfig {
                    initial_pages: 1,
                    max_pages: None,
                    protected: false,
                })
            },
            Sort::Core(CoreSort::Table) => ExportType::Table {
                element_type: crate::canonical_abi::ComponentType::U32,
                size: 0,
            },
            Sort::Core(CoreSort::Global) => ExportType::Global {
                value_type: crate::canonical_abi::ComponentType::S32,
                mutable: false,
            },
            Sort::Type | Sort::Core(CoreSort::Type) => {
                ExportType::Type(crate::canonical_abi::ComponentType::Bool)
            },
            Sort::Component
            | Sort::Instance
            | Sort::Value
            | Sort::Core(CoreSort::Module)
            | Sort::Core(CoreSort::Instance) => {
                // For now, treat these as functions (placeholder)
                ExportType::Function(FunctionSignature::default())
            },
        }
    }

    /// Convert ExternType to ImportType
    #[cfg(all(feature = "std", feature = "decoder"))]
    fn extern_type_to_import_type(&self, ty: &wrt_format::component::ExternType) -> ImportType {
        use wrt_format::component::ExternType as FormatExternType;

        match ty {
            FormatExternType::Function { params, results } => {
                ImportType::Function(self.convert_function_type(params, results))
            },
            FormatExternType::Value(_) => {
                ImportType::Memory(crate::component_instantiation::MemoryConfig {
                    initial_pages: 1,
                    max_pages: None,
                    protected: false,
                })
            },
            FormatExternType::Type(_) => {
                ImportType::Type(crate::canonical_abi::ComponentType::Bool)
            },
            FormatExternType::Instance { .. }
            | FormatExternType::Component { .. }
            | FormatExternType::Module { .. } => {
                // Treat as function import for now
                ImportType::Function(FunctionSignature::default())
            },
        }
    }

    /// Convert function type from format to instantiation types
    #[cfg(all(feature = "std", feature = "decoder"))]
    fn convert_function_type(
        &self,
        params: &[(String, wrt_format::component::FormatValType)],
        results: &[wrt_format::component::FormatValType],
    ) -> FunctionSignature {
        let param_types: Vec<crate::canonical_abi::ComponentType> = params
            .iter()
            .map(|(_, vt)| self.format_val_type_to_component_type(vt))
            .collect();

        let result_types: Vec<crate::canonical_abi::ComponentType> =
            results.iter().map(|vt| self.format_val_type_to_component_type(vt)).collect();

        // Use first param name as function name, or empty
        let name = params.first().map(|(n, _)| n.clone()).unwrap_or_default();

        crate::component_instantiation::create_function_signature(name, param_types, result_types)
    }

    /// Convert FormatValType from format to canonical_abi ComponentType
    #[cfg(all(feature = "std", feature = "decoder"))]
    fn format_val_type_to_component_type(
        &self,
        vt: &wrt_format::component::FormatValType,
    ) -> crate::canonical_abi::ComponentType {
        use wrt_format::component::FormatValType;

        match vt {
            FormatValType::Bool => crate::canonical_abi::ComponentType::Bool,
            FormatValType::S8 => crate::canonical_abi::ComponentType::S8,
            FormatValType::U8 => crate::canonical_abi::ComponentType::U8,
            FormatValType::S16 => crate::canonical_abi::ComponentType::S16,
            FormatValType::U16 => crate::canonical_abi::ComponentType::U16,
            FormatValType::S32 => crate::canonical_abi::ComponentType::S32,
            FormatValType::U32 => crate::canonical_abi::ComponentType::U32,
            FormatValType::S64 => crate::canonical_abi::ComponentType::S64,
            FormatValType::U64 => crate::canonical_abi::ComponentType::U64,
            FormatValType::F32 => crate::canonical_abi::ComponentType::F32,
            FormatValType::F64 => crate::canonical_abi::ComponentType::F64,
            FormatValType::Char => crate::canonical_abi::ComponentType::Char,
            FormatValType::String => crate::canonical_abi::ComponentType::String,
            // Complex types - map to their canonical equivalents (using placeholder types)
            FormatValType::List(_) => crate::canonical_abi::ComponentType::String, // Placeholder
            FormatValType::Record(_) => crate::canonical_abi::ComponentType::Bool, // Placeholder
            FormatValType::Tuple(_) => crate::canonical_abi::ComponentType::Bool,  // Placeholder
            FormatValType::Variant(_) => crate::canonical_abi::ComponentType::U32, // Discriminant
            FormatValType::Enum(_) => crate::canonical_abi::ComponentType::U32,    // Enums are u32
            FormatValType::Option(_) => crate::canonical_abi::ComponentType::Bool, // Placeholder
            FormatValType::Result { .. } => crate::canonical_abi::ComponentType::Bool, // Placeholder
            FormatValType::Flags(_) => crate::canonical_abi::ComponentType::U32, // Flags are u32
            FormatValType::Own(_) | FormatValType::Borrow(_) => {
                crate::canonical_abi::ComponentType::U32 // Resource handles are u32
            },
            // Fixed-length list (same as List for now)
            FormatValType::FixedList(_, _) => crate::canonical_abi::ComponentType::String,
            // Reference type
            FormatValType::Ref(_) => crate::canonical_abi::ComponentType::U32, // Ref as u32 index
            // Void type (no value)
            FormatValType::Void => crate::canonical_abi::ComponentType::Bool, // Placeholder
            // Error context type
            FormatValType::ErrorContext => crate::canonical_abi::ComponentType::U32, // Error context handle
        }
    }

    /// Fallback parsing when decoder is not available (std only)
    #[cfg(all(feature = "std", not(feature = "decoder")))]
    fn parse_component_fallback(
        &self,
        _binary: &[u8],
    ) -> core::result::Result<
        (
            Vec<ComponentExport>,
            Vec<ComponentImport>,
            ComponentMetadata,
        ),
        Error,
    > {
        // Without the decoder, return empty exports/imports
        // Components won't actually link without real parsing
        #[cfg(feature = "tracing")]
        tracing_warn!("Component parsing without decoder - exports/imports will be empty");

        Ok((Vec::new(), Vec::new(), ComponentMetadata::default()))
    }

    fn resolve_imports(
        &mut self,
        component_id: &ComponentId,
        imports: &[ComponentImport],
    ) -> Result<Vec<crate::instantiation::ResolvedImport>> {
        let mut resolved = Vec::new();

        for import in imports {
            let resolution = self.resolve_single_import(component_id, import)?;
            let _ = resolved.push(resolution);
        }

        self.stats.links_resolved += resolved.len() as u32;
        Ok(resolved)
    }

    fn resolve_single_import(
        &self,
        _component_id: &ComponentId,
        import: &ComponentImport,
    ) -> Result<crate::instantiation::ResolvedImport> {
        // Find a component that exports what we need
        for (provider_id, component) in &self.components {
            for export in &component.exports {
                if self.is_compatible_import_export(import, export)? {
                    // Return a placeholder resolved import (actual resolution would be more complex)
                    return Ok(crate::instantiation::ResolvedImport::Value(
                        crate::prelude::WrtComponentValue::Unit,
                    ));
                }
            }
        }

        Err(Error::component_not_found("Component not found"))
    }

    fn is_compatible_import_export(
        &self,
        import: &ComponentImport,
        export: &ComponentExport,
    ) -> Result<bool> {
        // Check name compatibility
        if import.name != export.name {
            return Ok(false);
        }

        // Check type compatibility
        match (&import.import_type, &export.export_type) {
            (ImportType::Function(import_sig), ExportType::Function(export_sig)) => {
                Ok(self.is_compatible_function_signature(import_sig, export_sig))
            },
            (ImportType::Memory(import_mem), ExportType::Memory(export_mem)) => {
                Ok(self.is_compatible_memory_config(import_mem, export_mem))
            },
            _ => Ok(false), // Other type combinations
        }
    }

    fn is_compatible_function_signature(
        &self,
        import_sig: &FunctionSignature,
        export_sig: &FunctionSignature,
    ) -> bool {
        // Simplified compatibility check
        import_sig.params == export_sig.params && import_sig.returns == export_sig.returns
    }

    fn is_compatible_memory_config(
        &self,
        _import_mem: &crate::component_instantiation::MemoryConfig,
        _export_mem: &crate::component_instantiation::MemoryConfig,
    ) -> bool {
        // Simplified compatibility check
        true
    }
}

impl Default for LinkGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkGraph {
    /// Create a new empty link graph
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a component to the graph
    pub fn add_component(&mut self, component_id: ComponentId) -> Result<()> {
        // Check if component already exists
        if self.find_node_index(&component_id).is_some() {
            return Err(Error::runtime_execution_error(
                "Component already exists in graph",
            ));
        }

        let node = GraphNode {
            component_id,
            index: self.nodes.len(),
            dependencies: Vec::new(),
            dependents: Vec::new(),
        };

        let _ = self.nodes.push(node);
        Ok(())
    }

    /// Remove a component from the graph
    pub fn remove_component(&mut self, component_id: &ComponentId) -> Result<()> {
        let node_index = self.find_node_index(component_id).ok_or_else(|| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::RESOURCE_NOT_FOUND,
                "Component not found in graph",
            )
        })?;

        // Remove all edges involving this node
        self.edges.retain(|edge| edge.from != node_index && edge.to != node_index);

        // Remove the node
        self.nodes.remove(node_index);

        // Update indices in remaining nodes and edges
        #[cfg(feature = "std")]
        for node in &mut self.nodes[node_index..] {
            node.index -= 1;
        }
        #[cfg(not(feature = "std"))]
        for i in node_index..self.nodes.len() {
            self.nodes[i].index -= 1;
        }

        for edge in &mut self.edges {
            if edge.from > node_index {
                edge.from -= 1;
            }
            if edge.to > node_index {
                edge.to -= 1;
            }
        }

        Ok(())
    }

    /// Perform topological sort to determine instantiation order
    pub fn topological_sort(&self) -> Result<Vec<ComponentId>> {
        #[cfg(feature = "std")]
        {
            let mut visited = vec![false; self.nodes.len()];
            let mut temp_visited = vec![false; self.nodes.len()];
            let mut result = Vec::new();

            for i in 0..self.nodes.len() {
                if !visited[i] {
                    self.topological_sort_visit(i, &mut visited, &mut temp_visited, &mut result)?;
                }
            }

            result.reverse();
            Ok(result)
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, create bounded vectors
            let mut visited = Vec::new();
            let mut temp_visited = Vec::new();
            let mut result = Vec::new();

            // Initialize with false values
            for _ in 0..self.nodes.len() {
                visited.push(false).map_err(|_| {
                    Error::platform_memory_allocation_failed("Memory allocation failed")
                })?;
                temp_visited.push(false).map_err(|_| {
                    Error::platform_memory_allocation_failed("Memory allocation failed")
                })?;
            }

            for i in 0..self.nodes.len() {
                if !visited[i] {
                    self.topological_sort_visit(i, &mut visited, &mut temp_visited, &mut result)?;
                }
            }

            result.reverse();
            Ok(result)
        }
    }

    fn topological_sort_visit(
        &self,
        node_index: usize,
        visited: &mut Vec<bool>,
        temp_visited: &mut Vec<bool>,
        result: &mut Vec<ComponentId>,
    ) -> Result<()> {
        if temp_visited[node_index] {
            return Err(Error::validation_error("Circular dependency detected"));
        }

        if visited[node_index] {
            return Ok(());
        }

        temp_visited[node_index] = true;

        // Visit dependencies first
        for &dep_index in &self.nodes[node_index].dependencies {
            self.topological_sort_visit(dep_index, visited, temp_visited, result)?;
        }

        temp_visited[node_index] = false;
        visited[node_index] = true;
        let _ = result.push(self.nodes[node_index].component_id.clone());

        Ok(())
    }

    fn find_node_index(&self, component_id: &ComponentId) -> Option<usize> {
        self.nodes
            .iter()
            .find(|node| &node.component_id == component_id)
            .map(|node| node.index)
    }
}

impl Default for ComponentLinker {
    fn default() -> Self {
        Self::new()
    }
}
