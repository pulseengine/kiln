//! Command-entry selection must not fall through to other exports (issue #530).
//!
//! With no `_start`, the entry is exactly one `wasi:cli/run@<ver>#run` export.
//! Its result is final: a trap must fail the process, and no other export may
//! be run in its place. Positional guesses ("0", "1", "main") are not entries.

use std::{fs, path::PathBuf, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn write_fixture(name: &str, wat: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    path
}

/// Runs kiln on the fixture; returns (exit success, stdout + stderr).
fn run_kiln(path: &PathBuf) -> (bool, String) {
    let out = Command::new(KILN).arg(path).output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_file(path);
    (out.status.success(), combined)
}

/// A trapping `run` must fail the process even though a numbered export would
/// have returned normally.
#[test]
fn trapping_run_export_fails_and_is_not_retried_as_another_export() {
    let p = write_fixture(
        "kilnd_530_trap.wasm",
        r#"(module
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              unreachable)
            (func (export "0") (result i32)
              i32.const 0))"#,
    );
    let (ok, combined) = run_kiln(&p);
    assert!(
        !ok,
        "a trapping wasi:cli/run must make kiln exit non-zero\n{combined}"
    );
    assert!(
        !combined.contains("Execution completed successfully"),
        "kiln must not report success for a failed run\n{combined}"
    );
}

/// A `run` that returns normally is the entry and succeeds.
#[test]
fn run_export_returning_normally_succeeds() {
    let p = write_fixture(
        "kilnd_530_ok.wasm",
        r#"(module
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0)
            (func (export "0") (result i32)
              unreachable))"#,
    );
    let (ok, combined) = run_kiln(&p);
    assert!(ok, "a run export returning 0 must succeed\n{combined}");
}

/// Two `run` exports are ambiguous; kiln refuses rather than picking one.
#[test]
fn more_than_one_run_export_is_refused() {
    let p = write_fixture(
        "kilnd_530_two_runs.wasm",
        r#"(module
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0)
            (func (export "wasi:cli/run@0.2.6#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, combined) = run_kiln(&p);
    assert!(!ok, "ambiguous run entries must be refused\n{combined}");
    assert!(
        combined.contains("more than one"),
        "the error must say there is more than one entry\n{combined}"
    );
}

/// Numbered and `main` exports are not command entries. Both trap if run, so
/// the listing message proves neither was executed.
#[test]
fn numbered_and_main_exports_are_not_run_as_entries() {
    let p = write_fixture(
        "kilnd_530_no_entry.wasm",
        r#"(module
            (func (export "0") (result i32) unreachable)
            (func (export "main") (result i32) unreachable))"#,
    );
    let (ok, combined) = run_kiln(&p);
    assert!(!ok, "a module with no command entry must fail\n{combined}");
    assert!(
        combined.contains("no command entry point"),
        "the no-entry listing must be shown, not a trap from a guessed export\n{combined}"
    );
}
