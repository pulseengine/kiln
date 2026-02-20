# wrtd - WebAssembly Runtime Daemon

Command-line runtime for executing WebAssembly components with WASI Preview 2 support.

## Current Status

**Early Development** - Basic component execution works:

```bash
wrtd hello_rust.wasm --component
# Output: Hello wasm component world from Rust!
```

### Working Features

- WebAssembly Component Model execution
- WASI Preview 2 stdout/stderr (`wasi:cli/stdout`, `wasi:io/streams`)
- Resource limits (fuel, memory)
- Memory profiling

### In Progress

- Additional WASI Preview 2 interfaces
- Filesystem access
- Environment variables and arguments

## Installation

```bash
cargo build --bin wrtd --features "std,wrt-execution" --release
```

## Usage

```bash
# Run a WebAssembly component
wrtd component.wasm --component

# With resource limits
wrtd component.wasm --component --fuel 100000 --memory 1048576

# Enable memory profiling
wrtd component.wasm --component --memory-profile
```

### CLI Options

```
wrtd [OPTIONS] <module.wasm>

Options:
  --component          Enable component model (required for .wasm components)
  --wasi               Enable WASI support
  --fuel <amount>      Maximum instruction fuel
  --memory <bytes>     Maximum memory limit
  --memory-profile     Show memory usage statistics
  --function <name>    Entry function (default: _start)
  --help               Show help
```

## Examples

```bash
# Run Rust-compiled WASI Preview 2 component
wrtd hello_rust.wasm --component

# Run with fuel limit to prevent infinite loops
wrtd compute.wasm --component --fuel 1000000

# Profile memory usage
wrtd large_module.wasm --component --memory-profile
```

## License

MIT License
