//! Test execution and management
//!
//! The test gate is fail-closed: a run only succeeds when every underlying
//! `cargo test` invocation exited zero, none timed out, at least one
//! `test result:` line was printed, and no test failed. Counts are summed over
//! every `test result:` line cargo prints (one per test binary and doctest
//! group), never taken from a single line.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use colored::Colorize;

use crate::{
    build::BuildSystem,
    diagnostics::{Diagnostic, DiagnosticCollection, Range, Severity, ToolOutputParser},
    error::{BuildError, BuildResult},
    parsers::CargoOutputParser,
};

/// Default wall-clock limit for one `cargo test` invocation.
///
/// A deadlocked test must fail the run instead of hanging CI. The limit covers
/// compilation as well as execution.
pub const DEFAULT_TEST_TIMEOUT: Duration = Duration::from_secs(45 * 60);

/// Test execution results
#[derive(Debug)]
pub struct TestResults {
    /// Whether all tests passed
    pub success: bool,
    /// Total number of tests run (passed + failed, ignored excluded)
    pub total_tests: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of ignored tests
    pub ignored: usize,
    /// Why the run failed, when it did
    pub failure_reasons: Vec<String>,
    /// Test execution duration
    pub duration_ms: u64,
    /// Test output and failures
    pub output: String,
}

/// Test execution options
#[derive(Debug, Clone)]
pub struct TestOptions {
    /// Run only specific test filter
    pub filter: Option<String>,
    /// Include integration tests
    pub integration: bool,
    /// Include doc tests
    pub doc_tests: bool,
    /// Run tests with --nocapture
    pub nocapture: bool,
    /// Parallel test execution
    pub parallel: bool,
    /// Wall-clock limit for each `cargo test` invocation
    pub timeout: Duration,
}

impl Default for TestOptions {
    fn default() -> Self {
        Self {
            filter: None,
            integration: true,
            doc_tests: true,
            nocapture: false,
            parallel: true,
            timeout: DEFAULT_TEST_TIMEOUT,
        }
    }
}

/// Counts summed over every `test result:` line of a cargo test run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestCounts {
    /// Tests that passed
    pub passed: usize,
    /// Tests that failed
    pub failed: usize,
    /// Tests that were ignored
    pub ignored: usize,
}

impl TestCounts {
    fn add(&mut self, other: TestCounts) {
        self.passed += other.passed;
        self.failed += other.failed;
        self.ignored += other.ignored;
    }
}

/// Outcome of one `cargo test` invocation.
#[derive(Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Counts found in the output (zero when none were printed)
    pub counts: TestCounts,
    /// Why the invocation is a failure; `None` only when it is a real pass
    pub failure: Option<String>,
}

/// Sum the counts of every `test result:` line in cargo test output.
///
/// Returns `None` when there is no `test result:` line, or when one cannot be
/// parsed: a gate must not guess a count.
pub fn parse_test_results(output: &str) -> Option<TestCounts> {
    let mut total = TestCounts::default();
    let mut lines_seen = 0usize;
    for line in output.lines() {
        // "test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; ..."
        let Some(rest) = line.trim_start().strip_prefix("test result:") else {
            continue;
        };
        let (_status, fields) = rest.trim_start().split_once(". ")?;
        let mut line_counts = TestCounts::default();
        let (mut saw_passed, mut saw_failed) = (false, false);
        for field in fields.split(';') {
            let mut words = field.split_whitespace();
            let (Some(count), Some(label)) = (words.next(), words.next()) else {
                continue;
            };
            match label {
                "passed" => {
                    line_counts.passed = count.parse().ok()?;
                    saw_passed = true;
                },
                "failed" => {
                    line_counts.failed = count.parse().ok()?;
                    saw_failed = true;
                },
                "ignored" => line_counts.ignored = count.parse().ok()?,
                _ => {},
            }
        }
        if !(saw_passed && saw_failed) {
            return None;
        }
        total.add(line_counts);
        lines_seen += 1;
    }
    (lines_seen > 0).then_some(total)
}

/// Decide whether one `cargo test` invocation passed.
///
/// Cargo's exit status is authoritative; parsed counts can only add reasons to
/// fail, never override a non-zero exit.
pub fn judge(exit_success: bool, timed_out: bool, stdout: &str) -> Verdict {
    let parsed = parse_test_results(stdout);
    let counts = parsed.unwrap_or_default();
    let failure = if timed_out {
        Some("timed out (killed): a hung test fails the run".to_string())
    } else if !exit_success {
        Some("cargo test exited non-zero (test failure, compile error or crash)".to_string())
    } else if parsed.is_none() {
        Some("no parsable `test result:` line in the output".to_string())
    } else if counts.failed > 0 {
        Some(format!("{} test(s) failed", counts.failed))
    } else {
        None
    };
    Verdict { counts, failure }
}

/// The `cargo test` target selections needed for the requested test kinds.
///
/// `cargo test --doc` cannot be combined with other target selectors, so
/// doctests without integration tests need a second invocation.
fn target_selections(options: &TestOptions) -> Vec<Vec<&'static str>> {
    match (options.integration, options.doc_tests) {
        (true, true) => vec![vec![]],
        (true, false) => vec![vec!["--lib", "--bins", "--tests"]],
        (false, false) => vec![vec!["--lib", "--bins"]],
        (false, true) => vec![vec!["--lib", "--bins"], vec!["--doc"]],
    }
}

/// Arguments for one workspace-wide `cargo test` invocation.
fn cargo_test_args(
    options: &TestOptions,
    selection: &[&str],
    features: &[String],
    extra: &[&str],
) -> Vec<String> {
    let mut args: Vec<String> = ["test", "--workspace", "--no-fail-fast"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend(selection.iter().map(|s| s.to_string()));
    args.extend(extra.iter().map(|s| s.to_string()));
    if !features.is_empty() {
        args.push("--features".to_string());
        args.push(features.join(","));
    }
    if let Some(filter) = &options.filter {
        args.push(filter.clone());
    }

    let mut harness: Vec<&str> = Vec::new();
    if options.nocapture {
        harness.push("--nocapture");
    }
    if !options.parallel {
        harness.push("--test-threads=1");
    }
    if !harness.is_empty() {
        args.push("--".to_string());
        args.extend(harness.iter().map(|s| s.to_string()));
    }
    args
}

/// Raw result of running `cargo test` as a child process.
struct CargoRun {
    exit_success: bool,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn spawn_reader<R: Read + Send + 'static>(
    source: R,
    mut sink: Box<dyn Write + Send>,
) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut reader = BufReader::new(source);
        let mut captured = String::new();
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // The echo is for humans watching the run; the capture is
                    // what the gate judges.
                    let _ = sink.write_all(&line);
                    captured.push_str(&String::from_utf8_lossy(&line));
                },
            }
        }
        captured
    })
}

/// Kill the child and, on unix, its whole process group, so a hung test
/// binary cannot outlive cargo and keep the pipes open.
fn kill_process_tree(child: &mut Child) -> BuildResult<()> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg("-KILL")
            .arg("--")
            .arg(format!("-{}", child.id()))
            .status()
            .map_err(|e| BuildError::Tool(format!("Failed to run kill: {}", e)))?;
        if !status.success() {
            return Err(BuildError::Tool(format!(
                "Failed to kill timed-out test process group {}",
                child.id()
            )));
        }
    }
    child
        .kill()
        .map_err(|e| BuildError::Tool(format!("Failed to kill timed-out cargo test: {}", e)))?;
    child
        .wait()
        .map_err(|e| BuildError::Tool(format!("Failed to reap timed-out cargo test: {}", e)))?;
    Ok(())
}

/// Run `cargo test` with a wall-clock limit, optionally echoing its output.
fn run_cargo_test(
    args: &[String],
    workdir: &std::path::Path,
    timeout: Duration,
    echo: bool,
) -> BuildResult<CargoRun> {
    let mut cmd = Command::new("cargo");
    cmd.args(args).current_dir(workdir);
    run_with_timeout(cmd, timeout, echo)
}

/// Run a command in its own process group with a wall-clock limit.
fn run_with_timeout(mut cmd: Command, timeout: Duration, echo: bool) -> BuildResult<CargoRun> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| BuildError::Tool(format!("Failed to execute cargo test: {}", e)))?;

    let stdout_sink: Box<dyn Write + Send> =
        if echo { Box::new(io::stdout()) } else { Box::new(io::sink()) };
    let stderr_sink: Box<dyn Write + Send> =
        if echo { Box::new(io::stderr()) } else { Box::new(io::sink()) };
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| BuildError::Tool("cargo test stdout was not piped".to_string()))?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| BuildError::Tool("cargo test stderr was not piped".to_string()))?;
    let stdout_reader = spawn_reader(child_stdout, stdout_sink);
    let stderr_reader = spawn_reader(child_stderr, stderr_sink);

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let exit_success = loop {
        match child
            .try_wait()
            .map_err(|e| BuildError::Tool(format!("Failed to wait for cargo test: {}", e)))?
        {
            Some(status) => break status.success(),
            None if Instant::now() >= deadline => {
                timed_out = true;
                kill_process_tree(&mut child)?;
                break false;
            },
            None => thread::sleep(Duration::from_millis(100)),
        }
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| BuildError::Tool("cargo test stdout reader panicked".to_string()))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| BuildError::Tool("cargo test stderr reader panicked".to_string()))?;
    Ok(CargoRun {
        exit_success,
        timed_out,
        stdout,
        stderr,
    })
}

impl BuildSystem {
    /// Run all tests in the workspace
    pub fn run_tests(&self) -> BuildResult<TestResults> {
        self.run_tests_with_options(&TestOptions::default())
    }

    /// Run tests with diagnostic output
    pub fn run_tests_with_diagnostics(
        &self,
        options: &TestOptions,
    ) -> BuildResult<DiagnosticCollection> {
        let start_time = std::time::Instant::now();
        let mut collection =
            DiagnosticCollection::new(self.workspace.root.clone(), "test".to_string());

        let args = cargo_test_args(
            options,
            &[],
            &self.config.features,
            &["--message-format=json"],
        );
        let run = run_cargo_test(&args, &self.workspace.root, options.timeout, false)?;

        // Parse cargo output for diagnostics
        let parser = CargoOutputParser::new(&self.workspace.root);
        match parser.parse_output(&run.stdout, &run.stderr, &self.workspace.root) {
            Ok(diagnostics) => collection.add_diagnostics(diagnostics),
            Err(e) => {
                collection.add_diagnostic(Diagnostic::new(
                    "<test>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("Failed to parse test output: {}", e),
                    "cargo-kiln".to_string(),
                ));
            },
        }

        let verdict = judge(run.exit_success, run.timed_out, &run.stdout);
        match verdict.failure {
            Some(reason) => collection.add_diagnostic(Diagnostic::new(
                "<test>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                format!(
                    "Tests failed ({} passed, {} failed): {}",
                    verdict.counts.passed, verdict.counts.failed, reason
                ),
                "cargo-kiln".to_string(),
            )),
            None => collection.add_diagnostic(Diagnostic::new(
                "<test>".to_string(),
                Range::entire_line(0),
                Severity::Info,
                format!("All tests passed: {} tests", verdict.counts.passed),
                "cargo-kiln".to_string(),
            )),
        }

        let duration = start_time.elapsed();
        Ok(collection.finalize(duration.as_millis() as u64))
    }

    /// Run tests with specific options.
    ///
    /// Runs `cargo test --workspace --no-fail-fast` (unit, integration and
    /// doc tests under workspace feature unification). Returns `Ok` with
    /// `success == false` when tests fail; callers must check
    /// [`TestResults::is_success`].
    pub fn run_tests_with_options(&self, options: &TestOptions) -> BuildResult<TestResults> {
        println!("{} Running Kiln test suite...", "🧪".bright_blue());

        let start_time = std::time::Instant::now();
        let mut counts = TestCounts::default();
        let mut failure_reasons = Vec::new();
        let mut output = String::new();

        for selection in target_selections(options) {
            let args = cargo_test_args(options, &selection, &self.config.features, &[]);
            if self.config.verbose {
                println!("  {} cargo {}", "▶".bright_cyan(), args.join(" "));
            }
            let run = run_cargo_test(&args, &self.workspace.root, options.timeout, true)?;
            let verdict = judge(run.exit_success, run.timed_out, &run.stdout);
            counts.add(verdict.counts);
            if let Some(reason) = verdict.failure {
                failure_reasons.push(format!("cargo {}: {}", args.join(" "), reason));
            }
            output.push_str(&run.stdout);
            output.push('\n');
            output.push_str(&run.stderr);
        }

        if failure_reasons.is_empty() && counts.passed == 0 {
            failure_reasons
                .push("no tests ran (0 passed): the gate refuses a vacuous pass".to_string());
        }

        let duration = start_time.elapsed();
        let success = failure_reasons.is_empty();
        let total_tests = counts.passed + counts.failed;

        if success {
            println!(
                "{} All tests passed! ({} passed, {} ignored in {:.2}s)",
                "✅".bright_green(),
                counts.passed,
                counts.ignored,
                duration.as_secs_f64()
            );
        } else {
            println!(
                "{} Test run failed: {} failed, {} passed ({:.2}s)",
                "❌".bright_red(),
                counts.failed,
                counts.passed,
                duration.as_secs_f64()
            );
            for reason in &failure_reasons {
                println!("  - {}", reason);
            }
        }

        Ok(TestResults {
            success,
            total_tests,
            passed: counts.passed,
            failed: counts.failed,
            ignored: counts.ignored,
            failure_reasons,
            duration_ms: duration.as_millis() as u64,
            output,
        })
    }
}

impl TestResults {
    /// Check if all tests passed
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Get test summary string
    pub fn summary(&self) -> String {
        format!(
            "Tests: {} total, {} passed, {} failed",
            self.total_tests, self.passed, self.failed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK_LINES: &str = "\
running 3 tests
test a ... ok
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 39 tests
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

   Doc-tests kiln_error
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";

    #[test]
    fn test_options_default() {
        let options = TestOptions::default();
        assert!(options.integration);
        assert!(options.doc_tests);
        assert!(!options.nocapture);
        assert_eq!(options.timeout, DEFAULT_TEST_TIMEOUT);
    }

    #[test]
    fn test_results_success() {
        let results = TestResults {
            success: true,
            total_tests: 10,
            passed: 10,
            failed: 0,
            ignored: 0,
            failure_reasons: Vec::new(),
            duration_ms: 1000,
            output: "All tests passed".to_string(),
        };

        assert!(results.is_success());
        assert_eq!(results.summary(), "Tests: 10 total, 10 passed, 0 failed");
    }

    #[test]
    fn parse_sums_every_result_line() {
        let counts = parse_test_results(OK_LINES).expect("result lines present");
        assert_eq!(
            counts,
            TestCounts {
                passed: 42,
                failed: 0,
                ignored: 1
            }
        );
    }

    #[test]
    fn parse_counts_failures_from_a_failed_line() {
        let out = "\
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: FAILED. 2 passed; 3 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
        let counts = parse_test_results(out).expect("result lines present");
        assert_eq!(
            counts,
            TestCounts {
                passed: 7,
                failed: 3,
                ignored: 1
            }
        );
    }

    #[test]
    fn parse_without_result_lines_is_none() {
        assert_eq!(parse_test_results(""), None);
        assert_eq!(
            parse_test_results("error[E0432]: unresolved import\n"),
            None
        );
        // Individual "test x ... ok" lines are not a summary.
        assert_eq!(
            parse_test_results("test a ... ok\ntest b ... FAILED\n"),
            None
        );
    }

    #[test]
    fn parse_unparsable_result_line_is_none() {
        assert_eq!(
            parse_test_results("test result: ok. many passed; 0 failed\n"),
            None
        );
    }

    #[test]
    fn judge_passes_only_on_zero_exit_with_counts() {
        let verdict = judge(true, false, OK_LINES);
        assert_eq!(verdict.failure, None);
        assert_eq!(verdict.counts.passed, 42);
    }

    #[test]
    fn judge_fails_on_nonzero_exit_even_if_counts_look_clean() {
        let verdict = judge(false, false, OK_LINES);
        assert!(verdict.failure.is_some());
    }

    #[test]
    fn judge_fails_when_no_result_lines_even_with_zero_exit() {
        let verdict = judge(true, false, "");
        assert!(verdict.failure.is_some());
        assert_eq!(verdict.counts, TestCounts::default());
    }

    #[test]
    fn judge_fails_on_failed_tests_even_with_zero_exit() {
        let out =
            "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let verdict = judge(true, false, out);
        assert!(verdict.failure.is_some());
        assert_eq!(verdict.counts.failed, 1);
    }

    #[test]
    fn judge_fails_on_timeout() {
        let verdict = judge(false, true, OK_LINES);
        let reason = verdict.failure.expect("timeout is a failure");
        assert!(reason.contains("timed out"), "reason was: {reason}");
    }

    #[test]
    fn selections_cover_the_requested_kinds() {
        let mut options = TestOptions::default();
        assert_eq!(target_selections(&options), vec![Vec::<&str>::new()]);
        options.doc_tests = false;
        assert_eq!(
            target_selections(&options),
            vec![vec!["--lib", "--bins", "--tests"]]
        );
        options.integration = false;
        assert_eq!(target_selections(&options), vec![vec!["--lib", "--bins"]]);
        options.doc_tests = true;
        assert_eq!(
            target_selections(&options),
            vec![vec!["--lib", "--bins"], vec!["--doc"]]
        );
    }

    #[test]
    fn args_never_stop_at_first_failure_and_pass_harness_flags_after_separator() {
        let options = TestOptions {
            filter: Some("foo".to_string()),
            nocapture: true,
            parallel: false,
            ..TestOptions::default()
        };
        let args = cargo_test_args(&options, &[], &["std".to_string()], &[]);
        assert_eq!(
            args,
            [
                "test",
                "--workspace",
                "--no-fail-fast",
                "--features",
                "std",
                "foo",
                "--",
                "--nocapture",
                "--test-threads=1"
            ]
        );
    }

    #[test]
    fn a_hung_process_is_killed_at_the_timeout() {
        let start = Instant::now();
        let mut cmd = Command::new("sleep");
        cmd.arg("60");
        let run = run_with_timeout(cmd, Duration::from_millis(500), false)
            .expect("runner itself must not error");
        assert!(run.timed_out);
        assert!(!run.exit_success);
        assert!(start.elapsed() < Duration::from_secs(30));
    }
}
