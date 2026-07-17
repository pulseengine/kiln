//! Panic-contract regression tests for the component library API (SR-52, #427).
//!
//! The library API (`ComponentInstance::from_parsed_library` and friends) must
//! return `Err` for a component it cannot handle — never abort the host process.
//! #427 reported `resolve_exports` escalating a `BoundedVec` failure into a
//! process abort via `.expect("Failed to create component type")` while building
//! a placeholder `ComponentType::unit(...)` signature for every `Sort::Function`
//! export.
//!
//! These tests drive the exact entry point #427 names against a component that
//! exports functions — the shape that triggered the abort — and assert the call
//! returns a `Result` either way. A panic fails the test.

use kiln_component::components::component_instantiation::ComponentInstance;

/// A wasip2-style component that exports a function. This is the shape that
/// drove `resolve_exports` down the placeholder-signature path in #427.
fn function_exporting_component() -> Vec<u8> {
    wat::parse_str(
        r#"
        (component
          (core module $m
            (func (export "run") (result i32)
              i32.const 53))
          (core instance $i (instantiate $m))
          (func $run (result u32)
            (canon lift (core func $i "run")))
          (export "run" (func $run)))
        "#,
    )
    .expect("fixture must build as a component")
}

/// SR-52 / #427: the library API must not panic on a function-exporting
/// component. Returning `Err` is acceptable; aborting the process is not.
#[test]
fn from_parsed_library_does_not_panic_on_function_exports() {
    let bytes = function_exporting_component();
    let mut parsed = Box::new(
        kiln_decoder::component::decode_component(&bytes).expect("fixture must decode"),
    );

    // The assertion is that this call RETURNS (Ok or Err) rather than
    // panicking. A panic here is the #427 defect and fails the test.
    let result = ComponentInstance::from_parsed_library(0, &mut parsed, None, None);

    match result {
        Ok(_) => {},
        Err(e) => {
            // An Err is a valid outcome — but it must be a real error, not the
            // escalated placeholder failure #427 described.
            assert!(
                !e.to_string().contains("Failed to create component type"),
                "library API must not surface the #427 placeholder-signature \
                 failure; got: {e}"
            );
        },
    }
}
