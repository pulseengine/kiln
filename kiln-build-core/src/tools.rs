//! External tool detection and management
//!
//! This module provides functionality to detect available external tools,
//! provide helpful error messages when tools are missing, and guide users
//! through the setup process.

use std::{collections::HashMap, process::Command};

use colored::Colorize;

use crate::{
    error::{BuildError, BuildResult},
    tool_versions::{ToolVersionConfig, VersionComparison, extract_version_from_output},
};

/// Information about an external tool
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Name of the tool/command
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// Installation command or instructions
    pub install_command: String,
    /// Whether this tool is required for basic functionality
    pub required: bool,
    /// Which cargo-wrt commands need this tool
    pub used_by: Vec<String>,
}

/// Tool detection results
#[derive(Debug)]
pub struct ToolStatus {
    /// Whether the tool is available
    pub available: bool,
    /// Version string if available
    pub version: Option<String>,
    /// Error message if detection failed
    pub error: Option<String>,
    /// Version compatibility status
    pub version_status: VersionStatus,
    /// Whether tool needs to be installed/updated
    pub needs_action: bool,
}

/// Version compatibility status
#[derive(Debug, Clone)]
pub enum VersionStatus {
    /// Version meets requirements
    Compatible,
    /// Version is too old
    TooOld {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
    /// Version is newer than required (warning)
    Newer {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
    /// Exact version mismatch
    Mismatch {
        /// Currently installed version
        installed: String,
        /// Required version
        required: String,
    },
    /// Version could not be determined
    Unknown,
    /// No version requirement specified
    NoRequirement,
}

/// Tool manager for detecting and validating external dependencies
#[derive(Debug)]
pub struct ToolManager {
    /// Map of tool name to tool info
    tools: HashMap<String, ToolInfo>,
    /// Version configuration
    version_config: ToolVersionConfig,
}

impl ToolManager {
    /// Create a new tool manager with default tool definitions
    pub fn new() -> Self {
        let version_config = ToolVersionConfig::load_or_default();
        let mut tools = HashMap::new();

        // Core tools (should always be available)
        tools.insert(
            "cargo".to_string(),
            ToolInfo {
                name: "cargo".to_string(),
                description: "Rust package manager".to_string(),
                install_command: "Install Rust from https://rustup.rs/".to_string(),
                required: true,
                used_by: vec!["build", "test", "check", "clean"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            },
        );

        tools.insert(
            "rustc".to_string(),
            ToolInfo {
                name: "rustc".to_string(),
                description: "Rust compiler".to_string(),
                install_command: "Install Rust from https://rustup.rs/".to_string(),
                required: true,
                used_by: vec!["build", "test"].into_iter().map(String::from).collect(),
            },
        );

        // Rust toolchain components (managed via rustup)
        tools.insert(
            "clippy".to_string(),
            ToolInfo {
                name: "clippy".to_string(),
                description: "Rust linter for code quality checks".to_string(),
                install_command: "rustup component add clippy".to_string(),
                required: false,
                used_by: vec!["check", "ci"].into_iter().map(String::from).collect(),
            },
        );

        tools.insert(
            "rustfmt".to_string(),
            ToolInfo {
                name: "rustfmt".to_string(),
                description: "Rust code formatter".to_string(),
                install_command: "rustup component add rustfmt".to_string(),
                required: false,
                used_by: vec!["check", "ci"].into_iter().map(String::from).collect(),
            },
        );

        // Optional tools for advanced features
        tools.insert(
            "kani".to_string(),
            ToolInfo {
                name: "kani".to_string(),
                description: "Formal verification tool for Rust".to_string(),
                install_command: "cargo install --locked kani-verifier && cargo kani setup"
                    .to_string(),
                required: false,
                used_by: vec!["kani-verify", "verify"].into_iter().map(String::from).collect(),
            },
        );

        tools.insert(
            "cargo-fuzz".to_string(),
            ToolInfo {
                name: "cargo-fuzz".to_string(),
                description: "Fuzzing tool for Rust".to_string(),
                install_command: "cargo install cargo-fuzz".to_string(),
                required: false,
                used_by: vec!["fuzz"].into_iter().map(String::from).collect(),
            },
        );

        tools.insert(
            "git".to_string(),
            ToolInfo {
                name: "git".to_string(),
                description: "Version control system".to_string(),
                install_command: "Install Git from https://git-scm.com/".to_string(),
                required: false,
                used_by: vec!["setup"].into_iter().map(String::from).collect(),
            },
        );

        // Documentation tools
        tools.insert(
            "python3".to_string(),
            ToolInfo {
                name: "python3".to_string(),
                description: "Python interpreter for documentation".to_string(),
                install_command: "Install Python from https://python.org or via package manager"
                    .to_string(),
                required: false,
                used_by: vec!["docs"].into_iter().map(String::from).collect(),
            },
        );

        tools.insert(
            "python-venv".to_string(),
            ToolInfo {
                name: "python-venv".to_string(),
                description: "Python virtual environment support".to_string(),
                install_command: "Included with Python 3.8+ - install Python if missing"
                    .to_string(),
                required: false,
                used_by: vec!["docs"].into_iter().map(String::from).collect(),
            },
        );

        Self {
            tools,
            version_config,
        }
    }

    /// Check if a specific tool is available and version compatible
    pub fn check_tool(&self, tool_name: &str) -> ToolStatus {
        self.check_tool_for_target(tool_name, None)
    }

    /// Check if a specific tool is available and version compatible for a
    /// target
    pub fn check_tool_for_target(&self, tool_name: &str, target: Option<&str>) -> ToolStatus {
        // First check if the target is supported for this tool
        if let Some(target) = target {
            if !self.version_config.is_target_supported(tool_name, target) {
                return ToolStatus {
                    available: false,
                    version: None,
                    error: Some(format!(
                        "Target '{}' not supported for tool '{}'",
                        target, tool_name
                    )),
                    version_status: VersionStatus::Unknown,
                    needs_action: true,
                };
            }
        }

        let basic_status = match tool_name {
            "cargo" => self.check_cargo(),
            "rustc" => self.check_rustc_for_target(target),
            "clippy" => self.check_clippy_for_target(target),
            "rustfmt" => self.check_rustfmt_for_target(target),
            "kani" => self.check_kani(),
            "cargo-fuzz" => self.check_cargo_fuzz(),
            "git" => self.check_git(),
            "python3" => self.check_python3(),
            "python-venv" => self.check_python_venv(),
            "aarch64-unknown-linux-gnu" => self.check_rustup_target("aarch64-unknown-linux-gnu"),
            "x86_64-unknown-linux-gnu" => self.check_rustup_target("x86_64-unknown-linux-gnu"),
            "thumbv7em-none-eabihf" => self.check_rustup_target("thumbv7em-none-eabihf"),
            "riscv32imac-unknown-none-elf" => {
                self.check_rustup_target("riscv32imac-unknown-none-elf")
            },
            "riscv64gc-unknown-none-elf" => self.check_rustup_target("riscv64gc-unknown-none-elf"),
            _ => ToolStatus {
                available: false,
                version: None,
                error: Some(format!("Unknown tool: {}", tool_name)),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        };

        // Enhance with version checking (target-aware)
        self.enhance_with_version_check_for_target(tool_name, target, basic_status)
    }

    /// Enhance tool status with version compatibility checking
    fn enhance_with_version_check(&self, tool_name: &str, mut status: ToolStatus) -> ToolStatus {
        self.enhance_with_version_check_for_target(tool_name, None, status)
    }

    /// Enhance tool status with target-aware version compatibility checking
    fn enhance_with_version_check_for_target(
        &self,
        tool_name: &str,
        target: Option<&str>,
        mut status: ToolStatus,
    ) -> ToolStatus {
        if !status.available {
            return status;
        }

        // Get version requirement for this tool (target-aware)
        let version_spec = match self.version_config.get_tool_version_for_target(tool_name, target)
        {
            Some(spec) => spec,
            None => {
                status.version_status = VersionStatus::NoRequirement;
                return status;
            },
        };

        // Extract actual version from the tool output
        let actual_version = match &status.version {
            Some(version_output) => {
                if let Some(pattern) = &version_spec.version_pattern {
                    extract_version_from_output(version_output, pattern)
                } else {
                    Some(version_output.clone())
                }
            },
            None => None,
        };

        // Check version compatibility
        let (version_status, needs_action) = match actual_version {
            Some(version) => {
                match self.version_config.check_version_compatibility(tool_name, &version) {
                    Some(VersionComparison::Satisfies) => (VersionStatus::Compatible, false),
                    Some(VersionComparison::TooOld {
                        installed,
                        required,
                    }) => (
                        VersionStatus::TooOld {
                            installed,
                            required,
                        },
                        true,
                    ),
                    Some(VersionComparison::Newer {
                        installed,
                        required,
                    }) => (
                        VersionStatus::Newer {
                            installed,
                            required,
                        },
                        false,
                    ),
                    Some(VersionComparison::Mismatch {
                        installed,
                        required,
                    }) => (
                        VersionStatus::Mismatch {
                            installed,
                            required,
                        },
                        true,
                    ),
                    None => (VersionStatus::Unknown, false),
                }
            },
            None => (VersionStatus::Unknown, false),
        };

        status.version_status = version_status;
        status.needs_action = needs_action;
        status
    }

    /// Check all tools and return a summary
    pub fn check_all_tools(&self) -> HashMap<String, ToolStatus> {
        let mut results = HashMap::new();

        for tool_name in self.tools.keys() {
            results.insert(tool_name.clone(), self.check_tool(tool_name));
        }

        results
    }

    /// Generate a helpful error message when a tool is missing
    pub fn generate_missing_tool_error(&self, tool_name: &str, command: &str) -> BuildError {
        if let Some(tool_info) = self.tools.get(tool_name) {
            let message = format!(
                "❌ {} is required for the '{}' command but is not installed.\n\n📝 {}: {}\n\n💿 \
                 To install:\n{}\n\n💡 After installation, you can verify it works with:\n{} \
                 --version\n\n🔧 You can also run 'cargo-wrt setup --all' to install recommended \
                 tools.",
                tool_name.bright_red(),
                command.bright_cyan(),
                "Description".bright_blue(),
                tool_info.description,
                tool_info.install_command.bright_green(),
                tool_name.bright_yellow()
            );
            BuildError::Tool(message)
        } else {
            BuildError::Tool(format!(
                "Unknown tool '{}' required for command '{}'",
                tool_name, command
            ))
        }
    }

    /// Print a tool status report
    pub fn print_tool_status(&self) {
        println!("{} Tool Status Report", "🔧".bright_blue());
        println!();

        let results = self.check_all_tools();

        // Required tools
        println!("{}", "Required Tools:".bright_yellow());
        for (tool_name, tool_info) in &self.tools {
            if tool_info.required {
                if let Some(status) = results.get(tool_name) {
                    let status_icon = if status.available { "✅" } else { "❌" };
                    let (version_info, status_detail) = self.format_version_status(&status);

                    println!(
                        "  {} {} - {}{}{}",
                        status_icon,
                        tool_name.bright_cyan(),
                        tool_info.description,
                        status_detail,
                        version_info.bright_black()
                    );
                }
            }
        }

        println!();
        println!("{}", "Optional Tools:".bright_yellow());
        for (tool_name, tool_info) in &self.tools {
            if !tool_info.required {
                if let Some(status) = results.get(tool_name) {
                    let status_icon = if status.available { "✅" } else { "⚠️" };
                    let (version_info, status_detail) = self.format_version_status(&status);

                    println!(
                        "  {} {} - {}{}{}",
                        status_icon,
                        tool_name.bright_cyan(),
                        tool_info.description,
                        status_detail,
                        version_info.bright_black()
                    );

                    if !status.available {
                        println!(
                            "      💿 Install: {}",
                            tool_info.install_command.bright_green()
                        );
                        println!(
                            "      📋 Used by: {}",
                            tool_info.used_by.join(", ").bright_magenta()
                        );
                    }
                }
            }
        }

        println!();
    }

    /// Get tool info for a specific tool
    pub fn get_tool_info(&self, tool_name: &str) -> Option<&ToolInfo> {
        self.tools.get(tool_name)
    }

    /// Format version status for display
    fn format_version_status(&self, status: &ToolStatus) -> (String, String) {
        match (&status.version, &status.version_status) {
            (Some(version), VersionStatus::Compatible) => {
                (format!(" ({})", version), " ✅".bright_green().to_string())
            },
            (
                Some(version),
                VersionStatus::TooOld {
                    installed: _,
                    required,
                },
            ) => (
                format!(" ({})", version),
                format!(" ⚠️ → {}", required).bright_yellow().to_string(),
            ),
            (
                Some(version),
                VersionStatus::Newer {
                    installed: _,
                    required,
                },
            ) => (
                format!(" ({})", version),
                format!(" ⬆️ (need {})", required).bright_blue().to_string(),
            ),
            (
                Some(version),
                VersionStatus::Mismatch {
                    installed: _,
                    required,
                },
            ) => (
                format!(" ({})", version),
                format!(" ❌ → {}", required).bright_red().to_string(),
            ),
            (Some(version), VersionStatus::NoRequirement) => {
                (format!(" ({})", version), "".to_string())
            },
            (Some(version), VersionStatus::Unknown) => {
                (format!(" ({})", version), " ❓".bright_black().to_string())
            },
            (None, _) => ("".to_string(), "".to_string()),
        }
    }

    /// Install or update a tool if needed
    pub fn install_tool_if_needed(&self, tool_name: &str) -> BuildResult<bool> {
        let status = self.check_tool(tool_name);

        // If tool is available and version is compatible, skip installation
        if status.available && !status.needs_action {
            match &status.version_status {
                VersionStatus::Compatible => {
                    println!(
                        "  ✅ {} is already installed with compatible version",
                        tool_name.bright_cyan()
                    );
                    return Ok(false); // No action needed
                },
                VersionStatus::Newer {
                    installed,
                    required,
                } => {
                    println!(
                        "  ⚠️  {} has newer version {} (required: {})",
                        tool_name.bright_cyan(),
                        installed.bright_yellow(),
                        required.bright_blue()
                    );
                    return Ok(false); // No action needed, newer is fine
                },
                _ => {}, // Continue with installation
            }
        }

        // Get installation command from version config
        let install_cmd = match self.version_config.get_install_command(tool_name) {
            Some(cmd) => cmd,
            None => {
                return Err(BuildError::Tool(format!(
                    "No installation command configured for tool '{}'",
                    tool_name
                )));
            },
        };

        match &status.version_status {
            VersionStatus::TooOld {
                installed,
                required,
            } => {
                println!(
                    "  🔄 Updating {} from {} to {}",
                    tool_name.bright_cyan(),
                    installed.bright_red(),
                    required.bright_green()
                );
            },
            VersionStatus::Mismatch {
                installed,
                required,
            } => {
                println!(
                    "  🔄 Replacing {} version {} with {}",
                    tool_name.bright_cyan(),
                    installed.bright_red(),
                    required.bright_green()
                );
            },
            _ => {
                println!(
                    "  📦 Installing {} for {}",
                    tool_name.bright_cyan(),
                    "build system functionality".bright_blue()
                );
            },
        }

        // Execute installation command
        if install_cmd.starts_with("cargo install") {
            self.execute_cargo_install(tool_name, install_cmd)
        } else if install_cmd.starts_with("rustup") {
            self.execute_rustup_command(install_cmd)
        } else {
            println!(
                "    💡 Manual installation required: {}",
                install_cmd.bright_yellow()
            );
            Ok(false)
        }
    }

    /// Execute cargo install command
    fn execute_cargo_install(&self, tool_name: &str, install_cmd: &str) -> BuildResult<bool> {
        let args: Vec<&str> = install_cmd.split_whitespace().skip(1).collect(); // Skip "cargo"

        let output = std::process::Command::new("cargo")
            .args(&args)
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute cargo install: {}", e)))?;

        if output.status.success() {
            println!("    ✅ {} installed successfully", tool_name.bright_green());

            // Run additional setup if needed (e.g., kani setup)
            if tool_name == "kani" && install_cmd.contains("kani setup") {
                println!("    🔧 Running kani setup...");
                let setup_output = std::process::Command::new("cargo")
                    .args(["kani", "setup"])
                    .output()
                    .map_err(|e| BuildError::Tool(format!("Failed to run kani setup: {}", e)))?;

                if setup_output.status.success() {
                    println!("    ✅ Kani setup completed");
                } else {
                    println!(
                        "    ⚠️  Kani setup had issues: {}",
                        String::from_utf8_lossy(&setup_output.stderr).bright_yellow()
                    );
                }
            }

            Ok(true)
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            Err(BuildError::Tool(format!(
                "Failed to install {}: {}",
                tool_name, error_msg
            )))
        }
    }

    /// Execute rustup command
    fn execute_rustup_command(&self, install_cmd: &str) -> BuildResult<bool> {
        let args: Vec<&str> = install_cmd.split_whitespace().skip(1).collect(); // Skip "rustup"

        let output = std::process::Command::new("rustup")
            .args(&args)
            .output()
            .map_err(|e| BuildError::Tool(format!("Failed to execute rustup: {}", e)))?;

        if output.status.success() {
            println!("    ✅ Rustup command executed successfully");
            Ok(true)
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            Err(BuildError::Tool(format!(
                "Failed to execute rustup command: {}",
                error_msg
            )))
        }
    }

    /// Install all tools that need updates
    pub fn install_all_needed_tools(&self) -> BuildResult<()> {
        let managed_tools = self.version_config.get_managed_tools();
        let mut installed_any = false;

        for tool_name in managed_tools {
            match self.install_tool_if_needed(tool_name) {
                Ok(true) => {
                    installed_any = true;
                },
                Ok(false) => {
                    // Tool was already compatible, continue
                },
                Err(e) => {
                    println!("  ❌ Failed to install {}: {}", tool_name.bright_red(), e);
                    // Continue with other tools instead of failing completely
                },
            }
        }

        if installed_any {
            println!();
            println!("🔄 Verifying installations...");
            self.print_tool_status();
        }

        Ok(())
    }

    // Individual tool check methods

    fn check_cargo(&self) -> ToolStatus {
        self.check_command_version("cargo", &["--version"])
    }

    fn check_rustc(&self) -> ToolStatus {
        self.check_rustc_for_target(None)
    }

    fn check_rustc_for_target(&self, target: Option<&str>) -> ToolStatus {
        let mut args = vec!["--version"];

        // Check if target is installed if specified
        if let Some(target) = target {
            if !self.version_config.check_rustup_target_installed(target) {
                return ToolStatus {
                    available: false,
                    version: None,
                    error: Some(format!("Target '{}' not installed", target)),
                    version_status: VersionStatus::Unknown,
                    needs_action: true,
                };
            }
        }

        self.check_command_version("rustc", &args)
    }

    fn check_clippy(&self) -> ToolStatus {
        self.check_clippy_for_target(None)
    }

    fn check_clippy_for_target(&self, target: Option<&str>) -> ToolStatus {
        // Check if target is installed if specified
        if let Some(target) = target {
            if !self.version_config.check_rustup_target_installed(target) {
                return ToolStatus {
                    available: false,
                    version: None,
                    error: Some(format!("Target '{}' not installed for clippy", target)),
                    version_status: VersionStatus::Unknown,
                    needs_action: true,
                };
            }
        }

        // clippy is called via `cargo clippy`
        let output = Command::new("cargo").args(["clippy", "--version"]).output();

        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                ToolStatus {
                    available: true,
                    version: Some(version),
                    error: None,
                    version_status: VersionStatus::Unknown, /* Will be set by
                                                             * enhance_with_version_check */
                    needs_action: false,
                }
            },
            Ok(output) => ToolStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
            Err(e) => ToolStatus {
                available: false,
                version: None,
                error: Some(e.to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        }
    }

    fn check_rustfmt(&self) -> ToolStatus {
        self.check_rustfmt_for_target(None)
    }

    fn check_rustfmt_for_target(&self, target: Option<&str>) -> ToolStatus {
        // Check if target is installed if specified
        if let Some(target) = target {
            if !self.version_config.check_rustup_target_installed(target) {
                return ToolStatus {
                    available: false,
                    version: None,
                    error: Some(format!("Target '{}' not installed for rustfmt", target)),
                    version_status: VersionStatus::Unknown,
                    needs_action: true,
                };
            }
        }

        // rustfmt is called via `cargo fmt`
        let output = Command::new("cargo").args(["fmt", "--version"]).output();

        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                ToolStatus {
                    available: true,
                    version: Some(version),
                    error: None,
                    version_status: VersionStatus::Unknown, /* Will be set by
                                                             * enhance_with_version_check */
                    needs_action: false,
                }
            },
            Ok(output) => ToolStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
            Err(e) => ToolStatus {
                available: false,
                version: None,
                error: Some(e.to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        }
    }

    fn check_kani(&self) -> ToolStatus {
        self.check_command_version("kani", &["--version"])
    }

    fn check_cargo_fuzz(&self) -> ToolStatus {
        // cargo-fuzz is called via `cargo +nightly fuzz`
        let output = Command::new("cargo").args(["+nightly", "fuzz", "--help"]).output();

        match output {
            Ok(output) if output.status.success() => ToolStatus {
                available: true,
                version: Some("available".to_string()),
                error: None,
                version_status: VersionStatus::Unknown, // Will be enhanced by version check
                needs_action: false,
            },
            Ok(output) => ToolStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
            Err(e) => ToolStatus {
                available: false,
                version: None,
                error: Some(e.to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        }
    }

    fn check_git(&self) -> ToolStatus {
        self.check_command_version("git", &["--version"])
    }

    fn check_python3(&self) -> ToolStatus {
        self.check_command_version("python3", &["--version"])
    }

    fn check_python_venv(&self) -> ToolStatus {
        let output = Command::new("python3").args(["-m", "venv", "--help"]).output();

        match output {
            Ok(output) if output.status.success() => ToolStatus {
                available: true,
                version: Some("available".to_string()),
                error: None,
                version_status: VersionStatus::Unknown, // Will be enhanced by version check
                needs_action: false,
            },
            Ok(output) => ToolStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
            Err(e) => ToolStatus {
                available: false,
                version: None,
                error: Some(e.to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        }
    }

    fn check_rustup_target(&self, target: &str) -> ToolStatus {
        // Check if rustup target is installed
        if self.version_config.check_rustup_target_installed(target) {
            ToolStatus {
                available: true,
                version: Some("installed".to_string()),
                error: None,
                version_status: VersionStatus::Compatible,
                needs_action: false,
            }
        } else {
            ToolStatus {
                available: false,
                version: None,
                error: Some(format!("Target '{}' not installed", target)),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            }
        }
    }

    /// Helper method to check a command and extract version
    fn check_command_version(&self, command: &str, args: &[&str]) -> ToolStatus {
        let output = Command::new(command).args(args).output();

        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                ToolStatus {
                    available: true,
                    version: Some(version),
                    error: None,
                    version_status: VersionStatus::Unknown, /* Will be set by
                                                             * enhance_with_version_check */
                    needs_action: false,
                }
            },
            Ok(output) => ToolStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
            Err(e) => ToolStatus {
                available: false,
                version: None,
                error: Some(e.to_string()),
                version_status: VersionStatus::Unknown,
                needs_action: true,
            },
        }
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a tool is available before using it
pub fn ensure_tool_available(tool_name: &str, command: &str) -> BuildResult<()> {
    let manager = ToolManager::new();
    let status = manager.check_tool(tool_name);

    if !status.available {
        return Err(manager.generate_missing_tool_error(tool_name, command));
    }

    Ok(())
}

/// Check if Kani is available (commonly used check)
pub fn is_kani_available() -> bool {
    let manager = ToolManager::new();
    manager.check_tool("kani").available
}

/// Check if cargo-fuzz is available
pub fn is_cargo_fuzz_available() -> bool {
    let manager = ToolManager::new();
    manager.check_tool("cargo-fuzz").available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_manager_creation() {
        let manager = ToolManager::new();
        assert!(manager.tools.contains_key("cargo"));
        assert!(manager.tools.contains_key("rustc"));
        assert!(manager.tools.contains_key("kani"));
    }

    #[test]
    fn test_tool_status_check() {
        let manager = ToolManager::new();

        // Cargo should be available in any Rust environment
        let cargo_status = manager.check_tool("cargo");
        assert!(cargo_status.available);
        assert!(cargo_status.version.is_some());
    }

    #[test]
    fn test_unknown_tool() {
        let manager = ToolManager::new();
        let status = manager.check_tool("nonexistent-tool");
        assert!(!status.available);
        assert!(status.error.is_some());
    }
}
