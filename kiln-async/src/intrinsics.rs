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
//! - `future.new` → [`crate::Scheduler::future_new`].
//! - `future.write` → [`crate::Scheduler::future_write`] (wakes the reader).
//! - `future.read` → [`crate::Scheduler::future_read`] +
//!   [`crate::Scheduler::wait_on_future`] (read-or-park).
//! - `waitable-set.new/join/drop` → [`crate::Scheduler::waitable_set_new`] /
//!   [`crate::Scheduler::waitable_set_join`] / [`crate::Scheduler::waitable_set_drop`].
//! - `task.wait` → [`crate::Scheduler::task_wait`] (arm-or-already-ready).
//! - `task.poll` → [`crate::Scheduler::task_poll`] (non-blocking).
//! - `stream.new/drop` → [`crate::Scheduler::stream_new`] /
//!   [`crate::Scheduler::stream_drop`].
//! - `stream.write` → [`crate::Scheduler::stream_write`] (buffers + wakes the
//!   reader, or backpressures the writer).
//! - `stream.read` → [`crate::Scheduler::stream_read`] (consumes + wakes a
//!   backpressured writer, or parks the reader / reports end).
//! - `stream.close-writable` → [`crate::Scheduler::stream_close`].
//! - `error-context.new/drop` → [`crate::ErrorContextTable::create`] /
//!   [`crate::ErrorContextTable::drop_context`]. (An `error-context` carries no
//!   scheduling state — no waiters/wakes — so it is a standalone backing the
//!   embedding owns directly, not a `Scheduler` field.)
//!
//! Every P3 intrinsic now has a real backing — no stub returns a silent `Ok`,
//! and none returns `NOT_IMPLEMENTED` for the embedded path.
