# Kani harness census, 2026-11-20

Measurement only, taken on branch `feat/kani-census` off `62e3102f`. Nothing in the product code is changed.

## Summary

- 110 `#[kani::proof]` harnesses exist (counted by grep over `*.rs`, excluding `target/`).
- 3 compile, run and pass (all in `kiln-async`).
- 1 more passes under an extra, non-default feature set (`kiln-math` `verify_i32_safe_add`, features `asil-c,formal-verification-required`).
- 1 runs and FAILS under the feature set the crate documents (`verify_i32_safe_add` with `formal-verification-required` only).
- 1 times out at 40 minutes with no verdict (`verify_i32_safe_div`).
- The other 104 never run. Four independent causes:
  1. `kiln-foundation` does not compile under `cfg(kani)` (72 errors in `bounded.rs`, `resource.rs`, `verify.rs`, `traits.rs`, `prelude.rs`, `safe_memory.rs`). Cargo builds dependencies with `--cfg kani`, so every crate that depends on `kiln-foundation` fails with the same 72 errors.
  2. `kiln-platform` has 19 errors of its own in `formal_verification.rs`. `kiln-component` and `kiln-wasi` hit these.
  3. `kiln-error` and `kiln-sync` `src/verify` modules are gated on `doc` only, so they are not compiled under Kani. `cargo kani` reports "Failed to detect Kani functions".
  4. `kiln-builtins` fails with a duplicate `panic_impl` lang item: its `#[panic_handler]` collides with `std` under Kani.
- Harnesses that compile are not necessarily meaningful. Several only exercise std (`checked_add`, `AtomicU32`, `index < size`) and do not reach kiln code.

## Tooling facts

- `cargo kani --version` already worked, so nothing was installed. `tool-versions.yaml` pins 0.63.0. The machine had Kani 0.67.0 at the start (the `kiln-async` and `kiln-foundation` runs). It became 0.68.0 mid-session (`~/.kani` holds both), which is not our doing: another job on the shared machine upgraded it. The `kiln-math` safe_add runs, the planted control and the baseline re-run used 0.68.0. Several crates declare `kani-verifier = "0.68.0"` as a dependency.
- `cargo kani -j N` is rejected without `--output-format=terse`. `CARGO_BUILD_JOBS=4` was used instead.
- `kiln-build-core/src/kani.rs:219` always passes `--tests`. In Kani 0.67 and 0.68 `--tests` means "verify `#[test]` functions as harnesses". This is a different mode from the `#[kani::proof]` census here, and CI's `cargo-kiln kani-verify` has never exercised the proof harnesses without it.
- The workflow and `cargo-kiln` never pass `--features kani`, so every harness gated on `feature = "kani"` is invisible to CI.
- The workspace `Cargo.toml` `[[workspace.metadata.kani.package]]` lists harness names that do not exist:
  - `kiln-runtime`: `verify_execution_stack_bounds`, `verify_function_call_depth`, `verify_memory_access_bounds`. There is no harness in `kiln-runtime`.
  - `kiln-error`: `verify_error_creation_safety`, `verify_error_propagation`.
  - `kiln-platform`: `verify_platform_memory_operations`, `verify_synchronization_primitives`, `verify_thread_safety`.
  - `kiln-foundation`: `verify_memory_budget_never_exceeded`, `verify_hierarchical_budget_consistency`, `verify_cross_crate_memory_isolation`.

## Commands

Environment for every run: a dedicated `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS=4`, prefix `nice -n 19`, `timeout 2400`, run in `kiln` worktree root.

| Run | Command (after `nice -n 19`) | Outcome |
|---|---|---|
| async | `cargo kani -p kiln-async` | 3 of 3 verified |
| math, default | `cargo kani -p kiln-math` | "Failed to detect Kani functions" (no_std, no harness without the feature) |
| math, fv | `cargo kani -p kiln-math --features formal-verification-required` | `verify_i32_safe_div` ran 2401 s, killed by timeout (exit 124). `safe_add` never reached. |
| math, safe_add | `cargo kani -p kiln-math --features formal-verification-required --harness verify_i32_safe_add` | FAILED (0.019 s) |
| math, safe_add asil-d | `... --features asil-d --harness verify_i32_safe_add` | does not compile: `kani_hook_i32_add` etc. not found (6 errors) |
| math, safe_add asil-c | `... --features asil-c,formal-verification-required --harness verify_i32_safe_add` | SUCCESSFUL (0.023 s) |
| builtins | `cargo kani -p kiln-builtins` | E0152 duplicate `panic_impl`, `kiln-builtins/src/lib.rs:39` |
| host | `cargo kani -p kiln-host` | 72 errors, all in `kiln-foundation` |
| format | `cargo kani -p kiln-format` and `... --features kani,std` | 72 / 69 errors in `kiln-foundation` |
| logging | `cargo kani -p kiln-logging --features kani` | 72 errors in `kiln-foundation` |
| intercept | `cargo kani -p kiln-intercept --features kani` | 72 errors in `kiln-foundation` |
| wasi | `cargo kani -p kiln-wasi` | 89 errors (foundation + `kiln-platform`) |
| error | `cargo kani -p kiln-error --features kani` | "Failed to detect Kani functions" |
| sync | `cargo kani -p kiln-sync --features kani --tests` | "Failed to detect Kani functions" (lib test target) |
| platform | `cargo kani -p kiln-platform --features kani` | 19 errors in `formal_verification.rs` |
| component | `cargo kani -p kiln-component --features kani` | 21 errors, `kiln-platform/src/formal_verification.rs` |
| foundation | `cargo kani -p kiln-foundation` and `... --features kani` | 72 errors each |

Raw logs are not committed.

## Per-harness table

Result key: PASS, FAIL, TIMEOUT, NOCOMPILE (the crate or a dependency fails to compile under `cfg(kani)`), NOTBUILT (module not compiled under Kani, harness never reached).

"Live caller" is judged by grep over non-test, non-example, non-kani code. Entries marked "std only" prove behaviour of the Rust standard library or of a local expression and do not touch kiln code.

### kiln-async

| Harness | file:line | Proven function | Live caller | Narrowing / assumptions | Result, time |
|---|---|---|---|---|---|
| `verify_fsm_step_total_and_terminal_absorbing` | `kiln-async/src/kani_proofs.rs:43` | `TaskState::step` | Yes in-crate: `TaskTable::transition` (`task.rs:240`) used by the scheduler. No consumer outside the crate in the workspace (only the excluded `benchmarks/executor-comparison`). | `any_state`/`any_event` use `kani::any::<u8>() % 6` and `% 7`: all enum values reachable, no gap | PASS, 0.026 s (0.67), 0.022 s (0.68) |
| `verify_ready_queue_never_overflows` | `kani_proofs.rs:61` | `ReadyQueue<4>::push/pop/len` | Yes in-crate (`lib.rs:186,225,246,456`), same no-external-consumer caveat | `#[kani::unwind(7)]`; 6 iterations; queue capacity 4; one fixed `TaskId` | PASS, 0.25 s (0.67), 0.16 s (0.68) |
| `verify_task_table_generation_aba_safe` | `kani_proofs.rs:81` | `TaskTable<2>::spawn/remove/state_of` | Yes in-crate (`lib.rs:187,270,468,472`), same caveat | table of 2 slots, one spawn/remove/spawn cycle (concrete, no symbolic input) | PASS, 0.24 s (0.67), 0.23 s (0.68) |

### kiln-math

| Harness | file:line | Proven function | Live caller | Narrowing / assumptions | Result, time |
|---|---|---|---|---|---|
| `verify_i32_safe_add` | `kiln-math/src/safety.rs:648` | `<i32 as SafeArithmetic>::safe_add` | No live caller: only `kiln-math/examples/safety_demo.rs` and tests | gated `cfg(all(kani, feature = "formal-verification-required"))`. The property (`== saturating_add`) only holds with `asil-c` or `asil-d`; the default build uses checked arithmetic. | FAIL with `formal-verification-required` only (0.019 s). PASS with `asil-c,formal-verification-required` (0.023 s). `asil-d` does not compile. |
| `verify_i32_safe_div` | `safety.rs:665` | `<i32 as SafeArithmetic>::safe_div` | No live caller (same) | same gate. Expects saturate on `MIN / -1` (asil-c/d behaviour) but the tested build returns `Err`, so it would also fail. | TIMEOUT, 2401 s (40 min), no verdict |

### kiln-builtins

| Harness | file:line | Proven function | Live caller | Narrowing / assumptions | Result |
|---|---|---|---|---|---|
| `cabi_realloc_never_overflows` | `kiln-builtins/src/lib.rs:769` | `cabi_realloc` | exported `extern "C"`, no in-tree caller (called from guest or synth code) | `assume(heap_start > 0)`, `assume(align <= 4096)`, `assume(align` is 0 or a power of 2`)` | NOCOMPILE (E0152) |
| `cabi_realloc_alignment_correct` | `lib.rs:790` | `cabi_realloc` | same | `heap_start < 0x1000_0000`, `align_shift <= 8`, `0 < new_size <= 1024` | NOCOMPILE |
| `dispatch_import_returns_zero_without_platform` | `lib.rs:810` | `__meld_dispatch_import` | exported `extern "C"`, no in-tree caller | none | NOCOMPILE |

### kiln-host, kiln-logging, kiln-intercept (all NOCOMPILE via `kiln-foundation`)

| Harness | file:line | Proven function | Live caller | Narrowing | Result |
|---|---|---|---|---|---|
| `verify_cloneable_fn` | `kiln-host/src/verify/mod.rs:16` | `CloneableFn::new/call` | partial: `new_with_args` is used by `kiln-runtime` `capability_engine.rs:580` | concrete, no `kani::any` | NOCOMPILE |
| `verify_callback_registry` | `kiln-host/src/verify/mod.rs:35` | `CallbackRegistry::register_host_function/has_host_function` | Yes: `kiln-host/src/callback.rs:558,585`, `builder.rs:132,233` | concrete | NOCOMPILE |
| `verify_log_level` | `kiln-logging/src/verify/mod.rs:10` | `LogLevel::from_string_or_default/as_str` | not confirmed beyond unit tests in `level.rs` | concrete | NOCOMPILE |
| `verify_log_operation` | `kiln-logging/src/verify/mod.rs:27` | `LogOperation::new/with_component` | not confirmed beyond tests in `operation.rs` | concrete | NOCOMPILE |
| `verify_logging_ext` | `kiln-logging/src/verify/mod.rs:47` | `LoggingExt::register_log_handler` | `kiln-logging/src/handler.rs:122` | concrete | NOCOMPILE |
| `verify_interceptor_modifies_args` | `kiln-intercept/src/verify/mod.rs:59` | `LinkInterceptor::intercept_call` | Yes: `kiln-host/src/callback.rs:400` | concrete; also gated `std` | NOCOMPILE |
| `verify_interceptor_passthrough` | `:77` | same | Yes | concrete | NOCOMPILE |
| `verify_multiple_strategies` | `:95` | same | Yes | concrete | NOCOMPILE |
| `verify_error_passthrough` | `:117` | same | Yes | concrete | NOCOMPILE |

### kiln-format (NOCOMPILE via `kiln-foundation`)

| Harness | file:line | Proven function | Live caller | Narrowing | Result |
|---|---|---|---|---|---|
| `verify_leb128_roundtrip` | `kiln-format/src/lib.rs:405` | `write_leb128_u32` / read | Yes: `section.rs:270,412` | `assume(value <= 0xFFFF)`: u32 narrowed to u16 range | NOCOMPILE |
| `verify_rle_roundtrip` | `kiln-format/src/verify.rs:17` | RLE compress/decompress | not checked | none recorded | NOCOMPILE |
| `verify_module_version_detection` | `verify.rs:31` | `uses_experimental_features` | none found | one concrete header | NOCOMPILE |
| `verify_state_section_roundtrip` | `verify.rs:44` | `create_state_section`/`extract_state_section` | `kiln-runtime/src/state/mod.rs:11` re-exports | empty `StateSection` only | NOCOMPILE |
| `verify_memory_limits` | `verify.rs:60` | none (`limits.max.unwrap() >= limits.min` on literals 1,2) | none: tautology | concrete | NOCOMPILE |
| `verify_feature_detection` | `verify.rs:73` | `VersionInfo::default` | not checked | concrete | NOCOMPILE |

### kiln-foundation (all NOCOMPILE: 72 errors)

| Harness | file:line | Proven function | Live caller | Narrowing | Result |
|---|---|---|---|---|---|
| `verify_bounded_vec_push_capacity` | `bounded.rs:4330` | `BoundedVec::push` | Yes (`kilnd/src/bounded_kilnd_infra.rs:151`) | capacity 8, `u32` items, concrete | NOCOMPILE |
| `verify_bounded_vec_pop_state` | `bounded.rs:4354` | `BoundedVec::pop` | Yes | concrete | NOCOMPILE |
| `verify_bounded_vec_indexing` | `bounded.rs:4385` | `BoundedVec::get` | Yes | `any_where(c <= 8)` | NOCOMPILE |
| `verify_bounded_string_utf8` | `bounded.rs:4414` | `BoundedString::from_str/as_str` | not checked | one literal string | NOCOMPILE |
| `verify_no_overflow_in_capacity_calculation` | `bounded.rs:4432` | none: `usize::checked_mul` (std only) | n/a | `any_where(0 < item_size <= 1024)` | NOCOMPILE |
| `verify_memory_provider_bounds` | `bounded.rs:4451` | `provider.write_data/read_data` | not checked | `any_where` offset < SMALL, len <= SMALL-offset | NOCOMPILE |
| `verify_resource_id_operations` | `resource.rs:633` | `ResourceId` ops | not checked | not recorded | NOCOMPILE |
| `verify_resource_operation_permissions` | `resource.rs:655` | resource ops | not checked | not recorded | NOCOMPILE |
| `verify_resource_type_serialization` | `resource.rs:696` | `ResourceType` (de)serialisation | not checked | not recorded | NOCOMPILE |
| `verify_resource_handle_properties` | `resource.rs:720` | `ResourceHandle` | not checked | not recorded | NOCOMPILE |
| `verify_resource_bounds_checking` | `resource.rs:735` | resource bounds | not checked | not recorded | NOCOMPILE |
| `verify_resource_access_safety` | `resource.rs:764` | resource access | not checked | not recorded | NOCOMPILE |
| `verify_safe_slice_bounds` | `linear_memory.rs:563` | `safe_slice_from_verified_bounds` | Yes: `linear_memory.rs:341` | `any_where(0 < size <= 4096)`, offset < size, len <= size-offset | NOCOMPILE |
| `verify_safe_slice_mut_bounds` | `linear_memory.rs:584` | `safe_slice_mut_from_verified_bounds` | Yes: `linear_memory.rs:356,429` | same | NOCOMPILE |
| `verify_no_overflow_in_bounds_check` | `linear_memory.rs:605` | none: `usize::checked_add` (std only) | n/a | none | NOCOMPILE |
| `verify_bounded_collections_memory_safety` | `verify.rs:140` | bounded collections | Yes (as above) | `assume(0 < capacity <= 64)`, `unwind(10)` | NOCOMPILE |
| `verify_safe_memory_bounds` | `verify.rs:195` | safe memory read/write | not checked | `assume(0 < size <= 256)`, `access_count <= 8`, `unwind(8)` | NOCOMPILE |
| `verify_atomic_memory_operations` | `verify.rs:234` | atomic memory ops | not checked | `unwind(5)` | NOCOMPILE |
| `verify_component_value_type_safety` | `verify.rs:270` | value types | not checked | `unwind(5)` | NOCOMPILE |
| `verify_value_type_validation` | `verify.rs:298` | value validation | not checked | `unwind(3)` | NOCOMPILE |
| `verify_arithmetic_safety` | `verify.rs:333` | arithmetic | not checked | `unwind(3)` | NOCOMPILE |
| `verify_bounds_checking` | `verify.rs:377` | bounds check | not checked | `assume(0 < buffer_size <= 64)`, access_len <= 4, `unwind(5)` | NOCOMPILE |
| `verify_budget_never_exceeded` | `formal_verification.rs:23` | `GenericMemoryCoordinator::register_allocation` | Yes: `generic_memory_guard.rs:90` | `assume(init ok)`, size1 <= 1024, size2 <= 2048, fixed budgets | NOCOMPILE |
| `verify_crate_budget_never_exceeded` | `formal_verification.rs:59` | same | Yes | `assume(0 < size <= 2048)` | NOCOMPILE |
| `verify_allocation_ids_unique` | `formal_verification.rs:90` | same | Yes | `assume(init ok)` | NOCOMPILE |
| `memory_coordinator_harness` | `formal_verification.rs:110` | same | Yes | `assume` budgets in (0, 1e6), total >= sum | NOCOMPILE |
| `verify_budget_safety` | `verified_allocator.rs:527` | `VerifiedAllocator::allocate` | static `SYSTEM_ALLOCATOR` (`:471`) | `any_where(0 < b < 1e6)`, `size > 0` | NOCOMPILE |
| `verify_deallocation_reduces` | `verified_allocator.rs:544` | `VerifiedAllocator` drop | same | `any_where(0 < size < 1e6)` | NOCOMPILE |
| `verify_push_never_exceeds_capacity` | `collections/static_vec.rs:1299` | `StaticVec::push` | Yes: `kiln-decoder/src/simple_types.rs:24` | not recorded | NOCOMPILE |
| `verify_bounds_checking` | `static_vec.rs:1317` | `StaticVec` | Yes | not recorded | NOCOMPILE |
| `verify_drop_cleanup` | `static_vec.rs:1335` | `StaticVec` drop | Yes | `unwind(6)` | NOCOMPILE |
| `verify_wcet_determinism` | `static_vec.rs:1346` | `StaticVec` | Yes | not recorded | NOCOMPILE |
| `verify_fifo_order`, `verify_circular_wraparound`, `verify_capacity_enforcement`, `verify_drop_cleanup` | `collections/static_queue.rs:426,442,463,476` | `StaticQueue` | no live caller found (not checked beyond `StaticVec`) | drop_cleanup has `unwind(6)` | NOCOMPILE |
| `verify_sorted_order`, `verify_update_existing`, `verify_remove`, `verify_capacity_enforcement` | `collections/static_map.rs:772,787,799,812` | `StaticMap` | no live caller found | none | NOCOMPILE |

### kiln-platform, kiln-component, kiln-wasi

| Harness | file:line | Proven function | Live caller | Narrowing | Result |
|---|---|---|---|---|---|
| `verify_wasm_page_allocation` | `kiln-platform/src/formal_verification.rs:151` | none: local `num_pages * WASM_PAGE_SIZE` | n/a (tautology) | `assume(0 < num_pages <= 1024)` | NOCOMPILE (19 errors in this file) |
| `verify_atomic_operations_safety` | `:221` | std atomics | n/a | `assume(old_value < usize::MAX)` | NOCOMPILE |
| `verify_mpsc_queue_safety` | `:238` | queue pointers | not checked | `assume` non-null, aligned 8 | NOCOMPILE |
| `verify_scheduling_properties` | `:309` | scheduling | not checked | `assume(function_ptr > 0x1000)` | NOCOMPILE |
| `verify_bounds_checking` | `:388` | local `index < buffer_size` | n/a (tautology) | `assume(0 < buffer_size <= 4096)` | NOCOMPILE |
| `comprehensive_safety_proof` | `:431` | mixed | not checked | not recorded | NOCOMPILE |
| `verify_memory_safety` | `:506` | `annotations::assert_valid_memory` | not checked | `assume(0 < size <= 16*WASM_PAGE_SIZE)`, `unwind(10)` | NOCOMPILE |
| `verify_concurrent_safety` | `:517` | std `AtomicU32` store/load (single thread) | n/a | `unwind(5)` | NOCOMPILE |
| `verify_bounds_checking` (second) | `:540` | local `index < buffer_size` | n/a (tautology) | `assume(0 < buffer_size <= 1024)`, `unwind(3)` | NOCOMPILE |
| `verify_component_type_safety` | `kiln-component/src/verify/mod.rs:21` | component types | not checked | `assume(import_count <= 8 && export_count <= 8)`, `unwind(5)` | NOCOMPILE |
| `verify_namespace_operations` | `:65` | namespaces | not checked | `unwind(4)` | NOCOMPILE |
| `verify_import_export_consistency` | `:118` | import/export | not checked | `param_count <= 4`; limits `max <= 65536`, table `<= 0xFFFF_FFFF`; `unwind(6)` | NOCOMPILE |
| `verify_value_type_safety` | `:210` | value types | not checked | `unwind(3)` | NOCOMPILE |
| `verify_component_instance_safety` | `:246` | instances | not checked | `unwind(4)` | NOCOMPILE |
| `verify_tensor_dimension_no_overflow` | `kiln-wasi/src/nn/formal_verification.rs:13` | `TensorDimensions::new` | Yes: `nn/tract_backend.rs:267` | `Vec<u32>` rank 1..=8, each dim `1..=65536` | NOCOMPILE |
| `verify_resource_id_no_wraparound` | `:39` | local counter plus `HashSet` (std only) | n/a | `assume(initial_id < u32::MAX - 1000)`, 100 iterations | NOCOMPILE |
| `verify_model_size_limits` | `:70` | model validation | not checked | `assume(0 < model_size <= 500 MiB)` | NOCOMPILE |
| `verify_tensor_memory_bounded` | `:98` | tensor sizing | not checked | rank <= 8, `tensor_type <= 8` | NOCOMPILE |
| `verify_capability_consistency` | `:121` | capabilities | not checked | not recorded | NOCOMPILE |
| `verify_rate_limiting_bounds` | `:143` | rate limiting | not checked | not recorded | NOCOMPILE |
| `verify_context_isolation` | `:183` | contexts | not checked | `assume(context_id1 != context_id2)` | NOCOMPILE |
| `verify_error_categorization` | `:198` | errors | not checked | not recorded | NOCOMPILE |
| `verify_model_validation` | `:229` | model validation | not checked | `model_data.len() <= 1024`, `encoding <= 4` | NOCOMPILE |
| `verify_resource_cleanup` | `:259` | resource cleanup | not checked | not recorded | NOCOMPILE |

### kiln-error and kiln-sync (NOTBUILT)

| Harness | file:line | Proven function | Live caller | Narrowing | Result |
|---|---|---|---|---|---|
| `verify_error_creation_and_display`, `verify_factory_methods`, `verify_error_from_verify_error`, `verify_result_type` | `kiln-error/src/verify.rs:61,85,108,120` | `Error` constructors | Yes (core error type) | none recorded | NOTBUILT: `pub mod verify` gated `cfg(all(not(coverage), doc))`; `cargo kani --features kani` ends "Failed to detect Kani functions" |
| `verify_mutex_no_data_races`, `verify_mutex_new_lock_unlock`, `verify_mutex_guard_deref`, `verify_mutex_guard_deref_mut`, `verify_rwlock_concurrent_access`, `verify_rwlock_new_write_read`, `verify_rwlock_new_read_read`, `verify_rwlock_read_guard_deref`, `verify_rwlock_write_guard_derefmut`, `verify_atomic_operations_safety` | `kiln-sync/src/verify.rs:20,62,79,88,107,171,187,210,219,239` | `KilnMutex`, `KilnRwLock` | Yes (`kiln-platform/src/ipc.rs:205` etc.) | `op_count <= 5`, `abs(increment) < 1000 / 100`, `new_value < 1000`, `unwind(3..6)` | NOTBUILT: module gated `doc` only |
| `verify_mutex_new_lock_unlock` ... `verify_rwlock_read_unlocks_for_write` (9) | `kiln-sync/tests/kani_proofs.rs:17,39,47,65,86,108,116,132,146` | `KilnMutex`, `KilnRwLock` | Yes | `unwind(3)` and `unwind(6)`; uses `println!` | NOTBUILT: `cargo kani --features kani --tests` fails with "Failed to detect Kani functions" on the lib test target; the integration test target is never selected |

## Planted control

Harness: `kiln-async` `verify_task_table_generation_aba_safe`, which passed on 0.67.0 and again on 0.68.0 right before the mutation. Proven function: `TaskTable::remove`, called by the scheduler at `kiln-async/src/lib.rs:187,270,468,472`.

Mutation: `kiln-async/src/task.rs`, `remove`: `slot.generation = slot.generation.wrapping_add(1);` changed to `wrapping_add(0);`.

- Command: `cargo kani -p kiln-async --harness verify_task_table_generation_aba_safe` (Kani 0.68.0).
- Mutant result: `VERIFICATION:- FAILED`, failure `assertion failed: t.state_of(old).is_none()` at `kani_proofs.rs:87:5`, verification time 0.237 s.
- After the revert, `git diff` is empty and the harness passes again (3 of 3 harnesses, `Complete - 3 successfully verified harnesses`).

Caveat: the harness is in a crate with no workspace consumer outside its own scheduler, so the control shows the harness is sensitive, not that a product path depends on it.

## What this means for a required Kani check

- Only the 3 `kiln-async` harnesses could be made a required check today. They pass, run in under a second and fail under mutation.
- `kiln-math` `verify_i32_safe_add` is also usable with `--features asil-c,formal-verification-required`, but it proves a function with no live caller.
- Everything else first needs `kiln-foundation` and `kiln-platform` to compile under `cfg(kani)`, which is a code-fixing task of roughly 90 errors, not a configuration change.
- `cargo-kiln kani-verify` should not pass `--tests`, and should pass the per-package feature set.
