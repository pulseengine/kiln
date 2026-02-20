========================================
ASIL-A Static Memory Design
========================================

This document describes the static memory design implemented for ASIL-A compliance, specifically the elimination of all dynamic memory allocation from the capability context system.

.. contents:: On this page
   :local:
   :depth: 2

Overview
--------

For ASIL-A compliance, the Kiln project has eliminated all dynamic memory allocation from safety-critical components. The most significant change is replacing the HashMap-based capability storage with a static array implementation that works identically in both std and no_std environments.

Design Rationale
----------------

**ISO 26262 Requirement:** ASIL-A systems should avoid dynamic memory allocation during runtime to ensure deterministic behavior and prevent memory-related failures.

**Previous Implementation:**
- Used ``std::collections::HashMap`` when std feature was enabled
- Switched to static arrays only in no_std mode
- This dual implementation created potential for non-deterministic behavior

**New Implementation:**
- Always uses static arrays regardless of std availability
- Guarantees no dynamic allocation in capability context
- Provides identical behavior across all configurations

Implementation Details
----------------------

MemoryCapabilityContext Structure
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct MemoryCapabilityContext {
       /// Static array of capabilities to ensure no dynamic allocation
       /// Each slot contains an optional (CrateId, Capability) pair
       capabilities: [(Option<CrateId>, Option<Box<dyn AnyMemoryCapability>>); MAX_CAPABILITIES],
       
       /// Default verification level for new capabilities
       default_verification_level: VerificationLevel,
       
       /// Whether runtime verification is enabled
       runtime_verification: bool,
   }

Key Changes
~~~~~~~~~~~

1. **Unified Storage Model**
   
   - Removed conditional compilation for HashMap vs array
   - Single implementation using static arrays
   - ``MAX_CAPABILITIES = 32`` provides sufficient slots

2. **Deterministic Operations**
   
   - All operations have bounded execution time
   - No heap allocations during capability operations
   - Predictable memory usage

3. **API Compatibility**
   
   - External API remains unchanged
   - Internal implementation uses array scanning
   - Performance characteristics are deterministic

Memory Layout
-------------

The capability context uses a fixed memory layout:

.. code-block:: text

   MemoryCapabilityContext:
   +-----------------------------------+
   | capabilities[0]:                  |
   |   Option<CrateId>: 2 bytes       |
   |   Option<Box<Capability>>: 8 bytes|
   +-----------------------------------+
   | capabilities[1]:                  |
   |   ...                            |
   +-----------------------------------+
   | ... (30 more slots) ...          |
   +-----------------------------------+
   | capabilities[31]:                 |
   |   Option<CrateId>: 2 bytes       |
   |   Option<Box<Capability>>: 8 bytes|
   +-----------------------------------+
   | default_verification_level: 1 byte|
   | runtime_verification: 1 byte      |
   +-----------------------------------+
   
   Total Size: ~322 bytes (fixed)

Operation Complexity
--------------------

All operations now have deterministic complexity:

================  ==================  =================
Operation         Time Complexity     Space Complexity
================  ==================  =================
register          O(n) worst case     O(1)
get_capability    O(n) worst case     O(1)
remove            O(n) worst case     O(1)
has_capability    O(n) worst case     O(1)
capability_count  O(n)                O(1)
================  ==================  =================

Where n = MAX_CAPABILITIES (32)

Safety Analysis
---------------

Memory Safety
~~~~~~~~~~~~~

- **No Heap Fragmentation:** Static allocation prevents fragmentation
- **Bounded Memory Usage:** Maximum 32 capabilities enforced
- **No Out-of-Memory:** Cannot fail due to heap exhaustion
- **Predictable Layout:** Memory layout is compile-time known

Concurrency Safety
~~~~~~~~~~~~~~~~~~

- **No Dynamic Allocation:** Eliminates allocation race conditions
- **Simple Synchronization:** Array access easier to protect
- **Deterministic Access:** Predictable memory access patterns

ASIL-A Compliance
-----------------

This design satisfies ASIL-A requirements:

1. **REQ_ASIL_A_MEM_003: No Dynamic Memory Allocation**
   
   - ✅ HashMap eliminated from all configurations
   - ✅ Static array used exclusively
   - ✅ Memory layout fixed at compile time

2. **Deterministic Execution**
   
   - ✅ All operations have bounded execution time
   - ✅ No allocation failures possible
   - ✅ Memory usage is predictable

3. **Verification**
   
   - ✅ Formal verification possible on static arrays
   - ✅ KANI can verify all array bounds
   - ✅ No dynamic behavior to verify

Migration Guide
---------------

For code using MemoryCapabilityContext:

**No API Changes Required**

The public API remains identical. All changes are internal implementation details.

**Performance Considerations**

- HashMap O(1) average → Array O(n) worst case
- For n=32, performance impact is negligible
- Deterministic performance more important than speed for ASIL-A

**Configuration Changes**

Remove any conditional compilation based on std feature for capability storage.

Testing and Validation
----------------------

Verification Approach
~~~~~~~~~~~~~~~~~~~~~

1. **Unit Tests:** Verify all operations with static arrays
2. **Integration Tests:** Ensure compatibility maintained
3. **KANI Proofs:** Formally verify array bounds and operations
4. **Performance Tests:** Confirm acceptable performance

Test Coverage
~~~~~~~~~~~~~

- Registration of all 32 capabilities
- Overflow handling when full
- Removal and re-registration
- Concurrent access patterns

Limitations and Constraints
---------------------------

Design Limitations
~~~~~~~~~~~~~~~~~~

1. **Maximum 32 Capabilities**
   
   - Sufficient for current use cases
   - Can be increased if needed (recompilation required)

2. **Linear Search Performance**
   
   - O(n) operations vs O(1) HashMap
   - Acceptable for small n (32)

3. **Fixed Memory Overhead**
   
   - ~322 bytes always allocated
   - Vs dynamic allocation in HashMap

Future Considerations
~~~~~~~~~~~~~~~~~~~~~

If more than 32 capabilities needed:

1. Increase ``MAX_CAPABILITIES`` constant
2. Consider hierarchical capability management
3. Implement capability pooling strategies

Conclusion
----------

The static memory design for MemoryCapabilityContext eliminates all dynamic allocation while maintaining API compatibility. This ensures ASIL-A compliance through:

- Deterministic memory usage
- Bounded execution times  
- Predictable behavior
- Formal verification compatibility

The trade-off of slightly increased search time (O(n) vs O(1)) is acceptable for the safety guarantees provided and the small number of capabilities (n=32).

References
----------

- ISO 26262-6:2018 Section 7.4.3 - Dynamic memory management
- MISRA C:2012 Rule 21.3 - Dynamic memory allocation
- :doc:`/safety/asil_a_safety_case` - Overall ASIL-A safety case
- :doc:`/requirements/asil_a_requirements` - ASIL-A requirements