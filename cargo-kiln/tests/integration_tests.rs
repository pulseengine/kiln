//! Integration tests for cargo-wrt
//!
//! Comprehensive tests for all cargo-wrt functionality including
//! command execution, helper modules, and workflow scenarios.

use std::time::Duration;

use cargo_wrt::testing::{ProjectFeature, TestConfig, TestContext, TestValidator, WorkspaceType};
use tempfile::TempDir;
use wrt_build_core::formatters::OutputFormat;

/// Test helper module functionality
#[cfg(test)]
mod helper_tests {
    use cargo_wrt::helpers::{
        CategorizedError, CommandSuggestionEngine, ContextDetector, ErrorCategory, OutputManager,
        PerformanceOptimizer, ProgressIndicator,
    };

    use super::*;

    #[test]
    fn test_output_manager_human_format() {
        let output = OutputManager::new(OutputFormat::Human).with_color(false);

        assert_eq!(output.format(), &OutputFormat::Human);
        assert!(!output.is_colored());

        // Test basic output methods (these would normally write to stdout)
        // In a real test, we'd capture stdout or use a mock writer
    }

    #[test]
    fn test_output_manager_json_format() {
        let output = OutputManager::new(OutputFormat::Json).with_color(false);

        assert_eq!(output.format(), &OutputFormat::Json);
        assert!(!output.is_colored());
    }

    #[test]
    fn test_progress_indicator_creation() {
        let progress = ProgressIndicator::spinner("Test operation", OutputFormat::Human, false);

        // Progress indicator created successfully
        // In a real test, we'd test the actual progress display
    }

    #[test]
    fn test_command_suggestion_engine() {
        let engine = CommandSuggestionEngine::new();

        // Test exact match
        let suggestions = engine.suggest("build", None);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].command, "build");

        // Test typo correction
        let suggestions = engine.suggest("buld", None);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].command, "build");

        // Test empty input
        let suggestions = engine.suggest("", None);
        // Should handle empty input gracefully
    }

    #[test]
    fn test_performance_optimizer() {
        let mut optimizer = PerformanceOptimizer::with_defaults();

        // Test timing
        optimizer.start_timer("test_command");
        std::thread::sleep(Duration::from_millis(10));
        optimizer.stop_timer("test_command");

        let report = optimizer.generate_report();
        assert!(report.metrics.command_times.contains_key("test_command"));

        // Test cache tracking
        optimizer.record_cache_hit();
        optimizer.record_cache_miss();
        optimizer.record_cache_hit();

        let ratio = optimizer.cache_hit_ratio();
        assert!((ratio - 0.666).abs() < 0.01); // 2/3 ≈ 0.666
    }

    #[test]
    fn test_categorized_error() {
        let error = CategorizedError::system_error("Test error")
            .with_context("Additional context")
            .with_suggestion("Try running cargo clean");

        assert_eq!(error.category, ErrorCategory::Build);
        assert!(error.context.contains(&"Additional context".to_string()));
        assert!(error.suggestions.contains(&"Try running cargo clean".to_string()));

        // Test formatting
        let human_format = error.format_human(false);
        assert!(human_format.contains("Build Error"));
        assert!(human_format.contains("Test error"));

        let json_format = error.format_json();
        assert!(json_format.is_object());
    }
}

/// Test project context detection
#[cfg(test)]
mod context_detection_tests {
    use super::*;

    #[test]
    fn test_wrt_workspace_detection() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::WrtWorkspace,
            project_features: vec![
                ProjectFeature::Tests,
                ProjectFeature::Documentation,
                ProjectFeature::CI,
            ],
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;
        let detector = ContextDetector::new(context.workspace_root().to_path_buf());
        let project_context = detector.detect()?;

        assert!(matches!(
            project_context.project_type,
            cargo_wrt::helpers::ProjectType::WrtWorkspace
        ));
        assert!(project_context.features.has_tests);
        assert!(project_context.features.has_docs);
        assert!(project_context.features.has_ci);

        Ok(())
    }

    #[test]
    fn test_rust_crate_detection() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::RustCrate,
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;
        let detector = ContextDetector::new(context.workspace_root().to_path_buf());
        let project_context = detector.detect()?;

        assert!(matches!(
            project_context.project_type,
            cargo_wrt::helpers::ProjectType::RustCrate
        ));

        Ok(())
    }

    #[test]
    fn test_empty_directory_detection() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::Empty,
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;
        let detector = ContextDetector::new(context.workspace_root().to_path_buf());
        let project_context = detector.detect()?;

        assert!(matches!(
            project_context.project_type,
            cargo_wrt::helpers::ProjectType::Unknown
        ));

        Ok(())
    }
}

/// Test validation framework
#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_comprehensive_validation() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::WrtWorkspace,
            project_features: vec![ProjectFeature::Tests, ProjectFeature::CI],
            output_format: OutputFormat::Human,
            use_colors: false,
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;
        let validator = TestValidator::new(context);

        let reports = validator.validate_all()?;

        // Should have validation reports for all major components
        assert!(!reports.is_empty());

        // Check specific validations
        let progress_report = reports
            .iter()
            .find(|r| r.name == "Progress Indicators")
            .expect("Progress indicators validation should exist");

        assert!(progress_report.total_tests() > 0);

        let suggestions_report = reports
            .iter()
            .find(|r| r.name == "Command Suggestions")
            .expect("Command suggestions validation should exist");

        assert!(suggestions_report.total_tests() > 0);

        // Print validation results
        for report in &reports {
            println!("{}", report.format(false));
        }

        Ok(())
    }

    #[test]
    fn test_validation_report_formatting() {
        let mut report = cargo_wrt::testing::ValidationReport::new("Test Report");
        report.add_success("Test passed");
        report.add_failure("Test failed");
        report.add_warning("Test warning");

        let formatted = report.format(false);
        assert!(formatted.contains("Test Report"));
        assert!(formatted.contains("Test passed"));
        assert!(formatted.contains("Test failed"));
        assert!(formatted.contains("Test warning"));

        assert!(!report.is_successful()); // Has failures
        assert_eq!(report.total_tests(), 2); // success + failure
    }
}

/// Test workspace setup functionality
#[cfg(test)]
mod workspace_tests {
    use super::*;

    #[test]
    fn test_wrt_workspace_creation() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::WrtWorkspace,
            project_features: vec![
                ProjectFeature::Tests,
                ProjectFeature::Documentation,
                ProjectFeature::SafetyVerification,
            ],
            enable_git: true,
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;

        // Verify workspace structure
        assert!(context.file_exists("Cargo.toml"));
        assert!(context.file_exists("wrt-foundation/Cargo.toml"));
        assert!(context.file_exists("wrt-build-core/Cargo.toml"));
        assert!(context.file_exists("cargo-wrt/Cargo.toml"));

        // Verify features
        assert!(context.file_exists("tests/integration.rs"));
        assert!(context.file_exists("README.md"));
        assert!(context.file_exists("requirements.toml"));

        // Verify git setup
        assert!(context.file_exists(".git/config"));

        // Test file operations
        context.create_file("test-file.txt", "test content")?;
        assert!(context.file_exists("test-file.txt"));

        let content = context.read_file("test-file.txt")?;
        assert_eq!(content, "test content");

        Ok(())
    }

    #[test]
    fn test_single_crate_creation() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::WrtCrate {
                name: "test-wrt-crate".to_string(),
            },
            project_features: vec![ProjectFeature::Benchmarks],
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;

        // Verify single crate structure
        assert!(context.file_exists("Cargo.toml"));
        assert!(context.file_exists("src/lib.rs"));
        assert!(context.file_exists("benches/benchmark.rs"));

        // Verify it's not a workspace
        assert!(!context.file_exists("wrt-foundation/Cargo.toml"));

        // Verify crate name in manifest
        let manifest = context.read_file("Cargo.toml")?;
        assert!(manifest.contains("test-wrt-crate"));

        Ok(())
    }
}

/// Test mock build system
#[cfg(test)]
mod mock_build_tests {
    use cargo_wrt::testing::MockBuildResult;

    use super::*;

    #[test]
    fn test_mock_build_system() -> anyhow::Result<()> {
        let context = TestContext::new()?;
        let mut mock_system = context.build_system;

        // Test default successful result
        mock_system.call_log.push("build_all".to_string());
        assert!(mock_system.was_called("build_all"));
        assert_eq!(mock_system.call_count("build_all"), 1);

        // Test setting custom result
        mock_system.set_result(
            "custom_operation",
            MockBuildResult {
                success: true,
                duration: Duration::from_millis(1000),
                warnings: vec!["test warning".to_string()],
                errors: vec![],
            },
        );

        // Test setting failure
        mock_system.set_failure("failing_operation", "Operation failed");

        // Test call logging
        mock_system.call_log.push("custom_operation".to_string());
        mock_system.call_log.push("failing_operation".to_string());

        assert!(mock_system.was_called("custom_operation"));
        assert!(mock_system.was_called("failing_operation"));
        assert!(!mock_system.was_called("non_existent_operation"));

        // Test clearing log
        mock_system.clear_log();
        assert!(!mock_system.was_called("build_all"));

        Ok(())
    }
}

/// Performance and stress tests
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_large_workspace_handling() -> anyhow::Result<()> {
        let config = TestConfig {
            workspace_type: WorkspaceType::WrtWorkspace,
            project_features: vec![
                ProjectFeature::Tests,
                ProjectFeature::Benchmarks,
                ProjectFeature::Examples,
                ProjectFeature::Documentation,
                ProjectFeature::CI,
                ProjectFeature::SafetyVerification,
            ],
            ..Default::default()
        };

        let context = TestContext::with_config(config)?;

        // Create many files to simulate large workspace
        for i in 0..100 {
            context.create_file(&format!("test_files/file_{}.rs", i), "// test file")?;
        }

        // Test context detection performance
        let start = std::time::Instant::now();
        let detector = ContextDetector::new(context.workspace_root().to_path_buf());
        let _project_context = detector.detect()?;
        let duration = start.elapsed();

        // Should complete quickly even with many files
        assert!(duration < Duration::from_secs(1));

        Ok(())
    }

    #[test]
    fn test_concurrent_progress_indicators() {
        use std::{sync::Arc, thread};

        let handles: Vec<_> = (0..5)
            .map(|i| {
                thread::spawn(move || {
                    let mut progress = ProgressIndicator::spinner(
                        format!("Concurrent operation {}", i),
                        OutputFormat::Human,
                        false,
                    );
                    progress.start();

                    for _ in 0..10 {
                        progress.tick();
                        thread::sleep(Duration::from_millis(10));
                    }

                    progress.finish();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All progress indicators should complete without panicking
    }
}

/// Error handling and edge case tests
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_workspace_handling() {
        // Test with completely invalid workspace
        let temp_dir = TempDir::new().unwrap();
        let invalid_path = temp_dir.path().join("non_existent");

        let detector = ContextDetector::new(invalid_path);
        let result = detector.detect();

        // Should handle invalid paths gracefully
        match result {
            Ok(context) => {
                assert!(matches!(
                    context.project_type,
                    cargo_wrt::helpers::ProjectType::Unknown
                ));
            },
            Err(_) => {
                // Error is also acceptable for completely invalid paths
            },
        }
    }

    #[test]
    fn test_command_suggestions_edge_cases() {
        let engine = CommandSuggestionEngine::new();

        // Test very long input
        let long_input = "a".repeat(1000);
        let suggestions = engine.suggest(&long_input, None);
        // Should not panic or take excessive time

        // Test special characters
        let special_input = "!@#$%^&*()";
        let suggestions = engine.suggest(special_input, None);
        // Should handle gracefully

        // Test unicode input
        let unicode_input = "测试";
        let suggestions = engine.suggest(unicode_input, None);
        // Should handle gracefully
    }

    #[test]
    fn test_performance_optimizer_edge_cases() {
        let mut optimizer = PerformanceOptimizer::with_defaults();

        // Test stopping timer that was never started
        optimizer.stop_timer("never_started");
        // Should not panic

        // Test starting same timer multiple times
        optimizer.start_timer("duplicate");
        optimizer.start_timer("duplicate");
        optimizer.stop_timer("duplicate");
        // Should handle gracefully

        // Test cache operations with no data
        let ratio = optimizer.cache_hit_ratio();
        assert_eq!(ratio, 0.0);
    }
}
