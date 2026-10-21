//! Command to generate a CycloneDX VEX document for a release (SR-69).
//!
//! The released `kiln` / `kilnd` binaries are built with
//! `cargo build -p kilnd --release`. `cargo audit` reports every RustSec
//! advisory matching the workspace `Cargo.lock`, but most of the lockfile
//! (cargo-kiln, dev tooling) is never linked into those binaries. This
//! command decides, per advisory, whether the vulnerable package version is
//! in the normal-dependency tree of `kilnd` for any release target, and
//! emits the decision as a CycloneDX VEX document: `exploitable` when the
//! package is shipped, `not_affected` / `code_not_present` when it is not,
//! and `in_triage` when the audit entry cannot be decided.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::{Value, json};

use crate::helpers::OutputManager;

/// Arguments for the vex command
#[derive(Debug, Clone, Args)]
pub struct VexArgs {
    /// CycloneDX SBOM of the release (`kiln-<version>.cdx.json`)
    #[arg(long = "sbom")]
    pub sbom: PathBuf,

    /// Output path of the VEX document
    #[arg(long = "out")]
    pub out: PathBuf,

    /// Release target triple; repeat once per shipped target
    #[arg(long = "target", required = true)]
    pub targets: Vec<String>,

    /// Use this `cargo audit --json` output instead of running `cargo audit`
    #[arg(long = "audit-json")]
    pub audit_json: Option<PathBuf>,
}

/// One advisory from `cargo audit --json`, bound to a locked package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// RustSec advisory id, e.g. `RUSTSEC-2026-0382`
    pub id: String,
    /// Advisory title
    pub title: String,
    /// Locked package name, if the audit entry carries one
    pub package: Option<String>,
    /// Locked package version, if the audit entry carries one
    pub version: Option<String>,
}

/// Reachability decision for one finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The package version is in the kilnd normal tree for these targets.
    Affected { targets: Vec<String> },
    /// The package version is in no release target's kilnd normal tree.
    NotAffected,
    /// The audit entry is incomplete; a human must decide.
    InTriage { reason: String },
}

/// The parts of the release SBOM the VEX refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sbom {
    /// CycloneDX spec version of the SBOM (the VEX uses the same)
    pub spec_version: String,
    /// SBOM serial number (`urn:uuid:...`), the target of bom-links
    pub serial_number: String,
    /// SBOM document version, the target of bom-links
    pub bom_version: u64,
    /// Release component name
    pub name: String,
    /// Release component version
    pub version: String,
    /// `(name, version)` -> bom-ref of every SBOM component
    pub refs: BTreeMap<(String, String), String>,
}

/// Parse `cargo audit --json` into findings (vulnerabilities and warnings).
///
/// Yanked-crate warnings carry no advisory (`"advisory": null`) and are not
/// RustSec advisories, so they are not part of a VEX and are skipped.
pub fn parse_audit(json_text: &str) -> Result<Vec<Finding>> {
    let root: Value =
        serde_json::from_str(json_text).context("cargo audit output is not valid JSON")?;

    let mut entries: Vec<&Value> = root
        .pointer("/vulnerabilities/list")
        .and_then(Value::as_array)
        .context("cargo audit JSON has no vulnerabilities.list array")?
        .iter()
        .collect();
    let warnings = root
        .get("warnings")
        .and_then(Value::as_object)
        .context("cargo audit JSON has no warnings object")?;
    for kind_entries in warnings.values() {
        entries.extend(
            kind_entries
                .as_array()
                .context("cargo audit JSON warnings entry is not an array")?,
        );
    }

    let mut findings: Vec<Finding> = Vec::new();
    for entry in entries {
        let advisory = match entry.get("advisory") {
            Some(advisory) if !advisory.is_null() => advisory,
            _ => continue,
        };
        let id = advisory
            .get("id")
            .and_then(Value::as_str)
            .context("cargo audit advisory has no id")?;
        let title = advisory
            .get("title")
            .and_then(Value::as_str)
            .with_context(|| format!("advisory {id} has no title"))?;
        let package = entry
            .pointer("/package/name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let version = entry
            .pointer("/package/version")
            .and_then(Value::as_str)
            .map(str::to_string);
        let finding = Finding { id: id.to_string(), title: title.to_string(), package, version };
        if !findings.contains(&finding) {
            findings.push(finding);
        }
    }
    Ok(findings)
}

/// Parse `cargo tree --prefix none` output into `(name, version)` pairs.
pub fn parse_tree(text: &str) -> Result<BTreeSet<(String, String)>> {
    let mut packages = BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut tokens = line.split_whitespace();
        let name = tokens.next();
        let version = tokens.next().and_then(|token| token.strip_prefix('v'));
        match (name, version) {
            (Some(name), Some(version)) => {
                packages.insert((name.to_string(), version.to_string()));
            },
            _ => bail!("unparseable cargo tree line: {line:?}"),
        }
    }
    Ok(packages)
}

/// Parse the release SBOM.
pub fn parse_sbom(json_text: &str) -> Result<Sbom> {
    let root: Value = serde_json::from_str(json_text).context("SBOM is not valid JSON")?;
    let text_field = |pointer: &str| -> Result<String> {
        root.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
            .with_context(|| format!("SBOM is missing {pointer}"))
    };

    let mut refs = BTreeMap::new();
    for component in root
        .get("components")
        .and_then(Value::as_array)
        .context("SBOM has no components array")?
    {
        let field = |key: &str| -> Result<String> {
            component
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .with_context(|| format!("SBOM component is missing {key}: {component}"))
        };
        refs.insert((field("name")?, field("version")?), field("bom-ref")?);
    }

    Ok(Sbom {
        spec_version: text_field("/specVersion")?,
        serial_number: text_field("/serialNumber")?,
        // CycloneDX defaults an absent document `version` to 1.
        bom_version: root.get("version").and_then(Value::as_u64).unwrap_or(1),
        name: text_field("/metadata/component/name")?,
        version: text_field("/metadata/component/version")?,
        refs,
    })
}

/// Decide whether a finding reaches the released binaries.
pub fn classify(
    finding: &Finding,
    reach: &BTreeMap<String, BTreeSet<(String, String)>>,
) -> Verdict {
    let (package, version) = match (&finding.package, &finding.version) {
        (Some(package), Some(version)) => (package, version),
        _ => {
            return Verdict::InTriage {
                reason: "the cargo audit entry names no locked package version".to_string(),
            };
        },
    };
    let key = (package.clone(), version.clone());
    let targets: Vec<String> = reach
        .iter()
        .filter(|(_, tree)| tree.contains(&key))
        .map(|(target, _)| target.clone())
        .collect();
    if targets.is_empty() { Verdict::NotAffected } else { Verdict::Affected { targets } }
}

/// Build the CycloneDX VEX document.
pub fn build_vex(
    sbom: &Sbom,
    findings: &[(Finding, Verdict)],
    targets: &[String],
    timestamp: &str,
) -> Result<Value> {
    if sbom.spec_version != "1.5" && sbom.spec_version != "1.6" {
        bail!(
            "SBOM specVersion {} is not supported (expected 1.5 or 1.6)",
            sbom.spec_version
        );
    }
    let bom_link_prefix = format!(
        "urn:cdx:{}/{}#",
        sbom.serial_number
            .strip_prefix("urn:uuid:")
            .with_context(|| format!("SBOM serialNumber {} is not a urn:uuid", sbom.serial_number))?,
        sbom.bom_version
    );

    let mut ordered: Vec<&(Finding, Verdict)> = findings.iter().collect();
    ordered.sort_by(|(a, _), (b, _)| {
        (&a.id, &a.package, &a.version).cmp(&(&b.id, &b.package, &b.version))
    });

    let target_list = targets.join(", ");
    let mut local_components: BTreeMap<String, Value> = BTreeMap::new();
    let mut vulnerabilities = Vec::new();
    for (finding, verdict) in ordered {
        let affects_ref = match (&finding.package, &finding.version) {
            (Some(package), Some(version)) => {
                match sbom.refs.get(&(package.clone(), version.clone())) {
                    Some(bom_ref) => Some(format!("{bom_link_prefix}{bom_ref}")),
                    None if matches!(verdict, Verdict::Affected { .. }) => bail!(
                        "{} {} is in the kilnd dependency tree but not in the SBOM",
                        package,
                        version
                    ),
                    None => {
                        let purl = format!("pkg:cargo/{package}@{version}");
                        local_components.insert(
                            purl.clone(),
                            json!({
                                "type": "library",
                                "bom-ref": purl,
                                "name": package,
                                "version": version,
                                "purl": purl,
                            }),
                        );
                        Some(purl)
                    },
                }
            },
            _ => None,
        };

        let subject = match (&finding.package, &finding.version) {
            (Some(package), Some(version)) => format!("{package} {version}"),
            _ => "the audited package".to_string(),
        };
        let analysis = match verdict {
            Verdict::Affected { targets: reached } => json!({
                "state": "exploitable",
                "response": ["update"],
                "detail": format!(
                    "{subject} is in the normal dependency tree of kilnd for {} and is \
                     compiled into the released binaries.",
                    reached.join(", ")
                ),
            }),
            Verdict::NotAffected => json!({
                "state": "not_affected",
                "justification": "code_not_present",
                "detail": format!(
                    "{subject} is in the normal dependency tree of kilnd for none of the \
                     release targets ({target_list}) (cargo tree -p kilnd -e normal); it is \
                     not compiled into the released binaries."
                ),
            }),
            Verdict::InTriage { reason } => json!({
                "state": "in_triage",
                "detail": format!("Not classified: {reason}."),
            }),
        };

        let mut vulnerability = json!({
            "id": finding.id,
            "source": {
                "name": "RustSec",
                "url": format!("https://rustsec.org/advisories/{}.html", finding.id),
            },
            "description": finding.title,
            "analysis": analysis,
        });
        if let Some(affects_ref) = affects_ref {
            vulnerability["affects"] = json!([{ "ref": affects_ref }]);
        }
        vulnerabilities.push(vulnerability);
    }

    let mut vex = json!({
        "bomFormat": "CycloneDX",
        "specVersion": sbom.spec_version,
        "version": 1,
        "metadata": {
            "timestamp": timestamp,
            "component": {
                "type": "application",
                "name": sbom.name,
                "version": sbom.version,
            },
        },
        "vulnerabilities": vulnerabilities,
    });
    if !local_components.is_empty() {
        vex["components"] = Value::Array(local_components.into_values().collect());
    }
    Ok(vex)
}

/// Run a command and return its stdout, failing loudly on a spawn error.
fn run_stdout(command: &mut Command, what: &str) -> Result<(std::process::ExitStatus, String)> {
    let result = command.output().with_context(|| format!("failed to run {what}"))?;
    let stdout = String::from_utf8(result.stdout)
        .with_context(|| format!("{what} printed non-UTF-8 output"))?;
    if !result.status.success() && stdout.trim().is_empty() {
        bail!(
            "{what} failed ({}): {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
    }
    Ok((result.status, stdout))
}

/// Execute the vex command
pub fn execute(args: VexArgs, output: &OutputManager) -> Result<()> {
    let sbom_text = std::fs::read_to_string(&args.sbom)
        .with_context(|| format!("failed to read SBOM {}", args.sbom.display()))?;
    let sbom = parse_sbom(&sbom_text)?;

    let audit_text = match &args.audit_json {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        None => {
            output.progress("Running cargo audit --json");
            // `cargo audit` exits non-zero when it finds vulnerabilities but
            // still prints the JSON report; an unparseable report is an error.
            run_stdout(Command::new("cargo").args(["audit", "--json"]), "cargo audit")?.1
        },
    };
    let findings = parse_audit(&audit_text)?;

    let mut reach = BTreeMap::new();
    for target in &args.targets {
        output.progress(&format!("Resolving kilnd normal dependency tree for {target}"));
        let (status, tree_text) = run_stdout(
            Command::new("cargo").args([
                "tree", "-p", "kilnd", "-e", "normal", "--target", target, "--prefix", "none",
                "--locked",
            ]),
            &format!("cargo tree for {target}"),
        )?;
        if !status.success() {
            bail!("cargo tree for {target} failed ({status})");
        }
        reach.insert(target.clone(), parse_tree(&tree_text)?);
    }

    let classified: Vec<(Finding, Verdict)> = findings
        .into_iter()
        .map(|finding| {
            let verdict = classify(&finding, &reach);
            (finding, verdict)
        })
        .collect();
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let vex = build_vex(&sbom, &classified, &args.targets, &timestamp)?;

    let mut text = serde_json::to_string_pretty(&vex)?;
    text.push('\n');
    std::fs::write(&args.out, text)
        .with_context(|| format!("failed to write {}", args.out.display()))?;

    for (finding, verdict) in &classified {
        output.info(&format!("{}: {verdict:?}", finding.id));
    }
    output.success(&format!(
        "Wrote VEX with {} advisories to {}",
        classified.len(),
        args.out.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUDIT: &str = r#"{
      "vulnerabilities": {"found": true, "count": 1, "list": [
        {"advisory": {"id": "RUSTSEC-2026-0382", "title": "webpki flaw", "package": "rustls-webpki"},
         "package": {"name": "rustls-webpki", "version": "0.103.12"}}
      ]},
      "warnings": {
        "unmaintained": [
          {"kind": "unmaintained", "package": {"name": "atty", "version": "0.2.14"},
           "advisory": {"id": "RUSTSEC-2024-0375", "title": "`atty` is unmaintained", "package": "atty"}}
        ],
        "yanked": [
          {"kind": "yanked", "package": {"name": "chacha20", "version": "0.10.1"}, "advisory": null}
        ]
      }
    }"#;

    const TREE: &str = "kilnd v0.6.0 (/w/kilnd)\n\
        kiln-error v0.6.0 (/w/kiln-error)\n\
        serde v1.0.200\n\
        rustls-webpki v0.103.12 (*)\n\
        \n";

    const SBOM: &str = r#"{
      "bomFormat": "CycloneDX", "specVersion": "1.5",
      "serialNumber": "urn:uuid:d73b3ca8-9116-4d8d-9447-8ea378864834",
      "metadata": {"component": {"type": "application", "name": "kilnd", "version": "0.6.0",
                                  "bom-ref": "path+file:///w/kilnd#0.6.0"}},
      "components": [
        {"type": "library", "name": "serde", "version": "1.0.200",
         "bom-ref": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.200"},
        {"type": "library", "name": "rustls-webpki", "version": "0.103.12",
         "bom-ref": "registry+https://github.com/rust-lang/crates.io-index#rustls-webpki@0.103.12"}
      ]
    }"#;

    fn finding(id: &str, name: &str, version: &str) -> Finding {
        Finding {
            id: id.to_string(),
            title: format!("{id} title"),
            package: Some(name.to_string()),
            version: Some(version.to_string()),
        }
    }

    fn reach_of(trees: &[(&str, &str)]) -> BTreeMap<String, BTreeSet<(String, String)>> {
        trees
            .iter()
            .map(|(target, tree)| (target.to_string(), parse_tree(tree).unwrap()))
            .collect()
    }

    #[test]
    fn parse_audit_collects_vulnerabilities_and_warnings_skipping_yanked() {
        let findings = parse_audit(AUDIT).unwrap();
        let ids: Vec<&str> = findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["RUSTSEC-2026-0382", "RUSTSEC-2024-0375"]);
        assert_eq!(findings[0].package.as_deref(), Some("rustls-webpki"));
        assert_eq!(findings[0].version.as_deref(), Some("0.103.12"));
    }

    #[test]
    fn parse_audit_rejects_non_json() {
        assert!(parse_audit("error: no lockfile").is_err());
    }

    #[test]
    fn parse_tree_reads_name_and_version_and_skips_blank_lines() {
        let tree = parse_tree(TREE).unwrap();
        assert!(tree.contains(&("serde".to_string(), "1.0.200".to_string())));
        assert!(tree.contains(&("rustls-webpki".to_string(), "0.103.12".to_string())));
        assert_eq!(tree.len(), 4);
    }

    #[test]
    fn parse_tree_rejects_unparseable_line() {
        assert!(parse_tree("garbage\n").is_err());
    }

    #[test]
    fn classify_affected_when_package_in_any_target_tree() {
        let reach = reach_of(&[
            ("x86_64-unknown-linux-gnu", "kilnd v0.6.0 (/w)\n"),
            ("aarch64-apple-darwin", TREE),
        ]);
        let verdict = classify(&finding("RUSTSEC-2026-0382", "rustls-webpki", "0.103.12"), &reach);
        assert_eq!(
            verdict,
            Verdict::Affected { targets: vec!["aarch64-apple-darwin".to_string()] }
        );
    }

    #[test]
    fn classify_not_affected_when_package_in_no_tree() {
        let reach = reach_of(&[("x86_64-unknown-linux-gnu", TREE)]);
        assert_eq!(
            classify(&finding("RUSTSEC-2024-0375", "atty", "0.2.14"), &reach),
            Verdict::NotAffected
        );
    }

    #[test]
    fn classify_not_affected_when_only_a_different_version_is_in_tree() {
        let reach = reach_of(&[("x86_64-unknown-linux-gnu", TREE)]);
        assert_eq!(
            classify(&finding("RUSTSEC-2026-0382", "rustls-webpki", "0.103.99"), &reach),
            Verdict::NotAffected
        );
    }

    #[test]
    fn classify_in_triage_when_audit_entry_has_no_package() {
        let reach = reach_of(&[("x86_64-unknown-linux-gnu", TREE)]);
        let incomplete = Finding {
            id: "RUSTSEC-2099-0001".to_string(),
            title: "t".to_string(),
            package: None,
            version: None,
        };
        assert!(matches!(classify(&incomplete, &reach), Verdict::InTriage { .. }));
    }

    fn sample_vex() -> Value {
        let sbom = parse_sbom(SBOM).unwrap();
        let reach = reach_of(&[("x86_64-unknown-linux-gnu", TREE)]);
        let findings: Vec<(Finding, Verdict)> = parse_audit(AUDIT)
            .unwrap()
            .into_iter()
            .map(|f| {
                let v = classify(&f, &reach);
                (f, v)
            })
            .collect();
        build_vex(
            &sbom,
            &findings,
            &["x86_64-unknown-linux-gnu".to_string()],
            "2026-10-21T00:00:00Z",
        )
        .unwrap()
    }

    #[test]
    fn vex_has_required_cyclonedx_fields() {
        let vex = sample_vex();
        assert_eq!(vex["bomFormat"], "CycloneDX");
        assert_eq!(vex["specVersion"], "1.5");
        assert_eq!(vex["version"], 1);
        assert_eq!(vex["metadata"]["timestamp"], "2026-10-21T00:00:00Z");
        assert_eq!(vex["metadata"]["component"]["name"], "kilnd");
        for vuln in vex["vulnerabilities"].as_array().unwrap() {
            assert!(vuln["id"].is_string());
            assert!(vuln["source"]["name"].is_string());
            assert!(vuln["analysis"]["state"].is_string());
            assert!(vuln["analysis"]["detail"].is_string());
            assert!(vuln["affects"][0]["ref"].is_string());
        }
    }

    #[test]
    fn vex_marks_shipped_package_exploitable_and_links_sbom_bom_ref() {
        let vex = sample_vex();
        let vuln = vex["vulnerabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "RUSTSEC-2026-0382")
            .unwrap();
        assert_eq!(vuln["analysis"]["state"], "exploitable");
        assert!(vuln["analysis"].get("justification").is_none());
        assert_eq!(
            vuln["affects"][0]["ref"],
            "urn:cdx:d73b3ca8-9116-4d8d-9447-8ea378864834/1#registry+https://github.com/rust-lang/crates.io-index#rustls-webpki@0.103.12"
        );
    }

    #[test]
    fn vex_marks_absent_package_not_affected_code_not_present_with_local_component() {
        let vex = sample_vex();
        let vuln = vex["vulnerabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "RUSTSEC-2024-0375")
            .unwrap();
        assert_eq!(vuln["analysis"]["state"], "not_affected");
        assert_eq!(vuln["analysis"]["justification"], "code_not_present");
        assert_eq!(vuln["affects"][0]["ref"], "pkg:cargo/atty@0.2.14");
        let components = vex["components"].as_array().unwrap();
        assert!(components.iter().any(|c| c["bom-ref"] == "pkg:cargo/atty@0.2.14"
            && c["purl"] == "pkg:cargo/atty@0.2.14"
            && c["name"] == "atty"));
    }

    #[test]
    fn vex_output_is_deterministic_and_sorted_by_advisory_id() {
        let first = sample_vex();
        let second = sample_vex();
        assert_eq!(first, second);
        let ids: Vec<&str> = first["vulnerabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["RUSTSEC-2024-0375", "RUSTSEC-2026-0382"]);
    }

    #[test]
    fn build_vex_rejects_unsupported_spec_version() {
        let mut sbom = parse_sbom(SBOM).unwrap();
        sbom.spec_version = "1.4".to_string();
        assert!(build_vex(&sbom, &[], &["t".to_string()], "2026-10-21T00:00:00Z").is_err());
    }

    #[test]
    fn build_vex_rejects_affected_package_missing_from_sbom() {
        let sbom = parse_sbom(SBOM).unwrap();
        let f = finding("RUSTSEC-2099-0002", "ghost", "1.0.0");
        let verdict = Verdict::Affected { targets: vec!["t".to_string()] };
        assert!(build_vex(&sbom, &[(f, verdict)], &["t".to_string()], "2026-10-21T00:00:00Z").is_err());
    }
}
