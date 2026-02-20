=============================
Static Memory Enforcement
=============================

.. image:: ../_static/icons/memory_management.svg
   :width: 64px
   :align: center
   :alt: Static Memory System Icon

The WebAssembly Runtime (Kiln) implements a comprehensive static memory enforcement system to ensure predictable memory usage, eliminate runtime allocation failures, and maintain safety-critical compliance.

.. contents:: On this page
   :local:
   :depth: 2

Core Principles
---------------

No Dynamic Allocation Post-Initialization
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- All memory is pre-allocated during system initialization
- Runtime allocations are strictly forbidden after initialization lock
- Memory budgets are enforced at compile-time where possible

Bounded Collections Everywhere
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- All collections have compile-time capacity limits
- ``BoundedVec<T, N, P>`` replaces ``Vec<T>``
- ``BoundedMap<K, V, N, P>`` replaces ``HashMap<K, V>``
- ``BoundedString<N, P>`` replaces ``String``

Provider-Based Memory Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- Memory providers abstract allocation strategies
- ``NoStdProvider<N>`` for no_std environments with fixed buffers
- ``ConfigurableProvider<N>`` for platform-specific requirements
- All providers implement the ``UnifiedMemoryProvider`` trait

Architecture
------------

Memory Provider Hierarchy
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: text

   UnifiedMemoryProvider (trait)
   ├── ConfigurableProvider<SIZE> (generic fixed-size)
   │   ├── SmallProvider (8KB)
   │   ├── MediumProvider (64KB)
   │   └── LargeProvider (1MB)
   ├── NoStdProvider<SIZE> (no_std compatible)
   └── UnifiedStdProvider (std feature only)

Bounded Collections
~~~~~~~~~~~~~~~~~~

BoundedVec Implementation
........................

.. code-block:: rust

   // Instead of:
   let mut vec = Vec::new();
   
   // Use:
   let mut vec: BoundedVec<T, 100, Provider> = BoundedVec::new(provider)?;

Key characteristics:

- **Compile-time capacity**: Maximum size known at compile time
- **Memory provider**: Configurable memory allocation strategy
- **Type safety**: Generic over element type, capacity, and provider
- **Error handling**: Graceful capacity exceeded errors

BoundedMap Implementation
........................

.. code-block:: rust

   // Instead of:
   let mut map = HashMap::new();
   
   // Use:
   let mut map: BoundedMap<K, V, 50, Provider> = BoundedMap::new(provider)?;

Features:

- **Hash-based lookup**: O(1) average case performance
- **Collision handling**: Open addressing with linear probing
- **Memory efficiency**: Fixed allocation, no dynamic growth
- **Iterator support**: Compatible with standard iteration patterns

BoundedString Implementation
...........................

.. code-block:: rust

   // Instead of:
   let mut string = String::new();
   
   // Use:
   let mut string: BoundedString<256, Provider> = BoundedString::new(provider);

Capabilities:

- **UTF-8 compliant**: Full Unicode support within bounds
- **String operations**: push, pop, truncate, clear
- **Format integration**: Compatible with write! and format! macros
- **Conversion methods**: From/to standard strings when std is available

Implementation Details
---------------------

Memory Providers
~~~~~~~~~~~~~~~

UnifiedMemoryProvider Trait
...........................

.. code-block:: rust

   pub trait UnifiedMemoryProvider: Clone + PartialEq + Eq {
       const SIZE: usize;
       
       fn allocate(&mut self, size: usize) -> Result<*mut u8, MemoryError>;
       fn deallocate(&mut self, ptr: *mut u8, size: usize);
       fn available(&self) -> usize;
       fn reset(&mut self);
   }

NoStdProvider Implementation
...........................

.. code-block:: rust

   pub struct NoStdProvider<const SIZE: usize> {
       buffer: [u8; SIZE],
       offset: usize,
   }
   
   impl<const SIZE: usize> NoStdProvider<SIZE> {
       pub const fn new() -> Self {
           Self {
               buffer: [0; SIZE],
               offset: 0,
           }
       }
   }

ConfigurableProvider Types
.........................

.. code-block:: rust

   // Pre-configured provider types
   pub type SmallProvider = ConfigurableProvider<8192>;     // 8KB
   pub type MediumProvider = ConfigurableProvider<65536>;   // 64KB  
   pub type LargeProvider = ConfigurableProvider<1048576>;  // 1MB
   
   // Usage
   let provider = MediumProvider::new();
   let vec = BoundedVec::<Item, 1000, MediumProvider>::new(provider)?;

Static Enforcement Mechanisms
----------------------------

Compile-Time Validation
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Compile-time size validation
   const _: () = {
       assert!(CAPACITY > 0, "Capacity must be positive");
       assert!(CAPACITY <= MAX_COLLECTION_SIZE, "Capacity too large");
       assert!(CAPACITY * core::mem::size_of::<T>() <= PROVIDER_SIZE, "Insufficient provider memory");
   };

Runtime Safety Checks
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   impl<T, const CAPACITY: usize, P: UnifiedMemoryProvider> BoundedVec<T, CAPACITY, P> {
       pub fn push(&mut self, value: T) -> Result<(), T> {
           if self.len >= CAPACITY {
               return Err(value);  // Return value instead of panicking
           }
           
           // Safe to insert - capacity checked
           unsafe {
               self.buffer.as_mut_ptr().add(self.len).write(value);
           }
           self.len += 1;
           Ok(())
       }
   }

Memory Budget Integration
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Budget-aware collection creation
   macro_rules! bounded_vec_with_budget {
       ($type:ty, $capacity:expr, $crate_id:expr) => {{
           // Validate against memory budget
           validate_allocation!($capacity * core::mem::size_of::<$type>(), $crate_id);
           
           // Create provider with budget
           let provider = safe_managed_alloc!($capacity * core::mem::size_of::<$type>(), $crate_id)?;
           
           BoundedVec::<$type, $capacity, _>::new(provider)
       }};
   }

Safety Features
--------------

No Use-After-Free
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Memory provider tied to collection lifetime
   impl<T, const CAPACITY: usize, P: UnifiedMemoryProvider> Drop for BoundedVec<T, CAPACITY, P> {
       fn drop(&mut self) {
           // Drop all elements
           for i in 0..self.len {
               unsafe {
                   self.buffer.as_mut_ptr().add(i).drop_in_place();
               }
           }
           // Provider automatically cleans up memory
       }
   }

No Buffer Overflows
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   impl<T, const CAPACITY: usize, P: UnifiedMemoryProvider> Index<usize> for BoundedVec<T, CAPACITY, P> {
       type Output = T;
       
       fn index(&self, index: usize) -> &Self::Output {
           if index >= self.len {
               // Safe panic - bounds violation detected
               panic!("Index {} out of bounds for length {}", index, self.len);
           }
           unsafe { &*self.buffer.as_ptr().add(index) }
       }
   }

No Memory Leaks
~~~~~~~~~~~~~~

.. code-block:: rust

   // Automatic cleanup through RAII
   {
       let provider = MediumProvider::new();
       let vec = BoundedVec::<Item, 1000, _>::new(provider)?;
       // ... use vec
   } // vec and provider automatically cleaned up

Usage Patterns
--------------

Basic Collection Usage
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_foundation::{BoundedVec, BoundedMap, BoundedString, MediumProvider};
   
   fn create_collections() -> Result<(), Error> {
       let provider = MediumProvider::new();
       
       // Create bounded vector
       let mut items = BoundedVec::<Item, 1000, _>::new(provider.clone())?;
       items.push(Item::new("example"))?;
       
       // Create bounded map  
       let mut lookup = BoundedMap::<String, Value, 100, _>::new(provider.clone())?;
       lookup.insert("key".to_string(), Value::default())?;
       
       // Create bounded string
       let mut text = BoundedString::<256, _>::new(provider);
       text.push_str("Hello, world!")?;
       
       Ok(())
   }

Advanced Provider Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Custom provider configuration
   struct CustomProvider<const SIZE: usize> {
       allocator: CustomAllocator,
       buffer: [u8; SIZE],
   }
   
   impl<const SIZE: usize> UnifiedMemoryProvider for CustomProvider<SIZE> {
       const SIZE: usize = SIZE;
       
       fn allocate(&mut self, size: usize) -> Result<*mut u8, MemoryError> {
           // Custom allocation logic
           self.allocator.allocate(size)
       }
       
       // ... other trait methods
   }

Integration with Safety Systems
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_foundation::{SafetyContext, SafetyLevel, safe_managed_alloc};
   
   fn create_safety_critical_collection() -> Result<SafeComponent, Error> {
       // Create safety context
       let safety_ctx = SafetyContext::new(SafetyLevel::ASIL_C)?;
       
       // Allocate with budget enforcement
       let memory_guard = safe_managed_alloc!(16384, CrateId::Component)?;
       let provider = NoStdProvider::<16384>::new(memory_guard);
       
       // Create bounded collection with safety context
       let data = BoundedVec::<SafeData, 256, _>::new_with_safety(provider, safety_ctx)?;
       
       Ok(SafeComponent::new(data))
   }

Performance Characteristics
--------------------------

Time Complexity
~~~~~~~~~~~~~~

.. list-table:: Operation Complexity
   :header-rows: 1
   :widths: 30 25 45

   * - Operation
     - Complexity
     - Notes
   * - BoundedVec::push
     - O(1)
     - Constant time, capacity checked
   * - BoundedVec::pop
     - O(1)
     - Constant time removal
   * - BoundedVec::get
     - O(1)
     - Direct array access
   * - BoundedMap::insert
     - O(1) average
     - Hash-based, O(n) worst case
   * - BoundedMap::get
     - O(1) average
     - Hash-based lookup
   * - BoundedString::push_str
     - O(n)
     - Length of added string

Memory Overhead
~~~~~~~~~~~~~~

.. list-table:: Memory Overhead Analysis
   :header-rows: 1
   :widths: 30 25 45

   * - Component
     - Overhead
     - Description
   * - BoundedVec
     - 8-16 bytes
     - Length + capacity fields
   * - BoundedMap
     - 16-32 bytes
     - Hash table metadata
   * - BoundedString
     - 8-16 bytes
     - Length + capacity fields
   * - Provider
     - 8-24 bytes
     - Provider state
   * - **Total**
     - **<64 bytes**
     - **Per collection**

Testing and Validation
----------------------

Unit Testing Patterns
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_bounded_vec_capacity() {
           let provider = SmallProvider::new();
           let mut vec = BoundedVec::<u32, 10, _>::new(provider).unwrap();
           
           // Test capacity enforcement
           for i in 0..10 {
               assert!(vec.push(i).is_ok());
           }
           
           // Should fail on capacity exceeded
           assert!(vec.push(10).is_err());
           assert_eq!(vec.len(), 10);
       }
       
       #[test]
       fn test_memory_cleanup() {
           let initial_memory = get_memory_usage();
           
           {
               let provider = MediumProvider::new();
               let _vec = BoundedVec::<LargeItem, 100, _>::new(provider).unwrap();
               // Memory should be allocated
           } // Memory should be cleaned up here
           
           let final_memory = get_memory_usage();
           assert_eq!(initial_memory, final_memory);
       }
   }

Property-Based Testing
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod property_tests {
       use proptest::prelude::*;
       use super::*;
       
       proptest! {
           #[test]
           fn test_bounded_vec_invariants(
               operations in prop::collection::vec(
                   prop::oneof![
                       (0..1000u32).prop_map(|x| Operation::Push(x)),
                       Just(Operation::Pop),
                   ],
                   0..200
               )
           ) {
               let provider = MediumProvider::new();
               let mut vec = BoundedVec::<u32, 100, _>::new(provider).unwrap();
               
               for op in operations {
                   match op {
                       Operation::Push(val) => {
                           let result = vec.push(val);
                           if vec.len() < 100 {
                               assert!(result.is_ok());
                           } else {
                               assert!(result.is_err());
                           }
                       }
                       Operation::Pop => {
                           vec.pop();
                       }
                   }
                   
                   // Invariants
                   assert!(vec.len() <= vec.capacity());
                   assert!(vec.capacity() == 100);
               }
           }
       }
   }

Formal Verification
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(kani)]
   mod formal_verification {
       use super::*;
       
       #[kani::proof]
       fn verify_bounded_vec_safety() {
           let provider = SmallProvider::new();
           let mut vec = BoundedVec::<u32, 10, _>::new(provider).unwrap();
           
           let value: u32 = kani::any();
           let result = vec.push(value);
           
           // Property: Push never causes buffer overflow
           if vec.len() < vec.capacity() {
               assert!(result.is_ok());
               assert!(vec.len() <= vec.capacity());
           } else {
               assert!(result.is_err());
           }
       }
       
       #[kani::proof]
       fn verify_memory_bounds() {
           let size: usize = kani::any();
           kani::assume(size <= 1024);
           
           let provider = SmallProvider::new();
           
           // Property: Allocation within provider bounds succeeds
           let result = provider.allocate(size);
           if size <= SmallProvider::SIZE {
               assert!(result.is_ok());
           }
       }
   }

Migration Guide
--------------

From Standard Collections
~~~~~~~~~~~~~~~~~~~~~~~~

Step-by-step migration from standard Rust collections:

1. **Replace Vec with BoundedVec**:

   .. code-block:: rust
   
      // Before
      let mut items = Vec::new();
      items.push(value);
      
      // After
      let provider = MediumProvider::new();
      let mut items = BoundedVec::<Item, 1000, _>::new(provider)?;
      items.push(value)?;

2. **Replace HashMap with BoundedMap**:

   .. code-block:: rust
   
      // Before
      let mut lookup = HashMap::new();
      lookup.insert(key, value);
      
      // After  
      let provider = MediumProvider::new();
      let mut lookup = BoundedMap::<Key, Value, 100, _>::new(provider)?;
      lookup.insert(key, value)?;

3. **Handle Capacity Errors**:

   .. code-block:: rust
   
      // Error handling for capacity limits
      match collection.push(value) {
           Ok(()) => { /* Success */ }
           Err(returned_value) => {
               // Handle capacity exceeded
               log::warn!("Collection capacity exceeded");
               // Implement overflow strategy
           }
       }

Best Practices
--------------

Capacity Planning
~~~~~~~~~~~~~~~~

1. **Profile First**: Measure actual usage before setting capacities
2. **Safety Margins**: Include 20-30% headroom for unexpected growth
3. **Component-Specific**: Different components may need different capacities
4. **Platform Considerations**: Adjust for target platform memory constraints

Provider Selection
~~~~~~~~~~~~~~~~~

1. **SmallProvider (8KB)**: For small collections, temporary data
2. **MediumProvider (64KB)**: For moderate collections, component state
3. **LargeProvider (1MB)**: For large collections, caching, buffers
4. **Custom Providers**: For specialized allocation strategies

Error Handling
~~~~~~~~~~~~~

1. **Graceful Degradation**: Handle capacity errors without panicking
2. **Overflow Strategies**: Implement strategies for when collections fill up
3. **Monitoring**: Track collection usage to optimize capacities
4. **Testing**: Test capacity limits and error conditions

See Also
--------

- :doc:`05_resource_management/memory_budgets` - Memory budget system
- :doc:`memory_safety_comparison` - Comparison with other approaches
- :doc:`../safety/formal_verification` - Formal verification of memory safety
- :doc:`../developer/testing/formal_verification_guide` - KANI verification guide