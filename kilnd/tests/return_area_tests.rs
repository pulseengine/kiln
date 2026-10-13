//! kilnd reports the values in a Canonical-ABI return area, not the pointer
//! (SR-61, issue #505).
//!
//! A component export whose result does not fit in one core value returns an
//! i32 that is the ADDRESS of the result. v0.5.0 printed that address, which is
//! constant across inputs: two engines agreeing on it say nothing about the
//! arithmetic. With meld's signature manifest kilnd can read the return area,
//! decode each field by its WIT type, and then call the export's post-return.
//!
//! The fixtures compute their result from the arguments so a test that passed
//! on a constant would fail here.

use std::{fs, process::Command};

const KILND: &str = env!("CARGO_BIN_EXE_kilnd");

/// Append meld's manifest custom section carrying `json`.
fn with_manifest(module_wat: &str, json: &str) -> Vec<u8> {
    let mut wasm = wat::parse_str(module_wat).expect("fixture must assemble");
    let name = "meld.signature-manifest";
    let mut payload = vec![name.len() as u8];
    payload.extend_from_slice(name.as_bytes());
    payload.extend_from_slice(json.as_bytes());
    wasm.push(0);
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

/// `mix(a, b)` writes `{ sum: a+b, product: a*b }` at 1024 and returns 1024.
/// `post_body` is the post-return function's body (it receives the pointer).
fn module(post_body: &str) -> String {
    format!(
        r#"(module
        (memory (export "memory") 1)
        (func (export "p:i/m@1.0.0#mix") (param f32 f32) (result i32)
            (f32.store (i32.const 1024) (f32.add (local.get 0) (local.get 1)))
            (f32.store (i32.const 1028) (f32.mul (local.get 0) (local.get 1)))
            i32.const 1024)
        (func (export "cabi_post_p:i/m@1.0.0#mix") (param i32) {post_body}))"#
    )
}

fn manifest(post_return: &str, result: &str, layout: &str) -> String {
    format!(
        r#"{{"version":1,"exports":[{{
            "export":"p:i/m@1.0.0#mix",
            "wit":{{"params":[["a","f32"],["b","f32"]],"result":"{result}"}},
            "core":{{"params":["f32","f32"],"results":["i32"]}},
            "flat_param_count":2,
            "needs":{{"memory":true,"realloc":false}},
            "memory":"memory","realloc":null,"post_return":{post_return},
            "return_area":{{"size":8,"align":4,"layout":{layout}}}
        }}]}}"#
    )
}

const RECORD: &str = "record { sum: f32, product: f32 }";
const LAYOUT: &str =
    r#"[{"offset":0,"field":"sum","size":4},{"offset":4,"field":"product","size":4}]"#;
const POST: &str = r#""cabi_post_p:i/m@1.0.0#mix""#;

fn run(bytes: &[u8], file: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let path = std::env::temp_dir().join(file);
    fs::write(&path, bytes).unwrap();
    let mut cmd = Command::new(KILND);
    cmd.args(["--invoke", "p:i/m@1.0.0#mix"]);
    for a in args {
        cmd.args(["--arg", a]);
    }
    let out = cmd.arg(&path).output().unwrap();
    let _ = fs::remove_file(&path);
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn prints_the_decoded_fields_and_they_track_the_inputs() {
    let wasm = with_manifest(&module(""), &manifest(POST, RECORD, LAYOUT));

    let (code, out, err) = run(&wasm, "kilnd_sr61_values_a.wasm", &["1.5", "2"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert!(out.contains("sum: f32 3.5"), "stdout: {out}");
    assert!(out.contains("product: f32 3"), "stdout: {out}");

    let (code, out, err) = run(&wasm, "kilnd_sr61_values_b.wasm", &["0.25", "4"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert!(out.contains("sum: f32 4.25"), "stdout: {out}");
    assert!(out.contains("product: f32 1"), "stdout: {out}");
}

#[test]
fn does_not_present_the_pointer_as_the_result() {
    let wasm = with_manifest(&module(""), &manifest(POST, RECORD, LAYOUT));
    let (_, out, _) = run(&wasm, "kilnd_sr61_noptr.wasm", &["1", "1"]);
    assert!(
        !out.contains("[0] i32 1024"),
        "the return-area address must not be reported as the value; stdout: {out}"
    );
}

#[test]
fn calls_post_return_with_the_returned_pointer() {
    // post-return traps unless it receives exactly the pointer mix returned.
    let checks_ptr = "(if (i32.ne (local.get 0) (i32.const 1024)) (then unreachable))";
    let wasm = with_manifest(&module(checks_ptr), &manifest(POST, RECORD, LAYOUT));
    let (code, out, err) = run(&wasm, "kilnd_sr61_post_ok.wasm", &["1", "2"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");

    // And it is really called: a post-return that always traps must fail the run.
    let wasm = with_manifest(&module("unreachable"), &manifest(POST, RECORD, LAYOUT));
    let (code, out, err) = run(&wasm, "kilnd_sr61_post_trap.wasm", &["1", "2"]);
    assert_ne!(
        code,
        Some(0),
        "a trapping post-return must fail the run; stdout: {out}\nstderr: {err}"
    );
}

#[test]
fn refuses_a_return_area_it_cannot_decode_instead_of_printing_the_pointer() {
    let nested = "record { sum: record { x: f32 }, product: f32 }";
    let wasm = with_manifest(&module(""), &manifest(POST, nested, LAYOUT));
    let (code, out, err) = run(&wasm, "kilnd_sr61_nested.wasm", &["1", "2"]);
    assert_ne!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert!(
        err.contains("`sum`"),
        "the refusal must name the field; stderr: {err}"
    );
    assert!(
        !out.contains("i32 1024"),
        "no pointer may be reported; stdout: {out}"
    );
}

#[test]
fn refuses_a_misaligned_return_pointer() {
    // Returns 1026, which violates the manifest's align: 4.
    let wat = r#"(module
        (memory (export "memory") 1)
        (func (export "p:i/m@1.0.0#mix") (param f32 f32) (result i32) i32.const 1026))"#;
    let wasm = with_manifest(wat, &manifest("null", RECORD, LAYOUT));
    let (code, out, err) = run(&wasm, "kilnd_sr61_align.wasm", &["1", "2"]);
    assert_ne!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert!(err.contains("align"), "stderr: {err}");
}

#[test]
fn refuses_a_return_area_outside_memory() {
    // One 64 KiB page; 65532 + 8 bytes runs off the end.
    let wat = r#"(module
        (memory (export "memory") 1)
        (func (export "p:i/m@1.0.0#mix") (param f32 f32) (result i32) i32.const 65532))"#;
    let wasm = with_manifest(wat, &manifest("null", RECORD, LAYOUT));
    let (code, out, err) = run(&wasm, "kilnd_sr61_oob.wasm", &["1", "2"]);
    assert_ne!(code, Some(0), "stdout: {out}\nstderr: {err}");
}
