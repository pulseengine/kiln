//! A guest that ends itself with `wasi:cli/exit.exit-with-code` (or `proc_exit`)
//! ends the `kiln` process with that status. The runtime returns the status to
//! the embedder; kiln turns it into the process exit status (SR-116, SR-111).

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

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

fn exit_with_code(code: i32) -> String {
    format!(
        r#"(module
    (import "wasi:cli/exit@0.2.12" "exit-with-code" (func $exit (param i32)))
    (memory (export "memory") 1)
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (call $exit (i32.const {code}))
      i32.const 0))"#
    )
}

// rivet: verifies SR-116
#[test]
fn exit_with_code_3_ends_kiln_with_status_3() {
    let out = run("kiln_sr116_exit3.wasm", &exit_with_code(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        !stderr.contains("Execution failed"),
        "a guest exit is not a runtime failure\nstderr: {stderr}"
    );
}

// rivet: verifies SR-116
#[test]
fn exit_with_code_0_ends_kiln_with_status_0() {
    let out = run("kiln_sr116_exit0.wasm", &exit_with_code(0));
    assert_eq!(out.status.code(), Some(0));
}

/// The Canonical ABI lifts a `u8` from the core `i32` by its low 8 bits
/// (`lift_flat_unsigned`: `i % 2**8`): 259 (0x103) is status 3.
// rivet: verifies SR-116
#[test]
fn exit_with_code_259_lifts_to_status_3() {
    let out = run("kiln_sr116_exit259.wasm", &exit_with_code(259));
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// rivet: verifies SR-116
#[test]
fn proc_exit_ends_kiln_with_its_status() {
    let wat = r#"(module
    (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
    (memory (export "memory") 1)
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (call $exit (i32.const 7))
      i32.const 0))"#;
    let out = run("kiln_sr116_procexit7.wasm", wat);
    assert_eq!(out.status.code(), Some(7));
}
