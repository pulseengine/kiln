==================
Safety Mechanisms
==================

.. image:: ../_static/icons/safety_features.svg
   :width: 64px
   :align: right
   :alt: Safety Mechanisms Icon

This document consolidates all safety mechanisms implemented in PulseEngine (Kiln Edition) to satisfy safety requirements and protect against systematic and random faults.

.. warning::
   **Implementation Note**: Safety mechanisms must be configured according to the 
   target ASIL level. See :doc:`configuration` for ASIL-specific configuration guidance.

.. contents:: On this page
   :local:
   :depth: 2

Overview
========

Mechanism Categories
--------------------

Safety mechanisms are organized by the fault types they address:

1. **Memory Safety Mechanisms** - Prevent memory corruption and access violations
2. **Control Flow Integrity** - Protect execution flow from hijacking
3. **Resource Protection** - Prevent resource exhaustion and starvation  
4. **Temporal Safety** - Ensure timing constraints and prevent lockups
5. **Data Integrity** - Protect data from corruption and tampering
6. **Fault Detection** - Identify and report fault conditions
7. **Error Recovery** - Restore safe operation after faults

Mechanism Attributes
--------------------

Each mechanism includes:

- **ID**: Unique identifier (SAFETY_CATEGORY_NNN)
- **Detection**: Fault detection capability and coverage
- **Reaction**: Response to detected faults
- **ASIL**: Required for ASIL levels (A, B, C, D)
- **Diagnostic Coverage**: Percentage of faults detected

Memory Safety Mechanisms
========================

.. safety:: Memory Bounds Checking
   :id: SAFETY_MEM_001
   :detection: 100% of out-of-bounds accesses
   :reaction: Reject access, return error
   :asil: A, B, C, D
   :diagnostic_coverage: 99%
   
   **Description**: The runtime validates all memory accesses against allocation boundaries before 
   execution. The system checks both base address and offset+length.
   
   **Implementation**:
   - SafeSlice wrapper enforces bounds on all slice operations
   - Integer overflow detection in offset calculations  
   - Dual-check strategy: pre-access and in-access validation
   - Zero-cost abstraction in release builds with bounds elision
   
   **Configuration**:
   - Always enabled in safety-critical mode
   - Performance mode allows selective bounds elision
   - Configurable trap vs. error return behavior

.. safety:: Memory Isolation Barriers  
   :id: SAFETY_MEM_002
   :detection: 100% of cross-module access attempts
   :reaction: Block access, signal violation
   :asil: C, D
   :diagnostic_coverage: 99%
   
   **Description**: The runtime isolates WebAssembly modules in separate memory spaces with no 
   shared mutable state except through validated interfaces.
   
   **Implementation**:
   - Separate linear memory per module instance
   - No direct memory sharing between modules
   - Explicit import/export validation
   - Hardware memory protection when available (Memory Protection Unit/Memory Management Unit)
   
   **Integration Requirements**:
   - Platform must provide memory protection unit
   - Minimum 4KB page granularity required

.. safety:: Stack Overflow Protection
   :id: SAFETY_MEM_003  
   :detection: 95% of stack overflows before corruption
   :reaction: Terminate execution, report error
   :asil: B, C, D
   :diagnostic_coverage: 90%
   
   **Description**: The runtime monitors and limits stack usage to prevent overflow into 
   adjacent memory regions.
   
   **Implementation**:
   - Stack depth counter with configurable limit
   - Guard pages when platform supports
   - Stack canary values for corruption detection
   - Pre-allocation of maximum stack space
   
   **Limitations**:
   - Cannot detect all forms of stack corruption
   - Guard pages require platform support

.. safety:: Memory Initialization Enforcement
   :id: SAFETY_MEM_004
   :detection: 100% of uninitialized access attempts  
   :reaction: Initialize to safe default or trap
   :asil: A, B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: All memory is initialized before use to prevent information leakage 
   and undefined behavior.
   
   **Implementation**:
   - Zero-initialization of linear memory on allocation
   - Explicit initialization tracking for tables
   - Trap on access to uninitialized table elements
   - Safe default values for all types

Control Flow Integrity Mechanisms
=================================

.. safety:: Indirect Call Validation
   :id: SAFETY_CFI_001
   :detection: 100% of invalid indirect calls
   :reaction: Trap execution, report violation
   :asil: C, D  
   :diagnostic_coverage: 100%
   
   **Description**: All indirect calls are validated against the function table before 
   execution to prevent control flow hijacking.
   
   **Implementation**:
   - Type signature validation on every indirect call
   - Function index bounds checking
   - Table element initialization tracking
   - No function pointer arithmetic allowed
   
   **Performance Impact**:
   - ~5-10% overhead on indirect call heavy workloads
   - Can be optimized with caching in performance mode

.. safety:: Control Stack Integrity
   :id: SAFETY_CFI_002
   :detection: >90% of control stack corruptions
   :reaction: Terminate execution, safe state transition
   :asil: D
   :diagnostic_coverage: >85%
   
   **Description**: The control stack is protected against corruption through redundancy 
   and validation checks.
   
   **Implementation**:
   - Shadow control stack with validation
   - Return address encryption when supported
   - Stack frame validation on unwind
   - Structured control flow enforcement
   
   **Platform Requirements**:
   - Hardware CET support provides additional protection
   - Software-only mode available with reduced coverage

Resource Protection Mechanisms
==============================

.. safety:: Memory Quota Enforcement
   :id: SAFETY_RESOURCE_001
   :detection: 100% of quota violations
   :reaction: Deny allocation, return error
   :asil: A, B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: Memory usage is limited per module with strict enforcement to prevent 
   resource exhaustion.
   
   **Implementation**:
   - Per-module memory quotas (default 1MB, configurable)
   - Allocation tracking with O(1) quota checks
   - Hierarchical quotas for module groups
   - No dynamic allocation after initialization in ASIL-D
   
   **Configuration**:
   ```rust
   const MAX_MEMORY_PAGES: u32 = 16; // 1MB with 64KB pages
   const MEMORY_GROWTH_ENABLED: bool = false; // For ASIL-D
   ```

.. safety:: Execution Fuel Limiting
   :id: SAFETY_RESOURCE_002
   :detection: 100% of fuel exhaustion
   :reaction: Controlled termination
   :asil: B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: Execution is limited through a fuel mechanism to prevent infinite 
   loops and ensure bounded execution time.
   
   **Implementation**:
   - Fuel consumption per instruction (configurable costs)
   - Fuel checks at loop headers and function entries
   - Interruptible execution for external timeout
   - Deterministic fuel consumption for WCET analysis
   
   **Fuel Costs** (example):
   - Basic arithmetic: 1 fuel
   - Memory access: 2 fuel  
   - Function call: 10 fuel
   - Indirect call: 15 fuel

.. safety:: Table Size Limits
   :id: SAFETY_RESOURCE_003
   :detection: 100% of limit violations
   :reaction: Deny growth, return error
   :asil: A, B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: Function and element tables are size-limited to prevent resource 
   exhaustion attacks.
   
   **Implementation**:
   - Configurable maximum table size (default 10K elements)
   - Pre-allocation in safety-critical mode
   - Growth tracking and validation
   - No dynamic table growth in ASIL-D

Temporal Safety Mechanisms
==========================

.. safety:: Watchdog Integration
   :id: SAFETY_TEMPORAL_001
   :detection: 100% of deadline violations
   :reaction: External watchdog reset
   :asil: C, D
   :diagnostic_coverage: 100%
   
   **Description**: Integration with external watchdog timer for detecting execution 
   lockups and deadline violations.
   
   **Implementation**:
   - Periodic watchdog feeding during execution
   - Configurable feeding intervals
   - Execution checkpoint markers
   - Clean shutdown on watchdog timeout warning
   
   **Integration Requirements**:
   - System must provide watchdog with warning period
   - Minimum 1ms warning before reset

.. safety:: Bounded Loop Detection
   :id: SAFETY_TEMPORAL_002
   :detection: >80% of potentially infinite loops
   :reaction: Fuel-based termination
   :asil: B, C, D
   :diagnostic_coverage: >75%
   
   **Description**: Loops are monitored for bounded execution through static analysis 
   and runtime checks.
   
   **Implementation**:
   - Loop fuel consumption tracking
   - Loop iteration counting for simple loops
   - Static analysis for loop bound inference
   - Runtime validation of loop variants

Data Integrity Mechanisms
=========================

.. safety:: Type Safety Enforcement
   :id: SAFETY_DATA_001
   :detection: 100% of type violations
   :reaction: Trap execution
   :asil: A, B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: WebAssembly type system is strictly enforced preventing type 
   confusion vulnerabilities.
   
   **Implementation**:
   - Static type checking during validation
   - Runtime type checks for indirect calls
   - No type casts or unions allowed
   - Memory is typed only as bytes
   
   **Guarantees**:
   - No undefined behavior from type errors
   - Predictable trap on type mismatch

.. safety:: Data Flow Tracking
   :id: SAFETY_DATA_002
   :detection: >90% of unauthorized data flows
   :reaction: Block data transfer
   :asil: C, D
   :diagnostic_coverage: >85%
   
   **Description**: Information flow control prevents data leakage between different 
   criticality levels.
   
   **Implementation**:
   - Taint tracking for high-criticality data
   - Interface validation for data exports
   - No implicit data sharing between modules
   - Audit logging of data transfers

Fault Detection Mechanisms
==========================

.. safety:: Built-In Self Test (BIST)
   :id: SAFETY_DETECT_001
   :detection: >95% of permanent faults
   :reaction: Prevent operation, report failure
   :asil: C, D
   :diagnostic_coverage: 90%
   
   **Description**: Power-on and periodic self-tests verify correct operation of safety 
   mechanisms.
   
   **Implementation**:
   - Memory pattern tests (walking 1s/0s)
   - Arithmetic unit verification  
   - Control flow test patterns
   - Safety mechanism verification
   
   **Test Schedule**:
   - Power-on: Full test suite (~100ms)
   - Periodic: Quick tests (~1ms every 100ms)
   - On-demand: Full test via API

.. safety:: Runtime Assertion Checking
   :id: SAFETY_DETECT_002
   :detection: 100% of assertion violations
   :reaction: Trap and diagnostic dump
   :asil: A, B, C, D
   :diagnostic_coverage: 100%
   
   **Description**: Critical invariants are continuously verified during execution with 
   immediate detection of violations.
   
   **Implementation**:
   - Precondition checks on safety-critical functions
   - Postcondition verification  
   - Invariant checks at key points
   - Diagnostic information collection
   
   **Performance Mode**:
   - Can be selectively disabled for QM components
   - Always enabled for ASIL components

Error Recovery Mechanisms
=========================

.. safety:: Checkpoint and Rollback
   :id: SAFETY_RECOVERY_001
   :detection: N/A (recovery mechanism)
   :reaction: Restore last known good state
   :asil: C, D
   :diagnostic_coverage: N/A
   
   **Description**: Execution state can be checkpointed and restored to recover from 
   transient faults.
   
   **Implementation**:
   - Lightweight state snapshots
   - Copy-on-write optimization
   - Configurable checkpoint intervals
   - Automatic rollback on fault detection
   
   **Limitations**:
   - I/O operations cannot be rolled back
   - External side effects must be managed by system

.. safety:: Graceful Degradation
   :id: SAFETY_RECOVERY_002
   :detection: N/A (recovery mechanism)
   :reaction: Reduced functionality mode
   :asil: B, C, D
   :diagnostic_coverage: N/A
   
   **Description**: System can operate in degraded mode with reduced functionality when 
   non-critical components fail.
   
   **Implementation**:
   - Component criticality classification
   - Degraded mode configuration
   - Feature disabling on fault
   - Performance reduction for safety
   
   **Degradation Levels**:
   1. Full operation (all features)
   2. Safe mode (critical features only)
   3. Limp mode (minimum functionality)
   4. Shutdown (safe state only)

Configuration Guidelines
========================

ASIL-Specific Configuration
---------------------------

.. list-table:: Mechanism Configuration by ASIL Level
   :widths: 30 15 15 15 15
   :header-rows: 1

   * - Safety Mechanism
     - ASIL-A
     - ASIL-B  
     - ASIL-C
     - ASIL-D
   * - Memory Bounds Checking
     - Enabled
     - Enabled
     - Enabled
     - Enabled
   * - Memory Isolation
     - Optional
     - Recommended
     - Required
     - Required
   * - Stack Protection
     - Basic
     - Enhanced
     - Full
     - Full+HW
   * - CFI Protection
     - Optional
     - Recommended
     - Required
     - Required
   * - Execution Fuel
     - Optional
     - Required
     - Required
     - Required
   * - Watchdog Integration
     - Optional
     - Optional
     - Required
     - Required
   * - BIST
     - Startup
     - Startup
     - Periodic
     - Continuous
   * - Checkpointing
     - No
     - Optional
     - Recommended
     - Required

Performance Impact
------------------

.. list-table:: Mechanism Performance Overhead
   :widths: 40 20 40
   :header-rows: 1

   * - Safety Mechanism
     - Overhead
     - Mitigation Strategy
   * - Memory Bounds Checking
     - 5-15%
     - Bounds elision optimization
   * - CFI Protection
     - 5-10%
     - Type caching, hw support
   * - Execution Fuel
     - 10-20%
     - Coarse-grained fuel checks
   * - Runtime Assertions
     - 5-30%
     - Selective deployment
   * - Checkpointing
     - 1-5%
     - Copy-on-write, intervals

Integration Checklist
=====================

Platform Requirements
---------------------

□ Memory protection unit (MPU) or MMU available
□ Hardware atomic operations support  
□ Reliable timer/clock source
□ Watchdog timer with warning period
□ Sufficient memory for safety mechanisms
□ Hardware CFI support (optional but recommended)

Configuration Steps
-------------------

1. Select target ASIL level
2. Configure mechanisms per ASIL table above
3. Set resource limits based on application
4. Configure watchdog integration
5. Enable appropriate diagnostic coverage
6. Verify configuration with built-in tests

Validation Requirements
-----------------------

- Fault injection testing for each mechanism
- Performance profiling with mechanisms enabled
- Diagnostic coverage measurement
- Integration testing with system-level safety

See Also
========

- :doc:`requirements` - Safety requirements addressed by these mechanisms
- :doc:`implementations` - Detailed implementation descriptions
- :doc:`verification` - Test procedures for mechanisms
- :doc:`configuration` - Configuration procedures