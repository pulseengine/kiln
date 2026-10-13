//! PulseEngine CLI baseline conformance for the `kiln` binary (issue #486).
//!
//! The baseline every varve-layer tool must meet:
//!   1. `--version` / `-V` print `<binary-name> <semver>` and exit 0.
//!   2. `--help` exits 0; an unknown flag exits 2 with usage on stderr.
//!
//! These are pinned as tests because kiln *executes other tools' artifacts*
//! (witness' `--harness` cross-check runs modules under kiln), so "which
//! runtime produced this run" has to be answerable from the binary itself.
//! Asserting the reported version equals `CARGO_PKG_VERSION` — which is the
//! workspace version the release tag is cut from — is the in-repo half of the
//! "assert `--version` == the tag" release check #486 asks for.

use std::process::Command;

/// Path to the freshly built binary under test (cargo sets this for integration
/// tests, so it is correct for any profile — no hand-built target/debug path).
const KILN: &str = env!("CARGO_BIN_EXE_kiln");

#[test]
fn version_flag_prints_name_and_semver_and_exits_zero() {
    for flag in ["--version", "-V"] {
        let out = Command::new(KILN)
            .arg(flag)
            .output()
            .expect("kiln must be executable");

        assert!(
            out.status.success(),
            "`kiln {flag}` must exit 0 (asking for the version is not an error); got {:?}",
            out.status.code()
        );

        let stdout = String::from_utf8_lossy(&out.stdout);
        let expected = format!("kiln {}", env!("CARGO_PKG_VERSION"));
        assert_eq!(
            stdout.trim(),
            expected,
            "`kiln {flag}` must print `<binary-name> <semver>`; got {stdout:?}"
        );
    }
}

/// The reported version must track the workspace version the tag is cut from —
/// this is what makes a run attributable to a release.
#[test]
fn reported_version_is_the_crate_version() {
    let out = Command::new(KILN).arg("--version").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let reported = stdout.trim().strip_prefix("kiln ").unwrap_or("").to_string();
    assert_eq!(
        reported,
        env!("CARGO_PKG_VERSION"),
        "reported version must equal CARGO_PKG_VERSION (the version the release tag is cut from)"
    );
    assert!(
        !reported.is_empty() && reported.chars().next().unwrap().is_ascii_digit(),
        "version must be a semver, got {reported:?}"
    );
}

#[test]
fn help_exits_zero() {
    for flag in ["--help", "-h"] {
        let status = Command::new(KILN).arg(flag).status().unwrap();
        assert!(
            status.success(),
            "`kiln {flag}` must exit 0 — asking for help is not an error; got {:?}",
            status.code()
        );
    }
}

#[test]
fn unknown_long_flag_exits_two_with_usage_on_stderr() {
    let out = Command::new(KILN)
        .arg("--definitely-not-a-flag")
        .output()
        .unwrap();

    assert_eq!(
        out.status.code(),
        Some(2),
        "an unrecognized flag must exit 2 per the CLI baseline (usage error), not 1 or 0"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized argument") && stderr.contains("--definitely-not-a-flag"),
        "the offending argument must be named on stderr; got {stderr:?}"
    );
    assert!(
        out.stdout.is_empty(),
        "usage errors belong on stderr, not stdout; stdout was {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `-V` used to fall through the `!starts_with("--")` positional guard and be
/// treated as the *module path*, so it failed with "failed to read module".
/// Any unknown single-dash flag must now be rejected as a flag.
#[test]
fn unknown_short_flag_is_rejected_not_treated_as_module_path() {
    let out = Command::new(KILN).arg("-x").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown short flag must be rejected as a flag, not consumed as a positional module path"
    );
}

/// The safety-relevant half of #486: silently ignoring unknown flags meant a
/// typo'd resource cap (`--memroy`) was dropped *and* its operand was then
/// eaten as a positional — so the module could run with no cap applied. That is
/// the same "configured control that never takes effect" class as SR-44/SR-45.
#[test]
fn typoed_resource_cap_flag_is_rejected_not_silently_dropped() {
    let out = Command::new(KILN)
        .args(["--memroy", "65536", "some_module.wasm"])
        .output()
        .unwrap();

    assert_eq!(
        out.status.code(),
        Some(2),
        "a mistyped cap flag must fail loud; silently dropping it runs the module unbounded"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--memroy"),
        "the typo must be named so the user can see it; got {stderr:?}"
    );
}
