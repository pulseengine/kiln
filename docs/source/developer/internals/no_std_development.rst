====================
no_std Development
====================

This section documents the no_std compatibility requirements and development practices for Kiln.

.. contents:: Table of Contents
   :local:
   :depth: 2

Overview
--------

Kiln is designed to run in no_std environments, enabling deployment on embedded systems, bare-metal targets, and other resource-constrained platforms. This document outlines the development practices and verification procedures for maintaining no_std compatibility.

no_std Compatibility Status
---------------------------

Current Issues
~~~~~~~~~~~~~~

Several crates in the Kiln ecosystem have no_std compatibility issues that need to be addressed:

1. **Import Organization**:

   - Missing ``#![no_std]`` declarations in some modules
   - Incorrect import paths for core vs std items
   - Inconsistent use of alloc features

2. **Type Usage**:

   - Use of std-only types like ``HashMap`` without fallbacks
   - Missing bounds for no_std collection types
   - Incorrect feature gating for std-specific functionality

3. **Error Handling**:

   - Use of ``std::error::Error`` without proper feature gating
   - Missing no_std implementations for error types

Fixed Issues
~~~~~~~~~~~~

The following no_std compatibility fixes have been implemented:

**kiln-error**:

- Added proper ``#![no_std]`` declaration
- Fixed imports for ``core`` and ``alloc`` items
- Properly feature-gated ``std::error::Error`` implementation
- Added no_std-compatible Display implementations

**kiln-sync**:

- Implemented no_std mutex using atomic operations
- Added Once implementation for no_std
- Proper feature gating for std-specific optimizations

**kiln-foundation**:

- Fixed bounded collection implementations for no_std
- Added no_std HashMap implementation
- Proper memory management without heap allocation

Development Guidelines
----------------------

Import Organization
~~~~~~~~~~~~~~~~~~~

Follow this import pattern for no_std compatibility::

    #![cfg_attr(not(feature = "std"), no_std)]

    #[cfg(feature = "alloc")]
    extern crate alloc;

    // Core imports (always available)
    use core::{
        fmt,
        ops::{Deref, DerefMut},
        mem,
        slice,
    };

    // Alloc imports (when alloc feature is enabled)
    #[cfg(feature = "alloc")]
    use alloc::{
        vec::Vec,
        string::String,
        boxed::Box,
    };

    // Std imports (when std feature is enabled)
    #[cfg(feature = "std")]
    use std::{
        collections::HashMap,
        error::Error,
    };

Feature Flags
~~~~~~~~~~~~~

Standard feature configuration for Kiln crates::

    [features]
    default = ["std"]
    std = ["alloc"]
    alloc = []

    # Safety features (orthogonal to std/no_std)
    safety = []

Error Handling
~~~~~~~~~~~~~~

Implement errors that work in both std and no_std::

    use core::fmt;

    #[derive(Debug)]
    pub struct MyError {
        kind: ErrorKind,
        message: &'static str,
    }

    impl fmt::Display for MyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}: {}", self.kind, self.message)
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for MyError {}

Collections
~~~~~~~~~~~

Use bounded collections for no_std environments::

    use kiln_foundation::prelude::{BoundedVec, BoundedStack};

    // Instead of Vec<T>
    let mut vec: BoundedVec<u8, 256> = BoundedVec::new();

    // Instead of Vec<T> with dynamic size
    let mut stack: BoundedStack<u32, 64> = BoundedStack::new();

Memory Management
~~~~~~~~~~~~~~~~~

For no_std environments without heap allocation::

    use kiln_foundation::prelude::NoStdProvider;

    // Fixed-size memory provider
    let provider = NoStdProvider::<4096>::new();

    // Use with safe memory operations
    let handler = SafeMemoryHandler::new(provider);

Verification Process
--------------------

Build Verification
~~~~~~~~~~~~~~~~~~

Verify no_std builds with different feature combinations::

    # No features (bare no_std)
    cargo build --no-default-features

    # With alloc only
    cargo build --no-default-features --features alloc

    # With specific platform
    cargo build --no-default-features --features platform-bare

Target Testing
~~~~~~~~~~~~~~

Test on actual no_std targets::

    # Bare metal ARM
    cargo build --target thumbv7em-none-eabi --no-default-features

    # WebAssembly
    cargo build --target wasm32-unknown-unknown --no-default-features

Verification Script
~~~~~~~~~~~~~~~~~~~

Use the verification script to check all crates::

    ./scripts/verify_no_std.sh

This script:

1. Builds each crate with ``--no-default-features``
2. Checks for std dependencies
3. Validates feature flag configurations
4. Reports any compatibility issues

Common Patterns
---------------

Conditional Compilation
~~~~~~~~~~~~~~~~~~~~~~~

Use cfg attributes for platform-specific code::

    #[cfg(feature = "std")]
    pub fn with_std_only() {
        // Code that requires std
    }

    #[cfg(not(feature = "std"))]
    pub fn without_std() {
        // Alternative implementation
    }

Type Aliases
~~~~~~~~~~~~

Provide compatible types for different environments::

    #[cfg(feature = "std")]
    pub type HashMap<K, V> = std::collections::HashMap<K, V>;

    #[cfg(not(feature = "std"))]
    pub type HashMap<K, V> = kiln_foundation::no_std_hashmap::NoStdHashMap<K, V>;

Platform Abstraction
~~~~~~~~~~~~~~~~~~~~

Use the platform layer for OS-specific operations::

    use kiln_platform::traits::{PageAllocator, FutexLike};

    #[cfg(feature = "platform-bare")]
    use kiln_platform::bare::{BareAllocator, BareFutex};

    #[cfg(feature = "platform-linux")]
    use kiln_platform::linux::{LinuxAllocator, LinuxFutex};

Testing Strategy
----------------

Feature Matrix Testing
~~~~~~~~~~~~~~~~~~~~~~

Test all feature combinations in CI::

    matrix:
      features:
        - ""                    # no_std bare
        - "alloc"              # no_std + alloc
        - "std"                # std (default)
        - "safety"             # safety features
        - "alloc,safety"       # combined features

Platform-Specific Tests
~~~~~~~~~~~~~~~~~~~~~~~

Include platform-specific test modules::

    #[cfg(all(test, not(feature = "std")))]
    mod no_std_tests {
        use super::*;
        
        #[test]
        fn test_no_heap_allocation() {
            // Test that operations work without heap
        }
    }

    #[cfg(all(test, feature = "std"))]
    mod std_tests {
        use super::*;
        
        #[test]
        fn test_with_std_features() {
            // Test std-specific functionality
        }
    }

Best Practices
--------------

1. **Always test with ``--no-default-features``** to catch std dependencies
2. **Use ``core`` types** instead of ``std`` types where possible
3. **Feature-gate std-only functionality** properly
4. **Provide no_std alternatives** for critical functionality
5. **Document feature requirements** in API documentation
6. **Minimize alloc usage** for better embedded support
7. **Use const generics** for compile-time sizing

Troubleshooting
---------------

Common Issues
~~~~~~~~~~~~~

**"can't find crate for 'std'"**:

- Add ``#![no_std]`` to the crate root
- Check all imports use ``core::`` instead of ``std::``
- Ensure dependencies support no_std

**"unresolved import 'alloc'"**:

- Add ``extern crate alloc;`` when using alloc features
- Ensure the alloc feature is properly defined
- Check that alloc imports are feature-gated

**Type mismatch errors**:

- Verify bounded types have correct const generic parameters
- Check that size calculations don't overflow
- Ensure proper type conversions for platform differences

Future Improvements
-------------------

1. **Automated no_std verification** in CI for all PRs
2. **Benchmarks** comparing std vs no_std performance
3. **Size optimization** for embedded deployments
4. **Custom allocator support** for specialized environments
5. **Formal verification** of no_std safety properties