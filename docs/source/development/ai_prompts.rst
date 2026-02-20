AI Agent Task Plan: Kiln Runtime
===================================

Goal
----

Refactor all crates in the ``kiln`` project for strict ``no_std`` support (excluding ``alloc``) and compliance with functional safety guidelines. Each crate must be self-contained, pass its success and safety checks, and maintain the dependency isolation rules outlined below.

Implementation Pattern Guidelines
---------------------------------

1. **Builder Pattern**: All complex types should use the Builder pattern:

   - Every non-trivial struct should have a corresponding ``{Type}Builder``
   - Builders should use method chaining (``with_x()`` methods)
   - Builders should enforce safety rules at compile-time when possible
   - Default values should be provided via ``Default`` implementation on the Builder
   - Builders should have a final ``build()`` method to create the target type

2. **External Dependencies**:

   - No external crates for kiln core crates (stick to std/core/alloc only)
   - Only use workspace dependencies (kiln-* crates)
   - Any third-party dependencies must be feature-gated and optional
   - libc dependency for platform-specific code must be behind "use-libc" feature

3. **Error Handling**:

   - All public APIs should return ``Result<T, kiln_error::Error>``
   - Use specific error constructors (e.g., ``memory_error``, ``system_error``)
   - Avoid unwrap/expect/panic at all costs
   - No default/panic error handling, propagate errors to caller

4. **Module Structure**:

   - Public types must be reexported via ``prelude.rs``
   - Implementation details should be private modules
   - Trait definitions before struct implementations
   - Common trait implementations should use macros when appropriate

Implementation Sequence
-----------------------

Follow this exact order, as it respects the internal crate dependency tree. Complete all steps for each crate before proceeding to the next.

1. ``kiln-error`` - Error handling: done
2. ``kiln-foundation`` - Core type definitions
3. ``kiln-sync`` - Synchronization primitives
4. ``kiln-logging`` - Logging utilities
5. ``kiln-math`` - Mathematical operations
6. ``kiln-format`` - Binary format handling
7. ``kiln-decoder`` - WebAssembly binary decoder
8. ``kiln-intercept`` - System call interception
9. ``kiln-instructions`` - WebAssembly instruction set
10. ``kiln-component`` - WebAssembly component model support
11. ``kiln-host`` - Host environment integration
12. ``kiln-runtime`` - Core runtime implementation
13. ``kiln-test-registry`` - Testing utilities
14. ``kiln-verification-tool`` - Verification utilities
15. ``kiln`` - The main WebAssembly runtime crate

Agent Execution Flow (per crate)
--------------------------------

1. Apply ``#![no_std]`` and ensure ``#![forbid(unsafe_code)]`` unless explicitly allowed (e.g., ``hal``)
2. Replace ``Vec``, ``Box``, ``String``, etc. with stack-allocated or safe memory abstractions
3. Implement crate internals according to plan (refer to ``memory_rework.plan.md``)
4. Run validation tests (see below)
5. Log any missing functionality or ask for clarification if a stub is ambiguous

Success Metrics
---------------

- [ ] Builds cleanly under both ``std`` and ``no_std`` (without ``alloc``). Default feature should be only no_std. Alloc and std only to be optional.
- [ ] Each crate only uses allowed dependencies (no external crates)
- [ ] Public types exposed via a ``prelude.rs``
- [ ] Builder pattern implemented for all complex types
- [ ] No ``unwrap``, ``expect``, or panics unless justified in non-safety path
- [ ] All API operations that can fail return ``Result<T, Error>``
- [ ] ``cargo clippy`` passes with no warnings
- [ ] ``cargo test`` runs under ``std`` and custom ``no_std`` test runner
- [ ] ``cargo doc`` builds without warnings
- [ ] No duplicate types or logic
- [ ] Type and error handling is unified across crates
- [ ] All ``kiln-runtime`` math goes through ``kiln-math``
- [ ] WASM 2.0 instructions implemented (`WASM 2.0 Spec <https://www.w3.org/TR/wasm-core-2>`_)
- [ ] Only ``kiln-decoder`` uses ``kiln-format``; other crates interact via ``kiln-foundation``

Functional Safety Checklist (per crate)
---------------------------------------

0. Header + Meta
~~~~~~~~~~~~~~~~

- [ ] File banner with SPDX: MIT license, copyright: 2025 Ralf Anton Beier
- [ ] UTF-8 + POSIX ``\n`` line endings

1. Language Restrictions
~~~~~~~~~~~~~~~~~~~~~~~~

- [ ] Stable toolchain only (``rustup show`` → ``stable`` or ``ferrocene``)
- [ ] No ``#![feature]``, ``proc-macro``, ``asm!``, ``TypeId``, ``transmute``
- [ ] No ``Box<dyn Trait>`` or floats in RT code

2. Unsafe Usage
~~~~~~~~~~~~~~~

- [ ] ``#![forbid(unsafe_code)]`` (except HAL)
- [ ] Each ``unsafe`` block ≤ 10 LOC, has ``/// # Safety`` doc
- [ ] No unchecked pointer ops

3. Error Handling
~~~~~~~~~~~~~~~~~

- [ ] ``panic = "abort"`` in all profiles. Defined in the workspace Cargo.toml
- [ ] No ``unwrap``, ``expect``, ``panic!``, etc.
- [ ] All fallible ops return ``Result<T, E>`` with domain errors
- [ ] ``?`` used for propagation, ``Err`` must be handled

4. Control-Flow Soundness
~~~~~~~~~~~~~~~~~~~~~~~~~

- [ ] Exhaustive ``match`` (no ``_``)
- [ ] No ``loop { break }`` as ``while``
- [ ] Recursion bounded and justified
- [ ] Cyclomatic complexity ≤ 10
- [ ] No ``unreachable_unchecked``

5. Memory & Concurrency
~~~~~~~~~~~~~~~~~~~~~~~

- [ ] use types from kiln-foundation and kiln-platform.
- [ ] No ``alloc``, ``Vec``, ``Arc``
- [ ] No ``static mut``
- [ ] Use ``Atomic*``, priority-safe mutexes
- [ ] Unsafe ``Send/Sync`` marked and reviewed
- [ ] ``cargo +nightly miri test`` passes

6. Determinism
~~~~~~~~~~~~~~

- [ ] No ``thread::sleep``, blocking sleeps, or RNG in core logic
- [ ] Use ``#[inline(always)]`` only when justified

7. Build Reproducibility
~~~~~~~~~~~~~~~~~~~~~~~~

- [ ] ``rust-toolchain.toml`` pins version
- [ ] ``cargo fetch --locked`` works offline
- [ ] Use ``cargo auditable`` to embed SBOM

8. Static Analysis (Local)
~~~~~~~~~~~~~~~~~~~~~~~~~~

- [ ] ``cargo clippy`` with ``-D warnings -W clippy::pedantic``
- [ ] ``cargo deny check``
- [ ] ``cargo llvm-cov`` ≥ 90% on safety crates
- [ ] Optional: ``cargo kani`` runs pass if marked

9. Documentation
~~~~~~~~~~~~~~~~

- [ ] All public APIs have rustdoc with Purpose, Inputs, Outputs, Safety
- [ ] Use ``debug_assert!`` for runtime invariants