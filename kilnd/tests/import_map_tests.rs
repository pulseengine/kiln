//! `--import-map`: meld's per-import ownership picks the memory a host import
//! runs against (SR-81).
//!
//! A fused `--memory multi` module has one memory per component. A host import
//! belongs to one of them, and only meld's import map says which. kiln reads
//! the map only when told to (`--import-map <path>`), matches each import by
//! index and checks its name, and refuses what the map cannot answer.

use std::{fs, path::PathBuf, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// Two memories, both holding 0xFFFFFFFF at offset 16. The suffixed import
/// writes `none` (0) at its retptr (16) in the memory it is handed; `run`
/// traps unless memory 1 got the write and memory 0 did not.
const TWO_MEMORIES: &str = r#"(module
    (import "wasi:cli/terminal-stdin@0.2.6" "get-terminal-stdin$3" (func (param i32)))
    (memory 1)
    (memory 1)
    (data (memory 0) (i32.const 16) "\ff\ff\ff\ff")
    (data (memory 1) (i32.const 16) "\ff\ff\ff\ff")
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      i32.const 16
      call 0
      ;; trap unless memory 1 holds 0 and memory 0 still holds -1
      (if
        (i32.or
          (i32.load 1 (i32.const 16))
          (i32.xor (i32.load 0 (i32.const 16)) (i32.const -1)))
        (then unreachable))
      i32.const 0))"#;

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn map_json(name: &str, memory: &str) -> String {
    format!(
        r#"{{"version":2,"imports":[{{"index":0,"module":"wasi:cli/terminal-stdin@0.2.6",
        "name":"{name}","kind":"wasi","component":3,"memory":{memory},"realloc":null,
        "encoding":"utf8"}}]}}"#
    )
}

/// Runs kiln on `wat` with an optional import map; returns (success, stderr).
fn run(tag: &str, wat: &str, map: Option<&str>) -> (bool, String) {
    let module = temp(&format!("kiln_sr81_{tag}.wasm"));
    fs::write(&module, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let mut cmd = Command::new(KILN);
    let map_path = temp(&format!("kiln_sr81_{tag}.json"));
    if let Some(json) = map {
        fs::write(&map_path, json).unwrap();
        cmd.arg("--import-map").arg(&map_path);
    }
    let out = cmd.arg(&module).output().unwrap();
    let _ = fs::remove_file(&module);
    let _ = fs::remove_file(&map_path);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// rivet: verifies SR-81
#[test]
fn the_map_sends_a_suffixed_import_to_the_memory_it_names() {
    let json = map_json("get-terminal-stdin$3", r#"{"export":"memory$3","index":1}"#);
    let (ok, stderr) = run("memory1", TWO_MEMORIES, Some(&json));
    assert!(ok, "the write must land in memory 1\n{stderr}");
}

// rivet: verifies SR-81
#[test]
fn the_same_module_fails_when_the_map_names_the_other_memory() {
    let json = map_json("get-terminal-stdin$3", r#"{"export":"memory","index":0}"#);
    let (ok, _stderr) = run("memory0", TWO_MEMORIES, Some(&json));
    assert!(
        !ok,
        "mapped to memory 0 the write lands there, so the module's own check fails"
    );
}

// rivet: verifies SR-81
#[test]
fn without_a_map_the_suffixed_import_is_refused() {
    let (ok, stderr) = run("nomap", TWO_MEMORIES, None);
    assert!(!ok, "no owner, no call\n{stderr}");
    assert!(stderr.contains("owning instance"), "{stderr}");
}

// rivet: verifies SR-81
#[test]
fn an_unknown_owner_refuses_a_call_that_needs_memory() {
    let json = map_json(
        "get-terminal-stdin$3",
        r#"{"unknown":"could not be traced"}"#,
    );
    let (ok, stderr) = run("unknown", TWO_MEMORIES, Some(&json));
    assert!(!ok, "{stderr}");
    assert!(stderr.contains("could not determine"), "{stderr}");
}

// rivet: verifies SR-81
#[test]
fn a_null_memory_refuses_a_call_that_asks_for_memory() {
    let json = map_json("get-terminal-stdin$3", "null");
    let (ok, stderr) = run("null", TWO_MEMORIES, Some(&json));
    assert!(!ok, "{stderr}");
    assert!(stderr.contains("takes none"), "{stderr}");
}

// rivet: verifies SR-81
#[test]
fn a_map_entry_naming_another_import_is_an_error() {
    let json = map_json("get-terminal-stdin$9", r#"{"export":"memory$3","index":1}"#);
    let (ok, stderr) = run("mismatch", TWO_MEMORIES, Some(&json));
    assert!(!ok, "{stderr}");
    assert!(stderr.contains("different import"), "{stderr}");
}

// rivet: verifies SR-81
#[test]
fn a_malformed_or_missing_map_is_an_error() {
    let (ok, stderr) = run("badjson", TWO_MEMORIES, Some("{not json"));
    assert!(!ok, "{stderr}");
    assert!(stderr.contains("import map"), "{stderr}");

    let module = temp("kiln_sr81_absent_map.wasm");
    fs::write(&module, wat::parse_str(TWO_MEMORIES).unwrap()).unwrap();
    let out = Command::new(KILN)
        .arg("--import-map")
        .arg(temp("kiln_sr81_absent.json"))
        .arg(&module)
        .output()
        .unwrap();
    let _ = fs::remove_file(&module);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("import map"));
}
