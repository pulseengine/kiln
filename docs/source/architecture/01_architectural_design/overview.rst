==========================
Architectural Overview
==========================

.. image:: ../../_static/icons/kiln_architecture.svg
   :width: 64px
   :align: right
   :alt: Architecture Icon

**Teaching Point**: This overview shows how PulseEngine (Kiln Edition) is decomposed into manageable, testable components that work together to execute WebAssembly safely.

.. warning::
   **Implementation Status**: This document describes the target architecture. Core WebAssembly 
   execution engine is under development. See :doc:`../../overview/implementation_status` for current state.

System Context
--------------

.. arch_component:: Kiln System
   :id: ARCH_COMP_SYSTEM
   :type: system
   :tags: core

PulseEngine (Kiln Edition) is WebAssembly infrastructure designed for safety-critical systems. It provides:

- **Implemented**: WebAssembly memory operations and type system
- **Implemented**: Safety-critical memory allocation strategies
- **Implemented**: Multi-platform abstraction layer
- **In Development**: WebAssembly Core specification execution
- **In Development**: Component Model support
- **Implemented**: Formal safety verification infrastructure

High-Level Architecture
-----------------------

The system follows a layered architecture with clear separation of concerns.

Component Decomposition
~~~~~~~~~~~~~~~~~~~~~~~

.. arch_decision:: Component-Based Architecture
   :id: ARCH_DEC_COMP_001
   :status: partial
   :rationale: Modular design enables independent testing, certification, and platform-specific optimization
   :impacts: All architectural components
   :links: ARCH_COMP_SYSTEM
   
   **Status Note**: Architecture is designed and infrastructure implemented. Core execution components in development.

The following diagram shows the high-level decomposition of PulseEngine into its constituent components:

.. uml:: ../../_static/high_level_decomposition_simple.puml

**Teaching Point**: Each component has a single, well-defined responsibility. The layered architecture ensures that:

- Higher layers depend only on lower layers
- Each layer provides a stable interface to the layer above
- Platform-specific code is isolated in the Platform Abstraction Layer

Deployment Architecture
~~~~~~~~~~~~~~~~~~~~~~~

.. arch_decision:: Multi-Platform Deployment Strategy
   :id: ARCH_DEC_DEPLOY_001
   :status: partial
   :rationale: Different deployment environments require different memory models and security features
   :impacts: Platform layer, memory management
   :links: ARCH_CON_001, ARCH_CON_002
   
   **Status Note**: Platform abstraction layer implemented. Platform-specific features vary in completion.

PulseEngine can be deployed across various platforms with platform-specific optimizations:

.. uml:: ../../_static/deployment_architecture_simple.puml

**Key Deployment Configurations**:

1. **Server/Desktop** (Linux, macOS)
   
   - Full ``std`` library support
   - Dynamic memory allocation
   - Multi-threading with OS primitives
   - Hardware security features (ARM PAC/BTI, Intel CET)

2. **Embedded Linux** (Yocto, BuildRoot)
   
   - ``no_std`` with custom allocator
   - Bounded memory usage
   - Optional ARM MTE support
   - Static or dynamic linking

3. **QNX Neutrino** (Automotive, Medical)
   
   - Memory partitioning for isolation
   - Capability-based security
   - Real-time scheduling
   - ASIL-D compliance support

4. **VxWorks** (Industrial, Aerospace)
   
   - LKM (kernel space) and RTP (user space) contexts
   - Memory partition management
   - Real-time priority scheduling
   - Industrial-grade reliability

5. **Zephyr RTOS** (IoT, Embedded)
   
   - ``no_std``, no heap allocation
   - Static memory pools
   - Minimal footprint
   - Direct hardware access

6. **Bare Metal** (Safety-critical)
   
   - No OS dependencies
   - Compile-time memory allocation
   - Deterministic execution
   - Minimal runtime overhead

Internal Module Structure
~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_decision:: Crate Organization Strategy
   :id: ARCH_DEC_CRATE_001
   :status: implemented
   :rationale: Fine-grained crates enable selective feature inclusion and minimize dependencies
   :impacts: Build system, dependency management
   :links: ARCH_CON_003

The following diagram shows the internal structure of key crates and their modules:

.. uml:: ../../_static/crate_module_structure_simple.puml

**Teaching Point**: The modular structure enables:

- Selective feature compilation (e.g., exclude Component Model for embedded)
- Platform-specific implementations behind common interfaces
- Clear dependency boundaries for safety analysis
- Independent testing and verification of each module

Workspace Organization
----------------------

The implementation consists of 24 specialized crates:

.. list-table:: Crate Organization
   :header-rows: 1
   :widths: 20 50 30

   * - Category
     - Crates
     - Purpose
   * - Foundation
     - ``kiln-error``, ``kiln-foundation``, ``kiln-format``, ``kiln-sync``
     - Core types, error handling, memory safety
   * - Decoding
     - ``kiln-decoder``
     - WebAssembly binary parsing
   * - Execution
     - ``kiln-runtime``, ``kiln-instructions``
     - Instruction execution and runtime
   * - Component Model
     - ``kiln-component``
     - Component Model implementation
   * - Platform
     - ``kiln-platform``
     - OS abstraction layer
   * - Integration
     - ``kiln-host``, ``kiln-intercept``, ``kiln-logging``
     - Host integration and monitoring
   * - Applications
     - ``kiln``, ``kilnd``
     - Library facade and CLI daemon

Environment Support Strategy
----------------------------

.. arch_decision:: Multi-Environment Architecture
   :id: ARCH_DEC_ENV_001
   :status: implemented
   :rationale: Different deployment scenarios require different resource trade-offs
   :impacts: All components

**Teaching Point**: The architecture supports four distinct environment configurations, each with specific trade-offs:

1. **Full std Environment**
   
   .. code-block:: rust
   
      // All standard library features available
      use std::collections::{HashMap, Vec};
      use std::sync::{Arc, Mutex};

   - **Use Case**: Server deployments, development
   - **Benefits**: Full functionality, familiar APIs
   - **Trade-offs**: Larger binary size, not suitable for embedded

2. **no_std with alloc**
   
   .. code-block:: rust
   
      #![no_std]
      extern crate alloc;
      use alloc::vec::Vec;
      use alloc::collections::BTreeMap as HashMap;

   - **Use Case**: Embedded systems with heap
   - **Benefits**: Dynamic allocation, smaller binary
   - **Trade-offs**: No file I/O, threading, or OS integration

3. **no_std without alloc**
   
   .. code-block:: rust
   
      #![no_std]
      use kiln_foundation::bounded::{BoundedVec, BoundedString};
      
      // Fixed capacity, no heap allocation
      let mut vec: BoundedVec<u32, 100> = BoundedVec::new();

   - **Use Case**: Safety-critical embedded, bare-metal
   - **Benefits**: Predictable memory usage, no heap fragmentation
   - **Trade-offs**: Fixed capacity limits, manual memory management

4. **Bare-metal**
   
   - **Use Case**: Minimal embedded systems
   - **Benefits**: Minimal overhead, direct hardware access
   - **Trade-offs**: Limited functionality, platform-specific

Key Architectural Principles
----------------------------

.. arch_constraint:: Safety First
   :id: ARCH_CON_001
   :priority: high
   
   All components must be memory-safe and avoid undefined behavior.

.. arch_constraint:: Deterministic Execution
   :id: ARCH_CON_002
   :priority: high
   
   Execution time and resource usage must be predictable.

.. arch_constraint:: Modular Design
   :id: ARCH_CON_003
   :priority: medium
   
   Components must be independently testable and replaceable.

Component Interaction Model
---------------------------

**Teaching Point**: Components interact through well-defined interfaces:

.. code-block:: rust

   // Example: How the decoder interacts with the runtime
   let module = kiln_decoder::decode_module(&wasm_bytes)?;
   let instance = kiln_runtime::instantiate(module, imports)?;
   let result = instance.invoke("function_name", &args)?;

Cross-References
----------------

- **Implementation Examples**: See :doc:`/examples/hello_world` for basic usage
- **Component Details**: See :doc:`components` for detailed component descriptions
- **Layer Architecture**: See :doc:`layers` for layer responsibilities
- **Design Patterns**: See :doc:`patterns` for architectural patterns used