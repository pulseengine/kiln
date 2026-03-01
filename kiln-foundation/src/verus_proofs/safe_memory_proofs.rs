// Kiln - kiln-foundation
// Module: Verus Verification Proofs for SafeMemoryHandler
// SW-REQ-ID: REQ_MEM_SAFETY_001, REQ_RESOURCE_001, REQ_TEMPORAL_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Verus formal verification proofs for `SafeMemoryHandler<P>`.
//!
//! This module provides SMT-backed deductive proofs that verify the following
//! properties hold for ALL possible inputs (not just bounded test cases):
//!
//! 1. **Capacity invariant**: `used <= capacity` is always maintained
//! 2. **Read-write consistency**: `write(addr, data); read(addr, len)` returns data
//! 3. **Bounds checking (write)**: Writes beyond capacity are rejected
//! 4. **Bounds checking (read)**: Reads beyond used region are rejected
//! 5. **Clear correctness**: `clear()` resets used length to 0
//! 6. **Integrity detection**: Checksum detects data corruption
//! 7. **Well-formedness preservation**: All operations preserve well-formedness
//! 8. **New creates empty handler**: `new()` produces a well-formed, empty handler
//! 9. **Write extends used region**: Writing past current used mark extends it
//! 10. **Capacity is constant**: Capacity never changes across operations
//!
//! # Comparison with Kani Proofs
//!
//! | Property | Kani (bounded) | Verus (unbounded) |
//! |----------|---------------|-------------------|
//! | `used <= capacity` after write | Checked for capacity<=256 | Proved for ALL capacities |
//! | Read-write consistency | Checked for small buffers | Proved universally |
//! | Bounds rejection | Checked for specific offsets | Proved for ALL offsets |
//! | Checksum integrity | Checked for sample data | Proved abstractly for ALL data |
//!
//! # Design Notes
//!
//! `SafeMemoryHandler<P>` wraps a memory `Provider` and delegates operations.
//! The verified model abstracts away the provider and directly models the
//! memory buffer as a ghost `Seq<u8>` with explicit capacity, used length,
//! and checksum tracking.
//!
//! Checksums are modeled abstractly via `uninterp spec fn` predicates,
//! similar to the UTF-8 modeling in `static_string_proofs.rs`. The key
//! axiom is that `compute_checksum` is deterministic: the same data always
//! produces the same checksum.
//!
//! All operations are `proof fn` since this model exists purely for
//! verification -- ghost fields can only be manipulated in proof/spec mode.
//!
//! # Running
//!
//! ```bash
//! # Via Bazel (preferred):
//! bazel test //kiln-foundation/src/verus_proofs:safe_memory_verify
//! ```

#[allow(unused_imports)]
use builtin::*;
use builtin_macros::*;
use vstd::prelude::*;
use vstd::seq::*;

verus! {

// ============================================================================
// Abstract Checksum Model
// ============================================================================

/// Abstract spec function for computing a checksum of a byte sequence.
///
/// We model this as an opaque (uninterpreted) function. The key axiom is
/// determinism: the same byte sequence always produces the same checksum.
/// This mirrors the production `Checksum::compute(data)` function.
pub uninterp spec fn compute_checksum(data: Seq<u8>) -> nat;

/// Axiom: Checksum computation is deterministic.
///
/// For any two byte sequences that are extensionally equal,
/// their checksums are equal. This is the fundamental property
/// that makes integrity verification sound.
#[verifier::external_body]
pub proof fn axiom_checksum_deterministic(a: Seq<u8>, b: Seq<u8>)
    requires a =~= b,
    ensures compute_checksum(a) == compute_checksum(b),
{
}

/// Axiom: Different data produces different checksums (collision resistance).
///
/// This is an idealized model -- real checksums can have collisions, but
/// for verification purposes we assume the checksum is a perfect integrity
/// detector. This matches the safety-critical design intent: if data changes,
/// the checksum MUST detect it.
#[verifier::external_body]
pub proof fn axiom_checksum_collision_resistant(a: Seq<u8>, b: Seq<u8>)
    requires !(a =~= b),
    ensures compute_checksum(a) != compute_checksum(b),
{
}

// ============================================================================
// Verified Model of SafeMemoryHandler<P>
// ============================================================================

/// A verified model of `SafeMemoryHandler<P>` from `safe_memory.rs`.
///
/// This struct mirrors the production type's logical state:
/// - `capacity`: fixed upper bound on memory size (from the Provider)
/// - `used`: number of bytes currently in use (0..used are valid)
/// - `ghost_data`: abstract byte sequence tracking memory content
/// - `ghost_checksum`: stored checksum for integrity verification
///
/// The production `SafeMemoryHandler` wraps a `Provider` which manages
/// the actual buffer. This model abstracts that into direct ghost state.
pub struct VerifiedSafeMemoryHandler<const N: usize> {
    /// Fixed capacity of the underlying memory provider.
    /// Invariant: this never changes after construction.
    pub capacity: usize,

    /// Number of bytes currently in use. Invariant: used <= capacity.
    pub used: usize,

    /// Ghost state: the abstract byte sequence representing memory content.
    pub ghost ghost_data: Seq<u8>,

    /// Ghost state: the stored checksum for integrity verification.
    /// This is updated on every write and checked on verify_integrity().
    pub ghost ghost_checksum: nat,
}

// ============================================================================
// Well-formedness Specification
// ============================================================================

impl<const N: usize> VerifiedSafeMemoryHandler<N> {
    /// The core well-formedness predicate for VerifiedSafeMemoryHandler.
    ///
    /// This captures the invariants that must hold at all times:
    /// 1. `capacity == N` (capacity matches the const generic)
    /// 2. `used <= capacity` (never exceeds memory capacity)
    /// 3. `ghost_data.len() == used` (ghost state matches concrete state)
    /// 4. `ghost_checksum == compute_checksum(ghost_data)` (checksum is current)
    pub open spec fn well_formed(&self) -> bool {
        self.capacity == N
        && self.used <= self.capacity
        && self.ghost_data.len() == self.used as nat
        && self.ghost_checksum == compute_checksum(self.ghost_data)
    }

    /// Spec function: the abstract view as a byte sequence.
    pub open spec fn view(&self) -> Seq<u8> {
        self.ghost_data
    }

    /// Spec function: capacity is the const generic N.
    pub open spec fn spec_capacity(&self) -> nat {
        N as nat
    }

    /// Spec function: is the handler empty?
    pub open spec fn spec_is_empty(&self) -> bool {
        self.used == 0
    }

    /// Spec function: current length of used memory.
    pub open spec fn spec_len(&self) -> nat {
        self.used as nat
    }

    /// Spec function: read a sub-sequence from offset with given length.
    ///
    /// Returns Some(sub_seq) iff offset + len <= used (the read is in bounds).
    /// Returns None if the read would be out of bounds.
    pub open spec fn spec_read(&self, offset: usize, len: usize) -> Option<Seq<u8>> {
        if offset as nat + len as nat <= self.used as nat {
            Some(self.ghost_data.subrange(offset as int, (offset + len) as int))
        } else {
            None
        }
    }

    /// Spec function: check whether a write at offset with given length
    /// would be within capacity.
    pub open spec fn spec_write_in_bounds(&self, offset: usize, len: usize) -> bool {
        offset as nat + len as nat <= self.capacity as nat
    }
}

// ============================================================================
// Verified Operations (proof mode)
// ============================================================================

impl<const N: usize> VerifiedSafeMemoryHandler<N> {

    // ---- new() ----

    /// Creates a new empty VerifiedSafeMemoryHandler.
    ///
    /// Postconditions:
    /// - Used length is 0
    /// - Capacity is N
    /// - The handler is well-formed
    /// - The abstract view is the empty sequence
    proof fn new() -> (result: Self)
        ensures
            result.well_formed(),
            result.used == 0,
            result.capacity == N,
            result.view() =~= Seq::<u8>::empty(),
    {
        let data = Seq::<u8>::empty();
        VerifiedSafeMemoryHandler {
            capacity: N,
            used: 0,
            ghost_data: data,
            ghost_checksum: compute_checksum(data),
        }
    }

    // ---- write() ----

    /// Writes data at the given offset.
    ///
    /// Models `SafeMemoryHandler::write_data()` which delegates to
    /// `Provider::write_data()`.
    ///
    /// If `offset + data.len() <= capacity`:
    ///   - Returns Ok(())
    ///   - The bytes at [offset, offset+data.len()) are replaced with data
    ///   - Used region is extended if write goes past current used mark
    ///   - Checksum is updated
    ///   - Well-formedness is maintained
    ///
    /// If `offset + data.len() > capacity`:
    ///   - Returns Err(())
    ///   - The handler is unchanged
    proof fn write(&mut self, offset: usize, data: Seq<u8>) -> (result: Result<(), ()>)
        requires
            old(self).well_formed(),
            // For simplicity in the model, we require writes start within
            // or at the current used region (no gaps). This matches the
            // production NoStdProvider's behavior where writes extend used.
            offset as nat <= old(self).used as nat,
        ensures
            self.well_formed(),
            match result {
                Ok(()) => {
                    offset as nat + data.len() <= old(self).capacity as nat
                    && self.capacity == old(self).capacity
                    // The written region contains exactly the provided data
                    && self.ghost_data.subrange(offset as int, offset + data.len()) =~= data
                    // Bytes before the write offset are unchanged
                    && self.ghost_data.subrange(0, offset as int)
                        =~= old(self).ghost_data.subrange(0, offset as int)
                    // Precise used accounting: extends if write goes past, unchanged otherwise
                    && (if offset as nat + data.len() >= old(self).used as nat {
                        self.used as nat == offset as nat + data.len()
                    } else {
                        self.used == old(self).used
                    })
                },
                Err(()) => {
                    offset as nat + data.len() > old(self).capacity as nat
                    && *self =~= *old(self)
                },
            },
    {
        let write_end = offset as nat + data.len();
        if write_end > N as nat {
            return Err(());
        }

        // Build the new data sequence:
        // [0..offset) from old data + data + padding if needed
        let prefix = self.ghost_data.subrange(0, offset as int);
        let new_data = prefix + data;

        // If the write extends past used, the new used is offset + data.len()
        // Otherwise, we need to append the suffix from old data after the write
        let new_used: usize;
        let final_data: Seq<u8>;

        if write_end >= self.used as nat {
            new_used = write_end as usize;
            final_data = new_data;
        } else {
            // Write is within existing region: preserve suffix
            let suffix = self.ghost_data.subrange(
                offset + data.len(),
                self.used as int,
            );
            final_data = new_data + suffix;
            new_used = self.used;
        }

        // Verify the constructed sequence has the right length
        assert(final_data.len() == new_used as nat);

        // Verify the written region
        assert(final_data.subrange(offset as int, offset + data.len()) =~= data);

        // Verify the prefix is preserved
        assert(final_data.subrange(0, offset as int)
            =~= self.ghost_data.subrange(0, offset as int));

        self.ghost_data = final_data;
        self.used = new_used;
        self.ghost_checksum = compute_checksum(self.ghost_data);

        Ok(())
    }

    // ---- read() ----

    /// Reads data from the given offset with the given length.
    ///
    /// Models `SafeMemoryHandler::borrow_slice()` which delegates to
    /// `Provider::borrow_slice()`.
    ///
    /// If `offset + len <= used`:
    ///   - Returns Ok(sub_sequence)
    ///   - The handler is unchanged
    ///
    /// If `offset + len > used`:
    ///   - Returns Err(())
    ///   - The handler is unchanged
    proof fn read(&self, offset: usize, len: usize) -> (result: Result<Seq<u8>, ()>)
        requires self.well_formed(),
        ensures
            match result {
                Ok(data) => {
                    offset as nat + len as nat <= self.used as nat
                    && data =~= self.ghost_data.subrange(offset as int, (offset + len) as int)
                    && data.len() == len as nat
                },
                Err(()) => {
                    offset as nat + len as nat > self.used as nat
                },
            },
    {
        if offset as nat + len as nat > self.used as nat {
            return Err(());
        }
        let data = self.ghost_data.subrange(offset as int, (offset + len) as int);
        Ok(data)
    }

    // ---- verify_integrity() ----

    /// Verifies the integrity of the memory handler.
    ///
    /// Models `SafeMemoryHandler::verify_integrity()` which checks
    /// that the stored checksum matches a freshly computed checksum.
    ///
    /// For a well-formed handler, this always succeeds because the
    /// checksum is kept in sync with the data.
    proof fn verify_integrity(&self) -> (result: Result<(), ()>)
        requires self.well_formed(),
        ensures result.is_ok(),
    {
        // well_formed() guarantees ghost_checksum == compute_checksum(ghost_data)
        // So recomputing and comparing always succeeds.
        axiom_checksum_deterministic(self.ghost_data, self.ghost_data);
        Ok(())
    }

    // ---- len() ----

    /// Returns the current used length.
    ///
    /// Models `SafeMemoryHandler::len()` which returns `provider.size()`.
    proof fn len(&self) -> (result: usize)
        requires self.well_formed(),
        ensures result == self.used,
    {
        self.used
    }

    // ---- clear() ----

    /// Clears the memory handler, resetting to empty state.
    ///
    /// Models `SafeMemoryHandler::clear()` which zeros out memory.
    ///
    /// Postconditions:
    /// - Used length is 0
    /// - Capacity is unchanged
    /// - The abstract view is the empty sequence
    /// - Checksum is updated
    /// - Well-formedness is maintained
    proof fn clear(&mut self)
        requires old(self).well_formed(),
        ensures
            self.well_formed(),
            self.used == 0,
            self.capacity == old(self).capacity,
            self.view() =~= Seq::<u8>::empty(),
    {
        let empty = Seq::<u8>::empty();
        self.ghost_data = empty;
        self.used = 0;
        self.ghost_checksum = compute_checksum(empty);
    }
}

// ============================================================================
// Proof Functions: Unbounded Property Verification
// ============================================================================

/// Proof: write followed by read returns the written data.
///
/// For ANY well-formed handler with sufficient capacity,
/// writing data at an offset and then reading from that same
/// offset returns exactly the written data.
proof fn proof_write_read_consistency<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    data: Seq<u8>,
)
    requires
        handler.well_formed(),
        offset as nat <= handler.used as nat,
        offset as nat + data.len() <= N as nat,
{
    let mut h = handler;
    let write_result = h.write(offset, data);
    assert(write_result.is_ok());
    assert(h.well_formed());

    // Read back from the same offset
    let read_result = h.read(offset, data.len() as usize);
    assert(read_result.is_ok());

    // The read data should match what was written
    match read_result {
        Ok(read_data) => {
            assert(read_data =~= data);
        },
        Err(()) => {
            // This branch is unreachable given our preconditions
            assert(false);
        },
    }
}

/// Proof: writing beyond capacity is rejected and leaves handler unchanged.
///
/// For ANY well-formed handler, if offset + data.len() > capacity,
/// the write returns Err and the handler is not modified.
proof fn proof_write_out_of_bounds_rejected<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    data: Seq<u8>,
)
    requires
        handler.well_formed(),
        offset as nat <= handler.used as nat,
        offset as nat + data.len() > N as nat,
{
    let mut h = handler;
    let result = h.write(offset, data);
    assert(result.is_err());
    assert(h =~= handler);
}

/// Proof: reading beyond the used region is rejected.
///
/// For ANY well-formed handler, if offset + len > used,
/// the read returns Err.
proof fn proof_read_out_of_bounds_rejected<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    len: usize,
)
    requires
        handler.well_formed(),
        offset as nat + len as nat > handler.used as nat,
{
    let result = handler.read(offset, len);
    assert(result.is_err());
}

/// Proof: clear resets to empty and preserves well-formedness.
proof fn proof_clear_resets_to_empty<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
)
    requires handler.well_formed(),
{
    let mut h = handler;
    h.clear();
    assert(h.well_formed());
    assert(h.used == 0);
    assert(h.view() =~= Seq::<u8>::empty());
    assert(h.capacity == N);
}

/// Proof: integrity verification always succeeds on a well-formed handler.
///
/// This proves that the checksum mechanism is sound: as long as the
/// handler is well-formed (i.e., no external corruption has occurred),
/// verify_integrity() always returns Ok.
proof fn proof_integrity_check_succeeds<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
)
    requires handler.well_formed(),
{
    let result = handler.verify_integrity();
    assert(result.is_ok());
}

/// Proof: checksum detects corruption.
///
/// If data is modified without updating the checksum (i.e., the handler
/// is no longer well-formed because ghost_checksum != compute_checksum(ghost_data)),
/// then the corrupted state is detectable.
///
/// We model this by showing that a handler with mismatched checksum
/// violates well-formedness, which means verify_integrity would catch it.
proof fn proof_checksum_detects_corruption<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    corrupted_data: Seq<u8>,
)
    requires
        handler.well_formed(),
        handler.used > 0,
        !(corrupted_data =~= handler.ghost_data),
        corrupted_data.len() == handler.ghost_data.len(),
{
    // If someone replaced ghost_data with corrupted_data but kept the old checksum,
    // the checksums would differ (by collision resistance axiom)
    axiom_checksum_collision_resistant(handler.ghost_data, corrupted_data);
    // Therefore: compute_checksum(corrupted_data) != handler.ghost_checksum
    // This means a handler with corrupted data but old checksum is NOT well_formed
    assert(compute_checksum(corrupted_data) != handler.ghost_checksum);
}

/// Proof: write preserves well-formedness.
proof fn proof_write_preserves_well_formed<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    data: Seq<u8>,
)
    requires
        handler.well_formed(),
        offset as nat <= handler.used as nat,
{
    let mut h = handler;
    let _ = h.write(offset, data);
    assert(h.well_formed());
}

/// Proof: clear preserves well-formedness.
proof fn proof_clear_preserves_well_formed<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
)
    requires handler.well_formed(),
{
    let mut h = handler;
    h.clear();
    assert(h.well_formed());
}

/// Proof: used length is always bounded by capacity.
///
/// For ANY well-formed handler, 0 <= used <= capacity == N.
/// This is the core ASIL-D invariant for memory safety.
proof fn proof_used_bounded_by_capacity<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
)
    requires handler.well_formed(),
    ensures
        handler.used <= N,
        handler.view().len() <= N as nat,
{
    // Direct consequence of well_formed() definition.
}

/// Proof: capacity is always N (constant).
///
/// The capacity of a VerifiedSafeMemoryHandler<N> is always exactly N,
/// regardless of the current state.
proof fn proof_capacity_is_constant<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
)
    requires handler.well_formed(),
    ensures handler.spec_capacity() == N as nat,
{
    // Direct consequence of spec_capacity() definition.
}

/// Proof: new() creates an empty, well-formed handler.
proof fn proof_new_is_empty_and_well_formed<const N: usize>()
{
    let handler = VerifiedSafeMemoryHandler::<N>::new();
    assert(handler.well_formed());
    assert(handler.used == 0);
    assert(handler.capacity == N);
    assert(handler.view() =~= Seq::<u8>::empty());
}

/// Proof: write at offset 0 to empty handler, then read back.
///
/// Specialized form of write-read consistency for the common case
/// of writing to a fresh (empty) handler.
proof fn proof_write_to_empty_then_read<const N: usize>(
    data: Seq<u8>,
)
    requires
        data.len() <= N as nat,
        N > 0,
{
    let mut h = VerifiedSafeMemoryHandler::<N>::new();
    assert(h.well_formed());
    assert(h.used == 0);

    let write_result = h.write(0, data);
    assert(write_result.is_ok());
    assert(h.well_formed());
    assert(h.used as nat == data.len());

    let read_result = h.read(0, data.len() as usize);
    assert(read_result.is_ok());
    match read_result {
        Ok(read_data) => {
            assert(read_data =~= data);
        },
        Err(()) => {
            assert(false);
        },
    }
}

/// Proof: write extends the used region correctly.
///
/// When writing past the current used mark, the used region
/// is extended to cover the written data.
proof fn proof_write_extends_used<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    data: Seq<u8>,
)
    requires
        handler.well_formed(),
        offset as nat <= handler.used as nat,
        offset as nat + data.len() <= N as nat,
        offset as nat + data.len() > handler.used as nat,
{
    let mut h = handler;
    let old_used = h.used;
    let result = h.write(offset, data);
    assert(result.is_ok());
    assert(h.used as nat == offset as nat + data.len());
    assert(h.used as nat > old_used as nat);
}

/// Proof: write within existing region does not change used length.
///
/// When writing entirely within the already-used region,
/// the used length remains unchanged.
proof fn proof_write_within_preserves_used<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
    data: Seq<u8>,
)
    requires
        handler.well_formed(),
        offset as nat + data.len() <= handler.used as nat,
        data.len() > 0,
{
    let mut h = handler;
    let old_used = h.used;
    let result = h.write(offset, data);
    assert(result.is_ok());
    assert(h.used == old_used);
}

/// Proof: reading from empty handler always fails (for non-zero length).
///
/// An empty handler has used == 0, so any read with len > 0 is out of bounds.
proof fn proof_read_empty_fails<const N: usize>(
    len: usize,
)
    requires len > 0,
{
    let h = VerifiedSafeMemoryHandler::<N>::new();
    assert(h.well_formed());
    assert(h.used == 0);
    let result = h.read(0, len);
    assert(result.is_err());
}

/// Proof: zero-length read always succeeds when offset is within bounds.
///
/// A read of length 0 at any valid offset (offset <= used) always succeeds.
proof fn proof_zero_length_read_succeeds<const N: usize>(
    handler: VerifiedSafeMemoryHandler<N>,
    offset: usize,
)
    requires
        handler.well_formed(),
        offset as nat <= handler.used as nat,
{
    let result = handler.read(offset, 0);
    assert(result.is_ok());
}

} // verus!
