# wrt-debug

DWARF debug information support for WebAssembly Runtime (WRT).

## Overview

This crate provides zero-allocation DWARF debug information parsing for WebAssembly modules in no_std environments. It supports:

- **Line number information** - Map instruction addresses to source locations
- **Function information** - Extract function names, ranges, and source locations
- **Abbreviation parsing** - Support for DWARF abbreviation tables
- **Zero-copy parsing** - Works directly with module bytes without allocation
- **No-std compatible** - Designed for embedded and resource-constrained environments

## Features

- DWARF line number program parsing (.debug_line)
- Basic .debug_info section parsing
- Abbreviation table support (.debug_abbrev)
- Function discovery and range mapping
- Minimal memory footprint with bounded collections
- Streaming parser for large debug sections
- Compatible with Rust and C++ generated DWARF

## Usage

```rust
use wrt_debug::prelude::*;

// Create debug info parser
let mut debug_info = DwarfDebugInfo::new(module_bytes);

// Register debug sections
debug_info.add_section(".debug_line", line_offset, line_size);
debug_info.add_section(".debug_info", info_offset, info_size);
debug_info.add_section(".debug_abbrev", abbrev_offset, abbrev_size);

// Initialize the debug info parser
debug_info.init_info_parser()?;

// Find line info for an instruction
if let Some(line_info) = debug_info.find_line_info(pc)? {
    println!("{}:{}", line_info.file_index, line_info.line);
}

// Find function containing an address
if let Some(func_info) = debug_info.find_function_info(pc) {
    println!("Function at {:x}-{:x}", func_info.low_pc, func_info.high_pc);
}

// Get all functions
if let Some(functions) = debug_info.get_functions() {
    for func in functions {
        println!("Function: {:x}-{:x} @ {}:{}", 
            func.low_pc, func.high_pc, func.file_index, func.line);
    }
}
```

## Architecture

The crate is organized into several modules:

- `abbrev` - DWARF abbreviation table parsing
- `cursor` - Zero-copy data cursor for parsing
- `info` - .debug_info section parsing
- `line_info` - Line number program implementation
- `types` - Common DWARF types and structures

## Feature Flags

The crate supports optional features to minimize code size and memory usage:

- `line-info` (default) - Line number information support
- `debug-info` - Full debug information parsing  
- `function-info` - Function discovery and mapping
- `full-debug` - All debug features enabled

See [FEATURES.md](FEATURES.md) for detailed information about feature flags and their impact.

### Minimal Usage (No Features)

```toml
[dependencies]
wrt-debug = { version = "0.1", default-features = false }
```

### Runtime Integration

```toml
[dependencies]
wrt-runtime = { version = "0.2", features = ["debug"] }
```

## Limitations

Due to no_std/no_alloc constraints:

- String data (function names) not yet extracted
- No variable or type information
- No expression evaluation
- No call frame information
- Limited to DWARF version 2-5
- Fixed-size caches for abbreviations and functions

## License

This project is licensed under the MIT License - see the LICENSE file for details.