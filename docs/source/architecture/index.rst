================================
Software Architecture
================================

.. image:: ../_static/icons/kiln_architecture.svg
   :width: 64px
   :align: right
   :alt: Architecture Icon

This section provides a comprehensive view of the PulseEngine (Kiln Edition) software architecture following ASPICE SWE.2 guidelines. The architecture is designed to be teachable, traceable, and suitable for safety-critical systems.

.. warning::
   **Implementation Status**: This section describes both implemented infrastructure and 
   planned functionality. Component structure exists but core WebAssembly execution is under 
   development. See :doc:`test_coverage` for actual implementation metrics.

.. admonition:: Architecture Organization
   :class: note

   This documentation follows ASPICE SWE.2 base practices:
   
   - **BP1**: Develop software architectural design
   - **BP2**: Allocate software requirements  
   - **BP3**: Define interfaces
   - **BP4**: Describe dynamic behavior
   - **BP5**: Define resource consumption objectives
   - **BP6**: Evaluate alternative architectures

Quick Navigation
----------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🏗️ Architectural Design
      :link: 01_architectural_design/overview
      :link-type: doc

      System decomposition, components, layers, and patterns

   .. grid-item-card:: 📊 Requirements Allocation
      :link: 02_requirements_allocation/allocation_matrix
      :link-type: doc

      Mapping requirements to architectural components

   .. grid-item-card:: 🔌 Interface Definitions
      :link: 03_interfaces/interface_catalog
      :link-type: doc

      Component interfaces, APIs, and contracts

   .. grid-item-card:: 🔄 Dynamic Behavior
      :link: 04_dynamic_behavior/interaction_flows
      :link-type: doc

      Runtime behavior, state machines, and sequences

   .. grid-item-card:: 📈 Resource Management
      :link: 05_resource_management/resource_overview
      :link-type: doc

      Memory, CPU, and I/O resource budgets

   .. grid-item-card:: 🤔 Design Decisions
      :link: 06_design_decisions/decision_log
      :link-type: doc

      Architectural decisions, trade-offs, and rationale

Key Architectural Decisions
---------------------------

.. arch_decision:: Multi-Environment Support Strategy
   :id: ARCH_DEC_CORE_001
   :status: accepted
   :tags: core, portability

   **Decision**: Support four environment configurations:
   
   1. **std** - Full standard library support
   2. **no_std + alloc** - No standard library, but dynamic allocation
   3. **no_std + no_alloc** - Only static allocation with bounded collections
   4. **bare_metal** - Minimal runtime for embedded systems

   **Rationale**: Different deployment scenarios require different trade-offs between functionality and resource constraints.

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Architectural Design (BP1)

   01_architectural_design/overview
   01_architectural_design/components
   01_architectural_design/interfaces
   01_architectural_design/layers
   01_architectural_design/patterns

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Requirements Allocation (BP2)

   02_requirements_allocation/allocation_matrix
   02_requirements_allocation/traceability
   02_requirements_allocation/coverage_analysis

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Interface Definitions (BP3)

   03_interfaces/interface_catalog
   03_interfaces/external_interfaces
   03_interfaces/internal_interfaces
   03_interfaces/api_contracts
   03_interfaces/data_types

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Dynamic Behavior (BP4)

   04_dynamic_behavior/interaction_flows
   04_dynamic_behavior/state_machines
   04_dynamic_behavior/sequence_diagrams
   04_dynamic_behavior/concurrency_model

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Resource Management (BP5)

   05_resource_management/resource_overview
   05_resource_management/memory_budgets
   05_resource_management/cpu_budgets
   05_resource_management/io_constraints

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Design Decisions (BP6)

   06_design_decisions/decision_log
   06_design_decisions/adr/index
   06_design_decisions/alternatives
   06_design_decisions/trade_offs

.. toctree::
   :maxdepth: 1
   :hidden:
   :caption: Legacy Documentation

   cli
   component_model
   core_runtime
   intercept_system
   logging
   memory_model
   memory_safety_comparison
   static_memory_system
   platform_layer
   qnx_platform
   resource_management
   safe_memory
   safety
   hardening
   testing
   no_std_collections
   test_coverage
   cfi_hardening