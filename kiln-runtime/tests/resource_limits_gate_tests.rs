//! Engine-level tests for the pre-allocation resource-limit gate
//! (SR-46 / SR-47 / SR-48; issues #436 / #439 / #440).
//!
//! The gate lives in `CapabilityAwareEngine::load_module` and validates the
//! TOTAL declared minimums of ALL memories and ALL tables against the host
//! byte budget (`EngineResourceLimits`) BEFORE `Module::from_kiln_module`
//! runs — i.e. before any eager `Memory::new` / `Table::new` allocation.
//! The gate's error message carries the "rejected before allocation" marker,
//! which no post-allocation path emits; asserting on it pins the reject to
//! the pre-allocation stage.
//!
//! These tests construct engines that draw on the global capability-based
//! memory budget, so they run serially (see CLAUDE.md).

use core::sync::atomic::Ordering;

use kiln_foundation::values::Value;
use kiln_runtime::engine::{
    CapabilityAwareEngine,
    CapabilityEngine,
    EnginePreset,
    EngineResourceLimits,
};

const WASM_PAGE: u64 = 64 * 1024;

fn engine_with_cap(max_bytes: u64) -> CapabilityAwareEngine {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine construction");
    engine.set_resource_limits(EngineResourceLimits::from_max_memory_bytes(max_bytes));
    engine
}

fn wasm(src: &str) -> Vec<u8> {
    wat::parse_str(src).expect("test WAT must assemble")
}

/// SR-46: a declared memory min over the cap is rejected BEFORE the eager
/// allocation. 65535 pages is ~4 GiB — under the old post-allocation reject
/// this test would zero-fill ~4 GiB before failing; the gate's distinct
/// "rejected before allocation" message proves the pre-allocation path fired.
#[test]
#[serial_test::serial]
fn oversized_memory_min_rejected_before_allocation() {
    let mut engine = engine_with_cap(WASM_PAGE); // 1 page budget
    let err = engine
        .load_module(&wasm("(module (memory 65535))"))
        .expect_err("declared memory min of 65535 pages must not load under a 1-page cap");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );
}

/// SR-47: a declared table min whose host cost exceeds the cap is rejected
/// before the table's element vector is allocated (the #439 repro used
/// `(table 100000000 funcref)` → 3.02 GB RSS, exit 0).
#[test]
#[serial_test::serial]
fn oversized_table_min_rejected_before_allocation() {
    let mut engine = engine_with_cap(WASM_PAGE); // 1 page budget
    let err = engine
        .load_module(&wasm("(module (table 100000000 funcref))"))
        .expect_err("declared table min of 100M elements must not load under a 64 KiB cap");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );
}

/// SR-48: the gate bounds the TOTAL declared min across ALL memories — two
/// 1-page memories exceed a 1-page budget even though each one individually
/// fits (the #440 repro hid the oversized min at index 1).
#[test]
#[serial_test::serial]
fn multi_memory_total_declared_min_gated() {
    let mut engine = engine_with_cap(WASM_PAGE); // 1 page budget
    let err = engine
        .load_module(&wasm("(module (memory 1) (memory 1))"))
        .expect_err("total declared min of 2 pages must not load under a 1-page cap");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );

    // Control: the same total under a 2-page budget loads and instantiates.
    let mut engine = engine_with_cap(2 * WASM_PAGE);
    let handle = engine
        .load_module(&wasm("(module (memory 1) (memory 1))"))
        .expect("total declared min at exactly the cap must load");
    engine
        .instantiate(handle)
        .expect("within-cap module must instantiate");
}

/// SR-47 + SR-48: every instantiated memory AND table carries the runtime
/// grow cap, so `memory.grow` / `table.grow` past the budget fail — including
/// on memory index 1 (before the fix only `memory(0)` was capped and
/// `set_runtime_max_elements` was dead code).
#[test]
#[serial_test::serial]
fn runtime_caps_set_on_all_instantiated_memories_and_tables() {
    let limits = EngineResourceLimits::from_max_memory_bytes(10 * WASM_PAGE);
    let mut engine = engine_with_cap(10 * WASM_PAGE);
    let handle = engine
        .load_module(&wasm(
            "(module (memory 1 40) (memory 2 40) (table 1 funcref))",
        ))
        .expect("within-cap module must load");
    let instance = engine.instantiate(handle).expect("must instantiate");
    let inst = engine.get_instance(instance).expect("instance must exist");

    // Both memories carry the runtime page cap (10 pages), not just index 0.
    for idx in 0..2u32 {
        let mem = inst.memory(idx).expect("memory must exist");
        assert_eq!(
            mem.0.runtime_max_pages.load(Ordering::Relaxed),
            10,
            "memory {} must carry the runtime page cap",
            idx
        );
    }

    // memory index 1: grow within the cap succeeds, grow past it fails while
    // still below the declared max (40) — the runtime cap is what fires.
    let mem1 = inst.memory(1).expect("memory 1 must exist");
    mem1.0
        .grow_shared(3)
        .expect("grow to 5 pages (cap 10) must succeed");
    assert!(
        mem1.0.grow_shared(20).is_err(),
        "grow to 25 pages must fail against the 10-page runtime cap (declared max is 40)"
    );

    // The table carries the derived element cap and grow past it fails.
    let table = inst.table(0).expect("table must exist");
    let elem_cap = limits.max_table_elements();
    assert_eq!(
        table.0.runtime_max_elements.load(Ordering::Relaxed),
        elem_cap,
        "table must carry the runtime element cap derived from the byte budget"
    );
    table
        .grow(1, Value::FuncRef(None))
        .expect("small table.grow under the cap must succeed");
    assert!(
        table.grow(elem_cap, Value::FuncRef(None)).is_err(),
        "table.grow past the derived element cap must fail"
    );
}

/// Control: without host limits configured, nothing is gated or capped —
/// the engine behaves exactly as before (0 = unlimited sentinel).
#[test]
#[serial_test::serial]
fn no_limits_configured_means_no_caps() {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine construction");
    let handle = engine
        .load_module(&wasm("(module (memory 1) (table 1 funcref))"))
        .expect("module must load without limits configured");
    let instance = engine.instantiate(handle).expect("must instantiate");
    let inst = engine.get_instance(instance).expect("instance must exist");
    let mem = inst.memory(0).expect("memory must exist");
    assert_eq!(mem.0.runtime_max_pages.load(Ordering::Relaxed), 0);
    let table = inst.table(0).expect("table must exist");
    assert_eq!(table.0.runtime_max_elements.load(Ordering::Relaxed), 0);
}
