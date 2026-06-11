# Plan: Verified Cooperative Async Scheduler for Kiln's Embedded Base (`kiln-async`)

**Status:** Design / clean-room. Supersedes the deleted `kiln-component/src/async_/`
(~32k LOC of simulation — **not reused**).
**Target:** `no_std`, **no `alloc`**, fixed-capacity, fuel-bounded, formally verified.
**Audience:** synth-compiled WASM on gale/Zephyr/Cortex-M (the embedded base, per RFC #46).

> Traceability: implements `REQ_ASYNC_SCHED` (see `safety/requirements/roadmap-requirements.yaml`).

## 1. Context and constraints

Per `docs/architecture/rfc46-toolchain-architecture.md`, Component Model async is **not** a feature
of the std interpreter — async coordination is handled by Meld's lowering or the host. On the
embedded path (`Meld → core .wasm → Synth → ELF → gale + kiln-builtins`) there is no interpreter;
synth-compiled native code calls host intrinsics in `kiln-builtins`. **This scheduler is the
host-intrinsic backing for the P3 async ABI on that embedded path.** It lives as a sibling crate of
`kiln-builtins` (`no_std`, buildable as `staticlib` for `thumbv7em-none-eabi`).

Builds on verified-in-repo facts:
- **Bounded structures** (`kiln-foundation/src/bounded.rs`, `bounded_collections.rs`): `BoundedVec`,
  `BoundedQueue`, `BoundedMap` over a capability `MemoryProvider` (`safe_managed_alloc!`).
- **Fuel metering** (`kiln-runtime/src/stackless/engine.rs`): `fuel: AtomicU64`, `set_fuel`,
  `remaining_fuel`, per-instruction decrement + "fuel exhausted" trap. A task's poll slice = a fuel budget.
- **spar Lean proofs** (`spar/proofs/Proofs/Scheduling/{EDF,RMBound,RTA}.lean`, 0 `sorry`): DBF/EDF
  feasibility, Liu&Layland RM bound, Joseph&Pandya RTA with `rta_terminates` / `iterN_le_fixed_point`.
- **Aeneas** wired in `rules_lean/aeneas/` (LLBC→Lean; `aeneas_verified_library`). **Gap:** the Charon
  (`rustc→.llbc`) front-end is not yet wired (Risk R3).
- **gale** (`gale/src/sched.rs`): the template for Verus-on-Rust invariants + Lean theory.

## 2. Architecture

New crate **`kiln-async`** (`no_std`, no `alloc`), depended on by `kiln-builtins`. Modules: `task`
(slot-map TaskTable + intrusive free-list), `ready` (bounded ring / bitmap-bucketed priority queue),
`waker` (index-based, no heap), `waitable`/`stream`/`future` (bounded backing), `poll` (cooperative
loop + fuel slicing), `intrinsics` (P3 ABI surface), `spec/` (Verus), `proofs/lean/` (Aeneas refinement).

All capacities are `const` generics from one `SchedConfig` (`NTASK`, `NREADY`, `NWAIT`, `NPRIO`), so a
target picks its budget at compile time. **Indices, not pointers** (Embassy uses pointers; we use array
indices over `BoundedVec` — pure safe Rust that Charon→Aeneas can translate, no provenance reasoning).
`TaskId = (index, generation)` for ABA-safe handles. Invariant: a `TaskId` is in the ready set at most
once (a per-slot `in_ready` flag), so `NREADY == NTASK` can never overflow.

Task FSM (proved exhaustive, gale-style): `Spawned→Ready→Running→{Ready|Blocked|Completed|Cancelled}`,
`Blocked→Ready`. Illegal transitions return an explicit error (no silent default).

## 3. SOTA patterns adopted

- **Embassy** (`embassy-executor`): intrusive task arena + waker-is-task-index — adopted, but indices
  instead of raw pointers and no atomics (single-core cooperative).
- **RTIC**: bitmap priority selection + all-capacities-known-at-compile-time discipline.
- **`futures`/`async-task`**: standard `Future`/`Poll`/`Waker` surface so synth-emitted and host futures
  interoperate, driven by a custom no-alloc executor.
- **Tock OS**: fixed-size, capability-checked process/grant tables (validates the approach).

## 4. WASM Component Model P3 async ABI surface

Backs: `task.yield/wait/poll/cancel/backpressure/return`, `future.new/read/write/cancel`,
`stream.new/read/write/cancel-*`, `waitable-set.new/join/drop`, `error-context.*`. Each maps to a
bounded structure (e.g. `Stream<T>` = `BoundedQueue<T,CAP,P>` + credit counter). **Backpressure =
credit-based flow control**: `stream.write` decrements credits, blocks the writer at zero (no spin, no
unbounded buffering); `stream.read` increments credits and wakes the writer.

## 5. Scheduling algorithm

Cooperative poll loop, **fuel as the time quantum**: `engine.set_fuel(slot.budget)` before each poll,
account `spent = budget - remaining_fuel()` against a global budget; `Ready` completes + wakes parent,
`Pending` stays blocked/yields, `FuelExhausted` re-enqueues at tail (budget preemption, not OS). This is
deterministic/replayable (no wall-clock); WCET = Σ fuel over a hyperperiod — exactly what spar's RTA/DBF
theory is parameterized over (their `exec` = our fuel units). Modes: FIFO (MVP, O(1)), fixed-priority
bitmap (O(1), → RMBound), EDF (optional, → spar EDF).

## 6. Verification plan ("full circle")

Three tools, each on its best layer; CI gates on all three.

| Property | Tool | Reuses |
|---|---|---|
| Ready-queue ordering & no-overflow; TaskTable invariant; FSM soundness; backpressure correctness | **Verus** (on real Rust) | gale `sched.rs` invariant pattern, `sem/pipe_proofs` |
| No-deadlock / progress; fuel-loop termination | **Lean** refinement | spar `RTA` `bounded_mono_nat_seq` / `rta_terminates` — **direct instantiation** |
| Bounded latency / WCRT; schedulability | **Lean + Aeneas** | spar `RTA` `iterN_le_fixed_point`, `RMBound`, `EDF` — **headline reuse** |
| Memory safety / no alloc | Verus + `#![forbid(unsafe_code)]` (except isolated waker vtable) | — |

The Lean+Aeneas loop: `kiln-async` Rust → Charon `.llbc` → Aeneas → Lean function → **refines** spar's
abstract `rtaStep`/`iterN`/`demandBound` → which are proofs about real-time schedulability → which bound
this scheduler's latency. CI: `cargo-kiln verify --asil d --package kiln-async` (Verus gate) +
`bazel test //kiln-async/proofs/lean:refinement_test` (Lean+Aeneas gate); rivet links each property to
its proof.

## 7. Phased roadmap

0. **Scaffolding** — `kiln-async` crate, `no_std`, `forbid(unsafe_code)`, `SchedConfig` const-generics,
   bounded structures wired, intrinsics return `not_implemented_error()` (no `Ok` stubs). thumbv7em staticlib.
   **Includes deleting the old `kiln-component/src/async_/` and untangling its references** (threading/,
   post_return, types.rs) — that surface is removed per RFC #46.
1. **MVP scheduler (FIFO)** — TaskTable, single-ring ReadyQueue, index waker, fuel-sliced poll loop,
   `task.yield/wait/poll`, `future.*`. Carries the criterion host-bench baseline for every primitive
   (REQ_ASYNC_BENCH): the O(1) claims are measured, not asserted, from the first commit.
   Exit: runs a real synth-lowered async core module on host sim / QEMU.
2. **Streams + backpressure** — `Stream<T>` ring + credits, `error-context`.
3. **Verus invariant proofs (CI gate)** — first ASIL-gating proofs.
4. **Fixed-priority + EDF modes.**
5. **Lean+Aeneas refinement (full circle)** — after Charon is wired in `rules_lean` (R3); reuse spar proofs.
6. **Hardening** — fuzz, Kani on array helpers, property tests over FSM/queue invariants, mutation
   testing of the scheduler core, WCET on hardware + measured fuel→cycles constant (R4), ASIL-D
   verify-matrix. (Dynamic-verification stack: REQ_ASYNC_BENCH.)

## 8. Risks / open questions

- **R1** Waker `RawWakerVTable` is the only `unsafe` + pointer code — isolate behind safe `mark_ready(TaskId)`,
  `external_body` for Verus, exclude from Aeneas (axiomatize wake).
- **R2** `Future`/`Poll` translatability — prove scheduler properties, treat `poll_fn` as opaque; user-future
  correctness is the synthesized code's concern.
- **R3** Charon (`rustc→.llbc`) not yet in `rules_lean` — Phase 5 blocks on landing a `charon_compile` rule;
  until then Verus carries CI gating and Lean reuse is manual refinement statements.
- **R4** Fuel→wall-clock for WCRT needs a measured fuel→cycles constant — owner TBD (synth vs kiln-async).
- **R5** Single-core cooperative only; ISR-driven waking would break the no-atomics assumption — decision needed.
- **R6** Priority inheritance / resource locks — include only if the embedded base needs them (reuse gale
  `PriorityCeiling.lean` if so), else drop to shrink the TCB.

## 9. Concrete reuse summary

spar `RTA.lean` (`rta_terminates`, `bounded_mono_nat_seq`, `iterN_le_fixed_point`), `RMBound.lean`, `EDF.lean`
→ termination + WCRT + schedulability proofs. gale `src/sched.rs` → Verus-on-Rust invariant template.
`rules_lean/aeneas` → Rust→Lean refinement (pending Charon). Embassy/RTIC → intrusive index arenas + bitmap
priority, minus pointers/atomics.
