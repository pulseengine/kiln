//! Interpreter instruction-throughput baseline.
//!
//! Makes the tree-walker's throughput a *measured number* so speed work can be
//! gated (the synth discipline: a regression is a number, not a vibe). Two
//! workloads, both built in-process via `wat` (no filesystem fixture):
//!
//! - `compute_loop` — a tight arithmetic loop (`i*i` accumulate). Exercises the
//!   instruction-dispatch + operand-stack + local get/set hot path.
//! - `memory_loop` — the same loop with an `i64.store` + `i64.load` per
//!   iteration. Exercises the per-access memory path (today a double `Mutex`),
//!   so an optimization there shows up as a delta against this baseline.
//!
//! Throughput is reported per loop *iteration*; multiply by the per-iteration
//! instruction count (~8 compute / ~14 memory) for instructions/sec.
//!
//! Baseline (2026-06, rivet 0.17 era, Apple-silicon dev box, release build):
//!   compute_loop  ~14.0 ms / 200k iters  → ~14.3 M iter/s  (~70 ns/iter)
//!   memory_loop   ~33.8 ms / 200k iters  →  ~5.9 M iter/s  (~169 ns/iter)
//! The ~99 ns/iter gap (only ~6 extra instructions) is dominated by the
//! per-access memory locking; a store+load costs ~47 ns of lock overhead.
//! That gap is the regression/optimization signal for the memory-access work.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use kiln_foundation::values::Value;
use kiln_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};

/// Loop trip count per `execute` call. Kept modest because the tree-walker is
/// reference-grade (SM-PERF-001) — large enough to dominate call/setup, small
/// enough to keep the bench wall-clock sane.
const N: i32 = 200_000;

fn compute_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "compute_loop") (param $n i32) (result i64)
                (local $i i32) (local $acc i64)
                (block $done
                    (loop $l
                        (br_if $done (i32.ge_s (local.get $i) (local.get $n)))
                        (local.set $acc
                            (i64.add (local.get $acc)
                                (i64.extend_i32_u (i32.mul (local.get $i) (local.get $i)))))
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br $l)))
                (local.get $acc)))"#,
    )
    .expect("compute wat parses")
}

fn memory_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (memory 1)
            (func (export "memory_loop") (param $n i32) (result i64)
                (local $i i32) (local $acc i64) (local $addr i32)
                (block $done
                    (loop $l
                        (br_if $done (i32.ge_s (local.get $i) (local.get $n)))
                        (local.set $addr
                            (i32.and (i32.shl (local.get $i) (i32.const 3)) (i32.const 0x7ff8)))
                        (i64.store (local.get $addr) (i64.extend_i32_u (local.get $i)))
                        (local.set $acc (i64.add (local.get $acc) (i64.load (local.get $addr))))
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br $l)))
                (local.get $acc)))"#,
    )
    .expect("memory wat parses")
}

/// Build an engine with an instantiated module and return a closure that runs
/// `export` once over `N` iterations. Setup is done once (outside the measured
/// loop); fuel is set high so the bound never trips mid-measurement.
fn bench_workload(c: &mut Criterion, name: &str, wasm: &[u8], export: &'static str) {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine");
    engine.set_fuel(u64::MAX);
    let module = engine.load_module(wasm).expect("load");
    let instance = engine.instantiate(module).expect("instantiate");

    // Sanity: the workload runs and returns a value before we measure it.
    let _ = engine
        .execute(instance, export, &[Value::I32(N)])
        .expect("workload executes");

    let mut group = c.benchmark_group("interpreter_throughput");
    group.throughput(Throughput::Elements(N as u64));
    group.sample_size(10);
    group.bench_function(name, |b| {
        b.iter(|| {
            let r = engine
                .execute(instance, export, &[Value::I32(black_box(N))])
                .expect("execute");
            black_box(r);
        });
    });
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_workload(c, "compute_loop", &compute_wasm(), "compute_loop");
    bench_workload(c, "memory_loop", &memory_wasm(), "memory_loop");
}

criterion_group!(throughput, benches);
criterion_main!(throughput);
