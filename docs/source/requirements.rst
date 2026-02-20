Requirements
============

This section contains the requirements for the Kiln project.

Functional Requirements
-----------------------

.. req:: Platform Abstraction Layer
   :id: REQ_PLATFORM_001
   :status: implemented

   The runtime shall provide a platform abstraction layer (PAL) with distinct backends 
   for target operating systems (macOS, Linux, QNX, Zephyr) and bare-metal environments, 
   selectable via compile-time features. The PAL shall abstract OS-specific APIs for 
   memory allocation/protection and synchronization primitives (futex-like operations).

.. req:: Resumable Interpreter
   :id: REQ_FUNC_001
   :status: implemented
   
   The interpreter shall be resumable, allowing operation with fuel or other implementations of bounded run-time that require the interpreter to be halted and later resumed as if it was not halted.

.. req:: Baremetal Support
   :id: REQ_FUNC_002
   :status: implemented
   
   The interpreter shall be executable on bare-metal environments with no reliance on any specific functionality from the provided execution environment, as it shall be ready for embedding to any environment that Rust can compile for.

.. req:: Bounded Execution
   :id: REQ_FUNC_003
   :status: implemented
   
   The interpreter shall yield back control flow eventually, allowing users to call the interpreter with a bound and expect a result in a finite amount of time or bytecode operations, even if the bytecode itself never finishes execution.

.. req:: State Migration
   :id: REQ_FUNC_004
   :status: planned
   
   The interpreter state shall be able to halt on one computer and continue execution on another, enabling various workflows in deployments of multiple computers for load-balancing or redundancy purposes.

.. req:: WebAssembly Component Model Support
   :id: REQ_FUNC_014
   :status: partial
   
   The interpreter shall implement the WebAssembly Component Model Preview 2 specification, including:
   - Component model validation
   - Resource type handling
   - Interface types
   - Component instantiation and linking
   - Resource lifetime management (see REQ_CMP_020, status: active)
   - Custom section handling
   - Component model binary format parsing

.. req:: Component Model Binary Format
   :id: REQ_FUNC_021
   :status: partial
   :links: REQ_FUNC_014
   
   The interpreter shall strictly implement the WebAssembly Component Model binary format as specified in 
   the official WebAssembly Component Model specification at 
   https://github.com/WebAssembly/component-model/blob/main/design/mvp/Binary.md, including:
   
   - Magic bytes (0x00, 0x61, 0x73, 0x6D) followed by version (0x0D, 0x00, 0x01, 0x00)
   - Component sections with proper section IDs
   - Type section encoding with canonical type representation
   - Import section handling with component, instance, and function imports
   - Core module sections including validation and embedding
   - Component instances section with proper instance creation
   - Export section with named exports and index references
   - Component custom sections for metadata and tooling
   - Canonical ABI encoding/decoding for interface values
   - LEB128 encoding for integers and field counts

.. req:: WASI Logging Support
   :id: REQ_FUNC_015
   :status: implemented
   
   The interpreter shall implement the WASI logging API as specified in the wasi-logging proposal, providing:
   - Support for all defined log levels (Error, Warn, Info, Debug, Trace)
   - Context-based logging
   - Stderr integration
   - Thread-safe logging operations

.. req:: Platform-Specific Logging
   :id: REQ_FUNC_016
   :status: planned
   :links: REQ_FUNC_015
   
   The WASI logging implementation shall provide platform-specific backends:
   - Linux: syslog integration with proper facility and priority mapping
   - macOS: Unified Logging System (os_log) integration
   - Generic fallback implementation for other platforms

.. req:: WAST Test Suite Compatibility
   :id: REQ_FUNC_022
   :status: partial
   
   The interpreter shall be testable against the official WebAssembly specification (WAST) test suite to ensure conformance and correctness.

Low-Level Functional Requirements
---------------------------------

.. req:: Helper Runtime C ABI Exports
   :id: REQ_HELPER_ABI_001
   :status: new

   The AOT helper runtime (`libkiln_helper`) shall export a stable C ABI including 
   functions for Wasm operations not efficiently inlined by the AOT compiler. 
   This shall include `kiln_memory_copy`, `kiln_memory_fill`, `kiln_memory_grow`, 
   `kiln_atomic_wait`, and `kiln_atomic_notify`.

.. req:: Stackless Implementation
   :id: REQ_LFUNC_005
   :status: implemented
   :links: REQ_FUNC_001
   
   The interpreter shall be stackless, storing the stack of the interpreted bytecode in a traditional data structure rather than using function calls in the host environment.

.. req:: No Standard Library
   :id: REQ_LFUNC_006
   :status: implemented
   :links: REQ_FUNC_002
   
   The interpreter shall be implemented in no_std Rust, only relying on functionality provided by no_std to enable execution on bare environments where no operating system is available.

.. req:: Fuel Mechanism
   :id: REQ_LFUNC_007
   :status: implemented
   :links: REQ_FUNC_003
   
   The interpreter shall support fuel bounded execution, where each bytecode instruction is associated with a specific amount of fuel consumed during execution.

.. req:: State Serialization
   :id: REQ_LFUNC_008
   :status: planned
   :links: REQ_FUNC_004
   
   The interpreter state shall be de-/serializable to enable migration to other computers and support check-point/lock-step execution.

.. req:: WebAssembly Core Implementation
   :id: REQ_LFUNC_018
   :status: partial
   :links: REQ_FUNC_014
   
   The interpreter shall implement the WebAssembly Core specification, including:
   - Module validation (see REQ_WASM_001 in safety_requirements.rst, status: active)
   - Value types and reference types
   - Instructions and control flow
   - Function calls and tables
   - Memory and data segments
   - Global variables
   - Exception handling
   - SIMD operations
   - Threading support

.. req:: Component Model Implementation
   :id: REQ_LFUNC_019
   :status: partial
   :links: REQ_FUNC_014
   
   The interpreter shall implement the Component Model specification, including:
   - WIT format parsing and validation
   - Component model binary format parsing
   - Resource type implementation
   - Interface type handling
   - Component instantiation
   - Component linking
   - Resource lifetime management

.. req:: Optimized Instruction Dispatch
   :id: REQ_LFUNC_023
   :status: planned
   :links: REQ_LFUNC_005

   The core instruction dispatch loop within the stackless engine shall be specifically optimized for execution speed. Techniques such as efficient instruction decoding, minimizing branching overhead, or platform-specific optimizations (where compatible with certifiability) should be considered.

.. req:: Efficient Operand Stack Implementation
   :id: REQ_LFUNC_024
   :status: planned
   :links: REQ_LFUNC_005

   The stackless operand stack implementation (`StacklessStack` or equivalent) shall be designed and optimized for efficient push/pop operations, minimal memory overhead, and robust handling of potential overflow conditions suitable for the target `no_std` environments.

.. req:: Efficient Branch Pre-calculation
   :id: REQ_LFUNC_025
   :status: planned
   :links: REQ_LFUNC_005, REQ_LFUNC_018

   The pre-calculation of branch targets (e.g., `label.continuation` values) shall be performed efficiently, ideally integrated with the module validation or loading process, to minimize runtime startup costs.

.. req:: Minimize Code Complexity for Certification
   :id: REQ_LFUNC_026
   :status: planned
   :links: REQ_OBS_012, REQ_OBS_013

   To enhance certifiability and maintainability, the Kiln interpreter codebase shall strive for simplicity, minimize the use of complex language features (e.g., procedural macros), and restrict external dependencies to those strictly necessary for core functionality or explicitly required features (like logging or `no_std` math).

Dependency Requirements
-----------------------

.. req:: Logging Support
   :id: REQ_DEP_009
   :status: implemented
   
   The interpreter shall have an optional dependency on the ``log`` crate version ``0.4.22`` for observability and debugging purposes.

.. req:: Math Library
   :id: REQ_DEP_010
   :status: planned
   
   The interpreter may depend on the ``libm`` crate version ``0.2.8`` for floating-point operations required in no_std environments.

.. req:: Rust Version
   :id: REQ_DEP_011
   :status: implemented
   
   The interpreter shall compile on Rust ``1.76.0`` and later versions.


Observability Requirements
--------------------------

.. req:: Instrumentation Support
   :id: REQ_OBS_012
   :status: partial
   
   The interpreter shall implement means for instrumentation to support certification evidence generation, debugging, and run-time monitoring.

.. req:: Coverage Measurement
   :id: REQ_OBS_013
   :status: partial
   :links: REQ_OBS_012
   
   The instrumentation shall enable the measurement of:
   
   - Statement coverage (DO-178C DAL-C)
   - Decision coverage (DO-178C DAL-B)
   - Modified condition/decision coverage (DO-178C DAL-A)

Advanced Runtime Requirements
-----------------------------

.. req:: Async/Await Runtime Support
   :id: REQ_FUNC_030
   :status: implemented
   
   The interpreter shall provide comprehensive async/await runtime support for WebAssembly Component Model, including:
   - Async canonical lifting and lowering
   - Async execution engine with future-based task management
   - Async resource cleanup and lifecycle management
   - Runtime bridge for async-to-sync interoperability
   - Context preservation across async boundaries

.. req:: Advanced Threading Support
   :id: REQ_FUNC_031
   :status: partial
   
   The interpreter shall implement advanced threading capabilities including:
   - Task manager with cancellation support
   - Thread spawning with fuel-based resource control
   - Waitable set primitives for thread synchronization
   - Thread-safe builtin operations
   - Integration with platform-specific threading backends

.. req:: Debug Infrastructure
   :id: REQ_FUNC_032
   :status: implemented
   
   The interpreter shall provide comprehensive debugging capabilities including:
   - DWARF debug information parsing and processing
   - WIT-aware debugging with source mapping
   - Runtime breakpoint management
   - Stack trace generation with source information
   - Memory inspection and variable examination
   - Step-by-step execution control

.. req:: Fuzzing Infrastructure
   :id: REQ_QA_010
   :status: implemented
   
   The interpreter shall include comprehensive fuzzing infrastructure covering:
   - Bounded collections fuzzing (stack, vec, queue)
   - Memory adapter and safe memory fuzzing
   - Component model parser fuzzing
   - Type bounds and canonical options fuzzing
   - WIT parser fuzzing
   - Continuous fuzzing integration

.. req:: Multiple Runtime Modes
   :id: REQ_FUNC_033
   :status: implemented
   
   The interpreter shall support multiple runtime deployment modes through the kilnd daemon:
   - Standard mode (kilnd-std) with full standard library support
   - Allocation mode (kilnd-alloc) for embedded systems with heap
   - No-std mode (kilnd-nostd) for pure bare-metal deployment
   - Each mode with appropriate resource limits and safety constraints

.. req:: Hardware Optimization Support
   :id: REQ_PERF_010
   :status: implemented
   
   The interpreter shall support platform-specific hardware optimizations including:
   - SIMD acceleration for supported architectures (x86_64, aarch64)
   - ARM Memory Tagging Extension (MTE) for spatial memory safety
   - Intel Control-flow Enforcement Technology (CET) integration
   - Hardware-accelerated atomic operations
   - CPU-specific instruction scheduling optimizations

.. req:: Formal Verification Support
   :id: REQ_VERIFY_010
   :status: implemented
   
   The interpreter shall include support for formal verification through:
   - Kani proof harness integration
   - Model checking annotations
   - Invariant specifications
   - Bounded verification for critical paths
   - Integration with verification registry

Implementation Status
---------------------

.. needtable::
   :columns: id;title;status
   :filter: type == 'req'

Requirement Relationships
-------------------------

.. needflow::
   :filter: id in ['REQ_PLATFORM_001', 'REQ_HELPER_ABI_001', 'REQ_FUNC_001', 'REQ_FUNC_002', 'REQ_FUNC_003', 'REQ_FUNC_004', 'REQ_LFUNC_005', 'REQ_LFUNC_006', 'REQ_LFUNC_007', 'REQ_LFUNC_008', 'REQ_DEP_009', 'REQ_DEP_010', 'REQ_DEP_011', 'REQ_OBS_012', 'REQ_OBS_013', 'REQ_FUNC_014', 'REQ_FUNC_015', 'REQ_FUNC_016', 'REQ_LFUNC_018', 'REQ_LFUNC_019', 'REQ_CMP_020', 'REQ_FUNC_021', 'REQ_FUNC_022', 'REQ_LFUNC_023', 'REQ_LFUNC_024', 'REQ_LFUNC_025', 'REQ_LFUNC_026']

Component Model Requirements
----------------------------

.. req:: Component Resource Lifecycle Management
   :id: REQ_CMP_020
   :status: active
   
   The WebAssembly component model implementation shall provide comprehensive lifecycle management for resource types, ensuring proper creation, tracking, and disposal of resources.