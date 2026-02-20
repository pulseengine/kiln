=========================
Core Runtime Architecture
=========================

.. image:: ../../_static/icons/execution_flow.svg
   :width: 48px
   :align: left
   :alt: Execution Flow Icon

The core runtime is responsible for executing WebAssembly instructions and managing the WebAssembly execution environment.

.. warning::
   **Implementation Status**: This document describes the target architecture. The core execution 
   engine is currently under development. See :doc:`../overview/implementation_status` for actual status.

.. spec:: Core Runtime Execution Flow
   :id: SPEC_006
   :links: REQ_PLATFORM_001, REQ_HELPER_ABI_001, REQ_007, REQ_RESOURCE_004
   
   .. code-block:: text
      
      User -> KILND CLI -> Engine -> Module
      Engine -> Memory
      
      When fuel is exhausted:
      Engine -> KILND CLI (paused)
      KILND CLI -> Engine (add fuel)
      Engine -> KILND CLI (resume)
      
      Final result:
      Engine -> KILND CLI -> User
   
   The execution flow demonstrates the bounded execution model, showing how the KilnD CLI interacts with the Engine and how the fuel-based execution can be paused when fuel is exhausted. The diagram also illustrates the difference between executing a standard WebAssembly module and a Component Model component, highlighting the resource management and canonical conversion aspects of the Component Model.

.. spec:: Core Runtime Architecture
   :id: SPEC_002
   :links: REQ_PLATFORM_001, REQ_HELPER_ABI_001, REQ_005, REQ_006, REQ_007, REQ_MEM_SAFETY_001, REQ_ERROR_003
   
   .. image:: ../../_static/icons/stackless_engine.svg
      :width: 48px
      :align: right
      :alt: Stackless Engine Icon
   
   .. code-block:: text
      
      Core Runtime
      ├── Engine
      │   ├── Stackless Engine
      │   ├── Execution State
      │   ├── Fuel Counter
      │   └── Statistics
      ├── Module
      │   ├── Binary Format
      │   ├── Types
      │   ├── Functions
      │   └── Validation
      └── Execution
          ├── Stack
          ├── Frame
          ├── Control Flow
          └── Instructions
   
   The core runtime provides a stackless interpreter implementation designed specifically for:
   
   - Bounded execution through fuel metering
   - Resumability after execution pauses
   - No-std compatibility for bare-metal environments
   - State serialization for migration between systems

.. impl:: Engine Implementation
   :id: IMPL_001
   :status: partial
   :links: SPEC_002, REQ_PLATFORM_001, REQ_HELPER_ABI_001, REQ_007, REQ_RESOURCE_004, IMPL_FUEL_001
   
   The ``Engine`` struct is the central execution component that:
   
   1. **Implemented**: Basic engine structure and state management
   2. **Implemented**: Fuel tracking infrastructure
   3. **In Development**: Instruction execution dispatch
   4. **Implemented**: Statistics gathering capabilities
   
   Key methods include:
   - ``set_fuel(amount)`` - Sets the fuel limit for bounded execution (infrastructure ready)
   - ``execute(instance_idx, func_idx, args)`` - **In Development**: WebAssembly function execution
   - ``remaining_fuel()`` - Returns the remaining fuel after execution
   - ``stats()`` - Returns execution statistics

.. impl:: Module Implementation
   :id: IMPL_002
   :status: partial
   :links: SPEC_002, REQ_018, REQ_WASM_001
   
   The ``Module`` struct encapsulates a WebAssembly module and provides:
   
   1. **Partial**: Binary parsing (type sections work, element/data segments missing)
   2. **Implemented**: Type system and validation framework
   3. **Partial**: Function table management infrastructure
   4. **Implemented**: Memory management infrastructure
   
   Key methods include:
   - ``load_from_binary(bytes)`` - **Partial**: Type sections only
   - ``validate()`` - **Partial**: Basic structure validation
   - ``instantiate(engine)`` - **In Development**: Module instantiation

.. impl:: Stack Implementation
   :id: IMPL_004
   :status: partial
   :links: SPEC_002, REQ_005, REQ_RESOURCE_002
   
   The ``Stack`` struct implements a value stack for the stackless interpreter model:
   
   1. **Implemented**: Basic stack data structure
   2. **In Development**: Control flow label tracking
   3. **Planned**: Execution pause/resume capability
   
   This infrastructure will enable bounded execution and state migration once complete. 