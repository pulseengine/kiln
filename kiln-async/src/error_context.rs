// Kiln - kiln-async :: error_context
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Bounded backing for Component Model **`error-context`** handles, no-alloc.
//!
//! An `error-context` is an immutable, reference-counted handle carrying an
//! async error (with a debug message). The message itself lives in guest memory
//! — the host backing only tracks **which handles are live**, so a `drop` of a
//! stale or unknown handle fails loud rather than corrupting the table. A bounded
//! `[ErrorContextSlot; N]`, generation-indexed for ABA safety like the other
//! kiln-async handle tables.

use kiln_error::{Error, Result};

/// ABA-safe error-context handle (mirrors [`crate::FutureId`] / [`crate::SetId`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorContextId {
    /// Index into the error-context table.
    pub index: u16,
    /// Generation at the time this handle was issued.
    pub generation: u32,
}

impl ErrorContextId {
    /// Sentinel "no error-context" handle.
    pub const NONE: Self = Self {
        index: u16::MAX,
        generation: 0,
    };

    /// Whether this is the [`ErrorContextId::NONE`] sentinel.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.index == u16::MAX
    }
}

#[derive(Clone, Copy)]
struct ErrorContextSlot {
    live: bool,
    generation: u32,
}

/// Fixed-capacity table of live `error-context` handles, indexed by
/// [`ErrorContextId`]. Backs `error-context.{new,drop}`.
pub struct ErrorContextTable<const N: usize> {
    slots: [ErrorContextSlot; N],
    live: usize,
}

impl<const N: usize> ErrorContextTable<N> {
    /// Create an empty table — all `N` slots free.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [ErrorContextSlot {
                live: false,
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

    /// Number of live error-context handles.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether no handles are live.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Whether `id` refers to a live handle (generation-validated).
    #[must_use]
    pub fn is_live(&self, id: ErrorContextId) -> bool {
        self.slots
            .get(id.index as usize)
            .is_some_and(|s| s.live && s.generation == id.generation)
    }

    /// `error-context.new` — allocate a fresh handle. Fails loud at capacity.
    pub fn create(&mut self) -> Result<ErrorContextId> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !slot.live {
                slot.live = true;
                self.live += 1;
                return Ok(ErrorContextId {
                    index: index as u16,
                    generation: slot.generation,
                });
            }
        }
        Err(Error::foundation_bounded_capacity_exceeded(
            "kiln-async: error-context table at capacity",
        ))
    }

    /// `error-context.drop` — free a handle, bumping its generation so stale
    /// handles are detectable. Errors on a stale/unknown handle.
    pub fn drop_context(&mut self, id: ErrorContextId) -> Result<()> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .filter(|s| s.live && s.generation == id.generation)
            .ok_or_else(|| {
                Error::validation_error("kiln-async: drop of a stale or unknown error-context handle")
            })?;
        slot.live = false;
        slot.generation = slot.generation.wrapping_add(1);
        self.live -= 1;
        Ok(())
    }
}

impl<const N: usize> Default for ErrorContextTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_starts_empty_and_creates_live_handles() {
        let mut t: ErrorContextTable<2> = ErrorContextTable::new();
        assert_eq!(t.capacity(), 2);
        assert!(t.is_empty());
        let c = t.create().unwrap();
        assert_eq!(t.len(), 1);
        assert!(t.is_live(c));
        assert!(!c.is_none());
    }

    #[test]
    fn create_errors_at_capacity() {
        let mut t: ErrorContextTable<1> = ErrorContextTable::new();
        t.create().unwrap();
        assert!(t.create().is_err());
    }

    #[test]
    fn drop_frees_and_is_aba_safe() {
        let mut t: ErrorContextTable<1> = ErrorContextTable::new();
        let old = t.create().unwrap();
        t.drop_context(old).unwrap();
        assert!(t.is_empty());
        assert!(!t.is_live(old)); // freed
        let new = t.create().unwrap();
        assert_eq!(new.index, old.index);
        assert_ne!(new.generation, old.generation); // reuse bumped generation
        assert!(!t.is_live(old)); // stale handle invalid
        assert!(t.is_live(new));
    }

    #[test]
    fn drop_rejects_stale_or_unknown_handle() {
        let mut t: ErrorContextTable<1> = ErrorContextTable::new();
        assert!(t.drop_context(ErrorContextId::NONE).is_err());
        let c = t.create().unwrap();
        t.drop_context(c).unwrap();
        assert!(t.drop_context(c).is_err()); // already dropped
    }
}
