// Kiln - kiln-async :: intrinsics
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! WASM Component Model **P3 async ABI** surface.
//!
//! These functions enumerate the canonical-ABI intrinsics the scheduler backs
//! on the embedded path (per RFC #46, Meld lowers the component-level async
//! constructs to these core import calls).
//!
//! ## Implemented — engine-bridge mapping (no stub here)
//!
//! The task-control intrinsics are scheduler capabilities; the engine bridge
//! dispatches them directly:
//!
//! - `task.yield` → end the slice with [`crate::TaskOutcome::Yielded`].
//! - `task.cancel(subtask)` → [`crate::Scheduler::cancel`]; a task cancelling
//!   *itself* ends its slice with [`crate::TaskOutcome::Cancelled`].
//! - `task.backpressure` → [`crate::Scheduler::set_backpressure`].
//!
//! ## Stubbed — waitable layer (next increment)
//!
//! The remaining intrinsics need the bounded waitable table (futures, streams,
//! waitable-sets). Each returns an explicit `NOT_IMPLEMENTED` error — never a
//! silent `Ok` — so the scaffold cannot be mistaken for a working
//! implementation.

use kiln_error::{Error, Result};

use crate::task::TaskId;

macro_rules! phase1_stub {
    ($name:literal) => {
        Err(Error::not_implemented_error(concat!(
            "kiln-async: P3 intrinsic `",
            $name,
            "` is implemented in Phase 1"
        )))
    };
}

/// `task.wait` — block the current task until any member of a waitable set fires.
pub fn task_wait() -> Result<TaskId> {
    phase1_stub!("task.wait")
}

/// `task.poll` — non-blocking check of a waitable set.
pub fn task_poll() -> Result<Option<TaskId>> {
    phase1_stub!("task.poll")
}

/// `stream.new` — create a bounded SPSC stream with credit-based flow control.
pub fn stream_new() -> Result<TaskId> {
    phase1_stub!("stream.new")
}

/// `stream.read` — read from a stream, granting a credit (wakes a blocked writer).
pub fn stream_read() -> Result<()> {
    phase1_stub!("stream.read")
}

/// `stream.write` — write to a stream; blocks the writer when credits hit zero.
pub fn stream_write() -> Result<()> {
    phase1_stub!("stream.write")
}

/// `future.new` — create a single-shot future.
pub fn future_new() -> Result<TaskId> {
    phase1_stub!("future.new")
}

/// `waitable-set.new` — create a set tracking future/stream readiness.
pub fn waitable_set_new() -> Result<TaskId> {
    phase1_stub!("waitable-set.new")
}

/// `error-context.new` — create an error-context handle for async propagation.
pub fn error_context_new() -> Result<TaskId> {
    phase1_stub!("error-context.new")
}
