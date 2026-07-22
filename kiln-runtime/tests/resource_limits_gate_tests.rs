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

// ---------------------------------------------------------------------------
// SR-45 (issues #415 / #421): the module's own signed `kiln.resource_limits`
// manifest must be APPLIED on-target at load — not extracted-then-discarded.
// An embedded/gale deployment with NO `--memory` CLI flag must still be
// bounded by the manifest, and a present-but-malformed manifest must fail
// the load (never silently proceed unbounded).
// ---------------------------------------------------------------------------

use kiln_decoder::resource_limits_section::{RESOURCE_LIMITS_SECTION_NAME, ResourceLimitsSection};
use kiln_foundation::budget_aware_provider::CrateId;
use kiln_runtime::engine::EngineBuilder;

/// Minimal unsigned LEB128 encoder for section sizes in test fixtures.
fn leb128(mut value: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// Append a custom section (id 0) with the given name and payload to a
/// WebAssembly binary — how a `kiln.resource_limits` manifest is embedded.
fn append_custom_section(mut wasm: Vec<u8>, name: &str, payload: &[u8]) -> Vec<u8> {
    let mut content = leb128(name.len() as u32);
    content.extend_from_slice(name.as_bytes());
    content.extend_from_slice(payload);
    wasm.push(0); // custom section id
    wasm.extend(leb128(content.len() as u32));
    wasm.extend(content);
    wasm
}

/// Encode a well-formed `kiln.resource_limits` payload declaring only a
/// memory bound (what sigil signs off-target for AD-WCMC-001).
fn manifest_payload(max_memory_usage: u64) -> Vec<u8> {
    let provider = kiln_foundation::safe_managed_alloc!(4096, CrateId::Decoder)
        .expect("test provider allocation");
    let section = ResourceLimitsSection::with_execution_limits(
        provider,
        None,
        Some(max_memory_usage),
        None,
        None,
        None,
    )
    .expect("manifest section construction");
    section.encode().expect("manifest section encoding")
}

/// SR-45 core: a module carrying a manifest memory bound, loaded WITHOUT any
/// CLI `--memory` limits, must have that bound enforced by the same
/// pre-allocation gate the CLI path uses. Declared min (2 pages) exceeds the
/// manifest bound (1 page) → reject at load, before allocation.
#[test]
#[serial_test::serial]
fn manifest_memory_bound_enforced_without_cli_limits() {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine construction");
    let binary = append_custom_section(
        wasm("(module (memory 2))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &manifest_payload(WASM_PAGE), // manifest allows 1 page
    );
    let err = engine
        .load_module(&binary)
        .expect_err("declared min of 2 pages must not load under a 1-page manifest bound");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );
}

/// SR-45 no-over-reject: a module whose declared min fits under its manifest
/// bound loads, instantiates, and carries the manifest-derived runtime grow
/// cap (2 pages) on its memory.
#[test]
#[serial_test::serial]
fn manifest_bound_admits_fitting_module_and_caps_growth() {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine construction");
    let binary = append_custom_section(
        wasm("(module (memory 1))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &manifest_payload(2 * WASM_PAGE), // manifest allows 2 pages
    );
    let handle = engine
        .load_module(&binary)
        .expect("declared min of 1 page must load under a 2-page manifest bound");
    let instance = engine.instantiate(handle).expect("must instantiate");
    let inst = engine.get_instance(instance).expect("instance must exist");
    let mem = inst.memory(0).expect("memory must exist");
    assert_eq!(
        mem.0.runtime_max_pages.load(Ordering::Relaxed),
        2,
        "memory must carry the manifest-derived runtime page cap"
    );
}

/// SR-45 precedence: manifest bound tighter than the CLI bound → the manifest
/// wins (an operator's `--memory` must not LOOSEN a module's signed
/// self-declared bound). CLI allows 10 pages, manifest allows 1, module
/// declares 2 → reject.
#[test]
#[serial_test::serial]
fn manifest_tighter_than_cli_wins() {
    let mut engine = engine_with_cap(10 * WASM_PAGE);
    let binary = append_custom_section(
        wasm("(module (memory 2))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &manifest_payload(WASM_PAGE), // manifest allows 1 page
    );
    let err = engine
        .load_module(&binary)
        .expect_err("the tighter manifest bound must win over a looser CLI bound");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );
}

/// SR-45 precedence (converse): CLI bound tighter than the manifest bound →
/// the CLI wins (a module's manifest must not loosen an operator cap). CLI
/// allows 1 page, manifest allows 10, module declares 2 → reject.
#[test]
#[serial_test::serial]
fn cli_tighter_than_manifest_wins() {
    let mut engine = engine_with_cap(WASM_PAGE);
    let binary = append_custom_section(
        wasm("(module (memory 2))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &manifest_payload(10 * WASM_PAGE), // manifest allows 10 pages
    );
    let err = engine
        .load_module(&binary)
        .expect_err("the tighter CLI bound must win over a looser manifest bound");
    assert!(
        err.message.contains("rejected before allocation"),
        "reject must come from the pre-allocation gate, got: {}",
        err.message
    );
}

/// SR-45 fail-loud: a PRESENT but MALFORMED `kiln.resource_limits` section is
/// a load error — never a silent unbounded pass-through. The payload below is
/// truncated mid-field (version + "present" flag for max_fuel, no value).
#[test]
#[serial_test::serial]
fn malformed_manifest_section_fails_loud() {
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine construction");
    let truncated_payload = [1u8, 0, 0, 0, 1]; // version=1, max_fuel "present" but value missing
    let binary = append_custom_section(
        wasm("(module (memory 1))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &truncated_payload,
    );
    engine
        .load_module(&binary)
        .expect_err("a present-but-malformed manifest must fail the load, not proceed unbounded");
}

/// SR-45 builder consistency: `EngineBuilder::from_binary` must not swallow a
/// malformed manifest either — the second silent-fallback path from #421.
#[test]
#[serial_test::serial]
fn builder_from_binary_fails_loud_on_malformed_manifest() {
    let truncated_payload = [1u8, 0, 0, 0, 1];
    let binary = append_custom_section(
        wasm("(module (memory 1))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &truncated_payload,
    );
    assert!(
        EngineBuilder::from_binary(&binary).is_err(),
        "EngineBuilder::from_binary must fail loud on a malformed manifest"
    );
}

/// SR-45 builder: the qualified ASIL level in a well-formed manifest selects
/// the builder's ASIL mode (today's stub unconditionally reports ASIL-D for
/// every binary).
#[test]
#[serial_test::serial]
fn builder_from_binary_selects_manifest_asil_level() {
    let provider = kiln_foundation::safe_managed_alloc!(4096, CrateId::Decoder)
        .expect("test provider allocation");
    let section = ResourceLimitsSection::with_execution_limits(
        provider,
        None,
        Some(WASM_PAGE),
        None,
        None,
        None,
    )
    .expect("manifest section construction")
    .with_qualification([0u8; 32], "ASIL-B")
    .expect("manifest qualification");
    let payload = section.encode().expect("manifest section encoding");
    let binary = append_custom_section(
        wasm("(module (memory 1))"),
        RESOURCE_LIMITS_SECTION_NAME,
        &payload,
    );
    let builder =
        EngineBuilder::from_binary(&binary).expect("well-formed manifest must be accepted");
    let debug = format!("{:?}", builder);
    assert!(
        debug.contains("AsilB"),
        "builder must select the manifest's qualified ASIL level, got: {}",
        debug
    );
}
