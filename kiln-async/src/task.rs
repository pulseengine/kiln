// Kiln - kiln-async :: task
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Task identity, the lifecycle state machine, and the fixed-capacity task table.

use kiln_error::{Error, Result};

/// ABA-safe task handle: a slot index plus a generation counter.
///
/// Reusing a slot bumps its generation, so a stale [`TaskId`] referring to a
/// freed-and-reused slot is detectably invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId {
    /// Index into the task table.
    pub index: u16,
    /// Generation at the time this handle was issued.
    pub generation: u32,
}

impl TaskId {
    /// Sentinel "no task" handle (used as the empty ready-queue fill value).
    pub const NONE: Self = Self {
        index: u16::MAX,
        generation: 0,
    };

    /// Whether this is the [`TaskId::NONE`] sentinel.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.index == u16::MAX
    }
}

/// Task lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Created, not yet admitted to the ready set.
    Spawned,
    /// Eligible to run, in the ready queue.
    Ready,
    /// Currently being polled.
    Running,
    /// Parked on a waitable (future/stream/set).
    Blocked,
    /// Finished successfully (terminal).
    Completed,
    /// Cancelled (terminal).
    Cancelled,
}

/// Events that drive [`TaskState`] transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEvent {
    /// Admit a freshly spawned task to the ready set.
    Admit,
    /// Dispatch a ready task to be polled.
    Dispatch,
    /// A running task yielded cooperatively.
    Yield,
    /// A running task parked on a waitable.
    Wait,
    /// A blocked task's waitable fired.
    Wake,
    /// A running task finished.
    Complete,
    /// Cancel a non-terminal task.
    Cancel,
}

impl TaskState {
    /// Total lifecycle transition function.
    ///
    /// Illegal transitions return an explicit error (fail-loud — never a silent
    /// default). This pure function is the target of the Verus FSM-soundness
    /// proof (property P3 in the design doc).
    pub fn step(self, event: TaskEvent) -> Result<Self> {
        use TaskEvent::{Admit, Cancel, Complete, Dispatch, Wait, Wake, Yield};
        use TaskState::{Blocked, Cancelled, Completed, Ready, Running, Spawned};

        let next = match (self, event) {
            (Spawned, Admit) => Ready,
            (Ready, Dispatch) => Running,
            (Running, Yield) => Ready,
            (Running, Wait) => Blocked,
            (Running, Complete) => Completed,
            (Blocked, Wake) => Ready,
            (Spawned | Ready | Running | Blocked, Cancel) => Cancelled,
            _ => {
                return Err(Error::invalid_state_error(
                    "kiln-async: illegal task lifecycle transition",
                ));
            }
        };
        Ok(next)
    }

    /// Whether this is a terminal state (no further transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

/// One task table slot. `Copy` so the table is a plain `[TaskSlot; N]` with no
/// heap and a `const` initializer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskSlot {
    /// Lifecycle state, or `None` when the slot is free.
    state: Option<TaskState>,
    /// Generation, bumped each time the slot is reused.
    generation: u32,
}

impl TaskSlot {
    const FREE: Self = Self {
        state: None,
        generation: 0,
    };
}

/// Fixed-capacity table of tasks, indexed by [`TaskId::index`].
///
/// No heap: a plain `[TaskSlot; N]`. Phase 1 adds the intrusive free-list and
/// `spawn`/`remove`; Phase 0 provides the storage, capacity accounting, and the
/// generation-checked lookup.
pub struct TaskTable<const N: usize> {
    slots: [TaskSlot; N],
    live: usize,
}

impl<const N: usize> TaskTable<N> {
    /// Create an empty table — all `N` slots free.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [TaskSlot::FREE; N],
            live: usize::MIN,
        }
    }

    /// Capacity (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of live (occupied) slots.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether no slots are occupied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Look up a task's state, validating the handle's generation.
    ///
    /// Returns `None` for an out-of-range index, a free slot, or a stale
    /// generation (the slot was reused since the handle was issued).
    #[must_use]
    pub fn state_of(&self, id: TaskId) -> Option<TaskState> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.state
        } else {
            None
        }
    }

    /// Admit a new task into a free slot.
    ///
    /// **Phase 1** — not yet implemented (the slot's poll thunk and waker wiring
    /// land with the executor). Returns an explicit error, not a silent `Ok`.
    pub fn spawn(&mut self) -> Result<TaskId> {
        Err(Error::not_implemented_error(
            "kiln-async: TaskTable::spawn is implemented in Phase 1",
        ))
    }
}

impl<const N: usize> Default for TaskTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsm_happy_path() {
        let s = TaskState::Spawned
            .step(TaskEvent::Admit)
            .unwrap();
        assert_eq!(s, TaskState::Ready);
        let s = s.step(TaskEvent::Dispatch).unwrap();
        assert_eq!(s, TaskState::Running);
        let s = s.step(TaskEvent::Wait).unwrap();
        assert_eq!(s, TaskState::Blocked);
        let s = s.step(TaskEvent::Wake).unwrap();
        assert_eq!(s, TaskState::Ready);
        let s = s.step(TaskEvent::Dispatch).unwrap();
        let s = s.step(TaskEvent::Complete).unwrap();
        assert_eq!(s, TaskState::Completed);
        assert!(s.is_terminal());
    }

    #[test]
    fn fsm_illegal_transitions_error() {
        assert!(TaskState::Completed.step(TaskEvent::Dispatch).is_err());
        assert!(TaskState::Spawned.step(TaskEvent::Complete).is_err());
        assert!(TaskState::Cancelled.step(TaskEvent::Wake).is_err());
    }

    #[test]
    fn cancel_from_any_nonterminal() {
        for s in [
            TaskState::Spawned,
            TaskState::Ready,
            TaskState::Running,
            TaskState::Blocked,
        ] {
            assert_eq!(s.step(TaskEvent::Cancel).unwrap(), TaskState::Cancelled);
        }
    }

    #[test]
    fn taskid_none_is_sentinel() {
        assert!(TaskId::NONE.is_none());
        assert!(!TaskId { index: 0, generation: 0 }.is_none());
    }

    #[test]
    fn table_starts_empty_and_spawn_is_phase1() {
        let t: TaskTable<8> = TaskTable::new();
        assert_eq!(t.capacity(), 8);
        assert!(t.is_empty());
        assert!(t.state_of(TaskId::NONE).is_none());
        let mut t = t;
        assert!(t.spawn().is_err()); // explicit not-implemented, never silent Ok
    }
}
