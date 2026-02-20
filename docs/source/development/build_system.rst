============
Build System
============

.. note::
   This documentation has been moved and updated. Please see the current build system documentation at :doc:`../developer/build_system/index`.

The Kiln build system has been completely redesigned around the unified cargo-kiln tool.

Quick Reference
---------------

**New Unified Commands:**

.. code-block:: bash

   # Install the build tool
   cargo install --path cargo-kiln

   # Core development commands
   cargo-kiln build        # Build all components
   cargo-kiln test         # Run tests
   cargo-kiln check        # Static analysis and formatting
   cargo-kiln ci           # Full CI pipeline

   # Safety verification
   cargo-kiln verify --asil d           # ASIL-D verification
   cargo-kiln kani-verify --asil-profile d  # Formal verification
   cargo-kiln verify-matrix --report    # Comprehensive verification

   # Documentation and coverage
   cargo-kiln docs --open               # Generate and open docs
   cargo-kiln coverage --html           # Coverage analysis

**Migration from Legacy Commands:**

.. list-table:: Legacy to cargo-kiln Command Mapping
   :widths: 40 40 20
   :header-rows: 1

   * - Legacy Command
     - New Command
     - Status
   * - ``just build``
     - ``cargo-kiln build``
     - ✅ Available
   * - ``just ci-test``
     - ``cargo-kiln test``
     - ✅ Available
   * - ``just ci-main``
     - ``cargo-kiln ci``
     - ✅ Available
   * - ``cargo xtask coverage``
     - ``cargo-kiln coverage --html``
     - ✅ Available
   * - ``./scripts/kani-verify.sh``
     - ``cargo-kiln kani-verify``
     - ✅ Available
   * - ``just verify-build-matrix``
     - ``cargo-kiln verify-matrix --report``
     - ✅ Available

For complete documentation, see :doc:`../developer/build_system/index`.