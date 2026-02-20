.. _internal_interfaces:

Internal Interfaces
===================

This section documents the internal interfaces between components within Pulseengine (Kiln Edition),
focusing on how components communicate and collaborate across different runtime environments.

.. arch_interface:: ARCH_IF_INT_001
   :title: Component-to-Component Interface System
   :status: stable
   :version: 1.0
   :rationale: Enable clean separation and communication between architectural components

   Internal interface system that maintains consistent communication patterns across
   std, no_std+alloc, and no_std+no_alloc environments.

Foundation Layer Interfaces
---------------------------

Memory Provider Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

The core memory abstraction used throughout the system (``kiln-foundation/src/safe_memory.rs:45-89``):

.. code-block:: rust

   /// Core memory provider trait - used by all components
   pub trait MemoryProvider: Clone + PartialEq + Eq + Send + Sync {
       /// Get the size of the memory region
       fn len(&self) -> usize;
       
       /// Check if memory is empty
       fn is_empty(&self) -> bool {
           self.len() == 0
       }
       
       /// Read bytes from memory with bounds checking
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError>;
       
       /// Write bytes to memory with bounds checking
       fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), MemoryError>;
       
       /// Get a slice view of the entire memory
       fn as_slice(&self) -> &[u8];
       
       /// Get a mutable slice view of the entire memory
       fn as_mut_slice(&mut self) -> &mut [u8];
   }

   // Environment-specific implementations communicate through this interface
   impl MemoryProvider for StandardMemory { /* std implementation */ }
   impl MemoryProvider for BoundedMemory { /* no_alloc implementation */ }

**Usage across components**:
- ``kiln-runtime``: Uses for linear memory management
- ``kiln-component``: Uses for component memory regions  
- ``kiln-decoder``: Uses for parsing buffer management

Component Value Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

Value exchange between components (``kiln-foundation/src/component_value.rs:67-123``):

.. code-block:: rust

   /// Internal value representation for component communication
   #[derive(Debug, Clone, PartialEq)]
   pub enum ComponentValue {
       I32(i32),
       I64(i64), 
       F32(f32),
       F64(f64),
       String(BoundedString),
       Bytes(BoundedVec<u8>),
       List(BoundedVec<ComponentValue>),
       Record(BoundedVec<(BoundedString, ComponentValue)>),
   }

   impl ComponentValue {
       /// Convert to external Value type
       pub fn to_external(&self) -> Result<Value, ConversionError> {
           match self {
               Self::I32(v) => Ok(Value::I32(*v)),
               Self::I64(v) => Ok(Value::I64(*v)),
               Self::String(s) => Ok(Value::String(s.as_str().into())),
               // ... conversion implementations
           }
       }
       
       /// Create from external Value type  
       pub fn from_external(value: &Value) -> Result<Self, ConversionError> {
           match value {
               Value::I32(v) => Ok(Self::I32(*v)),
               Value::I64(v) => Ok(Self::I64(*v)),
               Value::String(s) => {
                   let bounded = BoundedString::try_from(s.as_str())?;
                   Ok(Self::String(bounded))
               }
               // ... conversion implementations
           }
       }
   }

Runtime-Component Interface
---------------------------

.. arch_interface:: ARCH_IF_INT_002
   :title: Runtime-Component Communication
   :status: stable
   :version: 1.0
   :rationale: Enable runtime to manage and execute components

   Interface between the runtime engine and component instances.

Component Instance Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-runtime/src/component_impl.rs:89-156``:

.. code-block:: rust

   /// Internal component instance interface
   pub trait ComponentInstance {
       /// Get component metadata
       fn metadata(&self) -> &ComponentMetadata;
       
       /// Execute a function in this component
       fn execute(&mut self, function: &str, args: &[ComponentValue]) 
           -> Result<ComponentValue, ExecutionError>;
       
       /// Get component exports
       fn exports(&self) -> &ExportTable;
       
       /// Get component imports
       fn imports(&self) -> &ImportTable;
       
       /// Get component memory
       fn memory(&self) -> Option<&dyn MemoryProvider>;
       
       /// Get mutable component memory
       fn memory_mut(&mut self) -> Option<&mut dyn MemoryProvider>;
   }

   // Environment-specific implementations
   pub struct StandardComponentInstance {
       #[cfg(feature = "std")]
       exports: HashMap<String, ExportEntry>,
       #[cfg(feature = "std")]
       memory: Option<StandardMemory>,
   }

   pub struct BoundedComponentInstance {
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       exports: heapless::FnvIndexMap<&'static str, ExportEntry, 128>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       memory: Option<BoundedMemory>,
   }

Module Builder Interface
~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-runtime/src/module_builder.rs:78-134``:

.. code-block:: rust

   /// Internal module building interface
   pub trait ModuleBuilder {
       type Module;
       type Error;
       
       /// Add a function to the module
       fn add_function(&mut self, name: &str, func: Function) -> Result<(), Self::Error>;
       
       /// Add memory to the module
       fn add_memory(&mut self, memory: Memory) -> Result<(), Self::Error>;
       
       /// Add a table to the module
       fn add_table(&mut self, table: Table) -> Result<(), Self::Error>;
       
       /// Build the final module
       fn build(self) -> Result<Self::Module, Self::Error>;
   }

   // Environment-specific builders
   impl ModuleBuilder for StandardModuleBuilder {
       type Module = StandardModule;
       type Error = ModuleBuilderError;
       // Dynamic allocation implementation
   }

   impl ModuleBuilder for BoundedModuleBuilder {
       type Module = BoundedModule;  
       type Error = ModuleBuilderError;
       // Fixed allocation implementation
   }

Decoder-Component Interface
---------------------------

.. arch_interface:: ARCH_IF_INT_003
   :title: Decoder-Component Communication
   :status: stable
   :version: 1.0
   :rationale: Enable component creation from decoded WebAssembly modules

   Interface between the decoder and component management systems.

Parser Interface
~~~~~~~~~~~~~~~~

From ``kiln-decoder/src/parser.rs:123-189``:

.. code-block:: rust

   /// Internal parsing interface for WebAssembly modules
   pub trait WasmParser {
       type Output;
       type Error;
       
       /// Parse WebAssembly bytes into internal representation
       fn parse(&mut self, bytes: &[u8]) -> Result<Self::Output, Self::Error>;
       
       /// Validate parsed module
       fn validate(&self, module: &Self::Output) -> Result<(), Self::Error>;
       
       /// Extract module metadata
       fn extract_metadata(&self, module: &Self::Output) -> ModuleMetadata;
   }

   // Component-specific parser
   pub struct ComponentParser {
       validator: ComponentValidator,
       #[cfg(feature = "std")]
       sections: Vec<Section>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       sections: heapless::Vec<Section, 64>,
   }

   impl WasmParser for ComponentParser {
       type Output = ParsedComponent;
       type Error = ParseError;
       
       fn parse(&mut self, bytes: &[u8]) -> Result<Self::Output, Self::Error> {
           let mut reader = SectionReader::new(bytes);
           
           while let Some(section) = reader.next_section()? {
               self.sections.push(section)?;
           }
           
           Ok(ParsedComponent {
               sections: core::mem::take(&mut self.sections),
               metadata: self.extract_metadata_internal()?,
           })
       }
   }

Section Reader Interface
~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-decoder/src/section_reader.rs:67-123``:

.. code-block:: rust

   /// Internal interface for reading WebAssembly sections
   pub trait SectionReader {
       /// Read the next section from the byte stream
       fn next_section(&mut self) -> Result<Option<Section>, ParseError>;
       
       /// Peek at the next section type without consuming
       fn peek_section_type(&self) -> Result<Option<SectionType>, ParseError>;
       
       /// Skip the current section
       fn skip_section(&mut self) -> Result<(), ParseError>;
       
       /// Get current position in byte stream
       fn position(&self) -> usize;
   }

   pub struct StreamingSectionReader<'a> {
       bytes: &'a [u8],
       position: usize,
       #[cfg(feature = "std")]
       buffer: Vec<u8>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       buffer: [u8; 1024],
   }

Resource Management Interface
-----------------------------

.. arch_interface:: ARCH_IF_INT_004
   :title: Resource Management Communication
   :status: stable
   :version: 1.0
   :rationale: Enable controlled resource allocation and lifecycle management

   Internal interfaces for managing component resources across environments.

Resource Table Interface
~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/resources/resource_table.rs:89-156``:

.. code-block:: rust

   /// Internal resource management interface
   pub trait ResourceTable {
       type ResourceId;
       type Error;
       
       /// Allocate a new resource
       fn allocate<T: Any>(&mut self, resource: T) -> Result<Self::ResourceId, Self::Error>;
       
       /// Get a resource by ID
       fn get<T: Any>(&self, id: Self::ResourceId) -> Result<&T, Self::Error>;
       
       /// Get a mutable resource by ID
       fn get_mut<T: Any>(&mut self, id: Self::ResourceId) -> Result<&mut T, Self::Error>;
       
       /// Deallocate a resource
       fn deallocate(&mut self, id: Self::ResourceId) -> Result<(), Self::Error>;
       
       /// Check if resource exists
       fn contains(&self, id: Self::ResourceId) -> bool;
   }

   // Environment-specific implementations
   pub struct DynamicResourceTable {
       #[cfg(feature = "std")]
       resources: HashMap<ResourceId, Box<dyn Any>>,
       next_id: ResourceId,
   }

   pub struct BoundedResourceTable {
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       resources: heapless::Pool<ResourceSlot, 256>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       id_map: heapless::FnvIndexMap<ResourceId, usize, 256>,
   }

Resource Strategy Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/resources/resource_strategy.rs:78-134``:

.. code-block:: rust

   /// Internal resource allocation strategy interface
   pub trait ResourceStrategy {
       type ResourceId;
       type Error;
       
       /// Determine allocation strategy for resource type
       fn allocation_strategy<T: Any>(&self) -> AllocationStrategy;
       
       /// Validate resource allocation request
       fn validate_allocation<T: Any>(&self, size_hint: Option<usize>) -> Result<(), Self::Error>;
       
       /// Handle resource deallocation
       fn handle_deallocation(&mut self, id: Self::ResourceId) -> Result<(), Self::Error>;
   }

   #[derive(Debug, Clone, Copy)]
   pub enum AllocationStrategy {
       /// Dynamic heap allocation (std, no_std+alloc)
       Dynamic,
       /// Fixed pool allocation (no_std+no_alloc)
       Pool { pool_id: usize },
       /// Stack allocation (no_std+no_alloc, small objects)
       Stack,
   }

Platform Interface Layer
------------------------

.. arch_interface:: ARCH_IF_INT_005
   :title: Platform Abstraction Interface
   :status: stable
   :version: 1.0
   :rationale: Enable platform-specific optimizations while maintaining portability

   Internal interfaces for platform-specific functionality.

Synchronization Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-platform/src/sync.rs:67-123``:

.. code-block:: rust

   /// Internal synchronization primitive interface
   pub trait SyncProvider {
       type Mutex<T>: MutexLike<T>;
       type RwLock<T>: RwLockLike<T>;
       type Once: OnceLike;
       
       /// Create a new mutex
       fn create_mutex<T>(&self, value: T) -> Self::Mutex<T>;
       
       /// Create a new read-write lock
       fn create_rwlock<T>(&self, value: T) -> Self::RwLock<T>;
       
       /// Create a new once cell
       fn create_once(&self) -> Self::Once;
   }

   // Platform-specific implementations
   #[cfg(target_os = "linux")]
   impl SyncProvider for LinuxSync {
       type Mutex<T> = std::sync::Mutex<T>;
       type RwLock<T> = std::sync::RwLock<T>;
       type Once = std::sync::Once;
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   impl SyncProvider for EmbeddedSync {
       type Mutex<T> = heapless::pool::Mutex<T>;
       type RwLock<T> = heapless::pool::RwLock<T>;
       type Once = heapless::pool::Once;
   }

Memory Platform Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-platform/src/memory.rs:89-145``:

.. code-block:: rust

   /// Internal platform memory interface
   pub trait PlatformMemory {
       type Error;
       
       /// Allocate platform-specific memory region
       fn allocate(&self, size: usize, alignment: usize) -> Result<*mut u8, Self::Error>;
       
       /// Deallocate platform-specific memory region
       fn deallocate(&self, ptr: *mut u8, size: usize, alignment: usize) -> Result<(), Self::Error>;
       
       /// Get platform memory capabilities
       fn capabilities(&self) -> MemoryCapabilities;
       
       /// Protect memory region
       fn protect(&self, ptr: *mut u8, size: usize, protection: Protection) -> Result<(), Self::Error>;
   }

   #[derive(Debug, Clone, Copy)]
   pub struct MemoryCapabilities {
       pub has_virtual_memory: bool,
       pub has_memory_protection: bool,
       pub page_size: usize,
       pub max_allocation: Option<usize>,
   }

Error Propagation Interface
---------------------------

.. arch_interface:: ARCH_IF_INT_006
   :title: Error Propagation System
   :status: stable
   :version: 1.0
   :rationale: Enable consistent error handling across component boundaries

   Internal error communication and conversion interfaces.

Error Conversion Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-error/src/context.rs:78-134``:

.. code-block:: rust

   /// Internal error conversion interface
   pub trait ErrorContext {
       type Error;
       
       /// Add context to an error
       fn with_context<F>(self, f: F) -> ContextError<Self::Error>
       where
           F: FnOnce() -> BoundedString;
       
       /// Convert to external error type
       fn to_external(self) -> KilnError;
   }

   pub struct ContextError<E> {
       error: E,
       context: BoundedString,
       #[cfg(feature = "std")]
       backtrace: Option<std::backtrace::Backtrace>,
   }

   // Component-specific error conversions
   impl From<ComponentError> for KilnError {
       fn from(err: ComponentError) -> Self {
           KilnError::Component(err)
       }
   }

   impl From<RuntimeError> for KilnError {
       fn from(err: RuntimeError) -> Self {
           KilnError::Runtime(err)
       }
   }

Validation Interface
--------------------

.. arch_interface:: ARCH_IF_INT_007
   :title: Validation Interface System
   :status: stable
   :version: 1.0
   :rationale: Enable consistent validation across component boundaries

   Internal validation interfaces for ensuring component and data integrity.

Component Validation Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/validation.rs:89-156``:

.. code-block:: rust

   /// Internal component validation interface
   pub trait ComponentValidator {
       type Error;
       
       /// Validate component structure
       fn validate_structure(&self, component: &ParsedComponent) -> Result<(), Self::Error>;
       
       /// Validate component types
       fn validate_types(&self, component: &ParsedComponent) -> Result<(), Self::Error>;
       
       /// Validate component imports/exports
       fn validate_interfaces(&self, component: &ParsedComponent) -> Result<(), Self::Error>;
       
       /// Validate resource usage
       fn validate_resources(&self, component: &ParsedComponent) -> Result<(), Self::Error>;
   }

   pub struct StandardValidator {
       #[cfg(feature = "std")]
       type_cache: HashMap<TypeId, TypeInfo>,
   }

   pub struct BoundedValidator {
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       type_cache: heapless::FnvIndexMap<TypeId, TypeInfo, 128>,
   }

Interface Testing and Verification
-----------------------------------

Interface Contract Testing
~~~~~~~~~~~~~~~~~~~~~~~~~~

Internal interfaces are verified through contract testing:

.. code-block:: rust

   // Example from tests/interface_contracts_test.rs
   #[test]
   fn test_memory_provider_contract() {
       fn verify_memory_provider<M: MemoryProvider>(mut provider: M) {
           // Test contract requirements
           assert_eq!(provider.is_empty(), provider.len() == 0);
           
           // Test bounds checking
           let result = provider.read_bytes(provider.len(), 1);
           assert!(matches!(result, Err(MemoryError::OutOfBounds { .. })));
       }
       
       // Test all implementations
       verify_memory_provider(StandardMemory::new(1024));
       verify_memory_provider(BoundedMemory::new());
   }

Cross-Environment Testing
~~~~~~~~~~~~~~~~~~~~~~~~~

Interface compatibility across environments is verified:

.. code-block:: rust

   // Tests that interfaces work across environments
   #[test]
   fn test_value_conversion_interfaces() {
       let external_value = Value::I32(42);
       let internal_value = ComponentValue::from_external(&external_value).unwrap();
       let converted_back = internal_value.to_external().unwrap();
       
       assert_eq!(external_value, converted_back);
   }

Cross-References
-----------------

.. seealso::

   * :doc:`external` for external API interfaces
   * :doc:`api_contracts` for detailed interface contracts
   * :doc:`../01_architectural_design/components` for component implementation details
   * :doc:`../01_architectural_design/patterns` for interface design patterns