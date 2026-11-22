//! kiln reads its own options only before the module path; the module path and
//! everything after it are the guest's argv, unchanged, as `wasmtime run` does
//! (SR-128, #713).
//!
//! The fixture prints every argument after argv[0] on its own line, so a test
//! sees exactly what reached the guest.

use std::{fs, path::PathBuf, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// Calls `get-arguments`, then writes each argument except argv[0] followed by
/// a newline to stdout.
const PRINT_ARGS: &str = r#"(module
    (import "wasi:cli/environment@0.2.0" "get-arguments" (func $ga (param i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (param i32 i32 i32 i32)))
    (memory (export "memory") 1)
    (global $next (mut i32) (i32.const 4096))
    (data (i32.const 64) "\n")
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (local.get $r))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $list i32) (local $n i32) (local $i i32) (local $out i32) (local $e i32)
      (call $ga (i32.const 128))
      (local.set $list (i32.load (i32.const 128)))
      (local.set $n (i32.load (i32.const 132)))
      (local.set $out (call $get_stdout))
      (local.set $i (i32.const 1))
      (block $done
        (loop $next_arg
          (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
          (local.set $e (i32.add (local.get $list) (i32.mul (local.get $i) (i32.const 8))))
          (call $write (local.get $out)
            (i32.load (local.get $e)) (i32.load offset=4 (local.get $e)) (i32.const 192))
          (call $write (local.get $out) (i32.const 64) (i32.const 1) (i32.const 192))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br $next_arg)))
      i32.const 0))"#;

fn fixture(file: &str) -> PathBuf {
    let path = std::env::temp_dir().join(file);
    fs::write(
        &path,
        wat::parse_str(PRINT_ARGS).expect("fixture must assemble"),
    )
    .unwrap();
    path
}

/// Runs `kiln <before...> <module> <after...>`.
fn run(file: &str, before: &[&str], after: &[&str]) -> std::process::Output {
    let path = fixture(file);
    let out = Command::new(KILN)
        .args(before)
        .arg(&path)
        .args(after)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    out
}

fn guest_saw(out: &std::process::Output) -> String {
    assert!(
        out.status.success(),
        "kiln must run the guest; status {:?}, stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Control for the fixture: plain words already reached the guest before SR-128.
// rivet: verifies SR-128
#[test]
fn plain_arguments_reach_the_guest() {
    let out = run("kiln_sr128_plain.wasm", &["--wasi"], &["a", "b c"]);
    assert_eq!(guest_saw(&out), "a\nb c\n");
}

// rivet: verifies SR-128
#[test]
fn a_dash_after_the_module_reaches_the_guest() {
    let out = run("kiln_sr128_dash.wasm", &["--wasi"], &["check", "-"]);
    assert_eq!(guest_saw(&out), "check\n-\n");
}

// rivet: verifies SR-128
#[test]
fn version_after_the_module_is_the_guests_argument() {
    let out = run("kiln_sr128_version.wasm", &["--wasi"], &["--version"]);
    assert_eq!(
        guest_saw(&out),
        "--version\n",
        "kiln must not answer --version itself"
    );
}

// rivet: verifies SR-128
#[test]
fn options_after_the_module_reach_the_guest_unchanged() {
    let out = run(
        "kiln_sr128_opts.wasm",
        &["--wasi"],
        &[
            "check", "f.smt2", "--format", "json", "--dir", ".", "--", "-x",
        ],
    );
    assert_eq!(
        guest_saw(&out),
        "check\nf.smt2\n--format\njson\n--dir\n.\n--\n-x\n"
    );
}

// rivet: verifies SR-128
#[test]
fn double_dash_before_the_module_ends_kiln_options() {
    let out = run("kiln_sr128_dd.wasm", &["--wasi", "--"], &["--help"]);
    assert_eq!(guest_saw(&out), "--help\n");
}

// rivet: verifies SR-128
#[test]
fn an_unknown_option_before_the_module_still_exits_2() {
    let out = run("kiln_sr128_unknown.wasm", &["--no-such-flag"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unrecognized argument '--no-such-flag'")
    );
}
