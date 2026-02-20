===========================
Memory Subsystem Architecture
===========================

.. note::
   This section describes the *original* memory subsystem based on ``Vec<u8>``. It will be updated to reflect the new platform-abstracted ``LinearMemory<P: PageAllocator>`` design detailed in the memory rework plan.

.. image:: ../../_static/icons/memory_management.svg
   :width: 48px
   :align: right
   :alt: Memory Management Icon

The memory subsystem provides a consolidated implementation across the Kiln ecosystem with enhanced safety features.

.. spec:: Memory Subsystem Architecture (Original)
   :id: SPEC_007
   :links: REQ_018, REQ_023, REQ_024, REQ_MEM_SAFETY_001, REQ_MEM_SAFETY_002, REQ_MEM_SAFETY_003
   
   .. code-block:: text
      
      Memory Subsystem
      ├── Memory (Vec<u8> based)
      ├── Memory Type
      ├── Memory Metrics
      ├── Memory Operations
      ├── Bounds Checking
      └── Thread Safety
   
   The original memory subsystem architecture consisted of:
   
   1. Centralized memory implementation in ``kiln-runtime::Memory`` (using ``Vec<u8>``)
   2. Memory operations in ``kiln-instructions::memory_ops``
   3. Consistent memory access across core and component models
   4. Thread-safe memory metrics for profiling and optimization
   5. Comprehensive bounds checking for safety
   6. Performance tracking with access counts and peak usage monitoring
   7. Support for both standard and no-std environments
   8. Memory hooks for custom memory management integration

.. impl:: Memory Implementation (Original)
   :id: IMPL_003
   :status: implemented
   :links: SPEC_002, SPEC_007, REQ_018, REQ_023, REQ_024, REQ_MEM_SAFETY_001, IMPL_BOUNDS_001
   
   The original ``Memory`` struct in ``kiln-runtime`` provided a consolidated implementation that:
   
   1. Handled memory allocations and resizing (via ``Vec<u8>``)
   2. Enforced memory access boundaries
   3. Provided safe read/write operations
   4. Tracked memory access metrics
   5. Monitored peak memory usage
   6. Supported thread-safe operations
   7. Provided debug name capabilities
   8. Supported pre and post grow hooks
   9. Performed memory integrity verification
   10. Implemented thread-safe operations with environment-specific synchronization
   
   Key methods include:
   - ``grow(pages)`` - Grows memory by the specified number of pages
   - ``size()`` - Returns the current memory size in pages
   - ``read/write(addr, data)`` - Safely reads/writes memory with bounds checking
   - ``peak_memory()`` - Returns the peak memory usage during execution
   - ``access_count()`` - Returns the number of memory accesses for profiling
   - ``get_safe_slice()`` - Provides a memory-safe view of a memory region
   - ``verify_integrity()`` - Verifies memory integrity
   - ``with_pre_grow_hook/with_post_grow_hook`` - Registers hooks for memory growth events

.. impl:: Memory Operations (Original)
   :id: IMPL_011
   :status: implemented
   :links: SPEC_007, REQ_018, REQ_023, REQ_MEM_SAFETY_001, REQ_MEM_SAFETY_003, IMPL_WASM_MEM_001
   
   The ``memory_ops`` module in ``kiln-instructions`` provided:
   
   1. Standardized memory access operations
   2. Implementation of WebAssembly memory instructions
   3. Bounds and alignment checking
   4. Efficient memory load/store operations
   5. Memory fill, copy, and initialization operations
   
   Key operations include:
   - ``MemoryLoad`` - Loads values from memory with proper type conversion
   - ``MemoryStore`` - Stores values to memory with proper type conversion
   - ``MemorySize`` - Returns the current memory size
   - ``MemoryGrow`` - Expands the memory by a specified number of pages
   - ``MemoryFill`` - Fills a memory region with a specified value
   - ``MemoryCopy`` - Copies data between memory regions
   - ``MemoryInit`` - Initializes memory from data segments


New Memory Model (Overview)
---------------------------

The memory rework replaces the ``Vec<u8>`` backend with a ``LinearMemory<P: PageAllocator>`` structure.

*   ``LinearMemory``: Manages the WebAssembly linear memory abstraction.
*   ``PageAllocator``: A trait provided by the :doc:`platform_layer` responsible for:
    *   Allocating page-aligned memory (typically 64KiB Wasm pages).
    *   Handling memory growth requests.
    *   Applying memory protection (read/write/execute).
    *   Optionally mapping memory with MTE tags (if requested via :doc:`hardening` features and supported by the platform).

This allows the core runtime memory logic to be independent of the underlying OS memory management details.

(Further details and diagrams to be added here.) 