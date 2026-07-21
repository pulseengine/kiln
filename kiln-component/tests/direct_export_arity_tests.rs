//! Direct-hosting argument-arity enforcement tests (SR-53, #443).
//!
//! Two manifestations of the same missing check:
//!
//! 1. Invocation: `call_direct_export` validated only RESULT arity, and the
//!    export's registered `FunctionSignature.params` was always empty — so
//!    `validate_function_args` passed trivially and a param-taking lifted
//!    export invoked with no args ran with ZERO-FILLED core params, its wrong
//!    result reported as success.
//! 2. Load: a spec-invalid component whose `canon lift` declares a different
//!    param arity than the backing core function was ACCEPTED and run, though
//!    `wasm-tools validate` and wasmtime reject it.

#![cfg(all(feature = "std", feature = "kiln-execution"))]

use kiln_component::canonical_abi::ComponentValue;
use kiln_component::components::component_instantiation::ComponentInstance;

/// A valid component: `add: func(a: u32, b: u32) -> u32` lifted from a core
/// `add (param i32 i32) (result i32)`.
fn valid_add_component() -> Vec<u8> {
    wat::parse_str(
        r#"
        (component
          (core module $m
            (func (export "add") (param i32 i32) (result i32)
              (i32.add (local.get 0) (local.get 1))))
          (core instance $i (instantiate $m))
          (func $add (param "a" u32) (param "b" u32) (result u32)
            (canon lift (core func $i "add")))
          (export "add" (func $add)))
        "#,
    )
    .expect("valid fixture must assemble as a component")
}

/// The #443 second manifestation: the lift declares ONE param but the backing
/// core function takes TWO. `wasm-tools validate` rejects this ("lowered
/// parameter types [I32] do not match parameter types [I32, I32] of core
/// function 0"); kilnd accepted and ran it.
fn param_arity_mismatch_component() -> Vec<u8> {
    wat::parse_str(
        r#"
        (component
          (core module $m
            (func (export "add") (param i32 i32) (result i32)
              (i32.add (local.get 0) (local.get 1))))
          (core instance $i (instantiate $m))
          (func $add (param "a" u32) (result u32)
            (canon lift (core func $i "add")))
          (export "add" (func $add)))
        "#,
    )
    .expect("the invalid fixture is still syntactically well-formed WAT and must assemble")
}

fn instantiate(bytes: &[u8]) -> kiln_error::Result<ComponentInstance> {
    let mut parsed =
        Box::new(kiln_decoder::component::decode_component(bytes).expect("fixture must decode"));
    ComponentInstance::from_parsed_with_handler(0, &mut parsed, None, None)
}

/// SR-53 / #443: invoking a 2-param lifted export with NO arguments must be
/// an ERROR via the existing `validate_function_args` arity check — not a
/// zero-filled core invocation reported as success. Before the fix the
/// export's registered signature had empty params, so `&[]` passed
/// validation and the core `add` ran as add(0, 0).
#[test]
#[serial_test::serial]
fn direct_export_invoked_with_missing_args_errors() {
    let mut instance =
        instantiate(&valid_add_component()).expect("valid component must instantiate");
    assert!(
        instance.call_function("add", &[], None).is_err(),
        "invoking 'add' (2 params) with 0 arguments must fail loud, not run \
         with zero-filled core parameters"
    );
}

/// Matching arity must still work AND compute the correct answer:
/// add(5, 3) = 8. Before the fix this direction was ALSO broken — the empty
/// registered param signature made `validate_function_args` reject the
/// correctly-supplied arguments (2 args vs 0 declared params).
#[test]
#[serial_test::serial]
fn direct_export_with_matching_args_computes_correct_answer() {
    let mut instance =
        instantiate(&valid_add_component()).expect("valid component must instantiate");
    let results = instance
        .call_function(
            "add",
            &[ComponentValue::U32(5), ComponentValue::U32(3)],
            None,
        )
        .expect("matching-arity invocation must succeed");
    assert_eq!(results, vec![ComponentValue::U32(8)], "add(5, 3) must be 8");
}

/// SR-53 / #443 (load-time): a `canon lift` whose declared param arity
/// differs from the backing core function must be REJECTED at
/// decode/instantiation — matching wasm-tools and wasmtime — not accepted
/// and run. Before the fix `from_parsed_with_handler` succeeded and
/// `--invoke add` reported `✓ Execution completed successfully`.
#[test]
#[serial_test::serial]
fn canon_lift_param_arity_mismatch_rejected_at_load() {
    assert!(
        instantiate(&param_arity_mismatch_component()).is_err(),
        "a canon lift declaring 1 param over a 2-param core function must be \
         rejected at load, as wasm-tools validate and wasmtime do"
    );
}
