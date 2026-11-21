//! The released runtime binary is `kiln`, and it is the only one: the
//! deprecated `kilnd` binary alias was removed (SR-66). The Cargo package keeps
//! the name `kilnd`.
//!
//! The repo, the release tarball and the varve roster all say `kiln`, and the
//! CLI is a one-shot runner, not a daemon.

use std::process::Command;

const KILN: &str = env!("CARGO_BIN_EXE_kiln");

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
        stderr.is_empty(),
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
fn only_the_kiln_binary_is_declared_and_released() {
    let cargo_toml = include_str!("../Cargo.toml");
    let release_yml = include_str!("../../.github/workflows/release.yml");

    for section in cargo_toml.split("[[bin]]").skip(1) {
        // A bin section ends at the next table header.
        let body = section.split("\n[").next().unwrap_or(section);
        for line in body.lines() {
            let line = line.trim();
            assert!(
                line.replace(' ', "") != "name=\"kilnd\"",
                "kilnd/Cargo.toml must not declare a `kilnd` [[bin]]; got section {body:?}"
            );
        }
    }
    assert!(
        !release_yml.contains("release/kilnd"),
        "release.yml must not strip or stage a `kilnd` binary"
    );
    assert!(
        !release_yml.contains("for BIN in kiln kilnd"),
        "release.yml must not loop over a `kilnd` binary"
    );
}

#[test]
fn ci_does_not_run_a_kilnd_binary() {
    // The static musl job runs only on main, so a PR's required checks do not
    // see it; this test is what catches a leftover `kilnd` binary there.
    let ci_yml = include_str!("../../.github/workflows/ci.yml");
    for (n, line) in ci_yml.lines().enumerate() {
        assert!(
            !line.contains("for BIN in kiln kilnd") && !line.contains("release/kilnd"),
            "ci.yml line {} must not run a `kilnd` binary: {line:?}",
            n + 1
        );
    }
}
