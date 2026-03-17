# RFC #46 Toolchain Architecture

BA RFC #46 defines how the PulseEngine toolchain handles WebAssembly Component Model support. The key principle is that component model complexity is resolved at **build time** by Meld, not at runtime by Kiln.

## Toolchain Overview

```
                    Build Time                          Runtime
               +-----------------+            +---------------------+
  .wasm/.wit   |                 |  core .wasm|                     |
  components ---->    Meld       +----------->|   Kiln interpreter  |  (std)
               |  (lower to core)|            |   + host intrinsics |
               +-----------------+            +---------------------+
                        |
                        | core .wasm
                        v
               +-----------------+            +---------------------+
               |                 |  native ELF|                     |
               |     Synth       +----------->|   gale / embedded   |  (no_std)
               |  (AOT compile)  |            |   + kiln-builtins   |
               +-----------------+            +---------------------+
```

## Components

### Meld (Build Time)

Meld lowers Component Model constructs to core WebAssembly modules at build time. This includes:

- Resolving component imports and exports to core-level wiring
- Performing canonical ABI lift/lower transformations statically
- Fusing multi-component graphs into a single core module (or a set of core modules with a known linking protocol)
- Resolving WIT interface types to concrete core value types

After Meld processing, the output is one or more **core WebAssembly modules** with no Component Model constructs remaining. Host/WASI imports that cannot be resolved statically are preserved as core-level imports.

### Kiln Interpreter (std-only, Runtime)

The Kiln interpreter executes **core WebAssembly modules**. It does not interpret Component Model constructs at runtime. Its responsibilities:

- Decode and validate core WebAssembly binary format
- Interpret core instructions (MVP through Wasm 3.0 proposals)
- Manage linear memory, tables, globals
- Dispatch host function imports (WASI, custom host functions)
- Enforce fuel metering, memory budgets, and call depth limits
- Provide CFI (Control Flow Integrity) validation

The interpreter requires `std` (heap allocation, I/O for WASI, threads for shared memory). There is no `no_std` interpreter.

### kiln-builtins (no_std, Runtime)

`kiln-builtins` provides host intrinsics for code that Synth has compiled to native. This crate is `no_std` compatible and runs on bare-metal / RTOS targets. It provides:

- WASI host function implementations suitable for embedded (clocks, random, minimal I/O)
- Memory management intrinsics (linear memory bounds checking, table access)
- Trap handling and fault isolation primitives
- The C ABI surface that Synth-compiled ELF binaries call into

`kiln-builtins` does **not** contain a WebAssembly interpreter. It only provides the runtime support library for natively compiled code.

### Synth (Build Time)

Synth compiles core WebAssembly modules to native ELF binaries for gale and other embedded targets. Its input is the core module output from Meld (or a standalone core module). Synth:

- Translates core Wasm instructions to native machine code (ARM, RISC-V, x86-64)
- Generates calls to `kiln-builtins` for host intrinsics
- Produces position-independent ELF objects linkable into gale firmware images
- Applies platform-specific optimizations (register allocation, instruction scheduling)

### gale (Runtime, Embedded)

The gale RTOS loads and executes Synth-compiled ELF binaries. At runtime, gale provides:

- Task scheduling for Wasm-derived tasks
- Hardware abstraction (MPU, interrupt routing)
- The `kiln-builtins` library linked into the firmware image

## Three Execution Paths

### 1. std Path (Interpreter)

```
Component (.wasm) --> Meld --> Core module (.wasm) --> Kiln interpreter (std)
```

Used for: development, testing, server-side execution, CI. The Kiln interpreter runs the core module directly with full WASI support and dynamic memory allocation.

### 2. Embedded Path (Synth + gale)

```
Component (.wasm) --> Meld --> Core module (.wasm) --> Synth --> ELF --> gale + kiln-builtins
```

Used for: safety-critical embedded targets (ASIL-B through ASIL-D). No interpreter at runtime. All WebAssembly semantics are compiled to native code. `kiln-builtins` provides the minimal runtime support.

### 3. Direct Core Module Path

```
Core module (.wasm) --> Kiln interpreter (std)
                    --> Synth --> ELF --> gale + kiln-builtins
```

Used for: core modules that were never components (e.g., hand-written WAT, third-party core-only modules). Meld is skipped entirely since there are no Component Model constructs to lower.

## What Kiln Does NOT Do

- **Full Component Model runtime**: Kiln does not implement component instantiation, canonical ABI lift/lower, or component-level linking at runtime. Meld handles all of this at build time.
- **Canonical ABI at runtime**: There is no runtime canonical ABI encoder/decoder in the interpreter. Values cross the host boundary as core Wasm types (i32, i64, f32, f64, references).
- **Async model**: The Component Model async proposal (streams, futures, subtask scheduling) is not implemented in the Kiln interpreter. Async coordination, if needed, is handled by Meld's lowering or by the host application.
- **no_std interpreter**: There is no `no_std` WebAssembly interpreter. The `no_std` execution path is exclusively through Synth-compiled native code with `kiln-builtins`.
- **Component-to-component runtime linking**: Multiple components communicating at runtime are fused by Meld at build time into core modules with direct call wiring. Kiln does not perform dynamic component linking.

## Formal Verification Strategy

### Verus / Kani for kiln-builtins

`kiln-builtins` is the trusted computing base for the embedded path. It is small, `no_std`, and has a well-defined interface. Formal verification targets:

- **Memory bounds checking**: Prove that all linear memory accesses through builtins are within allocated bounds (Kani bounded model checking)
- **Trap semantics**: Prove that trap conditions produce correct fault signals, never silent corruption
- **ABI contract**: Verify that the C ABI surface matches what Synth generates (Kani harnesses for representative call patterns)
- **Verus proofs**: For critical arithmetic and state machine logic in builtins (e.g., fuel accounting, capability checks)

### WAST Conformance for Interpreter

The Kiln interpreter is validated against the official WebAssembly specification test suite:

- All WAST test files from the WebAssembly spec repository (excluding legacy exception handling)
- Proposal-specific test suites for enabled proposals (GC, memory64, multi-memory, exception handling, etc.)
- Custom regression tests for edge cases discovered during development

### Rocq for Specification

A shared Rocq (Coq) formal specification defines the canonical ABI element sizing, alignment, and string encoding rules. This specification is the single source of truth that Meld, Kiln, and Synth must all agree on:

- `canonical_abi_element_size` for all component value types
- `align_up` and field layout computation for records and variants
- String transcoding rules (UTF-8, UTF-16, Latin-1 boundaries)

The Rocq proofs establish that these functions satisfy the WebAssembly Component Model specification. Each tool extracts or reimplements these functions and validates agreement via shared test fixtures.
