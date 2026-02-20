==========================
Requirements Allocation Matrix
==========================

**Teaching Point**: Every requirement must be allocated to at least one architectural component. This ensures complete coverage and traceability.

.. warning::
   **Allocation vs Implementation**: This matrix shows requirement allocation to components.
   Allocation does not mean implementation is complete. See :doc:`../test_coverage` for 
   actual implementation status (currently <5% coverage).

Allocation Overview
-------------------

This matrix shows how requirements are distributed across components:

.. needtable::
   :filter: type == "req" and ("allocated_to" in links or "implements" in links_back)
   :columns: id, title, status, links_back
   :style: table

Component Allocation Summary
----------------------------

.. list-table:: Requirements per Component
   :header-rows: 1
   :widths: 30 40 15 15

   * - Component
     - Allocated Requirements
     - Count
     - Coverage
   * - Runtime Core (ARCH_COMP_001)
     - REQ_001, REQ_002, REQ_CORE_001, REQ_EXEC_001
     - 4
     - Allocated (not implemented)
   * - Memory Manager (ARCH_COMP_002)
     - REQ_MEM_001, REQ_MEM_SAFETY_001, REQ_BOUNDED_001
     - 3
     - Mostly implemented
   * - Component Runtime (ARCH_COMP_003)
     - REQ_COMP_001, REQ_COMP_002, REQ_COMP_IF_001
     - 3
     - Allocated (partial impl)
   * - Binary Decoder (ARCH_COMP_004)
     - REQ_DECODE_001, REQ_VALIDATE_001
     - 2
     - Partially implemented
   * - Platform Layer (ARCH_COMP_005)
     - REQ_PLATFORM_001, REQ_PLATFORM_002, REQ_OS_001
     - 3
     - Mostly implemented

Environment-Specific Allocations
--------------------------------

**Teaching Point**: Some requirements have different implementations based on the environment configuration.

std Environment
~~~~~~~~~~~~~~~

.. arch_component:: std Configuration
   :id: ARCH_COMP_STD
   :variant_of: ARCH_COMP_001
   :environment: std
   :implements: REQ_STD_001, REQ_THREAD_001

- Full standard library support
- Native threading with ``std::sync``
- Dynamic memory with ``std::vec::Vec``
- File I/O and networking available

no_std + alloc Environment
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_component:: no_std+alloc Configuration
   :id: ARCH_COMP_NOSTD_ALLOC
   :variant_of: ARCH_COMP_001
   :environment: no_std+alloc
   :implements: REQ_NOSTD_001, REQ_ALLOC_001

- No standard library but has allocator
- Custom synchronization via ``kiln-sync``
- Dynamic memory with ``alloc::vec::Vec``
- No file I/O or OS services

no_std + no_alloc Environment
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_component:: no_std+no_alloc Configuration
   :id: ARCH_COMP_NOSTD_NOALLOC
   :variant_of: ARCH_COMP_001
   :environment: no_std+no_alloc
   :implements: REQ_NOSTD_001, REQ_STATIC_001, REQ_BOUNDED_001

**Actual Implementation Example**:

.. code-block:: rust

   // From kiln-foundation/src/bounded.rs
   pub struct BoundedVec<T, const N_ELEMENTS: usize, P: MemoryProvider> {
       provider: P,
       len: usize,
       _phantom: PhantomData<T>,
   }
   
   // Usage in no_alloc environment
   type FunctionArgs = BoundedVec<Value, MAX_FUNCTION_ARGS, NoStdProvider<1024>>;

Critical Requirements Mapping
-----------------------------

Safety Requirements
~~~~~~~~~~~~~~~~~~~

.. list-table:: Safety Requirements Allocation
   :header-rows: 1

   * - Requirement
     - Primary Component
     - Implementation
   * - REQ_MEM_SAFETY_001
     - Memory Manager
     - ``SafeSlice``, bounds checking
   * - REQ_NO_UNSAFE_001
     - All Components
     - ``#![forbid(unsafe_code)]``
   * - REQ_DETERMINISTIC_001
     - Runtime Core
     - Fuel-based execution limits

Performance Requirements
~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table:: Performance Requirements Allocation
   :header-rows: 1

   * - Requirement
     - Component
     - Constraint
     - Implementation
   * - REQ_PERF_001
     - Runtime Core
     - < 10ms per instruction
     - Stackless execution
   * - REQ_MEM_LIMIT_001
     - Memory Manager
     - Max 64MB per instance
     - Bounded allocators
   * - REQ_STARTUP_001
     - Binary Decoder
     - < 100ms module load
     - Streaming decoder

Functional Requirements
~~~~~~~~~~~~~~~~~~~~~~~

.. list-table:: Core Functional Requirements
   :header-rows: 1

   * - Requirement
     - Components
     - Verification Method
   * - REQ_WASM_CORE_001
     - Runtime Core, Instructions
     - WebAssembly test suite
   * - REQ_COMP_MODEL_001
     - Component Runtime
     - Component Model tests
   * - REQ_MULTI_PLATFORM_001
     - Platform Layer
     - CI on Linux/macOS/QNX

Traceability Verification
-------------------------

**Teaching Point**: Use these queries to verify complete allocation:

Unallocated Requirements
~~~~~~~~~~~~~~~~~~~~~~~~

.. needtable::
   :filter: type == "req" and not ("allocated_to" in links or "implements" in links_back)
   :columns: id, title, status
   :style: table

Over-allocated Components
~~~~~~~~~~~~~~~~~~~~~~~~~

Components implementing conflicting requirements:

.. code-block:: python

   # Verification query (conceptual)
   for component in components:
       reqs = component.implements
       if has_conflicts(reqs):
           report_conflict(component, reqs)

Cross-References
----------------

- **Traceability Details**: See :doc:`traceability`
- **Coverage Analysis**: See :doc:`coverage_analysis`
- **Component Details**: See :doc:`../01_architectural_design/components`