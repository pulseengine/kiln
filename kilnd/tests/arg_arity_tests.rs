//! Argument-arity enforcement tests for kilnd function invocation (SR-53, #443).
//!
//! Bug repro (#412 fabricated-reporting family, function-invocation surface):
//! kilnd invoked a param-taking export with ZERO-FILLED arguments when params
//! could not be supplied and reported the resulting wrong value as SUCCESS
//! (`kilnd mod.wasm --function addone` printed `✓ returned I32(1)` — the
//! zero-filled `addone(0)` — where wasmtime computes `addone(5) = 6`).
//!
//! The fix is fail-loud: a param-taking function invoked without matching
//! arguments must return `Err`, never a zero-filled result reported as
//! success. Zero-param entry points must STILL run — the check is an
//! arity match, not "has params → always error".

#![cfg(all(feature = "std", feature = "kiln-execution"))]

use kilnd::{KilndConfig, KilndEngine};

/// Build a KilndEngine for an inline WAT module invoking `function_name`.
fn engine_for(wat_src: &str, function_name: Option<&str>) -> KilndEngine {
    let wasm = wat::parse_str(wat_src).expect("test WAT must assemble");
    let mut config = KilndConfig::default();
    // KilndConfig::module_data is &'static [u8]; leak the test fixture.
    config.module_data = Some(Box::leak(wasm.into_boxed_slice()));
    config.function_name = function_name.map(str::to_owned);
    KilndEngine::new(config).expect("KilndEngine construction must succeed")
}

/// The exact module shape from issue #443.
const PARAM_MODULE: &str = r#"(module
  (func (export "dbl") (param i32) (result i32)
    (i32.mul (local.get 0) (i32.const 2)))
  (func (export "addone") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))
  (func (export "_start")))"#;

/// SR-53 / #443: invoking a `(param i32) (result i32)` export with no
/// arguments must be an ERROR, not a zero-filled `Ok` reported as success.
/// Before the fix this returned Ok and printed `✓ returned I32(1)`
/// (= addone(0)) with exit 0.
#[test]
#[serial_test::serial]
fn param_taking_export_invoked_with_no_args_errors() {
    let mut engine = engine_for(PARAM_MODULE, Some("addone"));
    assert!(
        engine.execute_module().is_err(),
        "invoking 'addone' (1 param) with no supplied arguments must fail loud, \
         not run with a zero-filled parameter and report success"
    );
}

/// A zero-param entry point must STILL run — the check is arity match, not
/// "has params → always error".
#[test]
#[serial_test::serial]
fn zero_param_start_still_runs() {
    let mut engine = engine_for(PARAM_MODULE, None);
    engine
        .execute_module()
        .expect("zero-param _start must still execute successfully");
}

/// A zero-param export WITH a result must still run and succeed.
#[test]
#[serial_test::serial]
fn zero_param_result_export_still_runs() {
    let mut engine = engine_for(
        r#"(module (func (export "answer") (result i32) (i32.const 42))
                   (func (export "_start")))"#,
        Some("answer"),
    );
    engine
        .execute_module()
        .expect("zero-param result-returning export must still execute successfully");
}

/// Engine-level contract: the interpreter itself must reject an
/// argument-count mismatch instead of zero-filling missing params, and must
/// still compute the CORRECT answer when the right arguments are supplied.
/// Before the fix `execute(.., "dbl", &[])` returned Ok([I32(0)]) — the
/// zero-fill masking fallback in the stackless engine's locals init.
#[test]
#[serial_test::serial]
fn engine_rejects_missing_args_and_computes_correct_answer_with_args() {
    use kiln_foundation::values::Value;
    use kiln_runtime::engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset};

    kiln_foundation::memory_init::MemoryInitializer::initialize()
        .expect("memory system must initialize");

    let wasm = wat::parse_str(PARAM_MODULE).expect("test WAT must assemble");
    let mut engine =
        CapabilityAwareEngine::with_preset(EnginePreset::QM).expect("engine must construct");
    let module = engine.load_module(&wasm).expect("module must load");
    let instance = engine.instantiate(module).expect("module must instantiate");

    // Missing argument: must fail loud, not zero-fill.
    assert!(
        engine.execute(instance, "dbl", &[]).is_err(),
        "engine must reject a 1-param function invoked with 0 arguments"
    );

    // Extra argument: must also fail loud, not silently truncate.
    assert!(
        engine.execute(instance, "dbl", &[Value::I32(5), Value::I32(7)]).is_err(),
        "engine must reject a 1-param function invoked with 2 arguments"
    );

    // Correct arity: must compute the CORRECT answer (dbl(5) = 10).
    let results = engine
        .execute(instance, "dbl", &[Value::I32(5)])
        .expect("matching-arity invocation must succeed");
    assert_eq!(
        results,
        vec![Value::I32(10)],
        "dbl(5) must be 10 — the wasmtime-verified answer"
    );
}
