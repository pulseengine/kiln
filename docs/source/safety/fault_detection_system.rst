============================
Fault Detection System
============================

This document describes the systematic fault detection mechanisms implemented for ASIL-A compliance in the Kiln runtime.

.. contents:: On this page
   :local:
   :depth: 2

Overview
--------

The Kiln fault detection system provides runtime monitoring and detection of memory safety violations as required by ISO 26262 for ASIL-A systems. It operates as a lightweight, always-on system that can detect, report, and respond to various types of faults.

Architecture
------------

Core Components
~~~~~~~~~~~~~~~

1. **FaultDetector**: Central monitoring system
   
   - Atomic counters for fault statistics
   - Configurable response modes
   - No dynamic allocation

2. **FaultType**: Enumeration of detectable faults
   
   - Budget violations
   - Bounds violations
   - Capability violations
   - Memory corruption
   - Alignment violations

3. **FaultContext**: Contextual information for each operation
   
   - Crate ID performing operation
   - Operation type (read/write/allocate)
   - Memory address and size

Fault Response Modes
~~~~~~~~~~~~~~~~~~~~

The system supports three response modes for different deployment scenarios:

.. code-block:: rust

   pub enum FaultResponseMode {
       /// Log and continue (development mode)
       LogOnly,
       
       /// Log and degrade gracefully (ASIL-A default)
       GracefulDegradation,
       
       /// Log and halt execution (highest safety)
       HaltOnFault,
   }

**LogOnly Mode:**
- Records fault statistics
- Continues execution
- Suitable for development/testing

**GracefulDegradation Mode (Default):**
- Records fault statistics
- Attempts recovery where possible
- Returns errors for unrecoverable faults
- ASIL-A recommended mode

**HaltOnFault Mode:**
- Records fault statistics
- Immediately halts system on any fault
- Maximum safety for critical systems

Detectable Fault Types
----------------------

Memory Budget Violations
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   FaultType::BudgetExceeded { requested: usize, available: usize }

- Detected when allocation exceeds available memory budget
- Tracks both requested and available amounts
- Updates memory watermark for monitoring

Bounds Violations
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   FaultType::BoundsViolation { index: usize, limit: usize }

- Array/buffer access outside valid range
- Detected before memory access occurs
- Prevents buffer overflows/underflows

Capability Violations
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   FaultType::CapabilityViolation { crate_id: CrateId }

- Unauthorized memory access attempts
- Capability check failures
- Cross-component access violations

Memory Corruption
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   FaultType::MemoryCorruption { address: usize }

- Detected corruption at specific addresses
- Checksum mismatches
- Invalid memory patterns

Additional Fault Types
~~~~~~~~~~~~~~~~~~~~~~

- **UseAfterFree**: Accessing deallocated memory
- **NullPointer**: Null pointer dereferences
- **StackOverflow**: Stack exhaustion detection
- **AlignmentViolation**: Misaligned memory access

Integration with Kiln Components
-------------------------------

BoundedVec Integration
~~~~~~~~~~~~~~~~~~~~~~

The fault detection system is integrated into core data structures:

.. code-block:: rust

   // In BoundedVec::push()
   #[cfg(feature = "fault-detection")]
   {
       let context = FaultContext {
           crate_id: CrateId::Foundation,
           operation: OperationType::Write,
           address: None,
           size: Some(self.length + 1),
       };
       fault_detector().check_bounds(self.length + 1, N_ELEMENTS, &context)?;
   }

Memory Provider Integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Memory providers can use fault detection for:

- Budget enforcement
- Alignment checking
- Access validation

Global Fault Detector
~~~~~~~~~~~~~~~~~~~~~

A single global instance provides system-wide monitoring:

.. code-block:: rust

   static FAULT_DETECTOR: FaultDetector = 
       FaultDetector::new(FaultResponseMode::GracefulDegradation);
   
   pub fn fault_detector() -> &'static FaultDetector {
       &FAULT_DETECTOR
   }

Usage Examples
--------------

Basic Bounds Checking
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_foundation::fault_detection::{fault_detector, FaultContext, OperationType};
   
   let context = FaultContext {
       crate_id: CrateId::Component,
       operation: OperationType::Read,
       address: Some(0x1000),
       size: Some(128),
   };
   
   // Check bounds before array access
   fault_detector().check_bounds(index, array_len, &context)?;

Budget Verification
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   let context = FaultContext {
       crate_id: CrateId::Runtime,
       operation: OperationType::Allocate,
       address: None,
       size: Some(requested_size),
   };
   
   // Verify memory budget before allocation
   fault_detector().check_budget(requested_size, available_memory, &context)?;

Using Convenience Macros
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_foundation::{check_bounds, check_budget};
   
   // Bounds checking with automatic context
   check_bounds!(index, limit, CrateId::Foundation)?;
   
   // Budget checking with automatic context
   check_budget!(requested, available, CrateId::Component)?;

Fault Statistics
----------------

The system maintains runtime statistics:

.. code-block:: rust

   pub struct FaultStatistics {
       pub memory_violations: u32,
       pub budget_violations: u32,
       pub bounds_violations: u32,
       pub capability_violations: u32,
       pub memory_watermark: usize,
   }

Accessing statistics:

.. code-block:: rust

   let stats = fault_detector().get_statistics();
   println!("Bounds violations: {}", stats.bounds_violations);
   println!("Memory high water mark: {} bytes", stats.memory_watermark);

Platform Integration
--------------------

Logging Integration
~~~~~~~~~~~~~~~~~~~

The fault detector requires platform-specific logging implementation:

- **Linux**: syslog integration
- **Embedded**: UART/debug output
- **RTOS**: System event logger

Halt Mechanism
~~~~~~~~~~~~~~

For ``HaltOnFault`` mode, platforms must provide:

- Safe system halt function
- Optional diagnostic dump
- Watchdog integration

Performance Impact
------------------

Design Considerations
~~~~~~~~~~~~~~~~~~~~~

- **Zero-cost when disabled**: Feature flag allows complete removal
- **Minimal overhead**: Atomic operations for counters
- **Inline checking**: Critical paths use inline functions
- **No allocation**: Static global instance

Measured Impact
~~~~~~~~~~~~~~~

Typical overhead per check:

- Bounds check: ~5-10 CPU cycles
- Budget check: ~10-15 CPU cycles  
- Alignment check: ~5 CPU cycles

Memory overhead:

- Global detector: ~48 bytes
- Per-context: 32 bytes (stack allocated)

Configuration
-------------

Feature Flags
~~~~~~~~~~~~~

Enable fault detection with:

.. code-block:: toml

   [dependencies]
   kiln-foundation = { version = "0.1", features = ["fault-detection"] }

Runtime Configuration
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Enable/disable at runtime
   fault_detector().set_enabled(true);
   
   // Check if enabled
   if fault_detector().is_enabled() {
       // Perform additional validation
   }
   
   // Reset counters for testing
   fault_detector().reset_counters();

ASIL-A Compliance
-----------------

The fault detection system satisfies ASIL-A requirements:

1. **Systematic Fault Detection** (ISO 26262-6:7.4.14)
   
   - ✅ Detects memory violations before they occur
   - ✅ Provides diagnostic information
   - ✅ Supports graceful degradation

2. **Runtime Monitoring** (ISO 26262-6:7.4.13)
   
   - ✅ Continuous monitoring of safety properties
   - ✅ Statistical tracking for analysis
   - ✅ Configurable response strategies

3. **Defensive Programming** (ISO 26262-6:7.4.6)
   
   - ✅ Validates all inputs before use
   - ✅ Fail-safe defaults
   - ✅ Predictable error handling

Testing and Validation
----------------------

Unit Tests
~~~~~~~~~~

Comprehensive test coverage includes:

- All fault types
- All response modes
- Boundary conditions
- Concurrent access

Integration Tests
~~~~~~~~~~~~~~~~~

- BoundedVec with fault detection
- Memory provider integration
- Cross-component scenarios

Formal Verification
~~~~~~~~~~~~~~~~~~~

KANI proofs verify:

- Fault detection doesn't introduce new failures
- Atomic operations are race-free
- Response modes behave correctly

Future Enhancements
-------------------

Planned improvements for higher ASIL levels:

1. **ASIL-B/C Enhancements**
   
   - Redundant fault detection
   - Voting mechanisms
   - Enhanced diagnostics

2. **ASIL-D Features**
   
   - Hardware fault detection integration
   - Dual-channel checking
   - Certified fault response

3. **Advanced Monitoring**
   
   - Trend analysis
   - Predictive fault detection
   - Machine learning integration

References
----------

- ISO 26262-6:2018 - Software development
- MISRA C:2012 - Rule 21.3 (dynamic memory)
- :doc:`/safety/asil_a_safety_case` - Overall safety case
- :doc:`/architecture/memory_model` - Memory architecture