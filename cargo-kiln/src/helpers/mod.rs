//! Helper modules for cargo-wrt command implementations
//!
//! This module provides common functionality and patterns used across
//! all cargo-wrt command handlers to ensure consistency and reduce duplication.

pub mod autofix;
pub mod command_suggestions;
pub mod command_trait;
pub mod config;
pub mod diagnostics;
pub mod error_handling;
pub mod github;
pub mod global_args;
pub mod help_system;
pub mod output;
pub mod performance;
pub mod progress;
pub mod smart_defaults;
pub mod test_runner;
pub mod validation;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use autofix::{AutoFixManager, AutoFixResult, apply_project_fixes, supports_autofix};
pub use command_suggestions::{CommandSuggestionEngine, Suggestion, SuggestionType};
pub use command_trait::{
    BuildCommand, CheckCommand, CommandExt, CommandResult, StandardCommand, TestCommand,
};
pub use config::{
    CargoWrtConfig, MergedConfig, create_and_open_html_report, get_browser_command,
    load_config_file, merge_global_args, open_in_browser,
};
pub use diagnostics::{
    DiagnosticCommand, DiagnosticHelper, with_diagnostic_integration, with_simple_output,
};
pub use error_handling::{
    CategorizedError, ErrorCategory, ErrorContext, ErrorHandler, build_errors, config_errors,
};
pub use github::{GitHubConfig, GitHubContext, generate_workflow_summary};
pub use global_args::{GlobalArgs, WithGlobalArgs};
pub use help_system::{CommandCategory, CommandDoc, CommandExample, HelpSystem};
pub use output::{OutputManager, SimpleResponse, format_result, output_diagnostics, output_result};
pub use performance::{
    PerformanceConfig, PerformanceMetrics, PerformanceOptimizer, PerformanceReport,
};
pub use progress::{MultiStepProgress, ProgressConfig, ProgressIndicator, ProgressStyle};
pub use smart_defaults::{
    ContextDetector, ProjectContext, ProjectType, Recommendation, RecommendationPriority,
    SmartDefaults,
};
pub use test_runner::{TestConfig, TestRunner, run_asil_tests, run_no_std_tests, run_tests};
pub use validation::{StandardError, validate_asil_level, validate_file_path};
