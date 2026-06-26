//! Direct Component-Model hosting in kilnd.
//!
//! This is the DIRECT-HOSTING / std-QM path of decision `AD-COMPONENT-HOST-001`
//! (gale#63 / `REQ_COMPONENT_HOST`, issue #344): instantiate an external
//! (e.g. wac-composed) component and invoke a named export, lifting its scalar
//! result back to a [`ComponentValue`].
//!
//! It deliberately does NOT touch the meld-fuse path or anything async
//! (`SM-ASYNC-002`).

#[cfg(all(feature = "std", feature = "component-model"))]
use kiln_component::canonical_abi::ComponentValue;
use kiln_error::Result;

/// Instantiate `wasm` (a Component Model binary) and invoke the named `export`,
/// returning its lifted scalar result(s).
///
/// # Errors
///
/// Returns an error if the component fails to decode, instantiate, or if the
/// named export cannot be invoked.
#[cfg(all(feature = "std", feature = "component-model"))]
pub fn invoke_component_export(wasm: &[u8], export: &str) -> Result<Vec<ComponentValue>> {
    use kiln_component::components::component_instantiation::ComponentInstance;
    use kiln_decoder::component::decode_component;

    let mut parsed = decode_component(wasm)?;
    let mut instance = ComponentInstance::from_parsed_with_handler(0, &mut parsed, None, None)?;
    instance.call_function(export, &[], None)
}

#[cfg(all(test, feature = "std", feature = "component-model"))]
mod tests {
    use super::*;

    /// The acceptance fixture from #344: a component whose single export
    /// `run-demo` lifts a core function returning `(i32.const 53)` to a
    /// component-level `u32`.
    fn fixture() -> Vec<u8> {
        wat::parse_str(
            r#"
            (component
              (core module $m (func (export "run-demo") (result i32) (i32.const 53)))
              (core instance $i (instantiate $m))
              (func $rd (result u32) (canon lift (core func $i "run-demo")))
              (export "run-demo" (func $rd)))
            "#,
        )
        .expect("fixture must build + validate as a component")
    }

    #[test]
    fn invoke_named_export_lifts_scalar() {
        let wasm = fixture();
        let result = invoke_component_export(&wasm, "run-demo")
            .expect("instantiate + invoke run-demo must succeed");
        assert_eq!(result, vec![ComponentValue::U32(53)]);
    }

    /// Helper used to materialize the acceptance fixture to disk for the
    /// end-to-end `--invoke` CLI check. Run with `--ignored`.
    #[test]
    #[ignore]
    fn write_fixture_to_tmp() {
        std::fs::write("/tmp/fixture.wasm", fixture()).expect("write fixture");
    }
}
