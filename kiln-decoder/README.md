# WebAssembly Runtime (WRT) Decoder

A modular WebAssembly binary decoder supporting both core modules and Component Model components.

## Architecture

The decoder consists of several key components:

### 1. Utilities (`utils.rs`)

Contains shared functionality used across all parsers:
- Binary type detection (core module vs component)
- Header verification
- Name parsing
- LEB128 utilities

### 2. Core Module Parsing (`decoder_core/parse.rs`)

A streamlined parser dedicated to core WebAssembly modules:
- Parses all standard sections (types, imports, functions, etc.)
- Creates structured Module representations
- Provides focused core module functionality

### 3. Component Model Parsing (`component/parse.rs`)

A specialized parser for WebAssembly Component Model components:
- Parses component-specific sections
- Handles more complex data structures unique to the component model
- Leverages core module parsing when appropriate

### 4. Unified Streaming Interface (`parser.rs`)

A higher-level API providing a consistent streaming parser interface:
- Detects binary type automatically
- Delegates to the appropriate specialized parser
- Returns structured section payloads for both types
- Supports incremental parsing

## Usage

For general parsing, use the streaming interface:

```rust
use wrt_decoder::parser::{Parser, Payload};

// Create a parser
let parser = Parser::new(Some(wasm_binary), false);

// Process sections as they're parsed
for payload in parser {
    match payload? {
        Payload::Version(version, _) => println!("WebAssembly version: {}", version),
        Payload::TypeSection(data, size) => /* Process type section */,
        Payload::CustomSection { name, data, size } => /* Process custom section */,
        Payload::ComponentSection { data, size } => /* Process component section */,
        // ...other sections...
        Payload::End => break,
    }
}
```

For complete parsing in one step:

```rust
// For core modules:
let module = wrt_decoder::parser::parse_module(wasm_binary)?;

// For components:
let component = wrt_decoder::parser::parse_component(wasm_binary)?;
```

## Design Principles

1. **Separation of Concerns**: Core module and component parsing are kept separate
2. **Code Reuse**: Common functionality is shared via utilities
3. **Format Detection**: Binary type is automatically detected
4. **Streaming Capability**: Incremental parsing for memory efficiency
5. **Consistent Interface**: Unified API hiding implementation details 