=====================================================
ADR-001: Memory Allocation Strategy
=====================================================

.. arch_decision:: Memory Allocation Strategy
   :id: ARCH_DEC_MEM_001
   :date: 2024-01-15
   :status: accepted
   :deciders: Architecture Team
   :tags: memory, safety, no_std

Context
-------

**Teaching Point**: Start by explaining the problem space and constraints.

Pulseengine (Kiln Edition) must support multiple deployment environments:

1. **Cloud/Server**: Abundant memory, standard OS services
2. **Embedded Linux**: Limited memory, but has OS allocator
3. **RTOS**: Very limited memory, custom allocators
4. **Bare-metal**: No heap, only static allocation

The WebAssembly specification requires:
- Linear memory that can grow
- Multiple memory instances per module
- Predictable allocation/deallocation

Safety requirements mandate:
- No memory fragmentation in safety-critical systems
- Bounded memory usage
- Deterministic allocation time

Decision
--------

Implement a **three-tier memory allocation strategy** based on available features:

1. **Tier 1: Standard Allocator** (when `std` feature is enabled)
   
   .. code-block:: rust
   
      // Actual implementation in kiln-foundation/src/safe_memory.rs
      pub struct StdProvider {
          data: Vec<u8>,
          access_tracker: MemoryAccessTracker,
      }

2. **Tier 2: Custom Allocator** (when `alloc` but not `std`)
   
   .. code-block:: rust
   
      // Uses global allocator with safety wrapper
      pub struct AllocProvider {
          data: Vec<u8>,
          max_size: usize,
      }

3. **Tier 3: Static Pools** (when neither `std` nor `alloc`)
   
   .. code-block:: rust
   
      // Actual implementation for no_alloc environments
      pub struct NoStdProvider<const N: usize> {
          data: [u8; N],
          used: usize,
          verification_level: VerificationLevel,
      }

Consequences
------------

**Positive:**

- ✅ **Flexibility**: Each environment gets optimal implementation
- ✅ **Safety**: Bounded memory in all configurations
- ✅ **Performance**: No abstraction overhead (zero-cost)
- ✅ **Verification**: Static allocation enables formal verification

**Negative:**

- ❌ **Complexity**: Three different implementations to maintain
- ❌ **Feature Gates**: Complex conditional compilation
- ❌ **Testing**: Need to test all three configurations

**Trade-offs:**

.. list-table:: Memory Strategy Trade-offs
   :header-rows: 1

   * - Strategy
     - Memory Efficiency
     - Predictability
     - Safety
   * - Standard (Vec)
     - High (dynamic)
     - Low
     - Medium
   * - Custom Alloc
     - Medium
     - Medium
     - High
   * - Static Pools
     - Low (fixed)
     - High
     - Very High

Implementation Details
----------------------

**Teaching Point**: Show the actual code patterns used.

The `MemoryProvider` trait unifies all strategies:

.. code-block:: rust

   // From kiln-foundation/src/traits.rs
   pub trait MemoryProvider: Clone + PartialEq + Eq {
       type Allocator: Allocator;
       
       fn len(&self) -> usize;
       fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8]>;
       fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<()>;
       fn resize(&mut self, new_len: usize) -> Result<()>;
   }

Usage example across environments:

.. code-block:: rust

   // Automatically selects the right provider
   #[cfg(feature = "std")]
   type DefaultProvider = StdProvider;
   
   #[cfg(all(not(feature = "std"), feature = "alloc"))]
   type DefaultProvider = AllocProvider;
   
   #[cfg(not(feature = "alloc"))]
   type DefaultProvider = NoStdProvider<65536>; // 64KB static

Alternatives Considered
-----------------------

1. **Single Static Allocator**
   
   - ✅ Simplest implementation
   - ❌ Wastes memory in dynamic environments
   - ❌ Too restrictive for cloud deployments

2. **Dynamic Only (require alloc)**
   
   - ✅ Simpler codebase
   - ❌ Excludes bare-metal targets
   - ❌ Not suitable for safety-critical systems

3. **External Allocator Trait**
   
   - ✅ Maximum flexibility
   - ❌ Adds complexity for users
   - ❌ Harder to verify safety properties

Validation
----------

This decision is validated by:

1. **Test Coverage**: All three tiers tested in CI
2. **Benchmarks**: Performance meets requirements
3. **Static Analysis**: Formally verified for no_alloc case
4. **Real Deployment**: Successfully used in embedded projects

References
----------

- **Implementation**: ``kiln-foundation/src/safe_memory.rs``
- **Bounded Types**: ``kiln-foundation/src/bounded.rs``
- **Examples**: :doc:`/examples/foundation/safe_memory`
- **Related ADRs**: :doc:`adr-002-bounded-collections`