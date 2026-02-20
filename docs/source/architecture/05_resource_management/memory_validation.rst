=============================
Build-Time Memory Validation
=============================

.. image:: ../../_static/icons/memory_management.svg
   :width: 64px
   :align: center
   :alt: Memory Validation Icon

Kiln includes comprehensive build-time validation for memory budgets to ensure proper memory allocation planning before runtime.

.. contents:: On this page
   :local:
   :depth: 2

Overview
--------

The build-time validation system provides:

- Validates total and per-crate memory budgets
- Checks minimum and maximum allocation limits  
- Verifies crate memory system integration
- Generates compile-time constants for memory budgets
- Provides configuration flexibility through files and environment variables

Configuration
-------------

Default Budgets
~~~~~~~~~~~~~~~

The system includes sensible defaults for all Kiln crates:

.. list-table:: Default Memory Budgets
   :header-rows: 1
   :widths: 25 20 55

   * - Crate
     - Default Budget
     - Purpose
   * - kiln-foundation
     - 8MB
     - Core memory management
   * - kiln-runtime
     - 16MB
     - Execution engine
   * - kiln-component
     - 12MB
     - Component Model support
   * - kiln-decoder
     - 4MB
     - WebAssembly decoding
   * - kiln-format
     - 2MB
     - Format handling
   * - kiln-host
     - 4MB
     - Host integration
   * - kiln-debug
     - 2MB
     - Debug tools
   * - kiln-platform
     - 8MB
     - Platform abstraction
   * - kiln-instructions
     - 4MB
     - Instruction execution
   * - kiln-logging
     - 1MB
     - Logging infrastructure
   * - kiln-intercept
     - 1MB
     - Interception/monitoring
   * - kiln-panic
     - 512KB
     - Panic handling
   * - kiln-sync
     - 1MB
     - Synchronization
   * - kiln-math
     - 512KB
     - Mathematical operations
   * - kiln-error
     - 256KB
     - Error handling
     - 256KB
     - Helper utilities

**Total Default Budget: ~65.5MB**

Custom Configuration
~~~~~~~~~~~~~~~~~~~

Configuration File
..................

Create ``memory_budget.toml`` in the workspace root:

.. code-block:: toml

   # Custom memory budgets
   "kiln-runtime" = "32MB"
   "kiln-component" = "24MB"
   "kiln-foundation" = "16MB"
   
   # Global settings
   total_budget = "128MB"
   strict_mode = true

Environment Variables
....................

Override specific budgets using environment variables:

.. code-block:: bash

   export KILN_RUNTIME_BUDGET=32MB
   export KILN_COMPONENT_BUDGET=24MB
   export KILN_TOTAL_BUDGET=128MB

Budget Format
~~~~~~~~~~~~~

Supported units:

- **KB**: Kilobytes (1,024 bytes)
- **MB**: Megabytes (1,048,576 bytes)  
- **GB**: Gigabytes (1,073,741,824 bytes)
- **Numeric**: Raw bytes

Examples:

.. code-block:: toml

   "kiln-runtime" = "16MB"      # 16,777,216 bytes
   "kiln-decoder" = "4096KB"    # 4,194,304 bytes
   "kiln-math" = 524288         # 524,288 bytes

Validation Process
------------------

Compile-Time Checks
~~~~~~~~~~~~~~~~~~~

The validation system performs these checks during compilation:

1. **Budget Consistency**: Ensures sum of crate budgets ≤ total budget
2. **Minimum Requirements**: Validates each crate meets minimum memory needs
3. **Maximum Limits**: Prevents excessive allocations
4. **Platform Constraints**: Checks platform-specific memory limits

Validation Rules
~~~~~~~~~~~~~~~~

.. code-block:: rust

   // Compile-time validation
   const _: () = {
       // Rule 1: Total budget check
       assert!(TOTAL_CRATE_BUDGETS <= TOTAL_SYSTEM_BUDGET);
       
       // Rule 2: Individual crate minimums
       assert!(FOUNDATION_BUDGET >= 1024 * 1024); // 1MB minimum
       assert!(RUNTIME_BUDGET >= 4 * 1024 * 1024); // 4MB minimum
       
       // Rule 3: Individual crate maximums  
       assert!(FOUNDATION_BUDGET <= 64 * 1024 * 1024); // 64MB maximum
       assert!(RUNTIME_BUDGET <= 256 * 1024 * 1024); // 256MB maximum
       
       // Rule 4: Platform constraints
       #[cfg(target_arch = "arm")]
       assert!(TOTAL_SYSTEM_BUDGET <= 32 * 1024 * 1024); // 32MB on ARM
   };

Integration
-----------

Build System Integration
~~~~~~~~~~~~~~~~~~~~~~~~

Add to ``Cargo.toml``:

.. code-block:: toml

   [package.metadata.memory_validation]
   config_file = "memory_budget.toml"
   strict_mode = true
   
   [build-dependencies]
   kiln-memory-validator = "0.2"

Build Script Integration
~~~~~~~~~~~~~~~~~~~~~~~~

Create ``build.rs``:

.. code-block:: rust

   use kiln_memory_validator::{validate_budgets, BudgetConfig};
   
   fn main() {
       let config = BudgetConfig::from_file("memory_budget.toml")
           .unwrap_or_default();
           
       validate_budgets(&config)
           .expect("Memory budget validation failed");
           
       // Generate budget constants
       println!("cargo:rustc-env=KILN_TOTAL_BUDGET={}", config.total_budget);
   }

Runtime Integration
~~~~~~~~~~~~~~~~~~~

Access validated budgets at runtime:

.. code-block:: rust

   use kiln_foundation::{CRATE_BUDGETS, TOTAL_MEMORY_BUDGET};
   
   fn initialize_memory_system() -> Result<(), Error> {
       // Budgets are compile-time validated constants
       let runtime_budget = CRATE_BUDGETS[CrateId::Runtime as usize];
       let foundation_budget = CRATE_BUDGETS[CrateId::Foundation as usize];
       
       // Initialize with validated budgets
       let memory_system = MemorySystem::new(TOTAL_MEMORY_BUDGET)?;
       Ok(())
   }

Error Handling
--------------

Validation Errors
~~~~~~~~~~~~~~~~~

Common validation errors and solutions:

**Budget Overflow**:

.. code-block:: text

   error: Total crate budgets (128MB) exceed system budget (64MB)
   
   Solution: Reduce individual crate budgets or increase total budget

**Insufficient Budget**:

.. code-block:: text

   error: kiln-runtime budget (2MB) below minimum requirement (4MB)
   
   Solution: Increase kiln-runtime budget to at least 4MB

**Platform Constraint Violation**:

.. code-block:: text

   error: Total budget (64MB) exceeds platform limit (32MB) for target arm-unknown-linux-gnueabihf
   
   Solution: Reduce total budget for ARM targets

Best Practices
--------------

Memory Planning
~~~~~~~~~~~~~~~

1. **Start Conservative**: Begin with default budgets and measure actual usage
2. **Profile Early**: Use memory profiling to understand actual requirements
3. **Platform-Specific Tuning**: Adjust budgets based on target platform constraints
4. **Safety Margins**: Include 20-30% safety margin for unexpected usage

Configuration Management
~~~~~~~~~~~~~~~~~~~~~~~~

1. **Version Control**: Include ``memory_budget.toml`` in version control
2. **Environment-Specific**: Use different configs for development/production
3. **Documentation**: Document rationale for specific budget choices
4. **Regular Review**: Periodically review and update budgets based on usage data

Monitoring and Debugging
------------------------

Budget Utilization Reporting
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Enable budget monitoring:

.. code-block:: rust

   #[cfg(feature = "memory-monitoring")]
   fn report_budget_usage() {
       let usage = kiln_foundation::memory_monitor::get_usage_report();
       
       for (crate_id, info) in usage.iter() {
           println!("Crate {}: {}/{} bytes ({}%)", 
               crate_id.name(),
               info.used,
               info.budget,
               (info.used * 100) / info.budget
           );
       }
   }

Debug Features
~~~~~~~~~~~~~~

Compile with debug features for detailed memory tracking:

.. code-block:: bash

   cargo build --features memory-debug,budget-tracking

This enables:

- Per-allocation tracking
- Budget violation warnings
- Memory leak detection
- Usage pattern analysis

See Also
--------

- :doc:`memory_budgets` - Detailed budget implementation
- :doc:`../memory_model` - Overall memory architecture
- :doc:`../memory_safety_comparison` - Comparison with other approaches
- :doc:`../../safety/formal_verification` - Formal verification of memory safety