# wrt-host

Host function infrastructure for the WebAssembly Runtime (WRT).

This crate provides the core infrastructure for registering and managing host functions that can be called from WebAssembly components. It follows the Component Model specification for host functions and the Canonical ABI.

## Features

- `std` (default): Enables standard library features
- `alloc`: Enables allocation features for no_std environments with an allocator
- `no_std`: Builds without the standard library for embedded environments
- `kani`: Enables formal verification using Kani

## Usage

```rust
use wrt_host::{CallbackRegistry, HostFunctionHandler, CloneableFn};

// Create a callback registry
let mut registry = CallbackRegistry::new();

// Register a host function
registry.register_host_function(
    "module_name", 
    "function_name", 
    CloneableFn::new(|args| {
        // Function implementation
        Ok(vec![])
    })
);
```

## Component Model Integration

This crate implements the host function mechanism described in the [WebAssembly Component Model](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md), providing a way for WebAssembly components to interact with host capabilities.

## no_std Support

This crate supports no_std environments with the `no_std` feature. When used without `std`,
you must enable the `alloc` feature and provide an allocator.

## Verification

This crate supports formal verification using [Kani](https://github.com/model-checking/kani).
Enable the `kani` feature to include verification harnesses. 