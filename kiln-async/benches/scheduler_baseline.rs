// Kiln - kiln-async :: scheduler_baseline bench
// SW-REQ-ID: REQ_ASYNC_BENCH
// SPDX-License-Identifier: MIT

//! Criterion baseline for every kiln-async scheduler primitive.
//!
//! The library's documented complexity bounds (O(1) ready-queue push/pop,
//! slot-table operations) are measured here, not asserted. Two spawn cases are
//! benched deliberately: best case (first slot free) and worst case (only the
//! last slot free) — `TaskTable::spawn` is a linear free-slot scan until the
//! planned intrusive free-list lands, and this baseline is what justifies it.
//!
//! Bench units run on the host with std; the library itself is no_std/no_alloc.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use kiln_async::{
    FutureTable, ReadyQueue, SchedConfig, Scheduler, TaskEvent, TaskId, TaskOutcome, TaskTable,
    WaitableSetTable,
};

/// Capacity used throughout — the documented Cortex-M default profile.
const N: usize = 16;

fn task_table_spawn_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_table");

    // Best case: empty table, spawn hits slot 0 immediately.
    group.bench_function("spawn_remove_best_case", |b| {
        let mut t: TaskTable<N> = TaskTable::new();
        b.iter(|| {
            let id = t.spawn().unwrap();
            t.remove(black_box(id)).unwrap();
        });
    });

    // Worst case: slots 0..N-1 occupied, the free-slot scan walks the whole
    // table to find the last slot.
    group.bench_function("spawn_remove_worst_case_scan", |b| {
        let mut t: TaskTable<N> = TaskTable::new();
        for _ in 0..N - 1 {
            t.spawn().unwrap();
        }
        b.iter(|| {
            let id = t.spawn().unwrap();
            t.remove(black_box(id)).unwrap();
        });
    });

    group.finish();
}

fn task_table_transition(c: &mut Criterion) {
    // Transitions are one-way, so each measurement needs a fresh Spawned task.
    c.bench_function("task_table/transition_admit", |b| {
        b.iter_batched(
            || {
                let mut t: TaskTable<N> = TaskTable::new();
                let id = t.spawn().unwrap();
                (t, id)
            },
            |(mut t, id)| {
                t.transition(black_box(id), TaskEvent::Admit).unwrap();
                t
            },
            BatchSize::SmallInput,
        );
    });
}

fn ready_queue_push_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("ready_queue");

    group.bench_function("push_pop_empty", |b| {
        let mut q: ReadyQueue<N> = ReadyQueue::new();
        let id = TaskId {
            index: 0,
            generation: 0,
        };
        b.iter(|| {
            q.push(black_box(id)).unwrap();
            black_box(q.pop());
        });
    });

    // Ring at steady-state occupancy: head/tail wrap continuously.
    group.bench_function("push_pop_near_full", |b| {
        let mut q: ReadyQueue<N> = ReadyQueue::new();
        for i in 0..N - 1 {
            q.push(TaskId {
                index: i as u16,
                generation: 0,
            })
            .unwrap();
        }
        let id = TaskId {
            index: N as u16 - 1,
            generation: 0,
        };
        b.iter(|| {
            q.push(black_box(id)).unwrap();
            black_box(q.pop());
        });
    });

    group.finish();
}

fn scheduler_spawn(c: &mut Criterion) {
    // The composed admission path: slot allocation + FSM Admit + ready enqueue.
    c.bench_function("scheduler/spawn_admit", |b| {
        b.iter_batched(
            || Scheduler::<N, N, N, N>::new(SchedConfig::DEFAULT),
            |mut s| {
                black_box(s.spawn().unwrap());
                s
            },
            BatchSize::SmallInput,
        );
    });
}

fn scheduler_poll_round(c: &mut Criterion) {
    // The full cycle: dispatch → poll (trivial yielding body) → re-enqueue.
    // This is the per-task scheduling overhead a fuel slice pays.
    c.bench_function("scheduler/poll_round_yield_cycle", |b| {
        let mut s = Scheduler::<N, N, N, N>::new(SchedConfig::DEFAULT);
        s.spawn().unwrap();
        b.iter(|| {
            black_box(
                s.poll_round(|_, _, _| Ok(TaskOutcome::Yielded))
                    .unwrap(),
            );
        });
    });
}

fn future_table_lifecycle(c: &mut Criterion) {
    // The write-before-read path: create → complete → consume, one slot reused.
    c.bench_function("future_table/create_complete_consume", |b| {
        let mut t: FutureTable<N> = FutureTable::new();
        b.iter(|| {
            let f = t.create().unwrap();
            black_box(t.complete(f).unwrap());
            t.consume(black_box(f)).unwrap();
        });
    });
}

fn set_table_lifecycle(c: &mut Criterion) {
    // create → drop one set, slot reused each iteration.
    c.bench_function("set_table/create_drop", |b| {
        let mut tbl: WaitableSetTable<N, 8> = WaitableSetTable::new();
        b.iter(|| {
            let s = tbl.create().unwrap();
            tbl.drop_set(black_box(s)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    task_table_spawn_remove,
    task_table_transition,
    ready_queue_push_pop,
    scheduler_spawn,
    scheduler_poll_round,
    future_table_lifecycle,
    set_table_lifecycle
);
criterion_main!(benches);
