//! Resource-cap enforcement tests for the kilnd `--memory` cap (SR-46/47/48).
//!
//! These lock in the pre-allocation limits gate: declared memory AND table
//! minimums are validated against the cap BEFORE any eager allocation, the
//! TOTAL across ALL memories/tables is bounded (not just index 0), and peak
//! memory reporting sums ALL memories.
//!
//! Bug repros (issues #436, #439, #440):
//! - #439 (SR-47): the table cap was dead code — `(table N funcref)` with an
//!   oversized min ran to completion (measured 3.02 GB RSS, exit 0).
//! - #440 (SR-48): only memory index 0 was capped/rejected/reported — an
//!   oversized min on memory index 1 ran to exit 0 with a fabricated peak.
//! - #436 (SR-46): the declared-min reject ran post-instantiate, after the
//!   eager allocation had already been committed.

#![cfg(all(feature = "std", feature = "kiln-execution"))]

use kilnd::{KilndConfig, KilndEngine};

const WASM_PAGE: usize = 64 * 1024;

/// Build a KilndEngine for an inline WAT module under a `--memory` cap.
fn engine_for(wat_src: &str, max_memory: usize) -> KilndEngine {
    let wasm = wat::parse_str(wat_src).expect("test WAT must assemble");
    let mut config = KilndConfig::default();
    // KilndConfig::module_data is &'static [u8]; leak the test fixture.
    config.module_data = Some(Box::leak(wasm.into_boxed_slice()));
    config.max_memory = max_memory;
    KilndEngine::new(config).expect("KilndEngine construction must succeed")
}

/// SR-47 / #439: a declared table min whose host-side cost exceeds the
/// `--memory` cap must be rejected at load. Before the fix the table cap was
/// never wired (`set_runtime_max_elements` had zero live callers) and this
/// module executed to exit 0 while eagerly allocating the whole table.
#[test]
#[serial_test::serial]
fn table_min_over_cap_rejected_at_load() {
    let mut engine = engine_for(
        r#"(module (table 100000 funcref) (func (export "_start")))"#,
        WASM_PAGE,
    );
    assert!(
        engine.execute_module().is_err(),
        "table min of 100000 elements must be rejected under a 64 KiB --memory cap"
    );
}

/// SR-48 / #440: an oversized declared min on memory index 1 must be rejected.
/// Before the fix only `inst.memory(0)` was checked, so a multi-memory module
/// escaped the cap entirely (eager allocation, exit 0).
#[test]
#[serial_test::serial]
fn multi_memory_min_over_cap_rejected_at_load() {
    let mut engine = engine_for(
        r#"(module (memory 1) (memory 4) (func (export "_start")))"#,
        2 * WASM_PAGE,
    );
    assert!(
        engine.execute_module().is_err(),
        "total declared memory min (5 pages) must be rejected under a 2-page --memory cap \
         even when the oversized memory is index 1"
    );
}

/// SR-48 / #440: peak memory reporting must sum ALL memories, not just
/// index 0. Before the fix a `(memory 2) (memory 3)` module reported only
/// memory 0's peak.
#[test]
#[serial_test::serial]
fn multi_memory_peak_reporting_sums_all_memories() {
    let mut engine = engine_for(
        r#"(module (memory 2) (memory 3) (func (export "_start")))"#,
        10 * WASM_PAGE,
    );
    engine
        .execute_module()
        .expect("within-cap multi-memory module must execute");
    let peak = engine.stats().peak_memory;
    assert!(
        peak >= 5 * WASM_PAGE,
        "peak memory must sum all memories (expected >= {} bytes for 2+3 pages, got {})",
        5 * WASM_PAGE,
        peak
    );
}

/// SR-43 regression guard (behaviour now provided by the SR-46 pre-allocation
/// gate): a single memory whose declared min exceeds the cap is still
/// rejected — the reject moved earlier, it did not get weaker.
#[test]
#[serial_test::serial]
fn single_memory_min_over_cap_still_rejected() {
    let mut engine = engine_for(
        r#"(module (memory 4) (func (export "_start")))"#,
        2 * WASM_PAGE,
    );
    assert!(
        engine.execute_module().is_err(),
        "declared memory min (4 pages) over a 2-page --memory cap must be rejected"
    );
}
