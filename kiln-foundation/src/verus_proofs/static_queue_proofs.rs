// Kiln - kiln-foundation
// Module: Verus Verification Proofs for StaticQueue
// SW-REQ-ID: REQ_MEM_SAFETY_001, REQ_RESOURCE_001, REQ_TEMPORAL_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Verus formal verification proofs for `StaticQueue<T, N>`.
//!
//! This module provides SMT-backed deductive proofs that verify the following
//! properties hold for ALL possible inputs (not just bounded test cases):
//!
//! 1. **Capacity invariant**: `len <= N` is always maintained
//! 2. **Index bounds**: `head < N` and `tail < N` always hold
//! 3. **Head-tail-len consistency**: `tail == (head + len) % N` when `N > 0`
//! 4. **Enqueue correctness**: Enqueue to non-full queue succeeds, increments len, advances tail
//! 5. **Enqueue-full rejection**: Enqueue to full queue returns Err without mutation
//! 6. **Dequeue correctness**: Dequeue returns head element (FIFO), decrements len, advances head
//! 7. **Dequeue-empty rejection**: Dequeue on empty queue returns None without mutation
//! 8. **FIFO ordering**: Enqueue(a); enqueue(b); dequeue() returns a (not b)
//! 9. **Enqueue-dequeue inverse**: On empty queue: enqueue(x); dequeue() returns x
//! 10. **Peek correctness**: peek() returns same element as dequeue() without mutation
//!
//! # Comparison with Kani Proofs
//!
//! | Property | Kani (bounded) | Verus (unbounded) |
//! |----------|---------------|-------------------|
//! | `len <= N` after push | Checked for N<=5 | Proved for ALL N |
//! | FIFO ordering | Checked for N<=5 | Proved universally |
//! | Circular wraparound | Checked for N=3 | Proved for ALL N |
//! | Capacity enforcement | Checked for N<=3 | Proved for ALL N |
//!
//! # Design Notes
//!
//! The `VerifiedStaticQueue` model mirrors the production `StaticQueue`'s
//! circular buffer logic. Ghost field `ghost_elements: Seq<T>` tracks the
//! abstract FIFO sequence, while `head`, `tail`, and `len` track the
//! concrete circular buffer indices.
//!
//! All operations are `proof fn` since this model exists purely for
//! verification — ghost fields can only be manipulated in proof/spec mode.
//!
//! # Running
//!
//! ```bash
//! # Via Bazel (preferred):
//! bazel test //kiln-foundation/src/verus_proofs:static_queue_verify
//! ```

#[allow(unused_imports)]
use builtin::*;
use builtin_macros::*;
use vstd::prelude::*;
use vstd::seq::*;

verus! {

// ============================================================================
// Verified Model of StaticQueue<T, N>
// ============================================================================

/// A verified model of `StaticQueue<T, N>` from `collections/static_queue.rs`.
///
/// This struct mirrors the production type's logical state:
/// - `head`: index of the front element
/// - `tail`: index where next element will be inserted
/// - `len`: number of elements in the queue
/// - `ghost_elements`: abstract FIFO sequence (erased by Verus)
pub struct VerifiedStaticQueue<T, const N: usize> {
    /// Index of the first element (head of queue).
    pub head: usize,

    /// Index where next element will be inserted (past the last element).
    pub tail: usize,

    /// Number of elements currently in the queue. Invariant: len <= N.
    pub len: usize,

    /// Ghost state: the abstract FIFO sequence of elements.
    pub ghost ghost_elements: Seq<T>,
}

// ============================================================================
// Well-formedness Specification
// ============================================================================

impl<T, const N: usize> VerifiedStaticQueue<T, N> {
    /// The core well-formedness predicate for VerifiedStaticQueue.
    ///
    /// Captures the invariants that must hold at all times:
    /// 1. `len <= N` (never exceeds compile-time capacity)
    /// 2. `head < N` (valid index when N > 0)
    /// 3. `tail < N` (valid index when N > 0)
    /// 4. `tail == (head + len) % N` when `N > 0` (circular buffer consistency)
    /// 5. `ghost_elements.len() == len` (ghost state matches concrete state)
    /// 6. When `N == 0`, all indices and len must be 0
    pub open spec fn well_formed(&self) -> bool {
        if N == 0 {
            self.len == 0 && self.head == 0 && self.tail == 0
            && self.ghost_elements.len() == 0
        } else {
            self.len <= N
            && self.head < N
            && self.tail < N
            && self.tail == (self.head + self.len) % (N as int)
            && self.ghost_elements.len() == self.len as nat
        }
    }

    /// Spec function: the abstract view as a FIFO sequence.
    pub open spec fn view(&self) -> Seq<T> {
        self.ghost_elements
    }

    /// Spec function: capacity is the const generic N.
    pub open spec fn spec_capacity(&self) -> nat {
        N as nat
    }

    /// Spec function: is the queue empty?
    pub open spec fn spec_is_empty(&self) -> bool {
        self.len == 0
    }

    /// Spec function: is the queue full?
    pub open spec fn spec_is_full(&self) -> bool {
        self.len == N
    }

    /// Spec function: peek at the front element without removing it.
    pub open spec fn spec_peek(&self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            Some(self.ghost_elements[0])
        }
    }
}

// ============================================================================
// Verified Operations (proof mode)
// ============================================================================

impl<T, const N: usize> VerifiedStaticQueue<T, N> {

    // ---- new() ----

    /// Creates a new empty VerifiedStaticQueue.
    ///
    /// Postconditions:
    /// - Length is 0
    /// - The queue is well-formed
    /// - The abstract view is the empty sequence
    proof fn new() -> (result: Self)
        ensures
            result.well_formed(),
            result.len == 0,
            result.view() =~= Seq::<T>::empty(),
    {
        let r = VerifiedStaticQueue {
            head: 0,
            tail: 0,
            len: 0,
            ghost_elements: Seq::empty(),
        };
        // SMT hint: when N > 0, 0 % N == 0 so tail == (head + len) % N holds
        if N > 0 {
            assert(0int % (N as int) == 0);
        }
        r
    }

    // ---- enqueue (push) ----

    /// Enqueues an element at the back of the queue.
    ///
    /// If `len < N`:
    ///   - Returns Ok(())
    ///   - The abstract view has `value` appended
    ///   - Length increases by 1
    ///   - Tail advances by 1 (mod N)
    ///   - Head is unchanged
    ///   - Well-formedness is maintained
    ///
    /// If `len >= N`:
    ///   - Returns Err(())
    ///   - The queue is unchanged
    proof fn enqueue(&mut self, value: T) -> (result: Result<(), ()>)
        requires
            old(self).well_formed(),
            N > 0,
        ensures
            self.well_formed(),
            match result {
                Ok(()) => {
                    old(self).len < N
                    && self.len == old(self).len + 1
                    && self.head == old(self).head
                    && self.tail == (old(self).tail + 1) % (N as int)
                    && self.view() =~= old(self).view().push(value)
                },
                Err(()) => {
                    old(self).len >= N
                    && *self =~= *old(self)
                },
            },
    {
        if self.len >= N {
            return Err(());
        }
        let old_head = self.head;
        let old_len = self.len;
        let old_tail = self.tail;
        self.ghost_elements = self.ghost_elements.push(value);
        self.tail = ((self.tail + 1) % (N as int)) as usize;
        self.len = (self.len + 1) as usize;

        // SMT hint: prove new_tail == (head + new_len) % N
        // We know: old_tail == (old_head + old_len) % N  (from well_formed)
        // Need: (old_tail + 1) % N == (old_head + old_len + 1) % N
        assert(old_tail as int == (old_head + old_len) % (N as int));
        assert(self.tail as int == (self.head + self.len) % (N as int)) by {
            vstd::arithmetic::div_mod::lemma_add_mod_noop_right(
                1int, (old_head + old_len) as int, N as int,
            );
        }

        Ok(())
    }

    // ---- dequeue (pop) ----

    /// Dequeues and returns the front element (FIFO order).
    ///
    /// If `len > 0`:
    ///   - Returns Some(front_element)
    ///   - Length decreases by 1
    ///   - Head advances by 1 (mod N)
    ///   - Tail is unchanged
    ///   - The abstract view drops the first element
    ///
    /// If `len == 0`:
    ///   - Returns None
    ///   - The queue is unchanged
    proof fn dequeue(&mut self) -> (result: Option<T>)
        requires
            old(self).well_formed(),
            N > 0,
        ensures
            self.well_formed(),
            match result {
                Some(value) => {
                    old(self).len > 0
                    && self.len == old(self).len - 1
                    && self.head == (old(self).head + 1) % (N as int)
                    && self.tail == old(self).tail
                    && value == old(self).view()[0]
                    && self.view() =~= old(self).view().subrange(1, old(self).view().len() as int)
                },
                None => {
                    old(self).len == 0
                    && *self =~= *old(self)
                },
            },
    {
        if self.len == 0 {
            return None;
        }
        let old_head = self.head;
        let old_len = self.len;
        let old_tail = self.tail;
        let value = self.ghost_elements[0];
        self.ghost_elements = self.ghost_elements.subrange(1, self.ghost_elements.len() as int);
        self.head = ((self.head + 1) % (N as int)) as usize;
        self.len = (self.len - 1) as usize;

        // SMT hint: prove tail == (new_head + new_len) % N
        // We know: old_tail == (old_head + old_len) % N  (from well_formed)
        // new_head = (old_head + 1) % N, new_len = old_len - 1
        // Need: old_tail == ((old_head + 1) % N + (old_len - 1)) % N
        assert(old_tail as int == (old_head + old_len) % (N as int));
        assert(self.tail as int == (self.head + self.len) % (N as int)) by {
            vstd::arithmetic::div_mod::lemma_add_mod_noop_right(
                (old_len - 1) as int, (old_head + 1) as int, N as int,
            );
        }

        Some(value)
    }

    // ---- peek ----

    /// Returns the front element without removing it.
    ///
    /// This is a spec-level function since ghost_elements is ghost state.
    /// Verifies that peek is non-destructive: the queue is unchanged.
    proof fn peek(&self) -> (result: Option<T>)
        requires
            self.well_formed(),
        ensures
            result == self.spec_peek(),
    {
        if self.len == 0 {
            None
        } else {
            Some(self.ghost_elements[0])
        }
    }

    // ---- clear ----

    /// Clears the queue, logically dropping all elements.
    ///
    /// Postconditions:
    /// - Length is 0
    /// - Head and tail are reset to 0
    /// - The abstract view is the empty sequence
    /// - Well-formedness is maintained
    proof fn clear(&mut self)
        requires old(self).well_formed(),
        ensures
            self.well_formed(),
            self.len == 0,
            self.head == 0,
            self.tail == 0,
            self.view() =~= Seq::<T>::empty(),
    {
        self.ghost_elements = Seq::empty();
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }
}

// ============================================================================
// Proof Functions: Unbounded Property Verification
// ============================================================================

/// Proof: enqueue followed by dequeue is an identity on empty queue.
///
/// For ANY well-formed empty VerifiedStaticQueue with capacity N > 0,
/// enqueueing a value and immediately dequeueing returns that value
/// and restores the empty state.
proof fn proof_enqueue_dequeue_inverse<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
    value: T,
)
    requires
        queue.well_formed(),
        queue.len == 0,
        N > 0,
{
    let mut q = queue;
    let enq_result = q.enqueue(value);
    assert(enq_result.is_ok());
    assert(q.well_formed());
    assert(q.len == 1);

    let deq_result = q.dequeue();
    assert(deq_result.is_some());
    assert(q.well_formed());
    assert(q.len == 0);
    assert(q.view() =~= Seq::<T>::empty());
}

/// Proof: enqueueing to a full queue returns Err and leaves it unchanged.
proof fn proof_enqueue_full_returns_err<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
    value: T,
)
    requires
        queue.well_formed(),
        queue.len == N,
        N > 0,
{
    let mut q = queue;
    let result = q.enqueue(value);
    assert(result.is_err());
    assert(q =~= queue);
}

/// Proof: dequeueing from an empty queue returns None and is a no-op.
proof fn proof_dequeue_empty_is_noop<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires
        queue.well_formed(),
        queue.len == 0,
        N > 0,
{
    let mut q = queue;
    let result = q.dequeue();
    assert(result.is_none());
    assert(q =~= queue);
}

/// Proof: FIFO ordering — enqueue(a); enqueue(b); dequeue() returns a.
///
/// This is the fundamental FIFO property: the first element enqueued
/// is the first element dequeued.
proof fn proof_fifo_ordering<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
    a: T,
    b: T,
)
    requires
        queue.well_formed(),
        queue.len == 0,
        N >= 2,
{
    let mut q = queue;

    // Enqueue a, then b
    let r1 = q.enqueue(a);
    assert(r1.is_ok());

    let r2 = q.enqueue(b);
    assert(r2.is_ok());
    assert(q.len == 2);

    // Dequeue should return a (FIFO)
    let result = q.dequeue();
    assert(result.is_some());
    assert(result == Some(a));
    assert(q.len == 1);
}

/// Proof: enqueue preserves well-formedness.
proof fn proof_enqueue_preserves_well_formed<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
    value: T,
)
    requires
        queue.well_formed(),
        N > 0,
{
    let mut q = queue;
    let _ = q.enqueue(value);
    assert(q.well_formed());
}

/// Proof: dequeue preserves well-formedness.
proof fn proof_dequeue_preserves_well_formed<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires
        queue.well_formed(),
        N > 0,
{
    let mut q = queue;
    let _ = q.dequeue();
    assert(q.well_formed());
}

/// Proof: clear preserves well-formedness and empties the queue.
proof fn proof_clear_preserves_well_formed<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires queue.well_formed(),
{
    let mut q = queue;
    q.clear();
    assert(q.well_formed());
    assert(q.len == 0);
    assert(q.view() =~= Seq::<T>::empty());
}

/// Proof: length is always bounded by capacity.
///
/// For ANY well-formed queue, 0 <= len <= N.
/// This is the core ASIL-D invariant.
proof fn proof_length_bounded_by_capacity<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires queue.well_formed(),
    ensures
        queue.len <= N,
        queue.view().len() <= N as nat,
{
    // Direct consequence of well_formed() definition.
}

/// Proof: head and tail are always valid indices.
///
/// For ANY well-formed queue with N > 0, head < N and tail < N.
proof fn proof_indices_in_bounds<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires
        queue.well_formed(),
        N > 0,
    ensures
        queue.head < N,
        queue.tail < N,
{
    // Direct consequence of well_formed() definition.
}

/// Proof: head-tail-len consistency.
///
/// For ANY well-formed queue with N > 0,
/// tail == (head + len) % N.
proof fn proof_head_tail_len_consistent<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires
        queue.well_formed(),
        N > 0,
    ensures
        queue.tail == (queue.head + queue.len) % (N as int),
{
    // Direct consequence of well_formed() definition.
}

/// Proof: new() creates an empty, well-formed queue.
proof fn proof_new_is_empty_and_well_formed<T, const N: usize>()
{
    let queue = VerifiedStaticQueue::<T, N>::new();
    assert(queue.well_formed());
    assert(queue.len == 0);
    assert(queue.view() =~= Seq::<T>::empty());
}

/// Proof: peek returns the same element that dequeue would return,
/// without modifying the queue.
proof fn proof_peek_matches_dequeue<T, const N: usize>(
    queue: VerifiedStaticQueue<T, N>,
)
    requires
        queue.well_formed(),
        queue.len > 0,
        N > 0,
{
    // Peek at the front
    let peek_result = queue.peek();
    assert(peek_result.is_some());

    // Dequeue from the front
    let mut q = queue;
    let deq_result = q.dequeue();
    assert(deq_result.is_some());

    // Both should return the same element
    assert(peek_result == deq_result);
}

} // verus!
