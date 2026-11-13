//! Every WASI call that returns a guest-owned buffer gets a fresh
//! `cabi_realloc`'d one, whichever route reaches it, and an allocation failure
//! fails the call instead of turning into a different later error (SR-102).
//!
//! The decision "which calls need a fresh buffer" used to live in two name
//! lists that disagreed: `descriptor.read`, `readlink-at` and
//! `read-directory-entry` were only in the list used for canon-lowered links,
//! so a plain `call` of them (the route every meld-fused module uses) got no
//! buffer and failed. `get-directories` could only be served from a buffer
//! allocated before the guest ran, and a `cabi_realloc` that trapped became
//! "requires a cabi_realloc'd buffer".
//!
//! Each fixture is a core module run with `--wasi --wasi-fs <tmpdir>`: the
//! preopen is descriptor handle 1. The guest counts its own `cabi_realloc`
//! calls and reports, on stdout, what it saw.

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

/// A bump allocator that counts its calls; `$last` is the latest allocation.
const ALLOCATOR: &str = r#"
    (global $calls (mut i32) (i32.const 0))
    (global $next (mut i32) (i32.const 4096))
    (global $last (mut i32) (i32.const -1))
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
      (global.set $last (global.get $next))
      (global.set $next (i32.add (global.get $next) (local.get 3)))
      (global.get $last))"#;

/// A `cabi_realloc` that always traps.
const TRAPPING_ALLOCATOR: &str = r#"
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) unreachable)"#;

const STDOUT_IMPORTS: &str = r#"
    (type $w (func (param i32 i32 i32 i32)))
    (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $get_stdout (result i32)))
    (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush"
      (func $write (type $w)))"#;

const OPEN_AT_IMPORT: &str = r#"
    (import "wasi:filesystem/types@0.2.0" "[method]descriptor.open-at"
      (func $open (param i32 i32 i32 i32 i32 i32 i32)))"#;

struct Fixture {
    dir: PathBuf,
    file: PathBuf,
}

impl Fixture {
    /// A fresh preopen directory holding `hello.txt` ("hello world"), a
    /// symlink `link` to it, and nothing else.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kiln_sr102_policy_{name}_dir"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.txt"), b"hello world").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("hello.txt", dir.join("link")).unwrap();
        Self {
            dir,
            file: std::env::temp_dir().join(format!("kiln_sr102_policy_{name}.wasm")),
        }
    }

    fn run(&self, wat: &str, extra: &[&str]) -> Output {
        fs::write(
            &self.file,
            wat::parse_str(wat).expect("fixture must assemble"),
        )
        .unwrap();
        Command::new(KILN)
            .arg("--wasi")
            .arg("--wasi-fs")
            .arg(&self.dir)
            .args(extra)
            .arg(&self.file)
            .env_remove("RUST_LOG")
            .output()
            .unwrap()
    }

    fn run_ok(&self, wat: &str, extra: &[&str]) -> Vec<u8> {
        let out = self.run(wat, extra);
        assert!(
            out.status.success(),
            "run must succeed: {:?}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.file);
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn module(imports: &str, allocator: &str, body: &str) -> String {
    format!(
        r#"(module
    {STDOUT_IMPORTS}
    {imports}
    (memory (export "memory") 1)
    (data (i32.const 64) "hello.txt")
    (data (i32.const 80) "link")
    {allocator}
    (func (export "wasi:cli/run@0.2.0#run") (result i32)
      {body}
      i32.const 0))"#
    )
}

/// `open-at(preopen, hello.txt, read)` leaves the new descriptor at 132, then
/// `descriptor.read(fd, 11, 0, 144)` returns `result<tuple<list<u8>, bool>, _>`:
/// ok (0) at +0, list ptr at +4, list len at +8. Reports `[reallocs, ok,
/// ptr == last allocation, len, first byte, last byte]`.
#[test]
fn descriptor_read_returns_bytes_in_a_cabi_realloc_buffer() {
    let wat = module(
        &format!(
            "{OPEN_AT_IMPORT}
             (import \"wasi:filesystem/types@0.2.0\" \"[method]descriptor.read\"
               (func $read (param i32 i64 i64 i32)))"
        ),
        ALLOCATOR,
        r#"(call $open (i32.const 1) (i32.const 0) (i32.const 64) (i32.const 9)
                  (i32.const 0) (i32.const 1) (i32.const 128))
           (call $read (i32.load (i32.const 132)) (i64.const 11) (i64.const 0) (i32.const 144))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.load (i32.const 144)))
           (i32.store8 (i32.const 202) (i32.eq (i32.load (i32.const 148)) (global.get $last)))
           (i32.store8 (i32.const 203) (i32.load (i32.const 152)))
           (i32.store8 (i32.const 204) (i32.load8_u (i32.load (i32.const 148))))
           (i32.store8 (i32.const 205)
             (i32.load8_u (i32.add (i32.load (i32.const 148)) (i32.const 10))))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 6) (i32.const 0))"#,
    );
    let fx = Fixture::new("descread");
    assert_eq!(fx.run_ok(&wat, &[]), [1, 0, 1, 11, b'h', b'd']);
}

/// `descriptor.readlink-at(preopen, "link", 144)` returns `result<string, _>`:
/// ok (0) at +0, string ptr at +4, len at +8. The target is `hello.txt`.
#[cfg(unix)]
#[test]
fn readlink_at_returns_the_target_in_a_cabi_realloc_buffer() {
    let wat = module(
        r#"(import "wasi:filesystem/types@0.2.0" "[method]descriptor.readlink-at"
             (func $readlink (param i32 i32 i32 i32)))"#,
        ALLOCATOR,
        r#"(call $readlink (i32.const 1) (i32.const 80) (i32.const 4) (i32.const 144))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.load (i32.const 144)))
           (i32.store8 (i32.const 202) (i32.eq (i32.load (i32.const 148)) (global.get $last)))
           (i32.store8 (i32.const 203) (i32.load (i32.const 152)))
           (i32.store8 (i32.const 204) (i32.load8_u (i32.load (i32.const 148))))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 5) (i32.const 0))"#,
    );
    let fx = Fixture::new("readlink");
    assert_eq!(fx.run_ok(&wat, &[]), [1, 0, 1, 9, b'h']);
}

/// `read-directory(preopen, 128)` leaves the stream handle at 132; the first
/// `read-directory-entry(stream, 144)` returns `result<option<entry>, _>` whose
/// name string (ptr at +12, len at +16 in the canonical layout) must live in a
/// `cabi_realloc`'d buffer. The directory holds `hello.txt` and `link` (unix),
/// so the first entry's name is one of them; the test reports the allocation
/// count, the result discriminants and that the name pointer is the allocation.
#[test]
fn read_directory_entry_returns_the_name_in_a_cabi_realloc_buffer() {
    let wat = module(
        r#"(import "wasi:filesystem/types@0.2.0" "[method]descriptor.read-directory"
             (func $readdir (param i32 i32)))
           (import "wasi:filesystem/types@0.2.0"
             "[method]directory-entry-stream.read-directory-entry"
             (func $next (param i32 i32)))"#,
        ALLOCATOR,
        r#"(call $readdir (i32.const 1) (i32.const 128))
           (call $next (i32.load (i32.const 132)) (i32.const 144))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.load (i32.const 144)))
           (i32.store8 (i32.const 202) (i32.load (i32.const 148)))
           (i32.store8 (i32.const 203) (i32.eq (i32.load (i32.const 156)) (global.get $last)))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 4) (i32.const 0))"#,
    );
    let fx = Fixture::new("readdir");
    // one allocation, ok, some(entry), name allocated by cabi_realloc
    assert_eq!(fx.run_ok(&wat, &[]), [1, 0, 1, 1]);
}

/// `get-directories` on a core module: the preopen list and its path string
/// come from `cabi_realloc` called during the call (the guest's allocator is
/// ready by then), so the list is available without any pre-entry allocation.
/// Reports `[reallocs, list len, entry 0 handle]`, then the path string.
#[test]
fn get_directories_works_on_the_core_route() {
    let wat = module(
        r#"(import "wasi:filesystem/preopens@0.2.0" "get-directories" (func $gd (param i32)))"#,
        ALLOCATOR,
        r#"(local $list i32)
           (call $gd (i32.const 128))
           (local.set $list (i32.load (i32.const 128)))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.load (i32.const 132)))
           (i32.store8 (i32.const 202) (i32.load (local.get $list)))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 3) (i32.const 0))
           (call $write (call $get_stdout)
             (i32.load (i32.add (local.get $list) (i32.const 4)))
             (i32.load (i32.add (local.get $list) (i32.const 8)))
             (i32.const 0))"#,
    );
    let fx = Fixture::new("getdirs");
    let out = fx.run_ok(&wat, &[]);
    // list array + one path string = 2 allocations; one preopen with handle 1
    assert_eq!(&out[..3], [2, 1, 1]);
    let path = String::from_utf8_lossy(&out[3..]).into_owned();
    assert!(
        path.contains("kiln_sr102_policy_getdirs_dir"),
        "the preopen path must be the registered directory, got {path:?}"
    );
}

/// `get-directories` called twice hands out two distinct allocations: the
/// guest owns (and may free) each list, so the host must not reuse one.
#[test]
fn get_directories_twice_returns_distinct_allocations() {
    let wat = module(
        r#"(import "wasi:filesystem/preopens@0.2.0" "get-directories" (func $gd (param i32)))"#,
        ALLOCATOR,
        r#"(call $gd (i32.const 128))
           (call $gd (i32.const 144))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.ne (i32.load (i32.const 128)) (i32.load (i32.const 144))))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 2) (i32.const 0))"#,
    );
    let fx = Fixture::new("getdirs2");
    assert_eq!(fx.run_ok(&wat, &[]), [4, 1]);
}

/// `get-arguments` called twice hands out two distinct allocations (program
/// name + one argument: a list array and two strings, per call).
#[test]
fn get_arguments_twice_returns_distinct_allocations() {
    let wat = module(
        r#"(import "wasi:cli/environment@0.2.0" "get-arguments" (func $ga (param i32)))"#,
        ALLOCATOR,
        r#"(call $ga (i32.const 128))
           (call $ga (i32.const 144))
           (i32.store8 (i32.const 200) (global.get $calls))
           (i32.store8 (i32.const 201) (i32.ne (i32.load (i32.const 128)) (i32.load (i32.const 144))))
           (call $write (call $get_stdout) (i32.const 200) (i32.const 2) (i32.const 0))"#,
    );
    let fx = Fixture::new("args2");
    let out = fx.run_ok(&wat, &["--wasi-arg", "foo"]);
    assert_eq!(out[1], 1, "the two lists must be distinct allocations");
    assert_eq!(
        out[0] % 2,
        0,
        "each call allocates the same number of buffers"
    );
    assert!(out[0] >= 2, "each call allocates, got {}", out[0]);
}

/// A trapping `cabi_realloc` fails `descriptor.read`: the trap is the error,
/// not "requires a cabi_realloc'd buffer" and not a success.
#[test]
fn descriptor_read_realloc_trap_propagates() {
    let wat = module(
        &format!(
            "{OPEN_AT_IMPORT}
             (import \"wasi:filesystem/types@0.2.0\" \"[method]descriptor.read\"
               (func $read (param i32 i64 i64 i32)))"
        ),
        TRAPPING_ALLOCATOR,
        r#"(call $open (i32.const 1) (i32.const 0) (i32.const 64) (i32.const 9)
                  (i32.const 0) (i32.const 1) (i32.const 128))
           (call $read (i32.load (i32.const 132)) (i64.const 11) (i64.const 0) (i32.const 144))"#,
    );
    let fx = Fixture::new("readtrap");
    let out = fx.run(&wat, &[]);
    let err = stderr(&out);
    assert!(
        !out.status.success(),
        "a trapping cabi_realloc must fail the run"
    );
    assert!(
        !err.contains("requires a cabi_realloc'd buffer"),
        "the allocation failure was masked as a missing buffer:\n{err}"
    );
}

/// The same for an input-stream read: a trapping `cabi_realloc` is not turned
/// into a missing buffer.
#[test]
fn input_stream_read_realloc_trap_propagates() {
    let wat = module(
        r#"(import "wasi:cli/stdin@0.2.0" "get-stdin" (func $get_stdin (result i32)))
           (import "wasi:io/streams@0.2.0" "[method]input-stream.read"
             (func $rd (param i32 i64 i32)))"#,
        TRAPPING_ALLOCATOR,
        r#"(call $rd (call $get_stdin) (i64.const 16) (i32.const 160))"#,
    );
    let fx = Fixture::new("stdintrap");
    let out = fx.run(&wat, &[]);
    let err = stderr(&out);
    assert!(
        !out.status.success(),
        "a trapping cabi_realloc must fail the run"
    );
    assert!(
        !err.contains("requires a cabi_realloc'd buffer"),
        "the allocation failure was masked as a missing buffer:\n{err}"
    );
}
