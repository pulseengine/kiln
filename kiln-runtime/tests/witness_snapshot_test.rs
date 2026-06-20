//! Post-run exported-global snapshot — the witness MC/DC coverage backend (issue #340).
//!
//! witness instruments core modules with exported mutable globals
//! (`__witness_counter_<id>`) and reads coverage back by snapshotting those
//! globals after a run. These tests cover the host-reachable snapshot accessor
//! on a live `ModuleInstance`: prefix filtering, integer value reads, and that
//! the snapshot reflects mutations performed during the run.

use kiln_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};

/// The snapshot returns only exported globals whose name matches the prefix,
/// with their (post-instantiation) integer values, excluding non-matching ones.
#[test]
fn snapshot_filters_by_prefix_and_reads_integer_values() {
    let wasm = wat::parse_str(
        r#"(module
            (global (export "__witness_counter_0") i32 (i32.const 7))
            (global (export "__witness_counter_1") (mut i32) (i32.const 0))
            (global (export "other") i32 (i32.const 99)))"#,
    )
    .expect("wat fixture parses");

    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM).unwrap();
    let module_handle = engine.load_module(&wasm).unwrap();
    let instance_handle = engine.instantiate(module_handle).unwrap();
    let instance = engine.get_instance(instance_handle).unwrap();

    let snapshot = instance.export_global_snapshot("__witness_counter_").unwrap();

    assert_eq!(snapshot.len(), 2, "only the two prefixed globals, not `other`");
    assert_eq!(snapshot.get("__witness_counter_0"), Some(&7i64));
    assert_eq!(snapshot.get("__witness_counter_1"), Some(&0i64));
    assert_eq!(snapshot.get("other"), None);
}

/// The decisive coverage case: a counter incremented during the run is
/// reflected in the post-run snapshot (i.e. `execute` mutates the same global
/// state the snapshot reads).
#[test]
fn snapshot_reflects_counters_mutated_during_the_run() {
    let wasm = wat::parse_str(
        r#"(module
            (global $c (export "__witness_counter_0") (mut i32) (i32.const 0))
            (func (export "hit")
                (global.set $c (i32.add (global.get $c) (i32.const 1)))))"#,
    )
    .expect("wat fixture parses");

    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM).unwrap();
    let module_handle = engine.load_module(&wasm).unwrap();
    let instance_handle = engine.instantiate(module_handle).unwrap();

    // Run the instrumented branch three times.
    for _ in 0..3 {
        engine.execute(instance_handle, "hit", &[]).unwrap();
    }

    let instance = engine.get_instance(instance_handle).unwrap();
    let snapshot = instance.export_global_snapshot("__witness_counter_").unwrap();

    assert_eq!(snapshot.get("__witness_counter_0"), Some(&3i64));
}
