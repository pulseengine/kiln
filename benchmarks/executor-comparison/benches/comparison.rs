// Kiln - executor-comparison-bench :: comparison
// SW-REQ-ID: REQ_ASYNC_BENCH
// SPDX-License-Identifier: MIT

//! Head-to-head microbench: kiln-async vs. embassy-executor on the *same*
//! workload — N cooperative tasks, each yielding K times, driven to completion
//! (issue #368).
//!
//! This crate is workspace-excluded so embassy's transitive dependency closure
//! stays out of the safety-critical runtime's lockfile. Run it with:
//!   `cd benchmarks/executor-comparison && cargo bench`
//!
//! The comparison is **per-poll scheduling overhead**: cost to pick a ready
//! task, advance it one step (a trivial yield), and re-park it. Both runtimes
//! do exactly this; they differ in what a "task" is:
//!
//! - **embassy** polls real Rust `Future` state machines through wakers +
//!   vtable dispatch; tasks are statically allocated (`pool_size`).
//! - **kiln-async** dispatches a fixed-shape P3 task and calls an opaque poll
//!   callback (no Rust `Future`, no waker vtable); const-generic capacity,
//!   no_alloc, `forbid(unsafe_code)`.
//!
//! Same `N`×`K` work on both ⇒ total time / (N·K) is the per-poll cost.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{Criterion, criterion_group, criterion_main};

/// Cortex-M default profile capacity (matches scheduler_baseline).
const N: usize = 16;
/// Yields per task before completion.
const K: usize = 64;

// ---------------------------------------------------------------------------
// kiln-async: drive poll_round until every task has yielded K times + completed
// ---------------------------------------------------------------------------
fn kiln_run_to_completion() {
    use kiln_async::{SchedConfig, Scheduler, TaskOutcome};

    let mut s = Scheduler::<N, N, N, N, N>::new(SchedConfig::DEFAULT);
    for _ in 0..N {
        s.spawn().unwrap();
    }
    // Per-task-slot yield counters; complete once a slot has yielded K times.
    let mut yields = [0u32; N];
    loop {
        let round = s
            .poll_round(|_s, id, _fuel| {
                let slot = id.index as usize;
                yields[slot] += 1;
                if yields[slot] >= K as u32 {
                    Ok(TaskOutcome::Completed)
                } else {
                    Ok(TaskOutcome::Yielded)
                }
            })
            .unwrap();
        if matches!(round, kiln_async::PollRound::Idle) {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// embassy-executor: the same workload on its raw executor, polled to completion
// ---------------------------------------------------------------------------
/// embassy's pender: normally wakes the owning thread to call `poll()`. Here we
/// drive `poll()` manually in a busy loop, and a woken task is already back in
/// the run queue, so the notification is a no-op. (We build embassy with
/// `default-features = false` so no platform provides a conflicting `__pender`.)
#[unsafe(export_name = "__pender")]
fn __pender(_context: *mut ()) {}

static EMBASSY_DONE: AtomicUsize = AtomicUsize::new(0);

#[embassy_executor::task(pool_size = N)]
async fn embassy_yielder() {
    for _ in 0..K {
        embassy_futures::yield_now().await;
    }
    EMBASSY_DONE.fetch_add(1, Ordering::Relaxed);
}

fn embassy_run_to_completion(executor: &'static embassy_executor::raw::Executor) {
    EMBASSY_DONE.store(0, Ordering::Relaxed);
    let spawner = executor.spawner();
    for _ in 0..N {
        spawner.spawn(embassy_yielder().unwrap());
    }
    while EMBASSY_DONE.load(Ordering::Relaxed) < N {
        unsafe {
            executor.poll();
        }
    }
}

fn executor_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_comparison");
    group.throughput(criterion::Throughput::Elements((N * K) as u64));

    group.bench_function("kiln_async_16x64", |b| {
        b.iter(|| black_box(kiln_run_to_completion()));
    });

    // One leaked 'static raw executor, reused across iterations (task slots free
    // on completion and are re-spawned each iteration).
    let executor: &'static embassy_executor::raw::Executor = Box::leak(Box::new(
        embassy_executor::raw::Executor::new(core::ptr::null_mut()),
    ));
    group.bench_function("embassy_16x64", |b| {
        b.iter(|| black_box(embassy_run_to_completion(executor)));
    });

    group.finish();
}

criterion_group!(benches, executor_comparison);
criterion_main!(benches);
