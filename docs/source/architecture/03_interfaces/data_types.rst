.. _data_types:

Data Types and Structures
==========================

This section documents the core data types and structures used throughout Pulseengine (Kiln Edition),
with specific focus on how types adapt to different runtime environments (std, no_std+alloc, no_std+no_alloc).

.. arch_interface:: ARCH_IF_TYPES_001
   :title: Type System Architecture
   :status: stable
   :version: 1.0
   :rationale: Provide consistent type definitions across all runtime environments

   Unified type system that adapts to environment capabilities while maintaining
   type safety and API compatibility.

Core Value Types
----------------

WebAssembly Value Representation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/types.rs:45-89``:

.. code-block:: rust

   /// Core WebAssembly value types
   #[derive(Debug, Clone, Copy, PartialEq)]
   pub enum ValType {
       I32,
       I64,
       F32,
       F64,
   }

   /// Runtime value representation
   #[derive(Debug, Clone, PartialEq)]
   pub enum Value {
       I32(i32),
       I64(i64),
       F32(f32),
       F64(f64),
   }

   impl Value {
       /// Get the type of this value
       pub fn ty(&self) -> ValType {
           match self {
               Value::I32(_) => ValType::I32,
               Value::I64(_) => ValType::I64,
               Value::F32(_) => ValType::F32,
               Value::F64(_) => ValType::F64,
           }
       }
       
       /// Convert to bytes representation
       pub fn to_bytes(&self) -> [u8; 8] {
           match self {
               Value::I32(v) => {
                   let mut bytes = [0u8; 8];
                   bytes[0..4].copy_from_slice(&v.to_le_bytes());
                   bytes
               }
               Value::I64(v) => v.to_le_bytes(),
               Value::F32(v) => {
                   let mut bytes = [0u8; 8];
                   bytes[0..4].copy_from_slice(&v.to_bits().to_le_bytes());
                   bytes
               }
               Value::F64(v) => v.to_bits().to_le_bytes(),
           }
       }
   }

Component Model Types
~~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/component_value.rs:67-156``:

.. code-block:: rust

   /// Component model value types (environment-adaptive)
   #[derive(Debug, Clone, PartialEq)]
   pub enum ComponentValue {
       // Primitive types
       Bool(bool),
       U8(u8),
       U16(u16),
       U32(u32),
       U64(u64),
       S8(i8),
       S16(i16),
       S32(i32),
       S64(i64),
       F32(f32),
       F64(f64),
       Char(char),
       
       // String type (environment-adaptive)
       String(BoundedString),
       
       // Container types (environment-adaptive)
       List(BoundedVec<ComponentValue>),
       Record(BoundedVec<(BoundedString, ComponentValue)>),
       Variant {
           discriminant: u32,
           value: Option<Box<ComponentValue>>,
       },
       
       // Resource types
       Resource(ResourceId),
   }

   impl ComponentValue {
       /// Get the component type of this value
       pub fn component_type(&self) -> ComponentType {
           match self {
               Self::Bool(_) => ComponentType::Bool,
               Self::U32(_) => ComponentType::U32,
               Self::String(_) => ComponentType::String,
               Self::List(items) => {
                   let element_type = items.first()
                       .map(|v| v.component_type())
                       .unwrap_or(ComponentType::Unit);
                   ComponentType::List(Box::new(element_type))
               }
               // ... other type mappings
           }
       }
   }

Environment-Adaptive Collections
--------------------------------

Bounded Collections System
~~~~~~~~~~~~~~~~~~~~~~~~~~

The core of the multi-environment type system (``kiln-foundation/src/bounded_collections.rs:15-89``):

.. code-block:: rust

   /// Environment-adaptive vector type
   #[cfg(feature = "std")]
   pub type BoundedVec<T> = std::vec::Vec<T>;

   #[cfg(all(not(feature = "std"), feature = "alloc"))]
   pub type BoundedVec<T> = alloc::vec::Vec<T>;

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub type BoundedVec<T> = heapless::Vec<T, 1024>;

   /// Environment-adaptive string type
   #[cfg(feature = "std")]
   pub type BoundedString = std::string::String;

   #[cfg(all(not(feature = "std"), feature = "alloc"))]
   pub type BoundedString = alloc::string::String;

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub type BoundedString = heapless::String<256>;

   /// Environment-adaptive hash map type
   #[cfg(feature = "std")]
   pub type BoundedMap<K, V> = std::collections::HashMap<K, V>;

   #[cfg(all(not(feature = "std"), feature = "alloc"))]
   pub type BoundedMap<K, V> = alloc::collections::BTreeMap<K, V>;

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub type BoundedMap<K, V> = heapless::FnvIndexMap<K, V, 128>;

**Collection Behavior Guarantees**:

.. list-table:: Collection Type Guarantees
   :header-rows: 1
   :widths: 20 25 25 30

   * - Operation
     - std Behavior
     - no_std+alloc Behavior
     - no_std+no_alloc Behavior
   * - ``push()``
     - Dynamic growth
     - Dynamic growth
     - Fixed capacity check
   * - ``get()``
     - O(1) access
     - O(1) access
     - O(1) access
   * - Iteration
     - Iterator trait
     - Iterator trait
     - Iterator trait
   * - Memory usage
     - Heap allocated
     - Heap allocated
     - Stack allocated

Memory Types
------------

Safe Memory Abstraction
~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/safe_memory.rs:89-178``:

.. code-block:: rust

   /// Memory region descriptor
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct MemoryRegion {
       pub base: usize,
       pub size: usize,
       pub protection: MemoryProtection,
   }

   /// Memory protection flags
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub struct MemoryProtection {
       pub read: bool,
       pub write: bool,
       pub execute: bool,
   }

   /// Linear memory representation
   pub struct LinearMemory {
       regions: BoundedVec<MemoryRegion>,
       provider: Box<dyn MemoryProvider>,
   }

   /// Environment-specific memory implementations
   #[cfg(any(feature = "std", feature = "alloc"))]
   pub struct DynamicMemory {
       data: BoundedVec<u8>,
       max_size: Option<usize>,
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct BoundedMemory {
       data: [u8; 65536],  // 64KB fixed allocation
       size: usize,
   }

   impl MemoryProvider for DynamicMemory {
       fn len(&self) -> usize {
           self.data.len()
       }
       
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError> {
           if offset.saturating_add(length) > self.data.len() {
               return Err(MemoryError::OutOfBounds { offset, length });
           }
           Ok(&self.data[offset..offset + length])
       }
   }

   impl MemoryProvider for BoundedMemory {
       fn len(&self) -> usize {
           self.size
       }
       
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError> {
           if offset.saturating_add(length) > self.size {
               return Err(MemoryError::OutOfBounds { offset, length });
           }
           Ok(&self.data[offset..offset + length])
       }
   }

Component Types
---------------

Component Metadata Types
~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/types.rs:78-145``:

.. code-block:: rust

   /// Component type information
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum ComponentType {
       // Primitive types
       Bool,
       U8, U16, U32, U64,
       S8, S16, S32, S64,
       F32, F64,
       Char,
       String,
       
       // Composite types
       List(Box<ComponentType>),
       Record(BoundedVec<(BoundedString, ComponentType)>),
       Variant(BoundedVec<(BoundedString, Option<ComponentType>)>),
       Tuple(BoundedVec<ComponentType>),
       
       // Function types
       Function {
           params: BoundedVec<ComponentType>,
           results: BoundedVec<ComponentType>,
       },
       
       // Resource types
       Resource(ResourceType),
   }

   /// Function signature
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct FunctionType {
       pub params: BoundedVec<ComponentType>,
       pub results: BoundedVec<ComponentType>,
   }

   /// Import/Export specifications
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct ImportType {
       pub module: BoundedString,
       pub name: BoundedString,
       pub ty: ComponentType,
   }

   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct ExportType {
       pub name: BoundedString,
       pub ty: ComponentType,
   }

Resource Management Types
~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/resources/resource_operation.rs:45-123``:

.. code-block:: rust

   /// Resource identifier (environment-adaptive)
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub struct ResourceId(pub u32);

   /// Resource type descriptor
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct ResourceType {
       pub name: BoundedString,
       pub size_hint: Option<usize>,
       pub alignment: usize,
   }

   /// Resource operation types
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum ResourceOperation {
       Create {
           resource_type: ResourceType,
           initial_data: BoundedVec<u8>,
       },
       Read {
           resource_id: ResourceId,
           offset: usize,
           length: usize,
       },
       Write {
           resource_id: ResourceId,
           offset: usize,
           data: BoundedVec<u8>,
       },
       Delete {
           resource_id: ResourceId,
       },
   }

   /// Resource table entry (environment-specific storage)
   pub struct ResourceEntry {
       pub resource_type: ResourceType,
       #[cfg(any(feature = "std", feature = "alloc"))]
       pub data: BoundedVec<u8>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       pub data: [u8; 1024],  // Fixed-size storage
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       pub data_len: usize,
   }

Error Types
-----------

Hierarchical Error System
~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-error/src/errors.rs:67-156``:

.. code-block:: rust

   /// Top-level error type
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum KilnError {
       Component(ComponentError),
       Runtime(RuntimeError),
       Memory(MemoryError),
       Validation(ValidationError),
       Host(HostError),
       Platform(PlatformError),
   }

   /// Component-specific errors
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum ComponentError {
       ParseError {
           offset: usize,
           message: BoundedString,
       },
       ValidationError {
           constraint: BoundedString,
           location: BoundedString,
       },
       InstantiationError {
           component_id: ComponentId,
           reason: BoundedString,
       },
       ResourceError(ResourceError),
   }

   /// Memory-specific errors
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum MemoryError {
       OutOfBounds {
           offset: usize,
           length: usize,
       },
       AllocationFailure {
           requested_size: usize,
           available_size: usize,
       },
       ProtectionViolation {
           address: usize,
           attempted_operation: MemoryOperation,
       },
   }

   /// Runtime execution errors
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum RuntimeError {
       StackOverflow {
           current_depth: usize,
           max_depth: usize,
       },
       FunctionNotFound {
           component_id: ComponentId,
           function_name: BoundedString,
       },
       TypeMismatch {
           expected: ComponentType,
           actual: ComponentType,
       },
       ExecutionTrap {
           trap_code: TrapCode,
           location: ExecutionLocation,
       },
   }

Platform Types
--------------

Platform Abstraction Types
~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-platform/src/platform_abstraction.rs:89-156``:

.. code-block:: rust

   /// Platform capability descriptor
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub struct PlatformCapabilities {
       pub has_virtual_memory: bool,
       pub has_memory_protection: bool,
       pub has_threading: bool,
       pub has_async_io: bool,
       pub page_size: usize,
       pub max_memory: Option<usize>,
   }

   /// Platform-specific memory configuration
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct MemoryConfiguration {
       pub page_size: usize,
       pub guard_pages: bool,
       pub protection_enabled: bool,
       pub numa_aware: bool,
   }

   /// Platform synchronization types
   #[derive(Debug, Clone)]
   pub enum SyncPrimitive {
       #[cfg(feature = "std")]
       StdMutex(std::sync::Mutex<()>),
       #[cfg(feature = "std")]
       StdRwLock(std::sync::RwLock<()>),
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       SpinLock(core::sync::atomic::AtomicBool),
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       NoLock(()), // Single-threaded operation
   }

Instruction Types
-----------------

WebAssembly Instruction Representation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-instructions/src/lib.rs:45-123``:

.. code-block:: rust

   /// WebAssembly instruction enumeration
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum Instruction {
       // Control instructions
       Unreachable,
       Nop,
       Block { blocktype: BlockType },
       Loop { blocktype: BlockType },
       If { blocktype: BlockType },
       Else,
       End,
       Br { relative_depth: u32 },
       BrIf { relative_depth: u32 },
       BrTable { 
           targets: BoundedVec<u32>,
           default_target: u32,
       },
       Return,
       Call { function_index: u32 },
       CallIndirect { 
           type_index: u32,
           table_index: u32,
       },
       
       // Parametric instructions
       Drop,
       Select,
       
       // Variable instructions
       LocalGet { local_index: u32 },
       LocalSet { local_index: u32 },
       LocalTee { local_index: u32 },
       GlobalGet { global_index: u32 },
       GlobalSet { global_index: u32 },
       
       // Memory instructions
       I32Load { memarg: MemArg },
       I64Load { memarg: MemArg },
       F32Load { memarg: MemArg },
       F64Load { memarg: MemArg },
       I32Store { memarg: MemArg },
       I64Store { memarg: MemArg },
       F32Store { memarg: MemArg },
       F64Store { memarg: MemArg },
       MemorySize,
       MemoryGrow,
       
       // Numeric instructions
       I32Const { value: i32 },
       I64Const { value: i64 },
       F32Const { value: f32 },
       F64Const { value: f64 },
       
       // ... arithmetic and comparison instructions
   }

   /// Memory operand descriptor
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub struct MemArg {
       pub align: u32,
       pub offset: u32,
   }

   /// Block type specification
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum BlockType {
       Empty,
       Value(ValType),
       Type(u32), // Type index
   }

Type Conversion System
----------------------

Environment-Safe Conversions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/conversion.rs:78-145``:

.. code-block:: rust

   /// Type conversion trait for environment adaptation
   pub trait IntoEnvironment<T> {
       type Error;
       
       /// Convert to environment-specific type
       fn into_env(self) -> Result<T, Self::Error>;
   }

   /// String conversion implementations
   impl IntoEnvironment<BoundedString> for &str {
       type Error = ConversionError;
       
       fn into_env(self) -> Result<BoundedString, Self::Error> {
           #[cfg(any(feature = "std", feature = "alloc"))]
           {
               Ok(BoundedString::from(self))
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               BoundedString::from_str(self)
                   .map_err(|_| ConversionError::StringTooLong {
                       length: self.len(),
                       max_length: 256,
                   })
           }
       }
   }

   /// Vec conversion implementations
   impl<T: Clone> IntoEnvironment<BoundedVec<T>> for &[T] {
       type Error = ConversionError;
       
       fn into_env(self) -> Result<BoundedVec<T>, Self::Error> {
           #[cfg(any(feature = "std", feature = "alloc"))]
           {
               Ok(BoundedVec::from(self))
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               if self.len() > 1024 {
                   return Err(ConversionError::VecTooLong {
                       length: self.len(),
                       max_length: 1024,
                   });
               }
               let mut vec = BoundedVec::new();
               for item in self {
                   vec.push(item.clone()).map_err(|_| {
                       ConversionError::VecTooLong {
                           length: self.len(),
                           max_length: 1024,
                       }
                   })?;
               }
               Ok(vec)
           }
       }
   }

Type Safety Verification
------------------------

Compile-Time Type Safety
~~~~~~~~~~~~~~~~~~~~~~~~

The type system ensures safety through compile-time verification:

.. code-block:: rust

   // Type safety is enforced at compile time
   fn type_safety_example() {
       // This enforces that the same API works across environments
       fn use_bounded_vec<T>(vec: BoundedVec<T>) -> usize {
           vec.len()  // Works in all environments
       }
       
       // Environment detection at compile time
       #[cfg(feature = "std")]
       let vec: BoundedVec<i32> = std::vec::Vec::new();
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       let vec: BoundedVec<i32> = heapless::Vec::new();
       
       // Same function call works in all environments
       let length = use_bounded_vec(vec);
   }

Runtime Type Validation
~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-component/src/validation.rs:123-189``:

.. code-block:: rust

   /// Runtime type validation for component values
   pub fn validate_component_value(
       value: &ComponentValue,
       expected_type: &ComponentType,
   ) -> Result<(), ValidationError> {
       match (value, expected_type) {
           (ComponentValue::Bool(_), ComponentType::Bool) => Ok(()),
           (ComponentValue::U32(_), ComponentType::U32) => Ok(()),
           (ComponentValue::String(s), ComponentType::String) => {
               if s.len() <= MAX_STRING_LENGTH {
                   Ok(())
               } else {
                   Err(ValidationError::StringTooLong {
                       length: s.len(),
                       max_length: MAX_STRING_LENGTH,
                   })
               }
           }
           (ComponentValue::List(items), ComponentType::List(element_type)) => {
               for item in items {
                   validate_component_value(item, element_type)?;
               }
               Ok(())
           }
           _ => Err(ValidationError::TypeMismatch {
               expected: expected_type.clone(),
               actual: value.component_type(),
           }),
       }
   }

Usage Examples
--------------

Cross-Environment Type Usage
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Example showing type usage across environments
   use kiln_foundation::{BoundedVec, BoundedString, ComponentValue};
   
   fn process_component_data() -> Result<(), KilnError> {
       // These types work identically across all environments
       let mut values = BoundedVec::new();
       
       // Add some values (works in all environments)
       values.push(ComponentValue::U32(42))?;
       values.push(ComponentValue::String("hello".into_env()?))?;
       
       // Process values (same code in all environments)
       for value in &values {
           match value {
               ComponentValue::U32(n) => println!("Number: {}", n),
               ComponentValue::String(s) => println!("String: {}", s),
               _ => {}
           }
       }
       
       Ok(())
   }

Cross-References
-----------------

.. seealso::

   * :doc:`external` for external type specifications
   * :doc:`internal` for internal type interfaces
   * :doc:`../01_architectural_design/patterns` for type system patterns
   * :doc:`../02_requirements_allocation/allocation_matrix` for type requirement mappings