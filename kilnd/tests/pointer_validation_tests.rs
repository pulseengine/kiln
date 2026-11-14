//! Guest pointers a WASI host call receives or hands back are validated (SR-102).
//!
//! The canonical ABI requires a return-area pointer to be aligned for the result
//! and to keep the whole area inside linear memory, and a `cabi_realloc` result
//! to be aligned for the requested alignment and to keep `[ptr, ptr + size)`
//! inside linear memory. A violation traps (wasmtime: "pointer not aligned",
//! "pointer out of bounds", "realloc return: beyond end of memory"); the host
//! never accepts it, never silently skips the write, and never rounds a
//! zero-length request up to one byte.
//!
//! Each failing case has a positive control with a valid pointer, so a run
//! that fails here fails for the pointer and not for the fixture.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// `cabi_realloc` bump-allocates 8-byte aligned blocks, so it honours every alignment.
const REALLOC_BUMP: &str = r#"(local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next)
        (i32.and (i32.add (local.get 3) (i32.const 15)) (i32.const -8))))
      (local.get $r)"#;

/// `cabi_realloc` returns a pointer one past a block aligned for every alignment.
const REALLOC_MISALIGNED: &str = r#"(local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next)
        (i32.and (i32.add (local.get 3) (i32.const 15)) (i32.const -8))))
      (i32.add (local.get $r) (i32.const 1))"#;

/// `cabi_realloc` returns a pointer whose block runs past the end of the 64 KiB memory.
const REALLOC_PAST_END: &str = "(i32.const 65530)";

/// A core module with a one-page memory, a recording `cabi_realloc` and a `run`
/// export whose body is `run_body`. `$calls` counts `cabi_realloc` calls and
/// `$size` holds the `new_size` of the last one.
fn module(imports: &str, realloc_ret: &str, run_body: &str) -> String {
    format!(
        r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.write" (func $write (type $w)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $flush_write (type $w)))
    {imports}
    (memory (export "memory") 1)
    (data (i32.const 64) "x")
    (global $calls (mut i32) (i32.const 0))
    (global $size (mut i32) (i32.const -1))
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
      (global.set $size (local.get 3))
      {realloc_ret})
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      {run_body}
      i32.const 0))"#
    )
}

struct Run {
    success: bool,
    stderr: String,
}

fn run(name: &str, wat: &str) -> Run {
    let path = std::env::temp_dir().join(format!("kiln_sr102_{name}.wasm"));
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    Run {
        success: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// The run must fail and the error must name the problem.
fn assert_traps(name: &str, wat: &str, problem: &str) {
    let r = run(name, wat);
    assert!(
        !r.success,
        "{name}: a bad guest pointer must fail the run, but it succeeded"
    );
    assert!(
        r.stderr.contains(problem),
        "{name}: the error must mention {problem:?}, got:\n{}",
        r.stderr
    );
}

fn assert_runs(name: &str, wat: &str) {
    let r = run(name, wat);
    assert!(
        r.success,
        "{name}: the valid control must run:\n{}",
        r.stderr
    );
}

const NOW: &str = r#"(import "wasi:clocks/wall-clock@0.2.0" "now" (func $now (param i32)))"#;

fn now_at(retptr: u32) -> String {
    module(
        NOW,
        REALLOC_BUMP,
        &format!("(call $now (i32.const {retptr}))"),
    )
}

// wall-clock.now returns a `datetime` (u64 seconds, u32 nanoseconds): size 16, align 8.

#[test]
fn misaligned_return_area_traps() {
    assert_traps("now_misaligned", &now_at(1), "pointer not aligned");
}

#[test]
fn aligned_return_area_is_accepted() {
    assert_runs("now_aligned", &now_at(8));
}

#[test]
fn return_area_crossing_the_end_of_memory_traps() {
    // 8-aligned, but the 16-byte area ends at 65544 in a 65536-byte memory.
    assert_traps("now_crossing", &now_at(65528), "pointer out of bounds");
    // The same, with a pointer so large that `ptr + 16` wraps a u32.
    assert_traps(
        "now_wrapping",
        &now_at(0xFFFF_FFF8),
        "pointer out of bounds",
    );
}

#[test]
fn return_area_ending_at_the_end_of_memory_is_accepted() {
    assert_runs("now_at_end", &now_at(65520));
}

// output-stream.write returns `result<_, stream-error>`: size 12, align 4.

fn write_at(retptr: u32) -> String {
    module(
        "",
        REALLOC_BUMP,
        &format!(
            "(call $write (call $get_stdout) (i32.const 64) (i32.const 1) (i32.const {retptr}))"
        ),
    )
}

#[test]
fn twelve_byte_area_in_the_last_four_bytes_of_memory_traps() {
    assert_traps("write_last4", &write_at(65532), "pointer out of bounds");
}

#[test]
fn twelve_byte_area_that_fits_is_accepted() {
    assert_runs("write_fits", &write_at(65524));
}

fn flush_write_at(retptr: u32) -> String {
    module(
        "",
        REALLOC_BUMP,
        &format!(
            "(call $flush_write (call $get_stdout) (i32.const 64) (i32.const 1) (i32.const {retptr}))"
        ),
    )
}

#[test]
fn blocking_write_and_flush_with_an_out_of_range_retptr_traps() {
    // The call used to skip writing the result and return normally.
    assert_traps(
        "flush_oob",
        &flush_write_at(0xFFFF_FFF0),
        "pointer out of bounds",
    );
    assert_traps(
        "flush_crossing",
        &flush_write_at(65532),
        "pointer out of bounds",
    );
}

#[test]
fn blocking_write_and_flush_with_a_valid_retptr_is_accepted() {
    assert_runs("flush_ok", &flush_write_at(128));
}

// poll returns a `list<u32>`: its buffer is requested with align 4.

const POLL_IMPORTS: &str = r#"
    (import "wasi:clocks/monotonic-clock@0.2.0" "subscribe-duration"
      (func $subscribe (param i64) (result i32)))
    (import "wasi:io/poll@0.2.0" "poll" (func $poll (param i32 i32 i32)))"#;

const POLL_BODY: &str = "(i32.store (i32.const 256) (call $subscribe (i64.const 0)))
      (call $poll (i32.const 256) (i32.const 1) (i32.const 512))";

#[test]
fn realloc_result_misaligned_for_the_request_traps() {
    assert_traps(
        "poll_misaligned_realloc",
        &module(POLL_IMPORTS, REALLOC_MISALIGNED, POLL_BODY),
        "pointer not aligned",
    );
}

#[test]
fn realloc_result_aligned_for_the_request_is_accepted() {
    assert_runs(
        "poll_aligned_realloc",
        &module(POLL_IMPORTS, REALLOC_BUMP, POLL_BODY),
    );
}

// get-random-bytes returns a `list<u8>` in a buffer from cabi_realloc.

const RANDOM_IMPORTS: &str = r#"
    (import "wasi:random/random@0.2.0" "get-random-bytes" (func $grb (param i64 i32)))"#;

fn random_bytes(realloc_ret: &str, len: u32, check: &str) -> String {
    module(
        RANDOM_IMPORTS,
        realloc_ret,
        &format!("(call $grb (i64.const {len}) (i32.const 128))\n      {check}"),
    )
}

#[test]
fn realloc_result_past_the_end_of_memory_traps() {
    // 65530 + 16 > 65536; the block does not fit even though the pointer does.
    assert_traps(
        "grb_past_end",
        &random_bytes(REALLOC_PAST_END, 16, ""),
        "realloc return: beyond end of memory",
    );
}

#[test]
fn realloc_result_inside_memory_is_accepted() {
    assert_runs("grb_inside", &random_bytes(REALLOC_BUMP, 16, ""));
}

/// A read reserves an upper-bound buffer for the requested length but returns
/// only what it read (here nothing: stdin is empty). A request larger than memory
/// is not a bad `cabi_realloc` result, so it must not trap.
#[test]
fn oversized_read_request_does_not_trap() {
    let imports = r#"
    (import "wasi:cli/stdin@0.2.0" "get-stdin" (func $get_stdin (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]input-stream.read"
      (func $read (param i32 i64 i32)))"#;
    let body = "(call $read (call $get_stdin) (i64.const -1) (i32.const 64))";
    assert_runs("read_oversized", &module(imports, REALLOC_BUMP, body));
}

#[test]
fn zero_length_request_asks_realloc_for_zero_bytes() {
    // The run traps with `unreachable` unless cabi_realloc ran once, with
    // new_size 0 (wasmtime does not round an empty list up to one byte).
    let check = "(if (i32.ne (global.get $calls) (i32.const 1)) (then unreachable))
      (if (i32.ne (global.get $size) (i32.const 0)) (then unreachable))
      ;; the returned list is empty: (ptr, 0)
      (if (i32.ne (i32.load (i32.const 132)) (i32.const 0)) (then unreachable))";
    assert_runs("grb_zero", &random_bytes(REALLOC_BUMP, 0, check));
}
