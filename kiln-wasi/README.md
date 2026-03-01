# kiln-wasi

WASI Preview2 implementation for the Kiln WebAssembly Runtime with Preview3 preparation.

Provides WebAssembly System Interface (WASI) host implementations including filesystem, CLI, clocks, I/O, random, and neural network inference capabilities.

## Features

- `preview2` — WASI Preview2 interface support
- `preview3-prep` — Preview3 preparation (sockets, async)
- `wasi-filesystem` — Filesystem access
- `wasi-cli` — CLI environment (args, env, stdio)
- `wasi-clocks` — Wall and monotonic clocks
- `wasi-io` — Streams and polling
- `wasi-random` — Random number generation
- `wasi-nn` — Neural network inference (with tract backend)
- `component-model` — Component model integration

## Architecture

All WASI dispatch goes through `dispatcher.rs`. The runtime engine delegates WASI host calls to the dispatcher, which routes them to the appropriate Preview2/Preview3 implementation modules.

## Usage

```toml
[dependencies]
kiln-wasi = { workspace = true }
```

## License

See the workspace root for license information.
