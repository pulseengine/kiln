# `wrt-foundation`

Foundation library providing core types and memory safety primitives for the WebAssembly Runtime (WRT).

## Overview

`wrt-foundation` serves as the core foundation layer for the WRT ecosystem, providing essential types, memory safety abstractions, bounded collections, and other fundamental building blocks required by all WRT components.

This crate was previously named `wrt-foundation`. See [WRT_FOUNDATION_MIGRATION.md](../WRT_FOUNDATION_MIGRATION.md) for details on the migration.

## Features

- **Core WebAssembly Types**: Comprehensive type definitions for WebAssembly modules
- **Component Model Types**: Support for WebAssembly Component Model types
- **Safe Memory Primitives**: Memory safety mechanisms like `SafeSlice` and bounded collections
- **No-std Compatible**: Full support for environments without the standard library
- **Bounded Collections**: Fixed-capacity collections with runtime safety checks
- **Builder Patterns**: Flexible, fluent interfaces for constructing complex objects
- **Resource Management**: Type-safe resource handling primitives

## Usage Examples

### Safe Memory Operations

```rust
use wrt_foundation::{safe_memory::SafeSlice, WrtResult};

fn safe_memory_example(buffer: &[u8]) -> WrtResult<u32> {
    // Create a safe view over the buffer with bounds checking
    let safe_buffer = SafeSlice::new(buffer);
    
    // Safe read operations return results rather than panicking
    let value = safe_buffer.read_u32_le(4)?;
    
    Ok(value)
}
```

### Bounded Collections

```rust
use wrt_foundation::{BoundedVec, CapacityError};

fn bounded_vector_example() -> Result<(), CapacityError> {
    // Create a vector with a maximum capacity of 10 elements
    let mut bounded_vec = BoundedVec::<u32, 10>::new();
    
    // Safe push operations that return errors when capacity is exceeded
    for i in 0..8 {
        bounded_vec.push(i)?;
    }
    
    // Access elements safely
    let sum: u32 = bounded_vec.iter().sum();
    
    Ok(())
}
```

## Configuration

This crate provides several feature flags:

- `std`: Enables standard library support (implies `alloc`)
- `alloc`: Enables allocation support for no_std environments
- `no_std`: Explicitly enables no_std mode
- `safe-memory`: Enables enhanced memory safety features
- `platform-memory`: Enables platform-specific memory optimizations
- `component-model-*`: Various Component Model features
- `optimize`: Performance optimizations (with some safety trade-offs)
- `kani`: Support for formal verification

## License

MIT