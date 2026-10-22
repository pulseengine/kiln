//! Compiler-enforced unsafe policy (SR-73, #568).
//!
//! Every workspace crate's library and binary roots must carry
//! `#![deny(unsafe_code)]` or `#![forbid(unsafe_code)]`, unless the crate is
//! named in `safety/unsafe-allowlist.yaml` with a justification.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Allowlist {
    allowed: Vec<AllowedCrate>,
}

#[derive(Debug, Deserialize)]
struct AllowedCrate {
    #[serde(rename = "crate")]
    name: String,
    reason: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kiln-build-core sits directly under the workspace root")
        .to_path_buf()
}

fn load_allowlist() -> Allowlist {
    let path = workspace_root().join("safety/unsafe-allowlist.yaml");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    rivet_yaml::from_str(&content).unwrap_or_else(|e| panic!("invalid {}: {e}", path.display()))
}

/// One library or binary root of a workspace package.
struct Root {
    package: String,
    src_path: PathBuf,
}

fn workspace_roots() -> Vec<Root> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root())
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON");

    let mut roots = Vec::new();
    for package in metadata["packages"].as_array().expect("packages array") {
        let name = package["name"].as_str().expect("package name").to_string();
        for target in package["targets"].as_array().expect("targets array") {
            let is_root = target["kind"].as_array().expect("target kind").iter().any(|k| {
                matches!(
                    k.as_str(),
                    Some("lib" | "rlib" | "cdylib" | "staticlib" | "bin")
                )
            });
            if is_root {
                roots.push(Root {
                    package: name.clone(),
                    src_path: PathBuf::from(target["src_path"].as_str().expect("src_path")),
                });
            }
        }
    }
    roots
}

/// True if the source carries a crate-level `deny`/`forbid` of `unsafe_code`.
fn has_unsafe_policy(source: &str) -> bool {
    source.lines().any(|line| {
        let code = line.trim_start();
        code.starts_with("#![deny(unsafe_code)]") || code.starts_with("#![forbid(unsafe_code)]")
    })
}

/// Roots that violate the policy, as `package (path)`.
fn violations(roots: &[Root], allowed: &BTreeSet<&str>) -> Vec<String> {
    roots
        .iter()
        .filter(|root| !allowed.contains(root.package.as_str()))
        .filter(|root| {
            let source = std::fs::read_to_string(&root.src_path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.src_path.display()));
            !has_unsafe_policy(&source)
        })
        .map(|root| format!("{} ({})", root.package, root.src_path.display()))
        .collect()
}

#[test]
fn every_workspace_root_denies_unsafe_code_unless_allow_listed() {
    let allowlist = load_allowlist();
    let allowed: BTreeSet<&str> = allowlist.allowed.iter().map(|c| c.name.as_str()).collect();
    let roots = workspace_roots();

    let missing = violations(&roots, &allowed);
    assert!(
        missing.is_empty(),
        "crate roots without #![deny(unsafe_code)] or #![forbid(unsafe_code)] that are not in \
         safety/unsafe-allowlist.yaml:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn allowlist_names_only_workspace_crates_and_justifies_each() {
    let allowlist = load_allowlist();
    let packages: BTreeSet<String> = workspace_roots().into_iter().map(|r| r.package).collect();

    for entry in &allowlist.allowed {
        assert!(
            packages.contains(&entry.name),
            "allow-list names `{}`, which is not a workspace crate",
            entry.name
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "allow-list entry `{}` has no justification",
            entry.name
        );
    }
}

#[test]
fn policy_detector_rejects_a_root_without_the_attribute() {
    assert!(has_unsafe_policy(
        "//! docs\n#![deny(unsafe_code)] // note\nfn f() {}\n"
    ));
    assert!(has_unsafe_policy("#![forbid(unsafe_code)]\n"));
    assert!(!has_unsafe_policy(
        "//! docs\n// #![deny(unsafe_code)]\nfn f() {}\n"
    ));
    assert!(!has_unsafe_policy("#![allow(unsafe_code)]\n"));
}
