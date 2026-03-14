# BA RFC #46 Comparative Analysis

## Overview

Bytecode Alliance RFC #46 ("Propose tools and APIs for lowering components
to core modules", Joel Dice, 2026-03-09) proposes a four-part architecture
for running components on any compatible WebAssembly runtime. This analysis
maps the RFC's architecture to Kiln's implementation and identifies gaps
and hazards.

## Architecture Mapping

| RFC Component | Purpose | PulseEngine Equivalent |
|---------------|---------|----------------------|
| `lower-component` | Fuse components into core module | **Meld** |
| Host C API for Lowered Components | Fiber management, stack traces | **Gale task primitives** |
| `host-wit-bindgen` | Generate host bindings for target language | **Kiln WASI dispatcher + Synth stubs** |
| Host C API for Embedder Bindings | Runtime-agnostic module management | **Kiln engine API** |

## Key Architectural Differences

### 1. Embedded Runtime vs Clean Separation

| Aspect | RFC | PulseEngine |
|--------|-----|-------------|
| CM runtime code | Embedded in lowered module (Rust-compiled) | **Not embedded** — clean separation |
| Table management | Guest-side (in sandbox) | **Runtime-side** (kiln-component) |
| Stream/future I/O | Guest-side | **Runtime-side** (kiln-component async) |
| Trust boundary | Smaller (more in sandbox) | **Smaller host API** (less runtime code) |
| Memory overhead | 2+ linear memories per component | **1 per component** (standard) |

The RFC embeds a Rust-compiled CM runtime in every lowered module. This
puts table management and stream I/O in the sandbox (good for security)
but increases code size and memory usage. PulseEngine keeps these in the
runtime, trading a larger runtime for smaller module sizes.

### 2. Fiber vs Stackless Execution

| Aspect | RFC | PulseEngine |
|--------|-----|-------------|
| Async model | Fibers (host-provided intrinsics) | **Stackless** (fuel-aware waker) |
| Stack switching | Planned (Wasm stack switching proposal) | **Not required** (interpreter-based) |
| Host intrinsics | create/suspend/resume fiber | **None** (runtime manages async) |

The RFC requires the host to provide fiber management intrinsics. Kiln's
stackless interpreter naturally supports async without fibers — the
execution state is a data structure that can be suspended and resumed.

### 3. Multiply-Instantiated Modules

Both the RFC and Meld identify this as an unsolved problem. The RFC proposes
three options (reject, duplicate, multi-module output). Meld's
weighted-gap-analysis identifies this as GAP-P2-1 (CRITICAL).

Kiln's position: If the component arrives pre-fused (via Meld), this is
Meld's problem. If the component is loaded directly, Kiln must handle it.
Current status: Kiln supports single-instance-per-module only.

### 4. Type Checking Boundary

The RFC asks: who does type checking — `lower-component` or `host-wit-bindgen`?

PulseEngine's answer:
- **Meld** validates at fusion time (build-time guarantee)
- **Kiln decoder** validates modules at load time
- **Kiln linker** validates import/export type compatibility
- The **dispatcher** does NOT do type checking — it trusts the linker

## Hazards Introduced by RFC Scope (Not in PulseEngine)

These hazards exist in the RFC's architecture but not in PulseEngine's
current implementation, because PulseEngine avoids the patterns that
create them:

| Hazard | Title | Why PulseEngine avoids it |
|--------|-------|--------------------------|
| RL-1 | Loss of async correctness | No embedded CM runtime |
| RL-2 | Loss of fiber isolation | No fiber intrinsics needed |
| RL-3 | Loss of host/guest type safety | No host-wit-bindgen C API |
| RL-4 | Loss of portability | No Host C API divergence |
| RL-5 | Loss of code size efficiency | No function duplication in module |

## Gaps in PulseEngine vs RFC

| Gap | PulseEngine Status | Mitigation |
|-----|-------------------|------------|
| MG-1: Async CM | P2-only; no P3 async | Wait for stack-switching ecosystem |
| MG-2: Resource types | Partial (kiln-component) | Track with SR-12, SR-13 |
| MG-3: Multiply-instantiated modules | Not supported | Track with Meld GAP-P2-1 |
| MG-4: Host bindings generation | Manual (dispatcher) | Unified via HostImportHandler trait (SM-WASI-001) |

## Gaps in RFC vs PulseEngine

| Gap | RFC Status | PulseEngine Advantage |
|-----|-----------|----------------------|
| RG-1: Attestation/provenance | Not mentioned | Sigil integration |
| RG-2: Build reproducibility | Not mentioned | Meld deterministic output |
| RG-3: Formal verification | Not mentioned | 286 Rocq proofs (Meld) |
| RG-4: Certification evidence | Not mentioned | Full STPA + traceability |
| RG-5: Cross-toolchain consistency | Not mentioned | XH-1 through XH-5 tracked |
| RG-6: Safety-level guarantees | Not mentioned | ASIL-D through QM support |

## Implications for Kiln Dispatcher Redesign

The RFC validates the architectural split between "lowering" (Meld) and
"host bindings" (Kiln dispatcher). Key takeaways for the dispatcher:

1. **The dispatcher IS the "host-wit-bindgen" equivalent** — it generates
   the runtime-side bindings between lowered modules and WASI host functions

2. **Canonical ABI correctness is the primary safety concern** — both the
   RFC and STPA analysis identify this as the highest-risk boundary

3. **The dispatcher should NOT embed runtime code** — unlike the RFC's
   approach of compiling CM runtime into the module, Kiln keeps this
   in the runtime. This means the dispatcher must implement lift/lower
   correctly, not delegate to embedded guest code.

4. **Per-component capability isolation** is unique to PulseEngine's
   safety requirements (SR-17). The RFC does not address this because
   it targets general-purpose runtimes, not safety-critical systems.

## References

- [BA RFC #46 PR](https://github.com/bytecodealliance/rfcs/pull/46)
- [Meld RFC #46 Analysis](https://github.com/pulseengine/meld/safety/stpa/rfc46-comparative-analysis.md)
- [Meld Cross-Toolchain Consistency](https://github.com/pulseengine/meld/safety/stpa/cross-toolchain-consistency.yaml)
