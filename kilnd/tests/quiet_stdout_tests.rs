//! A kiln run's stdout carries only the program's output; the banner, startup
//! lines and statistics appear only with `--verbose`, on stderr (SR-74,
//! issue #574).
//!
//! The v0.6.0 binary printed a banner, `[INFO]` startup lines and run
//! statistics on stdout, so `kiln prog.wasm | consumer` handed the consumer
//! kiln's chatter mixed into the program's bytes.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A core module that writes known bytes (no trailing newline) to stdout
/// through `wasi:io/streams` `blocking-write-and-flush`.
const WRITER: &str = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (table 1 funcref)
    (elem (i32.const 0) $write)
    (memory (export "memory") 1)
    (data (i32.const 32) "OUT-ONE|OUT-TWO")
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (call_indirect (type $w) (call $get_stdout) (i32.const 32) (i32.const 15) (i32.const 0) (i32.const 0))
      i32.const 0))"#;

const PROGRAM_BYTES: &[u8] = b"OUT-ONE|OUT-TWO";

fn run_writer(name: &str, extra: &[&str]) -> (Vec<u8>, String) {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(WRITER).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .args(extra)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    assert!(out.status.success(), "run must succeed: {:?}", out.status);
    (out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn default_run_stdout_is_exactly_the_programs_bytes() {
    let (stdout, stderr) = run_writer("kiln_sr74_default.wasm", &[]);
    assert_eq!(
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(PROGRAM_BYTES),
        "stdout must carry only the program's output"
    );
    assert!(
        !stderr.contains("Kiln WebAssembly Runtime"),
        "no banner without --verbose\n{stderr}"
    );
}

#[test]
fn verbose_run_keeps_stdout_clean_and_puts_the_banner_on_stderr() {
    let (stdout, stderr) = run_writer("kiln_sr74_verbose.wasm", &["--verbose"]);
    assert_eq!(
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(PROGRAM_BYTES),
        "--verbose must not move kiln's own output onto stdout"
    );
    assert!(
        stderr.contains("Kiln WebAssembly Runtime"),
        "--verbose must print the banner on stderr\n{stderr}"
    );
    assert!(
        stderr.contains("[INFO]"),
        "--verbose must print startup lines on stderr\n{stderr}"
    );
    assert!(
        stderr.contains("Execution completed successfully"),
        "--verbose must print run statistics on stderr\n{stderr}"
    );
}

#[test]
fn help_documents_the_verbose_flag() {
    let out = Command::new(KILN).arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--verbose"), "help must mention --verbose\n{stdout}");
}
