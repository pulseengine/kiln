//! witness MC/DC coverage harness mode (issue #340 — REQ_WITNESS_COV / AD-MCDC-001).
//!
//! The `witness` tool measures MC/DC coverage by instrumenting core modules with
//! exported counter globals (`__witness_counter_<id>`) and reading their values
//! back after a run. It drives an external runtime via a "harness": it spawns the
//! configured command with three env vars and reads a snapshot JSON file back:
//!
//! - `WITNESS_MODULE`   — abs path to the instrumented `.wasm` (read this)
//! - `WITNESS_MANIFEST` — abs path to the manifest (optional for the v1 contract)
//! - `WITNESS_OUTPUT`   — abs path the harness MUST write the snapshot JSON to
//!
//! This module makes `kilnd` such a harness. It runs the module, snapshots the
//! counter globals via [`ModuleInstance::export_global_snapshot`], and writes the
//! `witness-harness-v1` JSON to `WITNESS_OUTPUT`:
//!
//! ```json
//! { "schema": "witness-harness-v1", "counters": { "0": 7, "1": 0 } }
//! ```
//!
//! The `counters` keys are the BARE numeric branch id (witness does
//! `key.parse::<u32>()`), so the `__witness_counter_` prefix is stripped here.
//! Contract pinned with witness on #340 (REQ_WITNESS_COV `harness-contract`).
//! The richer `witness-harness-v2` (per-row MC/DC) is intentionally not built yet.

use std::collections::BTreeMap;

use kiln_error::{Error, Result};

/// Prefix witness uses for its per-branch counter globals.
pub const WITNESS_COUNTER_PREFIX: &str = "__witness_counter_";

/// The `witness-harness-v1` schema identifier.
const WITNESS_HARNESS_SCHEMA: &str = "witness-harness-v1";

/// Build the `witness-harness-v1` snapshot JSON from a `name -> value` global
/// snapshot (as returned by `ModuleInstance::export_global_snapshot`).
///
/// Strips `prefix` from each global name to recover the bare numeric branch id
/// witness expects, and emits
/// `{"schema":"witness-harness-v1","counters":{"<id>":N}}` with the ids in
/// numeric order. A name that does not carry `prefix`, or whose remainder is not
/// a `u32`, is a contract violation and returns an error (fail-loud).
pub fn build_counters_json(snapshot: &BTreeMap<String, i64>, prefix: &str) -> Result<String> {
    // Re-key by parsed numeric id so the output is in numeric (not lexical) order
    // and any non-numeric id is rejected up front.
    let mut counters: BTreeMap<u32, i64> = BTreeMap::new();
    for (name, value) in snapshot {
        let id_str = name.strip_prefix(prefix).ok_or_else(|| {
            Error::runtime_error("witness counter global is missing the expected prefix")
        })?;
        let id: u32 = id_str.parse().map_err(|_| {
            Error::runtime_error("witness counter id is not a numeric u32")
        })?;
        counters.insert(id, *value);
    }

    let mut json = String::from("{\"schema\":\"");
    json.push_str(WITNESS_HARNESS_SCHEMA);
    json.push_str("\",\"counters\":{");
    for (i, (id, value)) in counters.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"{id}\":{value}"));
    }
    json.push_str("}}");
    Ok(json)
}

/// Entry exports tried, in order, to drive the instrumented module. `_start` and
/// `run` are the synchronous WASI/command entries; `0`/`1`/`main` cover
/// meld-fused P3 cores whose entry is a numbered `wasi:cli/run` export. The first
/// that exists is invoked; if none do, the harness fails loud.
const ENTRY_CANDIDATES: &[&str] = &["_start", "run", "0", "1", "main"];

/// Fuel budget for a coverage run. Generous but finite so a non-terminating
/// module is a bounded failure, never an unbounded hang (cf. SM-RES-002).
#[cfg(all(feature = "std", feature = "kiln-execution"))]
const DEFAULT_HARNESS_FUEL: u64 = 10_000_000_000;

/// True if a core module (binary version 1), false if a Component (version != 1).
fn is_core_module(wasm: &[u8]) -> bool {
    wasm.len() >= 8
        && wasm[0..4] == [0x00, 0x61, 0x73, 0x6d]
        && wasm[4..8] == [0x01, 0x00, 0x00, 0x00]
}

/// True when witness invoked us: both `WITNESS_MODULE` and `WITNESS_OUTPUT` set.
#[must_use]
pub fn harness_requested() -> bool {
    std::env::var_os("WITNESS_MODULE").is_some() && std::env::var_os("WITNESS_OUTPUT").is_some()
}

/// Run the witness harness from the env-var contract (`WITNESS_MODULE` ->
/// `WITNESS_OUTPUT`). Called from `main` when [`harness_requested`] is true.
#[cfg(all(feature = "std", feature = "kiln-execution"))]
pub fn run_from_env() -> Result<()> {
    let module = std::env::var("WITNESS_MODULE")
        .map_err(|_| Error::runtime_error("WITNESS_MODULE env var not set"))?;
    let output = std::env::var("WITNESS_OUTPUT")
        .map_err(|_| Error::runtime_error("WITNESS_OUTPUT env var not set"))?;
    run(std::path::Path::new(&module), std::path::Path::new(&output))
}

/// Load the instrumented core at `module_path`, run it, snapshot the
/// `__witness_counter_*` globals, and write the `witness-harness-v1` JSON to
/// `output_path`.
#[cfg(all(feature = "std", feature = "kiln-execution"))]
pub fn run(module_path: &std::path::Path, output_path: &std::path::Path) -> Result<()> {
    use kiln_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};

    let wasm = std::fs::read(module_path)
        .map_err(|_| Error::runtime_error("WITNESS_MODULE could not be read"))?;

    // v1 instruments meld-fused CORE modules; a Component (version != 1) is out
    // of scope — fail loud rather than guess at component-internal globals.
    if !is_core_module(&wasm) {
        return Err(Error::runtime_error(
            "WITNESS_MODULE is not a core module; witness-harness-v1 expects a meld-fused core",
        ));
    }

    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)
        .map_err(|_| Error::runtime_error("failed to create engine"))?;
    engine.set_fuel(DEFAULT_HARNESS_FUEL);

    let module_handle = engine
        .load_module(&wasm)
        .map_err(|_| Error::runtime_execution_error("failed to load WITNESS_MODULE"))?;
    let instance = engine
        .instantiate(module_handle)
        .map_err(|_| Error::runtime_execution_error("failed to instantiate WITNESS_MODULE"))?;

    let entry = ENTRY_CANDIDATES
        .iter()
        .copied()
        .find(|name| engine.has_function(instance, name).unwrap_or(false))
        .ok_or_else(|| {
            Error::runtime_function_not_found(
                "no witness harness entry export found (_start/run/0/1/main)",
            )
        })?;

    // Drive the instrumented branches. Propagate the engine error verbatim
    // (fuel exhaustion, traps) rather than masking it.
    engine.execute(instance, entry, &[])?;

    let inst = engine
        .get_instance(instance)
        .map_err(|_| Error::runtime_error("failed to access instance after run"))?;
    let snapshot = inst.export_global_snapshot(WITNESS_COUNTER_PREFIX)?;

    let json = build_counters_json(&snapshot, WITNESS_COUNTER_PREFIX)?;
    std::fs::write(output_path, json)
        .map_err(|_| Error::runtime_error("WITNESS_OUTPUT could not be written"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // rivet: verifies REQ_WITNESS_COV
    #[test]
    fn build_counters_json_strips_prefix_to_bare_id_in_v1_envelope() {
        let mut snapshot = BTreeMap::new();
        snapshot.insert("__witness_counter_0".to_string(), 7i64);
        snapshot.insert("__witness_counter_1".to_string(), 0i64);

        let json = build_counters_json(&snapshot, WITNESS_COUNTER_PREFIX).unwrap();

        assert_eq!(
            json,
            r#"{"schema":"witness-harness-v1","counters":{"0":7,"1":0}}"#
        );
    }

    // rivet: verifies REQ_WITNESS_COV
    #[test]
    fn build_counters_json_orders_ids_numerically_not_lexically() {
        let mut snapshot = BTreeMap::new();
        snapshot.insert("__witness_counter_2".to_string(), 1i64);
        snapshot.insert("__witness_counter_10".to_string(), 2i64);

        let json = build_counters_json(&snapshot, WITNESS_COUNTER_PREFIX).unwrap();

        // "2" must come before "10" (numeric), which a string sort would reverse.
        assert_eq!(
            json,
            r#"{"schema":"witness-harness-v1","counters":{"2":1,"10":2}}"#
        );
    }

    // rivet: verifies REQ_WITNESS_COV
    #[test]
    fn build_counters_json_rejects_non_numeric_id() {
        let mut snapshot = BTreeMap::new();
        snapshot.insert("__witness_counter_oops".to_string(), 1i64);

        assert!(build_counters_json(&snapshot, WITNESS_COUNTER_PREFIX).is_err());
    }

    /// End-to-end: `run` loads a core, invokes `_start` (which hits a counter
    /// twice), and writes the v1 snapshot reflecting the post-run value.
    // rivet: verifies REQ_WITNESS_COV
    #[cfg(all(feature = "std", feature = "kiln-execution"))]
    #[test]
    fn run_executes_entry_and_writes_v1_snapshot() {
        let wasm = wat::parse_str(
            r#"(module
                (global $c (export "__witness_counter_0") (mut i64) (i64.const 0))
                (func (export "_start")
                    (global.set $c (i64.add (global.get $c) (i64.const 1)))
                    (global.set $c (i64.add (global.get $c) (i64.const 1)))))"#,
        )
        .expect("wat fixture parses");

        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let module_path = dir.join(format!("kilnd_witness_mod_{pid}.wasm"));
        let output_path = dir.join(format!("kilnd_witness_out_{pid}.json"));
        std::fs::write(&module_path, &wasm).unwrap();

        run(&module_path, &output_path).unwrap();

        let got = std::fs::read_to_string(&output_path).unwrap();
        assert_eq!(got, r#"{"schema":"witness-harness-v1","counters":{"0":2}}"#);

        let _ = std::fs::remove_file(&module_path);
        let _ = std::fs::remove_file(&output_path);
    }
}
