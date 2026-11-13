//! Instantiation refuses a module with an unresolved import (SR-65, issue #527).
//!
//! The core spec requires instantiation to fail when an import cannot be
//! resolved. kiln must refuse before running anything — no start function, no
//! entry — and name every missing `module::name`, instead of instantiating and
//! failing (or not failing) only when the import is called.

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

/// The #527 reproduction: imports nothing serves, never called. wasmtime
/// refuses this with a link error; kiln used to run it to exit 0.
#[test]
fn uncalled_unresolved_imports_are_refused_and_each_is_named() {
    let p = write_fixture(
        "kiln_sr65_uncalled.wasm",
        r#"(module
            (import "no:such/iface@1.0.0" "missing" (func))
            (import "also:absent/x@2.0.0" "gone" (func (param i32) (result i32)))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&p);
    assert!(
        !ok,
        "a module with unresolved imports must be refused\n{stderr}"
    );
    assert!(
        stderr.contains("unresolved import"),
        "the refusal must say why\n{stderr}"
    );
    assert!(
        stderr.contains("no:such/iface@1.0.0::missing"),
        "every missing import must be named\n{stderr}"
    );
    assert!(
        stderr.contains("also:absent/x@2.0.0::gone"),
        "every missing import must be named, not only the first\n{stderr}"
    );
    assert!(
        !stdout.contains("Execution completed successfully"),
        "a refused module must not report success\n{stdout}"
    );
}

/// The refusal comes before anything runs: the start function would trap, so
/// a trap in the output means kiln ran code before checking imports.
#[test]
fn refusal_happens_before_the_start_function_runs() {
    let p = write_fixture(
        "kiln_sr65_start.wasm",
        r#"(module
            (import "no:such/iface@1.0.0" "missing" (func))
            (func $boom unreachable)
            (start $boom)
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(
        !ok,
        "a module with an unresolved import must be refused\n{stderr}"
    );
    assert!(
        stderr.contains("no:such/iface@1.0.0::missing"),
        "the refusal, not the start trap, must be what stops the module\n{stderr}"
    );
    assert!(
        !stderr.to_lowercase().contains("unreachable"),
        "the start function must not have run\n{stderr}"
    );
}

/// kiln serves no memories, tables or globals to a lone module; importing one
/// is unresolvable too.
#[test]
fn unresolved_memory_import_is_refused() {
    let p = write_fixture(
        "kiln_sr65_memory.wasm",
        r#"(module
            (import "env" "memory" (memory 1))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(!ok, "an unresolved memory import must be refused\n{stderr}");
    assert!(
        stderr.contains("env::memory"),
        "the import must be named\n{stderr}"
    );
}

/// The check must not refuse what kiln does serve: versioned WASI P2 imports
/// and the P1 `proc_exit` resolve, so this module runs.
#[test]
fn served_wasi_imports_are_not_refused() {
    let p = write_fixture(
        "kiln_sr65_served.wasm",
        r#"(module
            (import "wasi:cli/stdout@0.2.0" "get-stdout" (func (result i32)))
            (import "wasi:cli/environment@0.2.6" "get-arguments" (func (param i32)))
            (import "wasi_snapshot_preview1" "proc_exit" (func (param i32)))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(ok, "imports kiln serves must not be refused\n{stderr}");
    assert!(
        !stderr.contains("unresolved import"),
        "no import here is unresolved\n{stderr}"
    );
}

/// SR-81: meld's `$N` suffix on a served WASI function is not an unresolved
/// import; the link check resolves it to the base function.
// rivet: verifies SR-81
#[test]
fn suffixed_served_wasi_imports_are_not_refused() {
    let p = write_fixture(
        "kiln_sr81_suffixed.wasm",
        r#"(module
            (import "wasi:cli/stderr@0.2.6" "get-stderr$5" (func (result i32)))
            (import "wasi:cli/exit@0.2.6" "exit$2" (func (param i32)))
            (import "wasi:io/streams@0.2.6" "[method]output-stream.blocking-write-and-flush$5"
              (func (param i32 i32 i32 i32)))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(
        ok,
        "suffixed imports kiln serves must not be refused\n{stderr}"
    );
    assert!(
        !stderr.contains("unresolved import"),
        "no import here is unresolved\n{stderr}"
    );
}

/// SR-81: a suffixed name whose base is not a served function is still
/// refused with E21A0, as are an empty or non-numeric suffix.
// rivet: verifies SR-81
#[test]
fn suffixed_unknown_and_malformed_imports_are_still_refused() {
    let p = write_fixture(
        "kiln_sr81_unknown.wasm",
        r#"(module
            (import "wasi:cli/stderr@0.2.6" "not-a-function$3" (func))
            (import "wasi:cli/stderr@0.2.6" "get-stderr$" (func (result i32)))
            (import "wasi:cli/stderr@0.2.6" "get-stderr$x" (func (result i32)))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(!ok, "unknown suffixed imports must be refused\n{stderr}");
    assert!(
        stderr.contains("E21A0"),
        "the E21A0 path must be taken\n{stderr}"
    );
    for name in [
        "wasi:cli/stderr@0.2.6::not-a-function$3",
        "wasi:cli/stderr@0.2.6::get-stderr$,",
        "wasi:cli/stderr@0.2.6::get-stderr$x",
    ] {
        // The empty-suffix name is followed by a separator in the listing.
        let needle = name.trim_end_matches(',');
        assert!(stderr.contains(needle), "{needle} must be named\n{stderr}");
    }
}

/// SR-81: with one memory the owner of a suffixed import is unambiguous and
/// the call runs.
// rivet: verifies SR-81
#[test]
fn suffixed_import_call_runs_in_a_single_memory_module() {
    let p = write_fixture(
        "kiln_sr81_single_memory_call.wasm",
        r#"(module
            (import "wasi:cli/stderr@0.2.6" "get-stderr$5" (func (result i32)))
            (memory 1)
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              call 0
              drop
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(
        ok,
        "a suffixed call in a one-memory module must run\n{stderr}"
    );
}

/// SR-81: with several memories nothing in the module names the memory of the
/// instance that owns a suffixed import (meld's import map is not consumed and
/// the suffix is a component index, not a memory index). kiln refuses the
/// call instead of using memory 0 (the import is named in the trace log).
// rivet: verifies SR-81
#[test]
fn suffixed_import_call_in_a_multi_memory_module_fails_loud() {
    let p = write_fixture(
        "kiln_sr81_multi_memory_call.wasm",
        r#"(module
            (import "wasi:cli/stderr@0.2.6" "get-stderr$5" (func (result i32)))
            (memory 1)
            (memory 1)
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              call 0
              drop
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(!ok, "an owner-less suffixed call must fail\n{stderr}");
    assert!(
        stderr.contains("owning instance"),
        "the refusal must say the owner is undeterminable\n{stderr}"
    );
}

/// The corpus case behind #527: yolo_inference imports wasi:nn, which the
/// default kiln build does not serve. It must be refused, not counted as a pass.
#[cfg(not(feature = "wasi-nn"))]
#[test]
fn wasi_nn_import_is_refused_when_not_served() {
    let p = write_fixture(
        "kiln_sr65_nn.wasm",
        r#"(module
            (import "wasi:nn/inference@0.2.0-rc-2024-10-28" "load"
              (func (param i32 i32 i32 i32 i32)))
            (func (export "wasi:cli/run@0.2.0#run") (result i32)
              i32.const 0))"#,
    );
    let (ok, _stdout, stderr) = run_kiln(&p);
    assert!(!ok, "an unserved wasi:nn import must be refused\n{stderr}");
    assert!(
        stderr.contains("wasi:nn/inference@0.2.0-rc-2024-10-28::load"),
        "the import must be named\n{stderr}"
    );
}
