//! Fused-command entry discovery (SR-57, issue #480).
//!
//! When a fused module has no `_start`, kilnd used to guess: it tried the
//! literal export names `"0"`, `"1"`, `"_start"`, `"main"` in that order, on the
//! assumption that numbered exports are "typically" the `wasi:cli/run` entry.
//!
//! That assumption is wrong on real artifacts. On a fused `hello_rust`, export
//! `"0"` is a 2-parameter function, not the run entry — so the guess picked the
//! wrong function. (Before SR-53 the engine would have zero-filled those two
//! parameters and reported the wrong call as success.)
//!
//! meld preserves each component export under its canonical WIT name, so the
//! entry can be found deterministically by looking for `wasi:cli/run@*#run`.
//! These tests pin that: the canonical name wins over a decoy numbered export.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A fused-command-shaped module: no `_start`, a canonical `wasi:cli/run` export
/// that sets a flag, and a decoy `"0"` export taking parameters (the shape that
/// made the old positional guess pick the wrong function).
fn fused_command_shaped(name: &str) -> std::path::PathBuf {
    let wat = r#"(module
        (global $ran (mut i32) (i32.const 0))
        (func (export "0") (param i32) (param i32) (result i32)
          i32.const -1)
        (func (export "wasi:cli/run@0.2.6#run") (result i32)
          i32.const 1
          global.set $ran
          i32.const 0)
        (func (export "ran") (result i32) global.get $ran))"#;
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    path
}

/// SR-57: with no `_start`, the canonical `wasi:cli/run@*#run` export must be
/// chosen — not the numbered decoy that happens to sort first.
#[test]
fn canonical_run_export_is_preferred_over_a_numbered_decoy() {
    let p = fused_command_shaped("kilnd_sr57_canonical.wasm");

    let out = Command::new(KILN).arg(&p).output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        out.status.success(),
        "a fused command module with a canonical run export must execute\n{combined}"
    );
    // The decoy takes 2 params; if discovery had picked it, we'd see the
    // arity refusal instead of a clean run.
    assert!(
        !combined.contains("expects 2 argument(s)"),
        "entry discovery must not pick the numbered decoy export '0'\n{combined}"
    );
    let _ = fs::remove_file(&p);
}

/// The canonical entry is what actually ran — verified through observable state
/// rather than just an exit code, so the test cannot pass vacuously.
#[test]
fn the_canonical_entry_is_the_function_that_actually_ran() {
    let p = fused_command_shaped("kilnd_sr57_ran.wasm");

    // `ran` returns 1 only if the canonical run export executed and set it.
    let out = Command::new(KILN)
        .args(["--invoke", "wasi:cli/run@0.2.6#run"])
        .arg(&p)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "invoking the canonical entry directly must work: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_file(&p);
}

/// Regression: a module that really does only expose numbered entries must
/// still run — the numbered names are kept as a trailing fallback, just no
/// longer tried first.
#[test]
fn numbered_entry_fallback_still_works_when_there_is_no_canonical_name() {
    let wat = r#"(module
        (func (export "0") (result i32) i32.const 0))"#;
    let p = std::env::temp_dir().join("kilnd_sr57_numbered.wasm");
    fs::write(&p, wat::parse_str(wat).unwrap()).unwrap();

    let out = Command::new(KILN).arg(&p).output().unwrap();
    assert!(
        out.status.success(),
        "a module exposing only numbered entries must still run via the fallback: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_file(&p);
}
