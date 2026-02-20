//! Unified Component Model types for runtime integration
//!
//! This module provides unified component types that integrate with the
//! platform-aware memory system and resolve type conflicts between different
//! runtime components.

// alloc is imported in lib.rs with proper feature gates

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;
// Import Box for no_std compatibility
#[cfg(feature = "std")]
use alloc::boxed::Box;

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::{
    component::{
        ComponentType,
        ExternType,
    },
    prelude::*,
    safe_memory::{
        MemoryProvider,
        NoStdProvider,
    },
};

#[cfg(not(any(feature = "std", feature = "alloc")))]
use crate::unified_types::PlatformMemoryAdapter as GenericMemoryAdapter;
use crate::{
    bounded_runtime_infra::{
        create_runtime_provider,
        DefaultRuntimeProvider,
    },
    unified_types::{
        DefaultMediumVec,
        DefaultRuntimeTypes,
        ExportMap,
        ImportMap,
        MemoryBuffer,
        PlatformMemoryAdapter,
        RuntimeString,
        UnifiedMemoryAdapter,
    },
};

// DefaultRuntimeProvider definition moved to bounded_runtime_infra.rs to avoid
// conflicts

/// Unique identifier for component instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(u32);

impl ComponentId {
    /// Create a new unique component ID
    pub fn new() -> Self {
        use core::sync::atomic::{
            AtomicU32,
            Ordering,
        };
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the numeric value of this ID
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified component instance with platform-aware memory management
///
/// This struct provides a unified representation of component instances that
/// integrates with the platform memory system and provides consistent APIs.
#[derive(Debug)]
pub struct UnifiedComponentInstance<Provider = DefaultRuntimeProvider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Unique identifier for this component instance
    pub id: ComponentId,

    /// Component type definition
    pub component_type: ComponentType<Provider>,

    /// Memory adapter for this component's allocations
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub memory_adapter: PlatformMemoryAdapter<DefaultRuntimeProvider>,

    /// Memory adapter for this component's allocations (no_std version)
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    pub memory_adapter: PlatformMemoryAdapter<DefaultRuntimeProvider>,

    /// Exported functions and types from this component
    pub exports: ExportMap<ExternType<Provider>>,

    /// Imported functions and types required by this component
    pub imports: ImportMap<ExternType<Provider>>,

    /// Component's linear memory (if any)
    pub linear_memory: Option<MemoryBuffer>,

    /// Component execution state
    pub state: ComponentExecutionState,
}

// NOTE: Clone is intentionally NOT implemented for UnifiedComponentInstance
// because cloning requires memory allocation which can fail. Use try_clone() instead.
impl<Provider> UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Try to clone this instance, returning an error if memory allocation fails.
    /// This is the fallible alternative to Clone trait which cannot express fallibility.
    pub fn try_clone(&self) -> Result<Self> {
        let memory_adapter = PlatformMemoryAdapter::new(64 * 1024 * 1024)
            .map_err(|_| Error::memory_error("Failed to allocate memory adapter during clone"))?;

        Ok(Self {
            id: self.id,
            component_type: self.component_type.clone(),
            memory_adapter,
            exports: self.exports.clone(),
            imports: self.imports.clone(),
            linear_memory: self.linear_memory.clone(),
            state: self.state.clone(),
        })
    }
}

impl<Provider> UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Creates a new default instance of UnifiedComponentInstance.
    pub fn new_default() -> Result<Self> {
        let memory_adapter = PlatformMemoryAdapter::new(64 * 1024 * 1024)
            .map_err(|_e| Error::runtime_execution_error("Failed to create memory adapter"))?;

        Ok(Self {
            id: ComponentId::default(),
            component_type: ComponentType::default(),
            memory_adapter,
            exports: ExportMap::new(create_runtime_provider()?)?,
            imports: ImportMap::new(create_runtime_provider()?)?,
            linear_memory: None,
            state: ComponentExecutionState::Instantiating,
        })
    }
}

impl<Provider> PartialEq for UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<Provider> Eq for UnifiedComponentInstance<Provider> where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq
{
}

// NOTE: Default is intentionally NOT implemented for UnifiedComponentInstance
// because creating a default instance requires memory allocation which can fail.
// Use UnifiedComponentInstance::new_default() which returns Result<Self> instead.

impl<Provider> wrt_foundation::traits::Checksummable for UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.id.as_u32().to_le_bytes());
        self.component_type.update_checksum(checksum);
        self.exports.update_checksum(checksum);
        self.imports.update_checksum(checksum);
    }
}

impl<Provider> wrt_foundation::traits::ToBytes for UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn serialized_size(&self) -> usize {
        4 + self.component_type.serialized_size()
            + self.exports.serialized_size()
            + self.imports.serialized_size()
            + 8
    }

    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.id.as_u32().to_le_bytes())?;
        self.component_type.to_bytes_with_provider(writer, provider)?;
        self.exports.to_bytes_with_provider(writer, provider)?;
        self.imports.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl<Provider> wrt_foundation::traits::FromBytes for UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let mut id_bytes = [0u8; 4];
        reader.read_exact(&mut id_bytes)?;
        let id = ComponentId(u32::from_le_bytes(id_bytes));

        let component_type = ComponentType::from_bytes_with_provider(reader, provider)?;
        let exports = ExportMap::from_bytes_with_provider(reader, provider)?;
        let imports = ImportMap::from_bytes_with_provider(reader, provider)?;

        let memory_adapter = PlatformMemoryAdapter::new(64 * 1024 * 1024)
            .map_err(|e| Error::memory_error("Failed to create memory adapter"))?;

        Ok(Self {
            id,
            component_type,
            memory_adapter,
            exports,
            imports,
            linear_memory: None,
            state: ComponentExecutionState::Instantiating,
        })
    }
}

/// Component execution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentExecutionState {
    /// Component is being instantiated
    Instantiating,
    /// Component is ready for execution
    Ready,
    /// Component is currently executing
    Executing,
    /// Component execution is suspended
    Suspended,
    /// Component has completed execution
    Completed,
    /// Component execution failed
    Failed(RuntimeString),
}

impl<Provider> UnifiedComponentInstance<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Create a new component instance
    pub fn new(
        component_type: ComponentType<Provider>,
        #[cfg(any(feature = "std", feature = "alloc"))] memory_adapter: PlatformMemoryAdapter<
            DefaultRuntimeProvider,
        >,
        #[cfg(not(any(feature = "std", feature = "alloc")))] memory_adapter: PlatformMemoryAdapter<
            DefaultRuntimeProvider,
        >,
    ) -> Result<Self> {
        let exports = ExportMap::new(create_runtime_provider()?)?;
        let imports = ImportMap::new(create_runtime_provider()?)?;

        Ok(Self {
            id: ComponentId::new(),
            component_type,
            memory_adapter,
            exports,
            imports,
            linear_memory: None,
            state: ComponentExecutionState::Instantiating,
        })
    }

    /// Get the component's memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            total:     self.memory_adapter.total_memory(),
            available: self.memory_adapter.available_memory(),
            used:      self.memory_adapter.total_memory() - self.memory_adapter.available_memory(),
        }
    }

    /// Check if the component is in an executable state
    pub fn is_executable(&self) -> bool {
        matches!(
            self.state,
            ComponentExecutionState::Ready | ComponentExecutionState::Suspended
        )
    }

    /// Transition the component to ready state
    pub fn set_ready(&mut self) -> Result<()> {
        match self.state {
            ComponentExecutionState::Instantiating => {
                self.state = ComponentExecutionState::Ready;
                Ok(())
            },
            _ => Err(Error::runtime_execution_error(
                "Cannot transition to ready state from current state",
            )),
        }
    }

    /// Add an export to this component
    pub fn add_export(
        &mut self,
        name: RuntimeString,
        extern_type: ExternType<Provider>,
    ) -> Result<()> {
        self.exports
            .insert(name, extern_type)
            .map(|_| ())
            .map_err(|e| Error::runtime_error("Failed to add export"))
    }

    /// Add an import requirement to this component
    pub fn add_import(
        &mut self,
        name: RuntimeString,
        extern_type: ExternType<Provider>,
    ) -> Result<()> {
        self.imports
            .insert(name, extern_type)
            .map(|_| ())
            .map_err(|e| Error::runtime_error("Failed to add import"))
    }
}

/// Unified component runtime with external limit support
///
/// This runtime manages multiple component instances and enforces
/// platform-specific limits on resource usage, memory allocation, and component
/// interactions.
pub struct UnifiedComponentRuntime<Provider = DefaultRuntimeProvider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Collection of active component instances
    /// Using Vec because BoundedVec stores serialized data and can't return
    /// references
    #[cfg(any(feature = "std", feature = "alloc"))]
    instances: Vec<UnifiedComponentInstance<Provider>>,
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    instances: BoundedVec<UnifiedComponentInstance<Provider>, 32, DefaultRuntimeProvider>,

    /// Platform-specific limits and configuration
    #[cfg(feature = "comprehensive-limits")]
    platform_limits: wrt_platform::ComprehensivePlatformLimits,

    /// Memory budget for component operations
    memory_budget: ComponentMemoryBudget,

    /// Global memory adapter for cross-component resources
    #[cfg(any(feature = "std", feature = "alloc"))]
    global_memory_adapter: PlatformMemoryAdapter<DefaultRuntimeProvider>,

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    global_memory_adapter: PlatformMemoryAdapter<DefaultRuntimeProvider>,
}

impl<Provider> UnifiedComponentRuntime<Provider>
where
    Provider: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    /// Create a new unified component runtime
    #[cfg(feature = "comprehensive-limits")]
    pub fn new(
        limits: wrt_platform::ComprehensivePlatformLimits,
    ) -> core::result::Result<Self, wrt_error::Error> {
        let memory_budget = ComponentMemoryBudget::calculate_from_limits(&limits)?;
        let global_memory_adapter = PlatformMemoryAdapter::new(64 * 1024 * 1024)
            .map_err(|_| Error::memory_error("Failed to create memory adapter"))?;

        Ok(Self {
            #[cfg(any(feature = "std", feature = "alloc"))]
            instances: Vec::new(),
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            instances: BoundedVec::new(create_runtime_provider()?)?,
            platform_limits: limits,
            memory_budget,
            global_memory_adapter,
        })
    }

    /// Create a new unified component runtime with default limits
    #[cfg(not(feature = "comprehensive-limits"))]
    pub fn new_default() -> core::result::Result<Self, wrt_error::Error> {
        let memory_budget = ComponentMemoryBudget::default();
        let global_memory_adapter = PlatformMemoryAdapter::new(64 * 1024 * 1024)
            .map_err(|_| Error::memory_error("Failed to create memory adapter"))?; // 64MB default

        Ok(Self {
            #[cfg(any(feature = "std", feature = "alloc"))]
            instances: Vec::new(),
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            instances: BoundedVec::new(create_runtime_provider()?)?,
            memory_budget,
            global_memory_adapter,
        })
    }

    /// Instantiate a new component from bytes
    pub fn instantiate_component(
        &mut self,
        component_bytes: &[u8],
    ) -> core::result::Result<ComponentId, wrt_error::Error> {
        // Validate component against platform limits
        #[cfg(feature = "comprehensive-limits")]
        {
            let validator =
                wrt_decoder::ComprehensiveWasmValidator::new(self.platform_limits.clone())?;
            let config =
                validator.validate_comprehensive_single_pass(component_bytes, None, None)?;

            // Check memory budget
            if config.total_memory_requirement.total()
                > self.memory_budget.available_component_memory
            {
                return Err(Error::memory_error(
                    "Component memory requirements exceed available budget",
                ));
            }
        }

        // Create memory adapter for this component
        let component_memory_limit = self.memory_budget.component_overhead / 4; // Conservative allocation
        let memory_adapter = PlatformMemoryAdapter::new(component_memory_limit)
            .map_err(|_| Error::memory_error("Failed to create memory adapter"))?;

        // Parse component type from bytes (simplified)
        let component_type = ComponentType::default(); // TODO: Parse from bytes

        // Create component instance
        let mut instance = UnifiedComponentInstance::new(component_type, memory_adapter)?;

        // Initialize component
        instance.set_ready()?;

        let component_id = instance.id;

        // Add to instance collection
        self.instances.push(instance);

        Ok(component_id)
    }

    /// Get a reference to a component instance
    pub fn get_instance(&self, id: ComponentId) -> Option<&UnifiedComponentInstance<Provider>> {
        #[cfg(any(feature = "std", feature = "alloc"))]
        return self.instances.iter().find(|instance| instance.id == id);

        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // BoundedVec stores serialized data, so we can't return references
            // This is a limitation of the no_std implementation
            None
        }
    }

    /// Get a mutable reference to a component instance
    pub fn get_instance_mut(
        &mut self,
        id: ComponentId,
    ) -> Option<&mut UnifiedComponentInstance<Provider>> {
        #[cfg(any(feature = "std", feature = "alloc"))]
        return self.instances.iter_mut().find(|instance| instance.id == id);

        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // BoundedVec stores serialized data, so we can't return mutable references
            // This is a limitation of the no_std implementation
            None
        }
    }

    /// Get the number of active component instances
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Get total memory usage across all components
    pub fn total_memory_usage(&self) -> usize {
        self.instances
            .iter()
            .map(|instance| instance.memory_stats().used)
            .sum::<usize>()
            + self.global_memory_adapter.total_memory()
            - self.global_memory_adapter.available_memory()
    }

    /// Check if the runtime can accommodate a new component
    pub fn can_instantiate_component(&self, estimated_memory: usize) -> bool {
        self.total_memory_usage() + estimated_memory <= self.memory_budget.total_memory
    }
}

/// Component memory budget with platform awareness
///
/// This struct tracks memory allocation and usage for component operations,
/// ensuring that platform limits are respected and memory is efficiently
/// utilized.
#[derive(Debug, Clone)]
pub struct ComponentMemoryBudget {
    /// Total memory available for components
    pub total_memory: usize,

    /// Memory reserved for WebAssembly linear memory
    pub wasm_linear_memory: usize,

    /// Memory overhead for component model operations
    pub component_overhead: usize,

    /// Memory reserved for debug information (if enabled)
    pub debug_overhead: usize,

    /// Available memory for component instantiation
    pub available_component_memory: usize,
}

impl ComponentMemoryBudget {
    /// Calculate memory budget from platform limits
    #[cfg(feature = "comprehensive-limits")]
    pub fn calculate_from_limits(
        limits: &wrt_platform::ComprehensivePlatformLimits,
    ) -> core::result::Result<Self, wrt_error::Error> {
        let total_memory = limits.max_total_memory;
        let wasm_linear_memory = limits.max_wasm_linear_memory;
        let component_overhead = limits.estimated_component_overhead;
        let debug_overhead = limits.estimated_debug_overhead;

        let used_memory = wasm_linear_memory + component_overhead + debug_overhead;
        if used_memory > total_memory {
            return Err(Error::runtime_execution_error(
                "Component overhead exceeds available memory",
            ));
        }

        Ok(Self {
            total_memory,
            wasm_linear_memory,
            component_overhead,
            debug_overhead,
            available_component_memory: total_memory - used_memory,
        })
    }

    /// Create a default memory budget for testing
    pub fn default() -> Self {
        Self {
            total_memory:               64 * 1024 * 1024, // 64MB
            wasm_linear_memory:         32 * 1024 * 1024, // 32MB
            component_overhead:         16 * 1024 * 1024, // 16MB
            debug_overhead:             4 * 1024 * 1024,  // 4MB
            available_component_memory: 12 * 1024 * 1024, // 12MB
        }
    }

    /// Get the percentage of memory allocated to components
    pub fn component_memory_percentage(&self) -> f64 {
        if self.total_memory == 0 {
            0.0
        } else {
            (self.component_overhead as f64 / self.total_memory as f64) * 100.0
        }
    }

    /// Check if the budget allows for a specific allocation
    pub fn can_allocate(&self, size: usize, current_usage: usize) -> bool {
        current_usage + size <= self.available_component_memory
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStats {
    /// Total memory capacity
    pub total:     usize,
    /// Available memory
    pub available: usize,
    /// Used memory
    pub used:      usize,
}

impl MemoryStats {
    /// Get memory usage as a percentage
    pub fn usage_percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64) * 100.0
        }
    }

    /// Check if memory usage is above a threshold
    pub fn is_above_threshold(&self, threshold_percent: f64) -> bool {
        self.usage_percentage() > threshold_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_id_generation() {
        let id1 = ComponentId::new();
        let id2 = ComponentId::new();

        assert_ne!(id1, id2);
        assert_ne!(id1.as_u32(), id2.as_u32());
    }

    #[test]
    fn test_component_memory_budget() {
        let budget = ComponentMemoryBudget::default();

        assert!(budget.total_memory > 0);
        assert!(budget.available_component_memory <= budget.total_memory);
        assert!(budget.can_allocate(1024, 0));
        assert!(!budget.can_allocate(budget.available_component_memory + 1, 0));
    }

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats {
            total:     1000,
            available: 300,
            used:      700,
        };

        assert_eq!(stats.usage_percentage(), 70.0);
        assert!(stats.is_above_threshold(50.0));
        assert!(!stats.is_above_threshold(80.0));
    }

    #[test]
    fn test_component_execution_state() {
        let mut state = ComponentExecutionState::Instantiating;

        assert!(!matches!(state, ComponentExecutionState::Ready));

        state = ComponentExecutionState::Ready;
        assert!(matches!(state, ComponentExecutionState::Ready));
    }

    #[test]
    fn test_unified_component_runtime_creation() {
        let runtime = UnifiedComponentRuntime::<DefaultRuntimeProvider>::new_default();
        assert!(runtime.is_ok());

        let runtime = runtime.unwrap();
        assert_eq!(runtime.instance_count(), 0);
        assert!(runtime.can_instantiate_component(1024));
    }
}
