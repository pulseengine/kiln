//! `wasi:io/poll` `poll` returns its `list<u32>` of ready indices in a buffer
//! the guest allocates through `cabi_realloc`, 4-byte aligned, not at an
//! address the host picks (AD-WASIPOLL-001, SR-91).
//!
//! The fixture subscribes three pollables (one 10 s deadline, two zero-delay
//! ones that are ready at once), polls them, and reports on stdout what its
//! own `cabi_realloc` saw and what landed in the return area.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn module(allocator: &str) -> String {
    format!(
        r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (import "wasi:clocks/monotonic-clock@0.2.0" "subscribe-duration"
      (func $sub (param i64) (result i32)))
    (import "wasi:io/poll@0.2.0" "poll" (func $poll (param i32 i32 i32)))
    (memory (export "memory") 1)
    (global $calls (mut i32) (i32.const 0))
    (global $align (mut i32) (i32.const 0))
    (global $size (mut i32) (i32.const 0))
    (global $last (mut i32) (i32.const -1))
    {allocator}
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      ;; poll list at 64: [10 s deadline, ready, ready]
      (i32.store (i32.const 64) (call $sub (i64.const 10000000000)))
      (i32.store (i32.const 68) (call $sub (i64.const 0)))
      (i32.store (i32.const 72) (call $sub (i64.const 0)))
      (call $poll (i32.const 64) (i32.const 3) (i32.const 128))
      (i32.store8 (i32.const 200) (global.get $calls))
      (i32.store8 (i32.const 201) (global.get $align))
      (i32.store8 (i32.const 202) (global.get $size))
      (i32.store8 (i32.const 203) (i32.eq (i32.load (i32.const 128)) (global.get $last)))
      (i32.store8 (i32.const 204) (i32.load (i32.const 132)))
      (i32.store8 (i32.const 205) (i32.load (i32.load (i32.const 128))))
      (i32.store8 (i32.const 206)
        (i32.load (i32.add (i32.load (i32.const 128)) (i32.const 4))))
      (call $write (call $get_stdout) (i32.const 200) (i32.const 7) (i32.const 0))
      i32.const 0))"#
    )
}

/// A bump allocator that records its calls and its latest allocation.
const RECORDING_ALLOCATOR: &str = r#"
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
      (global.set $align (local.get 2))
      (global.set $size (local.get 3))
      (global.set $last (global.get $next))
      (global.set $next (i32.add (global.get $next)
        (i32.and (i32.add (local.get 3) (i32.const 7)) (i32.const -8))))
      (global.get $last))"#;

/// A `cabi_realloc` that always traps.
const TRAPPING_ALLOCATOR: &str = r#"
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) unreachable)"#;

fn run(file: &str, wat: &str) -> std::process::Output {
    let path = std::env::temp_dir().join(file);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    out
}

// rivet: verifies AD-WASIPOLL-001
#[test]
fn poll_returns_ready_indices_in_a_cabi_realloc_buffer() {
    let out = run("kiln_poll_owned_buf.wasm", &module(RECORDING_ALLOCATOR));
    assert!(
        out.status.success(),
        "run must succeed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    // calls=1, align=4, size=12 (3 * 4), ptr is the one cabi_realloc returned,
    // len=2, ready indices 1 and 2 (the 10 s deadline at index 0 is pending).
    assert_eq!(out.stdout, [1, 4, 12, 1, 2, 1, 2]);
}

// rivet: verifies AD-WASIPOLL-001
#[test]
fn poll_realloc_trap_propagates() {
    let out = run("kiln_poll_realloc_trap.wasm", &module(TRAPPING_ALLOCATOR));
    assert!(
        !out.status.success(),
        "a trapping cabi_realloc must fail the run, got {:?}",
        out.status
    );
    assert!(
        out.stdout.is_empty(),
        "no result may be reported after the trap"
    );
}
