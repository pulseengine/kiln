// Kiln - kiln-async :: ready
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Bounded, no-alloc FIFO ready queue (O(1) push/pop).

use kiln_error::{Error, Result};

use crate::task::TaskId;

/// Fixed-capacity FIFO ring of ready task handles.
///
/// Sized `== NTASK` in the scheduler: with the "a task is in the ready set at
/// most once" invariant, it can never overflow. Push/pop are O(1) with no heap.
/// This bounded-queue invariant is the target of the Verus no-overflow /
/// ordering proof (property P1 in the design doc).
pub struct ReadyQueue<const N: usize> {
    buf: [TaskId; N],
    head: usize,
    len: usize,
}

impl<const N: usize> ReadyQueue<N> {
    /// Create an empty ready queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [TaskId::NONE; N],
            head: usize::MIN,
            len: usize::MIN,
        }
    }

    /// Capacity (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of queued tasks.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the queue is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the queue is full.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Enqueue a task at the tail. O(1).
    ///
    /// Returns an error if the queue is full — which, given the
    /// at-most-once-in-ready invariant and `N == NTASK`, should be unreachable
    /// in correct use, but is checked rather than silently dropped.
    pub fn push(&mut self, id: TaskId) -> Result<()> {
        if self.is_full() {
            return Err(Error::validation_error(
                "kiln-async: ready queue at capacity",
            ));
        }
        let tail = (self.head + self.len) % N;
        self.buf[tail] = id;
        self.len += 1;
        Ok(())
    }

    /// Dequeue the head task. O(1). Returns `None` when empty.
    pub fn pop(&mut self) -> Option<TaskId> {
        if self.len == 0 {
            return None;
        }
        let id = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(id)
    }
}

impl<const N: usize> Default for ReadyQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid(i: u16) -> TaskId {
        TaskId {
            index: i,
            generation: 0,
        }
    }

    #[test]
    fn fifo_order() {
        let mut q: ReadyQueue<4> = ReadyQueue::new();
        assert!(q.is_empty());
        q.push(tid(1)).unwrap();
        q.push(tid(2)).unwrap();
        q.push(tid(3)).unwrap();
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop(), Some(tid(1)));
        assert_eq!(q.pop(), Some(tid(2)));
        assert_eq!(q.pop(), Some(tid(3)));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn ring_wraparound() {
        let mut q: ReadyQueue<2> = ReadyQueue::new();
        q.push(tid(1)).unwrap();
        q.pop();
        q.push(tid(2)).unwrap();
        q.push(tid(3)).unwrap();
        assert!(q.is_full());
        assert_eq!(q.pop(), Some(tid(2)));
        assert_eq!(q.pop(), Some(tid(3)));
        assert!(q.is_empty());
    }

    #[test]
    fn full_queue_rejects_rather_than_dropping() {
        let mut q: ReadyQueue<1> = ReadyQueue::new();
        q.push(tid(1)).unwrap();
        assert!(q.is_full());
        assert!(q.push(tid(2)).is_err());
    }
}
