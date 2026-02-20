============
Installation
============

This page provides installation instructions for PulseEngine (Kiln Edition) development environment.

.. warning::
   **Development Status**: PulseEngine provides WebAssembly infrastructure and tooling, but the core execution engine is under development. 
   Installation allows building modules and exploring the intended API design.

.. warning::
   **Source-Only Installation**: PulseEngine is currently available only as source code. 
   Pre-built binaries and package manager distributions are not yet available.

.. contents:: On this page
   :local:
   :depth: 2

System Requirements
===================

Hardware Requirements
---------------------

**Minimum:**

* 64 MB RAM (for basic embedded usage)
* 10 MB storage for runtime
* Any 32-bit or 64-bit processor with WebAssembly support

**Recommended for Development:**

* 4 GB RAM
* 1 GB storage for full toolchain
* x86_64 or ARM64 processor

Software Dependencies
=====================

Core Dependencies
-----------------

All platforms require:

1. **Rust Toolchain**: Version 1.86.0 or newer (stable)
2. **Git**: Source code management

The unified build tool (cargo-kiln) is included in the repository and installed automatically.

Install Rust
~~~~~~~~~~~~~

.. tabs::

   .. tab:: Linux/macOS

      .. code-block:: bash

         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
         source ~/.cargo/env

   .. tab:: Windows

      Download and run the installer from `rustup.rs <https://rustup.rs/>`_

   .. tab:: Manual

      Download from the `official Rust website <https://forge.rust-lang.org/infra/channel-layout.html#archives>`_

Install cargo-kiln
~~~~~~~~~~~~~~~~~~

The cargo-kiln unified build tool is installed from the repository:

.. code-block:: bash

   # After cloning the repository
   cargo install --path cargo-kiln

Verify installation:

.. code-block:: bash

   cargo-kiln --help
   rustc --version

Development Tool Setup
~~~~~~~~~~~~~~~~~~~~~~

After installing cargo-kiln, set up your development environment:

.. code-block:: bash

   # Check all tool dependencies
   cargo-kiln setup --check

   # Install optional development tools (kani, cargo-fuzz, etc.)
   cargo-kiln setup --install

   # Complete setup (tools + git hooks)
   cargo-kiln setup --all

   # Verify tool versions against requirements
   cargo-kiln tool-versions check --verbose

The build system includes sophisticated tool version management:

- **tool-versions.toml**: Configuration file specifying exact tool version requirements
- **Automated tool detection**: Missing tools trigger helpful installation messages  
- **Reproducible environments**: Consistent tool versions across all contributors

Optional development tools include:

- **kani**: Formal verification tool for safety-critical code
- **cargo-fuzz**: Fuzzing framework for security testing
- **llvm-tools**: Coverage analysis and profiling
- **mdbook**: Documentation generation

WebAssembly Targets
~~~~~~~~~~~~~~~~~~~

Kiln requires WebAssembly compilation targets:

.. code-block:: bash

   rustup target add wasm32-unknown-unknown
   rustup target add wasm32-wasip1
   rustup target add wasm32-wasip2

Additional Development Tools
----------------------------

For full development workflow:

.. code-block:: bash

   # Component tooling
   cargo install cargo-component

   # WebAssembly tools
   cargo install wasm-tools
   
   # PulseEngine command-line interface (from source)
   cargo install --path kilnd

   # Code coverage (optional)
   cargo install cargo-llvm-cov

Installation Methods
====================

Source Installation
-------------------

**Recommended for most users**

1. Clone the repository:

   .. code-block:: bash

      git clone https://github.com/pulseengine/kiln
      cd kiln

2. Install and build:

   .. code-block:: bash

      cargo install --path cargo-kiln
      cargo-kiln build

3. Run tests to verify:

   .. code-block:: bash

      cargo-kiln test

4. (Optional) Install system-wide:

   .. code-block:: bash

      cargo install --path kilnd

Binary Installation
-------------------

.. warning::
   **Not Available**: Pre-built binaries are not currently available. 
   Please use source installation method above.

Package Manager Installation
----------------------------

.. warning::
   **Not Available**: PulseEngine is not currently published to package managers including:
   
   - crates.io (Cargo)
   - Homebrew
   - APT repositories
   - Other package managers
   
   Please use source installation method above.

Configuration
=============

Environment Variables (Planned)
--------------------------------

The following environment variables are designed for the target runtime configuration:

.. code-block:: bash

   # Target runtime configuration (execution engine under development)
   export KILN_STACK_SIZE=1048576    # Stack size for PulseEngine runtime
   export KILN_FUEL_LIMIT=1000000    # Fuel limit for PulseEngine execution

   # Development options
   export KILN_LOG_LEVEL=info
   export KILN_DEBUG_MODE=1

Build Configuration (Planned)
------------------------------

The planned configuration system will use a ``.kiln/config.toml`` file:

.. code-block:: toml

   # Target configuration format (under development)
   [runtime]
   stack_size = 1048576
   fuel_limit = 1000000
   
   [security]
   enable_cfi = true
   sandbox_memory = true
   
   [performance]
   optimize_for_size = false
   enable_simd = true

Verification
============

Verify your development environment works correctly:

.. code-block:: bash

   # Check that kilnd builds (infrastructure verification)
   cargo run --bin kilnd -- --help

   # Build all crates to verify dependencies
   cargo-kiln build

   # Run infrastructure tests
   cargo-kiln test

.. note::
   **Development Status**: The kilnd tool currently provides infrastructure and module validation. 
   Full WebAssembly execution is under development. Expected output shows successful build and infrastructure validation.

Troubleshooting
===============

Common Issues
-------------

**Rust version mismatch:**

.. code-block:: bash

   rustup update stable
   rustup default stable

**Missing WebAssembly targets:**

.. code-block:: bash

   rustup target add wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2

**Build failures:**

.. code-block:: bash

   cargo-kiln clean
   cargo-kiln build

**Permission errors:**

.. code-block:: bash

   # Use cargo install without sudo
   cargo install --path kilnd

Platform-Specific Notes
=======================

For detailed platform-specific instructions, see:

* :doc:`linux` - Linux distributions
* :doc:`macos` - macOS and Apple Silicon
* :doc:`qnx` - QNX Neutrino real-time systems
* :doc:`zephyr` - Zephyr RTOS embedded systems
* :doc:`bare_metal` - Bare-metal and custom hardware

Next Steps
==========

After installation:

1. Try the :doc:`../examples/hello_world` example
2. Read the :doc:`../architecture/index` overview
3. Explore :doc:`../examples/index` for your use case