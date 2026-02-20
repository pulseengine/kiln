//! Standardized command execution trait for cargo-wrt
//!
//! Provides a consistent interface for all cargo-wrt commands,
//! ensuring standardized error handling, output formatting, and argument
//! processing.

use anyhow::Result;
use wrt_build_core::BuildSystem;

use super::GlobalArgs;

/// Standard result type for command execution
pub type CommandResult = Result<()>;

/// Trait for standardized command execution
#[async_trait::async_trait]
pub trait StandardCommand {
    /// Execute the command with standardized arguments
    async fn execute(&self, build_system: &BuildSystem, global: &mut GlobalArgs) -> CommandResult;

    /// Get command name for logging/diagnostics
    fn name(&self) -> &'static str;

    /// Check if command supports the current output format
    fn supports_output_format(&self, format: &wrt_build_core::formatters::OutputFormat) -> bool {
        // By default, all commands support all formats
        true
    }

    /// Validate command-specific arguments
    fn validate_args(&self, global: &GlobalArgs) -> Result<()> {
        // Default implementation - no validation
        Ok(())
    }
}

/// Extension methods for enhanced command execution
pub trait CommandExt: StandardCommand {
    /// Execute with pre and post processing
    async fn execute_with_hooks(
        &self,
        build_system: &BuildSystem,
        global: &mut GlobalArgs,
    ) -> CommandResult {
        // Pre-execution validation
        self.validate_args(global)?;

        // Check output format support
        if !self.supports_output_format(&global.output_format) {
            anyhow::bail!(
                "Command '{}' does not support output format '{:?}'",
                self.name(),
                global.output_format
            );
        }

        // Log command execution in verbose mode
        if global.verbose {
            global.output.subheader(&format!("Executing command: {}", self.name()));
        }

        // Execute main command
        let result = self.execute(build_system, global).await;

        // Post-execution processing
        if let Err(ref e) = result {
            if global.verbose {
                global.output.error(&format!("Command '{}' failed: {}", self.name(), e));
            }
        } else if global.verbose {
            global
                .output
                .success(&format!("Command '{}' completed successfully", self.name()));
        }

        result
    }
}

// Auto-implement CommandExt for all StandardCommand implementations
impl<T: StandardCommand> CommandExt for T {}

/// Standardized command structure for build-related commands
pub struct BuildCommand {
    pub package: Option<String>,
    pub clippy: bool,
    pub fmt_check: bool,
}

#[async_trait::async_trait]
impl StandardCommand for BuildCommand {
    async fn execute(&self, build_system: &BuildSystem, global: &mut GlobalArgs) -> CommandResult {
        crate::cmd_build(
            build_system,
            self.package.clone(),
            self.clippy,
            self.fmt_check,
            global,
        )
        .await
    }

    fn name(&self) -> &'static str {
        "build"
    }
}

/// Standardized command structure for test-related commands
pub struct TestCommand {
    pub package: Option<String>,
    pub filter: Option<String>,
    pub nocapture: bool,
    pub unit_only: bool,
    pub no_doc_tests: bool,
}

#[async_trait::async_trait]
impl StandardCommand for TestCommand {
    async fn execute(&self, build_system: &BuildSystem, global: &mut GlobalArgs) -> CommandResult {
        // Use global args directly instead of creating a CLI instance
        // Create a minimal CLI for compatibility
        let cli = crate::Cli {
            command: crate::Commands::Test {
                package: self.package.clone(),
                filter: self.filter.clone(),
                nocapture: self.nocapture,
                unit_only: self.unit_only,
                no_doc_tests: self.no_doc_tests,
            },
            verbose: global.verbose,
            dry_run: global.dry_run,
            trace_commands: global.trace_commands,
            profile: crate::ProfileArg::Dev, // Default profile
            features: None,
            workspace: global.workspace.clone(),
            output: crate::OutputFormatArg::Human, // Default format
            cache: global.cache,
            clear_cache: global.clear_cache,
            diff_only: global.diff_only,
            filter_severity: global.filter_severity.clone(),
            filter_source: global.filter_source.clone(),
            filter_file: global.filter_file.clone(),
            group_by: None,
            limit: global.limit,
        };

        let output_format = global.output_format.clone();
        let use_colors = global.output.is_colored();

        crate::cmd_test(
            build_system,
            self.package.clone(),
            self.filter.clone(),
            self.nocapture,
            self.unit_only,
            self.no_doc_tests,
            &output_format,
            use_colors,
            &cli,
            global,
        )
        .await
    }

    fn name(&self) -> &'static str {
        "test"
    }
}

/// Standardized command structure for check-related commands
pub struct CheckCommand {
    pub strict: bool,
    pub fix: bool,
}

#[async_trait::async_trait]
impl StandardCommand for CheckCommand {
    async fn execute(&self, build_system: &BuildSystem, global: &mut GlobalArgs) -> CommandResult {
        crate::cmd_check(build_system, self.strict, self.fix, global).await
    }

    fn name(&self) -> &'static str {
        "check"
    }
}

/// Macro for creating standardized command implementations
#[macro_export]
macro_rules! impl_standard_command {
    ($cmd_type:ident, $name:literal, $fn_name:ident, $($field:ident: $field_type:ty),*) => {
        pub struct $cmd_type {
            $(pub $field: $field_type,)*
        }

        #[async_trait::async_trait]
        impl StandardCommand for $cmd_type {
            async fn execute(
                &self,
                build_system: &BuildSystem,
                global: &mut GlobalArgs,
            ) -> CommandResult {
                crate::$fn_name(build_system, $(self.$field.clone(),)* global).await
            }

            fn name(&self) -> &'static str {
                $name
            }
        }
    };
}
