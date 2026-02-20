====================
no_std Verification
====================

This comprehensive verification plan ensures that all crates in the Kiln ecosystem can be built and tested in three configurations.

.. contents:: Table of Contents
   :local:
   :depth: 2

Core Verification Framework
---------------------------

Configuration Matrix
~~~~~~~~~~~~~~~~~~~~

Each crate will be verified against the following matrix:

.. list-table:: Configuration Matrix
   :header-rows: 1
   :widths: 25 25 50

   * - Configuration
     - Features
     - Description
   * - Standard
     - ``std``
     - Full standard library support
   * - No_std with alloc
     - ``no_std, alloc``
     - Heap allocation without std
   * - No_std without alloc
     - ``no_std``
     - Bare-metal compatibility

Verification Commands
~~~~~~~~~~~~~~~~~~~~~

For each crate, run the following commands to verify compatibility:

**Standard Configuration**::

    cargo build -p <crate-name> --features std
    cargo test -p <crate-name> --features std

**No_std with Alloc**::

    cargo build -p <crate-name> --no-default-features --features "no_std,alloc"
    cargo test -p <crate-name> --no-default-features --features "no_std,alloc"

**No_std without Alloc**::

    cargo build -p <crate-name> --no-default-features --features "no_std"
    cargo test -p <crate-name> --no-default-features --features "no_std"

Crate-Specific Verification
---------------------------

kiln-math
~~~~~~~~

This crate should work in all configurations including the most restrictive no_std without alloc::

    # Standard
    cargo build -p kiln-math --features std
    cargo test -p kiln-math --features std

    # No_std with alloc
    cargo build -p kiln-math --no-default-features --features "no_std,alloc"
    cargo test -p kiln-math --no-default-features --features "no_std,alloc"

    # No_std without alloc
    cargo build -p kiln-math --no-default-features --features "no_std"
    cargo test -p kiln-math --no-default-features --features "no_std"

kiln-sync
~~~~~~~~

Verify synchronization primitives work in all environments::

    # Standard
    cargo build -p kiln-sync --features std
    cargo test -p kiln-sync --features std

    # No_std with alloc
    cargo build -p kiln-sync --no-default-features --features "no_std,alloc"
    cargo test -p kiln-sync --no-default-features --features "no_std,alloc"

    # No_std without alloc
    cargo build -p kiln-sync --no-default-features --features "no_std"
    cargo test -p kiln-sync --no-default-features --features "no_std"

kiln-error
~~~~~~~~~

::

    # Standard
    cargo build -p kiln-error --features std
    cargo test -p kiln-error --features std
    cargo test -p kiln-error --features std -- integration_test

    # No_std with alloc
    cargo build -p kiln-error --no-default-features --features "no_std,alloc"
    cargo test -p kiln-error --no-default-features --features "no_std,alloc"
    cargo test -p kiln-error --no-default-features --features "no_std,alloc" -- integration_test

    # No_std without alloc 
    cargo build -p kiln-error --no-default-features --features "no_std"
    cargo test -p kiln-error --no-default-features --features "no_std"
    cargo test -p kiln-error --no-default-features --features "no_std" -- no_std_compatibility_test

kiln-foundation
~~~~~~~~~~~~~~

::

    # Standard
    cargo build -p kiln-foundation --features std
    cargo test -p kiln-foundation --features std
    cargo test -p kiln-foundation --features std -- bounded_collections_test
    cargo test -p kiln-foundation --features std -- safe_memory_test
    cargo test -p kiln-foundation --features std -- safe_stack_test

    # No_std with alloc
    cargo build -p kiln-foundation --no-default-features --features "no_std,alloc"
    cargo test -p kiln-foundation --no-default-features --features "no_std,alloc"
    cargo test -p kiln-foundation --no-default-features --features "no_std,alloc" -- bounded_collections_test
    cargo test -p kiln-foundation --no-default-features --features "no_std,alloc" -- safe_memory_test

    # No_std without alloc
    cargo build -p kiln-foundation --no-default-features --features "no_std"
    cargo test -p kiln-foundation --no-default-features --features "no_std"
    cargo test -p kiln-foundation --no-default-features --features "no_std" -- safe_stack_test

Additional Crates
~~~~~~~~~~~~~~~~~

Similar verification procedures apply to:

- kiln-format
- kiln-decoder
- kiln-instructions
- kiln-runtime
- kiln-host
- kiln-intercept
- kiln-component
- kiln-platform
- kiln-logging
- kiln (main crate)

Integration Testing
-------------------

After verifying individual crates, run integration tests that use multiple crates together::

    # Standard
    cargo test --workspace --features std
    cargo test --features std -- integration_with_kiln

    # No_std with alloc
    cargo test --workspace --no-default-features --features "no_std,alloc"
    cargo test --no-default-features --features "no_std,alloc" -- no_std_compatibility_test

    # No_std without alloc
    cargo test --workspace --no-default-features --features "no_std"

Automated Verification Script
-----------------------------

Create a shell script to automate the verification process::

    #!/bin/bash

    # Define configurations
    CONFIGS=("std" "no_std,alloc" "no_std")
    CRATES=(
      "kiln-math"
      "kiln-sync"
      "kiln-error"
      "kiln-foundation"
      "kiln-format"
      "kiln-decoder"
      "kiln-instructions"
      "kiln-runtime"
      "kiln-host"
      "kiln-intercept"
      "kiln-component"
      "kiln-platform"
      "kiln-logging"
      "kiln"
    )

    # Run verification for each crate in each configuration
    for crate in "${CRATES[@]}"; do
      echo "=== Verifying $crate ==="
      
      for config in "${CONFIGS[@]}"; do
        echo "--- Configuration: $config ---"
        
        if [ "$config" == "std" ]; then
          echo "Building with std..."
          cargo build -p "$crate" --features std || { echo "Build failed!"; exit 1; }
          
          echo "Testing with std..."
          cargo test -p "$crate" --features std || { echo "Tests failed!"; exit 1; }
        else
          echo "Building with $config..."
          cargo build -p "$crate" --no-default-features --features "$config" || { echo "Build failed!"; exit 1; }
          
          echo "Testing with $config..."
          cargo test -p "$crate" --no-default-features --features "$config" || { echo "Tests failed!"; exit 1; }
          
          echo "Running no_std_compatibility_test..."
          cargo test -p "$crate" --no-default-features --features "$config" -- no_std_compatibility_test || echo "No specific no_std test found or failed"
        fi
      done
    done

    # Run integration tests
    echo "=== Running integration tests ==="

    for config in "${CONFIGS[@]}"; do
      echo "--- Integration tests with $config ---"
      
      if [ "$config" == "std" ]; then
        cargo test --workspace --features std || { echo "Integration tests failed!"; exit 1; }
      else
        cargo test --workspace --no-default-features --features "$config" || { echo "Integration tests failed!"; exit 1; }
      fi
    done

    echo "Verification completed successfully!"

Specific Verification Points
----------------------------

For each configuration, verify:

Memory Management
~~~~~~~~~~~~~~~~~

- Bounded collections work correctly
- Safe memory abstractions function properly
- Memory allocation strategies respect configuration limitations

Error Handling
~~~~~~~~~~~~~~

- Error propagation works without panicking
- Error contexts are created and handled properly

Resource Management
~~~~~~~~~~~~~~~~~~~

- Resource creation and destruction work as expected
- Resource tables function in all environments

Component Model
~~~~~~~~~~~~~~~

- Component instantiation works across all supported environments
- Component values are properly handled

Type Conversions
~~~~~~~~~~~~~~~~

- Type conversions between formats work correctly
- No unexpected memory allocations occur in no_alloc mode

Documentation Update
--------------------

After verification, ensure all crates have proper documentation regarding their no_std compatibility:

1. Update each crate's README.md to clearly indicate:

   - Which configurations are supported
   - Any limitations in specific configurations
   - Examples for using the crate in each configuration

2. Add specific usage examples for no_std environments in the documentation.

Additional Verification Tests
-----------------------------

Create specific tests to verify that:

1. No implicit std dependencies exist
2. No hidden allocations occur in no_alloc mode
3. All error paths work correctly in no_std environments
4. Resource cleanup works properly in constrained environments

CI Integration
--------------

Update CI workflows to include testing for all three configurations to ensure ongoing compatibility.

Runtime Performance Verification
--------------------------------

For critical components, add benchmarks that compare performance across configurations to identify any significant differences.

Verification Checklist
----------------------

.. list-table:: Verification Checklist
   :header-rows: 1
   :widths: 50 25 25

   * - Task
     - Status
     - Notes
   * - Individual crate builds (std)
     - ☐
     - All crates build with std
   * - Individual crate builds (no_std + alloc)
     - ☐
     - All crates build with alloc only
   * - Individual crate builds (no_std)
     - ☐
     - All crates build bare no_std
   * - Individual crate tests (std)
     - ☐
     - All tests pass with std
   * - Individual crate tests (no_std + alloc)
     - ☐
     - All tests pass with alloc only
   * - Individual crate tests (no_std)
     - ☐
     - All tests pass bare no_std
   * - Integration tests (all configs)
     - ☐
     - Cross-crate tests pass
   * - CI workflow updated
     - ☐
     - All configs tested in CI
   * - Documentation updated
     - ☐
     - no_std usage documented
   * - Performance verified
     - ☐
     - No regressions identified