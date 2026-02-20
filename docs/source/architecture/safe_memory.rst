========================
Safe Memory Architecture
========================

.. image:: ../../_static/icons/safe_memory.svg
   :width: 48px
   :align: left
   :alt: Safe Memory Icon

The safe memory architecture provides memory safety abstractions designed for functional safety, implementing verification mechanisms to detect memory corruption and ensure safe memory access patterns.

.. spec:: Safe Memory Architecture
   :id: SPEC_012
   :links: REQ_MEM_SAFETY_001, REQ_MEM_SAFETY_002, REQ_MEM_SAFETY_003
   
   .. uml:: ../../_static/safe_memory_system.puml
      :alt: Safe Memory System Architecture
      :width: 100%
   
   The safe memory architecture consists of:
   
   1. Safe slice abstractions (Slice/SliceMut) with integrity verification
   2. Provider trait for different memory allocation strategies
   3. Data integrity verification with checksums
   4. Configurable verification levels for performance/safety balance
   5. Memory statistics tracking for analysis
   6. Thread-safe operations with atomic counters

.. impl:: Safe Memory Implementation
   :id: IMPL_SAFE_MEMORY_001
   :status: implemented
   :links: SPEC_012, REQ_MEM_SAFETY_001, REQ_MEM_SAFETY_002, REQ_MEM_SAFETY_003
   
   The safe memory system is implemented through:
   
   1. The ``Slice`` and ``SliceMut`` types providing memory-safe views with integrity checks
   2. The ``Provider`` trait for different memory backends
   3. The ``SafeMemoryHandler`` wrapper for safe memory operations
   4. Memory providers for different environments:
      - ``StdProvider`` for standard environments with Vec backing
      - ``NoStdProvider<N>`` for no_std environments with fixed-size arrays
   
   Key features include:
   - Checksums for data integrity verification
   - Configurable verification levels (Off, Minimal, Standard, Full, Critical)
   - Memory access logging and statistics via ``Stats`` struct
   - Thread-safe operations with atomic counters
   - Access verification for bounds checking
   - Support for sub-slicing with safety guarantees

Core Types
==========

Slice and SliceMut
------------------

The ``Slice`` and ``SliceMut`` types provide safe views into memory with integrated integrity checking:

.. code-block:: rust

   pub struct Slice<'a> {
       data: &'a [u8],
       checksum: Checksum,
       length: usize,
       verification_level: VerificationLevel,
   }
   
   pub struct SliceMut<'a> {
       data: &'a mut [u8],
       checksum: Checksum,
       length: usize,
       verification_level: VerificationLevel,
   }

Key methods:

- ``new(data)`` - Create with default verification level
- ``with_verification_level(data, level)`` - Create with specific verification
- ``data()`` / ``data_mut()`` - Access underlying data (with integrity check)
- ``verify_integrity()`` - Explicitly verify data integrity
- ``slice(start, len)`` - Create a sub-slice with safety checks
- ``update_checksum()`` - Update checksum after modifications (SliceMut only)

Provider Trait
--------------

The ``Provider`` trait abstracts over different memory allocation strategies:

.. code-block:: rust

   pub trait Provider: Send + Sync + fmt::Debug {
       type Allocator: Allocator + Clone + Send + Sync + 'static;
       
       fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>>;
       fn write_data(&mut self, offset: usize, data: &[u8]) -> Result<()>;
       fn verify_access(&self, offset: usize, len: usize) -> Result<()>;
       fn size(&self) -> usize;
       fn capacity(&self) -> usize;
       fn verify_integrity(&self) -> Result<()>;
       fn set_verification_level(&mut self, level: VerificationLevel);
       fn memory_stats(&self) -> Stats;
       // ... additional methods
   }

Memory Providers
================

StdProvider
-----------

For standard environments with heap allocation:

.. code-block:: rust

   pub struct StdProvider {
       data: Vec<u8>,
       access_log: Mutex<Vec<(usize, usize)>>,
       access_count: AtomicUsize,
       verification_level: VerificationLevel,
       // ... statistics fields
   }

Features:
- Dynamic resizing via ``Vec``
- Access logging for debugging
- Thread-safe statistics tracking
- Methods: ``with_capacity()``, ``add_data()``, ``resize()``, ``clear()``

NoStdProvider
-------------

For no_std environments with fixed-size memory:

.. code-block:: rust

   pub struct NoStdProvider<const N: usize> {
       data: [u8; N],
       used: usize,
       access_count: AtomicUsize,
       verification_level: VerificationLevel,
       // ... tracking fields
   }

Features:
- Fixed-size array backing (compile-time size)
- No heap allocations
- Lightweight tracking
- Methods: ``new()``, ``set_data()``, ``resize()``, ``clear()``

Default type alias for convenience:

.. code-block:: rust

   pub type DefaultNoStdProvider = NoStdProvider<4096>;

Verification Levels
===================

The ``VerificationLevel`` enum controls the trade-off between safety and performance:

- ``Off`` - No verification (maximum performance)
- ``Minimal`` - Basic bounds checking only
- ``Standard`` - Regular integrity checks
- ``Full`` - Comprehensive verification
- ``Critical`` - Maximum safety with redundant checks

Each level affects:
- Whether checksums are computed and verified
- Frequency of integrity checks
- Level of redundant validation
- Performance overhead

Memory Statistics
=================

The ``Stats`` struct tracks memory usage patterns:

.. code-block:: rust

   pub struct Stats {
       pub total_size: usize,      // Total memory capacity
       pub access_count: usize,    // Number of accesses
       pub unique_regions: usize,  // Unique memory regions accessed
       pub max_access_size: usize, // Largest single access
   }

Usage Example
=============

Basic usage with safe memory:

.. code-block:: rust

   use kiln_foundation::safe_memory::{StdProvider, Provider, Slice};
   use kiln_foundation::verification::VerificationLevel;
   
   // Create a provider
   let mut provider = StdProvider::with_capacity(1024);
   provider.set_verification_level(VerificationLevel::Standard);
   
   // Write data
   let data = b"Hello, safe memory!";
   provider.write_data(0, data)?;
   
   // Read with integrity verification
   let slice = provider.borrow_slice(0, data.len())?;
   let content = slice.data()?; // Performs integrity check
   
   // Get statistics
   let stats = provider.memory_stats();
   println!("Access count: {}", stats.access_count);

No-std usage:

.. code-block:: rust

   use kiln_foundation::safe_memory::{NoStdProvider, Provider};
   
   // Create fixed-size provider
   let mut provider = NoStdProvider::<512>::new();
   
   // Set data
   provider.set_data(b"Embedded safe memory")?;
   
   // Access with safety checks
   let slice = provider.borrow_slice(0, 20)?;

Thread Safety
=============

The safe memory system ensures thread safety through:

1. Atomic counters for statistics (lock-free)
2. Mutex protection for access logs (StdProvider only)
3. Send + Sync bounds on Provider trait
4. Immutable borrows for read operations

Integration with Collections
============================

The safe memory system integrates with bounded collections:

- ``BoundedVec`` uses ``SafeMemoryHandler<P>`` for storage
- ``BoundedStack`` (aliased as ``SafeStack``) provides safe stack operations
- Collections leverage provider verification for all operations

Error Handling
==============

Memory operations return ``Result<T>`` with specific error types:

- Memory bounds violations
- Integrity check failures (checksum mismatches)
- Capacity exceeded errors
- Allocation failures (std environments)

Errors include detailed context for debugging while maintaining safety.

Performance Considerations
==========================

1. **Verification Overhead**: Higher verification levels increase safety but reduce performance
2. **Checksum Computation**: Can be disabled with ``VerificationLevel::Off``
3. **Access Tracking**: Statistics collection can be minimized in critical paths
4. **Feature Flags**: Use ``optimize`` feature to disable some checks in release builds

Safety Guarantees
=================

The safe memory architecture provides:

1. **Bounds Safety**: All accesses are bounds-checked
2. **Integrity Verification**: Optional checksums detect corruption
3. **Lifetime Safety**: Rust's borrow checker ensures memory validity
4. **No Undefined Behavior**: Safe abstractions prevent UB
5. **Thread Safety**: Concurrent access is properly synchronized

This architecture enables building safety-critical systems with confidence in memory correctness.