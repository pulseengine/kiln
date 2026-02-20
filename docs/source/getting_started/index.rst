===============
Getting Started
===============

.. image:: ../_static/icons/logo.svg
   :width: 80px
   :align: right
   :alt: Kiln Logo

Welcome to PulseEngine (Kiln Edition)! This guide will help you install and set up PulseEngine on various platforms, from development environments to embedded systems.

.. contents:: On this page
   :local:
   :depth: 2

Quick Start
===========

For comprehensive installation instructions including prerequisites, platform-specific notes, and troubleshooting, see the :doc:`installation` guide.

For most development scenarios, follow these quick steps:

1. **Clone the repository**:

   .. code-block:: bash

      git clone https://github.com/pulseengine/kiln.git
      cd kiln

2. **Build and test** (requires Rust 1.86.0+):

   .. code-block:: bash

      cargo install --path cargo-kiln
      cargo-kiln build
      cargo-kiln test

Supported Platforms
===================

PulseEngine supports a wide range of platforms, from development machines to embedded systems:

.. grid:: 2

   .. grid-item-card:: Desktop Development
      :link: ../platform_guides/linux
      :link-type: doc

      * Linux (x86_64, ARM64)
      * macOS (Intel, Apple Silicon)
      * Complete toolchain and debugging support

   .. grid-item-card:: Real-Time Systems
      :link: ../platform_guides/qnx
      :link-type: doc

      * QNX Neutrino
      * Safety-critical automotive/medical
      * POSIX compliance with RT extensions

   .. grid-item-card:: Embedded RTOS
      :link: ../platform_guides/zephyr
      :link-type: doc

      * Zephyr RTOS
      * IoT and edge computing
      * Minimal resource footprint

   .. grid-item-card:: Bare Metal
      :link: ../platform_guides/bare_metal
      :link-type: doc

      * No operating system
      * Custom hardware platforms
      * Maximum control and performance

Basic Usage
===========

Once installed, you can use PulseEngine in several ways:

Command Line Interface
----------------------

Run WebAssembly modules directly:

.. code-block:: bash

   # Run a simple module
   cargo-kiln kilnd

   # Build and run example
   cargo run --bin kilnd -- module.wasm

Library Integration
-------------------

Add PulseEngine to your Rust project:

.. code-block:: toml

   [dependencies]
   kiln = { path = "kiln" }  # Adjust path or use published version

Basic runtime usage:

.. code-block:: rust

   use kiln::prelude::*;

   // Load and execute WebAssembly
   let module = Module::from_bytes(&wasm_bytes)?;
   let mut instance = ModuleInstance::new(module, imports)?;
   let result = instance.invoke("function_name", &args)?;

Component Model
---------------

Work with WebAssembly components:

.. code-block:: rust

   use kiln_component::prelude::*;

   // Load component with WIT interface
   let component = Component::from_bytes(&component_bytes)?;
   let instance = ComponentInstance::new(component, imports)?;

Next Steps
==========

.. grid:: 3

   .. grid-item-card:: 📖 Examples
      :link: ../examples/index
      :link-type: doc

      Learn through hands-on examples from Hello World to advanced component usage.

   .. grid-item-card:: 🏗️ Architecture
      :link: ../architecture/index
      :link-type: doc

      Understand Kiln's design, safety features, and performance characteristics.

   .. grid-item-card:: 🔧 Development
      :link: ../developer/index
      :link-type: doc

      Contributing guidelines, testing, and advanced development topics.

Need Help?
==========

* **Documentation**: Browse the complete documentation for detailed guides
* **Examples**: Check the ``example/`` directory for working code samples
* **Issues**: Report bugs or request features in the project repository
* **Platform Support**: Refer to platform-specific installation guides for detailed setup instructions