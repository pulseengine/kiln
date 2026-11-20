//! `wasi:cli/environment.get-environment` returns exactly the variables given
//! with `--wasi-env`, as `NAME=VALUE` or as `NAME` (the value kiln itself sees),
//! in order. With no `--wasi-env` the guest environment is empty.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A command whose exit status encodes what `get-environment` returned:
/// `count * 16 + len(value of entry 0)`, or 200 when entry 0's name does not
/// start with `A`. An empty environment exits 0.
const ENV_PROBE: &str = r#"(module
    (import "wasi:cli/environment@0.2.0" "get-environment" (func $env (param i32)))
    (import "wasi:cli/exit@0.2.0" "exit-with-code" (func $exit (param i32)))
    (memory (export "memory") 1)
    (global $bump (mut i32) (i32.const 1024))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $p i32)
      ;; align the bump pointer up to param 2, return it, advance by param 3
      (local.set $p
        (i32.and
          (i32.add (global.get $bump) (i32.sub (local.get 2) (i32.const 1)))
          (i32.sub (i32.const 0) (local.get 2))))
      (global.set $bump (i32.add (local.get $p) (local.get 3)))
      (local.get $p))
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (local $list i32) (local $count i32)
      (call $env (i32.const 16))
      (local.set $list (i32.load (i32.const 16)))
      (local.set $count (i32.load (i32.const 20)))
      (if (i32.eqz (local.get $count))
        (then (call $exit (i32.const 0))))
      ;; entry 0 = (name_ptr, name_len, value_ptr, value_len)
      (if (i32.ne (i32.load8_u (i32.load (local.get $list))) (i32.const 65))
        (then (call $exit (i32.const 200))))
      (call $exit
        (i32.add
          (i32.mul (local.get $count) (i32.const 16))
          (i32.load offset=12 (local.get $list))))
      i32.const 0))"#;

fn run(file: &str, kiln_args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let path = std::env::temp_dir().join(file);
    fs::write(
        &path,
        wat::parse_str(ENV_PROBE).expect("fixture must assemble"),
    )
    .unwrap();
    let mut cmd = Command::new(KILN);
    cmd.arg("--wasi").args(kiln_args).arg(&path).env_remove("RUST_LOG");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let _ = fs::remove_file(&path);
    out
}

// rivet: verifies SR-116
#[test]
fn wasi_env_name_value_pairs_reach_the_guest_in_order() {
    let out = run(
        "kiln_env_pairs.wasm",
        &["--wasi-env", "A_FIRST=xyz", "--wasi-env", "B_SECOND=12"],
        &[],
    );
    // 2 entries, entry 0 is A_FIRST=xyz (value length 3): 2 * 16 + 3
    assert_eq!(
        out.status.code(),
        Some(35),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// rivet: verifies SR-116
#[test]
fn wasi_env_name_alone_passes_the_value_kiln_sees() {
    let out = run(
        "kiln_env_inherit.wasm",
        &["--wasi-env", "A_INHERITED"],
        &[("A_INHERITED", "hello")],
    );
    // 1 entry, value "hello" (length 5): 16 + 5
    assert_eq!(
        out.status.code(),
        Some(21),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// rivet: verifies SR-116
#[test]
fn without_wasi_env_the_guest_environment_is_empty() {
    let out = run("kiln_env_none.wasm", &[], &[("A_NOT_PASSED", "secret")]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// rivet: verifies SR-116
#[test]
fn wasi_env_name_that_kiln_does_not_have_is_refused() {
    let out = run(
        "kiln_env_missing.wasm",
        &["--wasi-env", "A_KILN_TEST_UNSET_VARIABLE"],
        &[],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "stderr: {stderr}");
    assert!(
        stderr.contains("A_KILN_TEST_UNSET_VARIABLE"),
        "the error names the variable\nstderr: {stderr}"
    );
}
