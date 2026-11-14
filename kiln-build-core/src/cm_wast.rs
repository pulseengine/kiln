//! Component Model WAST on meld-fused output (SR-93).
//!
//! The upstream Component Model reference tests are WASTs whose commands name
//! component-level exports. Kiln's interpreter executes core modules, so the
//! components are first fused by meld (offline, see `tests/component-model`)
//! and this runner executes each WAST's commands against the fused core module
//! of the component the command targets.
//!
//! The vendored layout is:
//!
//! * `<dir>/upstream/test/**.wast`: the upstream tests;
//! * `<dir>/meld-<version>/manifest.json`: meld's outcome for every component
//!   and the command-to-component mapping, plus
//!   `<dir>/meld-<version>/test/**.fused.wasm`.
//!
//! The runner parses each WAST itself and matches its directives to the
//! manifest by directive index; a directive whose line or kind disagrees with
//! the manifest is a loud per-file failure, never a skip.
//!
//! Every file gets one [`Verdict`]:
//!
//! * [`Verdict::Pass`]: every check passed.
//! * [`Verdict::RefusedAtFuse`]: meld refused at least one component and kiln
//!   failed nothing. A refusal is meld's, not a kiln failure.
//! * [`Verdict::Fail`]: kiln failed at least one check, including values the
//!   runner cannot lower to core values (a failure, never a skip).

#![cfg(feature = "std")]

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use kiln_foundation::values::Value;
use serde::Deserialize;
use wast::{
    QuoteWat, Wast, WastArg, WastDirective, WastExecute, WastRet, Wat,
    component::WastVal,
    parser::{self, ParseBuffer},
};

use crate::wast_execution::WastEngine;

/// Reason shared by every command whose values need canonical-ABI lowering.
const LOWERING: &str = "unsupported: component value lowering";

// ---------------------------------------------------------------------------
// Manifest (written by meld's `cm_wast_ratchet` test)
// ---------------------------------------------------------------------------

/// What kind of top-level component directive a manifest entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ComponentKind {
    /// `(component ...)`: defined and instantiated.
    #[serde(rename = "component")]
    Component,
    /// `(component definition ...)`: defined only.
    #[serde(rename = "component-definition")]
    Definition,
    /// `assert_invalid` on a component.
    #[serde(rename = "assert_invalid")]
    AssertInvalid,
    /// `assert_malformed` on a component.
    #[serde(rename = "assert_malformed")]
    AssertMalformed,
    /// `assert_unlinkable` on a component.
    #[serde(rename = "assert_unlinkable")]
    AssertUnlinkable,
    /// A component instantiated inside `assert_trap`.
    #[serde(rename = "assert_trap-instantiate")]
    AssertTrapInstantiate,
}

/// What meld did with one component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Outcome {
    /// Fused and the output validates.
    #[serde(rename = "fused")]
    Fused,
    /// Meld refused the component.
    #[serde(rename = "refused")]
    Refused,
    /// Meld built output that failed its own validation and refused to return it.
    #[serde(rename = "meld-invalid")]
    MeldInvalid,
    /// The text never became a binary.
    #[serde(rename = "unencodable")]
    Unencodable,
    /// Meld panicked.
    #[serde(rename = "panic")]
    Panic,
    /// Meld returned output that fails validation.
    #[serde(rename = "fused-invalid")]
    FusedInvalid,
    /// An invalid component was refused, as the assertion expects.
    #[serde(rename = "invalid-refused")]
    InvalidRefused,
    /// An invalid component's text never became a binary.
    #[serde(rename = "invalid-unencodable")]
    InvalidUnencodable,
    /// Meld accepted an invalid component.
    #[serde(rename = "invalid-fused")]
    InvalidFused,
}

/// The kind of a manifest command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum CommandKind {
    /// `(component instance ...)`: a fresh instance.
    #[serde(rename = "instance")]
    Instance,
    /// A bare `invoke`.
    #[serde(rename = "invoke")]
    Invoke,
    /// `assert_return`, `assert_trap`, ... on an invoke.
    #[serde(rename = "assert")]
    Assert,
    /// `assert_trap` around an inline component.
    #[serde(rename = "assert-instantiate")]
    AssertInstantiate,
    /// `register`.
    #[serde(rename = "register")]
    Register,
}

/// One component entry in the manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestComponent {
    /// Position among the file's component entries.
    pub component_index: usize,
    /// Index of the WAST directive that defines the component.
    pub directive_index: usize,
    /// 1-based line of that directive.
    pub line: usize,
    /// Directive kind.
    pub kind: ComponentKind,
    /// Meld's outcome.
    pub outcome: Outcome,
    /// Meld's detail for a refusal.
    pub detail: String,
    /// Fused module path relative to the manifest directory, when fused.
    pub fused_path: Option<String>,
}

/// One command entry in the manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestCommand {
    /// Index of the WAST directive.
    pub index: usize,
    /// 1-based line of the directive.
    pub line: usize,
    /// Command kind.
    pub kind: CommandKind,
    /// The component entry the command targets.
    pub target_component: Option<usize>,
}

/// One WAST file's manifest entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    /// Path of the WAST, relative to `upstream/` (`test/...`).
    pub wast: String,
    /// Component entries in directive order.
    pub components: Vec<ManifestComponent>,
    /// Commands in directive order.
    pub commands: Vec<ManifestCommand>,
}

/// meld's `manifest.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    /// Upstream component-model commit the WASTs are from.
    pub component_model_commit: String,
    /// The meld version that produced the fused modules.
    pub meld_version: String,
    /// One entry per WAST file.
    pub files: Vec<ManifestFile>,
}

// ---------------------------------------------------------------------------
// Parsed WAST
// ---------------------------------------------------------------------------

/// A value kiln can pass to, or compare against, a core function directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scalar {
    /// `bool`, `s8`..`s32`, `u8`..`u32`, `char`.
    I32(i32),
    /// `s64`, `u64`.
    I64(i64),
    /// `f32` bits.
    F32(u32),
    /// `f64` bits.
    F64(u64),
}

fn scalar_of(v: &WastVal<'_>) -> Option<Scalar> {
    Some(match v {
        WastVal::Bool(b) => Scalar::I32(i32::from(*b)),
        WastVal::U8(x) => Scalar::I32(i32::from(*x)),
        WastVal::S8(x) => Scalar::I32(i32::from(*x)),
        WastVal::U16(x) => Scalar::I32(i32::from(*x)),
        WastVal::S16(x) => Scalar::I32(i32::from(*x)),
        WastVal::U32(x) => Scalar::I32(*x as i32),
        WastVal::S32(x) => Scalar::I32(*x),
        WastVal::U64(x) => Scalar::I64(*x as i64),
        WastVal::S64(x) => Scalar::I64(*x),
        WastVal::F32(x) => Scalar::F32(x.bits),
        WastVal::F64(x) => Scalar::F64(x.bits),
        WastVal::Char(c) => Scalar::I32(*c as i32),
        _ => return None,
    })
}

/// What a command expects.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// `assert_return`: the expected results, or `None` if not all scalar.
    Return(Option<Vec<Scalar>>),
    /// `assert_trap`.
    Trap,
    /// A bare `invoke`, run for its effect on state.
    Invoke,
}

/// An executable command.
#[derive(Debug, Clone, PartialEq)]
pub struct Exec {
    /// Export name.
    pub export: String,
    /// Arguments, or `None` if not all scalar.
    pub args: Option<Vec<Scalar>>,
    /// Expectation.
    pub expect: Expect,
}

/// A top-level component directive found in the WAST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedComponent {
    /// Directive index.
    pub directive: usize,
    /// 1-based line.
    pub line: usize,
    /// Directive kind.
    pub kind: ComponentKind,
}

/// A command directive found in the WAST.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCommand {
    /// Directive index.
    pub directive: usize,
    /// 1-based line.
    pub line: usize,
    /// Command kind.
    pub kind: CommandKind,
    /// What to execute, if the form is supported.
    pub exec: Option<Exec>,
}

/// The directives of one WAST that matter to the manifest.
#[derive(Debug, Clone, Default)]
pub struct ParsedWast {
    /// Component directives in order.
    pub components: Vec<ParsedComponent>,
    /// Commands in order.
    pub commands: Vec<ParsedCommand>,
}

fn is_component(q: &QuoteWat<'_>) -> bool {
    matches!(
        q,
        QuoteWat::Wat(Wat::Component(_)) | QuoteWat::QuoteComponent(..)
    )
}

fn scalars_of_args(args: &[WastArg<'_>]) -> Option<Vec<Scalar>> {
    args.iter()
        .map(|a| match a {
            WastArg::Component(v) => scalar_of(v),
            _ => None,
        })
        .collect()
}

fn scalars_of_rets(rets: &[WastRet<'_>]) -> Option<Vec<Scalar>> {
    rets.iter()
        .map(|r| match r {
            WastRet::Component(v) => scalar_of(v),
            _ => None,
        })
        .collect()
}

/// Parse a WAST and list its component directives and commands, using the same
/// selection rules as meld's `cm_wast_ratchet` so directive indices agree.
pub fn parse_wast(text: &str) -> Result<ParsedWast> {
    let buf = ParseBuffer::new(text).map_err(|e| anyhow::anyhow!("{e}"))?;
    let wast: Wast<'_> = parser::parse(&buf).map_err(|e| anyhow::anyhow!("{e}"))?;
    let line_of = |span: wast::token::Span| span.linecol_in(text).0 + 1;
    let mut out = ParsedWast::default();

    for (directive, d) in wast.directives.into_iter().enumerate() {
        let line = line_of(d.span());
        let mut component = |out: &mut ParsedWast, kind| {
            out.components.push(ParsedComponent {
                directive,
                line,
                kind,
            });
        };
        match d {
            WastDirective::Module(q) if is_component(&q) => {
                component(&mut out, ComponentKind::Component)
            },
            WastDirective::ModuleDefinition(q) if is_component(&q) => {
                component(&mut out, ComponentKind::Definition)
            },
            WastDirective::AssertInvalid { module, .. } if is_component(&module) => {
                component(&mut out, ComponentKind::AssertInvalid)
            },
            WastDirective::AssertMalformed { module, .. } if is_component(&module) => {
                component(&mut out, ComponentKind::AssertMalformed)
            },
            WastDirective::AssertUnlinkable { module, .. }
                if matches!(module, Wat::Component(_)) =>
            {
                component(&mut out, ComponentKind::AssertUnlinkable)
            },
            WastDirective::ModuleInstance { .. } => out.commands.push(ParsedCommand {
                directive,
                line,
                kind: CommandKind::Instance,
                exec: None,
            }),
            WastDirective::Register { .. } => out.commands.push(ParsedCommand {
                directive,
                line,
                kind: CommandKind::Register,
                exec: None,
            }),
            WastDirective::Invoke(inv) => out.commands.push(ParsedCommand {
                directive,
                line,
                kind: CommandKind::Invoke,
                exec: Some(Exec {
                    export: inv.name.to_string(),
                    args: scalars_of_args(&inv.args),
                    expect: Expect::Invoke,
                }),
            }),
            WastDirective::AssertReturn { exec, results, .. } => {
                let expect = Expect::Return(scalars_of_rets(&results));
                push_assert(&mut out, directive, line, exec, Some(expect));
            },
            WastDirective::AssertTrap { exec, .. } => {
                push_assert(&mut out, directive, line, exec, Some(Expect::Trap));
            },
            WastDirective::AssertException { exec, .. }
            | WastDirective::AssertSuspension { exec, .. } => {
                push_assert(&mut out, directive, line, exec, None);
            },
            WastDirective::AssertExhaustion { .. } => out.commands.push(ParsedCommand {
                directive,
                line,
                kind: CommandKind::Assert,
                exec: None,
            }),
            // Core modules, threads and custom-section assertions are not
            // component inputs for meld.
            _ => {},
        }
    }
    Ok(out)
}

/// Record an `assert_*` directive: an invoke is a command, an inline component
/// is a component plus an `assert-instantiate` command, anything else is not
/// in the manifest.
fn push_assert(
    out: &mut ParsedWast,
    directive: usize,
    line: usize,
    exec: WastExecute<'_>,
    expect: Option<Expect>,
) {
    match exec {
        WastExecute::Invoke(inv) => out.commands.push(ParsedCommand {
            directive,
            line,
            kind: CommandKind::Assert,
            exec: expect.map(|expect| Exec {
                export: inv.name.to_string(),
                args: scalars_of_args(&inv.args),
                expect,
            }),
        }),
        WastExecute::Wat(Wat::Component(_)) => {
            out.components.push(ParsedComponent {
                directive,
                line,
                kind: ComponentKind::AssertTrapInstantiate,
            });
            out.commands.push(ParsedCommand {
                directive,
                line,
                kind: CommandKind::AssertInstantiate,
                exec: None,
            });
        },
        _ => {},
    }
}

/// Check that the parsed WAST and the manifest entry describe the same
/// directives: same count, directive index, line and kind, in order.
pub fn match_manifest(parsed: &ParsedWast, mf: &ManifestFile) -> Result<(), String> {
    if parsed.components.len() != mf.components.len() {
        return Err(format!(
            "manifest lists {} components, the WAST has {}",
            mf.components.len(),
            parsed.components.len()
        ));
    }
    for (i, (p, m)) in parsed.components.iter().zip(&mf.components).enumerate() {
        if m.component_index != i {
            return Err(format!(
                "component entry {i} has component_index {}",
                m.component_index
            ));
        }
        if p.directive != m.directive_index || p.line != m.line || p.kind != m.kind {
            return Err(format!(
                "component {i}: WAST has {:?} at directive {} line {}, manifest has {:?} at directive {} line {}",
                p.kind, p.directive, p.line, m.kind, m.directive_index, m.line
            ));
        }
    }
    if parsed.commands.len() != mf.commands.len() {
        return Err(format!(
            "manifest lists {} commands, the WAST has {}",
            mf.commands.len(),
            parsed.commands.len()
        ));
    }
    for (i, (p, m)) in parsed.commands.iter().zip(&mf.commands).enumerate() {
        if p.directive != m.index || p.line != m.line || p.kind != m.kind {
            return Err(format!(
                "command {i}: WAST has {:?} at directive {} line {}, manifest has {:?} at directive {} line {}",
                p.kind, p.directive, p.line, m.kind, m.index, m.line
            ));
        }
        if let Some(t) = m.target_component {
            if t >= mf.components.len() {
                return Err(format!("command {i} targets missing component {t}"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Verdicts and reports
// ---------------------------------------------------------------------------

/// A file's verdict, ordered from best to worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Every check passed.
    Pass,
    /// Meld refused a component and kiln failed nothing.
    RefusedAtFuse,
    /// Kiln failed at least one check.
    Fail,
}

impl Verdict {
    /// The name used in reports and the baseline.
    pub fn name(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::RefusedAtFuse => "refused-at-fuse",
            Verdict::Fail => "fail",
        }
    }
}

/// Why a check failed. The label is the normalised reason that failure counts
/// are grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FailKind {
    /// Arguments or results need canonical-ABI lowering.
    ValueLowering,
    /// The fused module imports host functions nothing provides.
    UnresolvedImports,
    /// The fused module does not export the invoked name as a function.
    ExportNotFound,
    /// The fused module does not decode, validate or instantiate in kiln.
    LoadFailed,
    /// The call returned something other than the expected values.
    WrongResult,
    /// The call returned although a trap was expected.
    NoTrap,
    /// The call failed with an error that is not a runtime trap.
    NotATrap,
    /// The call failed although it was expected to return.
    ExecutionError,
    /// An assertion form the runner does not execute.
    UnsupportedAssertion,
    /// No live instance of the targeted component.
    NoLiveInstance,
    /// The WAST and manifest disagree.
    ManifestMismatch,
    /// Meld accepted a component an assertion says is invalid.
    MeldAcceptedInvalid,
    /// An outcome that is impossible for this directive kind.
    UnexpectedOutcome,
}

impl FailKind {
    /// The normalised reason.
    pub fn label(self) -> &'static str {
        match self {
            FailKind::ValueLowering => LOWERING,
            FailKind::UnresolvedImports => "unsupported: unresolved host imports",
            FailKind::ExportNotFound => "export not in the fused module",
            FailKind::LoadFailed => "fused module failed to load in kiln",
            FailKind::WrongResult => "wrong result",
            FailKind::NoTrap => "expected a trap, the call returned",
            FailKind::NotATrap => "expected a trap, got a non-trap error",
            FailKind::ExecutionError => "execution error",
            FailKind::UnsupportedAssertion => "unsupported: assertion form",
            FailKind::NoLiveInstance => "no live instance of the target component",
            FailKind::ManifestMismatch => "WAST and manifest disagree",
            FailKind::MeldAcceptedInvalid => "meld fused an invalid component",
            FailKind::UnexpectedOutcome => "unexpected meld outcome for this directive",
        }
    }
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// Passed.
    Pass,
    /// Meld refused the targeted component.
    Refused(String),
    /// Kiln failed.
    Fail(FailKind, String),
}

/// One executed check with its source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// 1-based WAST line.
    pub line: usize,
    /// Result.
    pub result: CheckResult,
}

/// Everything known about one file after a run.
#[derive(Debug, Clone)]
pub struct FileReport {
    /// Path of the WAST (`test/...`).
    pub wast: String,
    /// All checks in execution order.
    pub checks: Vec<Check>,
}

impl FileReport {
    /// The file's verdict.
    pub fn verdict(&self) -> Verdict {
        verdict_of(&self.checks)
    }

    /// Count of (pass, refused, fail) checks.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for check in &self.checks {
            match check.result {
                CheckResult::Pass => c.0 += 1,
                CheckResult::Refused(_) => c.1 += 1,
                CheckResult::Fail(..) => c.2 += 1,
            }
        }
        c
    }

    /// The first failure, or for a refused-at-fuse file the first refusal.
    pub fn first_reason(&self) -> Option<String> {
        self.checks
            .iter()
            .find_map(|c| match &c.result {
                CheckResult::Fail(k, d) => Some(format!("line {}: {}: {d}", c.line, k.label())),
                _ => None,
            })
            .or_else(|| {
                self.checks.iter().find_map(|c| match &c.result {
                    CheckResult::Refused(d) => {
                        Some(format!("line {}: refused at fuse: {d}", c.line))
                    },
                    _ => None,
                })
            })
    }
}

/// A file's verdict from its checks: any failure fails the file, otherwise any
/// refusal makes it refused-at-fuse.
pub fn verdict_of(checks: &[Check]) -> Verdict {
    if checks.iter().any(|c| matches!(c.result, CheckResult::Fail(..))) {
        Verdict::Fail
    } else if checks.iter().any(|c| matches!(c.result, CheckResult::Refused(_))) {
        Verdict::RefusedAtFuse
    } else {
        Verdict::Pass
    }
}

/// The result of a whole run.
#[derive(Debug, Clone)]
pub struct CmWastReport {
    /// Upstream commit.
    pub component_model_commit: String,
    /// meld version.
    pub meld_version: String,
    /// Per-file reports, sorted by path.
    pub files: Vec<FileReport>,
}

impl CmWastReport {
    /// Render the markdown report. The file table's first two columns (path,
    /// verdict) are what `scripts/cm-wast-gate.sh` reads.
    pub fn to_markdown(&self) -> String {
        let mut by_verdict = [0usize; 3];
        let (mut pass, mut refused, mut fail) = (0usize, 0usize, 0usize);
        let mut reasons: BTreeMap<&'static str, usize> = BTreeMap::new();
        for f in &self.files {
            by_verdict[f.verdict() as usize] += 1;
            let (p, r, x) = f.counts();
            pass += p;
            refused += r;
            fail += x;
            for c in &f.checks {
                if let CheckResult::Fail(k, _) = &c.result {
                    *reasons.entry(k.label()).or_insert(0) += 1;
                }
            }
        }
        let mut out = String::new();
        out.push_str("# Component Model WAST on meld-fused output\n\n");
        out.push_str(&format!(
            "- component-model commit: `{}`\n- meld: `{}`\n\n",
            self.component_model_commit, self.meld_version
        ));
        out.push_str("## Totals\n\n");
        out.push_str(&format!(
            "- Files: {} total, {} pass, {} refused-at-fuse, {} fail\n",
            self.files.len(),
            by_verdict[Verdict::Pass as usize],
            by_verdict[Verdict::RefusedAtFuse as usize],
            by_verdict[Verdict::Fail as usize]
        ));
        out.push_str(&format!(
            "- Checks: {} pass, {} refused-at-fuse, {} fail\n\n",
            pass, refused, fail
        ));
        out.push_str("## Failure reasons\n\n");
        let mut sorted: Vec<_> = reasons.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        if sorted.is_empty() {
            out.push_str("None.\n\n");
        }
        for (label, n) in sorted {
            out.push_str(&format!("- {n}: {label}\n"));
        }
        out.push_str("\n## File Results\n\n");
        out.push_str("| File | Verdict | Pass | Refused | Fail | First failure or refusal |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for f in &self.files {
            let (p, r, x) = f.counts();
            let reason =
                f.first_reason().unwrap_or_default().replace('|', "\\|").replace('\n', " ");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                f.wast,
                f.verdict().name(),
                p,
                r,
                x,
                reason
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

fn fail(kind: FailKind, detail: impl Into<String>) -> CheckResult {
    CheckResult::Fail(kind, detail.into())
}

/// Is this a WebAssembly trap? Kiln raises traps as `runtime_trap` (Runtime
/// category, `RUNTIME_TRAP_ERROR`) or in the `RuntimeTrap` category. Decode,
/// validation, link and not-implemented errors are never traps.
fn is_runtime_trap(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<kiln_error::Error>().is_some_and(|k| {
            k.category == kiln_error::ErrorCategory::RuntimeTrap
                || k.code == kiln_error::codes::RUNTIME_TRAP_ERROR
        })
    })
}

fn to_value(s: Scalar) -> Value {
    match s {
        Scalar::I32(x) => Value::I32(x),
        Scalar::I64(x) => Value::I64(x),
        Scalar::F32(b) => Value::F32(kiln_foundation::values::FloatBits32::from_bits(b)),
        Scalar::F64(b) => Value::F64(kiln_foundation::values::FloatBits64::from_bits(b)),
    }
}

/// Does `got` equal the expected scalar? NaN expectations match any NaN of the
/// same width, since the component model canonicalises NaNs.
fn matches_scalar(got: &Value, want: Scalar) -> bool {
    match (got, want) {
        (Value::I32(a), Scalar::I32(b)) => *a == b,
        (Value::I64(a), Scalar::I64(b)) => *a == b,
        (Value::F32(a), Scalar::F32(b)) => {
            a.to_bits() == b || (f32::from_bits(b).is_nan() && a.value().is_nan())
        },
        (Value::F64(a), Scalar::F64(b)) => {
            a.to_bits() == b || (f64::from_bits(b).is_nan() && a.value().is_nan())
        },
        _ => false,
    }
}

struct FileRun<'a> {
    engine: Box<WastEngine>,
    mf: &'a ManifestFile,
    meld_dir: &'a Path,
    /// Live instance name per component entry.
    live: HashMap<usize, String>,
    /// The failure of each component's latest instantiation, if it failed.
    failed: HashMap<usize, (FailKind, String)>,
    instances: usize,
}

impl FileRun<'_> {
    /// Classify what meld did with a component: `Ok(path)` to a fused module,
    /// `Err(result)` for the check result when there is nothing to run.
    fn fused_module(&self, comp: &ManifestComponent) -> Result<Vec<u8>, CheckResult> {
        match comp.outcome {
            Outcome::Fused => {
                let Some(rel) = &comp.fused_path else {
                    return Err(fail(
                        FailKind::ManifestMismatch,
                        "manifest marks the component fused but has no fused_path",
                    ));
                };
                fs::read(self.meld_dir.join(rel)).map_err(|e| {
                    fail(
                        FailKind::ManifestMismatch,
                        format!("cannot read {rel}: {e}"),
                    )
                })
            },
            Outcome::Refused
            | Outcome::MeldInvalid
            | Outcome::Unencodable
            | Outcome::Panic
            | Outcome::FusedInvalid => Err(CheckResult::Refused(format!(
                "meld {:?}: {}",
                comp.outcome, comp.detail
            ))),
            Outcome::InvalidFused => Err(fail(
                FailKind::MeldAcceptedInvalid,
                "meld fused a component the assertion says is invalid",
            )),
            Outcome::InvalidRefused | Outcome::InvalidUnencodable => Err(fail(
                FailKind::UnexpectedOutcome,
                format!("{:?} outcome on a {:?} directive", comp.outcome, comp.kind),
            )),
        }
    }

    /// Refuse to run a module whose imports nothing provides: the engine's
    /// host handler would otherwise answer them with no-ops.
    fn require_no_imports(bytes: &[u8]) -> Result<(), CheckResult> {
        match kiln_decoder::decoder::decode_module(bytes) {
            Ok(m) if !m.imports.is_empty() => Err(fail(
                FailKind::UnresolvedImports,
                format!("{} imports", m.imports.len()),
            )),
            Ok(_) => Ok(()),
            Err(e) => Err(fail(FailKind::LoadFailed, format!("decode: {e}"))),
        }
    }

    /// Load a fused module under a fresh instance name.
    ///
    /// A failed instantiation is remembered, so later commands on the same
    /// component report that root cause instead of a missing instance.
    fn instantiate(&mut self, comp_idx: usize, bytes: &[u8]) -> Result<(), CheckResult> {
        self.live.remove(&comp_idx);
        self.failed.remove(&comp_idx);
        let result = Self::require_no_imports(bytes).and_then(|()| {
            self.instances += 1;
            let name = format!("c{comp_idx}-{}", self.instances);
            match self.engine.load_module(Some(&name), bytes) {
                Ok(()) => {
                    self.live.insert(comp_idx, name);
                    Ok(())
                },
                Err(e) => Err(CheckResult::Fail(FailKind::LoadFailed, format!("{e:#}"))),
            }
        });
        if let Err(CheckResult::Fail(kind, detail)) = &result {
            self.failed.insert(comp_idx, (*kind, detail.clone()));
        }
        result
    }

    fn component_check(&mut self, comp: &ManifestComponent) -> Option<CheckResult> {
        match comp.kind {
            ComponentKind::AssertInvalid | ComponentKind::AssertMalformed => {
                Some(match comp.outcome {
                    Outcome::InvalidRefused | Outcome::InvalidUnencodable => CheckResult::Pass,
                    Outcome::InvalidFused => fail(
                        FailKind::MeldAcceptedInvalid,
                        "meld fused a component the assertion says is invalid",
                    ),
                    o => fail(
                        FailKind::UnexpectedOutcome,
                        format!("{o:?} outcome on a {:?} directive", comp.kind),
                    ),
                })
            },
            ComponentKind::AssertTrapInstantiate => None,
            ComponentKind::AssertUnlinkable => Some(match self.fused_module(comp) {
                Ok(_) => fail(
                    FailKind::UnsupportedAssertion,
                    "assert_unlinkable on a fused component is not executed",
                ),
                Err(r) => r,
            }),
            ComponentKind::Definition => Some(match self.fused_module(comp) {
                Ok(_) => CheckResult::Pass,
                Err(r) => r,
            }),
            ComponentKind::Component => Some(match self.fused_module(comp) {
                Ok(bytes) => match self.instantiate(comp.component_index, &bytes) {
                    Ok(()) => CheckResult::Pass,
                    Err(r) => r,
                },
                Err(r) => r,
            }),
        }
    }

    fn command_check(&mut self, cmd: &ParsedCommand, mc: &ManifestCommand) -> CheckResult {
        let Some(target) = mc.target_component else {
            return fail(
                FailKind::ManifestMismatch,
                "command has no target component",
            );
        };
        let comp = &self.mf.components[target];
        match mc.kind {
            CommandKind::Register => {
                fail(FailKind::UnsupportedAssertion, "register is not executed")
            },
            CommandKind::Instance => match self.fused_module(comp) {
                Ok(bytes) => match self.instantiate(target, &bytes) {
                    Ok(()) => CheckResult::Pass,
                    Err(r) => r,
                },
                Err(r) => r,
            },
            CommandKind::AssertInstantiate => match self.fused_module(comp) {
                Ok(bytes) => match self.instantiate_expecting_trap(target, &bytes) {
                    Ok(()) => CheckResult::Pass,
                    Err(r) => r,
                },
                Err(r) => r,
            },
            CommandKind::Invoke | CommandKind::Assert => {
                if let Err(r) = self.fused_module(comp) {
                    return r;
                }
                let Some(exec) = &cmd.exec else {
                    return fail(
                        FailKind::UnsupportedAssertion,
                        "assertion form is not executed (exception, exhaustion or suspension)",
                    );
                };
                self.invoke(target, exec)
            },
        }
    }

    fn instantiate_expecting_trap(
        &mut self,
        comp_idx: usize,
        bytes: &[u8],
    ) -> Result<(), CheckResult> {
        Self::require_no_imports(bytes)?;
        self.instances += 1;
        let name = format!("c{comp_idx}-{}", self.instances);
        match self.engine.load_module(Some(&name), bytes) {
            Ok(()) => Err(fail(FailKind::NoTrap, "instantiation succeeded")),
            Err(e) if is_runtime_trap(&e) => Ok(()),
            Err(e) => Err(fail(FailKind::NotATrap, format!("{e:#}"))),
        }
    }

    fn invoke(&mut self, target: usize, exec: &Exec) -> CheckResult {
        let Some(name) = self.live.get(&target).cloned() else {
            if let Some((kind, detail)) = self.failed.get(&target) {
                return fail(
                    *kind,
                    format!("`{}`: instantiation failed: {detail}", exec.export),
                );
            }
            return fail(
                FailKind::NoLiveInstance,
                format!(
                    "`{}` invoked before the component was instantiated",
                    exec.export
                ),
            );
        };
        let want_results = match &exec.expect {
            Expect::Return(None) => {
                return fail(
                    FailKind::ValueLowering,
                    format!("results of `{}`", exec.export),
                );
            },
            Expect::Return(Some(r)) => Some(r),
            Expect::Trap | Expect::Invoke => None,
        };
        let Some(args) = &exec.args else {
            return fail(
                FailKind::ValueLowering,
                format!("arguments of `{}`", exec.export),
            );
        };
        let exported = self.engine.get_module(&name).and_then(|m| m.get_export(&exec.export));
        match exported {
            Some(e) if e.kind == kiln_runtime::module::ExportKind::Function => {},
            _ => {
                return fail(
                    FailKind::ExportNotFound,
                    format!(
                        "`{}` is not a function export of the fused module",
                        exec.export
                    ),
                );
            },
        }
        let args: Vec<Value> = args.iter().copied().map(to_value).collect();
        let result = self.engine.invoke_function(Some(&name), &exec.export, &args);
        match (&exec.expect, result) {
            (Expect::Trap, Ok(_)) => fail(FailKind::NoTrap, format!("`{}` returned", exec.export)),
            (Expect::Trap, Err(e)) if is_runtime_trap(&e) => CheckResult::Pass,
            (Expect::Trap, Err(e)) => fail(FailKind::NotATrap, format!("`{}`: {e:#}", exec.export)),
            (_, Err(e)) => fail(
                FailKind::ExecutionError,
                format!("`{}`: {e:#}", exec.export),
            ),
            (_, Ok(got)) => match want_results {
                None => CheckResult::Pass,
                Some(want) => {
                    if got.len() == want.len()
                        && got.iter().zip(want).all(|(g, w)| matches_scalar(g, *w))
                    {
                        CheckResult::Pass
                    } else {
                        fail(
                            FailKind::WrongResult,
                            format!("`{}`: got {got:?}, want {want:?}", exec.export),
                        )
                    }
                },
            },
        }
    }
}

/// Run one WAST file against its manifest entry.
fn run_file(upstream: &Path, meld_dir: &Path, mf: &ManifestFile) -> Result<FileReport> {
    let text = fs::read_to_string(upstream.join(&mf.wast))
        .with_context(|| format!("reading {}", mf.wast))?;
    let single = |result| FileReport {
        wast: mf.wast.clone(),
        checks: vec![Check { line: 0, result }],
    };
    let parsed = match parse_wast(&text) {
        Ok(p) => p,
        Err(e) => {
            return Ok(single(fail(
                FailKind::ManifestMismatch,
                format!("WAST does not parse: {e}"),
            )));
        },
    };
    if let Err(msg) = match_manifest(&parsed, mf) {
        return Ok(single(fail(FailKind::ManifestMismatch, msg)));
    }

    let engine = WastEngine::new().context("creating the execution engine")?;
    let mut run = FileRun {
        engine: Box::new(engine),
        mf,
        meld_dir,
        live: HashMap::new(),
        failed: HashMap::new(),
        instances: 0,
    };

    // Replay in directive order. A directive's component entry (if any) comes
    // before its command (an `assert-instantiate` has both).
    let mut checks = Vec::new();
    let mut commands = parsed.commands.iter().zip(&mf.commands).peekable();
    for comp in &mf.components {
        while let Some((cmd, mc)) = commands.next_if(|(c, _)| c.directive < comp.directive_index) {
            checks.push(Check {
                line: cmd.line,
                result: run.command_check(cmd, mc),
            });
        }
        if let Some(result) = run.component_check(comp) {
            checks.push(Check {
                line: comp.line,
                result,
            });
        }
    }
    for (cmd, mc) in commands {
        checks.push(Check {
            line: cmd.line,
            result: run.command_check(cmd, mc),
        });
    }
    Ok(FileReport {
        wast: mf.wast.clone(),
        checks,
    })
}

/// Find the single `meld-*` directory under `dir`.
fn meld_dir(dir: &Path) -> Result<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.file_name().to_string_lossy().starts_with("meld-") {
            found.push(entry.path());
        }
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        n => bail!(
            "expected exactly one meld-* directory in {}, found {n}",
            dir.display()
        ),
    }
}

/// Run every WAST in the manifest under `dir` (`tests/component-model`).
///
/// An `Err` is an infrastructure problem (missing or unreadable vendored
/// files). Anything kiln or meld got wrong is a verdict in the report.
pub fn run(dir: &Path) -> Result<CmWastReport> {
    let meld = meld_dir(dir)?;
    let manifest_text = fs::read_to_string(meld.join("manifest.json"))
        .with_context(|| format!("reading {}/manifest.json", meld.display()))?;
    let manifest: Manifest =
        serde_json::from_str(&manifest_text).context("parsing manifest.json")?;
    let upstream = dir.join("upstream");
    let want_commit =
        fs::read_to_string(upstream.join("COMMIT")).context("reading upstream/COMMIT")?;
    if want_commit.trim() != manifest.component_model_commit {
        bail!(
            "manifest was produced against component-model {}, upstream/ is {}",
            manifest.component_model_commit,
            want_commit.trim()
        );
    }

    let mut on_disk = Vec::new();
    collect_wasts(&upstream.join("test"), &upstream, &mut on_disk)?;
    let in_manifest: Vec<&str> = manifest.files.iter().map(|f| f.wast.as_str()).collect();
    for w in &on_disk {
        if !in_manifest.contains(&w.as_str()) {
            bail!("{w} is vendored but missing from the manifest");
        }
    }

    let mut files = Vec::new();
    for mf in &manifest.files {
        // The workspace builds with panic = "abort", so a kiln panic ends the
        // run without a report and the gate fails loudly.
        files.push(run_file(&upstream, &meld, mf)?);
    }
    files.sort_by(|a, b| a.wast.cmp(&b.wast));
    Ok(CmWastReport {
        component_model_commit: manifest.component_model_commit,
        meld_version: manifest.meld_version,
        files,
    })
}

fn collect_wasts(dir: &Path, root: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_wasts(&path, root, out)?;
        } else if path.extension().is_some_and(|e| e == "wast") {
            out.push(path.strip_prefix(root)?.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAST: &str = r#"
(component
  (core module $m (func (export "f") (param i32) (result i32) local.get 0))
  (core instance $i (instantiate $m))
  (func (export "f") (param "x" u8) (result u8) (canon lift (core func $i "f")))
)
(assert_return (invoke "f" (u8.const 7)) (u8.const 7))
(assert_trap (invoke "f" (u8.const 1)) "unreachable")
(assert_invalid (component) "bad")
(assert_return (invoke "g" (str.const "x")) (u8.const 1))
"#;

    fn comp(
        i: usize,
        d: usize,
        line: usize,
        kind: ComponentKind,
        outcome: Outcome,
    ) -> ManifestComponent {
        ManifestComponent {
            component_index: i,
            directive_index: d,
            line,
            kind,
            outcome,
            detail: String::new(),
            fused_path: None,
        }
    }

    fn chk(result: CheckResult) -> Check {
        Check { line: 1, result }
    }

    #[test]
    fn verdict_is_the_worst_check() {
        assert_eq!(verdict_of(&[]), Verdict::Pass);
        assert_eq!(verdict_of(&[chk(CheckResult::Pass)]), Verdict::Pass);
        let refused = chk(CheckResult::Refused("x".into()));
        assert_eq!(
            verdict_of(&[chk(CheckResult::Pass), refused.clone()]),
            Verdict::RefusedAtFuse
        );
        let failed = chk(CheckResult::Fail(FailKind::WrongResult, "x".into()));
        assert_eq!(verdict_of(&[refused, failed]), Verdict::Fail);
        assert!(Verdict::Pass < Verdict::RefusedAtFuse && Verdict::RefusedAtFuse < Verdict::Fail);
    }

    #[test]
    fn parse_selects_components_and_commands() {
        let p = parse_wast(WAST).expect("parses");
        let kinds: Vec<_> = p.components.iter().map(|c| (c.directive, c.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                (0, ComponentKind::Component),
                (3, ComponentKind::AssertInvalid)
            ]
        );
        assert_eq!(p.commands.len(), 3);
        let first = p.commands[0].exec.as_ref().expect("exec");
        assert_eq!(first.args, Some(vec![Scalar::I32(7)]));
        assert_eq!(first.expect, Expect::Return(Some(vec![Scalar::I32(7)])));
        assert_eq!(
            p.commands[1].exec.as_ref().expect("exec").expect,
            Expect::Trap
        );
        // A string argument is not a core scalar.
        assert_eq!(p.commands[2].exec.as_ref().expect("exec").args, None);
    }

    #[test]
    fn manifest_matches_by_directive_line_and_kind() {
        let p = parse_wast(WAST).expect("parses");
        let mf = ManifestFile {
            wast: "test/x.wast".into(),
            components: vec![
                comp(
                    0,
                    0,
                    p.components[0].line,
                    ComponentKind::Component,
                    Outcome::Fused,
                ),
                comp(
                    1,
                    3,
                    p.components[1].line,
                    ComponentKind::AssertInvalid,
                    Outcome::InvalidRefused,
                ),
            ],
            commands: p
                .commands
                .iter()
                .map(|c| ManifestCommand {
                    index: c.directive,
                    line: c.line,
                    kind: c.kind,
                    target_component: Some(0),
                })
                .collect(),
        };
        assert_eq!(match_manifest(&p, &mf), Ok(()));

        let mut wrong_kind = mf.clone();
        wrong_kind.components[1].kind = ComponentKind::AssertMalformed;
        assert!(match_manifest(&p, &wrong_kind).is_err());

        let mut wrong_line = mf.clone();
        wrong_line.components[0].line += 1;
        assert!(match_manifest(&p, &wrong_line).is_err());

        let mut wrong_dir = mf.clone();
        wrong_dir.commands[1].index += 1;
        assert!(match_manifest(&p, &wrong_dir).is_err());

        let mut missing = mf.clone();
        missing.commands.pop();
        assert!(match_manifest(&p, &missing).is_err());

        let mut bad_target = mf;
        bad_target.commands[0].target_component = Some(9);
        assert!(match_manifest(&p, &bad_target).is_err());
    }

    #[test]
    fn manifest_rejects_unknown_outcomes() {
        let json = r#"{"component_index":0,"directive_index":0,"line":1,"kind":"component",
            "outcome":"mystery","detail":"","fused_path":null}"#;
        assert!(serde_json::from_str::<ManifestComponent>(json).is_err());
    }

    #[test]
    fn nan_expectations_match_any_nan() {
        use kiln_foundation::values::FloatBits32;
        let nan = Value::F32(FloatBits32::from_bits(0x7fc0_0001));
        assert!(matches_scalar(&nan, Scalar::F32(0x7fc0_0000)));
        assert!(!matches_scalar(&Value::I32(0), Scalar::F32(0)));
        assert!(matches_scalar(&Value::I32(-1), Scalar::I32(-1)));
        assert!(!matches_scalar(&Value::I32(1), Scalar::I32(-1)));
    }
}
