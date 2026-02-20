# WRT - wrt-platform

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Platform Abstraction Layer (PAL) for the WRT WebAssembly Runtime.

## Overview

This crate provides traits and implementations for platform-specific functionalities required by the WRT runtime, primarily:

*   **Memory Management:** The `PageAllocator` trait abstracts the allocation, growth, and protection of memory regions suitable for WebAssembly linear memory.
*   **Synchronization:** The `FutexLike` trait abstracts low-level wait/notify operations needed for Wasm atomics.

It aims to support various target platforms (macOS, Linux, QNX, Zephyr, bare-metal) through compile-time feature flags. A fallback implementation using standard Rust features (`Vec<u8>`, `Mutex`, `Condvar`) is provided under the `std` feature (enabled by default).

## Features

*   `std`: (Default) Enables the standard library-based fallback implementations (`FallbackAllocator`, `FallbackFutex`). Required for most host environments.
*   _(Upcoming)_ `platform-macos`, `platform-linux`, `platform-qnx`, `platform-zephyr`, `platform-baremetal`: Enable platform-specific backends.
*   _(Upcoming)_ `arm-hardening`: Enables optional Armv8.x security features (PAC/BTI, MTE) where supported by the platform backend.

## Usage

Add this crate to your `Cargo.toml`:

```toml
[dependencies]
wrt-platform = { path = "path/to/wrt-platform" } # Or version = "..."
```

Use the traits via the prelude:

```rust
use wrt_platform::prelude::*;

#[cfg(feature = "std")] // Example using fallback
fn main() -> Result<(), Error> {
    let mut allocator = FallbackAllocator::new();
    let (ptr, size) = allocator.allocate(1, Some(10))?;
    println!("Allocated {} bytes at {:?}", size, ptr);
    
    // ... use memory ...

    unsafe {
        allocator.deallocate(ptr, size)?;
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    // Requires a specific platform feature to be enabled
    println!("std feature not enabled");
}
```

## Safety

This crate aims to be suitable for safety-critical environments.

*   It uses `#![forbid(unsafe_code)]` by default. Platform-specific backends requiring `unsafe` for OS interactions will contain justified `unsafe` blocks, potentially isolated in submodules.
*   It uses `panic = "abort"`.
*   It depends on `wrt-error` for standardized error handling.

Refer to the main WRT project documentation for detailed safety guidelines.

## License

Licensed under the MIT license. See the [LICENSE](LICENSE) file for details.

Copyright (c) 2025 Ralf Anton Beier 