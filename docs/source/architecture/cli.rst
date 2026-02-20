=======================
CLI (KILND) Architecture
=======================

The KilnD command-line interface provides a user-friendly way to execute WebAssembly modules and components.

.. warning::
   **Implementation Status**: CLI infrastructure is implemented but WebAssembly execution 
   functionality requires completion of the core execution engine.

.. spec:: CLI Architecture
   :id: SPEC_005
   :links: REQ_003, REQ_015, REQ_RESOURCE_004
   
   .. uml:: ../../_static/cli_architecture.puml
      :alt: CLI Architecture
      :width: 100%
   
   The CLI architecture includes:
   
   1. Command-line argument parsing
   2. Module loading and instantiation
   3. Execution control with fuel limits
   4. Statistics reporting
   5. Logging configuration
   6. Component interface analysis capabilities
   7. WASI interface implementation

.. impl:: CLI Implementation
   :id: IMPL_008
   :status: partial
   :links: SPEC_005, REQ_003, REQ_015, REQ_RESOURCE_004, IMPL_FUEL_001
   
   The KilnD CLI provides:
   
   1. **Implemented**: Command-line argument parsing
   2. **In Development**: WebAssembly file loading (parsing only)
   3. **Not Implemented**: Function calling (requires execution engine)
   4. **Not Implemented**: Fuel-bounded execution (requires execution engine)
   5. **Implemented**: Execution statistics infrastructure
   6. **Implemented**: Logging configuration and output (see :doc:`logging`)
   7. **Partial**: Component interface parsing
   8. **In Development**: Module and component support
   
   Command-line options include:
   - ``--call <function>`` - **Not Functional**: Requires execution engine
   - ``--fuel <amount>`` - **Not Functional**: Requires execution engine  
   - ``--stats`` - **Implemented**: Shows statistics when available
   - ``--analyze-component-interfaces`` - **Partial**: Basic parsing only 