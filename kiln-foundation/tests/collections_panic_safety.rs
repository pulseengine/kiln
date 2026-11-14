//! Regression tests: a panicking `Drop` must never cause an element to be
//! dropped twice by the static collections.
//!
//! Each element carries a shared per-element drop counter, and one element
//! panics on its first drop. The panic is caught, the container is dropped,
//! and every element must have been dropped exactly once.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU32, Ordering},
};

use kiln_foundation::collections::{StaticMap, StaticQueue, StaticVec};

const COUNT: usize = 4;
const PANICKING_ID: usize = 1;

/// Counters are leaked so that a double drop of an element can never free
/// them: the assertions must observe the count, not undefined behaviour.
type Counters = &'static [AtomicU32];

fn new_counters(len: usize) -> Counters {
    Box::leak((0..len).map(|_| AtomicU32::new(0)).collect::<Vec<_>>().into_boxed_slice())
}

struct Tracked {
    id: usize,
    counters: Counters,
    panics_on_first_drop: bool,
}

impl Tracked {
    /// An element that panics on its first drop only if it is `PANICKING_ID`.
    fn new(id: usize, counters: Counters) -> Self {
        Self::with_panic(id, counters, id == PANICKING_ID)
    }

    fn with_panic(id: usize, counters: Counters, panics_on_first_drop: bool) -> Self {
        Self {
            id,
            counters,
            panics_on_first_drop,
        }
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        let previous = self.counters[self.id].fetch_add(1, Ordering::SeqCst);
        if previous == 0 && self.panics_on_first_drop {
            panic!("intentional panic in Tracked::drop");
        }
    }
}

fn assert_each_dropped_once(counters: Counters) {
    for (id, count) in counters.iter().enumerate() {
        assert_eq!(count.load(Ordering::SeqCst), 1, "element {id} drop count");
    }
}

// rivet: verifies SR-104
#[test]
fn retain_drops_each_element_once_when_a_drop_panics() {
    let counters = new_counters(COUNT);
    {
        let mut vec = StaticVec::<Tracked, 8>::new();
        for id in 0..COUNT {
            vec.push(Tracked::new(id, counters)).unwrap();
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            vec.retain(|element| element.id != PANICKING_ID);
        }));
        assert!(result.is_err(), "the panicking drop must propagate");
    }
    assert_each_dropped_once(counters);
}

// rivet: verifies SR-104
#[test]
fn into_iter_drop_drops_each_element_once_when_a_drop_panics() {
    let counters = new_counters(COUNT);
    {
        let mut vec = StaticVec::<Tracked, 8>::new();
        for id in 0..COUNT {
            vec.push(Tracked::new(id, counters)).unwrap();
        }
        let iter = vec.into_iter();
        let result = catch_unwind(AssertUnwindSafe(move || drop(iter)));
        assert!(result.is_err(), "the panicking drop must propagate");
    }
    assert_each_dropped_once(counters);
}

// rivet: verifies SR-104
#[test]
fn map_clear_drops_each_element_once_when_a_drop_panics() {
    let counters = new_counters(COUNT);
    {
        let mut map = StaticMap::<u32, Tracked, 8>::new();
        for id in 0..COUNT {
            map.insert(id as u32, Tracked::new(id, counters)).unwrap();
        }
        let result = catch_unwind(AssertUnwindSafe(|| map.clear()));
        assert!(result.is_err(), "the panicking drop must propagate");
    }
    assert_each_dropped_once(counters);
}

// rivet: verifies SR-104
#[test]
fn queue_clear_drops_each_element_once_when_a_drop_panics() {
    let counters = new_counters(COUNT);
    {
        let mut queue = StaticQueue::<Tracked, 8>::new();
        for id in 0..COUNT {
            queue.push(Tracked::new(id, counters)).unwrap();
        }
        let result = catch_unwind(AssertUnwindSafe(|| queue.clear()));
        assert!(result.is_err(), "the panicking drop must propagate");
    }
    assert_each_dropped_once(counters);
}

/// Ids 0 and 1 are pushed, popped and dropped (count 1 each). Ids 2 and 3 then
/// sit in the first part of the ring (slots 2..4) and ids 4 and 5 in the
/// wrapped part (slots 0..2).
// rivet: verifies SR-104
#[test]
fn queue_clear_drops_each_element_once_when_a_drop_panics_after_wrapping() {
    const CAPACITY: usize = 4;
    const POPPED: usize = 2;
    const TOTAL: usize = CAPACITY + POPPED;
    const FIRST_PART_ID: usize = 2;
    const WRAPPED_PART_ID: usize = 5;

    // Two panicking drops during one clear would abort the process (a panic
    // while unwinding), so each part is exercised in its own scenario.
    for panicking_id in [FIRST_PART_ID, WRAPPED_PART_ID] {
        let counters = new_counters(TOTAL);
        {
            let mut queue = StaticQueue::<Tracked, CAPACITY>::new();
            let make = |id| Tracked::with_panic(id, counters, id == panicking_id);
            for id in 0..CAPACITY {
                queue.push(make(id)).unwrap();
            }
            for _ in 0..POPPED {
                drop(queue.pop().unwrap());
            }
            for id in CAPACITY..TOTAL {
                queue.push(make(id)).unwrap();
            }
            let result = catch_unwind(AssertUnwindSafe(|| queue.clear()));
            assert!(
                result.is_err(),
                "the panicking drop of {panicking_id} must propagate"
            );
        }
        assert_each_dropped_once(counters);
    }
}
