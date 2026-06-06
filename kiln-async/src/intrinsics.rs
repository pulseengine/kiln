// Kiln - kiln-async :: intrinsics
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! WASM Component Model **P3 async ABI** surface.
//!
//! These functions enumerate the canonical-ABI intrinsics the scheduler backs
//! on the embedded path (per RFC #46, Meld lowers the component-level async
//! constructs to these core import calls). Every one is a **Phase 1** stub that
//! returns an explicit `NOT_IMPLEMENTED` error — never a silent `Ok` — so the
//! scaffold cannot be mistaken for a working implementation.
//!
//! Phase 1 wires each of these to [`crate::Scheduler`] over the bounded
//! task/ready/waitable structures.

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

/// `task.yield` — cooperatively re-enqueue the current task at its priority tail.
pub fn task_yield() -> Result<()> {
    phase1_stub!("task.yield")
}

/// `task.wait` — block the current task until any member of a waitable set fires.
pub fn task_wait() -> Result<TaskId> {
    phase1_stub!("task.wait")
}

/// `task.poll` — non-blocking check of a waitable set.
pub fn task_poll() -> Result<Option<TaskId>> {
    phase1_stub!("task.poll")
}

/// `task.cancel` — transition a subtask to `Cancelled` and free its slot.
pub fn task_cancel(_subtask: TaskId) -> Result<()> {
    phase1_stub!("task.cancel")
}

/// `task.backpressure` — gate admission of new subtasks.
pub fn task_backpressure(_enable: bool) -> Result<()> {
    phase1_stub!("task.backpressure")
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
