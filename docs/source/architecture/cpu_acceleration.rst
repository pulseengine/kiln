CPU Acceleration Analysis
=========================

Overview
--------

This document analyzes opportunities for CPU acceleration in the kiln-math crate and architectural considerations for platform-specific optimizations.

Current Architecture
-------------------

The kiln-math crate provides pure Rust implementations of WebAssembly numeric operations. These implementations:

- Use standard Rust integer/float operations
- Rely on LLVM for optimization
- Are portable across all platforms
- Work in no_std environments

CPU Acceleration Opportunities
------------------------------

1. Compiler Auto-vectorization (Current State)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The Rust compiler (via LLVM) already provides significant optimizations:

**What works well:**

- Basic arithmetic operations are already optimized by LLVM
- Simple comparisons compile to efficient CPU instructions
- Bit manipulation (clz, ctz, popcnt) often map to single CPU instructions
- Float operations use hardware FPU when available

**Example:**

.. code-block:: rust

    #[inline]
    pub fn i32_add(lhs: i32, rhs: i32) -> Result<i32> {
        Ok(lhs.wrapping_add(rhs))
    }

This compiles to a single ``add`` instruction on most architectures.

2. Intrinsics Opportunities
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Some operations could benefit from explicit intrinsics:

Population Count (popcnt)
^^^^^^^^^^^^^^^^^^^^^^^^^

- x86: ``_popcnt32``, ``_popcnt64``
- ARM: ``__builtin_popcount``
- Current Rust ``count_ones()`` usually optimizes well

Leading/Trailing Zeros
^^^^^^^^^^^^^^^^^^^^^^^

- x86: ``_lzcnt32``, ``_tzcnt32``
- ARM: ``__clz``, ``__ctz``
- Current Rust ``leading_zeros()``, ``trailing_zeros()`` usually optimize well

Saturating Arithmetic (Not yet implemented)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

- x86: ``_mm_adds_epi32`` (SIMD)
- ARM: ``qadd``, ``qsub`` instructions
- Would benefit from intrinsics

Fused Multiply-Add (FMA)
^^^^^^^^^^^^^^^^^^^^^^^^

- x86: ``_mm_fmadd_ps``
- ARM: ``vfma``
- Rust's ``f32::mul_add()`` may use FMA when available

3. SIMD Operations (Future)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

For v128 operations, platform-specific SIMD would be essential:

- x86: SSE2/SSE4/AVX/AVX2/AVX-512
- ARM: NEON/SVE
- RISC-V: Vector extension
- WebAssembly: SIMD proposal

4. Platform-specific Considerations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Should we move to kiln-platform?
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

**Pros of keeping in kiln-math:**

- Single source of truth for math operations
- Easier to maintain consistency
- Compiler can still optimize well
- No need for platform detection overhead

**Cons:**

- Can't use platform-specific intrinsics easily
- Miss some optimization opportunities
- Can't leverage special CPU features

**Recommendation:** Hybrid approach

1. Keep basic operations in kiln-math (they optimize well)
2. Add optional ``platform-accel`` feature that enables intrinsics
3. For SIMD operations, consider a separate ``kiln-math-simd`` crate that depends on kiln-platform

Implementation Strategy
-----------------------

Phase 1: Profile Current Performance
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

    # Profile with different architectures
    cargo bench --features benchmark
    # Check assembly output
    cargo rustc --release -- --emit asm

Phase 2: Selective Intrinsics
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Add intrinsics only where measurable benefit exists:

.. code-block:: rust

    #[cfg(all(target_arch = "x86_64", feature = "platform-accel"))]
    pub fn i32_popcnt_accel(val: i32) -> Result<i32> {
        #[cfg(target_feature = "popcnt")]
        unsafe {
            Ok(core::arch::x86_64::_popcnt32(val as i32) as i32)
        }
        #[cfg(not(target_feature = "popcnt"))]
        i32_popcnt(val) // Fallback
    }

Phase 3: SIMD Architecture
~~~~~~~~~~~~~~~~~~~~~~~~~~

When implementing v128 operations:

.. code-block:: text

    kiln-math-simd/
    ├── src/
    │   ├── lib.rs          # Public API
    │   ├── portable.rs     # Portable implementations
    │   ├── x86/           # x86-specific SIMD
    │   ├── arm/           # ARM NEON
    │   └── wasm/          # WebAssembly SIMD

Benchmarking Requirements
------------------------

Before adding platform-specific code, benchmark to verify benefits:

1. **Micro-benchmarks**: Individual operations
2. **Macro-benchmarks**: Real WASM workloads
3. **Cross-platform**: Test on x86_64, aarch64, wasm32

Recommendations
---------------

1. **Keep current architecture** for basic operations - LLVM does well
2. **Add benchmarks** to identify bottlenecks
3. **Selective intrinsics** only where proven benefit
4. **Separate SIMD crate** when implementing v128
5. **Feature flags** for platform acceleration:

   - ``default``: Portable Rust
   - ``platform-accel``: Enable intrinsics
   - ``simd``: Enable SIMD operations

Example: Saturating Addition (Future Implementation)
----------------------------------------------------

.. code-block:: rust

    // Portable version
    pub fn i32_add_sat_s(lhs: i32, rhs: i32) -> Result<i32> {
        Ok(lhs.saturating_add(rhs))
    }

    // Accelerated version (when available)
    #[cfg(all(target_arch = "aarch64", feature = "platform-accel"))]
    pub fn i32_add_sat_s_accel(lhs: i32, rhs: i32) -> Result<i32> {
        unsafe {
            // Use ARM qadd instruction via inline assembly
            let result: i32;
            asm!(
                "qadd {}, {}, {}",
                out(reg) result,
                in(reg) lhs,
                in(reg) rhs,
                options(pure, nomem, nostack)
            );
            Ok(result)
        }
    }

Conclusion
----------

The current pure-Rust implementation is sufficient for most operations. CPU acceleration should be added judiciously based on profiling data. SIMD operations will require platform-specific implementations and should be in a separate module or crate.