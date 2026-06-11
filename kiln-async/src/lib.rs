// Kiln - kiln-async
// SW-REQ-ID: REQ_ASYNC_SCHED
// Design: SM-ASYNC-001, docs/architecture/async-scheduler-plan.md
//
// Copyright (c) 2026 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! # kiln-async — cooperative async scheduler for Kiln's embedded base
//!
//! A `no_std`, **no-`alloc`** cooperative scheduler that backs the WebAssembly
//! Component Model P3 async ABI (task / stream / future / waitable-set, with
//! credit-based backpressure) for synth-compiled targets on gale/Zephyr/
//! Cortex-M. Per RFC #46, async is *not* a feature of the std interpreter — this
//! crate is the host-intrinsic backing for the embedded path only.
//!
//! ## Status — Phase 0 scaffold
//!
//! This crate currently provides the **foundational types** (task identity,
//! lifecycle FSM, bounded ready queue, compile-time config) which are real, and
//! **stubbed behavior** (the cooperative poll loop and the P3 intrinsic surface)
//! which returns an explicit `NOT_IMPLEMENTED` error — never a silent `Ok` (see
//! the project's fail-loud rule). Phase 1 implements the scheduler; Phase 3/5
//! add the Verus + Lean/Aeneas proofs (reusing spar's RTA/RM/EDF theory).
//!
//! ## Design invariants
//!
//! - Fixed-capacity arrays parameterized by compile-time const generics — no
//!   heap, no provider machinery (minimal trusted computing base).
//! - `#![forbid(unsafe_code)]` — the only planned `unsafe` (the waker vtable) is
//!   isolated and arrives in Phase 1.
//! - Scheduling is fuel-bounded: a task's poll slice is a fuel budget, so
//!   execution is deterministic and replayable.

#![no_std]
#![forbid(unsafe_code)]

pub mod config;
pub mod intrinsics;
pub mod ready;
pub mod task;

pub use config::SchedConfig;
pub use ready::ReadyQueue;
pub use task::{TaskEvent, TaskId, TaskState, TaskTable};

use kiln_error::{Error, Result};

/// Cooperative, fuel-bounded async scheduler over fixed-capacity storage.
///
/// `NTASK` is the maximum number of concurrent tasks; `NREADY` is the ready-queue
/// capacity (size it `== NTASK`: the per-slot "in ready set at most once"
/// invariant means it can never overflow).
pub struct Scheduler<const NTASK: usize, const NREADY: usize> {
    tasks: TaskTable<NTASK>,
    ready: ReadyQueue<NREADY>,
    config: SchedConfig,
}

impl<const NTASK: usize, const NREADY: usize> Scheduler<NTASK, NREADY> {
    /// Create an empty scheduler: all task slots free, ready queue empty.
    #[must_use]
    pub const fn new(config: SchedConfig) -> Self {
        Self {
            tasks: TaskTable::new(),
            ready: ReadyQueue::new(),
            config,
        }
    }

    /// The active scheduler configuration (fuel slice, priority levels).
    #[must_use]
    pub const fn config(&self) -> SchedConfig {
        self.config
    }

    /// Number of live (non-free) task slots.
    #[must_use]
    pub const fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Number of tasks currently in the ready set.
    #[must_use]
    pub const fn ready_len(&self) -> usize {
        self.ready.len()
    }

    /// The lifecycle state of a task, or `None` for a stale/unknown handle.
    #[must_use]
    pub fn task_state(&self, id: TaskId) -> Option<TaskState> {
        self.tasks.state_of(id)
    }

    /// Spawn a task and admit it to the ready set.
    ///
    /// Allocates a free table slot (`Spawned`), drives it `Spawned -> Ready`
    /// through the FSM, and enqueues it. Fails loud if the task table is full;
    /// the ready queue is sized `== NTASK` so its push cannot be the limiting
    /// factor. Returns the new task's handle.
    pub fn spawn(&mut self) -> Result<TaskId> {
        let id = self.tasks.spawn()?;
        // Admit the freshly spawned task: Spawned -> Ready, then enqueue. On the
        // (invariant-unreachable) ready-queue overflow, roll back the slot so the
        // table and ready set stay consistent rather than leaking a live task.
        self.tasks.transition(id, TaskEvent::Admit)?;
        if let Err(e) = self.ready.push(id) {
            self.tasks.remove(id)?;
            return Err(e);
        }
        Ok(id)
    }

    /// Wake a task: admit a `Blocked` task back into the ready set.
    ///
    /// This is the safe wake entry point the (Phase 1, isolated-`unsafe`) waker
    /// vtable will call — design R1 in the plan doc.
    ///
    /// Semantics follow the standard `Waker` contract, where wakes may
    /// legitimately race with task completion and may be duplicated:
    ///
    /// - `Blocked` → transitions to `Ready`, enqueues, returns `Ok(true)`.
    /// - Already `Ready` → idempotent no-op, `Ok(false)` (preserves the
    ///   at-most-once-in-ready-set invariant).
    /// - Stale/unknown handle, free slot, or terminal state → `Ok(false)`.
    ///   A waker outliving its task is normal asynchrony per the contract —
    ///   this is *specified* spurious-wake behavior, not a masked error.
    /// - `Spawned` or `Running` → loud error. Waking an unadmitted task is an
    ///   FSM violation; waking a `Running` task needs the wake-pending flag
    ///   (deferred with the poll loop) — erroring here means a lost wakeup is
    ///   impossible to introduce silently.
    pub fn mark_ready(&mut self, id: TaskId) -> Result<bool> {
        match self.tasks.state_of(id) {
            // Spurious wake (stale handle / freed slot) or duplicate wake of a
            // task already in the ready set, or a terminal task: specified
            // no-op per the Waker contract.
            None | Some(TaskState::Ready | TaskState::Completed | TaskState::Cancelled) => {
                Ok(false)
            }
            Some(TaskState::Blocked) => {
                // Enqueue before the FSM step: if the (invariant-unreachable)
                // push fails, the task is still consistently Blocked. The Wake
                // transition after a successful push cannot fail (handle just
                // validated, Blocked--Wake-->Ready is legal).
                self.ready.push(id)?;
                self.tasks.transition(id, TaskEvent::Wake)?;
                Ok(true)
            }
            Some(TaskState::Spawned) => Err(Error::invalid_state_error(
                "kiln-async: wake of a task that was never admitted",
            )),
            Some(TaskState::Running) => Err(Error::not_implemented_error(
                "kiln-async: wake during poll needs the wake-pending flag (poll-loop increment)",
            )),
        }
    }

    /// Dispatch the oldest ready task: dequeue it and drive `Ready -> Running`.
    ///
    /// Returns `Ok(None)` when the ready set is empty. A stale queue entry is
    /// an invariant violation today (nothing removes a queued task yet) and
    /// fails loud rather than being skipped.
    pub fn dispatch_next(&mut self) -> Result<Option<TaskId>> {
        let Some(id) = self.ready.pop() else {
            return Ok(None);
        };
        // A queued task must be Ready; anything else is an invariant violation
        // and the transition's own validation propagates it loudly.
        self.tasks.transition(id, TaskEvent::Dispatch)?;
        Ok(Some(id))
    }

    /// Run one cooperative poll round: select a ready task, poll it for one fuel
    /// slice, and re-dispatch per the result.
    ///
    /// **Phase 1** — not yet implemented. Returns an explicit error rather than a
    /// silent `Ok`, so callers cannot mistake the scaffold for a working loop.
    pub fn poll_round(&mut self) -> Result<()> {
        let _ = &mut self.ready;
        Err(Error::not_implemented_error(
            "kiln-async: cooperative poll loop is implemented in Phase 1",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Sched = Scheduler<4, 4>;

    #[test]
    fn spawn_admits_a_task_to_the_ready_set() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        assert_eq!(s.task_count(), 0);
        assert_eq!(s.ready_len(), 0);

        let id = s.spawn().unwrap();

        assert_eq!(s.task_count(), 1);
        assert_eq!(s.ready_len(), 1);
        assert_eq!(s.task_state(id), Some(TaskState::Ready));
    }

    #[test]
    fn spawn_uses_distinct_handles_per_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let a = s.spawn().unwrap();
        let b = s.spawn().unwrap();
        assert_ne!(a, b);
        assert_eq!(s.ready_len(), 2);
    }

    #[test]
    fn spawn_errors_when_the_task_table_is_full() {
        let mut s: Scheduler<2, 2> = Scheduler::new(SchedConfig::DEFAULT);
        s.spawn().unwrap();
        s.spawn().unwrap();
        assert!(s.spawn().is_err()); // explicit capacity error, never silent drop
    }

    #[test]
    fn dispatch_next_runs_the_oldest_ready_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let a = s.spawn().unwrap();
        let b = s.spawn().unwrap();

        let dispatched = s.dispatch_next().unwrap();

        assert_eq!(dispatched, Some(a)); // FIFO: oldest first
        assert_eq!(s.task_state(a), Some(TaskState::Running));
        assert_eq!(s.task_state(b), Some(TaskState::Ready));
        assert_eq!(s.ready_len(), 1);
    }

    #[test]
    fn dispatch_next_on_empty_ready_set_returns_none() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        assert_eq!(s.dispatch_next().unwrap(), None);
    }

    #[test]
    fn mark_ready_wakes_a_blocked_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.dispatch_next().unwrap();
        // Park it (the poll loop's task.wait path; wired in a later increment).
        s.tasks.transition(id, TaskEvent::Wait).unwrap();
        assert_eq!(s.ready_len(), 0);

        assert!(s.mark_ready(id).unwrap()); // woke it

        assert_eq!(s.task_state(id), Some(TaskState::Ready));
        assert_eq!(s.ready_len(), 1);
    }

    #[test]
    fn mark_ready_is_idempotent_for_an_already_ready_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap(); // Ready, queued once
        assert!(!s.mark_ready(id).unwrap()); // duplicate wake: no-op
        assert_eq!(s.ready_len(), 1); // still at most once in the ready set
    }

    #[test]
    fn mark_ready_tolerates_a_spurious_wake_of_a_dead_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.dispatch_next().unwrap();
        s.tasks.transition(id, TaskEvent::Complete).unwrap();
        s.tasks.remove(id).unwrap();

        // A waker outliving its task is normal per the Waker contract.
        assert!(!s.mark_ready(id).unwrap());
        assert_eq!(s.ready_len(), 0);
    }

    #[test]
    fn mark_ready_during_running_fails_loud() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.dispatch_next().unwrap();
        // Wake-during-poll needs the wake-pending flag (deferred); silently
        // ignoring it would lose the wakeup, so it must error.
        assert!(s.mark_ready(id).is_err());
    }
}
