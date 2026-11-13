//! Scheduling of P3 cooperative threads for one synchronous task (SR-82).
//!
//! meld passes the nine cooperative-thread built-ins through to the host as
//! `$root::[thread-*]` imports in the producer ABI. The stackless engine's
//! trampoline keeps the running thread's frames; this module keeps everything
//! else: the threads that are not running, their per-owner handle tables, the
//! ready queue and the rules for picking the next thread.
//!
//! The rules follow the Component Model as wasmtime implements it
//! (`concurrent.rs`: `suspension_intrinsic`, `resume_thread`):
//!
//! - Each owner (component instance) has its own handle table. The task's
//!   implicit thread is handle 1 in every table, entered when the table is
//!   first used; `new-indirect` hands out the next handle.
//! - `resume-later` and a yielding thread go to the back of one FIFO ready
//!   queue. `*-then-resume` names the next thread directly; `*-then-promote`
//!   does so only when the named thread is ready, and is otherwise a plain
//!   yield or suspend.
//! - With nothing named, the first ready thread of the caller's owner runs
//!   next. If there is none, a yield returns at once and a suspend traps:
//!   kiln runs every export as a synchronous task, which may not block before
//!   it returns. An explicit thread that finishes with nothing ready traps the
//!   same way.
//!
//! The scheduler is generic over the frame type so that it can be tested
//! without an engine.

use std::{
    collections::{HashMap, VecDeque},
    vec::Vec,
};

use kiln_error::{Error, Result};
use kiln_foundation::values::Value;

/// The trap message of a synchronous task that would block (wasmtime's
/// `Trap::CannotBlockSyncTask`).
pub(crate) const CANNOT_BLOCK_SYNC_TASK: &str = "cannot block a synchronous task before returning";

/// Most threads one task may have alive at once, the implicit thread
/// included. A not-started thread costs no frames, so without this cap a
/// spawn loop would grow host memory without bound; the call-depth limit
/// (which counts parked frames, see the engine) bounds the rest.
pub(crate) const MAX_COOP_THREADS: usize = 1024;

/// One of the nine cooperative-thread built-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadOp {
    /// `[thread-new-indirect-v0]` `(fn_idx: i32, arg: i32) -> i32`
    NewIndirect,
    /// `[thread-index]` `() -> i32`
    Index,
    /// `[thread-yield]` `() -> i32`
    Yield,
    /// `[thread-suspend]` `() -> i32`
    Suspend,
    /// `[thread-resume-later]` `(i32) -> ()`
    ResumeLater,
    /// `[thread-yield-then-resume]` `(i32) -> i32`
    YieldThenResume,
    /// `[thread-suspend-then-resume]` `(i32) -> i32`
    SuspendThenResume,
    /// `[thread-yield-then-promote]` `(i32) -> i32`
    YieldThenPromote,
    /// `[thread-suspend-then-promote]` `(i32) -> i32`
    SuspendThenPromote,
}

impl ThreadOp {
    /// The built-in a `$root` import field (instance suffix already removed)
    /// names, if it is one of the nine.
    pub(crate) fn from_base_name(name: &str) -> Option<Self> {
        Some(match name {
            "[thread-new-indirect-v0]" => Self::NewIndirect,
            "[thread-index]" => Self::Index,
            "[thread-yield]" => Self::Yield,
            "[thread-suspend]" => Self::Suspend,
            "[thread-resume-later]" => Self::ResumeLater,
            "[thread-yield-then-resume]" => Self::YieldThenResume,
            "[thread-suspend-then-resume]" => Self::SuspendThenResume,
            "[thread-yield-then-promote]" => Self::YieldThenPromote,
            "[thread-suspend-then-promote]" => Self::SuspendThenPromote,
            _ => return None,
        })
    }

    /// For the built-ins that give up the running thread: whether the caller
    /// stays ready (yield) or is suspended, and which thread it names.
    pub(crate) fn suspension(self, handle: Option<u32>) -> Option<(bool, Target)> {
        let named = |make: fn(u32) -> Target| handle.map(make);
        match self {
            Self::Yield => Some((true, Target::None)),
            Self::Suspend => Some((false, Target::None)),
            Self::YieldThenResume => named(Target::Resume).map(|t| (true, t)),
            Self::SuspendThenResume => named(Target::Resume).map(|t| (false, t)),
            Self::YieldThenPromote => named(Target::Promote).map(|t| (true, t)),
            Self::SuspendThenPromote => named(Target::Promote).map(|t| (false, t)),
            Self::NewIndirect | Self::Index | Self::ResumeLater => None,
        }
    }
}

/// The thread a suspension built-in names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// None: the first ready thread runs next, if any.
    None,
    /// `*-then-resume`: this suspended thread runs next.
    Resume(u32),
    /// `*-then-promote`: this thread runs next if it is ready.
    Promote(u32),
}

/// What a thread does when it next runs.
#[derive(Debug)]
pub(crate) enum Continuation<F> {
    /// Not started: call `func_idx` of `instance_id` with `[arg]`.
    Start {
        /// Engine instance of the start function.
        instance_id: usize,
        /// Function index of the start function.
        func_idx: usize,
        /// The `arg` passed to `[thread-new-indirect-v0]`.
        arg: i32,
    },
    /// Parked: these frames (innermost last), resumed with `results` as the
    /// return values of the built-in call that parked them.
    Resume {
        /// The thread's whole frame stack.
        frames: Vec<F>,
        /// Return values of the parking built-in.
        results: Vec<Value>,
    },
}

#[derive(Debug)]
enum State<F> {
    Running,
    Ready(Continuation<F>),
    Suspended(Continuation<F>),
}

#[derive(Debug)]
struct Thread<F> {
    /// The owner whose table holds the thread; `None` for the implicit
    /// thread, which is in every table.
    owner: Option<u32>,
    state: State<F>,
}

type ThreadId = u32;

/// The implicit thread: the one running the export that was called.
const IMPLICIT: ThreadId = 0;

/// The cooperative threads of one synchronous task.
#[derive(Debug)]
pub(crate) struct CoopScheduler<F> {
    threads: HashMap<ThreadId, Thread<F>>,
    /// Per owner: handle -> thread.
    tables: HashMap<u32, HashMap<u32, ThreadId>>,
    /// Per owner: the next handle to hand out.
    next_handle: HashMap<u32, u32>,
    ready: VecDeque<ThreadId>,
    current: ThreadId,
    next_id: ThreadId,
    /// Frames held by parked threads. The engine keeps them counted in its
    /// call depth while parked and uncounts them when the task ends.
    parked_frames: usize,
}

impl<F> CoopScheduler<F> {
    /// A task whose implicit thread is running and has no other threads.
    pub(crate) fn new() -> Self {
        let mut threads = HashMap::new();
        threads.insert(
            IMPLICIT,
            Thread {
                owner: None,
                state: State::Running,
            },
        );
        Self {
            threads,
            tables: HashMap::new(),
            next_handle: HashMap::new(),
            ready: VecDeque::new(),
            current: IMPLICIT,
            next_id: IMPLICIT + 1,
            parked_frames: 0,
        }
    }

    /// Frames held by threads that are parked (not running).
    pub(crate) fn parked_frames(&self) -> usize {
        self.parked_frames
    }

    /// Whether the running thread is the task's implicit thread.
    pub(crate) fn implicit_is_running(&self) -> bool {
        self.current == IMPLICIT
    }

    /// Number of threads that are ready but have not run.
    pub(crate) fn ready_count(&self) -> usize {
        self.ready.len()
    }

    /// The owner's table, created with the implicit thread as handle 1.
    fn table(&mut self, owner: u32) -> &mut HashMap<u32, ThreadId> {
        self.tables.entry(owner).or_insert_with(|| {
            let mut table = HashMap::new();
            table.insert(1, IMPLICIT);
            self.next_handle.insert(owner, 2);
            table
        })
    }

    fn lookup(&mut self, owner: u32, handle: u32) -> Result<ThreadId> {
        self.table(owner)
            .get(&handle)
            .copied()
            .ok_or_else(|| Error::runtime_trap("P3 thread built-in: unknown thread handle"))
    }

    fn handle_of(&mut self, owner: u32, id: ThreadId) -> Result<u32> {
        self.table(owner)
            .iter()
            .find_map(|(h, t)| (*t == id).then_some(*h))
            .ok_or_else(|| {
                Error::not_implemented_error(
                    "P3 [thread-index]: the running thread belongs to another component \
                     instance; a cross-instance thread call needs a task boundary kiln does not \
                     model",
                )
            })
    }

    /// `[thread-index]`: the running thread's handle in `owner`'s table.
    pub(crate) fn index(&mut self, owner: u32) -> Result<u32> {
        let current = self.current;
        self.handle_of(owner, current)
    }

    /// `[thread-new-indirect-v0]`: a new, not started thread; returns its handle.
    pub(crate) fn new_thread(&mut self, owner: u32, start: Continuation<F>) -> Result<u32> {
        if self.threads.len() >= MAX_COOP_THREADS {
            return Err(Error::runtime_trap(
                "P3 [thread-new-indirect-v0]: too many live threads in this task (limit 1024)",
            ));
        }
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| Error::runtime_trap("P3 [thread-new-indirect-v0]: too many threads"))?;
        self.table(owner);
        let handle = *self.next_handle.get(&owner).ok_or_else(|| {
            Error::runtime_error("P3 thread table has no next handle (scheduler bug)")
        })?;
        let following = handle.checked_add(1).ok_or_else(|| {
            Error::runtime_trap("P3 [thread-new-indirect-v0]: thread handles exhausted")
        })?;
        self.next_handle.insert(owner, following);
        self.table(owner).insert(handle, id);
        self.threads.insert(
            id,
            Thread {
                owner: Some(owner),
                state: State::Suspended(start),
            },
        );
        Ok(handle)
    }

    /// The thread `handle` names, which must be suspended (or not started)
    /// and not the running thread.
    fn resumable(&mut self, owner: u32, handle: u32) -> Result<ThreadId> {
        let id = self.lookup(owner, handle)?;
        if id == self.current {
            return Err(Error::runtime_trap(
                "P3 thread built-in: cannot resume the running thread",
            ));
        }
        match self.threads.get(&id).map(|t| &t.state) {
            Some(State::Suspended(_)) => Ok(id),
            Some(State::Ready(_) | State::Running) => Err(Error::runtime_trap(
                "P3 thread built-in: cannot resume a thread that is not suspended",
            )),
            None => Err(Error::runtime_trap(
                "P3 thread built-in: unknown thread handle",
            )),
        }
    }

    /// Move a suspended thread's continuation to the ready state.
    fn make_ready(&mut self, id: ThreadId) -> Result<()> {
        let thread = self
            .threads
            .get_mut(&id)
            .ok_or_else(|| Error::runtime_trap("P3 thread built-in: unknown thread handle"))?;
        let state = core::mem::replace(&mut thread.state, State::Running);
        match state {
            State::Suspended(cont) => {
                thread.state = State::Ready(cont);
                Ok(())
            },
            other => {
                thread.state = other;
                Err(Error::runtime_trap(
                    "P3 thread built-in: cannot resume a thread that is not suspended",
                ))
            },
        }
    }

    /// `[thread-resume-later]`: the suspended thread joins the back of the
    /// ready queue.
    pub(crate) fn resume_later(&mut self, owner: u32, handle: u32) -> Result<()> {
        let id = self.resumable(owner, handle)?;
        self.make_ready(id)?;
        self.ready.push_back(id);
        Ok(())
    }

    /// The first ready thread `owner` may switch to (its own threads and the
    /// implicit thread), removed from the queue.
    fn take_first_ready(&mut self, owner: Option<u32>) -> Option<ThreadId> {
        let threads = &self.threads;
        let pos = self.ready.iter().position(|id| {
            let thread_owner = threads.get(id).and_then(|t| t.owner);
            owner.is_none() || thread_owner.is_none() || thread_owner == owner
        })?;
        self.ready.remove(pos)
    }

    /// Decide the thread a suspension built-in switches to.
    ///
    /// `Ok(None)`: nothing else can run and the caller yields, so the built-in
    /// returns at once. A suspend with nothing to run traps with
    /// [`CANNOT_BLOCK_SYNC_TASK`].
    pub(crate) fn pick_next(
        &mut self,
        owner: u32,
        yielding: bool,
        target: Target,
    ) -> Result<Option<ThreadId>> {
        match target {
            Target::None => {},
            Target::Resume(handle) => {
                let id = self.resumable(owner, handle)?;
                return Ok(Some(id));
            },
            Target::Promote(handle) => {
                let id = self.lookup(owner, handle)?;
                if id == self.current {
                    return Err(Error::runtime_trap(
                        "P3 thread built-in: cannot resume the running thread",
                    ));
                }
                if let Some(pos) = self.ready.iter().position(|r| *r == id) {
                    self.ready.remove(pos);
                    return Ok(Some(id));
                }
            },
        }
        match self.take_first_ready(Some(owner)) {
            Some(id) => Ok(Some(id)),
            None if yielding => Ok(None),
            None => Err(Error::runtime_trap(CANNOT_BLOCK_SYNC_TASK)),
        }
    }

    /// Park the running thread (ready at the back of the queue when it
    /// yields, suspended otherwise) and make `next` the running thread.
    /// Returns what `next` runs.
    pub(crate) fn switch_to(
        &mut self,
        next: ThreadId,
        parked: Continuation<F>,
        yielding: bool,
    ) -> Result<Continuation<F>> {
        let current = self.current;
        if let Continuation::Resume { frames, .. } = &parked {
            self.parked_frames = self.parked_frames.checked_add(frames.len()).ok_or_else(|| {
                Error::runtime_error("P3 parked frame count overflow (scheduler bug)")
            })?;
        }
        let thread = self
            .threads
            .get_mut(&current)
            .ok_or_else(|| Error::runtime_error("P3 running thread vanished (scheduler bug)"))?;
        if yielding {
            thread.state = State::Ready(parked);
            self.ready.push_back(current);
        } else {
            thread.state = State::Suspended(parked);
        }
        self.run(next)
    }

    /// Make `id` the running thread and return its continuation.
    fn run(&mut self, id: ThreadId) -> Result<Continuation<F>> {
        let thread = self
            .threads
            .get_mut(&id)
            .ok_or_else(|| Error::runtime_trap("P3 thread built-in: unknown thread handle"))?;
        match core::mem::replace(&mut thread.state, State::Running) {
            State::Ready(cont) | State::Suspended(cont) => {
                if let Continuation::Resume { frames, .. } = &cont {
                    self.parked_frames =
                        self.parked_frames.checked_sub(frames.len()).ok_or_else(|| {
                            Error::runtime_error("P3 parked frame count underflow (scheduler bug)")
                        })?;
                }
                self.current = id;
                Ok(cont)
            },
            State::Running => Err(Error::runtime_error(
                "P3 thread to run is already running (scheduler bug)",
            )),
        }
    }

    /// The running explicit thread's start function returned: remove it and
    /// run the first ready thread, or trap with [`CANNOT_BLOCK_SYNC_TASK`]
    /// when there is none.
    pub(crate) fn exit_current(&mut self) -> Result<Continuation<F>> {
        let current = self.current;
        if current == IMPLICIT {
            return Err(Error::runtime_error(
                "P3 implicit thread cannot exit as an explicit thread (scheduler bug)",
            ));
        }
        let owner = self.threads.remove(&current).and_then(|t| t.owner);
        if let Some(owner) = owner {
            if let Some(table) = self.tables.get_mut(&owner) {
                table.retain(|_, t| *t != current);
            }
        }
        match self.take_first_ready(owner) {
            Some(next) => self.run(next),
            None => Err(Error::runtime_trap(CANNOT_BLOCK_SYNC_TASK)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Sched = CoopScheduler<u8>;

    fn start() -> Continuation<u8> {
        Continuation::Start {
            instance_id: 1,
            func_idx: 6,
            arg: 7,
        }
    }

    fn parked() -> Continuation<u8> {
        Continuation::Resume {
            frames: vec![1],
            results: vec![Value::I32(0)],
        }
    }

    // rivet: verifies SR-82
    #[test]
    fn implicit_thread_is_handle_1_and_new_threads_follow() -> Result<()> {
        let mut s = Sched::new();
        assert_eq!(s.index(0)?, 1);
        assert_eq!(s.new_thread(0, start())?, 2);
        assert_eq!(s.new_thread(0, start())?, 3);
        // Another owner's table starts again at 1 for the implicit thread.
        assert_eq!(s.index(5)?, 1);
        assert_eq!(s.new_thread(5, start())?, 2);
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn yield_with_nothing_ready_returns_and_suspend_traps() -> Result<()> {
        let mut s = Sched::new();
        assert_eq!(s.pick_next(0, true, Target::None)?, None);
        let err = s.pick_next(0, false, Target::None).err();
        assert!(err.is_some_and(|e| e.message().contains(CANNOT_BLOCK_SYNC_TASK)));
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn resume_later_then_yield_runs_the_thread_and_requeues_the_caller() -> Result<()> {
        let mut s = Sched::new();
        let t = s.new_thread(0, start())?;
        s.resume_later(0, t)?;
        // Resuming a ready thread again is a trap.
        assert!(s.resume_later(0, t).is_err());
        let next = s.pick_next(0, true, Target::None)?;
        assert!(next.is_some());
        let cont = s.switch_to(next.unwrap_or_default(), parked(), true)?;
        assert!(matches!(cont, Continuation::Start { arg: 7, .. }));
        assert!(!s.implicit_is_running());
        assert_eq!(s.index(0)?, t);
        // The worker exits; the yielded implicit thread runs again.
        let back = s.exit_current()?;
        assert!(matches!(back, Continuation::Resume { .. }));
        assert!(s.implicit_is_running());
        assert_eq!(s.ready_count(), 0);
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn suspend_then_resume_and_exit_with_nothing_ready_traps() -> Result<()> {
        let mut s = Sched::new();
        let t = s.new_thread(0, start())?;
        let next = s.pick_next(0, false, Target::Resume(t))?;
        assert!(next.is_some());
        s.switch_to(next.unwrap_or_default(), parked(), false)?;
        let err = s.exit_current().err();
        assert!(err.is_some_and(|e| e.message().contains(CANNOT_BLOCK_SYNC_TASK)));
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn promote_of_a_thread_that_is_not_ready_is_a_plain_yield_or_suspend() -> Result<()> {
        let mut s = Sched::new();
        let t = s.new_thread(0, start())?;
        assert_eq!(s.pick_next(0, true, Target::Promote(t))?, None);
        assert!(s.pick_next(0, false, Target::Promote(t)).is_err());
        s.resume_later(0, t)?;
        assert!(s.pick_next(0, false, Target::Promote(t))?.is_some());
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn live_threads_are_capped() -> Result<()> {
        let mut s = Sched::new();
        // The implicit thread is one of the MAX_COOP_THREADS.
        for _ in 1..MAX_COOP_THREADS {
            s.new_thread(0, start())?;
        }
        let err = s.new_thread(0, start()).err();
        assert!(err.is_some_and(|e| e.message().contains("too many live threads")));
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn parked_frames_are_counted_until_the_thread_runs_again() -> Result<()> {
        let mut s = Sched::new();
        let t = s.new_thread(0, start())?;
        let next = s.pick_next(0, false, Target::Resume(t))?;
        let three = Continuation::Resume {
            frames: vec![1, 2, 3],
            results: vec![Value::I32(0)],
        };
        s.switch_to(next.unwrap_or_default(), three, false)?;
        assert_eq!(s.parked_frames(), 3);
        // The worker resumes the implicit thread and exits: its frames are
        // running again, so they no longer count as parked.
        s.resume_later(0, 1)?;
        s.exit_current()?;
        assert_eq!(s.parked_frames(), 0);
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn bad_handles_and_resuming_the_running_thread_trap() {
        let mut s = Sched::new();
        assert!(s.resume_later(0, 42).is_err());
        assert!(s.pick_next(0, true, Target::Resume(42)).is_err());
        // Handle 1 is the running implicit thread.
        assert!(s.resume_later(0, 1).is_err());
        assert!(s.pick_next(0, true, Target::Promote(1)).is_err());
    }
}
