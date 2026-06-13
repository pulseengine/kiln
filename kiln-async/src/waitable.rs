// Kiln - kiln-async :: waitable
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Bounded, no-alloc backing for Component Model **single-shot futures**.
//!
//! A `future<T>` is single-writer / single-reader and delivers exactly one
//! value. At the scheduler level the *payload* is the guest's concern (it moves
//! through linear memory via the engine); this table tracks only the
//! synchronization state and which task — if any — is blocked reading the
//! future, so the scheduler knows whom to wake when it is written.
//!
//! Lifecycle (single-shot):
//! ```text
//!   create ──▶ Empty ──register_waiter──▶ Empty(+waiter)
//!               │                              │
//!            complete                       complete  (wakes the waiter)
//!               ▼                              ▼
//!             Ready ──────────consume────────▶ (freed)
//!   any state ──drop──▶ (freed)
//! ```
//! `complete` (`future.write`) and `consume` (`future.read` of a ready future)
//! each fire at most once; violating that fails loud rather than silently
//! double-delivering.

use kiln_error::{Error, Result};

use crate::task::TaskId;

/// ABA-safe future handle: a slot index plus a generation counter
/// (mirrors [`crate::TaskId`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FutureId {
    /// Index into the future table.
    pub index: u16,
    /// Generation at the time this handle was issued.
    pub generation: u32,
}

impl FutureId {
    /// Sentinel "no future" handle.
    pub const NONE: Self = Self {
        index: u16::MAX,
        generation: 0,
    };

    /// Whether this is the [`FutureId::NONE`] sentinel.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.index == u16::MAX
    }
}

/// Synchronization state of a single-shot future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureState {
    /// Created; no value written yet (a reader may be parked — see `waiter`).
    Empty,
    /// Value written; not yet consumed by the reader.
    Ready,
}

/// One future table slot. `Copy` so the table is a plain `[FutureSlot; N]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FutureSlot {
    /// State, or `None` when the slot is free.
    state: Option<FutureState>,
    /// The reader parked on this future, or [`TaskId::NONE`].
    waiter: TaskId,
    /// Generation, bumped each time the slot is reused.
    generation: u32,
}

impl FutureSlot {
    const FREE: Self = Self {
        state: None,
        waiter: TaskId::NONE,
        generation: 0,
    };
}

/// Fixed-capacity table of single-shot futures, indexed by [`FutureId::index`].
///
/// No heap: a plain `[FutureSlot; N]`. The bounded-capacity / single-completion
/// invariants are the target of the Verus waitable proof.
pub struct FutureTable<const N: usize> {
    slots: [FutureSlot; N],
    live: usize,
}

impl<const N: usize> FutureTable<N> {
    /// Create an empty table — all `N` slots free.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [FutureSlot::FREE; N],
            live: usize::MIN,
        }
    }

    /// Capacity (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of live (allocated) futures.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether no futures are allocated.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Look up a future's state, validating the handle's generation.
    #[must_use]
    pub fn state_of(&self, id: FutureId) -> Option<FutureState> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.state
        } else {
            None
        }
    }

    /// Allocate a fresh `Empty` future (`future.new`). Fails loud at capacity.
    pub fn create(&mut self) -> Result<FutureId> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.state.is_none() {
                slot.state = Some(FutureState::Empty);
                slot.waiter = TaskId::NONE;
                self.live += 1;
                return Ok(FutureId {
                    index: index as u16,
                    generation: slot.generation,
                });
            }
        }
        Err(Error::foundation_bounded_capacity_exceeded(
            "kiln-async: future table at capacity",
        ))
    }

    /// Park a reader on an `Empty` future (the `future.read`-before-write path).
    ///
    /// Errors if the future is already `Ready` (the reader should consume, not
    /// block), already has a waiter (single-reader violation), or the handle is
    /// stale.
    pub fn register_waiter(&mut self, id: FutureId, reader: TaskId) -> Result<()> {
        let slot = self.live_slot_mut(id)?;
        match slot.state {
            Some(FutureState::Empty) => {
                if !slot.waiter.is_none() {
                    return Err(Error::validation_error(
                        "kiln-async: future already has a reader (single-reader)",
                    ));
                }
                slot.waiter = reader;
                Ok(())
            }
            // Ready: the reader should consume, not block.
            Some(FutureState::Ready) => Err(Error::invalid_state_error(
                "kiln-async: cannot park a reader on an already-ready future",
            )),
            None => unreachable!("live_slot_mut guarantees an occupied slot"),
        }
    }

    /// Complete a future (`future.write`): `Empty -> Ready`, returning the
    /// parked reader to wake (`None` if no reader is waiting yet).
    ///
    /// Errors if the future is already `Ready` (single-write violation) or the
    /// handle is stale.
    pub fn complete(&mut self, id: FutureId) -> Result<Option<TaskId>> {
        let slot = self.live_slot_mut(id)?;
        match slot.state {
            Some(FutureState::Empty) => {
                slot.state = Some(FutureState::Ready);
                Ok(Self::take_waiter(slot))
            }
            Some(FutureState::Ready) => Err(Error::invalid_state_error(
                "kiln-async: future already completed (single-shot write)",
            )),
            None => unreachable!("live_slot_mut guarantees an occupied slot"),
        }
    }

    /// Consume a `Ready` future (`future.read` completing) and free its slot,
    /// bumping the generation so stale handles are detectable.
    ///
    /// Errors if the future is not `Ready` or the handle is stale.
    pub fn consume(&mut self, id: FutureId) -> Result<()> {
        let slot = self.live_slot_mut(id)?;
        if slot.state != Some(FutureState::Ready) {
            return Err(Error::invalid_state_error(
                "kiln-async: consume of a future that is not ready",
            ));
        }
        Self::free(slot);
        self.live -= 1;
        Ok(())
    }

    /// Drop a future in any state (`future.cancel`), freeing its slot. Returns
    /// the parked reader, if any, so the scheduler can fail its pending read.
    /// Errors only on a stale handle.
    pub fn drop_future(&mut self, id: FutureId) -> Result<Option<TaskId>> {
        let slot = self.live_slot_mut(id)?;
        let waiter = Self::take_waiter(slot);
        Self::free(slot);
        self.live -= 1;
        Ok(waiter)
    }

    /// Generation-validated mutable access to a live (occupied) slot.
    fn live_slot_mut(&mut self, id: FutureId) -> Result<&mut FutureSlot> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|s| s.state.is_some() && s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: stale or unknown future handle")
            })
    }

    /// Take the parked reader out of a slot, returning it if one was set.
    fn take_waiter(slot: &mut FutureSlot) -> Option<TaskId> {
        if slot.waiter.is_none() {
            None
        } else {
            let w = slot.waiter;
            slot.waiter = TaskId::NONE;
            Some(w)
        }
    }

    /// Free a slot and bump its generation so stale handles are detectable.
    fn free(slot: &mut FutureSlot) {
        slot.state = None;
        slot.waiter = TaskId::NONE;
        slot.generation = slot.generation.wrapping_add(1);
    }
}

impl<const N: usize> Default for FutureTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// A bounded set of futures a task can wait on as a unit (`waitable-set`).
///
/// Backs `waitable-set.{new,join,drop}` and the `task.{wait,poll}` query: a
/// task blocked on the set is runnable as soon as **any** member is ready.
/// Fixed capacity `N`, no heap; membership is a plain `[FutureId; N]` prefix.
///
/// Phase 1 members are single-shot futures; streams join the same structure in
/// Phase 2. This type owns only the membership + the `poll` query; the blocking
/// `task.wait` wiring (parking the task, waking on member completion) layers on
/// top in the scheduler.
pub struct WaitableSet<const N: usize> {
    members: [FutureId; N],
    len: usize,
}

impl<const N: usize> WaitableSet<N> {
    /// Create an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            members: [FutureId::NONE; N],
            len: usize::MIN,
        }
    }

    /// Capacity (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of joined members.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the set has no members.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether `future` is a member of this set.
    #[must_use]
    pub fn contains(&self, future: FutureId) -> bool {
        self.members[..self.len].contains(&future)
    }

    /// `waitable-set.join` — add a future to the set.
    ///
    /// Fails loud at capacity, and rejects a duplicate join (a waitable belongs
    /// to one set at most once — a double join is a usage error, not a no-op).
    pub fn join(&mut self, future: FutureId) -> Result<()> {
        if self.contains(future) {
            return Err(Error::validation_error(
                "kiln-async: future already joined to this waitable-set",
            ));
        }
        if self.len == N {
            return Err(Error::foundation_bounded_capacity_exceeded(
                "kiln-async: waitable-set at capacity",
            ));
        }
        self.members[self.len] = future;
        self.len += 1;
        Ok(())
    }

    /// `waitable-set` membership removal (used by `drop`/completion cleanup).
    /// Returns whether `future` was present.
    pub fn remove(&mut self, future: FutureId) -> bool {
        let Some(pos) = self.members[..self.len].iter().position(|&m| m == future) else {
            return false;
        };
        // swap-remove: order in a wait set is not significant.
        self.len -= 1;
        self.members[pos] = self.members[self.len];
        self.members[self.len] = FutureId::NONE;
        true
    }

    /// `task.poll` — non-blocking: return the first member that is ready in
    /// `futures`, or `None` if none are ready yet. Pure query, no state change.
    #[must_use]
    pub fn poll_ready<const NF: usize>(&self, futures: &FutureTable<NF>) -> Option<FutureId> {
        self.members[..self.len]
            .iter()
            .copied()
            .find(|&m| futures.state_of(m) == Some(FutureState::Ready))
    }
}

impl<const N: usize> Default for WaitableSet<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(i: u16) -> TaskId {
        TaskId {
            index: i,
            generation: 0,
        }
    }

    #[test]
    fn table_starts_empty() {
        let t: FutureTable<4> = FutureTable::new();
        assert_eq!(t.capacity(), 4);
        assert!(t.is_empty());
        assert!(t.state_of(FutureId::NONE).is_none());
    }

    #[test]
    fn create_allocates_an_empty_future() {
        let mut t: FutureTable<4> = FutureTable::new();
        let f = t.create().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.state_of(f), Some(FutureState::Empty));
        assert!(!f.is_none());
    }

    #[test]
    fn create_uses_distinct_slots_and_errors_when_full() {
        let mut t: FutureTable<2> = FutureTable::new();
        let a = t.create().unwrap();
        let b = t.create().unwrap();
        assert_ne!(a.index, b.index);
        assert!(t.create().is_err()); // explicit capacity error, never silent
    }

    #[test]
    fn write_before_read_has_no_waiter_to_wake() {
        let mut t: FutureTable<2> = FutureTable::new();
        let f = t.create().unwrap();
        // Writer completes first: nobody parked yet.
        assert_eq!(t.complete(f).unwrap(), None);
        assert_eq!(t.state_of(f), Some(FutureState::Ready));
    }

    #[test]
    fn read_before_write_parks_then_write_wakes_that_reader() {
        let mut t: FutureTable<2> = FutureTable::new();
        let f = t.create().unwrap();
        t.register_waiter(f, task(7)).unwrap();
        // Writer completes: the parked reader is returned to be woken.
        assert_eq!(t.complete(f).unwrap(), Some(task(7)));
        assert_eq!(t.state_of(f), Some(FutureState::Ready));
    }

    #[test]
    fn consume_frees_the_slot_and_invalidates_the_handle() {
        let mut t: FutureTable<1> = FutureTable::new();
        let f = t.create().unwrap();
        t.complete(f).unwrap();
        t.consume(f).unwrap();
        assert!(t.is_empty());
        assert!(t.state_of(f).is_none()); // freed
        // Slot reuse bumps generation: the stale handle stays invalid (ABA).
        let g = t.create().unwrap();
        assert_eq!(g.index, f.index);
        assert_ne!(g.generation, f.generation);
        assert!(t.state_of(f).is_none());
    }

    #[test]
    fn consume_of_a_non_ready_future_fails_loud() {
        let mut t: FutureTable<1> = FutureTable::new();
        let f = t.create().unwrap();
        assert!(t.consume(f).is_err()); // still Empty
    }

    #[test]
    fn double_complete_fails_loud() {
        let mut t: FutureTable<1> = FutureTable::new();
        let f = t.create().unwrap();
        t.complete(f).unwrap();
        assert!(t.complete(f).is_err()); // single-shot: at most one write
    }

    #[test]
    fn register_waiter_rejects_a_ready_future_and_a_second_waiter() {
        let mut t: FutureTable<2> = FutureTable::new();
        let f = t.create().unwrap();
        t.register_waiter(f, task(1)).unwrap();
        assert!(t.register_waiter(f, task(2)).is_err()); // single-reader
        let g = t.create().unwrap();
        t.complete(g).unwrap();
        assert!(t.register_waiter(g, task(3)).is_err()); // already Ready
    }

    #[test]
    fn stale_handles_are_rejected_by_every_mutator() {
        let mut t: FutureTable<1> = FutureTable::new();
        assert!(t.complete(FutureId::NONE).is_err());
        assert!(t.consume(FutureId::NONE).is_err());
        assert!(t.register_waiter(FutureId::NONE, task(0)).is_err());
        assert!(t.drop_future(FutureId::NONE).is_err());
    }

    #[test]
    fn drop_future_frees_any_state_and_returns_a_parked_reader() {
        let mut t: FutureTable<2> = FutureTable::new();
        // Drop with a parked reader: returned so the scheduler can fail its read.
        let f = t.create().unwrap();
        t.register_waiter(f, task(5)).unwrap();
        assert_eq!(t.drop_future(f).unwrap(), Some(task(5)));
        assert!(t.state_of(f).is_none());
        // Drop a Ready future with no reader: no one to return.
        let g = t.create().unwrap();
        t.complete(g).unwrap();
        assert_eq!(t.drop_future(g).unwrap(), None);
        assert!(t.is_empty());
    }

    #[test]
    fn set_starts_empty_and_joins_members() {
        let mut t: FutureTable<4> = FutureTable::new();
        let a = t.create().unwrap();
        let b = t.create().unwrap();
        let mut set: WaitableSet<4> = WaitableSet::new();
        assert!(set.is_empty());
        set.join(a).unwrap();
        set.join(b).unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(a));
        assert!(set.contains(b));
    }

    #[test]
    fn set_join_rejects_duplicates_and_overflow() {
        let mut t: FutureTable<4> = FutureTable::new();
        let a = t.create().unwrap();
        let b = t.create().unwrap();
        let mut set: WaitableSet<1> = WaitableSet::new();
        set.join(a).unwrap();
        assert!(set.join(a).is_err()); // duplicate join is a usage error
        assert!(set.join(b).is_err()); // capacity 1: overflow fails loud
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn set_poll_returns_a_ready_member_else_none() {
        let mut t: FutureTable<4> = FutureTable::new();
        let a = t.create().unwrap();
        let b = t.create().unwrap();
        let mut set: WaitableSet<4> = WaitableSet::new();
        set.join(a).unwrap();
        set.join(b).unwrap();

        // Nothing ready yet.
        assert_eq!(set.poll_ready(&t), None);
        // Complete b → poll surfaces b.
        t.complete(b).unwrap();
        assert_eq!(set.poll_ready(&t), Some(b));
    }

    #[test]
    fn set_poll_ignores_ready_futures_that_are_not_members() {
        let mut t: FutureTable<4> = FutureTable::new();
        let member = t.create().unwrap();
        let outsider = t.create().unwrap();
        let mut set: WaitableSet<4> = WaitableSet::new();
        set.join(member).unwrap();

        t.complete(outsider).unwrap(); // ready, but not in the set
        assert_eq!(set.poll_ready(&t), None);
        t.complete(member).unwrap();
        assert_eq!(set.poll_ready(&t), Some(member));
    }

    #[test]
    fn set_remove_drops_membership() {
        let mut t: FutureTable<4> = FutureTable::new();
        let a = t.create().unwrap();
        let mut set: WaitableSet<4> = WaitableSet::new();
        set.join(a).unwrap();
        assert!(set.remove(a));
        assert!(!set.contains(a));
        assert!(set.is_empty());
        assert!(!set.remove(a)); // already gone
    }
}
