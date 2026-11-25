//! Proof-ledger checker (SR-121).
//!
//! A committed ledger (`safety/verification/proof-ledger.toml`) lists every
//! Kani harness and every input-narrowing site (assumptions, unwind bounds,
//! stubs, opaque bodies, admits) found in the source tree. `scan` produces a
//! draft ledger from the source; `check` fails when the ledger disagrees with
//! the source, when it grows relative to a base ledger, or when a floor
//! harness did not verify in a supplied Kani log.
//!
//! The pattern strings are assembled with `concat!` so this file does not
//! contain them literally and therefore never needs a scan exclusion.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::Path,
};

use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use walkdir::WalkDir;

/// Directory names never scanned.
const SKIP_DIRS: [&str; 4] = ["target", "external", ".claude", ".git"];

/// Substrings that mark a Kani harness.
fn harness_markers() -> [&'static str; 3] {
    [
        concat!("#[kani", "::proof]"),
        concat!("kani", "::proof)"),
        concat!("kani", "::proof_for_contract"),
    ]
}

/// How a narrowing kind attaches to a function.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Attach {
    /// Attribute: belongs to the next `fn` at or after the line.
    Next,
    /// Call: belongs to the nearest preceding `fn` line.
    Preceding,
}

/// Kind name of opaque-body attributes (built so it is not a literal here).
const OPAQUE_KIND: &str = concat!("external", "_body");

/// Narrowing kinds: name, literal pattern, attachment.
fn narrowing_kinds() -> [(&'static str, &'static str, Attach); 4] {
    [
        ("assume", concat!("kani", "::assume("), Attach::Preceding),
        ("unwind", concat!("kani", "::unwind("), Attach::Next),
        ("stub", concat!("kani", "::stub("), Attach::Next),
        (OPAQUE_KIND, concat!("external", "_body"), Attach::Next),
    ]
}

/// A scanned harness.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScannedHarness {
    /// Repo-relative file.
    pub file: String,
    /// Function name.
    pub name: String,
}

/// Key of a narrowing group: (file, function, kind).
pub type NarrowKey = (String, String, String);

/// Result of scanning a source tree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanResult {
    /// Harnesses found.
    pub harnesses: BTreeSet<ScannedHarness>,
    /// Narrowing site counts by (file, function, kind).
    pub narrowing: BTreeMap<NarrowKey, u32>,
}

fn fn_regex() -> Result<Regex> {
    Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").context("compile fn regex")
}

fn admit_regex() -> Result<Regex> {
    Regex::new(r"\badmit\(").context("compile admit regex")
}

/// Scan one file's text.
pub fn scan_text(file: &str, text: &str, out: &mut ScanResult) -> Result<()> {
    let fn_re = fn_regex()?;
    let admit_re = admit_regex()?;
    let lines: Vec<&str> = text.lines().collect();
    let is_comment = |l: &str| l.trim_start().starts_with("//");

    let names: Vec<Option<String>> = lines
        .iter()
        .map(
            |l| {
                if is_comment(l) { None } else { fn_re.captures(l).map(|c| c[1].to_string()) }
            },
        )
        .collect();
    // prev[i]: last fn at index <= i. next[i]: first fn at index >= i.
    let mut prev: Vec<Option<&str>> = Vec::with_capacity(lines.len());
    let mut last: Option<&str> = None;
    for n in &names {
        if let Some(n) = n {
            last = Some(n.as_str());
        }
        prev.push(last);
    }
    let mut next: Vec<Option<&str>> = vec![None; lines.len()];
    let mut following: Option<&str> = None;
    for i in (0..lines.len()).rev() {
        if let Some(n) = &names[i] {
            following = Some(n.as_str());
        }
        next[i] = following;
    }

    let mut add = |function: Option<&str>, kind: &str, n: u32, out: &mut ScanResult| {
        let function = function.unwrap_or(file).to_string();
        *out.narrowing.entry((file.to_string(), function, kind.to_string())).or_insert(0) += n;
    };

    for (i, line) in lines.iter().enumerate() {
        if is_comment(line) {
            continue;
        }
        if harness_markers().iter().any(|m| line.contains(m)) {
            if let Some(name) = next[i] {
                out.harnesses.insert(ScannedHarness {
                    file: file.to_string(),
                    name: name.to_string(),
                });
            }
        }
        for (kind, pat, attach) in narrowing_kinds() {
            let n = line.matches(pat).count();
            if n > 0 {
                let function = if attach == Attach::Next { next[i] } else { prev[i] };
                add(function, kind, n as u32, out);
            }
        }
        let n = admit_re.find_iter(line).count();
        if n > 0 {
            add(prev[i], "admit", n as u32, out);
        }
    }
    Ok(())
}

/// Scan every `*.rs` file under `root`, skipping tool directories and `exclude`.
pub fn scan_root(root: &Path, exclude: &[String]) -> Result<ScanResult> {
    let mut out = ScanResult::default();
    let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
        !(e.file_type().is_dir() && e.file_name().to_str().is_some_and(|n| SKIP_DIRS.contains(&n)))
    });
    for entry in walker {
        let entry = entry.context("walk source tree")?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if exclude.iter().any(|x| *x == rel) {
            continue;
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        scan_text(&rel, &text, &mut out)?;
    }
    Ok(out)
}

/// Render a draft ledger for a scan result.
pub fn render_draft(scan: &ScanResult) -> String {
    let mut s = String::new();
    s.push_str("[scan]\nexclude = []\n");
    for h in &scan.harnesses {
        let _ = write!(
            s,
            "\n[[harness]]\nfile = {:?}\nname = {:?}\nstatus = \"unknown\"\n",
            h.file, h.name
        );
    }
    for ((file, function, kind), count) in &scan.narrowing {
        let _ = write!(
            s,
            "\n[[narrowing]]\nfile = {file:?}\nfunction = {function:?}\nkind = {kind:?}\ncount = {count}\n"
        );
    }
    s
}

/// `[scan]` table of the ledger.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanConfig {
    /// Repo-relative files to skip.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A package whose listed harnesses must verify in CI.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Floor {
    /// Cargo package name.
    pub package: String,
    /// Cargo features used for the run.
    #[serde(default)]
    pub features: Vec<String>,
    /// Harness names as `cargo kani` prints them.
    pub harnesses: Vec<String>,
}

/// A harness row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessRow {
    /// Repo-relative file.
    pub file: String,
    /// Function name.
    pub name: String,
    /// verified | not-compiled | feature-gated | unbounded | wrong-config | unknown
    pub status: String,
    /// What the harness proves (required when verified).
    #[serde(default)]
    pub proves: Option<String>,
    /// Product entry path that reaches the proven code (required when verified).
    #[serde(default)]
    pub entry_path: Option<String>,
    /// Free-form note.
    #[serde(default)]
    pub note: Option<String>,
}

/// A narrowing row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrowingRow {
    /// Repo-relative file.
    pub file: String,
    /// Enclosing or attached function (or the file when none).
    pub function: String,
    /// assume | unwind | stub | external_body | admit
    pub kind: String,
    /// Number of sites.
    pub count: u32,
    /// Free-form note.
    #[serde(default)]
    pub note: Option<String>,
}

/// The whole ledger.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    /// Scan configuration.
    #[serde(default)]
    pub scan: ScanConfig,
    /// Floors.
    #[serde(default)]
    pub floor: Vec<Floor>,
    /// Harness rows.
    #[serde(default)]
    pub harness: Vec<HarnessRow>,
    /// Narrowing rows.
    #[serde(default)]
    pub narrowing: Vec<NarrowingRow>,
}

impl Ledger {
    /// Parse a ledger from TOML text.
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("parse proof ledger")
    }

    /// Load a ledger from a file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read ledger {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("in {}", path.display()))
    }

    fn narrowing_map(&self) -> BTreeMap<NarrowKey, u32> {
        let mut m = BTreeMap::new();
        for r in &self.narrowing {
            *m.entry((r.file.clone(), r.function.clone(), r.kind.clone())).or_insert(0) += r.count;
        }
        m
    }
}

/// Parse `cargo kani` output: harness name -> verified.
pub fn parse_kani_log(text: &str) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Checking harness ") {
            let name = rest.trim().trim_end_matches('.').trim().to_string();
            out.entry(name.clone()).or_insert(false);
            current = Some(name);
        } else if t.contains("VERIFICATION:- SUCCESSFUL") {
            if let Some(name) = &current {
                out.insert(name.clone(), true);
            }
        }
    }
    out
}

/// Outcome of `check`.
#[derive(Debug, Default)]
pub struct CheckReport {
    /// One line per violation.
    pub violations: Vec<String>,
    /// Informational lines (skipped rules).
    pub notes: Vec<String>,
    /// Harnesses scanned.
    pub harnesses: usize,
    /// Narrowing groups scanned.
    pub narrowing_groups: usize,
}

impl CheckReport {
    /// One-line summary.
    pub fn summary(&self) -> String {
        format!(
            "proof-ledger: {} harnesses, {} narrowing groups, {} violation(s)",
            self.harnesses,
            self.narrowing_groups,
            self.violations.len()
        )
    }
}

fn first_component(file: &str) -> &str {
    file.split('/').next().unwrap_or(file)
}

fn suffix_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn nonempty(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.trim().is_empty())
}

/// Check a ledger against a scan, optionally a base ledger and Kani logs.
///
/// `logs` is `None` when no `--kani-log` was given (rule 6 skipped).
pub fn check_ledger(
    ledger: &Ledger,
    scan: &ScanResult,
    base: Option<&Ledger>,
    logs: Option<&BTreeMap<String, BTreeMap<String, bool>>>,
) -> CheckReport {
    let mut r = CheckReport {
        harnesses: scan.harnesses.len(),
        narrowing_groups: scan.narrowing.len(),
        ..CheckReport::default()
    };

    // Rule 1: harness rows vs scan.
    let mut rows: BTreeMap<(String, String), usize> = BTreeMap::new();
    for h in &ledger.harness {
        *rows.entry((h.file.clone(), h.name.clone())).or_insert(0) += 1;
    }
    for h in &scan.harnesses {
        match rows.get(&(h.file.clone(), h.name.clone())) {
            None => r.violations.push(format!(
                "unledgered harness: {} {} has no [[harness]] row",
                h.file, h.name
            )),
            Some(n) if *n > 1 => r.violations.push(format!(
                "duplicate harness rows: {} {} has {n} rows, expected exactly one",
                h.file, h.name
            )),
            Some(_) => {},
        }
    }
    let scanned: BTreeSet<(&str, &str)> =
        scan.harnesses.iter().map(|h| (h.file.as_str(), h.name.as_str())).collect();
    for (file, name) in rows.keys() {
        if !scanned.contains(&(file.as_str(), name.as_str())) {
            r.violations.push(format!(
                "stale harness row: {file} {name} is not in the source, remove it from the ledger"
            ));
        }
    }

    // Rule 2: narrowing rows vs scan.
    let ledger_n = ledger.narrowing_map();
    for (key, count) in &scan.narrowing {
        let (file, function, kind) = key;
        match ledger_n.get(key) {
            None => r.violations.push(format!(
                "unledgered narrowing: {file} {function} {kind} x{count} has no [[narrowing]] row"
            )),
            Some(row) if count > row => r.violations.push(format!(
                "unledgered narrowing: {file} {function} {kind} has {count} sites, ledger says {row}"
            )),
            Some(row) if count < row => r.violations.push(format!(
                "ledger is stale: {file} {function} {kind} has {count} sites, ledger says {row}, lower the count"
            )),
            Some(_) => {},
        }
    }
    for (key, row) in &ledger_n {
        if !scan.narrowing.contains_key(key) {
            let (file, function, kind) = key;
            r.violations.push(format!(
                "ledger is stale: {file} {function} {kind} x{row} has no site in the source, remove it"
            ));
        }
    }

    // Rule 3: status requirements.
    for h in &ledger.harness {
        if h.status == "verified" && !(nonempty(&h.proves) && nonempty(&h.entry_path)) {
            r.violations.push(format!(
                "incomplete verified row: {} {} needs non-empty proves and entry_path",
                h.file, h.name
            ));
        }
        if h.status == "unknown" {
            r.violations.push(format!(
                "unknown status: {} {} must be classified",
                h.file, h.name
            ));
        }
    }

    // Rule 4: verified iff listed in the matching floor.
    let mut floored: BTreeSet<(String, String)> = BTreeSet::new();
    for f in &ledger.floor {
        for n in &f.harnesses {
            floored.insert((f.package.clone(), suffix_name(n).to_string()));
        }
    }
    for h in &ledger.harness {
        let in_floor = floored.contains(&(first_component(&h.file).to_string(), h.name.clone()));
        let verified = h.status == "verified";
        if verified && !in_floor {
            r.violations.push(format!(
                "verified without floor: {} {} is verified but no floor of package {} lists it",
                h.file,
                h.name,
                first_component(&h.file)
            ));
        } else if !verified && in_floor {
            r.violations.push(format!(
                "floor without verified: {} {} is in a floor but status is {}",
                h.file, h.name, h.status
            ));
        }
    }
    let row_keys: BTreeSet<(String, String)> = ledger
        .harness
        .iter()
        .map(|h| (first_component(&h.file).to_string(), h.name.clone()))
        .collect();
    for (pkg, name) in &floored {
        if !row_keys.contains(&(pkg.clone(), name.clone())) {
            r.violations.push(format!(
                "floor harness without row: package {pkg} harness {name} has no [[harness]] row"
            ));
        }
    }

    // Rule 5: the ledger may only shrink.
    if let Some(base) = base {
        let base_n = base.narrowing_map();
        let total: u32 = ledger_n.values().sum();
        let base_total: u32 = base_n.values().sum();
        if total > base_total {
            r.violations.push(format!(
                "ledger grew: total narrowing count {total} exceeds base {base_total}"
            ));
        }
        for (key, count) in &ledger_n {
            let was = base_n.get(key).copied().unwrap_or(0);
            if *count > was {
                r.violations.push(format!(
                    "ledger grew: {} {} {} is {count}, base has {was}",
                    key.0, key.1, key.2
                ));
            }
        }
    }

    // Rule 6: floors verified in the supplied Kani logs.
    if let Some(logs) = logs {
        for f in &ledger.floor {
            let Some(log) = logs.get(&f.package) else {
                r.violations.push(format!(
                    "missing kani log: package {} has a floor but no --kani-log was given",
                    f.package
                ));
                continue;
            };
            for n in &f.harnesses {
                match log.get(n) {
                    Some(true) => {},
                    Some(false) => r.violations.push(format!(
                        "floor harness failed: package {} harness {n} did not report VERIFICATION SUCCESSFUL",
                        f.package
                    )),
                    None => r.violations.push(format!(
                        "floor harness missing: package {} harness {n} was not checked in the log",
                        f.package
                    )),
                }
            }
        }
    }
    r
}

/// Run `scan` and return the draft ledger text.
pub fn run_scan(root: &Path) -> Result<String> {
    Ok(render_draft(&scan_root(root, &[])?))
}

/// Run `check` end to end.
pub fn run_check(
    root: &Path,
    ledger_path: &Path,
    base_path: Option<&Path>,
    kani_logs: &[(String, std::path::PathBuf)],
) -> Result<CheckReport> {
    let ledger = Ledger::load(ledger_path)?;
    let scan = scan_root(root, &ledger.scan.exclude)?;

    let mut notes = Vec::new();
    let base = match base_path {
        Some(p) if p.exists() => Some(Ledger::load(p)?),
        Some(p) => {
            notes.push(format!(
                "base ledger {} does not exist, skipping growth check",
                p.display()
            ));
            None
        },
        None => {
            notes.push("no --base given, skipping growth check".to_string());
            None
        },
    };

    let logs = if kani_logs.is_empty() {
        notes.push("no --kani-log given, skipping floor verification".to_string());
        None
    } else {
        let mut m = BTreeMap::new();
        for (pkg, path) in kani_logs {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read kani log {}", path.display()))?;
            m.insert(pkg.clone(), parse_kani_log(&text));
        }
        Some(m)
    };

    let mut report = check_ledger(&ledger, &scan, base.as_ref(), logs.as_ref());
    report.notes = notes;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> String {
        concat!("#[kani", "::proof]").to_string()
    }
    fn assume() -> String {
        concat!("kani", "::assume(").to_string()
    }
    fn unwind() -> String {
        concat!("#[kani", "::unwind(").to_string()
    }

    fn scan_one(file: &str, text: &str) -> ScanResult {
        let mut out = ScanResult::default();
        scan_text(file, text, &mut out).expect("scan");
        out
    }

    fn key(file: &str, f: &str, k: &str) -> NarrowKey {
        (file.to_string(), f.to_string(), k.to_string())
    }

    #[test]
    fn harness_marker_forms() {
        let text = format!(
            "{}\nfn plain() {{}}\n#[cfg_attr(kani, {})]\nfn gated() {{}}\n#[kani{}proof_for_contract(f)]\nfn contract() {{}}\n",
            proof(),
            concat!("kani", "::proof"),
            "::"
        );
        let r = scan_one("a/src/x.rs", &text);
        let names: Vec<&str> = r.harnesses.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, vec!["contract", "gated", "plain"]);
    }

    #[test]
    fn attribute_goes_to_next_fn_call_to_preceding_fn() {
        let text = format!(
            "fn helper() {{\n    {}x);\n}}\n{}\n{}(3)]\nfn harness() {{\n    {}y);\n    {}z);\n}}\n",
            assume(),
            proof(),
            unwind(),
            assume(),
            assume()
        );
        let r = scan_one("f.rs", &text);
        assert_eq!(r.narrowing[&key("f.rs", "helper", "assume")], 1);
        assert_eq!(r.narrowing[&key("f.rs", "harness", "unwind")], 1);
        assert_eq!(r.narrowing[&key("f.rs", "harness", "assume")], 2);
    }

    #[test]
    fn no_fn_falls_back_to_file_name() {
        let r = scan_one("f.rs", &format!("{}x);\n", assume()));
        assert_eq!(r.narrowing[&key("f.rs", "f.rs", "assume")], 1);
    }

    #[test]
    fn comment_lines_skipped() {
        let text = format!("// {}\n/// {}x);\nfn f() {{}}\n", proof(), assume());
        let r = scan_one("f.rs", &text);
        assert!(r.harnesses.is_empty());
        assert!(r.narrowing.is_empty());
    }

    #[test]
    fn readmit_not_counted_admit_is() {
        let text = format!(
            "fn f() {{\n    {}(1);\n    {}();\n}}\n",
            concat!("re", "ad", "mit"),
            concat!("ad", "mit")
        );
        let r = scan_one("f.rs", &text);
        assert_eq!(r.narrowing[&key("f.rs", "f", "admit")], 1);
    }

    #[test]
    fn stub_and_opaque_attach_forward() {
        let eb = concat!("#[verifier::external", "_body]");
        let stub = concat!("#[kani", "::stub(a, b)]");
        let text = format!("{eb}\nfn opaque() {{}}\n{stub}\nfn stubbed() {{}}\n");
        let r = scan_one("f.rs", &text);
        assert_eq!(r.narrowing[&key("f.rs", "opaque", OPAQUE_KIND)], 1);
        assert_eq!(r.narrowing[&key("f.rs", "stubbed", "stub")], 1);
    }

    #[test]
    fn draft_is_a_valid_ledger() {
        let text = format!("{}\nfn h() {{\n    {}x);\n}}\n", proof(), assume());
        let draft = render_draft(&scan_one("p/f.rs", &text));
        let ledger = Ledger::parse(&draft).expect("draft parses");
        assert_eq!(ledger.harness.len(), 1);
        assert_eq!(ledger.harness[0].status, "unknown");
        assert_eq!(ledger.narrowing[0].count, 1);
    }

    #[test]
    fn unknown_keys_rejected() {
        assert!(Ledger::parse("[scan]\nexclude = []\nbogus = 1\n").is_err());
    }

    // --- check rules ---

    const GOOD: &str = r#"
[[floor]]
package = "p"
harnesses = ["m::h"]

[[harness]]
file = "p/src/f.rs"
name = "h"
status = "verified"
proves = "X"
entry_path = "none"

[[narrowing]]
file = "p/src/f.rs"
function = "h"
kind = "assume"
count = 1
"#;

    fn good_scan() -> ScanResult {
        let text = format!("{}\nfn h() {{\n    {}x);\n}}\n", proof(), assume());
        scan_one("p/src/f.rs", &text)
    }

    fn violations(ledger: &str, scan: &ScanResult) -> Vec<String> {
        check_ledger(&Ledger::parse(ledger).expect("ledger"), scan, None, None).violations
    }

    #[test]
    fn good_ledger_passes() {
        assert!(violations(GOOD, &good_scan()).is_empty());
    }

    #[test]
    fn rule1_unledgered_and_stale_harness() {
        let v = violations("[scan]\n", &good_scan());
        assert!(v.iter().any(|l| l.contains("unledgered harness") && l.contains(" h ")));
        let ledger = format!(
            "{GOOD}\n[[harness]]\nfile = \"p/src/f.rs\"\nname = \"gone\"\nstatus = \"unbounded\"\n"
        );
        let v = violations(&ledger, &good_scan());
        assert!(v.iter().any(|l| l.contains("stale harness row") && l.contains("gone")));
    }

    #[test]
    fn rule2_count_mismatch_both_ways() {
        let lower = GOOD.replace("count = 1", "count = 0");
        let v = violations(&lower, &good_scan());
        assert!(v.iter().any(|l| l.contains("unledgered narrowing")));
        let higher = GOOD.replace("count = 1", "count = 3");
        let v = violations(&higher, &good_scan());
        assert!(v.iter().any(|l| l.contains("lower the count")));
        let mut scan = good_scan();
        scan.narrowing.clear();
        let v = violations(GOOD, &scan);
        assert!(v.iter().any(|l| l.contains("ledger is stale") && l.contains("no site")));
    }

    #[test]
    fn rule3_verified_needs_proves_and_unknown_fails() {
        let v = violations(
            &GOOD.replace("proves = \"X\"", "proves = \"\""),
            &good_scan(),
        );
        assert!(v.iter().any(|l| l.contains("incomplete verified row")));
        let v = violations(
            &GOOD.replace("status = \"verified\"", "status = \"unknown\""),
            &good_scan(),
        );
        assert!(v.iter().any(|l| l.contains("unknown status")));
    }

    #[test]
    fn rule4_verified_iff_floor() {
        let no_floor = GOOD.replace("harnesses = [\"m::h\"]", "harnesses = []");
        let v = violations(&no_floor, &good_scan());
        assert!(v.iter().any(|l| l.contains("verified without floor")));
        let v = violations(
            &GOOD.replace("status = \"verified\"", "status = \"unbounded\""),
            &good_scan(),
        );
        assert!(v.iter().any(|l| l.contains("floor without verified")));
        let ghost = GOOD.replace("m::h", "m::ghost");
        let v = violations(&ghost, &good_scan());
        assert!(v.iter().any(|l| l.contains("floor harness without row")));
    }

    #[test]
    fn rule5_base_growth_fails_shrink_passes() {
        let ledger = Ledger::parse(GOOD).expect("ledger");
        let scan = good_scan();
        let grown_base = Ledger::parse(&GOOD.replace("count = 1", "count = 0")).expect("base");
        let r = check_ledger(&ledger, &scan, Some(&grown_base), None);
        assert!(r.violations.iter().any(|l| l.contains("ledger grew")));
        let bigger_base = Ledger::parse(&GOOD.replace("count = 1", "count = 4")).expect("base");
        let r = check_ledger(&ledger, &scan, Some(&bigger_base), None);
        assert!(!r.violations.iter().any(|l| l.contains("ledger grew")));
    }

    #[test]
    fn kani_log_parse_successful_and_failed() {
        let log = "Checking harness m::h...\nVERIFICATION:- SUCCESSFUL\n\
                   Checking harness m::bad...\nVERIFICATION:- FAILED\n\
                   Checking harness m::none...\n";
        let m = parse_kani_log(log);
        assert_eq!(m["m::h"], true);
        assert_eq!(m["m::bad"], false);
        assert_eq!(m["m::none"], false);
    }

    #[test]
    fn rule6_floor_requires_verified_log() {
        let ledger = Ledger::parse(GOOD).expect("ledger");
        let scan = good_scan();
        let mut logs = BTreeMap::new();
        logs.insert(
            "p".to_string(),
            parse_kani_log("Checking harness m::h...\nVERIFICATION:- SUCCESSFUL\n"),
        );
        let r = check_ledger(&ledger, &scan, None, Some(&logs));
        assert!(r.violations.is_empty(), "{:?}", r.violations);

        logs.insert(
            "p".to_string(),
            parse_kani_log("Checking harness m::h...\nVERIFICATION:- FAILED\n"),
        );
        let r = check_ledger(&ledger, &scan, None, Some(&logs));
        assert!(r.violations.iter().any(|l| l.contains("floor harness failed")));

        logs.insert("p".to_string(), parse_kani_log(""));
        let r = check_ledger(&ledger, &scan, None, Some(&logs));
        assert!(r.violations.iter().any(|l| l.contains("floor harness missing")));

        let r = check_ledger(&ledger, &scan, None, Some(&BTreeMap::new()));
        assert!(r.violations.iter().any(|l| l.contains("missing kani log")));
    }

    #[test]
    fn scan_root_skips_dirs_and_excludes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = format!("{}\nfn h() {{}}\n", proof());
        for p in ["a/src", "a/target", "a/external", "a/skipme"] {
            std::fs::create_dir_all(dir.path().join(p)).expect("mkdir");
        }
        for p in [
            "a/src/k.rs",
            "a/target/t.rs",
            "a/external/e.rs",
            "a/skipme/s.rs",
        ] {
            std::fs::write(dir.path().join(p), &body).expect("write");
        }
        let r = scan_root(dir.path(), &["a/skipme/s.rs".to_string()]).expect("scan");
        let files: Vec<&str> = r.harnesses.iter().map(|h| h.file.as_str()).collect();
        assert_eq!(files, vec!["a/src/k.rs"]);
    }
}
