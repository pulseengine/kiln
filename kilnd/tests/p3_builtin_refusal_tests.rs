//! P3 async built-ins that kiln serves either do their job or refuse (SR-83).
//!
//! Until kiln-async backs them (SR-84), `[waitable-set-poll]`,
//! `[waitable-join]`, `[waitable-set-drop]` and `[task-cancel]` trap with an
//! error that names the built-in, instead of returning 0 or nothing.
//! `[context-set-N]` refuses a missing or mistyped argument; `[context-get-N]`,
//! `[context-set-N]` with an i32 and `[task-return]` keep working.

use std::{fs, path::PathBuf, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn write_fixture(name: &str, wat: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    path
}

/// Runs kiln on the fixture; returns (exit success, stdout, stderr).
fn run_kiln(path: &PathBuf) -> (bool, String, String) {
    let out = Command::new(KILN).arg(path).output().unwrap();
    let _ = fs::remove_file(path);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A command module whose `run` calls one built-in (`import` is the full WAT
/// import line, `call` the instructions that call it and leave nothing on the
/// stack) and then returns 0, so an exit success means the call was a no-op.
fn calls_builtin(name: &str, import: &str, call: &str) -> (bool, String, String) {
    let wat = format!(
        r#"(module
            {import}
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              {call}
              i32.const 0))"#
    );
    run_kiln(&write_fixture(&format!("kiln_sr83_{name}.wasm"), &wat))
}

fn assert_refuses_naming(name: &str, builtin: &str, import: &str, call: &str) {
    let (ok, stdout, stderr) = calls_builtin(name, import, call);
    assert!(!ok, "{builtin} must trap, not return\n{stdout}\n{stderr}");
    assert!(
        stderr.contains(builtin),
        "the trap must name {builtin}\n{stderr}"
    );
    assert!(
        !stdout.contains("Execution completed successfully"),
        "a trapped module must not report success\n{stdout}"
    );
}

// rivet: verifies SR-83
#[test]
fn waitable_set_poll_traps_naming_itself() {
    assert_refuses_naming(
        "poll",
        "[waitable-set-poll]",
        r#"(import "$root" "[waitable-set-poll]" (func (param i32 i32) (result i32)))"#,
        "i32.const 1 i32.const 0 call 0 drop",
    );
}

// rivet: verifies SR-83
#[test]
fn waitable_join_traps_naming_itself() {
    assert_refuses_naming(
        "join",
        "[waitable-join]",
        r#"(import "$root" "[waitable-join]" (func (param i32 i32)))"#,
        "i32.const 1 i32.const 2 call 0",
    );
}

// rivet: verifies SR-83
#[test]
fn waitable_set_drop_traps_naming_itself() {
    assert_refuses_naming(
        "drop",
        "[waitable-set-drop]",
        r#"(import "$root" "[waitable-set-drop]" (func (param i32)))"#,
        "i32.const 1 call 0",
    );
}

// rivet: verifies SR-83
#[test]
fn task_cancel_traps_naming_itself() {
    assert_refuses_naming(
        "cancel",
        "[task-cancel]",
        r#"(import "[export]$root" "[task-cancel]" (func))"#,
        "call 0",
    );
}

/// The meld instance suffix (`$2`) is the same built-in and refuses too.
// rivet: verifies SR-83
#[test]
fn suffixed_builtin_traps_naming_itself() {
    assert_refuses_naming(
        "poll_suffixed",
        "[waitable-set-poll]",
        r#"(import "$root" "[waitable-set-poll]$2" (func (param i32 i32) (result i32)))"#,
        "i32.const 1 i32.const 0 call 0 drop",
    );
}

/// A mistyped argument is refused, not stored as 0. The import is declared
/// `(i64)` so the interpreter receives an i64 where `[context-set-N]` takes i32.
// rivet: verifies SR-83
#[test]
fn context_set_with_a_mistyped_argument_is_refused() {
    let (ok, stdout, stderr) = calls_builtin(
        "ctxset_mistyped",
        r#"(import "$root" "[context-set-0]" (func (param i64)))"#,
        "i64.const 7 call 0",
    );
    assert!(
        !ok,
        "a mistyped context-set argument must trap\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("[context-set-"),
        "the refusal must name the built-in\n{stderr}"
    );
}

/// A missing argument is refused, not stored as 0.
// rivet: verifies SR-83
#[test]
fn context_set_with_a_missing_argument_is_refused() {
    let (ok, stdout, stderr) = calls_builtin(
        "ctxset_missing",
        r#"(import "$root" "[context-set-0]" (func))"#,
        "call 0",
    );
    assert!(
        !ok,
        "a missing context-set argument must trap\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("[context-set-"),
        "the refusal must name the built-in\n{stderr}"
    );
}

/// What keeps working: context-set with an i32, context-get reading it back,
/// and task-return.
// rivet: verifies SR-83
#[test]
fn context_get_set_and_task_return_still_work() {
    let wat = r#"(module
        (import "$root" "[context-set-0]" (func $set (param i32)))
        (import "$root" "[context-get-0]" (func $get (result i32)))
        (import "$root" "[task-return]0" (func $ret (param i32)))
        (func (export "wasi:cli/run@0.2.0#run") (result i32)
          i32.const 42 call $set
          i32.const 7 call $ret
          ;; run returns 0 (success) only if the stored value reads back as 42
          call $get i32.const 42 i32.ne))"#;
    let (ok, stdout, stderr) = run_kiln(&write_fixture("kiln_sr83_ctx_ok.wasm", wat));
    assert!(
        ok,
        "valid context-get/set and task-return must work\n{stdout}\n{stderr}"
    );
}
