//! A `list<u8>` returned by `wasi:random/random` `get-random-bytes` lands in
//! memory the guest allocated through `cabi_realloc`, on both the direct-call
//! and the `call_indirect` host-call paths (SR-38).
//!
//! The host used to write the bytes at `retptr + 8` without calling
//! `cabi_realloc`, overwriting guest memory next to the return area. The
//! fixture puts a sentinel there and reports, on stdout, how many times
//! `cabi_realloc` ran, whether the sentinel survived, whether the returned
//! pointer is the one `cabi_realloc` handed out, and the returned length.
//!
//! `output-stream.write` likewise writes only inside its 12-byte return area,
//! and a trapping `cabi_realloc` during `get-arguments` fails the run instead
//! of leaving the host to rewrite the previous call's lists.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// `call` is the instruction sequence that invokes `get-random-bytes(8, 128)`.
fn module(call: &str) -> String {
    format!(
        r#"(module
    (type $grb_t (func (param i64 i32)))
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:random/random@0.2.0" "get-random-bytes" (func $grb (type $grb_t)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (table 1 funcref)
    (elem (i32.const 0) $grb)
    (memory (export "memory") 1)
    (global $calls (mut i32) (i32.const 0))
    (global $next (mut i32) (i32.const 4096))
    (global $last (mut i32) (i32.const -1))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
      (global.set $last (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (global.get $last))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      ;; sentinel right after the 8-byte return area at 128
      (i64.store (i32.const 136) (i64.const 0x5a5a5a5a5a5a5a5a))
      {call}
      (i32.store8 (i32.const 200) (global.get $calls))
      (i32.store8 (i32.const 201)
        (i64.eq (i64.load (i32.const 136)) (i64.const 0x5a5a5a5a5a5a5a5a)))
      (i32.store8 (i32.const 202) (i32.eq (i32.load (i32.const 128)) (global.get $last)))
      (i32.store8 (i32.const 203) (i32.load (i32.const 132)))
      (call $write (call $get_stdout) (i32.const 200) (i32.const 4) (i32.const 0))
      i32.const 0))"#
    )
}

fn run(file: &str, wat: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(file);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    assert!(
        out.status.success(),
        "run must succeed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// One `cabi_realloc` call, sentinel intact, pointer from the allocation, len 8.
const EXPECTED: &[u8] = &[1, 1, 1, 8];

#[test]
fn get_random_bytes_direct_call_uses_cabi_realloc() {
    let wat = module("(call $grb (i64.const 8) (i32.const 128))");
    assert_eq!(run("kiln_sr38_grb_direct.wasm", &wat), EXPECTED);
}

/// `output-stream.write` returns `result<_, stream-error>`: a 12-byte return
/// area. The host used to write a u64 "bytes written" at +8, 4 bytes past it.
const WRITE_RETURN_AREA: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.write" (func $write (type $w)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $flush_write (type $w)))
    (memory (export "memory") 1)
    (data (i32.const 64) "x")
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (i32.store (i32.const 128) (i32.const 0x77777777))
      (i64.store (i32.const 140) (i64.const 0x5a5a5a5a5a5a5a5a))
      (call $write (call $get_stdout) (i32.const 64) (i32.const 1) (i32.const 128))
      (i32.store8 (i32.const 200) (i32.load (i32.const 128)))
      (i32.store8 (i32.const 201)
        (i64.eq (i64.load (i32.const 140)) (i64.const 0x5a5a5a5a5a5a5a5a)))
      (call $flush_write (call $get_stdout) (i32.const 200) (i32.const 2) (i32.const 0))
      i32.const 0))"#;

#[test]
fn output_stream_write_stays_inside_its_return_area() {
    // "x" from the write, then ok-discriminant 0 and sentinel-intact 1.
    assert_eq!(
        run("kiln_sr38_write_area.wasm", WRITE_RETURN_AREA),
        b"x\x00\x01"
    );
}

/// `cabi_realloc` succeeds for the first `get-arguments` (program name + one
/// arg = 3 allocations) and traps on the second call's first allocation.
const ARGS_REALLOC_TRAP: &str = r#"(module
    (import "wasi:cli/environment@0.2.0" "get-arguments" (func $ga (param i32)))
    (memory (export "memory") 1)
    (global $calls (mut i32) (i32.const 0))
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
      (if (i32.gt_u (global.get $calls) (i32.const 3)) (then unreachable))
      (local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (local.get $r))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (call $ga (i32.const 128))
      (call $ga (i32.const 128))
      i32.const 0))"#;

#[test]
fn get_arguments_realloc_trap_is_not_swallowed() {
    let path = std::env::temp_dir().join("kiln_sr38_args_trap.wasm");
    fs::write(
        &path,
        wat::parse_str(ARGS_REALLOC_TRAP).expect("fixture must assemble"),
    )
    .unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .args(["--wasi-arg", "foo"])
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    // Swallowing the trap made the second call reuse the first call's lists and succeed.
    assert!(
        !out.status.success(),
        "a trapping cabi_realloc must fail the run, got {:?}",
        out.status
    );
}

#[test]
fn get_random_bytes_call_indirect_uses_cabi_realloc() {
    let wat = module("(call_indirect (type $grb_t) (i64.const 8) (i32.const 128) (i32.const 0))");
    assert_eq!(run("kiln_sr38_grb_indirect.wasm", &wat), EXPECTED);
}
