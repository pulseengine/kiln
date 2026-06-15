//! Kani (CBMC) formal-verification harnesses for kiln-async scheduler
//! invariants.
//!
//! kiln-async is `#![forbid(unsafe_code)]` (permanent — see SM-ASYNC-001 / the
//! plan §8 R1). Rather than backing an `unsafe` block, this firepower proves the
//! **safe scheduler invariants**: the lifecycle FSM is total and terminal states
//! are absorbing, the bounded ready queue never overflows, and freed task
//! handles are ABA-detectable. CBMC explores these exhaustively over the bounded
//! state space.
//!
//! Run: `cargo kani -p kiln-async --features kani`
//! (the installed `cargo-kiln kani-verify -p kiln-async` wraps this).

use crate::{ReadyQueue, TaskEvent, TaskId, TaskState, TaskTable};

fn any_state() -> TaskState {
    match kani::any::<u8>() % 6 {
        0 => TaskState::Spawned,
        1 => TaskState::Ready,
        2 => TaskState::Running,
        3 => TaskState::Blocked,
        4 => TaskState::Completed,
        _ => TaskState::Cancelled,
    }
}

fn any_event() -> TaskEvent {
    match kani::any::<u8>() % 7 {
        0 => TaskEvent::Admit,
        1 => TaskEvent::Dispatch,
        2 => TaskEvent::Yield,
        3 => TaskEvent::Wait,
        4 => TaskEvent::Wake,
        5 => TaskEvent::Complete,
        _ => TaskEvent::Cancel,
    }
}

/// FSM soundness: for *every* (state, event), `step` either returns a valid
/// transition or an error — it never panics — and a successful transition is
/// only possible from a non-terminal state (terminal states are absorbing).
#[kani::proof]
fn verify_fsm_step_total_and_terminal_absorbing() {
    let state = any_state();
    let event = any_event();
    match state.step(event) {
        Ok(_next) => {
            // A successful transition can only leave a non-terminal state.
            assert!(!state.is_terminal());
        }
        Err(_) => {
            // Illegal transitions are rejected loud — no panic, no silent Ok.
        }
    }
}

/// Bounded ready queue: across every sequence of push/pop, the queue length
/// never exceeds capacity (no overflow), and no operation panics.
#[kani::proof]
#[kani::unwind(7)]
fn verify_ready_queue_never_overflows() {
    let mut q: ReadyQueue<4> = ReadyQueue::new();
    let id = TaskId { index: 1, generation: 0 };
    for _ in 0..6 {
        if kani::any() {
            // push returns Err when full — never overflows the backing array.
            let _ = q.push(id);
        } else {
            let _ = q.pop();
        }
        assert!(q.len() <= q.capacity());
    }
}

/// ABA safety: after a slot is freed and reused, a handle to the old occupant
/// no longer validates (the generation was bumped), while the new handle does.
#[kani::proof]
fn verify_task_table_generation_aba_safe() {
    let mut t: TaskTable<2> = TaskTable::new();
    let old = t.spawn().unwrap();
    t.remove(old).unwrap();
    let new = t.spawn().unwrap();
    // Stale handle (old generation) must not resolve, even if the slot is reused.
    assert!(t.state_of(old).is_none());
    assert!(t.state_of(new).is_some());
}
