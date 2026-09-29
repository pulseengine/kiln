//! kilnd refuses calls the signature manifest says it cannot make (SR-60/SR-62).
//!
//! The core type alone cannot distinguish a genuine scalar parameter from a
//! pointer to a lowered argument block — both present as `(i32) -> i32`. v0.5.0
//! therefore accepted `--arg 99999` for a pointer, wrote an arbitrary address
//! into guest memory, and reported the run as success. meld's
//! `meld.signature-manifest` section is the only thing in a fused artifact that
//! knows the difference, so the refusal is driven by it.
//!
//! These tests synthesise the manifest section directly rather than shelling
//! out to meld, so they run in CI without the toolchain. The shape is meld's
//! version 1, verified by hand against a real fused falcon cascade
//! (`meld 0.58.3 --emit-manifest`), where `rate#tick` reports
//! `flat_param_count: 18` beside a `core` of one `i32`.

use std::{fs, process::Command};

const KILND: &str = env!("CARGO_BIN_EXE_kilnd");

/// Append a custom section carrying `json` under meld's section name.
fn with_manifest(module_wat: &str, json: &str) -> Vec<u8> {
    let mut wasm = wat::parse_str(module_wat).expect("fixture must assemble");
    let name = "meld.signature-manifest";

    let mut payload = Vec::new();
    payload.push(name.len() as u8);
    payload.extend_from_slice(name.as_bytes());
    payload.extend_from_slice(json.as_bytes());

    wasm.push(0); // custom section id
    let mut n = payload.len();
    loop {
        let mut b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            b |= 0x80;
        }
        wasm.push(b);
        if n == 0 {
            break;
        }
    }
    wasm.extend_from_slice(&payload);
    wasm
}

/// A one-i32-parameter export, the shape both a scalar and a lowered-args
/// pointer collapse to.
const MODULE: &str = r#"(module
    (memory (export "memory") 1)
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 0)
    (func (export "take_one") (param i32) (result i32) local.get 0))"#;

fn manifest(flat: u32, realloc: &str) -> String {
    format!(
        r#"{{"version":1,"exports":[{{
            "export":"take_one",
            "wit":{{"params":[],"result":"u32"}},
            "core":{{"params":["i32"],"results":["i32"]}},
            "flat_param_count":{flat},
            "needs":{{"memory":true,"realloc":true}},
            "memory":"memory","realloc":{realloc},"post_return":null,
            "return_area":{{"size":16,"align":4,"layout":[]}}
        }}]}}"#
    )
}

fn run(bytes: &[u8], name: &str, args: &[&str]) -> (Option<i32>, String) {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, bytes).unwrap();
    let mut cmd = Command::new(KILND);
    cmd.args(["--invoke", "take_one"]);
    for a in args {
        cmd.args(["--arg", a]);
    }
    let out = cmd.arg(&path).output().unwrap();
    let _ = fs::remove_file(&path);
    (
        out.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// SR-62: 18 flattened values behind one core i32 means the parameter is a
/// pointer. Accepting a number for it is the bug v0.5.0 shipped.
#[test]
fn refuses_an_argument_for_an_indirectly_lowered_parameter() {
    let wasm = with_manifest(MODULE, &manifest(18, "\"cabi_realloc\""));
    let (code, out) = run(&wasm, "kilnd_sr62_indirect.wasm", &["99999"]);

    assert_ne!(code, Some(0), "a pointer parameter must not accept a number\n{out}");
    assert!(
        out.contains("POINTER") && out.contains("18"),
        "the refusal must say the parameter is a pointer and how many values \
         were flattened\n{out}"
    );
}

/// The flat case must remain callable — the refusal keys on disagreement
/// between `flat_param_count` and the core arity, not on "has a manifest".
#[test]
fn a_genuinely_flat_parameter_still_works_with_a_manifest_present() {
    let wasm = with_manifest(MODULE, &manifest(1, "\"cabi_realloc\""));
    let (code, out) = run(&wasm, "kilnd_sr62_flat.wasm", &["7"]);

    assert_eq!(code, Some(0), "a genuine scalar must still be callable\n{out}");
    assert!(out.contains('7'), "the call must return its argument\n{out}");
}

/// `needs.realloc: true` with `realloc: null` is meld stating a contradiction
/// it found in the artifact. A reader must refuse, not substitute an allocator
/// that merely looks like one.
#[test]
fn refuses_when_the_required_allocator_is_unreachable() {
    let wasm = with_manifest(MODULE, &manifest(1, "null"));
    let (code, out) = run(&wasm, "kilnd_sr62_norealloc.wasm", &["7"]);

    assert_ne!(code, Some(0), "an unreachable allocator must refuse\n{out}");
    assert!(
        out.contains("realloc") && out.contains("not callable"),
        "the refusal must name the missing allocator condition\n{out}"
    );
}

/// The cross-check: `core` is read back by meld from the emitted function type,
/// so disagreement with the module means one side is wrong and calling would be
/// guessing which. This is the check that caught meld's own flattening bug.
#[test]
fn refuses_when_the_manifest_disagrees_with_the_module() {
    // Manifest claims two core params; the module declares one.
    let json = manifest(2, "\"cabi_realloc\"")
        .replace(r#""params":["i32"],"results":["i32"]"#, r#""params":["i32","i32"],"results":["i32"]"#);
    let wasm = with_manifest(MODULE, &json);
    let (code, out) = run(&wasm, "kilnd_sr62_disagree.wasm", &["1", "2"]);

    assert_ne!(code, Some(0), "a manifest/module disagreement must refuse\n{out}");
    assert!(
        out.contains("disagrees"),
        "the refusal must say the manifest disagrees with the module\n{out}"
    );
}

/// A module with no manifest is not an error — most modules kilnd runs have
/// none, and absence must not become a refusal.
#[test]
fn a_module_without_a_manifest_is_unaffected() {
    let wasm = wat::parse_str(MODULE).unwrap();
    let (code, out) = run(&wasm, "kilnd_sr62_nomanifest.wasm", &["7"]);

    assert_eq!(code, Some(0), "absent manifest must not refuse\n{out}");
}

/// A present-but-broken manifest must fail loud rather than be read as absent.
#[test]
fn a_malformed_manifest_fails_loud_rather_than_being_ignored() {
    let wasm = with_manifest(MODULE, "{not valid json");
    let (code, out) = run(&wasm, "kilnd_sr62_malformed.wasm", &["7"]);

    assert_ne!(
        code,
        Some(0),
        "a broken manifest must not be silently treated as absent\n{out}"
    );
}
