//! P3 cooperative threads on the meld-fused core path (SR-82, issue #620).
//!
//! meld 0.67.0 passes the nine cooperative-thread built-ins through to the
//! host as `$root::[thread-*]` imports with the producer's core types and
//! exports the funcref table `[thread-new-indirect-v0]` indexes as
//! `__meld_thread_table`. The oracle is the seven #620 C components: fused by
//! meld 0.67.0 (fixtures/p3_threads, see its README), kiln must give the
//! results wasmtime gives for the original components.
//!
//! wasi-threads and shared-everything threads stay out of scope (meld ADR-9):
//! their imports are still refused before anything runs.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// The synchronous-task trap wasmtime raises for t4 and t6.
const SYNC_TASK_TRAP: &str = "cannot block a synchronous task before returning";

fn fixture(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/p3_threads")
        .join(name);
    // A missing fixture must fail the test, not make it assert nothing.
    assert!(path.is_file(), "fixture missing: {}", path.display());
    path
}

/// Runs `kiln --import-map <t>.map.json --invoke run <t>.core.wasm`; returns
/// (exit success, stdout, stderr).
fn run_oracle(name: &str) -> (bool, String, String) {
    let module = fixture(&format!("{name}.core.wasm"));
    let map = fixture(&format!("{name}.map.json"));
    let out = Command::new(KILN)
        .arg("--import-map")
        .arg(&map)
        .args(["--invoke", "run"])
        .arg(&module)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn assert_returns(name: &str, expected: i32) {
    let (ok, stdout, stderr) = run_oracle(name);
    assert!(ok, "{name} must run\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let line = format!("[0] i32 {expected}");
    assert!(
        stdout.contains(&line),
        "{name} must return {expected} (wasmtime's result)\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

fn assert_sync_task_trap(name: &str) {
    let (ok, stdout, stderr) = run_oracle(name);
    assert!(
        !ok,
        "{name} must trap, as on wasmtime\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(SYNC_TASK_TRAP),
        "{name} must trap with the synchronous-task error\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("returned"),
        "a trapped call must not report a result\n{stdout}"
    );
}

// rivet: verifies SR-82
#[test]
fn new_resume_returns_7() {
    assert_returns("t1_new_resume", 7);
}

// rivet: verifies SR-82
#[test]
fn yield_index_returns_3() {
    assert_returns("t2_yield_index", 3);
}

// rivet: verifies SR-82
#[test]
fn suspend_resume_returns_2() {
    assert_returns("t3_suspend_resume", 2);
}

// rivet: verifies SR-82
#[test]
fn suspend_then_resume_traps_with_the_synchronous_task_error() {
    assert_sync_task_trap("t4_suspend_then_resume");
}

// rivet: verifies SR-82
#[test]
fn yield_then_resume_returns_9() {
    assert_returns("t5_yield_then_resume", 9);
}

// rivet: verifies SR-82
#[test]
fn promote_traps_with_the_synchronous_task_error() {
    assert_sync_task_trap("t6_promote");
}

// rivet: verifies SR-82
#[test]
fn many_threads_returns_36() {
    assert_returns("t8_many_threads", 36);
}

fn write_wat(name: &str, wat: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    path
}

/// Runs kiln with extra arguments before the module; returns (ok, stdout, stderr).
fn run_kiln(args: &[&str], path: &PathBuf) -> (bool, String, String) {
    let out = Command::new(KILN).args(args).arg(path).output().unwrap();
    let _ = fs::remove_file(path);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// wasi-threads stays out of scope: its spawn import is still refused before
/// anything runs, alongside the cooperative built-ins kiln now serves.
// rivet: verifies SR-82
#[test]
fn wasi_threads_spawn_stays_refused() {
    let p = write_wat(
        "kiln_sr82_wasi_threads.wasm",
        r#"(module
            (import "wasi" "thread-spawn" (func (param i32) (result i32)))
            (import "$root" "[thread-yield]" (func (result i32)))
            (func (export "run") (result i32) i32.const 0))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(!ok, "wasi-threads must stay refused\n{stdout}\n{stderr}");
    assert!(
        stderr.contains("wasi::thread-spawn"),
        "the refusal must name the wasi-threads import\n{stderr}"
    );
    assert!(
        !stderr.contains("[thread-yield]"),
        "the cooperative built-in is served, so it is not named as unresolved\n{stderr}"
    );
}

/// `[thread-new-indirect-v0]` resolves fn_idx only through
/// `__meld_thread_table`; a module that does not export it is refused at the
/// call instead of indexing some other table.
// rivet: verifies SR-82
#[test]
fn new_indirect_without_the_meld_thread_table_is_refused() {
    let p = write_wat(
        "kiln_sr82_no_thread_table.wasm",
        r#"(module
            (import "$root" "[thread-new-indirect-v0]" (func $new (param i32 i32) (result i32)))
            (table 2 funcref)
            (elem (i32.const 1) func $worker)
            (func $worker (param i32))
            (func (export "run") (result i32)
              i32.const 1 i32.const 0 call $new))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(
        !ok,
        "new-indirect without __meld_thread_table must fail\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("__meld_thread_table"),
        "the error must name the missing table\n{stderr}"
    );
}

/// A start function of the wrong type traps instead of being called.
// rivet: verifies SR-82
#[test]
fn new_indirect_with_a_mistyped_start_function_traps() {
    let p = write_wat(
        "kiln_sr82_bad_start_type.wasm",
        r#"(module
            (import "$root" "[thread-new-indirect-v0]" (func $new (param i32 i32) (result i32)))
            (table (export "__meld_thread_table") 2 funcref)
            (elem (i32.const 1) func $worker)
            (func $worker (param i32) (result i32) i32.const 0)
            (func (export "run") (result i32)
              i32.const 1 i32.const 0 call $new))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(
        !ok,
        "a mistyped start function must trap\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("start function"),
        "the trap must say what is wrong\n{stderr}"
    );
}

/// Resuming a handle that names no thread traps (no thread is invented).
// rivet: verifies SR-82
#[test]
fn resume_later_of_an_unknown_handle_traps() {
    let p = write_wat(
        "kiln_sr82_unknown_handle.wasm",
        r#"(module
            (import "$root" "[thread-resume-later]" (func $later (param i32)))
            (func (export "run") (result i32)
              i32.const 42 call $later
              i32.const 0))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(
        !ok,
        "an unknown thread handle must trap\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("thread handle"),
        "the trap must name the bad handle\n{stderr}"
    );
}

/// The export returning while a thread it made ready never ran is refused:
/// kiln has no event loop to run that thread after the call, and dropping it
/// would report success for work that did not happen.
// rivet: verifies SR-82
#[test]
fn returning_with_a_ready_thread_left_is_refused() {
    let p = write_wat(
        "kiln_sr82_ready_left.wasm",
        r#"(module
            (import "$root" "[thread-new-indirect-v0]" (func $new (param i32 i32) (result i32)))
            (import "$root" "[thread-resume-later]" (func $later (param i32)))
            (table (export "__meld_thread_table") 2 funcref)
            (elem (i32.const 1) func $worker)
            (func $worker (param i32))
            (func (export "run") (result i32)
              i32.const 1 i32.const 0 call $new
              call $later
              i32.const 0))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(
        !ok,
        "a ready thread left at return must be refused\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("still ready to run"),
        "the refusal must say why\n{stderr}"
    );
}

/// A `$N`-suffixed thread built-in in a multi-memory module has no owner
/// without the import map (SR-81's rule), so it is refused.
// rivet: verifies SR-82
#[test]
fn suffixed_thread_import_without_a_map_is_refused_in_multi_memory() {
    let p = write_wat(
        "kiln_sr82_suffix_no_map.wasm",
        r#"(module
            (import "$root" "[thread-index]$1" (func $index (result i32)))
            (memory 1)
            (memory 1)
            (func (export "run") (result i32) call $index))"#,
    );
    let (ok, stdout, stderr) = run_kiln(&["--invoke", "run"], &p);
    assert!(
        !ok,
        "an ownerless suffixed import must be refused\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("owning instance"),
        "the refusal must say the owner is undetermined\n{stderr}"
    );
}
