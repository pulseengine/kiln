//! `--dir HOST::GUEST` preopens HOST under the guest-visible name GUEST, as
//! `wasmtime run --dir` does (SR-126).
//!
//! The fixture calls `get-directories` and prints the name of the first
//! preopen, so a test sees exactly the label that reached the guest.

use std::{fs, path::PathBuf, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// Calls `get-directories` and writes the first preopen's name to stdout.
const PRINT_FIRST_PREOPEN: &str = r#"(module
    (import "wasi:filesystem/preopens@0.2.0" "get-directories" (func $gd (param i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (param i32 i32 i32 i32)))
    (memory (export "memory") 1)
    (global $next (mut i32) (i32.const 4096))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (local.get $r))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $list i32)
      (call $gd (i32.const 128))
      (local.set $list (i32.load (i32.const 128)))
      (call $write (call $get_stdout)
        (i32.load offset=4 (local.get $list))
        (i32.load offset=8 (local.get $list))
        (i32.const 192))
      i32.const 0))"#;

fn fixture(file: &str) -> PathBuf {
    let path = std::env::temp_dir().join(file);
    fs::write(
        &path,
        wat::parse_str(PRINT_FIRST_PREOPEN).expect("fixture must assemble"),
    )
    .unwrap();
    path
}

fn host_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kiln_sr126_{tag}_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(file: &str, dir_arg: &str) -> std::process::Output {
    let path = fixture(file);
    let out = Command::new(KILN)
        .args(["--wasi", "--dir", dir_arg])
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    out
}

// rivet: verifies SR-126
#[test]
fn host_double_colon_guest_reports_the_guest_name() {
    let host = host_dir("named");
    let out = run(
        "kiln_sr126_named.wasm",
        &format!("{}::sandbox", host.display()),
    );
    let _ = fs::remove_dir_all(&host);
    assert!(
        out.status.success(),
        "kiln must run the guest; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "sandbox");
}

// rivet: verifies SR-126
#[test]
fn host_alone_reports_the_host_as_given() {
    let host = host_dir("plain");
    let arg = host.display().to_string();
    let out = run("kiln_sr126_plain.wasm", &arg);
    let _ = fs::remove_dir_all(&host);
    assert!(
        out.status.success(),
        "kiln must run the guest; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), arg);
}

// rivet: verifies SR-126
#[test]
fn a_missing_host_ends_kiln_and_names_the_argument() {
    let arg = "/nonexistent/kiln_sr126_missing::x";
    let out = run("kiln_sr126_missing.wasm", arg);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "kiln must not run without the preopen"
    );
    assert!(
        stderr.contains(arg),
        "stderr must name the argument: {stderr}"
    );
    assert!(out.stdout.is_empty(), "the guest must not have run");
}

// rivet: verifies SR-126
#[test]
fn an_empty_guest_name_ends_kiln_and_names_the_argument() {
    let host = host_dir("emptyguest");
    let arg = format!("{}::", host.display());
    let out = run("kiln_sr126_emptyguest.wasm", &arg);
    let _ = fs::remove_dir_all(&host);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "kiln must not run without the preopen"
    );
    assert!(
        stderr.contains(&arg),
        "stderr must name the argument: {stderr}"
    );
}
