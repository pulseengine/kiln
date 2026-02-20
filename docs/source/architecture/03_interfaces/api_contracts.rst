.. _api_contracts:

API Contracts and Specifications
=================================

This section defines the formal contracts and specifications for all APIs in Pulseengine (Kiln Edition),
ensuring consistent behavior across std, no_std+alloc, and no_std+no_alloc environments.

.. arch_interface:: ARCH_IF_CONTRACT_001
   :title: API Contract Specification System
   :status: stable
   :version: 1.0
   :rationale: Ensure consistent API behavior across all environments and use cases

   Formal contracts that define expected behavior, error conditions, and performance
   characteristics for all public and internal APIs.

Memory Provider Contracts
-------------------------

Core Memory Provider Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Contract Definition** (``kiln-foundation/src/safe_memory.rs:45-89``):

.. code-block:: rust

   /// Memory Provider Contract
   /// 
   /// INVARIANTS:
   /// 1. len() returns the actual size of the memory region
   /// 2. is_empty() ≡ (len() == 0)
   /// 3. read_bytes(offset, length) succeeds iff offset + length <= len()
   /// 4. write_bytes(offset, data) succeeds iff offset + data.len() <= len()
   /// 5. as_slice().len() == len()
   /// 6. Multiple readers OR single writer (no data races)
   ///
   /// PRECONDITIONS:
   /// - offset and length parameters must not overflow when added
   /// - data parameter must be valid for the lifetime of the call
   ///
   /// POSTCONDITIONS:
   /// - Successful operations do not modify the memory size
   /// - Failed operations do not modify memory contents
   /// - Error conditions are properly reported
   pub trait MemoryProvider: Clone + PartialEq + Eq + Send + Sync {
       /// Get memory size in bytes
       /// 
       /// CONTRACT: Always returns the same value unless memory is grown
       /// COMPLEXITY: O(1)
       /// ERRORS: None
       fn len(&self) -> usize;
       
       /// Check if memory is empty
       /// 
       /// CONTRACT: Returns true iff len() == 0
       /// COMPLEXITY: O(1)  
       /// ERRORS: None
       fn is_empty(&self) -> bool {
           self.len() == 0
       }
       
       /// Read bytes from memory
       ///
       /// CONTRACT: 
       /// - Returns slice of exactly `length` bytes starting at `offset`
       /// - Slice is valid for the lifetime of the borrow
       /// - Does not modify memory contents
       ///
       /// PRECONDITIONS:
       /// - offset + length must not overflow
       /// - offset + length <= len()
       ///
       /// POSTCONDITIONS:
       /// - On success: returned slice has length `length`
       /// - On failure: memory is unchanged
       ///
       /// COMPLEXITY: O(1)
       /// ERRORS: MemoryError::OutOfBounds if bounds check fails
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError>;
       
       /// Write bytes to memory
       ///
       /// CONTRACT:
       /// - Writes exactly data.len() bytes starting at `offset`
       /// - Overwrites existing data at those positions
       /// - Does not change memory size
       ///
       /// PRECONDITIONS:
       /// - offset + data.len() must not overflow
       /// - offset + data.len() <= len()
       ///
       /// POSTCONDITIONS:
       /// - On success: memory[offset..offset+data.len()] == data
       /// - On failure: memory is unchanged
       ///
       /// COMPLEXITY: O(data.len())
       /// ERRORS: MemoryError::OutOfBounds if bounds check fails
       fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), MemoryError>;
   }

**Environment-Specific Contract Fulfillment**:

.. list-table:: Memory Provider Contract Compliance
   :header-rows: 1
   :widths: 20 25 25 30

   * - Requirement
     - std Implementation
     - no_std+alloc Implementation
     - no_std+no_alloc Implementation
   * - Memory bounds checking
     - ✅ Vec bounds check
     - ✅ Vec bounds check
     - ✅ Array bounds check
   * - Thread safety
     - ✅ Send + Sync
     - ✅ Send + Sync
     - ✅ Send + Sync
   * - O(1) access
     - ✅ Direct indexing
     - ✅ Direct indexing
     - ✅ Direct indexing
   * - No panics
     - ✅ Result<T, E> errors
     - ✅ Result<T, E> errors
     - ✅ Result<T, E> errors

Component Instance Contracts
----------------------------

Component Execution Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Contract Definition** (``kiln-runtime/src/component_impl.rs:89-156``):

.. code-block:: rust

   /// Component Instance Execution Contract
   ///
   /// INVARIANTS:
   /// 1. Component state is consistent before and after execution
   /// 2. Memory is preserved across function calls unless explicitly modified
   /// 3. Function execution is deterministic given same inputs
   /// 4. Resource limits are enforced
   ///
   /// PRECONDITIONS:
   /// - Component must be in initialized state
   /// - Function name must exist in component exports
   /// - Arguments must match function signature
   ///
   /// POSTCONDITIONS:
   /// - Component state is valid (no corruption)
   /// - Return value matches function signature
   /// - Side effects are contained within component boundaries
   pub trait ComponentInstance {
       /// Execute a function in the component
       ///
       /// CONTRACT:
       /// - Function execution is atomic (all-or-nothing)
       /// - Memory changes are committed only on success
       /// - Stack/resource usage is bounded
       ///
       /// PRECONDITIONS:
       /// - function must be a valid export name
       /// - args must match the function's parameter types
       /// - Component must not be in error state
       ///
       /// POSTCONDITIONS:
       /// - On success: return value matches function return type
       /// - On failure: component state is unchanged
       /// - Resource usage is tracked and enforced
       ///
       /// COMPLEXITY: Depends on function implementation
       /// ERRORS: ExecutionError for various failure modes
       fn execute(&mut self, function: &str, args: &[ComponentValue]) 
           -> Result<ComponentValue, ExecutionError>;
   }

**Contract Verification**:

.. code-block:: rust

   // Contract test from tests/component_contracts_test.rs
   #[test]
   fn verify_component_execution_contract() {
       let mut component = create_test_component();
       let initial_memory = component.memory().unwrap().as_slice().to_vec();
       
       // Test successful execution
       let result = component.execute("add", &[
           ComponentValue::I32(5),
           ComponentValue::I32(3)
       ]);
       assert_eq!(result.unwrap(), ComponentValue::I32(8));
       
       // Test that failed execution doesn't corrupt state
       let error_result = component.execute("divide", &[
           ComponentValue::I32(5),
           ComponentValue::I32(0)  // Division by zero
       ]);
       assert!(error_result.is_err());
       
       // Verify memory is unchanged after error
       assert_eq!(component.memory().unwrap().as_slice(), initial_memory);
   }

Resource Management Contracts
-----------------------------

Resource Table Contract
~~~~~~~~~~~~~~~~~~~~~~~

**Contract Definition** (``kiln-component/src/resources/resource_table.rs:89-156``):

.. code-block:: rust

   /// Resource Table Management Contract
   ///
   /// INVARIANTS:
   /// 1. Resource IDs are unique within a table
   /// 2. Allocated resources remain valid until explicitly deallocated
   /// 3. Resource type safety is maintained
   /// 4. Resource limits are enforced in no_alloc environments
   ///
   /// PRECONDITIONS:
   /// - Resource type T must be 'static
   /// - Resource allocation must not exceed environment limits
   ///
   /// POSTCONDITIONS:
   /// - Successful allocation returns unique ResourceId
   /// - Resource can be retrieved using returned ID
   /// - Deallocation invalidates the ResourceId
   pub trait ResourceTable {
       type ResourceId: Copy + Eq + Hash;
       type Error;
       
       /// Allocate a new resource
       ///
       /// CONTRACT:
       /// - Returns unique ID for successful allocation
       /// - Resource remains valid until deallocated
       /// - Type information is preserved
       ///
       /// PRECONDITIONS:
       /// - Available resource slots (no_alloc environments)
       /// - Memory available for resource storage
       ///
       /// POSTCONDITIONS:
       /// - On success: resource is stored and accessible via returned ID
       /// - On failure: no state changes occur
       ///
       /// COMPLEXITY: O(1) average, O(log n) worst case
       /// ERRORS: ResourceError::OutOfMemory, ResourceError::LimitExceeded
       fn allocate<T: Any>(&mut self, resource: T) -> Result<Self::ResourceId, Self::Error>;
       
       /// Retrieve a resource by ID
       ///
       /// CONTRACT:
       /// - Returns reference to resource if ID is valid and type matches
       /// - Reference is valid for the lifetime of the borrow
       /// - Type safety is enforced at runtime
       ///
       /// PRECONDITIONS:
       /// - ID must have been returned by allocate()
       /// - ID must not have been deallocated
       /// - Type T must match allocated type
       ///
       /// POSTCONDITIONS:
       /// - On success: returned reference points to correct resource
       /// - On failure: no state changes occur
       ///
       /// COMPLEXITY: O(1) average, O(log n) worst case
       /// ERRORS: ResourceError::NotFound, ResourceError::TypeMismatch
       fn get<T: Any>(&self, id: Self::ResourceId) -> Result<&T, Self::Error>;
   }

**Environment-Specific Contract Implementation**:

.. list-table:: Resource Table Contract Implementation
   :header-rows: 1
   :widths: 25 25 25 25

   * - Contract Requirement
     - std Implementation
     - no_std+alloc Implementation  
     - no_std+no_alloc Implementation
   * - Unique ID generation
     - ✅ Atomic counter
     - ✅ Atomic counter
     - ✅ Pool index
   * - Type safety
     - ✅ TypeId + Any
     - ✅ TypeId + Any
     - ✅ Manual type tracking
   * - Memory management
     - ✅ Box<dyn Any>
     - ✅ Box<dyn Any>
     - ✅ Fixed pools
   * - Resource limits
     - ✅ Configurable
     - ✅ Configurable
     - ✅ Compile-time bounds

Parser Interface Contracts
---------------------------

WebAssembly Parser Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Contract Definition** (``kiln-decoder/src/parser.rs:123-189``):

.. code-block:: rust

   /// WebAssembly Parser Contract
   ///
   /// INVARIANTS:
   /// 1. Parser validates WebAssembly format compliance
   /// 2. Malformed input produces appropriate errors
   /// 3. Valid input produces correct internal representation
   /// 4. Parser is stateless between calls
   ///
   /// PRECONDITIONS:
   /// - Input bytes must be valid memory region
   /// - Parser must be properly initialized
   ///
   /// POSTCONDITIONS:
   /// - On success: output represents valid WebAssembly module/component
   /// - On failure: detailed error information is provided
   /// - Parser state is not modified by parse operation
   pub trait WasmParser {
       type Output;
       type Error: core::fmt::Debug + core::fmt::Display;
       
       /// Parse WebAssembly bytes
       ///
       /// CONTRACT:
       /// - Validates WebAssembly format according to specification
       /// - Produces deterministic output for identical input
       /// - Fails fast on format violations
       ///
       /// PRECONDITIONS:
       /// - bytes must contain valid memory region
       /// - bytes.len() must be > 0
       ///
       /// POSTCONDITIONS:
       /// - On success: Output represents valid parsed module
       /// - On failure: Error describes specific violation
       /// - Parser internal state is unchanged
       ///
       /// COMPLEXITY: O(bytes.len())
       /// ERRORS: ParseError with specific error codes
       fn parse(&mut self, bytes: &[u8]) -> Result<Self::Output, Self::Error>;
       
       /// Validate parsed output
       ///
       /// CONTRACT:
       /// - Performs semantic validation beyond format parsing
       /// - Checks type consistency and constraint satisfaction
       /// - Validation is independent of parsing
       ///
       /// PRECONDITIONS:
       /// - module must be output from successful parse() call
       ///
       /// POSTCONDITIONS:
       /// - On success: module is semantically valid
       /// - On failure: specific validation error is reported
       ///
       /// COMPLEXITY: O(module complexity)
       /// ERRORS: ValidationError with constraint violations
       fn validate(&self, module: &Self::Output) -> Result<(), Self::Error>;
   }

Error Handling Contracts
-------------------------

Error Context Contract
~~~~~~~~~~~~~~~~~~~~~~

**Contract Definition** (``kiln-error/src/context.rs:78-134``):

.. code-block:: rust

   /// Error Context and Propagation Contract
   ///
   /// INVARIANTS:
   /// 1. Error context is preserved across propagation
   /// 2. Original error information is not lost
   /// 3. Context chains are maintained in chronological order
   /// 4. Error conversion is deterministic
   ///
   /// PRECONDITIONS:
   /// - Context strings must be valid UTF-8 (in no_std environments)
   /// - Error types must implement required traits
   ///
   /// POSTCONDITIONS:
   /// - Context information is attached to errors
   /// - Error chain is preserved during conversion
   /// - Display formatting includes all context
   pub trait ErrorContext {
       type Error;
       
       /// Add context to an error
       ///
       /// CONTRACT:
       /// - Context is prepended to error description
       /// - Original error is preserved in error chain
       /// - Context string is stored efficiently
       ///
       /// PRECONDITIONS:
       /// - Context function must return valid string
       /// - Context string must fit in bounded storage (no_alloc)
       ///
       /// POSTCONDITIONS:
       /// - Returned error includes both original and context
       /// - Error chain allows unwrapping to original error
       ///
       /// COMPLEXITY: O(context.len())
       /// ERRORS: None (context addition should not fail)
       fn with_context<F>(self, f: F) -> ContextError<Self::Error>
       where
           F: FnOnce() -> BoundedString;
   }

Performance Contracts
---------------------

Runtime Performance Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Performance Guarantees**:

.. list-table:: Runtime Performance Contracts
   :header-rows: 1
   :widths: 30 20 20 30

   * - Operation
     - Complexity
     - Environment Impact
     - Contract Guarantee
   * - Component instantiation
     - O(bytecode_size)
     - std: Dynamic allocation
       no_alloc: Fixed pools
     - Linear in bytecode size,
       bounded by available memory
   * - Function dispatch
     - O(1)
     - All environments
     - Constant time lookup
   * - Memory access (bounds-checked)
     - O(1)
     - All environments  
     - Constant time with bounds check
   * - Resource allocation
     - O(1) amortized
     - std: HashMap
       no_alloc: Array lookup
     - Bounded allocation time
   * - Error propagation
     - O(context_chain_length)
     - All environments
     - Linear in error context depth

Memory Usage Contracts
~~~~~~~~~~~~~~~~~~~~~~

**Memory Guarantees**:

.. list-table:: Memory Usage Contracts
   :header-rows: 1
   :widths: 25 25 25 25

   * - Component
     - std Memory Usage
     - no_std+alloc Memory Usage
     - no_std+no_alloc Memory Usage
   * - Runtime core
     - Unbounded (heap)
     - Unbounded (heap)
     - 64KB static allocation
   * - Component storage
     - Dynamic (HashMap)
     - Dynamic (BTreeMap)
     - 256 component slots
   * - Resource table
     - Dynamic growth
     - Dynamic growth
     - 1024 resource slots
   * - Memory buffers
     - Vec<u8> growth
     - Vec<u8> growth
     - Fixed 64KB pages

Contract Verification
---------------------

Automated Contract Testing
~~~~~~~~~~~~~~~~~~~~~~~~~~

Contract compliance is verified through automated testing:

.. code-block:: rust

   // Example from tests/contract_verification_test.rs
   #[test]
   fn verify_memory_provider_contracts() {
       fn test_memory_contract<M: MemoryProvider>(mut memory: M) {
           let size = memory.len();
           
           // Test invariant: is_empty() ≡ (len() == 0)
           assert_eq!(memory.is_empty(), size == 0);
           
           // Test bounds checking contract
           if size > 0 {
               // Valid access should succeed
               assert!(memory.read_bytes(0, 1).is_ok());
               
               // Out-of-bounds access should fail  
               assert!(memory.read_bytes(size, 1).is_err());
               assert!(memory.read_bytes(0, size + 1).is_err());
           }
           
           // Test overflow protection
           assert!(memory.read_bytes(usize::MAX, 1).is_err());
       }
       
       // Test all implementations
       test_memory_contract(StandardMemory::new(1024));
       test_memory_contract(BoundedMemory::new());
   }

Property-Based Testing
~~~~~~~~~~~~~~~~~~~~~

Complex contracts are verified using property-based testing:

.. code-block:: rust

   // Example property-based contract test
   #[quickcheck]
   fn resource_table_allocation_contract(resources: Vec<i32>) -> bool {
       let mut table = BoundedResourceTable::new();
       let mut allocated_ids = Vec::new();
       
       // Test allocation contract
       for resource in resources {
           if let Ok(id) = table.allocate(resource) {
               allocated_ids.push((id, resource));
           }
       }
       
       // Test retrieval contract
       for (id, expected_value) in allocated_ids {
           if let Ok(retrieved) = table.get::<i32>(id) {
               assert_eq!(*retrieved, expected_value);
           } else {
               return false;  // Contract violation
           }
       }
       
       true
   }

Contract Documentation Standards
-------------------------------

Documentation Requirements
~~~~~~~~~~~~~~~~~~~~~~~~~~

All API contracts must include:

1. **Invariants**: Properties that must always hold
2. **Preconditions**: Requirements that must be met before calling
3. **Postconditions**: Guarantees about state after completion
4. **Complexity**: Time and space complexity bounds
5. **Errors**: Exhaustive list of possible error conditions
6. **Environment Variations**: Behavior differences across environments

Contract Review Process
~~~~~~~~~~~~~~~~~~~~~~

1. **Design Review**: Contracts reviewed during API design
2. **Implementation Review**: Contract fulfillment verified in code review
3. **Test Review**: Contract tests verified for completeness
4. **Documentation Review**: Contract documentation accuracy verified

Cross-References
-----------------

.. seealso::

   * :doc:`external` for external API specifications
   * :doc:`internal` for internal interface definitions
   * :doc:`../01_architectural_design/patterns` for contract implementation patterns
   * :doc:`../05_resource_management/memory_budgets` for memory contract specifics