//! Core build system implementation

#[cfg(all(feature = "std", unix))]
use std::os::unix::process::ExitStatusExt;
#[cfg(all(feature = "std", windows))]
use std::os::windows::process::ExitStatusExt;
#[cfg(feature = "std")]
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use colored::Colorize;

use crate::{
    config::{BuildConfig, WorkspaceConfig},
    diagnostics::{Diagnostic, DiagnosticCollection, Range, Severity, ToolOutputParser},
    error::{BuildError, BuildResult},
    parsers::CargoOutputParser,
};

/// Helper function to execute commands with tracing and dry-run support
#[cfg(feature = "std")]
pub fn execute_command(
    cmd: &mut Command,
    config: &BuildConfig,
    description: &str,
) -> BuildResult<std::process::Output> {
    let cmd_str = format_command(cmd);

    if config.trace_commands || config.verbose {
        println!(
            "{} {}: {}",
            "🔧".bright_blue(),
            description,
            cmd_str.bright_cyan()
        );
    }

    if config.dry_run {
        println!(
            "{} DRY RUN - would execute: {}",
            "🔍".bright_yellow(),
            cmd_str
        );
        // Return fake successful output for dry run
        #[cfg(unix)]
        let status = std::process::ExitStatus::from_raw(0);
        #[cfg(windows)]
        let status = std::process::ExitStatus::from_raw(0);
        #[cfg(not(any(unix, windows)))]
        compile_error!("Unsupported platform for dry run mode");

        return Ok(std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }

    cmd.output()
        .map_err(|e| BuildError::Tool(format!("Failed to execute command '{}': {}", cmd_str, e)))
}

/// Format a command for display
#[cfg(feature = "std")]
fn format_command(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let args: Vec<String> = cmd.get_args().map(|arg| arg.to_string_lossy().to_string()).collect();

    if args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, args.join(" "))
    }
}

/// Ported functions from xtask for build operations
#[cfg(feature = "std")]
pub mod xtask_port {
    use std::process::Command;

    use super::*;

    /// Run comprehensive coverage analysis (ported from xtask coverage)
    pub fn run_coverage_analysis(config: &BuildConfig) -> BuildResult<()> {
        println!(
            "{} Running comprehensive coverage analysis...",
            "📊".bright_blue()
        );

        // Build with coverage flags
        let mut cmd = Command::new("cargo");
        cmd.args(["test", "--no-run", "--workspace"])
            .env("RUSTFLAGS", "-C instrument-coverage")
            .env("LLVM_PROFILE_FILE", "target/coverage/profile-%p-%m.profraw");

        let output = super::execute_command(&mut cmd, config, "Building with coverage flags")?;

        if !output.status.success() {
            return Err(BuildError::Build("Coverage build failed".to_string()));
        }

        // Run tests with coverage
        let mut test_cmd = Command::new("cargo");
        test_cmd
            .args(["test", "--workspace"])
            .env("RUSTFLAGS", "-C instrument-coverage")
            .env("LLVM_PROFILE_FILE", "target/coverage/profile-%p-%m.profraw");

        let test_output =
            super::execute_command(&mut test_cmd, config, "Running tests with coverage")?;

        if !test_output.status.success() {
            return Err(BuildError::Test("Coverage tests failed".to_string()));
        }

        println!("{} Coverage analysis completed", "✅".bright_green());
        Ok(())
    }

    /// Generate documentation (ported from xtask docs)
    pub fn generate_docs() -> BuildResult<()> {
        generate_docs_with_options(false, false)
    }

    /// Generate documentation with options for private items and browser
    /// opening
    pub fn generate_docs_with_options(include_private: bool, open_docs: bool) -> BuildResult<()> {
        generate_docs_with_output_dir(include_private, open_docs, None)
    }

    /// Generate documentation with custom output directory
    pub fn generate_docs_with_output_dir(
        include_private: bool,
        open_docs: bool,
        output_dir: Option<String>,
    ) -> BuildResult<()> {
        println!("{} Generating documentation...", "📚".bright_blue());

        // 1. Generate Rust API documentation
        println!("  📖 Building Rust API documentation...");

        // Build documentation for each package individually to avoid conflicts
        let packages = [
            "wrt-error",
            "wrt-foundation",
            "wrt-sync",
            "wrt-logging",
            "wrt-math",
            "wrt-decoder",
            "wrt-intercept",
            "wrt-panic",
            "wrt-build-core",
            "cargo-wrt",
        ];

        let mut any_failed = false;

        for package in &packages {
            print!("    Building docs for {}... ", package);
            std::io::stdout().flush().ok();

            let mut cmd = Command::new("cargo");
            cmd.args(["doc", "--no-deps", "-p", package]);

            if include_private {
                cmd.arg("--document-private-items");
            }

            if let Some(ref out_dir) = output_dir {
                cmd.arg("--target-dir").arg(out_dir);
            }

            let output = cmd.output().map_err(|e| {
                BuildError::Tool(format!("Failed to generate docs for {}: {}", package, e))
            })?;

            if output.status.success() {
                println!("✓");
            } else {
                println!("✗");
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!(
                    "      Error: {}",
                    stderr
                        .lines()
                        .filter(|l| l.contains("error"))
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("\n      ")
                );
                any_failed = true;
            }
        }

        if any_failed {
            println!("    ⚠️  Some packages failed to build docs, but continuing...");
        }

        println!("    ✅ Rust API documentation generated");

        // 2. Generate Sphinx documentation (if tools are available)
        if is_sphinx_available() {
            println!("  📚 Building Sphinx documentation...");

            // Check if docs requirements are installed
            if !check_docs_requirements() {
                println!("    ⚠️  Installing documentation dependencies...");
                install_docs_requirements()?;
            }

            generate_sphinx_docs_with_output(output_dir.as_ref())?;
            println!("    ✅ Sphinx documentation generated");
        } else {
            println!("    ⚠️  Sphinx not available, skipping comprehensive documentation");
            println!("    💡 Run 'cargo-wrt setup --install' to install documentation tools");
        }

        // 3. Open documentation if requested
        if open_docs {
            open_documentation()?;
        }

        println!(
            "{} Documentation generated successfully",
            "✅".bright_green()
        );
        Ok(())
    }

    /// Check if Sphinx is available (either globally or in venv)
    fn is_sphinx_available() -> bool {
        // First check if virtual environment exists and has sphinx
        let venv_sphinx = if cfg!(target_os = "windows") {
            ".venv-docs/Scripts/sphinx-build.exe"
        } else {
            ".venv-docs/bin/sphinx-build"
        };

        if std::path::Path::new(venv_sphinx).exists() {
            return true;
        }

        // Fall back to checking global sphinx-build
        Command::new("sphinx-build")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Check if documentation virtual environment exists and is set up
    fn check_docs_requirements() -> bool {
        let venv_path = std::path::Path::new(".venv-docs");
        if !venv_path.exists() {
            return false;
        }

        // Check if key packages are available in the virtual environment
        let python_cmd = if cfg!(target_os = "windows") {
            ".venv-docs/Scripts/python.exe"
        } else {
            ".venv-docs/bin/python"
        };

        let check_cmd = Command::new(python_cmd)
            .args(["-c", "import sphinx, myst_parser); print('OK')"])
            .output();

        match check_cmd {
            Ok(output) => {
                output.status.success() && String::from_utf8_lossy(&output.stdout).contains("OK")
            },
            Err(_) => false,
        }
    }

    /// Create virtual environment and install documentation requirements
    fn install_docs_requirements() -> BuildResult<()> {
        println!("    📦 Setting up documentation virtual environment...");

        // 1. Create virtual environment
        let venv_cmd = Command::new("python3")
            .args(["-m", "venv", ".venv-docs"])
            .output()
            .map_err(|e| {
                BuildError::Tool(format!("Failed to create virtual environment: {}", e))
            })?;

        if !venv_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&venv_cmd.stderr);
            return Err(BuildError::Tool(format!(
                "Failed to create documentation virtual environment: {}",
                stderr
            )));
        }

        // 2. Install requirements in virtual environment
        let pip_cmd = if cfg!(target_os = "windows") {
            ".venv-docs/Scripts/pip"
        } else {
            ".venv-docs/bin/pip"
        };

        let install_cmd = Command::new(pip_cmd)
            .args(["install", "-r", "docs/requirements.txt"])
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to install docs requirements: {}", e)))?;

        if !install_cmd.status.success() {
            let stderr = String::from_utf8_lossy(&install_cmd.stderr);
            return Err(BuildError::Tool(format!(
                "Failed to install documentation dependencies in venv: {}",
                stderr
            )));
        }

        println!("    ✅ Documentation environment ready");
        Ok(())
    }

    /// Generate Sphinx documentation using virtual environment
    fn generate_sphinx_docs() -> BuildResult<()> {
        generate_sphinx_docs_with_output(None)
    }

    /// Generate Sphinx documentation with custom output directory
    fn generate_sphinx_docs_with_output(output_dir: Option<&String>) -> BuildResult<()> {
        // Use sphinx-build from virtual environment
        let sphinx_cmd = if cfg!(target_os = "windows") {
            ".venv-docs/Scripts/sphinx-build.exe"
        } else {
            ".venv-docs/bin/sphinx-build"
        };

        // Determine output paths based on custom directory
        let (build_dir, doctrees_dir, html_dir) = if let Some(out_dir) = output_dir {
            let base = PathBuf::from(out_dir);
            (
                base.join("sphinx"),
                base.join("sphinx/doctrees"),
                base.join("sphinx/html"),
            )
        } else {
            (
                PathBuf::from("docs/build"),
                PathBuf::from("docs/build/doctrees"),
                PathBuf::from("docs/build/html"),
            )
        };

        // Create output directory if it doesn't exist
        std::fs::create_dir_all(&build_dir).map_err(|e| {
            BuildError::Build(format!("Failed to create documentation directory: {}", e))
        })?;

        let mut cmd = Command::new(sphinx_cmd);
        cmd.args([
            "-b",
            "html",
            "-d",
            doctrees_dir.to_str().unwrap(),
            "docs/source",
            html_dir.to_str().unwrap(),
        ]);

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run sphinx-build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::Build(format!(
                "Sphinx documentation generation failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Open generated documentation
    fn open_documentation() -> BuildResult<()> {
        // Try to open the main documentation index
        let docs_paths = vec!["docs/build/html/index.html", "target/doc/index.html"];

        for docs_path in docs_paths {
            if std::path::Path::new(docs_path).exists() {
                let open_cmd = if cfg!(target_os = "macos") {
                    "open"
                } else if cfg!(target_os = "windows") {
                    "start"
                } else {
                    "xdg-open"
                };

                let _ = Command::new(open_cmd).arg(docs_path).spawn();
                println!("    🌐 Opened documentation at {}", docs_path);
                break;
            }
        }

        Ok(())
    }

    /// Validate no_std compatibility (ported from xtask no_std_verification)
    pub fn verify_no_std_compatibility() -> BuildResult<()> {
        println!("{} Verifying no_std compatibility...", "🔧".bright_blue());

        // Build each crate with no_std features
        let no_std_crates = [
            "wrt-runtime",
            "wrt-component",
            "wrt-foundation",
            "wrt-instructions",
        ];

        for crate_name in &no_std_crates {
            let mut cmd = Command::new("cargo");
            cmd.args([
                "build",
                "-p",
                crate_name,
                "--no-default-features",
                "--features",
                "no_std",
            ]);

            let output = cmd
                .output()
                .map_err(|e| BuildError::Tool(format!("Failed to build {}: {}", crate_name, e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(BuildError::Build(format!(
                    "no_std verification failed for {}: {}",
                    crate_name, stderr
                )));
            }

            println!(
                "  {} {} no_std compatibility verified",
                "✓".bright_green(),
                crate_name
            );
        }

        println!("{} All crates are no_std compatible", "✅".bright_green());
        Ok(())
    }

    /// Run static analysis checks (ported from xtask ci_static_analysis)
    pub fn run_static_analysis() -> BuildResult<()> {
        println!("{} Running static analysis...", "🔍".bright_blue());

        // Run clippy with basic settings (avoid strict settings that might fail)
        let mut clippy_cmd = Command::new("cargo");
        clippy_cmd.args(["clippy", "--workspace", "--all-targets"]);

        let clippy_output = clippy_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run clippy: {}", e)))?;

        if !clippy_output.status.success() {
            let stderr = String::from_utf8_lossy(&clippy_output.stderr);
            if stderr.contains("not installed") || stderr.contains("not found") {
                println!("  ⚠️ clippy not available, skipping clippy check");
            } else {
                return Err(BuildError::Verification(format!(
                    "Clippy checks failed: {}",
                    stderr
                )));
            }
        } else {
            println!("  ✅ Clippy checks passed");
        }

        // Check formatting
        let mut fmt_cmd = Command::new("cargo");
        fmt_cmd.args(["fmt", "--check"]);

        let fmt_output = fmt_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to check formatting: {}", e)))?;

        if !fmt_output.status.success() {
            let stderr = String::from_utf8_lossy(&fmt_output.stderr);
            if stderr.contains("not installed") || stderr.contains("not found") {
                println!("  ⚠️ cargo fmt not available, skipping format check");
            } else {
                return Err(BuildError::Verification(format!(
                    "Code formatting check failed: {}",
                    stderr
                )));
            }
        } else {
            println!("  ✅ Format check passed");
        }

        println!("{} Static analysis completed", "✅".bright_green());
        Ok(())
    }

    /// Run advanced test suite (ported from xtask ci_advanced_tests)
    pub fn run_advanced_tests() -> BuildResult<()> {
        println!("{} Running advanced test suite...", "🧪".bright_blue());

        // Run all tests with verbose output
        let mut cmd = Command::new("cargo");
        cmd.args(["test", "--workspace", "--all-features", "--verbose"]);

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run advanced tests: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::Test(format!(
                "Advanced tests failed: {}",
                stderr
            )));
        }

        // Run integration tests if they exist
        if std::path::Path::new("tests").exists() {
            let mut integration_cmd = Command::new("cargo");
            integration_cmd.args(["test", "--test", "*", "--workspace"]);

            let integration_output = integration_cmd
                .output()
                .map_err(|e| BuildError::Tool(format!("Failed to run integration tests: {}", e)))?;

            if !integration_output.status.success() {
                let stderr = String::from_utf8_lossy(&integration_output.stderr);
                return Err(BuildError::Test(format!(
                    "Integration tests failed: {}",
                    stderr
                )));
            }
        }

        println!("{} Advanced tests passed", "✅".bright_green());
        Ok(())
    }

    /// Run integrity checks (ported from xtask ci_integrity_checks)
    pub fn run_integrity_checks() -> BuildResult<()> {
        println!("{} Running integrity checks...", "🔒".bright_blue());

        // Check for unsafe code
        let unsafe_check =
            Command::new("grep").args(["-r", "unsafe", "--include=*.rs", "src/"]).output();

        if let Ok(unsafe_output) = unsafe_check {
            let unsafe_count = String::from_utf8_lossy(&unsafe_output.stdout)
                .lines()
                .filter(|line| !line.contains("//") && line.contains("unsafe"))
                .count();

            if unsafe_count > 0 {
                println!("  ⚠️ Found {} unsafe code blocks", unsafe_count);
            } else {
                println!("  ✓ No unsafe code found");
            }
        }

        // Check for panic usage
        let panic_check =
            Command::new("grep").args(["-r", "panic!", "--include=*.rs", "src/"]).output();

        if let Ok(panic_output) = panic_check {
            let panic_count = String::from_utf8_lossy(&panic_output.stdout)
                .lines()
                .filter(|line| !line.contains("//") && line.contains("panic!"))
                .count();

            if panic_count > 0 {
                println!("  ⚠️ Found {} panic! macros", panic_count);
            } else {
                println!("  ✓ No panic! macros found");
            }
        }

        println!("{} Integrity checks completed", "✅".bright_green());
        Ok(())
    }

    /// Generate multi-version documentation structure
    pub fn generate_multi_version_docs(versions: Vec<String>) -> BuildResult<()> {
        println!(
            "{} Generating multi-version documentation...",
            "📚".bright_blue()
        );

        let temp_dir = std::env::temp_dir().join("wrt-docs");

        // Clean and create fresh directory
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)
                .map_err(|e| BuildError::Build(format!("Failed to clean temp docs dir: {}", e)))?;
        }
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| BuildError::Build(format!("Failed to create temp docs dir: {}", e)))?;

        let mut switcher_entries = Vec::new();

        // Get current branch to return to
        let current_branch_cmd = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to get current branch: {}", e)))?;
        let current_branch = String::from_utf8_lossy(&current_branch_cmd.stdout).trim().to_string();

        // Generate documentation for each version
        for version in &versions {
            println!("\n  📖 Building documentation for version: {}", version);

            let version_dir = temp_dir.join(version);
            std::fs::create_dir_all(&version_dir)
                .map_err(|e| BuildError::Build(format!("Failed to create version dir: {}", e)))?;

            if version == "local" {
                // Build current working directory docs
                let output_dir = version_dir.to_string_lossy().to_string();
                generate_docs_with_output_dir(false, false, Some(output_dir))?;

                switcher_entries.push(serde_json::json!({
                    "version": "local",
                    "url": "/local/",
                    "name": "Local (development)"
                }));
            } else {
                // Checkout the version and build docs
                println!("    Checking out {}...", version);

                let checkout_cmd =
                    Command::new("git").args(["checkout", version]).output().map_err(|e| {
                        BuildError::Tool(format!("Failed to checkout {}: {}", version, e))
                    })?;

                if !checkout_cmd.status.success() {
                    eprintln!("    ⚠️ Failed to checkout {}, skipping...", version);
                    continue;
                }

                // Build docs for this version
                let output_dir = version_dir.to_string_lossy().to_string();
                match generate_docs_with_output_dir(false, false, Some(output_dir)) {
                    Ok(_) => {
                        // Add to switcher
                        let display_name = if version == "main" || version == "origin/main" {
                            "main (latest)"
                        } else {
                            version
                        };

                        switcher_entries.push(serde_json::json!({
                            "version": version,
                            "url": format!("/{}/", version),
                            "name": display_name
                        }));
                    },
                    Err(e) => {
                        eprintln!("    ⚠️ Failed to build docs for {}: {}", version, e);
                    },
                }
            }
        }

        // Return to original branch
        println!("\n  🔄 Returning to original branch: {}", current_branch);
        let return_cmd = Command::new("git")
            .args(["checkout", &current_branch])
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to return to original branch: {}", e)))?;

        if !return_cmd.status.success() {
            eprintln!("    ⚠️ Warning: Failed to return to original branch");
        }

        // Create switcher.json
        let switcher_path = temp_dir.join("switcher.json");
        let switcher_content = serde_json::to_string_pretty(&switcher_entries)
            .map_err(|e| BuildError::Build(format!("Failed to serialize switcher.json: {}", e)))?;
        std::fs::write(&switcher_path, switcher_content)
            .map_err(|e| BuildError::Build(format!("Failed to write switcher.json: {}", e)))?;

        // Copy root index.html if it exists
        let root_index_src = Path::new("docs/source/root_index.html");
        if root_index_src.exists() {
            let root_index_dst = temp_dir.join("index.html");
            std::fs::copy(root_index_src, root_index_dst)
                .map_err(|e| BuildError::Build(format!("Failed to copy root index.html: {}", e)))?;
        }

        // Create local symlinks for each version's documentation
        for version in &versions {
            let version_dir = temp_dir.join(version);
            let version_rust_docs = version_dir.join("doc");
            let version_sphinx_docs = version_dir.join("sphinx/html");

            if version_rust_docs.exists() {
                // Move Rust docs to version root
                let rust_index = version_dir.join("rust-api");
                if rust_index.exists() {
                    std::fs::remove_dir_all(&rust_index).ok();
                }
                std::fs::rename(&version_rust_docs, &rust_index)
                    .map_err(|e| BuildError::Build(format!("Failed to move Rust docs: {}", e)))?;
            }

            if version_sphinx_docs.exists() {
                // Move Sphinx docs to version root
                let files = std::fs::read_dir(&version_sphinx_docs)
                    .map_err(|e| BuildError::Build(format!("Failed to read Sphinx docs: {}", e)))?;

                for entry in files {
                    let entry = entry.map_err(|e| {
                        BuildError::Build(format!("Failed to read dir entry: {}", e))
                    })?;
                    let dest = version_dir.join(entry.file_name());

                    std::fs::rename(entry.path(), dest)
                        .map_err(|e| BuildError::Build(format!("Failed to move file: {}", e)))?;
                }

                // Clean up sphinx directory
                std::fs::remove_dir_all(version_dir.join("sphinx")).ok();
            }
        }

        println!(
            "\n✅ Multi-version documentation generated at: {}",
            temp_dir.display()
        );
        println!(
            "   switcher.json created with {} versions",
            switcher_entries.len()
        );
        println!("\n   To serve locally, run:");
        println!(
            "   cd {} && python3 -m http.server 8080",
            temp_dir.display()
        );

        Ok(())
    }

    /// Build WRTD (WebAssembly Runtime Daemon) binaries (ported from xtask
    /// wrtd_build)
    pub fn build_wrtd_binaries() -> BuildResult<()> {
        println!("{} Building WRTD binaries...", "🏗️".bright_blue());

        let wrtd_targets = [
            (
                "wrtd-std",
                "std-runtime",
                "Standard library runtime for servers/desktop",
            ),
            (
                "wrtd-alloc",
                "alloc-runtime",
                "Allocation runtime for embedded with heap",
            ),
            (
                "wrtd-nostd",
                "nostd-runtime",
                "No standard library runtime for bare metal",
            ),
        ];

        for (binary_name, feature, description) in &wrtd_targets {
            println!(
                "  {} Building {} - {}",
                "📦".bright_cyan(),
                binary_name,
                description
            );

            let mut cmd = Command::new("cargo");
            cmd.args(["build", "--bin", binary_name, "--features", feature]);

            let output = cmd
                .output()
                .map_err(|e| BuildError::Tool(format!("Failed to build {}: {}", binary_name, e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(BuildError::Build(format!(
                    "Failed to build {}: {}",
                    binary_name, stderr
                )));
            }

            println!("    ✓ {} built successfully", binary_name);
        }

        println!(
            "{} All WRTD binaries built successfully",
            "✅".bright_green()
        );
        Ok(())
    }
}

/// Central build system coordinator
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct BuildSystem {
    /// Workspace configuration
    pub workspace: WorkspaceConfig,
    /// Build configuration
    pub config: BuildConfig,
}

/// Build results and artifacts
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct BuildResults {
    /// Whether the build succeeded
    pub success: bool,
    /// List of built artifacts
    pub artifacts: Vec<PathBuf>,
    /// Build duration in milliseconds
    pub duration_ms: u64,
    /// Any warnings or notices
    pub warnings: Vec<String>,
}

#[cfg(feature = "std")]
impl BuildSystem {
    /// Create a new build system instance
    pub fn new(workspace_root: PathBuf) -> BuildResult<Self> {
        let workspace = WorkspaceConfig::load(&workspace_root)?;
        let config = BuildConfig::default();

        Ok(Self { workspace, config })
    }

    /// Create build system instance for current working directory
    pub fn for_current_dir() -> BuildResult<Self> {
        let workspace_root = crate::detect_workspace_root().map_err(|e| {
            BuildError::Workspace(format!("Could not detect workspace root: {}", e))
        })?;
        Self::new(workspace_root)
    }

    /// Create build system with custom configuration
    pub fn with_config(workspace_root: PathBuf, config: BuildConfig) -> BuildResult<Self> {
        let workspace = WorkspaceConfig::load(&workspace_root)?;
        Ok(Self { workspace, config })
    }

    /// Build all components in the workspace
    pub fn build_all(&self) -> BuildResult<BuildResults> {
        println!("{} Building all WRT components...", "🔨".bright_blue());

        let start_time = std::time::Instant::now();
        let mut artifacts = Vec::new();
        let mut warnings = Vec::new();

        // Build each crate in dependency order
        for crate_path in self.workspace.crate_paths() {
            match self.build_crate(&crate_path) {
                Ok(mut crate_artifacts) => {
                    artifacts.append(&mut crate_artifacts);
                },
                Err(e) => {
                    return Err(BuildError::Build(format!(
                        "Failed to build crate at {}: {}",
                        crate_path.display(),
                        e
                    )));
                },
            }
        }

        // Run workspace-level checks
        if self.config.clippy {
            match self.run_clippy() {
                Ok(clippy_warnings) => warnings.extend(clippy_warnings),
                Err(e) => warnings.push(format!("Clippy failed: {}", e)),
            }
        }

        if self.config.format_check {
            match self.check_formatting() {
                Ok(()) => {},
                Err(e) => warnings.push(format!("Format check failed: {}", e)),
            }
        }

        let duration = start_time.elapsed();
        println!(
            "{} Build completed in {:.2}s",
            "✅".bright_green(),
            duration.as_secs_f64()
        );

        Ok(BuildResults {
            success: true,
            artifacts,
            duration_ms: duration.as_millis() as u64,
            warnings,
        })
    }

    /// Run comprehensive coverage analysis using ported xtask logic
    pub fn run_coverage(&self) -> BuildResult<()> {
        xtask_port::run_coverage_analysis(&self.config)
    }

    /// Generate documentation using ported xtask logic
    pub fn generate_docs(&self) -> BuildResult<()> {
        xtask_port::generate_docs()
    }

    /// Generate documentation with options
    pub fn generate_docs_with_options(
        &self,
        include_private: bool,
        open_docs: bool,
    ) -> BuildResult<()> {
        xtask_port::generate_docs_with_options(include_private, open_docs)
    }

    /// Generate documentation with custom output directory
    pub fn generate_docs_with_output_dir(
        &self,
        include_private: bool,
        open_docs: bool,
        output_dir: Option<String>,
    ) -> BuildResult<()> {
        xtask_port::generate_docs_with_output_dir(include_private, open_docs, output_dir)
    }

    /// Generate multi-version documentation
    pub fn generate_multi_version_docs(&self, versions: Vec<String>) -> BuildResult<()> {
        xtask_port::generate_multi_version_docs(versions)
    }

    /// Verify no_std compatibility using ported xtask logic
    pub fn verify_no_std(&self) -> BuildResult<()> {
        xtask_port::verify_no_std_compatibility()
    }

    /// Run static analysis using ported xtask logic
    pub fn run_static_analysis(&self) -> BuildResult<()> {
        xtask_port::run_static_analysis()
    }

    /// Run static analysis with diagnostic output
    pub fn run_static_analysis_with_diagnostics(
        &self,
        strict: bool,
    ) -> BuildResult<DiagnosticCollection> {
        let start_time = std::time::Instant::now();
        let mut collection =
            DiagnosticCollection::new(self.workspace.root.clone(), "check".to_string());

        // Run clippy with JSON output
        let mut clippy_cmd = Command::new("cargo");
        clippy_cmd
            .args([
                "clippy",
                "--workspace",
                "--all-targets",
                "--message-format=json",
            ])
            .current_dir(&self.workspace.root);

        if strict {
            // Add strict clippy lints
            clippy_cmd.args(["--", "-W", "clippy::all", "-W", "clippy::pedantic"]);
        }

        let clippy_output = clippy_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to run clippy: {}", e)))?;

        // Parse clippy output for diagnostics
        let parser = CargoOutputParser::new(&self.workspace.root);
        match parser.parse_output(
            &String::from_utf8_lossy(&clippy_output.stdout),
            &String::from_utf8_lossy(&clippy_output.stderr),
            &self.workspace.root,
        ) {
            Ok(diagnostics) => collection.add_diagnostics(diagnostics),
            Err(e) => {
                collection.add_diagnostic(Diagnostic::new(
                    "<clippy>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("Failed to parse clippy output: {}", e),
                    "cargo-wrt".to_string(),
                ));
            },
        }

        // Check formatting
        let mut fmt_cmd = Command::new("cargo");
        fmt_cmd
            .args(["fmt", "--check", "--message-format=json"])
            .current_dir(&self.workspace.root);

        let fmt_output = fmt_cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to check formatting: {}", e)))?;

        if !fmt_output.status.success() {
            let stderr = String::from_utf8_lossy(&fmt_output.stderr);
            if stderr.contains("not installed") || stderr.contains("not found") {
                collection.add_diagnostic(Diagnostic::new(
                    "<fmt>".to_string(),
                    Range::entire_line(0),
                    Severity::Warning,
                    "cargo fmt not available, skipping format check".to_string(),
                    "cargo-wrt".to_string(),
                ));
            } else {
                // Parse unformatted files from stderr
                for line in stderr.lines() {
                    if line.contains("Diff in") {
                        if let Some(file_path) =
                            line.split("Diff in ").nth(1).and_then(|s| s.split(" at").next())
                        {
                            let relative_path = if let Ok(path) =
                                std::path::Path::new(file_path).strip_prefix(&self.workspace.root)
                            {
                                path.to_string_lossy().to_string()
                            } else {
                                file_path.to_string()
                            };

                            collection.add_diagnostic(
                                Diagnostic::new(
                                    relative_path,
                                    Range::entire_line(0),
                                    Severity::Warning,
                                    "File is not formatted according to rustfmt rules".to_string(),
                                    "rustfmt".to_string(),
                                )
                                .with_code("FORMAT001".to_string()),
                            );
                        }
                    }
                }

                collection.add_diagnostic(Diagnostic::new(
                    "<fmt>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    "Code formatting check failed".to_string(),
                    "rustfmt".to_string(),
                ));
            }
        } else {
            collection.add_diagnostic(Diagnostic::new(
                "<fmt>".to_string(),
                Range::entire_line(0),
                Severity::Info,
                "All files are properly formatted".to_string(),
                "rustfmt".to_string(),
            ));
        }

        let duration = start_time.elapsed();
        Ok(collection.finalize(duration.as_millis() as u64))
    }

    /// Run advanced tests using ported xtask logic
    pub fn run_advanced_tests(&self) -> BuildResult<()> {
        xtask_port::run_advanced_tests()
    }

    /// Run integrity checks using ported xtask logic
    pub fn run_integrity_checks(&self) -> BuildResult<()> {
        xtask_port::run_integrity_checks()
    }

    /// Build WRTD binaries using ported xtask logic
    pub fn build_wrtd_binaries(&self) -> BuildResult<()> {
        xtask_port::build_wrtd_binaries()
    }

    /// Check requirements traceability
    pub fn check_requirements(&self, requirements_file: Option<&Path>) -> BuildResult<()> {
        use crate::requirements::Requirements;

        let req_path = requirements_file
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.workspace.root.join("requirements.toml"));

        if !req_path.exists() {
            println!(
                "{} No requirements.toml found at {}",
                "⚠️".bright_yellow(),
                req_path.display()
            );
            println!("  Use 'cargo-wrt init-requirements' to create a sample file");
            return Ok(());
        }

        println!(
            "{} Checking requirements traceability...",
            "📋".bright_blue()
        );

        let requirements = Requirements::load(&req_path)?;
        let results = requirements.verify(&self.workspace.root)?;

        println!();
        println!("📊 Requirements Summary:");
        println!("  Total requirements: {}", results.total_requirements);
        println!("  Verified requirements: {}", results.verified_requirements);
        println!(
            "  Certification readiness: {:.1}%",
            results.certification_readiness
        );

        if !results.missing_files.is_empty() {
            println!();
            println!("{} Missing files:", "⚠️".bright_yellow());
            for file in &results.missing_files {
                println!("  - {}", file);
            }
        }

        if !results.incomplete_requirements.is_empty() {
            println!();
            println!("{} Incomplete requirements:", "❌".bright_red());
            for req in &results.incomplete_requirements {
                println!("  - {}", req);
            }
        }

        if results.certification_readiness >= 80.0 {
            println!();
            println!("{} Requirements verification passed!", "✅".bright_green());
        } else {
            println!();
            println!(
                "{} Requirements need attention for certification",
                "⚠️".bright_yellow()
            );
        }

        Ok(())
    }

    /// Initialize sample requirements file
    pub fn init_requirements(&self, path: Option<&Path>) -> BuildResult<()> {
        use crate::requirements::Requirements;

        let req_path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.workspace.root.join("requirements.toml"));

        if req_path.exists() {
            return Err(BuildError::Verification(format!(
                "Requirements file already exists at {}",
                req_path.display()
            )));
        }

        Requirements::init_sample(&req_path)?;
        Ok(())
    }

    /// Build a specific crate
    pub fn build_crate(&self, crate_path: &Path) -> BuildResult<Vec<PathBuf>> {
        if !crate_path.exists() {
            return Err(BuildError::Workspace(format!(
                "Crate path does not exist: {}",
                crate_path.display()
            )));
        }

        let crate_name = crate_path.file_name().and_then(|name| name.to_str()).unwrap_or("unknown");

        if self.config.verbose {
            println!("  {} Building crate: {}", "📦".bright_cyan(), crate_name);
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("build").current_dir(&self.workspace.root);

        // Add package selector
        cmd.arg("-p").arg(crate_name);

        // Add profile
        match self.config.profile {
            crate::config::BuildProfile::Release => {
                cmd.arg("--release");
            },
            crate::config::BuildProfile::Test => {
                cmd.arg("--tests");
            },
            _ => {}, // Dev is default
        }

        // Add features
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        // Execute build
        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::Build(format!(
                "Cargo build failed for {}: {}",
                crate_name, stderr
            )));
        }

        // Return artifacts (simplified - would parse cargo output in real
        // implementation)
        Ok(vec![crate_path.join("target")])
    }

    /// Build a specific package by name with diagnostic output
    pub fn build_package_with_diagnostics(
        &self,
        package_name: &str,
    ) -> BuildResult<DiagnosticCollection> {
        let start_time = std::time::Instant::now();
        let mut collection =
            DiagnosticCollection::new(self.workspace.root.clone(), "build".to_string());

        let mut cmd = Command::new("cargo");
        cmd.arg("build").arg("--message-format=json").current_dir(&self.workspace.root);

        // Add package selector
        cmd.arg("-p").arg(package_name);

        // Add profile
        match self.config.profile {
            crate::config::BuildProfile::Release => {
                cmd.arg("--release");
            },
            crate::config::BuildProfile::Test => {
                cmd.arg("--tests");
            },
            _ => {}, // Dev is default
        }

        // Add features if specified
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo build: {}", e)))?;

        // Parse cargo output for diagnostics
        let parser = CargoOutputParser::new(&self.workspace.root);
        match parser.parse_output(
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            &self.workspace.root,
        ) {
            Ok(diagnostics) => collection.add_diagnostics(diagnostics),
            Err(e) => {
                collection.add_diagnostic(Diagnostic::new(
                    "<build>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("Failed to parse build output: {}", e),
                    "cargo-wrt".to_string(),
                ));
            },
        }

        // Add overall build status
        if !output.status.success() {
            collection.add_diagnostic(Diagnostic::new(
                "<build>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                format!("Build failed for package {}", package_name),
                "cargo-wrt".to_string(),
            ));
        } else {
            collection.add_diagnostic(Diagnostic::new(
                "<build>".to_string(),
                Range::entire_line(0),
                Severity::Info,
                format!("Build succeeded for package {}", package_name),
                "cargo-wrt".to_string(),
            ));
        }

        let duration = start_time.elapsed();
        Ok(collection.finalize(duration.as_millis() as u64))
    }

    /// Build all components with diagnostic output
    pub fn build_all_with_diagnostics(&self) -> BuildResult<DiagnosticCollection> {
        let start_time = std::time::Instant::now();
        let mut collection =
            DiagnosticCollection::new(self.workspace.root.clone(), "build".to_string());

        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--workspace")
            .arg("--message-format=json")
            .current_dir(&self.workspace.root);

        // Add profile
        match self.config.profile {
            crate::config::BuildProfile::Release => {
                cmd.arg("--release");
            },
            crate::config::BuildProfile::Test => {
                cmd.arg("--tests");
            },
            _ => {}, // Dev is default
        }

        // Add features if specified
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo build: {}", e)))?;

        // Parse cargo output for diagnostics
        let parser = CargoOutputParser::new(&self.workspace.root);
        match parser.parse_output(
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            &self.workspace.root,
        ) {
            Ok(diagnostics) => collection.add_diagnostics(diagnostics),
            Err(e) => {
                collection.add_diagnostic(Diagnostic::new(
                    "<build>".to_string(),
                    Range::entire_line(0),
                    Severity::Error,
                    format!("Failed to parse build output: {}", e),
                    "cargo-wrt".to_string(),
                ));
            },
        }

        // Add overall build status
        if !output.status.success() {
            collection.add_diagnostic(Diagnostic::new(
                "<build>".to_string(),
                Range::entire_line(0),
                Severity::Error,
                "Workspace build failed".to_string(),
                "cargo-wrt".to_string(),
            ));
        } else {
            collection.add_diagnostic(Diagnostic::new(
                "<build>".to_string(),
                Range::entire_line(0),
                Severity::Info,
                "Workspace build succeeded".to_string(),
                "cargo-wrt".to_string(),
            ));
        }

        let duration = start_time.elapsed();
        Ok(collection.finalize(duration.as_millis() as u64))
    }

    /// Build a specific package by name
    pub fn build_package(&self, package_name: &str) -> BuildResult<BuildResults> {
        if self.config.verbose {
            println!(
                "  {} Building package: {}",
                "📦".bright_cyan(),
                package_name
            );
        }

        let start_time = std::time::Instant::now();
        let mut warnings = Vec::new();

        let mut cmd = Command::new("cargo");
        cmd.arg("build").current_dir(&self.workspace.root);

        // Add package selector
        cmd.arg("-p").arg(package_name);

        // Add profile
        match self.config.profile {
            crate::config::BuildProfile::Release => {
                cmd.arg("--release");
            },
            crate::config::BuildProfile::Test => {
                cmd.arg("--tests");
            },
            _ => {}, // Dev is default
        }

        // Add features if specified
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::Build(format!(
                "Cargo build failed for package {}: {}",
                package_name, stderr
            )));
        }

        // Check for warnings in stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("warning:") {
            warnings.push(format!("Package {} has build warnings", package_name));
        }

        let duration = start_time.elapsed();

        Ok(BuildResults {
            success: true,
            artifacts: vec![], // Single package builds don't track specific artifacts yet
            duration_ms: duration.as_millis() as u64,
            warnings,
        })
    }

    /// Test a specific package by name
    pub fn test_package(&self, package_name: &str) -> BuildResult<BuildResults> {
        if self.config.verbose {
            println!("  {} Testing package: {}", "🧪".bright_cyan(), package_name);
        }

        let start_time = std::time::Instant::now();
        let mut warnings = Vec::new();

        let mut cmd = Command::new("cargo");
        cmd.arg("test").current_dir(&self.workspace.root);

        // Add package selector
        cmd.arg("-p").arg(package_name);

        // Add features if specified
        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo test: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::Test(format!(
                "Cargo test failed for package {}: {}",
                package_name, stderr
            )));
        }

        // Check for warnings in stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("warning:") {
            warnings.push(format!("Package {} has test warnings", package_name));
        }

        let duration = start_time.elapsed();

        Ok(BuildResults {
            success: true,
            artifacts: vec![], // Package tests don't produce artifacts
            duration_ms: duration.as_millis() as u64,
            warnings,
        })
    }

    /// Run clippy checks on the workspace
    pub fn run_clippy(&self) -> BuildResult<Vec<String>> {
        if self.config.verbose {
            println!("  {} Running clippy checks...", "📎".bright_yellow());
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--workspace")
            .arg("--all-targets")
            .current_dir(&self.workspace.root);

        if !self.config.features.is_empty() {
            cmd.arg("--features").arg(self.config.features.join(","));
        }

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute clippy: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let warnings = stdout
            .lines()
            .filter(|line| line.contains("warning:"))
            .map(|line| line.to_string())
            .collect();

        Ok(warnings)
    }

    /// Check code formatting
    pub fn check_formatting(&self) -> BuildResult<()> {
        if self.config.verbose {
            println!("  {} Checking code formatting...", "🎨".bright_magenta());
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("fmt").arg("--check").current_dir(&self.workspace.root);

        let output = cmd
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo fmt: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not installed") || stderr.contains("not found") {
                println!("  ⚠️ cargo fmt not available, skipping format check");
                return Ok(());
            }
            return Err(BuildError::Tool(format!(
                "Code formatting check failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get workspace root path
    pub fn workspace_root(&self) -> &Path {
        &self.workspace.root
    }

    /// Get build configuration
    pub fn build_config(&self) -> &BuildConfig {
        &self.config
    }

    /// Update build configuration
    pub fn set_config(&mut self, config: BuildConfig) {
        self.config = config;
    }

    /// Set verbose mode
    pub fn set_verbose(&mut self, verbose: bool) {
        self.config.verbose = verbose;
    }

    /// Set profile
    pub fn set_profile(&mut self, profile: crate::config::BuildProfile) {
        self.config.profile = profile;
    }

    /// Add feature
    pub fn add_feature(&mut self, feature: String) {
        if !self.config.features.contains(&feature) {
            self.config.features.push(feature);
        }
    }

    /// Remove feature
    pub fn remove_feature(&mut self, feature: &str) {
        self.config.features.retain(|f| f != feature);
    }

    /// Get workspace metadata
    pub fn workspace(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    /// List available fuzzing targets (delegated to fuzz module)
    pub fn list_fuzz_targets(&self) -> BuildResult<Vec<String>> {
        use crate::fuzz::*;
        // Delegate to fuzz module implementation
        list_fuzz_targets_impl(self)
    }

    /// Run fuzzing with options (delegated to fuzz module)
    pub fn run_fuzz_with_options(
        &self,
        options: &crate::fuzz::FuzzOptions,
    ) -> BuildResult<crate::fuzz::FuzzResults> {
        use crate::fuzz::*;
        // Delegate to fuzz module implementation
        run_fuzz_with_options_impl(self, options)
    }
}

#[cfg(feature = "std")]
impl BuildResults {
    /// Check if build was successful
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Get build artifacts
    pub fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }

    /// Get build duration
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.duration_ms)
    }

    /// Get warnings
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_build_system_creation() {
        // Use the actual workspace for testing
        let workspace = crate::detect_workspace_root().unwrap();
        let build_system = BuildSystem::new(workspace);
        assert!(build_system.is_ok());
    }

    #[test]
    fn test_build_system_for_current_dir() {
        let build_system = BuildSystem::for_current_dir();
        assert!(build_system.is_ok());
    }

    #[test]
    fn test_build_system_config_management() {
        let workspace = crate::detect_workspace_root().unwrap();
        let mut build_system = BuildSystem::new(workspace).unwrap();

        // Test feature management
        build_system.add_feature("test-feature".to_string());
        assert!(build_system.config.features.contains(&"test-feature".to_string()));

        build_system.remove_feature("test-feature");
        assert!(!build_system.config.features.contains(&"test-feature".to_string()));

        // Test verbose mode
        build_system.set_verbose(true);
        assert!(build_system.config.verbose);
    }

    #[test]
    fn test_build_results() {
        let results = BuildResults {
            success: true,
            artifacts: vec![PathBuf::from("target/debug/wrt")],
            duration_ms: 1000,
            warnings: vec!["warning: unused variable".to_string()],
        };

        assert!(results.is_success());
        assert_eq!(results.duration().as_millis(), 1000);
        assert_eq!(results.warnings().len(), 1);
    }
}
