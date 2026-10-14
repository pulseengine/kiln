//! Passing arguments to an invoked export (SR-59, issue #480).
//!
//! Before this, kilnd had no CLI surface for wasm function parameters at all —
//! `--wasi-arg` is WASI argv, not wasm params — so any export taking >= 1
//! parameter was uninvokable. (Earlier still, the engine silently zero-filled
//! the missing parameters and reported the wrong answer as success; SR-53
//! replaced that with a loud refusal, which is what exposed this gap.)
//!
//! Scope: core scalars (i32/i64/f32/f64). Component-typed exports arrive here
//! already flattened by the canonical ABI into pointers and lengths and cannot
//! be constructed from the command line, because a meld-fused core module
//! carries no WIT type information — tracked as pulseengine/meld#400.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn fixture(name: &str) -> std::path::PathBuf {
    let wat = r#"(module
        (func (export "mul") (param i32) (param i32) (result i32)
          local.get 0 local.get 1 i32.mul)
        (func (export "addf") (param f64) (param f64) (result f64)
          local.get 0 local.get 1 f64.add)
        (func (export "noargs") (result i32) i32.const 7))"#;
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    path
}

fn run(path: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(KILN).args(args).arg(path).output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

/// The capability itself: arguments reach the export and the answer is correct.
#[test]
fn integer_arguments_are_passed_and_the_result_is_correct() {
    let p = fixture("kilnd_sr59_mul.wasm");
    let (ok, out) = run(&p, &["--invoke", "mul", "--arg", "21", "--arg", "2"]);
    assert!(ok, "mul(21,2) must run\n{out}");
    assert!(
        out.contains("42"),
        "mul(21,2) must return 42 — a wrong or zero-filled answer is the bug \
         this guards\n{out}"
    );
    let _ = fs::remove_file(&p);
}

#[test]
fn float_arguments_are_passed_and_printed_readably() {
    let p = fixture("kilnd_sr59_addf.wasm");
    let (ok, out) = run(&p, &["--invoke", "addf", "--arg", "1.5", "--arg", "2.25"]);
    assert!(ok, "addf(1.5,2.25) must run\n{out}");
    assert!(
        out.contains("3.75"),
        "the result must be the number 3.75, not a raw bit pattern\n{out}"
    );
    let _ = fs::remove_file(&p);
}

/// Arity is exact in both directions — a missing argument must never be
/// zero-filled (the SR-53 invariant), and an extra one must not be dropped.
#[test]
fn argument_arity_must_match_exactly() {
    let p = fixture("kilnd_sr59_arity.wasm");

    let (ok, out) = run(&p, &["--invoke", "mul", "--arg", "21"]);
    assert!(!ok, "too few arguments must fail\n{out}");
    assert!(
        out.contains("expects 2 argument(s) but 1 were supplied"),
        "the error must state both counts\n{out}"
    );

    let (ok, out) = run(&p, &["--invoke", "mul", "--arg", "1", "--arg", "2", "--arg", "3"]);
    assert!(!ok, "too many arguments must fail\n{out}");

    let _ = fs::remove_file(&p);
}

#[test]
fn unparseable_argument_fails_loud_naming_the_value_and_type() {
    let p = fixture("kilnd_sr59_badtype.wasm");
    let (ok, out) = run(&p, &["--invoke", "mul", "--arg", "notanumber", "--arg", "2"]);
    assert!(!ok, "a non-numeric argument for an i32 parameter must fail\n{out}");
    assert!(
        out.contains("notanumber") && out.contains("I32"),
        "the error must name the offending value and the expected type\n{out}"
    );
    let _ = fs::remove_file(&p);
}

/// A zero-parameter export must still work with no --arg at all.
#[test]
fn zero_parameter_export_still_runs_without_args() {
    let p = fixture("kilnd_sr59_noargs.wasm");
    let (ok, out) = run(&p, &["--invoke", "noargs"]);
    assert!(ok, "a zero-arg export must not require --arg\n{out}");
    assert!(out.contains("7"), "noargs() must return 7\n{out}");
    let _ = fs::remove_file(&p);
}
