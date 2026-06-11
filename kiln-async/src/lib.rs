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

/// How a polled task left its fuel slice. Returned by the poll callback —
/// the bridge to the engine running the task's synth-lowered code.
///
/// Fuel exhaustion is reported as [`TaskOutcome::Yielded`]: preemption at the
/// quantum boundary is a forced cooperative yield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    /// Yielded cooperatively (or exhausted its fuel slice) — still runnable.
    Yielded,
    /// Parked on a waitable; re-admitted via [`Scheduler::mark_ready`].
    Waited,
    /// Finished; its slot is freed.
    Completed,
    /// Cancelled itself (or honored a cancel request) during the slice;
    /// terminal, its slot is freed.
    Cancelled,
}

/// Result of one [`Scheduler::poll_round`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRound {
    /// The ready set was empty — nothing was polled.
    Idle,
    /// This task was polled for one fuel slice.
    Polled(TaskId),
}

/// Cooperative, fuel-bounded async scheduler over fixed-capacity storage.
///
/// `NTASK` is the maximum number of concurrent tasks; `NREADY` is the ready-queue
/// capacity (size it `== NTASK`: the per-slot "in ready set at most once"
/// invariant means it can never overflow).
pub struct Scheduler<const NTASK: usize, const NREADY: usize> {
    tasks: TaskTable<NTASK>,
    ready: ReadyQueue<NREADY>,
    config: SchedConfig,
    /// When set, new-task admission is refused (`task.backpressure`).
    backpressure: bool,
}

impl<const NTASK: usize, const NREADY: usize> Scheduler<NTASK, NREADY> {
    /// Create an empty scheduler: all task slots free, ready queue empty.
    #[must_use]
    pub const fn new(config: SchedConfig) -> Self {
        Self {
            tasks: TaskTable::new(),
            ready: ReadyQueue::new(),
            config,
            backpressure: false,
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
        if self.backpressure {
            return Err(Error::async_task_spawn_failed(
                "kiln-async: admission refused, backpressure is enabled",
            ));
        }
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
    /// - `Running` → records the wake-pending flag and returns `Ok(true)`
    ///   (`Ok(false)` for a coalesced duplicate); the poll round consumes the
    ///   flag so a wake during the task's own poll slice is never lost.
    /// - `Spawned` → loud error: waking a never-admitted task is an FSM
    ///   violation.
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
            // Wake during the task's own poll slice: record it so the poll
            // round re-admits the task instead of parking it — a wakeup is
            // never lost. Duplicates coalesce (Ok(false)).
            Some(TaskState::Running) => self.tasks.flag_wake_pending(id),
        }
    }

    /// Dispatch the oldest ready task: dequeue it and drive `Ready -> Running`.
    ///
    /// Returns `Ok(None)` when the ready set is empty. Stale queue entries —
    /// tombstones left by [`Scheduler::cancel`] of a queued task — are
    /// *specified* and skipped; a live entry in any state other than `Ready`
    /// is an invariant violation and fails loud via the FSM transition.
    pub fn dispatch_next(&mut self) -> Result<Option<TaskId>> {
        while let Some(id) = self.ready.pop() {
            if self.tasks.state_of(id).is_none() {
                continue; // cancelled while queued: tombstone, skip
            }
            self.tasks.transition(id, TaskEvent::Dispatch)?;
            return Ok(Some(id));
        }
        Ok(None)
    }

    /// Cancel a task: drive it to `Cancelled` and free its slot.
    ///
    /// Valid for `Spawned`, `Ready`, and `Blocked` tasks; a `Ready` task's
    /// queue entry is left as a tombstone that dispatch skips. Cancelling the
    /// `Running` task is a loud error — the in-flight slice must end itself by
    /// returning [`TaskOutcome::Cancelled`] instead. A stale handle is a loud
    /// error too: cancel is a directed request, not a best-effort wake.
    pub fn cancel(&mut self, id: TaskId) -> Result<()> {
        if self.tasks.state_of(id) == Some(TaskState::Running) {
            return Err(Error::invalid_state_error(
                "kiln-async: cancel of the running task must end its slice as TaskOutcome::Cancelled",
            ));
        }
        self.tasks.transition(id, TaskEvent::Cancel)?;
        self.tasks.remove(id)
    }

    /// Gate admission of new tasks (the `task.backpressure` intrinsic).
    pub fn set_backpressure(&mut self, enable: bool) {
        self.backpressure = enable;
    }

    /// Run one cooperative poll round: dispatch the oldest ready task, run it
    /// for one fuel slice via `poll`, and re-dispatch per the outcome.
    ///
    /// `poll` is the engine bridge: it receives the scheduler itself (so the
    /// task's intrinsics can wake other tasks — or itself — mid-poll), the
    /// dispatched task, and the fuel budget for the slice. Outcomes:
    ///
    /// - [`TaskOutcome::Yielded`] → back to `Ready`, re-enqueued at the tail
    ///   (round-robin FIFO).
    /// - [`TaskOutcome::Waited`] → `Blocked` — unless a wake arrived during
    ///   the poll (the wake-pending flag), in which case it is re-admitted
    ///   immediately: **a wakeup is never lost**.
    /// - [`TaskOutcome::Completed`] → terminal; the slot is freed.
    ///
    /// A `poll` error propagates and leaves the task `Running` — fail loud;
    /// crash-recovery policy is the embedding's decision, not a silent retry.
    pub fn poll_round<F>(&mut self, poll: F) -> Result<PollRound>
    where
        F: FnOnce(&mut Self, TaskId, u64) -> Result<TaskOutcome>,
    {
        let Some(id) = self.dispatch_next()? else {
            return Ok(PollRound::Idle);
        };
        let outcome = poll(self, id, self.config.fuel_slice)?;
        // Consume the wake-pending flag for every outcome so it can't go
        // stale; it only changes the disposition of `Waited`.
        let woke_mid_poll = self.tasks.take_wake_pending(id)?;
        match outcome {
            TaskOutcome::Yielded => {
                self.ready.push(id)?;
                self.tasks.transition(id, TaskEvent::Yield)?;
            }
            TaskOutcome::Waited => {
                self.tasks.transition(id, TaskEvent::Wait)?;
                if woke_mid_poll {
                    // The wakeup raced the park: re-admit immediately.
                    self.mark_ready(id)?;
                }
            }
            TaskOutcome::Completed => {
                self.tasks.transition(id, TaskEvent::Complete)?;
                self.tasks.remove(id)?;
            }
            TaskOutcome::Cancelled => {
                self.tasks.transition(id, TaskEvent::Cancel)?;
                self.tasks.remove(id)?;
            }
        }
        Ok(PollRound::Polled(id))
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
    fn mark_ready_during_running_records_a_pending_wake() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.dispatch_next().unwrap();
        assert!(s.mark_ready(id).unwrap()); // recorded
        assert!(!s.mark_ready(id).unwrap()); // duplicate coalesced
        assert_eq!(s.ready_len(), 0); // not enqueued while Running
    }

    #[test]
    fn poll_round_on_an_empty_ready_set_is_idle() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let round = s
            .poll_round(|_, _, _| unreachable!("nothing to poll"))
            .unwrap();
        assert_eq!(round, PollRound::Idle);
    }

    #[test]
    fn poll_round_reenqueues_a_yielding_task_round_robin() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let a = s.spawn().unwrap();
        let b = s.spawn().unwrap();

        let round = s
            .poll_round(|sched, id, fuel| {
                assert_eq!(id, a); // FIFO: oldest first
                assert_eq!(fuel, SchedConfig::DEFAULT.fuel_slice);
                assert_eq!(sched.task_state(id), Some(TaskState::Running));
                Ok(TaskOutcome::Yielded)
            })
            .unwrap();

        assert_eq!(round, PollRound::Polled(a));
        assert_eq!(s.task_state(a), Some(TaskState::Ready));
        assert_eq!(s.ready_len(), 2);
        // Round-robin: a went to the tail, so b runs next.
        let next = s.poll_round(|_, id, _| {
            assert_eq!(id, b);
            Ok(TaskOutcome::Yielded)
        });
        assert_eq!(next.unwrap(), PollRound::Polled(b));
    }

    #[test]
    fn poll_round_parks_a_waiting_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();

        s.poll_round(|_, _, _| Ok(TaskOutcome::Waited)).unwrap();

        assert_eq!(s.task_state(id), Some(TaskState::Blocked));
        assert_eq!(s.ready_len(), 0);
        // ...and the normal wake path re-admits it.
        assert!(s.mark_ready(id).unwrap());
        assert_eq!(s.task_state(id), Some(TaskState::Ready));
    }

    #[test]
    fn poll_round_frees_a_completed_task() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();

        s.poll_round(|_, _, _| Ok(TaskOutcome::Completed)).unwrap();

        assert_eq!(s.task_count(), 0);
        assert!(s.task_state(id).is_none()); // slot freed, handle stale
    }

    #[test]
    fn a_wake_during_the_tasks_own_poll_is_never_lost() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();

        // The task wakes itself mid-poll (e.g. an already-ready future), then
        // reports Waited. Without the wake-pending flag this would deadlock.
        s.poll_round(|sched, tid, _| {
            assert!(sched.mark_ready(tid).unwrap());
            Ok(TaskOutcome::Waited)
        })
        .unwrap();

        assert_eq!(s.task_state(id), Some(TaskState::Ready));
        assert_eq!(s.ready_len(), 1);
    }

    #[test]
    fn a_cross_task_wake_during_a_poll_takes_effect() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let a = s.spawn().unwrap();
        let b = s.spawn().unwrap();
        // Park b first.
        s.poll_round(|_, id, _| {
            assert_eq!(id, a);
            Ok(TaskOutcome::Yielded)
        })
        .unwrap();
        s.poll_round(|_, id, _| {
            assert_eq!(id, b);
            Ok(TaskOutcome::Waited)
        })
        .unwrap();
        assert_eq!(s.task_state(b), Some(TaskState::Blocked));

        // a's intrinsic (e.g. future.write) wakes b mid-poll.
        s.poll_round(|sched, id, _| {
            assert_eq!(id, a);
            assert!(sched.mark_ready(b).unwrap());
            Ok(TaskOutcome::Yielded)
        })
        .unwrap();

        assert_eq!(s.task_state(b), Some(TaskState::Ready));
        assert_eq!(s.ready_len(), 2); // a re-enqueued + b woken
    }

    #[test]
    fn cancel_of_a_queued_task_leaves_a_tombstone_dispatch_skips() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let a = s.spawn().unwrap();
        let b = s.spawn().unwrap();

        s.cancel(a).unwrap();
        assert!(s.task_state(a).is_none()); // slot freed

        // Dispatch must skip a's tombstone and run b.
        assert_eq!(s.dispatch_next().unwrap(), Some(b));
        assert_eq!(s.task_state(b), Some(TaskState::Running));
    }

    #[test]
    fn cancel_of_a_blocked_task_frees_it() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.poll_round(|_, _, _| Ok(TaskOutcome::Waited)).unwrap();
        assert_eq!(s.task_state(id), Some(TaskState::Blocked));

        s.cancel(id).unwrap();

        assert!(s.task_state(id).is_none());
        assert!(!s.mark_ready(id).unwrap()); // late wake: benign spurious
    }

    #[test]
    fn cancel_of_the_running_task_fails_loud() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.dispatch_next().unwrap();
        // The in-flight slice must end itself as TaskOutcome::Cancelled.
        assert!(s.cancel(id).is_err());
        assert_eq!(s.task_state(id), Some(TaskState::Running)); // untouched
    }

    #[test]
    fn cancel_of_a_stale_handle_fails_loud() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.cancel(id).unwrap();
        assert!(s.cancel(id).is_err()); // already gone: directed op, not a wake
    }

    #[test]
    fn poll_outcome_cancelled_frees_the_slot() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        let id = s.spawn().unwrap();
        s.poll_round(|_, _, _| Ok(TaskOutcome::Cancelled)).unwrap();
        assert!(s.task_state(id).is_none());
        assert_eq!(s.task_count(), 0);
    }

    #[test]
    fn backpressure_gates_admission_and_releases() {
        let mut s = Sched::new(SchedConfig::DEFAULT);
        s.set_backpressure(true);
        assert!(s.spawn().is_err()); // refused, explicit error
        assert_eq!(s.task_count(), 0);

        s.set_backpressure(false);
        assert!(s.spawn().is_ok()); // admission restored
    }

    #[test]
    fn wake_pending_does_not_leak_across_slot_reuse() {
        let mut s: Scheduler<1, 1> = Scheduler::new(SchedConfig::DEFAULT);
        let old = s.spawn().unwrap();
        // Wake itself mid-poll, then complete: the pending flag must die with
        // the slot, not leak into the next occupant.
        s.poll_round(|sched, tid, _| {
            assert!(sched.mark_ready(tid).unwrap());
            Ok(TaskOutcome::Completed)
        })
        .unwrap();
        assert!(s.task_state(old).is_none());

        let new = s.spawn().unwrap();
        assert_eq!(new.index, old.index); // same slot reused
        s.poll_round(|_, _, _| Ok(TaskOutcome::Waited)).unwrap();
        // A leaked flag would have spuriously re-readied it.
        assert_eq!(s.task_state(new), Some(TaskState::Blocked));
    }
}
