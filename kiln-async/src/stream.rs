// Kiln - kiln-async :: stream
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Bounded SPSC stream with credit-based backpressure (Component Model
//! `stream<T>`), no-alloc.
//!
//! Like [`crate::FutureTable`], the *payload* `T` crosses through guest memory
//! via the engine; this tracks only the synchronization state — how many items
//! are buffered, whether the producer end has closed, and the single parked
//! reader / writer task — so the scheduler knows whom to wake.
//!
//! **Backpressure is credit-based, not spin/unbounded-buffer:** a write into a
//! full stream parks the writer ([`StreamWrite::Backpressured`]); a read frees a
//! slot and wakes it. A read from an empty stream parks the reader
//! ([`StreamRead::Empty`]); a write buffers an item and wakes it. Either op on a
//! closed-and-drained stream reports the end rather than blocking forever.

use kiln_error::{Error, Result};

use crate::task::TaskId;

/// Outcome of [`Stream::write`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamWrite {
    /// Item buffered; if a reader was parked it is returned to be woken.
    Buffered { wake_reader: Option<TaskId> },
    /// Stream is full (no credit) — the writer has been parked; the caller
    /// ends its slice with `TaskOutcome::Waited`.
    Backpressured,
}

/// Outcome of [`Stream::read`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamRead {
    /// Item consumed; if a writer was parked (was full) it is returned to wake.
    Consumed { wake_writer: Option<TaskId> },
    /// Stream is empty and open — the reader has been parked.
    Empty,
    /// Stream is empty and closed — no more items will ever arrive.
    Ended,
}

/// A bounded SPSC stream's synchronization state (no payload, no heap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stream {
    capacity: u32,
    /// Buffered item count, `0..=capacity`.
    count: u32,
    /// Producer end closed (`stream.close-writable`): no further writes.
    closed: bool,
    /// Reader parked because the stream was empty, or [`TaskId::NONE`].
    reader: TaskId,
    /// Writer parked because the stream was full, or [`TaskId::NONE`].
    writer: TaskId,
}

impl Stream {
    /// Create an open, empty stream that can buffer up to `capacity` items.
    #[must_use]
    pub const fn new(capacity: u32) -> Self {
        Self {
            capacity,
            count: 0,
            closed: false,
            reader: TaskId::NONE,
            writer: TaskId::NONE,
        }
    }

    /// Capacity (the credit bound).
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Buffered item count.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.count
    }

    /// Whether no items are buffered.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Whether the buffer is at capacity (writes will backpressure).
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.count == self.capacity
    }

    /// Whether the producer end has closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// `stream.write` one item by `writer`. Buffers if there is credit (waking a
    /// parked reader); otherwise parks `writer` and reports backpressure. Errors
    /// if the stream is already closed (writing to a closed producer end).
    pub fn write(&mut self, writer: TaskId) -> Result<StreamWrite> {
        if self.closed {
            return Err(Error::validation_error(
                "kiln-async: write to a closed stream",
            ));
        }
        if self.is_full() {
            // No credit — park the writer (backpressure, no spin/overflow).
            self.writer = writer;
            return Ok(StreamWrite::Backpressured);
        }
        self.count += 1;
        Ok(StreamWrite::Buffered {
            wake_reader: take_waiter(&mut self.reader),
        })
    }

    /// `stream.read` one item by `reader`. Consumes a buffered item if present
    /// (waking a parked writer, since a slot freed); parks `reader` if empty and
    /// open; reports `Ended` if empty and closed.
    pub fn read(&mut self, reader: TaskId) -> Result<StreamRead> {
        if self.count > 0 {
            self.count -= 1;
            // A slot freed: wake a parked (backpressured) writer if any.
            return Ok(StreamRead::Consumed {
                wake_writer: take_waiter(&mut self.writer),
            });
        }
        if self.closed {
            return Ok(StreamRead::Ended);
        }
        self.reader = reader;
        Ok(StreamRead::Empty)
    }

    /// Close the producer end (`stream.close-writable`). Returns the parked
    /// reader, if any, so the scheduler can wake it to observe the end.
    pub fn close(&mut self) -> Option<TaskId> {
        self.closed = true;
        take_waiter(&mut self.reader)
    }
}

/// Take a parked waiter out of a slot, returning it if one was set.
fn take_waiter(slot: &mut TaskId) -> Option<TaskId> {
    if slot.is_none() {
        None
    } else {
        let w = *slot;
        *slot = TaskId::NONE;
        Some(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(i: u16) -> TaskId {
        TaskId { index: i, generation: 0 }
    }

    #[test]
    fn new_stream_is_open_and_empty() {
        let s = Stream::new(2);
        assert_eq!(s.capacity(), 2);
        assert!(s.is_empty());
        assert!(!s.is_full());
        assert!(!s.is_closed());
    }

    #[test]
    fn write_buffers_with_no_reader_then_read_consumes() {
        let mut s = Stream::new(2);
        // write-before-read: nobody parked to wake.
        assert_eq!(s.write(task(1)).unwrap(), StreamWrite::Buffered { wake_reader: None });
        assert_eq!(s.len(), 1);
        // read consumes; no writer was parked (wasn't full).
        assert_eq!(s.read(task(2)).unwrap(), StreamRead::Consumed { wake_writer: None });
        assert!(s.is_empty());
    }

    #[test]
    fn read_when_empty_parks_reader_then_write_wakes_it() {
        let mut s = Stream::new(2);
        // reader reads empty → parks.
        assert_eq!(s.read(task(7)).unwrap(), StreamRead::Empty);
        // a write buffers and returns the parked reader to wake.
        assert_eq!(
            s.write(task(1)).unwrap(),
            StreamWrite::Buffered { wake_reader: Some(task(7)) }
        );
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn write_when_full_backpressures_then_read_wakes_writer() {
        let mut s = Stream::new(1);
        s.write(task(1)).unwrap(); // fills the single slot
        assert!(s.is_full());
        // second write has no credit → writer parks, backpressure.
        assert_eq!(s.write(task(9)).unwrap(), StreamWrite::Backpressured);
        // a read frees the slot and returns the parked writer to wake.
        assert_eq!(
            s.read(task(2)).unwrap(),
            StreamRead::Consumed { wake_writer: Some(task(9)) }
        );
    }

    #[test]
    fn write_to_closed_stream_errors() {
        let mut s = Stream::new(2);
        s.close();
        assert!(s.write(task(1)).is_err());
    }

    #[test]
    fn read_empty_closed_stream_ends() {
        let mut s = Stream::new(2);
        s.close();
        assert_eq!(s.read(task(3)).unwrap(), StreamRead::Ended);
    }

    #[test]
    fn close_returns_a_parked_reader_to_wake() {
        let mut s = Stream::new(2);
        assert_eq!(s.read(task(5)).unwrap(), StreamRead::Empty); // reader parks
        assert_eq!(s.close(), Some(task(5))); // close wakes it to observe Ended
        // and a still-buffered item is still readable after close.
        let mut s2 = Stream::new(2);
        s2.write(task(1)).unwrap();
        s2.close();
        assert_eq!(s2.read(task(2)).unwrap(), StreamRead::Consumed { wake_writer: None });
    }
}
