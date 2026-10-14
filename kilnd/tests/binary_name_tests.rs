//! The released runtime binary is `kiln`; `kilnd` is a deprecated alias for one
//! release (SR-64, issue #486 ask 3).
//!
//! The repo, the release tarball and the varve roster all say `kiln`, and the
//! CLI is a one-shot runner, not a daemon. `kilnd` keeps working for v0.6.0 so
//! existing callers (witness `--harness`) don't break, but it warns on stderr —
//! never on stdout, which callers parse.

use std::process::Command;

const KILN: &str = env!("CARGO_BIN_EXE_kiln");
const KILND: &str = env!("CARGO_BIN_EXE_kilnd");

const DEPRECATION_MARKER: &str = "`kilnd` is deprecated";

#[test]
fn kiln_version_reports_the_kiln_name() {
    let out = Command::new(KILN).arg("--version").output().expect("kiln must be executable");
    assert!(
        out.status.success(),
        "`kiln --version` must exit 0; got {:?}",
        out.status.code()
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("kiln {}", env!("CARGO_PKG_VERSION")),
        "`kiln --version` must print `kiln <semver>`"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains(DEPRECATION_MARKER),
        "`kiln` must not warn; stderr: {stderr:?}"
    );
}

#[test]
fn kiln_help_uses_the_kiln_name() {
    let out = Command::new(KILN).arg("--help").output().unwrap();
    assert!(out.status.success(), "`kiln --help` must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Usage: kiln [OPTIONS] <module.wasm>"),
        "help must name the invoked binary; got {stdout:?}"
    );
    assert!(
        !stdout.contains("kilnd"),
        "`kiln --help` must not mention kilnd; got {stdout:?}"
    );
}

#[test]
fn kiln_unknown_flag_usage_names_kiln() {
    let out = Command::new(KILN).arg("--no-such-flag").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "unknown flag must exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Try 'kiln --help'"),
        "usage hint must name the invoked binary; got {stderr:?}"
    );
}

#[test]
fn kilnd_alias_reports_its_own_name_and_keeps_stdout_clean() {
    let out = Command::new(KILND).arg("--version").output().expect("kilnd alias must ship");
    assert!(
        out.status.success(),
        "`kilnd --version` must still exit 0 during deprecation"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("kilnd {}", env!("CARGO_PKG_VERSION")),
        "the alias reports its installed name, and stdout carries nothing else"
    );
}

#[test]
fn kilnd_alias_warns_on_stderr_and_points_at_kiln() {
    let out = Command::new(KILND).arg("--version").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(DEPRECATION_MARKER),
        "alias must warn on stderr; got {stderr:?}"
    );
    assert!(
        stderr.contains("use `kiln`"),
        "warning must name the replacement; got {stderr:?}"
    );
    assert!(
        stderr.contains("v0.7.0"),
        "warning must say when the alias goes away; got {stderr:?}"
    );
}
