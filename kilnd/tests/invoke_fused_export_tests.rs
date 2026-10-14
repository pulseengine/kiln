//! `--invoke` on the meld-fused core path (SR-58, issue #480).
//!
//! Post-RFC #46, kilnd refuses Component Model components and runs the core
//! module `meld fuse` produces. meld emits each component export under its
//! canonical WIT name (verified against the real released
//! `scry-3.3.0-wasm32-wasip2.wasm`, whose fused module exports
//! `pulseengine:scry/analyzer@0.1.0#analyze`), so no new cross-tool contract is
//! needed — kilnd just has to consume that name.
//!
//! It didn't: `--invoke` was wired only to the direct-component path, so a
//! library/reactor component (no `_start`, no `wasi:cli/run`) had **no**
//! invocation route at all and died with "No entry point found".
//!
//! These tests use a plain core module rather than a fused artifact so they run
//! without meld in CI; the code path under test — export-name resolution on the
//! core path — is the same one the fused module takes.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A library-shaped core module: no `_start`, one zero-arg exported function
/// carrying a canonical WIT-style name of the kind meld emits.
const CANONICAL_EXPORT: &str = "example:greeter/greet@0.1.0#hello";

fn library_shaped_module() -> Vec<u8> {
    let wat = format!(
        r#"(module
             (func (export "{CANONICAL_EXPORT}") (result i32) i32.const 42)
             (func (export "other_export") (result i32) i32.const 7))"#
    );
    wat::parse_str(&wat).expect("fixture must assemble")
}

fn write_fixture(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, library_shaped_module()).expect("fixture must be writable");
    path
}

/// SR-58: `--invoke <canonical-name>` must resolve the export on the core path.
/// Before the fix this was ignored and the run died at entry discovery.
#[test]
fn invoke_resolves_a_canonical_export_on_the_core_path() {
    let path = write_fixture("kilnd_sr58_invoke.wasm");

    let out = Command::new(KILN)
        .args(["--invoke", CANONICAL_EXPORT])
        .arg(&path)
        .output()
        .expect("kilnd must be executable");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        out.status.success(),
        "`--invoke {CANONICAL_EXPORT}` must resolve and run the export on the \
         fused-core path; got {:?}\n{combined}",
        out.status.code()
    );
    assert!(
        !combined.contains("No entry point found"),
        "--invoke must not fall through to entry discovery\n{combined}"
    );
    let _ = fs::remove_file(&path);
}

/// SR-58: a module with no command entry is usually a library component. The
/// old "No entry point found" told the user nothing; the error must now name
/// the exports they can actually invoke.
#[test]
fn library_module_without_invoke_lists_the_invokable_exports() {
    let path = write_fixture("kilnd_sr58_list.wasm");

    let out = Command::new(KILN).arg(&path).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "a library module with no entry and no --invoke must still fail"
    );
    assert!(
        stderr.contains("no command entry point"),
        "the error must say the module has no command entry, not just \
         'no entry point found'\n{stderr}"
    );
    assert!(
        stderr.contains(CANONICAL_EXPORT),
        "the error must name the invokable export so the user can act on it\n{stderr}"
    );
    let _ = fs::remove_file(&path);
}

/// Guard the precedence: an explicit `--function` still works, and `--invoke`
/// takes precedence when both are given (it is the more specific request).
#[test]
fn function_flag_still_works_and_invoke_takes_precedence() {
    let path = write_fixture("kilnd_sr58_prec.wasm");

    let via_function = Command::new(KILN)
        .args(["--function", CANONICAL_EXPORT])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        via_function.status.success(),
        "--function must keep resolving a named export"
    );

    // --invoke names a real export, --function names a missing one: --invoke wins.
    let both = Command::new(KILN)
        .args(["--function", "does_not_exist", "--invoke", CANONICAL_EXPORT])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        both.status.success(),
        "--invoke must take precedence over --function"
    );
    let _ = fs::remove_file(&path);
}
