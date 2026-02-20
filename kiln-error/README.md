# wrt-error

> Error handling foundation for WebAssembly Runtime

## Overview

Provides lightweight, no_std compatible error handling for WRT. Supports error chaining, context preservation, and specific error types for WebAssembly operations.

## Features

- **Zero dependencies** - Pure Rust error handling
- **no_std compatible** - Works in embedded environments  
- **Error chaining** - Add context with `.context()` method
- **WebAssembly specific** - Predefined error types for runtime operations
- **Formally verified** - Kani verification support

## Quick Start

```toml
[dependencies]
wrt-error = "0.1"
```

```rust
use wrt_error::{Error, WrtResult, ResultExt};

fn parse_module(bytes: &[u8]) -> WrtResult<Module> {
    validate_magic(bytes)
        .context("Invalid WebAssembly module")?;
    
    Module::from_bytes(bytes)
        .context("Failed to parse module")
}
```

## See Also

- [API Documentation](https://docs.rs/wrt-error)
- [Error Handling Guide](../docs/source/development/error_handling.rst)