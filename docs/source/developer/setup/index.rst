=======================
Development Environment
=======================

This guide covers setting up a complete development environment for contributing to Kiln.

.. toctree::
   :maxdepth: 2

   environment_setup
   toolchain_requirements
   ide_setup
   debugging_setup

Prerequisites
=============

For basic prerequisites and installation instructions, see the :doc:`/getting_started/installation` guide.

Additional Development Requirements
-----------------------------------

* **Docker or Podman** (for Dagger CI)
* **4GB RAM minimum** (8GB recommended for development)
* **Python 3.8+** (for documentation generation)

Platform Support
-----------------

* **Linux**: Primary development platform
* **macOS**: Full support with native tools
* **Windows**: WSL2 recommended

Development Tools
=================

Beyond the basic installation requirements, developers need additional tools:

.. code-block:: bash

   # Install Dagger (CI tool)
   curl -fsSL https://dl.dagger.io/dagger/install.sh | sh

   # Code quality tools
   cargo install cargo-llvm-cov
   cargo install cargo-deny

   # Documentation dependencies
   pip install -r docs/requirements.txt

   # Optional: Additional development utilities
   cargo install cargo-watch  # For automatic rebuilds
   cargo install cargo-expand # For macro expansion debugging

Quick Setup
===========

.. code-block:: bash

   # Clone repository
   git clone https://github.com/pulseengine/kiln.git
   cd kiln

   # Install unified build tool
   cargo install --path cargo-kiln

   # Setup development environment and tools
   cargo-kiln setup --check       # Check tool dependencies
   cargo-kiln setup --all         # Install tools and setup git hooks

   # Verify tool versions (optional)
   cargo-kiln tool-versions check --verbose

   # Verify build
   cargo-kiln build

   # Run tests
   cargo-kiln test

   # Format code and run checks
   cargo-kiln check

Tool Version Management
=======================

Kiln uses a sophisticated tool version management system to ensure reproducible development environments:

.. code-block:: bash

   # Check all tool versions against requirements
   cargo-kiln tool-versions check

   # Show detailed version information
   cargo-kiln tool-versions check --verbose

   # Check specific tool
   cargo-kiln tool-versions check --tool kani

   # Generate tool-versions.toml configuration file
   cargo-kiln tool-versions generate

The ``tool-versions.toml`` file in the workspace root specifies:

* **Exact version requirements** (e.g., kani must be exactly 0.63.0)
* **Minimum version requirements** (e.g., git must be at least 2.30.0) 
* **Installation commands** for each tool
* **Command mapping** - which cargo-kiln commands require each tool

This ensures all contributors use compatible tool versions for consistent builds and testing.

Next Steps
==========

* See :doc:`ide_setup` for editor configuration
* See :doc:`../contributing/index` for contribution guidelines
* See :doc:`../build_system/index` for build system details