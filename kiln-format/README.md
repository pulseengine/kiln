# wrt-format

WebAssembly format handling for the WRT runtime.

This crate provides utilities for handling WebAssembly formats, including serialization and deserialization of modules and state, binary format parsing, and compression.

## Features

- **Module handling**: Parse and manipulate WebAssembly modules
- **Section management**: Work with individual WebAssembly sections
- **Binary format**: Low-level binary format utilities
- **State serialization**: Save and load WebAssembly runtime state
- **Compression**: Efficient compression of WebAssembly modules and state
- **Version management**: Handle different versions of the WebAssembly specification
- **`no_std` support**: Compatible with bare-metal environments
- **Formally verified**: Critical components are verified using the Kani verifier

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
wrt-format = { version = "0.2.0" }
```

For no_std environments:

```toml
[dependencies]
wrt-format = { version = "0.2.0", default-features = false, features = ["no_std"] }
```

### Basic example

```rust
use wrt_format::{module::Module, binary::Binary};

// Parse a WebAssembly module from binary
fn parse_module(wasm_bytes: &[u8]) -> Result<Module, wrt_error::Error> {
    let binary = Binary::from_bytes(wasm_bytes)?;
    Module::from_binary(binary)
}

// Use the module
fn use_module() -> Result<(), wrt_error::Error> {
    let wasm_bytes = include_bytes!("path/to/module.wasm");
    let module = parse_module(wasm_bytes)?;
    
    // Work with the module...
    
    Ok(())
}
```

## Features

- `std`: Enables integration with the standard library (default)
- `no_std`: Enables compatibility with no_std environments
- `kani`: Enables formal verification with the Kani verifier

## No Std Support

The `wrt-format` crate is fully compatible with `no_std` environments. When the `no_std` feature is enabled:

- The crate doesn't rely on the Rust standard library
- It uses `alloc` for dynamic memory management (Vec, String, etc.)
- HashMap/HashSet are replaced with BTreeMap/BTreeSet
- Core functionality works exactly the same as with the `std` feature

This allows you to use the crate in environments like embedded systems, WebAssembly, or other platforms where the standard library is not available.

## Formal Verification

The `wrt-format` crate includes formal verification using the [Kani Verifier](https://github.com/model-checking/kani) for critical components.

To run the verification:

```bash
# Install Kani
cargo install --locked kani-verifier

# Run verification on all proofs
cd wrt-format
cargo kani --features kani

# Run verification on a specific proof
cargo kani --features kani --harness verify_binary_format
```

Verified properties include:
- Binary format parsing correctness
- Module serialization and deserialization
- State consistency
- Section handling

## License

MIT 