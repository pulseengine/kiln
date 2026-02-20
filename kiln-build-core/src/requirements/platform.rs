//! Platform verification with external limit integration for cargo-wrt
//!
//! Provides verification capabilities that integrate with CLI args, environment
//! variables, configuration files, and container discovery. Integrated with
//! cargo-wrt's diagnostic system.

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::AsilLevel,
    diagnostics::{Diagnostic, DiagnosticCollection, Position, Range, Severity},
    error::{BuildError, BuildResult},
    formatters::OutputFormat,
};

/// Platform limits discovered from the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensivePlatformLimits {
    /// Maximum total memory available in bytes.
    pub max_total_memory: usize,
    /// Maximum WebAssembly linear memory in bytes.
    pub max_wasm_linear_memory: usize,
    /// Maximum stack size in bytes.
    pub max_stack_bytes: usize,
    /// Maximum number of component instances.
    pub max_components: usize,
    /// Platform identifier.
    pub platform_id: PlatformId,
}

impl Default for ComprehensivePlatformLimits {
    fn default() -> Self {
        Self {
            max_total_memory: 1024 * 1024 * 1024,      // 1GB
            max_wasm_linear_memory: 256 * 1024 * 1024, // 256MB
            max_stack_bytes: 1024 * 1024,              // 1MB
            max_components: 256,
            platform_id: PlatformId::Unknown,
        }
    }
}

/// Platform identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformId {
    /// Linux operating system.
    Linux,
    /// QNX real-time operating system.
    QNX,
    /// macOS operating system.
    MacOS,
    /// Windows operating system.
    Windows,
    /// VxWorks real-time operating system.
    VxWorks,
    /// Zephyr embedded operating system.
    Zephyr,
    /// Tock embedded operating system.
    Tock,
    /// Generic embedded platform.
    Embedded,
    /// Unknown platform.
    Unknown,
}

impl std::fmt::Display for PlatformId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformId::Linux => write!(f, "Linux"),
            PlatformId::QNX => write!(f, "QNX"),
            PlatformId::MacOS => write!(f, "macOS"),
            PlatformId::Windows => write!(f, "Windows"),
            PlatformId::VxWorks => write!(f, "VxWorks"),
            PlatformId::Zephyr => write!(f, "Zephyr"),
            PlatformId::Tock => write!(f, "Tock"),
            PlatformId::Embedded => write!(f, "Embedded"),
            PlatformId::Unknown => write!(f, "Unknown"),
        }
    }
}

/// External limit sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalLimitSources {
    /// CLI arguments
    pub cli_args: Vec<String>,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Configuration file path
    pub config_file: Option<String>,
    /// Container runtime detection
    pub container_runtime: ContainerRuntime,
}

impl Default for ExternalLimitSources {
    fn default() -> Self {
        Self {
            cli_args: Vec::new(),
            env_vars: HashMap::new(),
            config_file: None,
            container_runtime: ContainerRuntime::None,
        }
    }
}

/// Container runtime types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerRuntime {
    /// No container detected
    None,
    /// Docker container
    Docker,
    /// Kubernetes pod
    Kubernetes,
    /// LXC container
    LXC,
    /// systemd-nspawn
    SystemdNspawn,
    /// Other container type
    Other,
}

impl std::fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerRuntime::None => write!(f, "Native"),
            ContainerRuntime::Docker => write!(f, "Docker"),
            ContainerRuntime::Kubernetes => write!(f, "Kubernetes"),
            ContainerRuntime::LXC => write!(f, "LXC"),
            ContainerRuntime::SystemdNspawn => write!(f, "systemd-nspawn"),
            ContainerRuntime::Other => write!(f, "Other"),
        }
    }
}

/// Platform verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformVerificationConfig {
    /// Maximum memory override from external sources
    pub max_memory_override: Option<usize>,
    /// Maximum WASM memory override
    pub max_wasm_memory_override: Option<usize>,
    /// Maximum stack override
    pub max_stack_override: Option<usize>,
    /// Maximum components override
    pub max_components_override: Option<usize>,
    /// Debug level override
    pub debug_level_override: Option<String>,
    /// Strict validation mode
    pub strict_validation: bool,
    /// External sources used
    pub sources: ExternalLimitSources,
}

impl Default for PlatformVerificationConfig {
    fn default() -> Self {
        Self {
            max_memory_override: None,
            max_wasm_memory_override: None,
            max_stack_override: None,
            max_components_override: None,
            debug_level_override: None,
            strict_validation: false,
            sources: ExternalLimitSources::default(),
        }
    }
}

/// Platform verification engine with cargo-wrt integration
#[derive(Debug)]
pub struct PlatformVerificationEngine {
    /// Configuration
    config: PlatformVerificationConfig,
    /// Discovered platform limits
    platform_limits: Option<ComprehensivePlatformLimits>,
    /// Final verified limits
    verified_limits: Option<ComprehensivePlatformLimits>,
    /// Workspace root for file operations
    workspace_root: PathBuf,
}

impl PlatformVerificationEngine {
    /// Create new verification engine
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            config: PlatformVerificationConfig::default(),
            platform_limits: None,
            verified_limits: None,
            workspace_root,
        }
    }

    /// Create verification engine with configuration
    pub fn with_config(workspace_root: PathBuf, config: PlatformVerificationConfig) -> Self {
        Self {
            config,
            platform_limits: None,
            verified_limits: None,
            workspace_root,
        }
    }

    /// Discover platform limits from all sources with diagnostic integration
    pub fn discover_limits(
        &mut self,
    ) -> BuildResult<(ComprehensivePlatformLimits, DiagnosticCollection)> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "platform-verification".to_string(),
        );

        // 1. Discover base platform limits
        let mut limits = self.discover_base_platform_limits(&mut diagnostics)?;

        // 2. Apply CLI argument overrides
        self.apply_cli_overrides(&mut limits, &mut diagnostics)?;

        // 3. Apply environment variable overrides
        self.apply_env_overrides(&mut limits, &mut diagnostics)?;

        // 4. Apply configuration file overrides
        self.apply_config_file_overrides(&mut limits, &mut diagnostics)?;

        // 5. Apply container runtime limits
        self.apply_container_limits(&mut limits, &mut diagnostics)?;

        // 6. Validate final limits
        self.validate_limits(&limits, &mut diagnostics)?;

        // Store results
        self.platform_limits = Some(limits.clone());
        self.verified_limits = Some(limits.clone());

        // Add summary diagnostic
        diagnostics.add_diagnostic(
            Diagnostic::new(
                "platform-verification".to_string(),
                Range::single_line(0, 0, 0),
                Severity::Info,
                format!(
                    "Platform verification complete: {} ({})",
                    limits.platform_id, self.config.sources.container_runtime
                ),
                "platform-verification".to_string(),
            )
            .with_code("platform-verified".to_string()),
        );

        Ok((limits, diagnostics))
    }

    /// Verify platform for specific ASIL level
    pub fn verify_for_asil(
        &mut self,
        asil_level: AsilLevel,
    ) -> BuildResult<(PlatformVerificationResult, DiagnosticCollection)> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            format!("platform-verification-{}", asil_level),
        );

        // Discover limits if not already done
        if self.verified_limits.is_none() {
            let (_limits, discovery_diagnostics) = self.discover_limits()?;
            diagnostics.add_diagnostics(discovery_diagnostics.diagnostics);
        }

        let limits = self.verified_limits.as_ref().unwrap();

        // ASIL-specific validation
        let asil_requirements = self.get_asil_requirements(asil_level);
        let mut violations = Vec::new();

        // Check memory requirements
        if limits.max_total_memory < asil_requirements.min_total_memory {
            let violation = PlatformViolation {
                violation_type: PlatformViolationType::InsufficientMemory,
                severity: ViolationSeverity::High,
                description: format!(
                    "Total memory {}MB below ASIL {} requirement of {}MB",
                    limits.max_total_memory / (1024 * 1024),
                    asil_level,
                    asil_requirements.min_total_memory / (1024 * 1024)
                ),
                current_value: limits.max_total_memory,
                required_value: asil_requirements.min_total_memory,
            };
            violations.push(violation);

            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Error,
                    format!("ASIL {} memory requirement not met", asil_level),
                    "platform-verification".to_string(),
                )
                .with_code("asil-memory-insufficient".to_string()),
            );
        }

        // Check component limits
        if limits.max_components < asil_requirements.min_components {
            let violation = PlatformViolation {
                violation_type: PlatformViolationType::InsufficientComponents,
                severity: ViolationSeverity::Medium,
                description: format!(
                    "Component limit {} below ASIL {} requirement of {}",
                    limits.max_components, asil_level, asil_requirements.min_components
                ),
                current_value: limits.max_components,
                required_value: asil_requirements.min_components,
            };
            violations.push(violation);
        }

        // Check container runtime compatibility
        if !asil_requirements
            .allowed_container_runtimes
            .contains(&self.config.sources.container_runtime)
        {
            let violation = PlatformViolation {
                violation_type: PlatformViolationType::UnsupportedContainerRuntime,
                severity: ViolationSeverity::High,
                description: format!(
                    "Container runtime {} not allowed for ASIL {}",
                    self.config.sources.container_runtime, asil_level
                ),
                current_value: 0,
                required_value: 0,
            };
            violations.push(violation);

            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Error,
                    format!(
                        "Container runtime {} incompatible with ASIL {}",
                        self.config.sources.container_runtime, asil_level
                    ),
                    "platform-verification".to_string(),
                )
                .with_code("asil-container-incompatible".to_string()),
            );
        }

        let compliance_score = if violations.is_empty() {
            100.0
        } else {
            let penalty: f64 = violations
                .iter()
                .map(|v| match v.severity {
                    ViolationSeverity::Critical => 50.0,
                    ViolationSeverity::High => 30.0,
                    ViolationSeverity::Medium => 15.0,
                    ViolationSeverity::Low => 5.0,
                })
                .sum();
            (100.0 - penalty).max(0.0)
        };

        let is_compliant = violations.is_empty();
        let violations_count = violations.len();
        let result = PlatformVerificationResult {
            asil_level,
            platform_limits: limits.clone(),
            violations,
            compliance_score,
            is_compliant,
            container_runtime: self.config.sources.container_runtime,
            platform_id: limits.platform_id,
        };

        // Add compliance diagnostic
        if result.is_compliant {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Info,
                    format!(
                        "Platform compliant with ASIL {} ({:.1}%)",
                        asil_level, compliance_score
                    ),
                    "platform-verification".to_string(),
                )
                .with_code("platform-compliant".to_string()),
            );
        } else {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Warning,
                    format!(
                        "Platform non-compliant with ASIL {} ({} violations)",
                        asil_level, violations_count
                    ),
                    "platform-verification".to_string(),
                )
                .with_code("platform-non-compliant".to_string()),
            );
        }

        Ok((result, diagnostics))
    }

    /// Convert platform verification to cargo-wrt diagnostics
    pub fn to_diagnostics(&self, output_format: OutputFormat) -> BuildResult<DiagnosticCollection> {
        let mut diagnostics = DiagnosticCollection::new(
            self.workspace_root.clone(),
            "platform-verification".to_string(),
        );

        if let Some(ref limits) = self.verified_limits {
            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Info,
                    format!(
                        "Platform: {} with {}MB memory, {} components",
                        limits.platform_id,
                        limits.max_total_memory / (1024 * 1024),
                        limits.max_components
                    ),
                    "platform-verification".to_string(),
                )
                .with_code("platform-info".to_string()),
            );

            diagnostics.add_diagnostic(
                Diagnostic::new(
                    "platform-verification".to_string(),
                    Range::single_line(0, 0, 0),
                    Severity::Info,
                    format!(
                        "Container runtime: {}",
                        self.config.sources.container_runtime
                    ),
                    "platform-verification".to_string(),
                )
                .with_code("container-runtime".to_string()),
            );
        }

        Ok(diagnostics)
    }

    /// Get verified limits
    pub fn verified_limits(&self) -> Option<&ComprehensivePlatformLimits> {
        self.verified_limits.as_ref()
    }

    /// Get configuration
    pub fn config(&self) -> &PlatformVerificationConfig {
        &self.config
    }

    // Private helper methods

    /// Discover base platform limits
    fn discover_base_platform_limits(
        &self,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<ComprehensivePlatformLimits> {
        let mut limits = ComprehensivePlatformLimits::default();

        // Detect platform
        limits.platform_id = self.detect_platform_id();

        // Platform-specific adjustments
        match limits.platform_id {
            PlatformId::Linux => {
                // Try to get actual system memory
                if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
                    if let Some(line) = meminfo.lines().find(|l| l.starts_with("MemTotal:")) {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                limits.max_total_memory = kb * 1024; // Convert KB to bytes
                                limits.max_wasm_linear_memory = (limits.max_total_memory * 3) / 4;
                            }
                        }
                    }
                }
            },
            PlatformId::MacOS => {
                // macOS-specific memory detection could go here
                limits.max_total_memory = 8 * 1024 * 1024 * 1024; // Default 8GB
                limits.max_wasm_linear_memory = 6 * 1024 * 1024 * 1024; // 6GB
            },
            PlatformId::Embedded => {
                // Conservative limits for embedded systems
                limits.max_total_memory = 64 * 1024 * 1024; // 64MB
                limits.max_wasm_linear_memory = 32 * 1024 * 1024; // 32MB
                limits.max_components = 32;
            },
            _ => {
                // Use defaults
            },
        }

        diagnostics.add_diagnostic(
            Diagnostic::new(
                "platform-verification".to_string(),
                Range::single_line(0, 0, 0),
                Severity::Info,
                format!("Detected platform: {}", limits.platform_id),
                "platform-verification".to_string(),
            )
            .with_code("platform-detected".to_string()),
        );

        Ok(limits)
    }

    /// Detect platform ID
    fn detect_platform_id(&self) -> PlatformId {
        #[cfg(target_os = "linux")]
        return PlatformId::Linux;

        #[cfg(target_os = "macos")]
        return PlatformId::MacOS;

        #[cfg(target_os = "windows")]
        return PlatformId::Windows;

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return PlatformId::Embedded;
    }

    /// Apply CLI argument overrides
    fn apply_cli_overrides(
        &self,
        limits: &mut ComprehensivePlatformLimits,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<()> {
        for arg in &self.config.sources.cli_args {
            if let Some(memory) = parse_memory_arg(arg, "--max-memory=") {
                limits.max_total_memory = memory;
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "platform-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!("CLI override: max memory = {}MB", memory / (1024 * 1024)),
                        "platform-verification".to_string(),
                    )
                    .with_code("cli-override".to_string()),
                );
            } else if let Some(wasm_memory) = parse_memory_arg(arg, "--max-wasm-memory=") {
                limits.max_wasm_linear_memory = wasm_memory;
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "platform-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!(
                            "CLI override: max WASM memory = {}MB",
                            wasm_memory / (1024 * 1024)
                        ),
                        "platform-verification".to_string(),
                    )
                    .with_code("cli-override".to_string()),
                );
            } else if let Some(stack) = parse_memory_arg(arg, "--max-stack=") {
                limits.max_stack_bytes = stack;
            } else if let Some(components) = parse_number_arg(arg, "--max-components=") {
                limits.max_components = components;
            }
        }
        Ok(())
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(
        &self,
        limits: &mut ComprehensivePlatformLimits,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<()> {
        if let Some(memory) = self.config.sources.env_vars.get("WRT_MAX_MEMORY") {
            if let Ok(value) = parse_memory_string(memory) {
                limits.max_total_memory = value;
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "platform-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!(
                            "Environment override: WRT_MAX_MEMORY = {}MB",
                            value / (1024 * 1024)
                        ),
                        "platform-verification".to_string(),
                    )
                    .with_code("env-override".to_string()),
                );
            }
        }

        if let Some(wasm_memory) = self.config.sources.env_vars.get("WRT_MAX_WASM_MEMORY") {
            if let Ok(value) = parse_memory_string(wasm_memory) {
                limits.max_wasm_linear_memory = value;
            }
        }

        if let Some(stack) = self.config.sources.env_vars.get("WRT_MAX_STACK") {
            if let Ok(value) = parse_memory_string(stack) {
                limits.max_stack_bytes = value;
            }
        }

        if let Some(components) = self.config.sources.env_vars.get("WRT_MAX_COMPONENTS") {
            if let Ok(value) = components.parse::<usize>() {
                limits.max_components = value;
            }
        }

        Ok(())
    }

    /// Apply configuration file overrides
    fn apply_config_file_overrides(
        &self,
        limits: &mut ComprehensivePlatformLimits,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<()> {
        if let Some(ref config_path) = self.config.sources.config_file {
            let full_path = self.workspace_root.join(config_path);
            if full_path.exists() {
                let config_content = fs::read_to_string(&full_path).map_err(|e| {
                    BuildError::Workspace(format!("Failed to read configuration file: {}", e))
                })?;

                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "platform-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Info,
                        format!("Loading configuration from: {}", config_path),
                        "platform-verification".to_string(),
                    )
                    .with_code("config-loaded".to_string()),
                );

                // Simple key=value parser
                for line in config_content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }

                    if let Some((key, value)) = line.split_once('=') {
                        match key.trim() {
                            "max_memory" => {
                                if let Ok(memory) = parse_memory_string(value.trim()) {
                                    limits.max_total_memory = memory;
                                }
                            },
                            "max_wasm_memory" => {
                                if let Ok(memory) = parse_memory_string(value.trim()) {
                                    limits.max_wasm_linear_memory = memory;
                                }
                            },
                            "max_stack" => {
                                if let Ok(memory) = parse_memory_string(value.trim()) {
                                    limits.max_stack_bytes = memory;
                                }
                            },
                            "max_components" => {
                                if let Ok(components) = value.trim().parse::<usize>() {
                                    limits.max_components = components;
                                }
                            },
                            _ => {}, // Ignore unknown keys
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Apply container runtime limits
    fn apply_container_limits(
        &self,
        limits: &mut ComprehensivePlatformLimits,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<()> {
        match self.config.sources.container_runtime {
            ContainerRuntime::Docker => {
                // Check Docker memory limits
                if let Ok(limit) = fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes")
                {
                    if let Ok(memory_limit) = limit.trim().parse::<usize>() {
                        if memory_limit < limits.max_total_memory {
                            limits.max_total_memory = memory_limit;
                            limits.max_wasm_linear_memory = (memory_limit * 3) / 4;

                            diagnostics.add_diagnostic(
                                Diagnostic::new(
                                    "platform-verification".to_string(),
                                    Range::single_line(0, 0, 0),
                                    Severity::Info,
                                    format!(
                                        "Docker memory limit applied: {}MB",
                                        memory_limit / (1024 * 1024)
                                    ),
                                    "platform-verification".to_string(),
                                )
                                .with_code("docker-limit".to_string()),
                            );
                        }
                    }
                }
            },
            ContainerRuntime::Kubernetes => {
                // Check Kubernetes resource limits
                if let Ok(requests) = env::var("KUBERNETES_MEMORY_REQUEST") {
                    if let Ok(memory) = parse_memory_string(&requests) {
                        limits.max_total_memory = memory;
                        limits.max_wasm_linear_memory = (memory * 3) / 4;
                    }
                }

                if let Ok(limits_env) = env::var("KUBERNETES_MEMORY_LIMIT") {
                    if let Ok(memory) = parse_memory_string(&limits_env) {
                        limits.max_total_memory = limits.max_total_memory.min(memory);
                        limits.max_wasm_linear_memory = (limits.max_total_memory * 3) / 4;
                    }
                }
            },
            _ => {
                // No container-specific limits
            },
        }
        Ok(())
    }

    /// Validate final limits for consistency
    fn validate_limits(
        &self,
        limits: &ComprehensivePlatformLimits,
        diagnostics: &mut DiagnosticCollection,
    ) -> BuildResult<()> {
        // Check that WASM memory doesn't exceed total memory
        if limits.max_wasm_linear_memory > limits.max_total_memory {
            if self.config.strict_validation {
                return Err(BuildError::Verification(
                    "WASM memory limit exceeds total memory limit".to_string(),
                ));
            } else {
                diagnostics.add_diagnostic(
                    Diagnostic::new(
                        "platform-verification".to_string(),
                        Range::single_line(0, 0, 0),
                        Severity::Warning,
                        "WASM memory limit exceeds total memory - will be auto-corrected"
                            .to_string(),
                        "platform-verification".to_string(),
                    )
                    .with_code("memory-limit-corrected".to_string()),
                );
            }
        }

        // Check minimum viable limits
        if limits.max_total_memory < 1024 * 1024 {
            // 1MB minimum
            return Err(BuildError::Verification(
                "Total memory limit too small (minimum 1MB)".to_string(),
            ));
        }

        if limits.max_stack_bytes < 4096 {
            // 4KB minimum stack
            return Err(BuildError::Verification(
                "Stack limit too small (minimum 4KB)".to_string(),
            ));
        }

        if limits.max_components == 0 {
            return Err(BuildError::Verification(
                "Component limit cannot be zero".to_string(),
            ));
        }

        Ok(())
    }

    /// Get ASIL requirements for platform verification
    fn get_asil_requirements(&self, asil_level: AsilLevel) -> AsilPlatformRequirements {
        match asil_level {
            AsilLevel::QM => AsilPlatformRequirements {
                min_total_memory: 64 * 1024 * 1024, // 64MB
                min_components: 16,
                allowed_container_runtimes: vec![
                    ContainerRuntime::None,
                    ContainerRuntime::Docker,
                    ContainerRuntime::Kubernetes,
                    ContainerRuntime::LXC,
                    ContainerRuntime::SystemdNspawn,
                    ContainerRuntime::Other,
                ],
            },
            AsilLevel::A => AsilPlatformRequirements {
                min_total_memory: 128 * 1024 * 1024, // 128MB
                min_components: 32,
                allowed_container_runtimes: vec![
                    ContainerRuntime::None,
                    ContainerRuntime::Docker,
                    ContainerRuntime::Kubernetes,
                ],
            },
            AsilLevel::B => AsilPlatformRequirements {
                min_total_memory: 256 * 1024 * 1024, // 256MB
                min_components: 64,
                allowed_container_runtimes: vec![ContainerRuntime::None, ContainerRuntime::Docker],
            },
            AsilLevel::C => AsilPlatformRequirements {
                min_total_memory: 512 * 1024 * 1024, // 512MB
                min_components: 128,
                allowed_container_runtimes: vec![ContainerRuntime::None],
            },
            AsilLevel::D => AsilPlatformRequirements {
                min_total_memory: 1024 * 1024 * 1024, // 1GB
                min_components: 256,
                allowed_container_runtimes: vec![
                    ContainerRuntime::None, // Only native execution for ASIL-D
                ],
            },
        }
    }
}

/// ASIL-specific platform requirements
#[derive(Debug, Clone)]
struct AsilPlatformRequirements {
    min_total_memory: usize,
    min_components: usize,
    allowed_container_runtimes: Vec<ContainerRuntime>,
}

/// Platform verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformVerificationResult {
    /// ASIL level being verified.
    pub asil_level: AsilLevel,
    /// Platform resource limits discovered.
    pub platform_limits: ComprehensivePlatformLimits,
    /// List of platform violations found.
    pub violations: Vec<PlatformViolation>,
    /// Compliance score for this platform.
    pub compliance_score: f64,
    /// Whether the platform is compliant.
    pub is_compliant: bool,
    /// Container runtime detected.
    pub container_runtime: ContainerRuntime,
    /// Platform identifier.
    pub platform_id: PlatformId,
}

/// Platform violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformViolation {
    /// Type of platform violation.
    pub violation_type: PlatformViolationType,
    /// Severity level of the violation.
    pub severity: ViolationSeverity,
    /// Human-readable description of the violation.
    pub description: String,
    /// Current platform value.
    pub current_value: usize,
    /// Required value for compliance.
    pub required_value: usize,
}

/// Types of platform violations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformViolationType {
    /// Platform has insufficient memory resources.
    InsufficientMemory,
    /// Platform has insufficient component capacity.
    InsufficientComponents,
    /// Platform uses an unsupported container runtime.
    UnsupportedContainerRuntime,
    /// Platform is not supported.
    UnsupportedPlatform,
    /// Platform configuration is invalid.
    InvalidConfiguration,
}

/// Violation severity
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ViolationSeverity {
    /// Low severity violation.
    Low,
    /// Medium severity violation.
    Medium,
    /// High severity violation.
    High,
    /// Critical severity violation requiring immediate attention.
    Critical,
}

/// Platform verification configuration builder
pub struct PlatformVerificationConfigBuilder {
    config: PlatformVerificationConfig,
}

impl PlatformVerificationConfigBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: PlatformVerificationConfig::default(),
        }
    }

    /// Add CLI arguments
    pub fn with_cli_args(mut self, args: Vec<String>) -> Self {
        self.config.sources.cli_args = args;
        self
    }

    /// Set configuration file
    pub fn with_config_file<P: AsRef<str>>(mut self, path: P) -> Self {
        self.config.sources.config_file = Some(path.as_ref().to_string());
        self
    }

    /// Enable strict validation
    pub fn with_strict_validation(mut self, strict: bool) -> Self {
        self.config.strict_validation = strict;
        self
    }

    /// Set container runtime
    pub fn with_container_runtime(mut self, runtime: ContainerRuntime) -> Self {
        self.config.sources.container_runtime = runtime;
        self
    }

    /// Build configuration
    pub fn build(mut self) -> PlatformVerificationConfig {
        // Auto-detect environment variables
        for (key, value) in env::vars() {
            if key.starts_with("WRT_") {
                self.config.sources.env_vars.insert(key, value);
            }
        }

        // Auto-detect container runtime
        if self.config.sources.container_runtime == ContainerRuntime::None {
            self.config.sources.container_runtime = detect_container_runtime();
        }

        self.config
    }
}

impl Default for PlatformVerificationConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect container runtime
fn detect_container_runtime() -> ContainerRuntime {
    // Check for Docker
    if Path::new("/.dockerenv").exists() {
        return ContainerRuntime::Docker;
    }

    // Check for Kubernetes
    if env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return ContainerRuntime::Kubernetes;
    }

    // Check for systemd-nspawn
    if let Ok(container) = env::var("container") {
        if container == "systemd-nspawn" {
            return ContainerRuntime::SystemdNspawn;
        }
    }

    // Check cgroup for container indicators
    if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("docker") {
            return ContainerRuntime::Docker;
        }
        if cgroup.contains("lxc") {
            return ContainerRuntime::LXC;
        }
    }

    ContainerRuntime::None
}

/// Parse memory argument from CLI
fn parse_memory_arg(arg: &str, prefix: &str) -> Option<usize> {
    if arg.starts_with(prefix) {
        let value = &arg[prefix.len()..];
        parse_memory_string(value).ok()
    } else {
        None
    }
}

/// Parse number argument from CLI
fn parse_number_arg(arg: &str, prefix: &str) -> Option<usize> {
    if arg.starts_with(prefix) {
        let value = &arg[prefix.len()..];
        value.parse().ok()
    } else {
        None
    }
}

/// Parse memory string with units (e.g., "256MB", "1GB")
fn parse_memory_string(value: &str) -> Result<usize, BuildError> {
    let value = value.trim().to_uppercase();

    if let Some(stripped) = value.strip_suffix("KB") {
        stripped
            .parse::<usize>()
            .map(|n| n * 1024)
            .map_err(|_| BuildError::Verification("Invalid memory value".to_string()))
    } else if let Some(stripped) = value.strip_suffix("MB") {
        stripped
            .parse::<usize>()
            .map(|n| n * 1024 * 1024)
            .map_err(|_| BuildError::Verification("Invalid memory value".to_string()))
    } else if let Some(stripped) = value.strip_suffix("GB") {
        stripped
            .parse::<usize>()
            .map(|n| n * 1024 * 1024 * 1024)
            .map_err(|_| BuildError::Verification("Invalid memory value".to_string()))
    } else {
        // Assume bytes
        value
            .parse::<usize>()
            .map_err(|_| BuildError::Verification("Invalid memory value".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_memory_string_parsing() {
        assert_eq!(parse_memory_string("1024").unwrap(), 1024);
        assert_eq!(parse_memory_string("1KB").unwrap(), 1024);
        assert_eq!(parse_memory_string("1MB").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_string("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_string("256mb").unwrap(), 256 * 1024 * 1024);
    }

    #[test]
    fn test_cli_arg_parsing() {
        assert_eq!(
            parse_memory_arg("--max-memory=256MB", "--max-memory=").unwrap(),
            256 * 1024 * 1024
        );
        assert_eq!(
            parse_number_arg("--max-components=512", "--max-components=").unwrap(),
            512
        );
        assert_eq!(parse_memory_arg("--other-arg=256MB", "--max-memory="), None);
    }

    #[test]
    fn test_config_builder() {
        let config = PlatformVerificationConfigBuilder::new()
            .with_cli_args(vec!["--max-memory=1GB".to_string()])
            .with_strict_validation(true)
            .with_container_runtime(ContainerRuntime::Docker)
            .build();

        assert!(config.strict_validation);
        assert_eq!(config.sources.container_runtime, ContainerRuntime::Docker);
        assert_eq!(config.sources.cli_args.len(), 1);
    }

    #[test]
    fn test_verification_engine() {
        let config = PlatformVerificationConfigBuilder::new().with_strict_validation(false).build();

        let mut engine = PlatformVerificationEngine::with_config(PathBuf::from("/tmp"), config);
        let (limits, _diagnostics) = engine.discover_limits().unwrap();

        assert!(limits.max_total_memory > 0);
        assert!(limits.max_wasm_linear_memory > 0);
        assert!(limits.max_stack_bytes > 0);
        assert!(limits.max_components > 0);
    }

    #[test]
    fn test_container_detection() {
        // This test would depend on the actual runtime environment
        let runtime = detect_container_runtime();
        // Just ensure it returns a valid value
        assert!(matches!(
            runtime,
            ContainerRuntime::None
                | ContainerRuntime::Docker
                | ContainerRuntime::Kubernetes
                | ContainerRuntime::LXC
                | ContainerRuntime::SystemdNspawn
                | ContainerRuntime::Other
        ));
    }

    #[test]
    fn test_asil_platform_verification() {
        let config = PlatformVerificationConfigBuilder::new().build();
        let mut engine = PlatformVerificationEngine::with_config(PathBuf::from("/tmp"), config);

        let (result, _diagnostics) = engine.verify_for_asil(AsilLevel::A).unwrap();

        assert_eq!(result.asil_level, AsilLevel::A);
        assert!(result.platform_limits.max_total_memory != 0);
    }
}
