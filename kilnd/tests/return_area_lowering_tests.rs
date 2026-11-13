//! A WASI function whose result is a `result<..>` returned through a return
//! area (retptr) has that area written, and leaves no stray operand on the
//! stack, however the guest reaches it (SR-102).
//!
//! The dispatcher returns some results (`descriptor.get-type`,
//! `input-stream.read`, ...) as core values and relies on the engine to lower
//! them into the canonical-ABI return area. Calling them through a plain
//! `call` used to push the first value and write nothing. Each call below is
//! wrapped as `i32.const 41; call; i32.const 1; i32.add`, so a stray operand
//! makes the stored sum differ from 42.

use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn run(file: &str, wat: &str, stdin: &[u8]) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!("kiln_sr102_{file}_dir"));
    fs::create_dir_all(&dir).unwrap();
    let path = std::env::temp_dir().join(file);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let mut child = Command::new(KILN)
        .arg("--wasi")
        .arg("--wasi-fs")
        .arg(&dir)
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
    let _ = fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "run must succeed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// `descriptor.get-type(preopen, 128)` on the preopened directory: the area holds
/// `result<descriptor-type, error-code>` = ok (0), directory (3), and the
/// stack carried nothing extra (41 + 1 = 42).
const GET_TYPE: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (import "wasi:filesystem/types@0.2.0" "[method]descriptor.get-type"
      (func $gt (param i32 i32)))
    (memory (export "memory") 1)
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local i32)
      (i64.store (i32.const 128) (i64.const -1))
      (i32.const 41)
      ;; handle 1 is the `--wasi-fs` preopen (the first descriptor the host hands out)
      (call $gt (i32.const 1) (i32.const 128))
      (i32.const 1)
      (i32.add)
      (local.set 0)
      (i32.store8 (i32.const 200) (local.get 0))
      (i32.store8 (i32.const 201) (i32.load8_u (i32.const 128)))
      (i32.store8 (i32.const 202) (i32.load8_u (i32.const 129)))
      (call $write (call $get_stdout) (i32.const 200) (i32.const 3) (i32.const 0))
      i32.const 0))"#;

#[test]
fn get_type_writes_return_area_and_pushes_nothing() {
    assert_eq!(run("kiln_sr102_gt.wasm", GET_TYPE, b""), [42, 0, 3]);
}

/// `input-stream.read(stdin, 16, 160)`: stdin is not a file-backed stream, so
/// the host answers `result<list<u8>, stream-error>` = err (1) at +0 with
/// `stream-error::closed` (1) at +4, and the stack carries nothing extra.
const STDIN_READ: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:cli/stdin@0.2.0" "get-stdin" (func $get_stdin (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (import "wasi:io/streams@0.2.0" "[method]input-stream.read"
      (func $rd (param i32 i64 i32)))
    (memory (export "memory") 1)
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (local.get $r))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $s i32)
      (local.set $s (call $get_stdin))
      (i64.store (i32.const 160) (i64.const -1))
      (i64.store (i32.const 168) (i64.const -1))
      (i32.const 41)
      (call $rd (local.get $s) (i64.const 16) (i32.const 160))
      (i32.const 1)
      (i32.add)
      (local.set $s)
      (i32.store8 (i32.const 200) (local.get $s))
      (i32.store8 (i32.const 201) (i32.load8_u (i32.const 160)))
      (i32.store8 (i32.const 202) (i32.load (i32.const 164)))
      (call $write (call $get_stdout) (i32.const 200) (i32.const 3) (i32.const 0))
      i32.const 0))"#;

#[test]
fn input_stream_read_writes_return_area_and_pushes_nothing() {
    assert_eq!(run("kiln_sr102_rd.wasm", STDIN_READ, b"hello"), [42, 1, 1]);
}
