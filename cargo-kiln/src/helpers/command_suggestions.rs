//! Command suggestion system for cargo-wrt
//!
//! Provides intelligent command suggestions based on context,
//! typo correction, and workflow patterns.

use std::collections::HashMap;

use colored::Colorize;

use super::{ProjectContext, RecommendationPriority};

/// Command suggestion engine
pub struct CommandSuggestionEngine {
    /// Known commands and their metadata
    commands: HashMap<String, CommandInfo>,
    /// Common typos and their corrections
    typo_corrections: HashMap<String, String>,
    /// Workflow patterns
    workflows: Vec<WorkflowPattern>,
}

/// Information about a command
#[derive(Debug, Clone)]
struct CommandInfo {
    name: String,
    aliases: Vec<String>,
    category: String,
    description: String,
    usage_frequency: f64,
    typical_context: Vec<String>,
}

/// Workflow pattern definition
#[derive(Debug, Clone)]
struct WorkflowPattern {
    name: String,
    description: String,
    commands: Vec<String>,
    context_triggers: Vec<String>,
}

/// Suggestion result
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub suggestion_type: SuggestionType,
    pub command: String,
    pub description: String,
    pub confidence: f64,
    pub reason: String,
}

/// Types of suggestions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionType {
    /// Exact command match
    Exact,
    /// Typo correction
    TypoCorrection,
    /// Similar command
    Similar,
    /// Workflow suggestion
    Workflow,
    /// Context-based suggestion
    Contextual,
}

impl CommandSuggestionEngine {
    /// Create a new suggestion engine
    pub fn new() -> Self {
        let mut engine = Self {
            commands: HashMap::new(),
            typo_corrections: HashMap::new(),
            workflows: Vec::new(),
        };

        engine.initialize_commands();
        engine.initialize_typo_corrections();
        engine.initialize_workflows();
        engine
    }

    /// Initialize known commands
    fn initialize_commands(&mut self) {
        let commands = vec![
            CommandInfo {
                name: "build".to_string(),
                aliases: vec!["b".to_string(), "compile".to_string()],
                category: "Build".to_string(),
                description: "Build all WRT components".to_string(),
                usage_frequency: 0.9,
                typical_context: vec!["development".to_string(), "ci".to_string()],
            },
            CommandInfo {
                name: "test".to_string(),
                aliases: vec!["t".to_string(), "tests".to_string()],
                category: "Test".to_string(),
                description: "Run tests across the workspace".to_string(),
                usage_frequency: 0.8,
                typical_context: vec!["development".to_string(), "ci".to_string()],
            },
            CommandInfo {
                name: "check".to_string(),
                aliases: vec!["c".to_string(), "lint".to_string()],
                category: "Quality".to_string(),
                description: "Run static analysis and formatting checks".to_string(),
                usage_frequency: 0.7,
                typical_context: vec!["development".to_string(), "pre-commit".to_string()],
            },
            CommandInfo {
                name: "verify".to_string(),
                aliases: vec!["v".to_string(), "safety".to_string()],
                category: "Verification".to_string(),
                description: "Run safety verification and compliance checks".to_string(),
                usage_frequency: 0.6,
                typical_context: vec!["ci".to_string(), "release".to_string()],
            },
            CommandInfo {
                name: "clean".to_string(),
                aliases: vec!["clear".to_string()],
                category: "Maintenance".to_string(),
                description: "Clean build artifacts".to_string(),
                usage_frequency: 0.3,
                typical_context: vec!["troubleshooting".to_string()],
            },
            CommandInfo {
                name: "docs".to_string(),
                aliases: vec!["doc".to_string(), "documentation".to_string()],
                category: "Documentation".to_string(),
                description: "Generate documentation".to_string(),
                usage_frequency: 0.4,
                typical_context: vec!["development".to_string(), "release".to_string()],
            },
            CommandInfo {
                name: "help".to_string(),
                aliases: vec!["h".to_string(), "--help".to_string()],
                category: "Help".to_string(),
                description: "Show help information".to_string(),
                usage_frequency: 0.5,
                typical_context: vec!["learning".to_string()],
            },
        ];

        for cmd in commands {
            self.commands.insert(cmd.name.clone(), cmd);
        }
    }

    /// Initialize common typo corrections
    fn initialize_typo_corrections(&mut self) {
        let corrections = vec![
            ("buld", "build"),
            ("biuld", "build"),
            ("buidl", "build"),
            ("tets", "test"),
            ("tesst", "test"),
            ("teste", "test"),
            ("chekc", "check"),
            ("chck", "check"),
            ("verufy", "verify"),
            ("varify", "verify"),
            ("verfiy", "verify"),
            ("claen", "clean"),
            ("clen", "clean"),
            ("hlep", "help"),
            ("hepl", "help"),
            ("dcuos", "docs"),
            ("odcs", "docs"),
        ];

        for (typo, correction) in corrections {
            self.typo_corrections.insert(typo.to_string(), correction.to_string());
        }
    }

    /// Initialize workflow patterns
    fn initialize_workflows(&mut self) {
        self.workflows = vec![
            WorkflowPattern {
                name: "Development Cycle".to_string(),
                description: "Typical development workflow".to_string(),
                commands: vec!["build".to_string(), "test".to_string(), "check".to_string()],
                context_triggers: vec!["development".to_string(), "feature".to_string()],
            },
            WorkflowPattern {
                name: "CI/CD Pipeline".to_string(),
                description: "Continuous integration workflow".to_string(),
                commands: vec![
                    "build".to_string(),
                    "test".to_string(),
                    "verify".to_string(),
                ],
                context_triggers: vec!["ci".to_string(), "pipeline".to_string()],
            },
            WorkflowPattern {
                name: "Release Preparation".to_string(),
                description: "Preparing for a release".to_string(),
                commands: vec!["verify".to_string(), "docs".to_string(), "test".to_string()],
                context_triggers: vec!["release".to_string(), "tag".to_string()],
            },
            WorkflowPattern {
                name: "Quick Check".to_string(),
                description: "Quick validation before commit".to_string(),
                commands: vec!["check".to_string(), "test".to_string()],
                context_triggers: vec!["commit".to_string(), "quick".to_string()],
            },
        ];
    }

    /// Suggest commands based on input
    pub fn suggest(&self, input: &str, context: Option<&ProjectContext>) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        let input_lower = input.to_lowercase();

        // Exact match
        if let Some(cmd) = self.commands.get(&input_lower) {
            suggestions.push(Suggestion {
                suggestion_type: SuggestionType::Exact,
                command: cmd.name.clone(),
                description: cmd.description.clone(),
                confidence: 1.0,
                reason: "Exact match".to_string(),
            });
            return suggestions;
        }

        // Alias match
        for cmd in self.commands.values() {
            if cmd.aliases.contains(&input_lower) {
                suggestions.push(Suggestion {
                    suggestion_type: SuggestionType::Exact,
                    command: cmd.name.clone(),
                    description: cmd.description.clone(),
                    confidence: 1.0,
                    reason: format!("Alias for '{}'", cmd.name),
                });
                return suggestions;
            }
        }

        // Typo correction
        if let Some(correction) = self.typo_corrections.get(&input_lower) {
            if let Some(cmd) = self.commands.get(correction) {
                suggestions.push(Suggestion {
                    suggestion_type: SuggestionType::TypoCorrection,
                    command: cmd.name.clone(),
                    description: cmd.description.clone(),
                    confidence: 0.9,
                    reason: format!("Did you mean '{}'?", cmd.name),
                });
            }
        }

        // Fuzzy matching for similar commands
        for cmd in self.commands.values() {
            let similarity = self.calculate_similarity(&input_lower, &cmd.name);
            if similarity > 0.6 {
                suggestions.push(Suggestion {
                    suggestion_type: SuggestionType::Similar,
                    command: cmd.name.clone(),
                    description: cmd.description.clone(),
                    confidence: similarity,
                    reason: format!("Similar to '{}'", input),
                });
            }
        }

        // Context-based suggestions
        if let Some(ctx) = context {
            suggestions.extend(self.suggest_from_context(ctx));
        }

        // Workflow suggestions
        suggestions.extend(self.suggest_workflows(&input_lower));

        // Sort by confidence and limit results
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.truncate(5);

        suggestions
    }

    /// Calculate similarity between two strings
    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        let max_len = a.len().max(b.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = self.levenshtein_distance(a, b);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Calculate Levenshtein distance
    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        let mut dp = vec![vec![0; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            dp[i][0] = i;
        }
        for j in 0..=b_len {
            dp[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1).min(dp[i][j - 1] + 1).min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[a_len][b_len]
    }

    /// Suggest commands based on project context
    fn suggest_from_context(&self, context: &ProjectContext) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Suggest based on recommendations
        for rec in &context.recommendations {
            if let Some(command) = &rec.command {
                let command_name = command.split_whitespace().next().unwrap_or(command);
                if let Some(cmd_info) = self.commands.get(command_name) {
                    let confidence = match rec.priority {
                        RecommendationPriority::Critical => 0.95,
                        RecommendationPriority::High => 0.85,
                        RecommendationPriority::Medium => 0.75,
                        RecommendationPriority::Low => 0.65,
                        RecommendationPriority::Suggestion => 0.55,
                    };

                    suggestions.push(Suggestion {
                        suggestion_type: SuggestionType::Contextual,
                        command: command.clone(),
                        description: rec.description.clone(),
                        confidence,
                        reason: format!("{} priority recommendation", rec.priority.name()),
                    });
                }
            }
        }

        suggestions
    }

    /// Suggest workflow patterns
    fn suggest_workflows(&self, input: &str) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        for workflow in &self.workflows {
            // Check if input matches workflow triggers
            for trigger in &workflow.context_triggers {
                if input.contains(trigger) || trigger.contains(input) {
                    for cmd in &workflow.commands {
                        if let Some(cmd_info) = self.commands.get(cmd) {
                            suggestions.push(Suggestion {
                                suggestion_type: SuggestionType::Workflow,
                                command: cmd.clone(),
                                description: format!(
                                    "{} (part of {})",
                                    cmd_info.description, workflow.name
                                ),
                                confidence: 0.7,
                                reason: format!("Suggested by {} workflow", workflow.name),
                            });
                        }
                    }
                    break;
                }
            }
        }

        suggestions
    }

    /// Format suggestions for display
    pub fn format_suggestions(&self, suggestions: &[Suggestion], use_colors: bool) -> String {
        if suggestions.is_empty() {
            return if use_colors {
                "No suggestions available. Try 'cargo-wrt help' for available commands."
                    .bright_black()
                    .to_string()
            } else {
                "No suggestions available. Try 'cargo-wrt help' for available commands.".to_string()
            };
        }

        let mut output = String::new();

        if use_colors {
            output.push_str(&format!("{}\n", "Did you mean:".bright_yellow().bold()));
        } else {
            output.push_str("Did you mean:\n");
        }

        for (i, suggestion) in suggestions.iter().enumerate() {
            let icon = match suggestion.suggestion_type {
                SuggestionType::Exact => "✅",
                SuggestionType::TypoCorrection => "📝",
                SuggestionType::Similar => "🔍",
                SuggestionType::Workflow => "🔄",
                SuggestionType::Contextual => "💡",
            };

            if use_colors {
                output.push_str(&format!(
                    "  {} {}: {} {}\n",
                    format!("{}", i + 1).bright_blue(),
                    icon,
                    suggestion.command.bright_green(),
                    suggestion.description.bright_white()
                ));
                output.push_str(&format!(
                    "     {} (confidence: {:.0}%)\n",
                    suggestion.reason.bright_black(),
                    suggestion.confidence * 100.0
                ));
            } else {
                output.push_str(&format!(
                    "  {}: {} {}: {}\n",
                    i + 1,
                    icon,
                    suggestion.command,
                    suggestion.description
                ));
                output.push_str(&format!(
                    "     {} (confidence: {:.0}%)\n",
                    suggestion.reason,
                    suggestion.confidence * 100.0
                ));
            }
        }

        output.push('\n');
        if use_colors {
            output.push_str(&format!(
                "Run: {}\n",
                format!("cargo-wrt <command>").bright_cyan()
            ));
        } else {
            output.push_str("Run: cargo-wrt <command>\n");
        }

        output
    }
}

impl Default for CommandSuggestionEngine {
    fn default() -> Self {
        Self::new()
    }
}
