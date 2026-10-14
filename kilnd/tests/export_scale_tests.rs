//! kiln loads modules with real-app export counts (SR-34, issue #391).
//!
//! witness MC/DC instrumentation exports one global per branch counter, so an
//! instrumented core carries thousands of exports: 4,092 for the released
//! `hello_rust` example, 35,581 for `hello_go`. kiln's runtime export table
//! held 256 entries and the decoder's default profile accepted 16,384, so these
//! modules failed at load ("DirectMap capacity exceeded" / "exceeds maximum
//! export count"), blocking the meld → witness → kiln MC/DC path.
//!
//! The fixtures are generated: `n` exported mutable globals plus a `_start`
//! that reads the last one, so a module that loaded but lost exports would
//! still fail to resolve it.

use std::{fmt::Write as _, fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

fn module_with_exported_globals(n: usize) -> Vec<u8> {
    let mut wat = String::from("(module\n");
    for i in 0..n {
        writeln!(
            wat,
            "  (global (export \"g{i}\") (mut i32) (i32.const {i}))"
        )
        .unwrap();
    }
    writeln!(
        wat,
        "  (func (export \"_start\") (drop (global.get {})))\n)",
        n - 1
    )
    .unwrap();
    wat::parse_str(&wat).expect("fixture must assemble")
}

fn run(bytes: &[u8], file: &str, extra: &[&str]) -> (Option<i32>, String) {
    let path = std::env::temp_dir().join(file);
    fs::write(&path, bytes).unwrap();
    let out = Command::new(KILN).args(extra).arg(&path).output().unwrap();
    let _ = fs::remove_file(&path);
    (
        out.status.code(),
        format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn loads_a_module_with_witness_scale_exports() {
    // hello_rust instrumented by witness 0.44.0: 4,092 exports.
    let (code, out) = run(
        &module_with_exported_globals(4_100),
        "kiln_sr34_4k.wasm",
        &[],
    );
    assert_eq!(code, Some(0), "{out}");
}

#[test]
fn loads_a_module_with_go_scale_exports() {
    // hello_go instrumented by witness 0.44.0: 35,581 exports.
    let (code, out) = run(
        &module_with_exported_globals(36_000),
        "kiln_sr34_36k.wasm",
        &[],
    );
    assert_eq!(code, Some(0), "{out}");
}

#[test]
fn every_export_stays_reachable_by_name() {
    // Lookup must still resolve names past the old 256-entry table: invoke
    // an export that sits deep in a large export section.
    let mut wat = String::from("(module\n");
    for i in 0..3_000 {
        writeln!(wat, "  (global (export \"g{i}\") i32 (i32.const {i}))").unwrap();
    }
    wat.push_str("  (func (export \"deep\") (result i32) (global.get 2999))\n)");
    let wasm = wat::parse_str(&wat).unwrap();
    let (code, out) = run(&wasm, "kiln_sr34_deep.wasm", &["--invoke", "deep"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("[0] i32 2999"), "{out}");
}
