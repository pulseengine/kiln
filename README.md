<div align="center">

# Kiln

<sup>WebAssembly runtime for safety-critical systems</sup>

&nbsp;

[![CI](https://github.com/pulseengine/kiln/actions/workflows/ci.yml/badge.svg)](https://github.com/pulseengine/kiln/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/pulseengine/kiln/graph/badge.svg)](https://codecov.io/gh/pulseengine/kiln)
![Rust](https://img.shields.io/badge/Rust-CE422B?style=flat-square&logo=rust&logoColor=white&labelColor=1a1b27)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat-square&logo=webassembly&logoColor=white&labelColor=1a1b27)
![no_std](https://img.shields.io/badge/no__std-compatible-654FF0?style=flat-square&labelColor=1a1b27)
![Formally Verified](https://img.shields.io/badge/Formally_Verified-00C853?style=flat-square&logoColor=white&labelColor=1a1b27)
![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square&labelColor=1a1b27)

&nbsp;

<h6>
  <a href="https://github.com/pulseengine/meld">Meld</a>
  &middot;
  <a href="https://github.com/pulseengine/loom">Loom</a>
  &middot;
  <a href="https://github.com/pulseengine/synth">Synth</a>
  &middot;
  <a href="https://github.com/pulseengine/kiln">Kiln</a>
  &middot;
  <a href="https://github.com/pulseengine/sigil">Sigil</a>
</h6>

</div>

&nbsp;

Meld fuses. Loom weaves. Synth transpiles. Kiln fires. Sigil seals.

A Rust implementation of a WebAssembly runtime with full Component Model and WASI 0.2 support. Designed for safety-critical embedded systems with bounded allocations, deterministic execution, and a modular `no_std` architecture for automotive, medical, and aerospace environments.

Kiln bridges the gap between WebAssembly's portability and the strict requirements of safety-critical deployment. It runs on everything from cloud servers to bare-metal Cortex-M targets.

## Quick Start

```bash
# Clone and build
git clone https://github.com/pulseengine/kiln
cd kiln
cargo build --bin kilnd --features "std,kiln-execution"

# Run a WebAssembly component
./target/debug/kilnd your_component.wasm --component
```

## Architecture

- **`kilnd/`** — Runtime daemon (main executable)
- **`kiln-runtime/`** — Execution engine
- **`kiln-component/`** — Component Model support
- **`kiln-decoder/`** — Binary format parsing
- **`kiln-foundation/`** — Core types and bounded collections
- **`cargo-kiln/`** — Build tooling

## Usage

```bash
# Basic component execution
kilnd component.wasm --component

# With WASI support
kilnd component.wasm --component --wasi

# Set resource limits
kilnd component.wasm --component --fuel 100000 --memory 1048576
```

## Building

```bash
# Install build tool (optional but recommended)
cargo install --path cargo-kiln

# Build runtime
cargo build --bin kilnd --features "std,kiln-execution"

# Run tests
cargo test --workspace
```

## Current Status

**Early Development** — Basic WebAssembly component execution is working:

```bash
./target/debug/kilnd hello_rust.wasm --component
# Output: Hello wasm component world from Rust!
```

### Working

- WebAssembly Component Model parsing and instantiation
- WASI 0.2 stdout/stderr output (`wasi:cli/stdout`, `wasi:io/streams`)
- Core WebAssembly module execution
- Basic memory management with bounds checking
- `no_std` compatible foundation

### In Progress

- Additional WASI 0.2 interfaces (filesystem, environment)
- Cross-component function calls
- Full Component Model linking

## Formal Verification

> [!NOTE]
> **Cross-cutting verification** &mdash; Rocq mechanized proofs, Kani bounded model checking, Z3 SMT verification, and Verus Rust verification are used across the PulseEngine toolchain. Sigil attestation chains bind it all together.

## License

MIT License &mdash; see [LICENSE](LICENSE).

---

<div align="center">

<sub>Part of <a href="https://github.com/pulseengine">PulseEngine</a> &mdash; formally verified WebAssembly toolchain for safety-critical systems</sub>

</div>
