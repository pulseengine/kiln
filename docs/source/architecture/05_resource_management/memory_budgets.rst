==========================
Memory Budget Allocation
==========================

**Teaching Point**: In safety-critical systems, every byte must be accounted for. This document shows how memory is budgeted across different deployment scenarios.

Memory Architecture Overview
----------------------------

.. arch_constraint:: Memory Predictability
   :id: ARCH_CON_MEM_001
   :priority: critical
   
   All memory allocation must be predictable and bounded.

Total System Memory Budget
--------------------------

.. list-table:: Deployment Environment Memory Limits
   :header-rows: 1
   :widths: 25 20 20 20 15

   * - Environment
     - Total RAM
     - Kiln Allocation
     - User Modules
     - Safety Margin
   * - Cloud/Server
     - 8+ GB
     - 512 MB
     - 4 GB
     - Dynamic
   * - Embedded Linux
     - 512 MB
     - 64 MB
     - 128 MB
     - 20%
   * - RTOS (QNX)
     - 256 MB
     - 32 MB
     - 64 MB
     - 25%
   * - Bare-metal
     - 64 MB
     - 8 MB
     - 16 MB
     - 50%

Component Memory Budgets
------------------------

Runtime Core Memory
~~~~~~~~~~~~~~~~~~~

**Based on actual implementation constants**:

.. code-block:: rust

   // From kiln-foundation/src/bounded.rs
   pub const MAX_STACK_SIZE: usize = 1024;        // Stack frames
   pub const MAX_GLOBALS: usize = 1024;           // Global variables
   pub const MAX_FUNCTIONS: usize = 10000;        // Function definitions
   pub const MAX_TABLES: usize = 100;             // Table instances

.. list-table:: Runtime Core Memory Breakdown
   :header-rows: 1

   * - Component
     - std Size
     - no_std Size
     - no_alloc Size
   * - Execution Stack
     - Dynamic
     - 1 MB max
     - 64 KB fixed
   * - Global Storage
     - Dynamic
     - 256 KB max
     - 32 KB fixed
   * - Function Table
     - Dynamic
     - 1 MB max
     - 128 KB fixed
   * - Instance Pool
     - Dynamic
     - Dynamic
     - 10 instances fixed

WebAssembly Linear Memory
~~~~~~~~~~~~~~~~~~~~~~~~~

**Per-instance memory limits**:

.. code-block:: rust

   // From kiln-runtime/src/memory.rs
   pub const WASM_PAGE_SIZE: usize = 65536;      // 64 KB per page
   pub const MAX_PAGES_32: usize = 65536;        // 4 GB max (32-bit)
   
   // Environment-specific limits
   #[cfg(feature = "std")]
   pub const DEFAULT_MAX_PAGES: usize = 1024;    // 64 MB default
   
   #[cfg(not(feature = "std"))]
   pub const DEFAULT_MAX_PAGES: usize = 16;      // 1 MB for embedded

Bounded Collections Sizing
~~~~~~~~~~~~~~~~~~~~~~~~~~

**Teaching Point**: In no_alloc environments, all collections have fixed capacity.

.. code-block:: rust

   // Actual constants from kiln-foundation
   pub const MAX_WASM_NAME_LENGTH: usize = 255;
   pub const MAX_BUFFER_SIZE: usize = 65536;
   pub const MAX_EXPORTS: usize = 1000;
   pub const MAX_IMPORTS: usize = 1000;
   pub const MAX_TYPES: usize = 10000;

.. list-table:: Collection Memory Requirements
   :header-rows: 1

   * - Collection Type
     - Max Elements
     - Element Size
     - Total Memory
   * - BoundedVec<Value>
     - 1024
     - 16 bytes
     - 16 KB
   * - BoundedString
     - 255 chars
     - 1 byte
     - 255 bytes
   * - BoundedHashMap
     - 1024 entries
     - ~64 bytes
     - 64 KB

Platform-Specific Allocations
-----------------------------

Linux Memory Layout
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // From kiln-platform/src/linux_memory.rs
   pub struct LinuxAllocator {
       max_pages: usize,
       guard_pages: bool,
       mte_enabled: bool,
   }

- Base allocation: 1 MB minimum
- Guard pages: +8 KB per allocation
- MTE tags: +3.125% overhead

QNX Memory Partitioning
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // From kiln-platform/src/qnx_memory.rs  
   pub struct QnxPartition {
       name: &'static str,
       size: usize,
       flags: PartitionFlags,
   }

.. list-table:: QNX Partition Layout
   :header-rows: 1

   * - Partition
     - Size
     - Purpose
   * - kiln_runtime
     - 8 MB
     - Core runtime
   * - wasm_instances
     - 16 MB
     - Module instances
   * - wasm_memory
     - 32 MB
     - Linear memory
   * - emergency
     - 4 MB
     - Error recovery

Static Allocation Strategy
--------------------------

For no_std + no_alloc environments:

.. code-block:: rust

   // Memory layout for bare-metal
   #[link_section = ".kiln_runtime"]
   static RUNTIME_MEMORY: [u8; 1024 * 1024] = [0; 1024 * 1024];  // 1 MB
   
   #[link_section = ".kiln_instances"]  
   static INSTANCE_MEMORY: [u8; 4 * 1024 * 1024] = [0; 4 * 1024 * 1024];  // 4 MB
   
   #[link_section = ".kiln_scratch"]
   static SCRATCH_MEMORY: [u8; 512 * 1024] = [0; 512 * 1024];  // 512 KB

Memory Growth Policies
----------------------

.. list-table:: Growth Strategies by Environment
   :header-rows: 1

   * - Environment
     - Growth Strategy
     - Limit Check
     - Failure Mode
   * - std
     - Dynamic (OS)
     - Soft limit
     - OOM exception
   * - no_std + alloc
     - Realloc
     - Hard limit
     - Error return
   * - no_std + no_alloc
     - Pre-allocated
     - Compile-time
     - Panic/abort

Memory Safety Verification
--------------------------

**Teaching Point**: Every allocation is verified at multiple levels.

.. code-block:: rust

   // From kiln-foundation/src/verification.rs
   pub enum VerificationLevel {
       None,        // No checks (dangerous!)
       Minimal,     // Bounds only
       Standard,    // Bounds + alignment
       Full,        // Bounds + alignment + checksums
   }

Memory overhead by verification level:

- None: 0% overhead
- Minimal: ~1% overhead  
- Standard: ~5% overhead
- Full: ~10% overhead

Monitoring and Metrics
----------------------

Runtime memory tracking:

.. code-block:: rust

   // From kiln-runtime/src/memory.rs
   pub struct MemoryMetrics {
       peak_usage: AtomicUsize,
       current_usage: AtomicUsize,
       allocation_count: AtomicU64,
       failed_allocations: AtomicU64,
   }

Cross-References
----------------

- **CPU Budgets**: See :doc:`cpu_budgets`
- **Platform Details**: See :doc:`../platform_layer`
- **Implementation**: ``kiln-platform/src/memory.rs``