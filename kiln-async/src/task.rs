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
    /// A wake arrived while the task was `Running` (the wake-pending flag).
    /// Consumed by the poll round so the wakeup is never lost.
    wake_pending: bool,
}

impl TaskSlot {
    const FREE: Self = Self {
        state: None,
        generation: 0,
        wake_pending: false,
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
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.state.is_none() {
                slot.state = Some(TaskState::Spawned);
                self.live += 1;
                return Ok(TaskId {
                    // `index < N <= u16::MAX` by construction; u16::MAX is the
                    // NONE sentinel, so a table that large is unsupported.
                    index: index as u16,
                    generation: slot.generation,
                });
            }
        }
        Err(Error::foundation_bounded_capacity_exceeded(
            "kiln-async: task table at capacity",
        ))
    }

    /// Record that a wake arrived for a live task (used while it is `Running`).
    /// Returns whether the flag was newly set (`false` = duplicate, coalesced).
    pub(crate) fn flag_wake_pending(&mut self, id: TaskId) -> Result<bool> {
        let slot = self.live_slot_mut(id)?;
        let newly_set = !slot.wake_pending;
        slot.wake_pending = true;
        Ok(newly_set)
    }

    /// Consume a task's wake-pending flag, returning whether it was set.
    pub(crate) fn take_wake_pending(&mut self, id: TaskId) -> Result<bool> {
        let slot = self.live_slot_mut(id)?;
        let was_set = slot.wake_pending;
        slot.wake_pending = false;
        Ok(was_set)
    }

    /// Generation-validated mutable access to a live (occupied) slot.
    fn live_slot_mut(&mut self, id: TaskId) -> Result<&mut TaskSlot> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|s| s.state.is_some() && s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: stale or unknown task handle")
            })
    }

    /// Drive a task slot through one FSM transition, validating the handle.
    ///
    /// The table owns slot state, so all state changes go through the
    /// [`TaskState::step`] FSM here — a slot can never hold an FSM-invalid
    /// state, and an illegal event fails loud rather than mutating the slot.
    /// Returns the new state on success.
    pub fn transition(&mut self, id: TaskId, event: TaskEvent) -> Result<TaskState> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .filter(|s| s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: transition on a stale or unknown task handle")
            })?;
        let current = slot.state.ok_or_else(|| {
            Error::validation_error("kiln-async: transition on a free task slot")
        })?;
        // `step` validates the event; an illegal transition errors here and the
        // slot is left untouched (no partial write).
        let next = current.step(event)?;
        slot.state = Some(next);
        Ok(next)
    }

    /// Free a task slot, validating the handle, and bump its generation so any
    /// stale [`TaskId`] referring to the old occupant is detectably invalid.
    pub fn remove(&mut self, id: TaskId) -> Result<()> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .filter(|s| s.state.is_some() && s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: remove of a stale or unknown task handle")
            })?;
        slot.state = None;
        slot.generation = slot.generation.wrapping_add(1);
        slot.wake_pending = false; // the flag dies with the slot, never leaks
        self.live -= 1;
        Ok(())
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
    fn table_starts_empty() {
        let t: TaskTable<8> = TaskTable::new();
        assert_eq!(t.capacity(), 8);
        assert!(t.is_empty());
        assert!(t.state_of(TaskId::NONE).is_none());
    }

    #[test]
    fn spawn_allocates_a_spawned_slot() {
        let mut t: TaskTable<4> = TaskTable::new();
        let id = t.spawn().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.state_of(id), Some(TaskState::Spawned));
        assert!(!id.is_none());
    }

    #[test]
    fn spawn_uses_distinct_slots() {
        let mut t: TaskTable<4> = TaskTable::new();
        let a = t.spawn().unwrap();
        let b = t.spawn().unwrap();
        assert_ne!(a.index, b.index);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn spawn_errors_when_full_rather_than_overwriting() {
        let mut t: TaskTable<2> = TaskTable::new();
        t.spawn().unwrap();
        t.spawn().unwrap();
        assert!(t.spawn().is_err()); // explicit capacity error, never silent reuse
    }

    #[test]
    fn remove_frees_the_slot() {
        let mut t: TaskTable<2> = TaskTable::new();
        let id = t.spawn().unwrap();
        t.remove(id).unwrap();
        assert!(t.is_empty());
        assert!(t.state_of(id).is_none());
    }

    #[test]
    fn remove_bumps_generation_so_stale_handles_are_invalid() {
        let mut t: TaskTable<1> = TaskTable::new();
        let old = t.spawn().unwrap();
        t.remove(old).unwrap();
        // Slot reused: the new handle is valid, the stale one is not (ABA-safe).
        let new = t.spawn().unwrap();
        assert_eq!(new.index, old.index);
        assert_ne!(new.generation, old.generation);
        assert_eq!(t.state_of(new), Some(TaskState::Spawned));
        assert!(t.state_of(old).is_none());
    }

    #[test]
    fn transition_drives_a_slot_through_a_valid_event() {
        let mut t: TaskTable<2> = TaskTable::new();
        let id = t.spawn().unwrap();
        assert_eq!(t.state_of(id), Some(TaskState::Spawned));
        let next = t.transition(id, TaskEvent::Admit).unwrap();
        assert_eq!(next, TaskState::Ready);
        assert_eq!(t.state_of(id), Some(TaskState::Ready));
    }

    #[test]
    fn transition_rejects_an_illegal_event_without_mutating() {
        let mut t: TaskTable<2> = TaskTable::new();
        let id = t.spawn().unwrap();
        // Spawned --Complete--> is illegal (must be Admit'd then run first).
        assert!(t.transition(id, TaskEvent::Complete).is_err());
        // Slot state is unchanged — fail-loud, never a silent half-transition.
        assert_eq!(t.state_of(id), Some(TaskState::Spawned));
    }

    #[test]
    fn transition_rejects_a_stale_or_unknown_handle() {
        let mut t: TaskTable<1> = TaskTable::new();
        assert!(t.transition(TaskId::NONE, TaskEvent::Admit).is_err());
        let id = t.spawn().unwrap();
        t.remove(id).unwrap();
        assert!(t.transition(id, TaskEvent::Admit).is_err()); // freed slot
    }

    #[test]
    fn remove_rejects_stale_or_unknown_handle() {
        let mut t: TaskTable<1> = TaskTable::new();
        assert!(t.remove(TaskId::NONE).is_err());
        let id = t.spawn().unwrap();
        t.remove(id).unwrap();
        assert!(t.remove(id).is_err()); // already freed
    }
}
