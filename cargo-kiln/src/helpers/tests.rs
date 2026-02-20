//! Unit tests for cargo-wrt helper modules
//!
//! Comprehensive unit tests for all helper module functionality
//! to ensure reliability and correctness.

#[cfg(test)]
mod output_tests {
    use wrt_build_core::formatters::OutputFormat;

    use super::super::output::*;

    #[test]
    fn test_output_manager_creation() {
        let output = OutputManager::new(OutputFormat::Human);
        assert_eq!(output.format(), &OutputFormat::Human);
        assert!(!output.is_colored()); // Default is no color

        let colored_output = output.with_color(true);
        assert!(colored_output.is_colored());
    }

    #[test]
    fn test_output_manager_json_mode() {
        let output = OutputManager::new(OutputFormat::Json);
        assert_eq!(output.format(), &OutputFormat::Json);

        let json_lines_output = OutputManager::new(OutputFormat::JsonLines);
        assert_eq!(json_lines_output.format(), &OutputFormat::JsonLines);
    }

    #[test]
    fn test_simple_response() {
        let response = SimpleResponse::new("Test message");
        assert_eq!(response.message, "Test message");
        assert!(response.details.is_empty());

        let detailed_response = response.with_detail("Additional detail");
        assert_eq!(detailed_response.details.len(), 1);
        assert_eq!(detailed_response.details[0], "Additional detail");
    }
}

#[cfg(test)]
mod progress_tests {
    use std::time::Duration;

    use wrt_build_core::formatters::OutputFormat;

    use super::super::progress::*;

    #[test]
    fn test_progress_config_default() {
        let config = ProgressConfig::default();
        assert!(matches!(config.style, ProgressStyle::Spinner));
        assert_eq!(config.message, "Processing...");
        assert!(config.show_elapsed);
        assert!(!config.show_eta);
        assert!(config.use_colors);
    }

    #[test]
    fn test_progress_indicator_creation() {
        let indicator = ProgressIndicator::spinner("Test operation", OutputFormat::Human, false);
        // Should create without panicking
    }

    #[test]
    fn test_progress_bar_creation() {
        let indicator = ProgressIndicator::bar("Test progress", 100, OutputFormat::Human, false);
        // Should create without panicking
    }

    #[test]
    fn test_steps_progress_creation() {
        let indicator = ProgressIndicator::steps("Test steps", 5, OutputFormat::Human, false);
        // Should create without panicking
    }

    #[test]
    fn test_multi_step_progress() {
        let steps = vec![
            "Step 1".to_string(),
            "Step 2".to_string(),
            "Step 3".to_string(),
        ];

        let mut progress = MultiStepProgress::new(steps, OutputFormat::Human, false);
        progress.start();

        progress.begin_step("Starting step 1");
        progress.update_operation("Doing work");
        progress.finish_step("Step 1 complete");

        progress.begin_step("Starting step 2");
        progress.finish_step_with_error("Step 2 failed");

        progress.finish("All done");
        // Should complete without panicking
    }

    #[test]
    fn test_format_duration() {
        // This would test the internal format_duration function
        // In a real implementation, we'd make it public or testable
    }
}

#[cfg(test)]
mod smart_defaults_tests {
    use std::fs;

    use tempfile::TempDir;

    use super::super::smart_defaults::*;

    #[test]
    fn test_project_context_detection() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Create a basic Rust crate
        fs::write(
            workspace_root.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        fs::create_dir_all(workspace_root.join("src")).unwrap();
        fs::write(workspace_root.join("src").join("lib.rs"), "// Test crate").unwrap();

        let detector = ContextDetector::new(workspace_root);
        let context = detector.detect().unwrap();

        assert!(matches!(context.project_type, ProjectType::RustCrate));
        assert!(!context.features.has_tests);
        assert!(!context.features.has_ci);
    }

    #[test]
    fn test_project_features_detection() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Create structure with features
        fs::create_dir_all(workspace_root.join("tests")).unwrap();
        fs::create_dir_all(workspace_root.join(".github/workflows")).unwrap();
        fs::write(workspace_root.join("README.md"), "# Test").unwrap();

        let detector = ContextDetector::new(workspace_root);
        let context = detector.detect().unwrap();

        assert!(context.features.has_tests);
        assert!(context.features.has_ci);
        assert!(context.features.has_docs);
    }

    #[test]
    fn test_smart_defaults() {
        let context = ProjectContext {
            workspace_root: std::path::PathBuf::from("/test"),
            project_type: ProjectType::WrtWorkspace,
            features: ProjectFeatures::default(),
            git_context: None,
            ci_context: None,
            recommendations: vec![],
        };

        let defaults = SmartDefaults::new(context);

        let default_command = defaults.suggest_default_command();
        assert_eq!(default_command, Some("build".to_string()));

        let output_format = defaults.suggest_output_format();
        assert_eq!(output_format, "human");

        let cache_settings = defaults.suggest_cache_settings();
        assert!(cache_settings); // WRT workspace should use cache
    }

    #[test]
    fn test_recommendation_priority() {
        use std::cmp::Ordering;

        assert_eq!(
            RecommendationPriority::Critical.cmp(&RecommendationPriority::High),
            Ordering::Less
        );
        assert_eq!(
            RecommendationPriority::High.cmp(&RecommendationPriority::Medium),
            Ordering::Less
        );

        assert_eq!(RecommendationPriority::Critical.emoji(), "🚨");
        assert_eq!(RecommendationPriority::Critical.name(), "Critical");
    }
}

#[cfg(test)]
mod command_suggestions_tests {
    use super::super::command_suggestions::*;

    #[test]
    fn test_suggestion_engine_creation() {
        let engine = CommandSuggestionEngine::new();
        // Should create without panicking
    }

    #[test]
    fn test_exact_command_match() {
        let engine = CommandSuggestionEngine::new();
        let suggestions = engine.suggest("build", None);

        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].command, "build");
        assert_eq!(suggestions[0].suggestion_type, SuggestionType::Exact);
        assert_eq!(suggestions[0].confidence, 1.0);
    }

    #[test]
    fn test_typo_correction() {
        let engine = CommandSuggestionEngine::new();

        // Test common typos
        let typos = vec![
            ("buld", "build"),
            ("tets", "test"),
            ("chekc", "check"),
            ("hlep", "help"),
        ];

        for (typo, expected) in typos {
            let suggestions = engine.suggest(typo, None);
            assert!(!suggestions.is_empty());
            assert_eq!(suggestions[0].command, expected);
            assert_eq!(
                suggestions[0].suggestion_type,
                SuggestionType::TypoCorrection
            );
        }
    }

    #[test]
    fn test_similarity_matching() {
        let engine = CommandSuggestionEngine::new();
        let suggestions = engine.suggest("bui", None);

        // Should find similar commands like "build"
        assert!(!suggestions.is_empty());
        let build_suggestion = suggestions
            .iter()
            .find(|s| s.command == "build")
            .expect("Should suggest 'build' for 'bui'");

        assert_eq!(build_suggestion.suggestion_type, SuggestionType::Similar);
        assert!(build_suggestion.confidence > 0.6);
    }

    #[test]
    fn test_empty_input() {
        let engine = CommandSuggestionEngine::new();
        let suggestions = engine.suggest("", None);

        // Should handle empty input gracefully
        // May return empty suggestions or default suggestions
    }

    #[test]
    fn test_suggestion_formatting() {
        let engine = CommandSuggestionEngine::new();
        let suggestions = engine.suggest("buld", None);

        let formatted = engine.format_suggestions(&suggestions, false);
        assert!(formatted.contains("Did you mean:"));

        let formatted_color = engine.format_suggestions(&suggestions, true);
        // Should contain ANSI color codes when colors are enabled
    }

    #[test]
    fn test_levenshtein_distance() {
        let engine = CommandSuggestionEngine::new();

        // Test similarity calculation
        let similarity1 = engine.calculate_similarity("build", "build");
        assert_eq!(similarity1, 1.0); // Exact match

        let similarity2 = engine.calculate_similarity("build", "buld");
        assert!(similarity2 > 0.8); // Very similar

        let similarity3 = engine.calculate_similarity("build", "xyz");
        assert!(similarity3 < 0.5); // Very different
    }
}

#[cfg(test)]
mod performance_tests {
    use std::time::Duration;

    use super::super::performance::*;

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert!(config.parallel_execution);
        assert!(config.max_parallel_jobs > 0);
        assert!(config.aggressive_caching);
        assert!(config.incremental_builds);
    }

    #[test]
    fn test_performance_optimizer_creation() {
        let optimizer = PerformanceOptimizer::with_defaults();
        // Should create without panicking
    }

    #[test]
    fn test_timer_functionality() {
        let mut optimizer = PerformanceOptimizer::with_defaults();

        optimizer.start_timer("test_command");
        std::thread::sleep(Duration::from_millis(10));
        optimizer.stop_timer("test_command");

        let report = optimizer.generate_report();
        assert!(report.metrics.command_times.contains_key("test_command"));

        let duration = report.metrics.command_times["test_command"];
        assert!(duration >= Duration::from_millis(10));
    }

    #[test]
    fn test_cache_tracking() {
        let mut optimizer = PerformanceOptimizer::with_defaults();

        // Initially no cache data
        assert_eq!(optimizer.cache_hit_ratio(), 0.0);

        // Add some cache data
        optimizer.record_cache_hit();
        optimizer.record_cache_miss();
        optimizer.record_cache_hit();

        let ratio = optimizer.cache_hit_ratio();
        assert!((ratio - 0.666).abs() < 0.01); // 2/3 ≈ 0.666
    }

    #[test]
    fn test_performance_recommendations() {
        let mut optimizer = PerformanceOptimizer::with_defaults();

        // Add data that should trigger recommendations
        optimizer.record_cache_miss();
        optimizer.record_cache_miss();
        optimizer.record_cache_miss();
        optimizer.record_cache_hit();

        // Add a slow command
        optimizer.start_timer("slow_command");
        std::thread::sleep(Duration::from_millis(100));
        optimizer.stop_timer("slow_command");

        let recommendations = optimizer.get_recommendations();
        // Should have some recommendations based on the data

        // Check for cache-related recommendation
        let cache_rec = recommendations
            .iter()
            .find(|r| matches!(r.category, RecommendationCategory::Caching));
        // Might have cache recommendation due to low hit ratio
    }

    #[test]
    fn test_performance_report_formatting() {
        let mut optimizer = PerformanceOptimizer::with_defaults();
        optimizer.start_timer("test");
        std::thread::sleep(Duration::from_millis(10));
        optimizer.stop_timer("test");

        let report = optimizer.generate_report();

        let human_format = report.format_human(false);
        assert!(human_format.contains("Performance Report"));
        assert!(human_format.contains("test"));

        let json_format = report.format_json().unwrap();
        assert!(json_format.contains("command_times"));
        assert!(json_format.contains("test"));
    }

    #[test]
    fn test_impact_levels() {
        assert_eq!(ImpactLevel::Critical.emoji(), "🔴");
        assert_eq!(ImpactLevel::High.emoji(), "🟠");
        assert_eq!(ImpactLevel::Medium.emoji(), "🟡");
        assert_eq!(ImpactLevel::Low.emoji(), "🟢");

        // Test ordering
        assert!(ImpactLevel::Critical < ImpactLevel::High);
        assert!(ImpactLevel::High < ImpactLevel::Medium);
        assert!(ImpactLevel::Medium < ImpactLevel::Low);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use wrt_build_core::formatters::OutputFormat;

    use super::super::{OutputManager, error_handling::*};

    #[test]
    fn test_error_category() {
        assert_eq!(ErrorCategory::Build.emoji(), "🔨");
        assert_eq!(ErrorCategory::Build.name(), "Build Error");
        assert_eq!(ErrorCategory::Build.color(), colored::Color::Red);
    }

    #[test]
    fn test_categorized_error_creation() {
        let error = CategorizedError::system_error("Test error message");

        assert_eq!(error.category, ErrorCategory::Build);
        assert_eq!(error.message, "Test error message");
        assert!(error.context.is_empty());
        assert!(error.suggestions.is_empty());
    }

    #[test]
    fn test_categorized_error_with_context() {
        let error = CategorizedError::configuration_error("Config error")
            .with_context("File: config.toml")
            .with_context("Line: 15")
            .with_suggestion("Check the configuration syntax")
            .with_suggestion("Refer to the documentation");

        assert_eq!(error.context.len(), 2);
        assert_eq!(error.suggestions.len(), 2);
        assert!(error.context.contains(&"File: config.toml".to_string()));
        assert!(error.suggestions.contains(&"Check the configuration syntax".to_string()));
    }

    #[test]
    fn test_error_formatting_human() {
        let error = CategorizedError::system_error("Missing dependency")
            .with_context("Required: rustc")
            .with_suggestion("Install rustc");

        let formatted = error.format_human(false);
        assert!(formatted.contains("Tool Error"));
        assert!(formatted.contains("Missing dependency"));
        assert!(formatted.contains("Required: rustc"));
        assert!(formatted.contains("Install rustc"));

        let formatted_color = error.format_human(true);
        // Should contain the same content, possibly with ANSI codes
        assert!(formatted_color.contains("Tool Error"));
    }

    #[test]
    fn test_error_formatting_json() {
        let error = CategorizedError::runtime_execution_error("Invalid input")
            .with_context("Input: invalid_value")
            .with_suggestion("Use valid input");

        let json = error.format_json();
        assert!(json.is_object());

        let json_str = serde_json::to_string(&json).unwrap();
        assert!(json_str.contains("Validation"));
        assert!(json_str.contains("Invalid input"));
        assert!(json_str.contains("Input: invalid_value"));
        assert!(json_str.contains("Use valid input"));
    }

    #[test]
    fn test_error_context_trait() {
        use anyhow::Result;

        fn failing_operation() -> Result<()> {
            Err(anyhow::anyhow!("Operation failed"))
        }

        let result = failing_operation()
            .build_context("Build operation failed")
            .config_context("Configuration error")
            .io_context("I/O error")
            .validation_context("Validation error")
            .tool_context("Tool error");

        assert!(result.is_err());
    }

    #[test]
    fn test_error_handler() {
        let output = OutputManager::new(OutputFormat::Human);
        let handler = ErrorHandler::new(&output);

        let error = anyhow::anyhow!("Test error");
        let formatted = handler.handle_error(&error);

        assert!(formatted.contains("Test error"));
    }

    #[test]
    fn test_build_errors() {
        let error = build_errors::compilation_failed("Syntax error in main.rs");
        assert_eq!(error.category, ErrorCategory::Build);
        assert!(error.message.contains("Compilation failed"));
        assert!(!error.suggestions.is_empty());

        let test_error = build_errors::test_failed("test_function");
        assert_eq!(test_error.category, ErrorCategory::Build);
        assert!(test_error.message.contains("test_function"));

        let dep_error = build_errors::dependency_missing("rustc");
        assert_eq!(dep_error.category, ErrorCategory::Tool);
        assert!(dep_error.message.contains("rustc"));
    }

    #[test]
    fn test_config_errors() {
        let invalid_error = config_errors::invalid_config_file("/path/to/config.toml");
        assert_eq!(invalid_error.category, ErrorCategory::Configuration);
        assert!(invalid_error.context.iter().any(|c| c.contains("/path/to/config.toml")));

        let missing_error = config_errors::missing_config_file("/path/to/missing.toml");
        assert_eq!(missing_error.category, ErrorCategory::Configuration);
        assert!(missing_error.context.iter().any(|c| c.contains("/path/to/missing.toml")));
    }
}

#[cfg(test)]
mod help_system_tests {
    use super::super::help_system::*;

    #[test]
    fn test_help_system_creation() {
        let help_system = HelpSystem::new();
        // Should create without panicking and have built-in commands
    }

    #[test]
    fn test_command_registration() {
        let mut help_system = HelpSystem::new();

        let test_doc = CommandDoc {
            name: "test-command",
            brief: "Test command",
            description: "This is a test command for unit testing",
            examples: vec![CommandExample {
                command: "cargo-wrt test-command",
                description: "Run test command",
                output_sample: None,
            }],
            see_also: vec!["help"],
            category: CommandCategory::Utility,
        };

        help_system.register_command(test_doc);

        let doc = help_system.get_command_doc("test-command");
        assert!(doc.is_some());
        assert_eq!(doc.unwrap().brief, "Test command");
    }

    #[test]
    fn test_command_help_formatting() {
        let help_system = HelpSystem::new();

        // Test with built-in command
        let help_text = help_system.format_command_help("build", false);
        assert!(help_text.is_some());

        let help_content = help_text.unwrap();
        assert!(help_content.contains("build"));
        assert!(help_content.contains("DESCRIPTION"));
        assert!(help_content.contains("EXAMPLES"));

        // Test with colored output
        let help_colored = help_system.format_command_help("build", true);
        assert!(help_colored.is_some());
    }

    #[test]
    fn test_overview_help() {
        let help_system = HelpSystem::new();

        let overview = help_system.format_overview_help(false);
        assert!(overview.contains("cargo-wrt"));
        assert!(overview.contains("Build Commands"));
        assert!(overview.contains("build"));
        assert!(overview.contains("test"));

        let overview_colored = help_system.format_overview_help(true);
        assert!(overview_colored.contains("cargo-wrt"));
    }

    #[test]
    fn test_command_categories() {
        assert_eq!(CommandCategory::Build.name(), "Build Commands");
        assert_eq!(CommandCategory::Build.emoji(), "🔨");

        assert_eq!(CommandCategory::Test.name(), "Testing Commands");
        assert_eq!(CommandCategory::Test.emoji(), "🧪");

        assert_eq!(
            CommandCategory::Verification.name(),
            "Verification Commands"
        );
        assert_eq!(CommandCategory::Verification.emoji(), "🛡️");
    }

    #[test]
    fn test_command_example() {
        let example = CommandExample {
            command: "cargo-wrt test --verbose",
            description: "Run tests with verbose output",
            output_sample: Some("Running tests...\ntest result: ok"),
        };

        assert_eq!(example.command, "cargo-wrt test --verbose");
        assert_eq!(example.description, "Run tests with verbose output");
        assert!(example.output_sample.is_some());
    }
}
