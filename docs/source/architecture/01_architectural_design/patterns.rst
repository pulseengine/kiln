.. _architectural_patterns:

Architectural Patterns
======================

This section documents the architectural patterns used throughout the Pulseengine (Kiln Edition) implementation, with specific focus on how these patterns handle the multi-environment requirement (std, no_std+alloc, no_std+no_alloc).

.. arch_component:: ARCH_COMP_PATTERNS_001
   :title: Multi-Environment Pattern System
   :status: implemented
   :version: 1.0
   :rationale: Enable runtime execution across std, no_std+alloc, and no_std+no_alloc environments

   The core architectural pattern that enables Pulseengine to operate across different Rust environments
   while maintaining full feature parity.

Environment Abstraction Pattern
-------------------------------

.. arch_decision:: ARCH_DEC_PATTERNS_001
   :title: Three-Tier Environment Support
   :status: accepted
   :version: 1.0

   **Problem**: Support std, no_std+alloc, and no_std+no_alloc environments with full feature parity.

   **Decision**: Implement a three-tier abstraction pattern using conditional compilation and trait-based abstractions.

Implementation in Foundation Layer
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The pattern is implemented in ``kiln-foundation`` using conditional compilation:

.. code-block:: rust

   // From kiln-foundation/src/bounded_collections.rs
   #[cfg(feature = "std")]
   pub type BoundedVec<T> = Vec<T>;

   #[cfg(all(not(feature = "std"), feature = "alloc"))]
   pub type BoundedVec<T> = alloc::vec::Vec<T>;

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub type BoundedVec<T> = heapless::Vec<T, 1024>;

Memory Management Patterns
---------------------------

.. arch_component:: ARCH_COMP_PATTERNS_002
   :title: Adaptive Memory Management
   :status: implemented
   :version: 1.0
   :rationale: Provide consistent memory interface across environments

   Memory management adapts to available environment capabilities while maintaining
   the same API surface.

Safe Memory Provider Pattern
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implemented in ``kiln-foundation/src/safe_memory.rs``:

.. code-block:: rust

   pub trait MemoryProvider: Clone + PartialEq + Eq {
       fn len(&self) -> usize;
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError>;
       fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), MemoryError>;
   }

   // Environment-specific implementations
   #[cfg(feature = "std")]
   impl MemoryProvider for StandardMemory { /* std implementation */ }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   impl MemoryProvider for BoundedMemory { /* no_alloc implementation */ }

Builder Pattern with Environment Adaptation
--------------------------------------------

.. arch_component:: ARCH_COMP_PATTERNS_003
   :title: Environment-Aware Builder Pattern
   :status: implemented
   :version: 1.0
   :rationale: Provide consistent configuration API across environments

   Builder pattern that adapts its internal storage and validation based on
   the target environment.

Component Builder Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/component_builder.rs``:

.. code-block:: rust

   pub struct ComponentBuilder {
       #[cfg(feature = "std")]
       imports: std::collections::HashMap<String, ImportValue>,
       
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       imports: alloc::collections::BTreeMap<String, ImportValue>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       imports: heapless::FnvIndexMap<String, ImportValue, 64>,
   }

   impl ComponentBuilder {
       pub fn new() -> Self {
           Self {
               imports: Default::default(),
           }
       }
       
       pub fn add_import(&mut self, name: impl Into<String>, value: ImportValue) -> &mut Self {
           #[cfg(any(feature = "std", feature = "alloc"))]
           {
               self.imports.insert(name.into(), value);
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               let _ = self.imports.insert(name.into(), value);
           }
           
           self
       }
   }

Error Handling Patterns
------------------------

.. arch_component:: ARCH_COMP_PATTERNS_004
   :title: Unified Error Handling
   :status: implemented
   :version: 1.0
   :rationale: Consistent error handling across all environments

   Error handling that works in no_std environments while providing rich
   diagnostics when possible.

No-Std Error Pattern
~~~~~~~~~~~~~~~~~~~~

From ``kiln-error/src/errors.rs``:

.. code-block:: rust

   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum KilnError {
       Memory(MemoryError),
       Component(ComponentError),
       Runtime(RuntimeError),
   }

   #[cfg(feature = "std")]
   impl std::error::Error for KilnError {}

   impl core::fmt::Display for KilnError {
       fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
           match self {
               Self::Memory(e) => write!(f, "Memory error: {}", e),
               Self::Component(e) => write!(f, "Component error: {}", e),
               Self::Runtime(e) => write!(f, "Runtime error: {}", e),
           }
       }
   }

Resource Management Patterns
-----------------------------

.. arch_component:: ARCH_COMP_PATTERNS_005
   :title: Bounded Resource Management
   :status: implemented
   :version: 1.0
   :rationale: Ensure deterministic resource usage in no_alloc environments

   Resource management patterns that provide compile-time bounds checking
   for no_alloc environments.

Resource Table Pattern
~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/resources/resource_table_no_std.rs``:

.. code-block:: rust

   pub struct ResourceTable {
       #[cfg(feature = "std")]
       resources: std::collections::HashMap<ResourceId, Box<dyn Any>>,
       
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       resources: alloc::collections::BTreeMap<ResourceId, Box<dyn Any>>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       resources: heapless::FnvIndexMap<ResourceId, ResourceSlot, 256>,
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct ResourceSlot {
       data: [u8; 128],  // Fixed-size storage
       type_id: core::any::TypeId,
       in_use: bool,
   }

Factory Pattern with Environment Constraints
---------------------------------------------

.. arch_component:: ARCH_COMP_PATTERNS_006
   :title: Environment-Constrained Factory Pattern
   :status: implemented
   :version: 1.0
   :rationale: Create components with appropriate constraints for target environment

   Factory pattern that enforces environment-specific constraints at compile time.

Component Factory Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/factory.rs``:

.. code-block:: rust

   pub trait ComponentFactory {
       type Component;
       type Config;
       type Error;

       fn create(&self, config: Self::Config) -> Result<Self::Component, Self::Error>;
   }

   // No-alloc factory with compile-time bounds
   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct BoundedComponentFactory<const MAX_COMPONENTS: usize> {
       components: heapless::Vec<ComponentSlot, MAX_COMPONENTS>,
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   impl<const MAX_COMPONENTS: usize> ComponentFactory for BoundedComponentFactory<MAX_COMPONENTS> {
       type Component = ComponentId;
       type Config = ComponentConfig;
       type Error = ComponentError;

       fn create(&self, config: Self::Config) -> Result<Self::Component, Self::Error> {
           if self.components.len() >= MAX_COMPONENTS {
               return Err(ComponentError::ResourceLimitExceeded);
           }
           // Implementation continues...
       }
   }

Observer Pattern for Runtime Events
------------------------------------

.. arch_component:: ARCH_COMP_PATTERNS_007
   :title: No-Alloc Observer Pattern
   :status: implemented
   :version: 1.0
   :rationale: Enable event notification without dynamic allocation

   Observer pattern implementation that works in no_alloc environments using
   fixed-size observer arrays.

Runtime Event System
~~~~~~~~~~~~~~~~~~~~

From ``kiln-runtime/src/execution.rs``:

.. code-block:: rust

   pub trait RuntimeObserver {
       fn on_component_created(&self, component_id: ComponentId);
       fn on_function_called(&self, function_name: &str);
       fn on_memory_allocated(&self, size: usize);
   }

   pub struct RuntimeEventSystem {
       #[cfg(feature = "std")]
       observers: Vec<Box<dyn RuntimeObserver>>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       observers: heapless::Vec<&'static dyn RuntimeObserver, 16>,
   }

Type-State Pattern for Safety
------------------------------

.. arch_component:: ARCH_COMP_PATTERNS_008
   :title: Compile-Time State Validation
   :status: implemented
   :version: 1.0
   :rationale: Use Rust's type system to enforce correct component lifecycle

   Type-state pattern that prevents invalid operations at compile time.

Component Lifecycle States
~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/component.rs``:

.. code-block:: rust

   pub struct Component<S> {
       inner: ComponentInner,
       _state: core::marker::PhantomData<S>,
   }

   pub struct Uninitialized;
   pub struct Initialized;
   pub struct Running;

   impl Component<Uninitialized> {
       pub fn new() -> Self { /* ... */ }
       
       pub fn initialize(self, config: ComponentConfig) 
           -> Result<Component<Initialized>, ComponentError> {
           // Can only initialize from Uninitialized state
       }
   }

   impl Component<Initialized> {
       pub fn start(self) -> Result<Component<Running>, ComponentError> {
           // Can only start from Initialized state
       }
   }

   impl Component<Running> {
       pub fn execute(&self, function: &str, args: &[Value]) -> Result<Value, RuntimeError> {
           // Can only execute when Running
       }
   }

Pattern Cross-References
-------------------------

For detailed implementation examples, see:

* :ref:`safe_memory_system` - MemoryProvider pattern implementation
* :ref:`component_model` - Component lifecycle and factory patterns
* :ref:`resource_management` - Resource table and bounded management patterns
* :ref:`platform_layer` - Environment abstraction implementations

.. seealso::

   * :doc:`../02_requirements_allocation/allocation_matrix` for pattern-to-requirement mappings
   * :doc:`../03_interfaces/interface_catalog` for pattern interface definitions
   * :doc:`../06_design_decisions/adr/adr-001-memory-allocation-strategy` for memory pattern rationale