=========================
Linux Installation Guide
=========================

Kiln is a pure Rust WebAssembly runtime that supports both core WebAssembly and the Component Model. This guide covers building and using Kiln on Linux systems.

.. contents:: On this page
   :local:
   :depth: 2

System Requirements
===================

**Minimum Requirements:**

* Linux kernel 4.14 or newer
* glibc 2.17 or newer (or musl libc)
* 4GB RAM (8GB recommended for development)
* Rust 1.86.0 or newer

**Supported Architectures:**

* **x86_64** (Intel/AMD 64-bit) - Primary platform
* **aarch64** (ARM 64-bit) - Full support
* **armv7** (ARM 32-bit) - Limited support
* **riscv64** (RISC-V 64-bit) - Experimental

Prerequisites
=============

Install system dependencies based on your distribution:

**Ubuntu/Debian:**

.. code-block:: bash

   sudo apt update
   sudo apt install build-essential curl git pkg-config libssl-dev

**CentOS/RHEL/Fedora:**

.. code-block:: bash

   sudo dnf groupinstall "Development Tools"
   sudo dnf install curl git pkg-config openssl-devel

**Arch Linux:**

.. code-block:: bash

   sudo pacman -S base-devel curl git pkg-config openssl

**Alpine Linux:**

.. code-block:: bash

   sudo apk add build-base curl git pkgconfig openssl-dev

**Docker/Podman (for Dagger):**

Dagger requires a container runtime. Install one of:

.. code-block:: bash

   # Docker
   curl -fsSL https://get.docker.com | sh
   sudo usermod -aG docker $USER
   
   # Or Podman (rootless alternative)
   sudo apt install podman  # Ubuntu/Debian
   sudo dnf install podman  # Fedora/CentOS

Building from Source
====================

Install Rust
------------

.. code-block:: bash

   # Install Rust using rustup
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env

   # Verify installation
   rustc --version  # Should show 1.86.0 or newer

Install Build Tools
-------------------

.. code-block:: bash

   # Install cargo-kiln (unified build tool)
   # This will be installed from the cloned repository

   # Install Dagger (required for CI/testing tasks)
   # Option 1: Using official installer
   curl -fsSL https://dl.dagger.io/dagger/install.sh | sh
   
   # Option 2: Using package manager (if available)
   # Ubuntu/Debian (via snap)
   sudo snap install dagger
   
   # macOS/Linux via Homebrew
   brew install dagger/tap/dagger

   # Add dagger to PATH if using installer
   echo 'export PATH=$HOME/.local/bin:$PATH' >> ~/.bashrc
   source ~/.bashrc

   # Verify Dagger installation
   dagger version

   # Install cargo-component for WebAssembly components (optional)
   cargo install cargo-component --locked

   # Install additional development tools
   cargo install wasm-tools      # WASM validation and manipulation
   cargo install cargo-llvm-cov  # Code coverage
   cargo install cargo-deny      # Dependency auditing

Clone and Build Kiln
-------------------

.. code-block:: bash

   # Clone the repository
   git clone https://github.com/pulseengine/kiln.git
   cd kiln

   # Install the unified build tool
   cargo install --path cargo-kiln

   # Build all components
   cargo-kiln build

   # Or build specific targets:
   cargo-kiln kilnd      # Runtime daemon
   cargo build -p kiln  # Core library only

Running Tests
-------------

.. code-block:: bash

   # Run comprehensive test suite
   cargo-kiln test

   # Run full CI pipeline
   cargo-kiln ci

   # Run specific test (direct cargo)
   cargo test -p kiln -- test_name

   # Run tests without Dagger
   cargo test --workspace

**Additional testing commands:**

.. code-block:: bash

   # Quality assurance checks:
   cargo-kiln check          # Format, lint, and static analysis
   cargo-kiln verify --asil d  # Safety verification
   cargo-kiln coverage --html  # Test coverage analysis
   cargo-kiln kani-verify    # Formal verification
   
   # To see available commands:
   cargo-kiln --help

Using Kiln
=========

Command Line Usage (kilnd)
-------------------------

The `kilnd` daemon provides a command-line interface for running WebAssembly modules:

.. code-block:: bash

   # Show help
   ./target/debug/kilnd --help

   # Run a WebAssembly module
   ./target/debug/kilnd module.wasm

   # Run a WebAssembly component with function call
   ./target/debug/kilnd --call namespace:package/interface#function component.wasm

   # Run with fuel limit (execution steps)
   ./target/debug/kilnd --fuel 10000 module.wasm

   # Show execution statistics
   ./target/debug/kilnd --stats module.wasm

Example:

.. code-block:: bash

   # Build the example component first
   cargo-kiln kilnd
   
   # Run example with the runtime
   ./target/debug/kilnd --call example:hello/example#hello ./target/wasm32-wasip2/release/example.wasm

Library Usage
-------------

Add Kiln to your Rust project:

.. code-block:: toml

   # Cargo.toml
   [dependencies]
   kiln = "0.2.0"

Basic usage example:

.. code-block:: rust

   use kiln::prelude::*;

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Load WebAssembly bytes
       let wasm_bytes = std::fs::read("module.wasm")?;
       
       // Create module from bytes
       let module = Module::from_bytes(&wasm_bytes)?;
       
       // Create instance with imports
       let mut instance = ModuleInstance::new(module, imports)?;
       
       // Call exported function
       let result = instance.invoke("function_name", &args)?;
       
       Ok(())
   }

Development Setup
=================

VS Code Configuration
---------------------

.. code-block:: bash

   # Install VS Code (Ubuntu/Debian)
   sudo snap install code --classic

   # Install rust-analyzer extension
   code --install-extension rust-lang.rust-analyzer

Create `.vscode/settings.json`:

.. code-block:: json

   {
     "rust-analyzer.cargo.features": "all",
     "rust-analyzer.checkOnSave.command": "clippy"
   }

Development Commands
--------------------

.. code-block:: bash

   # Format code and run checks
   cargo-kiln check

   # Check formatting only
   cargo fmt --check

   # Run lints
   cargo clippy --all-features

   # Generate API documentation
   cargo doc --workspace --open

   # Generate documentation with cargo-kiln
   cargo-kiln docs --open

   # Run benchmarks
   cargo bench

   # Generate code coverage
   cargo-kiln coverage --html

   # Clean build artifacts
   cargo-kiln clean

Debugging and Profiling
=======================

Debug Builds
------------

.. code-block:: bash

   # Build with debug symbols
   cargo build

   # Run with debug logging
   RUST_LOG=debug ./target/debug/kilnd module.wasm

   # Run with GDB
   gdb ./target/debug/kilnd
   (gdb) run module.wasm

Performance Profiling
---------------------

.. code-block:: bash

   # Profile with perf
   perf record -g ./target/release/kilnd module.wasm
   perf report

   # Profile with Valgrind
   valgrind --tool=callgrind ./target/release/kilnd module.wasm

   # Analyze cache performance
   valgrind --tool=cachegrind ./target/release/kilnd module.wasm

Advanced Features
=================

no_std Support
--------------

Kiln supports `no_std` environments for embedded Linux:

.. code-block:: toml

   # Cargo.toml
   [dependencies]
   kiln = { version = "0.2.0", default-features = false }

.. code-block:: rust

   #![no_std]
   use kiln::prelude::*;

Cross Compilation
-----------------

Build for different targets:

.. code-block:: bash

   # Add target
   rustup target add aarch64-unknown-linux-gnu

   # Install cross-compilation tools
   sudo apt install gcc-aarch64-linux-gnu

   # Build for ARM64
   cargo build --target aarch64-unknown-linux-gnu

   # Or use cross tool
   cargo install cross
   cross build --target aarch64-unknown-linux-gnu

Platform-Specific Optimizations
-------------------------------

Kiln includes platform-specific optimizations for Linux:

.. code-block:: bash

   # Build with all optimizations
   cargo build --release --features "platform-linux,cfi-hardware"

   # Check available features
   cargo metadata --no-deps --format-version 1 | jq '.packages[].features'

Troubleshooting
===============

Build Issues
------------

**Rust version too old:**

.. code-block:: bash

   # Check Rust version
   rustc --version
   
   # Update Rust
   rustup update stable

**Missing dependencies:**

.. code-block:: bash

   # Ubuntu/Debian
   sudo apt install build-essential pkg-config libssl-dev

   # Fedora/CentOS
   sudo dnf groupinstall "Development Tools"
   sudo dnf install pkg-config openssl-devel

**Cargo build fails:**

.. code-block:: bash

   # Clean and rebuild
   cargo clean
   cargo build

   # Check for disk space
   df -h

**Dagger issues:**

.. code-block:: bash

   # Check if Docker is running
   docker info
   
   # Or for Podman
   podman info
   
   # Check Dagger version
   dagger version
   
   # Clear Dagger cache if needed
   rm -rf ~/.cache/dagger
   
   # Run with debug logging
   RUST_LOG=debug cargo-kiln test

Runtime Issues
--------------

**Module fails to load:**

.. code-block:: bash

   # Verify WASM file
   file module.wasm
   
   # Check with wasm-tools (install if needed)
   cargo install wasm-tools
   wasm-tools validate module.wasm

**Out of memory:**

.. code-block:: bash

   # Check available memory
   free -h
   
   # Limit Kiln memory usage
   ./target/debug/kilnd --memory-limit 100M module.wasm

**Performance issues:**

.. code-block:: bash

   # Build in release mode
   cargo build --release
   
   # Check CPU governor
   cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
   
   # Set to performance mode
   echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

Getting Help
============

Resources
---------

* **Documentation**: Full API docs at `cargo doc --open`
* **Examples**: See the `example/` directory in the repository
* **Tests**: Browse `tests/` for usage examples
* **Issues**: Report bugs at https://github.com/pulseengine/kiln/issues

Community
---------

* GitHub Discussions: https://github.com/pulseengine/kiln/discussions
* Issue Tracker: https://github.com/pulseengine/kiln/issues

Next Steps
==========

* Try the :doc:`../examples/hello_world` example
* Learn about :doc:`../architecture/component_model`
* Explore :doc:`../development/no_std_development` for embedded systems
* Read the :doc:`../architecture/platform_layer` for Linux-specific optimizations