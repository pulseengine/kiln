//! A guest that reads `wasi:cli/stdin` receives the bytes the host's stdin
//! carries, then end-of-stream (SR-127, issue #712).
//!
//! `get-stdin` used to return an input-stream handle that `read` and
//! `blocking-read` answered with `stream-error::closed` at once, so every guest
//! saw an empty stdin whatever the host piped in.

use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// Echoes stdin to stdout: `blocking-read` up to 4096 bytes at a time into a
/// fresh `cabi_realloc` buffer, `blocking-write-and-flush` them to stdout, and
/// end when the read answers `stream-error::closed` (result tag 1 at the
/// return area). The return area at 160 is `result<list<u8>, stream-error>`:
/// tag at +0, list pointer at +4, list length at +8.
const ECHO: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:cli/stdin@0.2.0" "get-stdin" (func $get_stdin (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (import "wasi:io/streams@0.2.0" "[method]input-stream.blocking-read"
      (func $rd (param i32 i64 i32)))
    (memory (export "memory") 32)
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (local.get $r))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $in i32)
      (local $out i32)
      (local.set $in (call $get_stdin))
      (local.set $out (call $get_stdout))
      (loop $again
        (call $rd (local.get $in) (i64.const 4096) (i32.const 160))
        (if (i32.eqz (i32.load (i32.const 160)))
          (then
            (call $write (local.get $out) (i32.load (i32.const 164))
              (i32.load (i32.const 168)) (i32.const 200))
            (br $again))))
      i32.const 0))"#;

fn echo(file: &str, stdin: &[u8]) -> std::process::Output {
    let path = std::env::temp_dir().join(file);
    fs::write(&path, wat::parse_str(ECHO).expect("fixture must assemble")).unwrap();
    let mut child = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    let _ = fs::remove_file(&path);
    out
}

fn assert_echoed(file: &str, stdin: &[u8]) {
    let out = echo(file, stdin);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(out.stdout.len(), stdin.len(), "stderr: {stderr}");
    assert!(out.stdout == stdin, "stdout must equal the piped bytes");
}

// rivet: verifies SR-127
#[test]
fn short_stdin_reaches_the_guest() {
    assert_echoed("kiln_sr127_short.wasm", b"hello, stdin\n");
}

// rivet: verifies SR-127
#[test]
fn stdin_larger_than_one_read_reaches_the_guest_whole() {
    let input: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    assert_echoed("kiln_sr127_large.wasm", &input);
}

// rivet: verifies SR-127
#[test]
fn empty_stdin_is_end_of_stream_at_once() {
    assert_echoed("kiln_sr127_empty.wasm", b"");
}
