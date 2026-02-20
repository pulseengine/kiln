# wrt-runtime

WebAssembly execution engine for WRT.

## Current Status

**Working** - Core execution engine is functional:

- Module parsing and instantiation
- Stackless instruction execution
- Memory management with bounds checking
- Table and global variable support
- Host function calls (WASI Preview 2)

## Architecture

### Core Components

- **Module** - Parsed WebAssembly module
- **ModuleInstance** - Runtime instance with memory, tables, globals
- **StacklessEngine** - Main execution engine using explicit stack frames
- **Memory** - Linear memory with bounds checking

### Execution Model

The runtime uses a stackless execution model suitable for constrained environments:

```rust
use wrt_runtime::stackless::StacklessEngine;

let engine = StacklessEngine::new();
engine.instantiate_module(&module)?;
engine.call_function("_start", &[])?;
```

## Features

- `std` - Standard library support (default)
- `alloc` - Heap allocation without full std
- `no_std` - Bare metal support with bounded collections

## no_std Support

Works in `no_std` environments using bounded collections from `wrt-foundation`. All collections have compile-time capacity limits.

## License

MIT License
