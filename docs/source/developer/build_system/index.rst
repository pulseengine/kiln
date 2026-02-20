============
Build System
============

This section documents the Kiln unified build system powered by cargo-kiln.

.. contents:: Table of Contents
   :local:
   :depth: 2

Overview
--------

Kiln uses a modern, unified build system that consolidates all build operations:

1. **cargo-kiln**: Unified build tool providing all development commands
2. **kiln-build-core**: Core library implementing build system functionality
3. **Cargo**: Standard Rust build tool for basic compilation

The new architecture replaces the previous fragmented approach (justfile, xtask, shell scripts) with a single, AI-friendly tool.

Installation
------------

Install the unified build tool::

    cargo install --path cargo-kiln

Verify installation::

    cargo-kiln --help

Core Commands
-------------

All development tasks are available through cargo-kiln:

Available Commands
~~~~~~~~~~~~~~~~~~

Run ``cargo-kiln --help`` to see available commands::

    cargo-kiln --help

Essential commands include:

**Build Operations**:

- ``cargo-kiln build``: Build all Kiln components
- ``cargo-kiln clean``: Clean build artifacts
- ``cargo-kiln kilnd``: Build KILND daemon binaries

**Testing**:

- ``cargo-kiln test``: Run comprehensive test suite
- ``cargo-kiln test --filter pattern``: Run filtered tests
- ``cargo-kiln coverage --html``: Generate test coverage reports

**Quality Assurance**:

- ``cargo-kiln check``: Run static analysis and formatting
- ``cargo-kiln check --strict``: Run strict linting and checks
- ``cargo-kiln ci``: Run full CI pipeline locally

**Safety Verification**:

- ``cargo-kiln verify --asil d``: Run ASIL-D safety verification
- ``cargo-kiln kani-verify --asil-profile d``: Formal verification with KANI
- ``cargo-kiln verify-matrix --report``: Comprehensive build matrix verification

**Documentation**:

- ``cargo-kiln docs``: Generate documentation
- ``cargo-kiln docs --open``: Generate and open documentation
- ``cargo-kiln docs --private``: Include private items

**Development Utilities**:

- ``cargo-kiln no-std``: Verify no_std compatibility
- ``cargo-kiln simulate-ci``: Simulate CI workflow locally

**Tool Management**:

- ``cargo-kiln setup --check``: Check all tool dependencies
- ``cargo-kiln setup --install``: Install optional development tools
- ``cargo-kiln setup --all``: Complete environment setup
- ``cargo-kiln tool-versions check``: Verify tool versions against requirements
- ``cargo-kiln tool-versions generate``: Generate tool version configuration

Architecture
------------

The build system is built on three key components:

cargo-kiln CLI
~~~~~~~~~~~~~

The main command-line interface that provides:

- Unified command structure
- Consistent argument handling
- Progress reporting and logging
- Error handling and diagnostics

kiln-build-core Library
~~~~~~~~~~~~~~~~~~~~~~

The core build system library that implements:

- Workspace management
- Build orchestration
- Test execution
- Safety verification
- Documentation generation
- Coverage analysis

Build Operations
~~~~~~~~~~~~~~~~

All build operations follow a consistent pattern:

1. **Initialization**: Detect workspace and load configuration
2. **Validation**: Check prerequisites and dependencies
3. **Execution**: Run the requested operation with progress reporting
4. **Verification**: Validate results and generate reports
5. **Cleanup**: Clean up temporary files and resources

Configuration
-------------

The build system uses multiple configuration sources:

**Cargo.toml**:
  Workspace configuration, dependencies, and build profiles

**ASIL Levels**:
  Safety verification profiles (QM, A, B, C, D)

**Environment Variables**:
  CI detection, custom paths, and feature flags

**tool-versions.toml**:
  Tool version requirements and installation commands for reproducible development environments

Common Workflows
----------------

**Development Workflow**::

    # Start development
    cargo-kiln build
    cargo-kiln test
    
    # Make changes...
    
    # Verify changes
    cargo-kiln check
    cargo-kiln test --filter new_feature
    
    # Before commit
    cargo-kiln ci

**Safety-Critical Development**::

    # ASIL-D verification
    cargo-kiln verify --asil d
    cargo-kiln kani-verify --asil-profile d
    cargo-kiln verify-matrix --report
    
    # Generate compliance reports
    cargo-kiln simulate-ci --verbose

**Documentation Workflow**::

    # Generate and preview docs
    cargo-kiln docs --open
    
    # Verify documentation
    cargo-kiln docs --private
    cargo-kiln verify --detailed

Migration from Legacy System
-----------------------------

If you're migrating from the legacy build system:

**Command Mapping**:

.. list-table:: Legacy to cargo-kiln Command Mapping
   :widths: 40 40 20
   :header-rows: 1

   * - Legacy Command
     - New Command
     - Notes
   * - ``just build``
     - ``cargo-kiln build``
     - Direct replacement
   * - ``just ci-test``
     - ``cargo-kiln test``
     - Enhanced test reporting
   * - ``just ci-main``
     - ``cargo-kiln ci``
     - Comprehensive CI checks
   * - ``cargo xtask coverage``
     - ``cargo-kiln coverage --html``
     - Improved coverage reporting
   * - ``./scripts/kani-verify.sh``
     - ``cargo-kiln kani-verify``
     - Rust-based implementation
   * - ``just verify-build-matrix``
     - ``cargo-kiln verify-matrix --report``
     - Enhanced reporting

**Benefits of Migration**:

- Unified command interface
- Better error messages and diagnostics
- Consistent progress reporting
- AI-friendly architecture
- Cross-platform compatibility
- Integrated safety verification

Troubleshooting
---------------

**Common Issues**:

**Build Failures**:
  Run ``cargo-kiln build --verbose`` for detailed output

**Test Failures**:
  Use ``cargo-kiln test --nocapture`` to see test output

**Verification Issues**:
  Run ``cargo-kiln simulate-ci`` to reproduce CI environment locally

**Getting Help**:
  Use ``cargo-kiln <command> --help`` for command-specific help

For more detailed troubleshooting, see the :doc:`../troubleshooting/index` section.