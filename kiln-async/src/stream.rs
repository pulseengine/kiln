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

/// ABA-safe stream handle (mirrors [`crate::FutureId`] / [`crate::SetId`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamId {
    /// Index into the stream table.
    pub index: u16,
    /// Generation at the time this handle was issued.
    pub generation: u32,
}

impl StreamId {
    /// Sentinel "no stream" handle.
    pub const NONE: Self = Self {
        index: u16::MAX,
        generation: 0,
    };

    /// Whether this is the [`StreamId::NONE`] sentinel.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.index == u16::MAX
    }
}

#[derive(Clone, Copy)]
struct StreamSlot {
    stream: Option<Stream>,
    generation: u32,
}

/// Fixed-capacity table of [`Stream`]s, indexed by [`StreamId`].
///
/// `N` streams, no heap (`[StreamSlot; N]`). Backs `stream.{new,drop}`; the
/// scheduler looks a stream up by handle to `read`/`write`/`close` it.
pub struct StreamTable<const N: usize> {
    slots: [StreamSlot; N],
    live: usize,
}

impl<const N: usize> StreamTable<N> {
    /// Create an empty table — all `N` slots free.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [StreamSlot {
                stream: None,
                generation: 0,
            }; N],
            live: usize::MIN,
        }
    }

    /// Capacity (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of live (allocated) streams.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether no streams are allocated.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// `stream.new` — allocate a fresh stream buffering up to `item_capacity`
    /// items. Fails loud when the table is at capacity.
    pub fn create(&mut self, item_capacity: u32) -> Result<StreamId> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.stream.is_none() {
                slot.stream = Some(Stream::new(item_capacity));
                self.live += 1;
                return Ok(StreamId {
                    index: index as u16,
                    generation: slot.generation,
                });
            }
        }
        Err(Error::foundation_bounded_capacity_exceeded(
            "kiln-async: stream table at capacity",
        ))
    }

    /// Generation-validated shared access to a live stream.
    #[must_use]
    pub fn get(&self, id: StreamId) -> Option<&Stream> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.stream.as_ref()
        } else {
            None
        }
    }

    /// Generation-validated mutable access to a live stream.
    pub fn get_mut(&mut self, id: StreamId) -> Option<&mut Stream> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation == id.generation {
            slot.stream.as_mut()
        } else {
            None
        }
    }

    /// `stream.drop` — free a stream, bumping its generation so stale handles
    /// are detectable. Errors on a stale/unknown handle.
    pub fn drop_stream(&mut self, id: StreamId) -> Result<()> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .filter(|s| s.stream.is_some() && s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: drop of a stale or unknown stream handle")
            })?;
        slot.stream = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.live -= 1;
        Ok(())
    }
}

impl<const N: usize> Default for StreamTable<N> {
    fn default() -> Self {
        Self::new()
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
    fn stream_table_create_get_drop() {
        let mut t: StreamTable<2> = StreamTable::new();
        assert_eq!(t.capacity(), 2);
        assert!(t.is_empty());
        let s = t.create(4).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.get(s).unwrap().capacity(), 4);
        // operate through the handle
        t.get_mut(s).unwrap().write(task(1)).unwrap();
        assert_eq!(t.get(s).unwrap().len(), 1);
        t.drop_stream(s).unwrap();
        assert!(t.is_empty());
        assert!(t.get(s).is_none());
    }

    #[test]
    fn stream_table_create_errors_at_capacity() {
        let mut t: StreamTable<1> = StreamTable::new();
        t.create(2).unwrap();
        assert!(t.create(2).is_err());
    }

    #[test]
    fn stream_table_drop_is_aba_safe_and_rejects_stale() {
        let mut t: StreamTable<1> = StreamTable::new();
        let old = t.create(2).unwrap();
        t.drop_stream(old).unwrap();
        assert!(t.drop_stream(old).is_err()); // already gone
        let new = t.create(2).unwrap();
        assert_eq!(new.index, old.index);
        assert_ne!(new.generation, old.generation); // reuse bumped generation
        assert!(t.get(old).is_none()); // stale handle invalid
        assert!(t.get(new).is_some());
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
