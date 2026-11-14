//! Instantiation must leave guest linear memory exactly as the data segments
//! initialised it (SR-105).
//!
//! The engine used to write 16 zero bytes at fixed guest address 0x100 after
//! data-segment initialisation, clobbering whatever the guest kept there.

use std::{fs, process::Command};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

// rivet: verifies SR-105
#[test]
fn data_segment_at_0x100_survives_instantiation() {
    let wat = r#"(module
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))
    (memory (export "memory") 1)
    (data (i32.const 256) "KILNKILNKILNKILN")
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      (call $write (call $get_stdout) (i32.const 256) (i32.const 16) (i32.const 128))
      i32.const 0))"#;
    let path = std::env::temp_dir().join("kiln_sr105_data_0x100.wasm");
    fs::write(&path, wat::parse_str(wat).expect("fixture must assemble")).unwrap();
    let out = Command::new(KILN)
        .arg("--wasi")
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    assert!(
        out.status.success(),
        "run must succeed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"KILNKILNKILNKILN");
}
