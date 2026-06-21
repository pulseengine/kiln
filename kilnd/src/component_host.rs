//! Direct Component-Model hosting (#344 — REQ_COMPONENT_HOST / AD-COMPONENT-HOST-001).
//!
//! The std/QM reference host (kilnd) can load an external (e.g. `wac`-composed)
//! Component-Model component, instantiate it, and invoke a named export, lifting
//! the scalar result through the canonical ABI. This is the direct-hosting half
//! of AD-COMPONENT-HOST-001 (the embedded-cert path stays meld-fuse-to-core).

#[cfg(all(feature = "std", feature = "component-model"))]
use kiln_component::canonical_abi::ComponentValue;
use kiln_error::Result;

/// Decode an external component, instantiate it, and invoke `export` with no
/// arguments, returning the lifted result values.
#[cfg(all(feature = "std", feature = "component-model"))]
pub fn invoke_component_export(wasm: &[u8], export: &str) -> Result<Vec<ComponentValue>> {
    use kiln_component::components::component_instantiation::ComponentInstance;
    use kiln_decoder::component::decode_component;

    let mut parsed = decode_component(wasm)?;
    let mut instance = ComponentInstance::from_parsed_with_handler(0, &mut parsed, None, None)?;
    instance.call_function(export, &[], None)
}

#[cfg(test)]
#[cfg(all(feature = "std", feature = "component-model"))]
mod tests {
    use super::*;

    /// Acceptance (#344 kill-criterion): instantiate a component whose `run-demo`
    /// export is a `canon lift` of a core function returning 53, invoke it, and
    /// get the scalar result lifted to a component `u32`.
    #[test]
    fn hosts_typed_component_export_and_lifts_scalar_result() {
        let wasm = wat::parse_str(
            r#"(component
                (core module $m (func (export "run-demo") (result i32) (i32.const 53)))
                (core instance $i (instantiate $m))
                (func $rd (result u32) (canon lift (core func $i "run-demo")))
                (export "run-demo" (func $rd)))"#,
        )
        .expect("component wat parses");

        let result = invoke_component_export(&wasm, "run-demo").expect("invoke run-demo");

        assert_eq!(result, vec![ComponentValue::U32(53)]);
    }
}
