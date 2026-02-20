========================
macOS Installation Guide
========================

Kiln provides native support for macOS, optimized for both Intel and Apple Silicon Macs.

.. contents:: On this page
   :local:
   :depth: 2

Supported Versions
==================

**Fully Supported:**

* macOS 12 (Monterey) and later
* macOS 11 (Big Sur) with Xcode 13+
* Both Intel (x86_64) and Apple Silicon (arm64)

**Minimum Requirements:**

* macOS 10.15 (Catalina) - Limited support
* Xcode Command Line Tools
* 4 GB RAM (8 GB recommended)
* 2 GB free disk space

Installation Methods
====================

Homebrew Installation
---------------------

**Recommended for most users**

.. code-block:: bash

   # Install Homebrew (if not already installed)
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

   # Add Kiln tap
   brew tap your-org/kiln

   # Install Kiln
   brew install kiln

**Update to latest version:**

.. code-block:: bash

   brew update
   brew upgrade kiln

MacPorts Installation
---------------------

.. code-block:: bash

   # Install MacPorts (if not already installed)
   # Download from https://www.macports.org/install.php

   # Install Kiln
   sudo port install kiln

Source Installation
-------------------

**Prerequisites:**

.. code-block:: bash

   # Install Xcode Command Line Tools
   xcode-select --install

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env

   # Install just
   cargo install just

**Build from source:**

.. code-block:: bash

   git clone https://github.com/pulseengine/kiln.git
   cd kiln
   cargo-kiln build

Apple Silicon Considerations
============================

Native Apple Silicon Support
-----------------------------

Kiln has full native support for Apple Silicon (M1, M2, M3, M4):

.. code-block:: bash

   # Verify native architecture
   uname -m  # Should show "arm64"

   # Check Rust target
   rustc --print target-list | grep aarch64-apple-darwin

**Performance optimizations:**

.. code-block:: bash

   # Build with Apple Silicon optimizations
   export RUSTFLAGS="-C target-cpu=native"
   cargo-kiln build

Rosetta 2 Compatibility
-----------------------

If using Intel binaries on Apple Silicon:

.. code-block:: bash

   # Install Rosetta 2
   sudo softwareupdate --install-rosetta

   # Force Intel mode (if needed)
   arch -x86_64 zsh
   cargo build --target x86_64-apple-darwin

Development Environment
=======================

Xcode Integration
-----------------

**Install Xcode (optional but recommended):**

* Download from Mac App Store
* Or install Command Line Tools only: ``xcode-select --install``

**VS Code setup:**

.. code-block:: bash

   # Install VS Code
   brew install --cask visual-studio-code

   # Install Rust extensions
   code --install-extension rust-lang.rust-analyzer
   code --install-extension vadimcn.vscode-lldb

**Rust debugging with LLDB:**

.. code-block:: bash

   # Install CodeLLDB extension for debugging
   code --install-extension vadimcn.vscode-lldb

Create `.vscode/launch.json`:

.. code-block:: json

   {
     "version": "0.2.0",
     "configurations": [
       {
         "type": "lldb",
         "request": "launch",
         "name": "Debug Kiln",
         "cargo": {
           "args": ["build", "--bin=kilnd"],
           "filter": {
             "name": "kilnd",
             "kind": "bin"
           }
         },
         "args": ["example.wasm"],
         "cwd": "${workspaceFolder}"
       }
     ]
   }

Performance Optimization
========================

CPU Features
------------

**Check available CPU features:**

.. code-block:: bash

   # Apple Silicon features
   sysctl -a | grep machdep.cpu

   # Intel features
   sysctl -a | grep machdep.cpu.features

**Build optimizations:**

.. code-block:: bash

   # Apple Silicon optimized build
   export RUSTFLAGS="-C target-cpu=apple-m1"  # or apple-m2, apple-m3

   # Intel optimized build
   export RUSTFLAGS="-C target-cpu=native"

   cargo-kiln build

Memory Management
-----------------

**Configure memory limits:**

.. code-block:: bash

   # Check memory pressure
   memory_pressure

   # Increase stack size if needed
   export KILN_STACK_SIZE=2097152  # 2MB

   # Monitor memory usage
   top -pid $(pgrep kilnd)

Security Features
=================

macOS Security Integration
--------------------------

**Gatekeeper and code signing:**

For distribution, sign your Kiln binaries:

.. code-block:: bash

   # Sign binary (requires Apple Developer account)
   codesign --force --sign "Developer ID Application: Your Name" target/release/kilnd

   # Verify signature
   codesign --verify --verbose target/release/kilnd

**Hardened Runtime:**

.. code-block:: bash

   # Enable hardened runtime
   codesign --force --options runtime --sign "Developer ID Application: Your Name" target/release/kilnd

**App Sandbox (for Mac App Store):**

Add entitlements file for sandboxed applications.

System Integration
==================

LaunchDaemon Configuration
--------------------------

Create `/Library/LaunchDaemons/com.yourorg.kiln.plist`:

.. code-block:: xml

   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.yourorg.kiln</string>
       <key>ProgramArguments</key>
       <array>
           <string>/usr/local/bin/kilnd</string>
           <string>--config</string>
           <string>/etc/kiln/config.toml</string>
       </array>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
   </dict>
   </plist>

Load the service:

.. code-block:: bash

   sudo launchctl load /Library/LaunchDaemons/com.yourorg.kiln.plist
   sudo launchctl start com.yourorg.kiln

Environment Configuration
=========================

Shell Setup
-----------

**For zsh (default on macOS 10.15+):**

Add to `~/.zshrc`:

.. code-block:: bash

   # Kiln environment
   export PATH="$HOME/.cargo/bin:$PATH"
   export KILN_LOG_LEVEL=info

   # Apple Silicon optimizations
   if [[ $(uname -m) == "arm64" ]]; then
       export RUSTFLAGS="-C target-cpu=apple-m1"
   fi

**For bash:**

Add to `~/.bash_profile`:

.. code-block:: bash

   # Kiln environment
   export PATH="$HOME/.cargo/bin:$PATH"
   source ~/.cargo/env

macOS-Specific Features
======================

Metal Performance Shaders
--------------------------

Kiln can leverage Metal for GPU acceleration:

.. code-block:: rust

   // Enable Metal features in your Kiln configuration
   [features]
   metal-acceleration = true

Framework Integration
--------------------

**Objective-C bindings:**

.. code-block:: rust

   // Link with Foundation framework
   #[link(name = "Foundation", kind = "framework")]
   extern "C" {}

**Swift integration:**

Create Swift package with Kiln:

.. code-block:: swift

   import KilnRuntime

   let runtime = KilnRuntime()
   let result = runtime.execute(wasmModule: moduleData)

Testing and Validation
======================

**Run macOS-specific tests:**

.. code-block:: bash

   # Test Apple Silicon build
   cargo test --target aarch64-apple-darwin

   # Test Intel build  
   cargo test --target x86_64-apple-darwin

   # Run comprehensive test suite
   cargo-kiln ci

**Performance benchmarking:**

.. code-block:: bash

   # Run benchmarks
   cargo bench

   # Profile with Instruments
   xcrun xctrace record --template "Time Profiler" --launch -- target/release/kilnd example.wasm

Troubleshooting
===============

Common Issues
-------------

**Xcode Command Line Tools missing:**

.. code-block:: bash

   xcode-select --install

**Library linking errors:**

.. code-block:: bash

   # Update Xcode
   sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer

   # Clear derived data
   rm -rf ~/Library/Developer/Xcode/DerivedData

**Homebrew PATH issues:**

.. code-block:: bash

   # Add Homebrew to PATH
   echo 'export PATH="/opt/homebrew/bin:$PATH"' >> ~/.zshrc
   source ~/.zshrc

**Apple Silicon compatibility:**

.. code-block:: bash

   # Check if running under Rosetta
   sysctl -n sysctl.proc_translated

   # Force native mode
   arch -arm64 zsh

**Permission issues:**

.. code-block:: bash

   # Fix Homebrew permissions
   sudo chown -R $(whoami) /opt/homebrew/

   # Reset security settings
   sudo spctl --master-disable

Performance Issues
------------------

**Memory pressure:**

.. code-block:: bash

   # Check memory pressure
   memory_pressure

   # Close unnecessary applications
   # Increase swap if needed (not recommended)

**Thermal throttling:**

.. code-block:: bash

   # Monitor CPU temperature
   sudo powermetrics -n 1 | grep -i temp

   # Check for thermal throttling
   pmset -g thermstate

Distribution
============

App Store Distribution
---------------------

For Mac App Store distribution:

1. Enable App Sandbox
2. Add required entitlements
3. Use Xcode for submission

Notarization
------------

For distribution outside App Store:

.. code-block:: bash

   # Create zip for notarization
   zip -r kiln.zip kilnd

   # Submit for notarization
   xcrun notarytool submit kiln.zip --keychain-profile "AC_PASSWORD"

   # Staple ticket
   xcrun stapler staple kilnd

Next Steps
==========

* Try the :doc:`../examples/hello_world` example
* Explore :doc:`../examples/platform/macos_features`
* Review :doc:`../architecture/platform_layer` for technical details