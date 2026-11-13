# SR-82 oracle fixtures: P3 cooperative threads, fused by meld 0.67.0

The seven #620 oracle components are the clang C components in meld's
`tests/p3/coop_threads_547/` (meld v0.67.0; source, sha256 and the C body of
each `run` are in that directory's README). Each was fused here with

    meld fuse tN_x.wasm -o tN_x.core.wasm --memory multi --emit-import-map tN_x.map.json

using meld 0.67.0 (varve covalent layer 2026.11.7). meld writes a timestamp
into the attestation section, so a re-fusion is not byte-identical; the code,
imports and exports are.

| fixture | `$root` imports | wasmtime 50.0.1 on the component (`-W component-model-async=y,component-model-threading=y`) |
|---|---|---|
| `t1_new_resume` | thread-index, thread-resume-later, thread-yield, thread-new-indirect-v0 | 7 |
| `t2_yield_index` | thread-index, thread-yield | 3 |
| `t3_suspend_resume` | thread-index, thread-resume-later, thread-suspend, thread-new-indirect-v0 | 2 |
| `t4_suspend_then_resume` | thread-suspend-then-resume, thread-new-indirect-v0 | trap: cannot block a synchronous task before returning |
| `t5_yield_then_resume` | thread-yield-then-resume, thread-new-indirect-v0 | 9 |
| `t6_promote` | thread-yield-then-promote, thread-suspend-then-promote, thread-new-indirect-v0 | trap: cannot block a synchronous task before returning |
| `t8_many_threads` | thread-resume-later, thread-yield, thread-new-indirect-v0 | 36 |

wasmtime 51-dev gives the same results (kiln#620). The tests in
`kilnd/tests/p3_thread_tests.rs` run each fused module on kiln with its import
map and `--invoke run` and assert these results.
