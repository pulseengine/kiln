<div align="center">

# Kiln

<sup>WebAssembly runtime for safety-critical systems</sup>

&nbsp;

[![CI](https://github.com/pulseengine/kiln/actions/workflows/ci.yml/badge.svg)](https://github.com/pulseengine/kiln/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/pulseengine/kiln/graph/badge.svg)](https://codecov.io/gh/pulseengine/kiln)
![Rust](https://img.shields.io/badge/Rust-CE422B?style=flat-square&logo=rust&logoColor=white&labelColor=1a1b27)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat-square&logo=webassembly&logoColor=white&labelColor=1a1b27)
![no_std](https://img.shields.io/badge/no__std-compatible-654FF0?style=flat-square&labelColor=1a1b27)
[![WebAssembly spec suite](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fpulseengine%2Fkiln%2Fbadges%2Fspec-suite.json&style=flat-square&labelColor=1a1b27)](https://github.com/pulseengine/kiln/actions/workflows/verification-honesty.yml)
![Kani: 4 of 109 harnesses verify, rest ledgered](https://img.shields.io/badge/Kani-4_of_109_harnesses_verify%2C_rest_ledgered-1f6feb?style=flat-square&labelColor=1a1b27)
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
cargo build --bin kiln --features "std,kiln-execution"

# Run a WebAssembly component
./target/debug/kiln --component your_component.wasm
```

## Architecture

- **`kilnd/`** — Runtime CLI crate (builds the `kiln` binary)
- **`kiln-runtime/`** — Execution engine
- **`kiln-component/`** — Component Model support
- **`kiln-decoder/`** — Binary format parsing
- **`kiln-foundation/`** — Core types and bounded collections
- **`cargo-kiln/`** — Build tooling

## Usage

```bash
# Basic component execution
kiln --component component.wasm

# With WASI support
kiln --component --wasi component.wasm

# Set resource limits
kiln --component --fuel 100000 --memory 1048576 component.wasm

# kiln's options go before the module; everything after it is the guest's
# argv, as with `wasmtime run`
kiln --dir . tool.wasm check - --format json

# --dir HOST::GUEST preopens HOST under the guest-visible name GUEST, as with
# `wasmtime run --dir`; --dir HOST uses HOST as the name
kiln --dir ./data::/data tool.wasm
```

## Building

```bash
# Install build tool (optional but recommended)
cargo install --path cargo-kiln

# Build runtime
cargo build --bin kiln --features "std,kiln-execution"

# Run tests
cargo test --workspace
```

## Current Status

**Early Development** — Basic WebAssembly component execution is working:

```bash
./target/debug/kiln --component hello_rust.wasm
# Output: Hello wasm component world from Rust!
```

### Working

- WebAssembly Component Model parsing and instantiation
- WASI 0.2 stdout/stderr output (`wasi:cli/stdout`, `wasi:io/streams`)
- WASI 0.2 arguments, environment (`--wasi-env NAME`) and exit status for meld-fused command components
- WASI 0.2 filesystem under `--dir HOST::GUEST` preopens: open, read and write (including via streams and append), stat, read-directory, create and remove directories, unlink and rename
- Core WebAssembly module execution
- Basic memory management with bounds checking
- `no_std` compatible foundation

### In Progress

- WASI filesystem error reporting as in wasmtime: a path the sandbox refuses ends the run instead of returning an error code to the guest, and a `..` that stays inside the preopen is refused (SR-106)
- Cross-component function calls
- Full Component Model linking

## Verification &amp; correctness

<!-- claim: KILN-VERIFICATION-STATUS -->
kiln's correctness rests on **conformance testing and bounded model checking** &mdash;
not (yet) a mechanized proof of its WebAssembly execution semantics. We state that plainly.

- **WebAssembly spec suite:** the current pass rate is **re-derived by CI on every
  run and shown in the badge above** (files passed / total · executed-assertion rate),
  so the number here can't drift from the evidence. We report the **file ratio** as the
  headline on purpose: a file that fails to *parse* (e.g. the `custom-descriptors`
  proposal) contributes zero assertions to both sides of the assertion rate, so only
  the file count reflects it &mdash; the assertion rate alone would flatter exactly
  where it shouldn't. Known gaps: validation strictness on some GC-proposal cases (a few
  should-be-invalid modules are currently accepted) and the `custom-descriptors` parse
  failure. Reproduce locally with `cargo-kiln testsuite --run-wast`.
- **Unit tests:** 300+ across the interpreter core.
- **Kani (CBMC bounded model checking):** 4 of 109 harnesses verify, and the rest are
  ledgered. The four are the kiln-async scheduler invariants (3) and the kiln-builtins import
  dispatch with no platform (1); they run on every PR in the "Kani proofs and ledger" job.
  [`safety/verification/proof-ledger.toml`](safety/verification/proof-ledger.toml) records
  why each other harness does not run (most do not compile under Kani) and every
  input-narrowing assumption, and CI fails if that list grows. The interpreter core
  (`kiln-runtime` / `kiln-instructions` / `kiln-decoder`) has no formal proofs.
- **Not claimed:** formal proof of the interpreter's execution semantics. Mechanized
  semantics (WasmCert-style) is a research direction, not a shipped guarantee.

Other PulseEngine tools use additional techniques (Rocq, Z3/translation-validation,
Verus) on *their own* artifacts; that does not transfer a proof onto kiln's interpreter.
These claims are gated by [`claim-check`](claims.yaml) so they cannot drift from the
evidence.

## License

MIT License &mdash; see [LICENSE](LICENSE).

---

<div align="center">

<sub>Part of <a href="https://github.com/pulseengine">PulseEngine</a> &mdash; a WebAssembly toolchain for safety-critical systems, grounded in spec-suite conformance and a ledgered Kani baseline</sub>

</div>
