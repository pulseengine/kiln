//! WASI calls are not traced to stderr unless tracing is enabled, and trace
//! records never split program output (SR-67, issue #542).
//!
//! The v0.6.0 release binary printed a `[WASI-TRACE]` line on stderr for every
//! WASI call, unconditionally, interleaved with the program's own stderr. With
//! default settings kiln's stderr must carry the program's bytes and nothing
//! diagnostic; the program's stdout must be byte-exact too.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A core module that writes known bytes to stderr and stdout through
/// `wasi:io/streams` `blocking-write-and-flush`, in alternation, with no
/// trailing newline on the stderr data so a trace record landing mid-line
/// would be visible. Like meld-fused output, the write import is reached
/// through a table with `call_indirect`.
const WRITER: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stderr@0.2.0" "get-stderr" (func $get_stderr (result i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (table 1 funcref)
    (elem (i32.const 0) $write)
    (memory (export "memory") 1)
    (data (i32.const 16) "ERR-ONE|")
    (data (i32.const 32) "OUT-ONE|")
    (data (i32.const 48) "ERR-TWO|")
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $err i32) (local $out i32)
      (local.set $err (call $get_stderr))
      (local.set $out (call $get_stdout))
      (call_indirect (type $w) (local.get $err) (i32.const 16) (i32.const 8) (i32.const 0) (i32.const 0))
      (call_indirect (type $w) (local.get $out) (i32.const 32) (i32.const 8) (i32.const 0) (i32.const 0))
      (call_indirect (type $w) (local.get $err) (i32.const 48) (i32.const 8) (i32.const 0) (i32.const 0))
      i32.const 0))"#;

fn run_writer(name: &str) -> (String, String) {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(WRITER).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn default_run_prints_only_the_programs_own_stderr() {
    let (stdout, stderr) = run_writer("kiln_sr67_writer.wasm");
    assert!(
        stdout.contains("OUT-ONE|"),
        "the program's stdout bytes must be intact\n{stdout}"
    );
    assert_eq!(
        stderr, "ERR-ONE|ERR-TWO|",
        "kiln's stderr must be exactly the program's bytes, nothing diagnostic"
    );
}

#[test]
fn default_run_emits_no_wasi_diagnostics_on_either_stream() {
    let (stdout, stderr) = run_writer("kiln_sr67_diag.wasm");
    for tag in ["[WASI-TRACE]", "[WASI-PREALLOC]", "[DEBUG-"] {
        assert!(!stderr.contains(tag), "{tag} leaked to stderr\n{stderr}");
        assert!(!stdout.contains(tag), "{tag} leaked to stdout\n{stdout}");
    }
}
