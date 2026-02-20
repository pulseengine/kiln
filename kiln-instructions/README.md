# wrt-instructions

> WebAssembly instruction encoding, decoding, and execution

## Overview

Provides comprehensive support for WebAssembly instructions including encoding, decoding, validation, and execution semantics. Supports both Core WebAssembly and SIMD instructions.

## Features

- **Complete instruction set** - All WebAssembly Core and SIMD instructions
- **Encoding/decoding** - Binary format support
- **Validation** - Instruction validation and type checking
- **Execution traits** - Abstract execution interfaces
- **no_std support** - Works in embedded environments

## Quick Start

```toml
[dependencies]
wrt-instructions = "0.1"
```

```rust
use wrt_instructions::{Instruction, InstructionDecoder};

// Decode instruction from bytes
let decoder = InstructionDecoder::new(bytes);
let instruction = decoder.next_instruction()?;

match instruction {
    Instruction::I32Add => {
        // Handle i32.add instruction
    }
    Instruction::LocalGet(index) => {
        // Handle local.get instruction  
    }
    // ... other instructions
}
```

## See Also

- [API Documentation](https://docs.rs/wrt-instructions)
- [WebAssembly Instruction Reference](https://webassembly.github.io/spec/core/syntax/instructions.html)