//! Standardized help and documentation system for cargo-wrt
//!
//! Provides consistent help formatting, command documentation,
//! and example generation across all cargo-wrt commands.

use std::collections::HashMap;

use colored::Colorize;

/// Command documentation structure
#[derive(Debug, Clone)]
pub struct CommandDoc {
    pub name: &'static str,
    pub brief: &'static str,
    pub description: &'static str,
    pub examples: Vec<CommandExample>,
    pub see_also: Vec<&'static str>,
    pub category: CommandCategory,
}

/// Command example with description
#[derive(Debug, Clone)]
pub struct CommandExample {
    pub command: &'static str,
    pub description: &'static str,
    pub output_sample: Option<&'static str>,
}

/// Command categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Build,
    Test,
    Verification,
    Documentation,
    Utility,
    Advanced,
}

impl CommandCategory {
    pub fn name(&self) -> &'static str {
        match self {
            CommandCategory::Build => "Build Commands",
            CommandCategory::Test => "Testing Commands",
            CommandCategory::Verification => "Verification Commands",
            CommandCategory::Documentation => "Documentation Commands",
            CommandCategory::Utility => "Utility Commands",
            CommandCategory::Advanced => "Advanced Commands",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            CommandCategory::Build => "🔨",
            CommandCategory::Test => "🧪",
            CommandCategory::Verification => "🛡️",
            CommandCategory::Documentation => "📚",
            CommandCategory::Utility => "🔧",
            CommandCategory::Advanced => "⚡",
        }
    }
}

/// Help system for managing command documentation
pub struct HelpSystem {
    commands: HashMap<&'static str, CommandDoc>,
}

impl HelpSystem {
    pub fn new() -> Self {
        let mut system = Self {
            commands: HashMap::new(),
        };
        system.register_builtin_commands();
        system
    }

    /// Register all built-in commands
    fn register_builtin_commands(&mut self) {
        // Build commands
        self.register_command(CommandDoc {
            name: "build",
            brief: "Build all WRT components",
            description: "Compiles the WRT runtime and all its components with support for \
                          package-specific builds, clippy integration, and format checking.",
            examples: vec![
                CommandExample {
                    command: "cargo-wrt build",
                    description: "Build all components",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt build --package wrt-foundation",
                    description: "Build only the wrt-foundation package",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt build --clippy --output json",
                    description: "Build with clippy checks and JSON diagnostic output",
                    output_sample: None,
                },
            ],
            see_also: vec!["check", "test", "verify"],
            category: CommandCategory::Build,
        });

        // Test commands
        self.register_command(CommandDoc {
            name: "test",
            brief: "Run tests across the workspace",
            description: "Executes unit tests, integration tests, and doc tests with support for \
                          filtering, package-specific testing, and detailed output control.",
            examples: vec![
                CommandExample {
                    command: "cargo-wrt test",
                    description: "Run all tests",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt test --package wrt-foundation --filter memory",
                    description: "Run memory-related tests in wrt-foundation",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt test --nocapture --unit-only",
                    description: "Run unit tests only with output capture disabled",
                    output_sample: None,
                },
            ],
            see_also: vec!["build", "verify", "coverage"],
            category: CommandCategory::Test,
        });

        // Verification commands
        self.register_command(CommandDoc {
            name: "verify",
            brief: "Run safety verification and compliance checks",
            description: "Performs comprehensive safety verification including ASIL compliance \
                          checks, formal verification with KANI, and Miri validation.",
            examples: vec![
                CommandExample {
                    command: "cargo-wrt verify --asil d",
                    description: "Run ASIL-D compliance verification",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt verify --asil c --detailed",
                    description: "Detailed ASIL-C verification with full report",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt verify --no-kani --no-miri",
                    description: "Basic verification without formal verification tools",
                    output_sample: None,
                },
            ],
            see_also: vec!["kani-verify", "safety", "check"],
            category: CommandCategory::Verification,
        });

        // Check commands
        self.register_command(CommandDoc {
            name: "check",
            brief: "Run static analysis and formatting checks",
            description: "Performs static analysis using clippy, formatting checks, and other \
                          code quality validations with auto-fix capabilities.",
            examples: vec![
                CommandExample {
                    command: "cargo-wrt check",
                    description: "Run standard static analysis",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt check --strict --fix",
                    description: "Strict checking with automatic fixes applied",
                    output_sample: None,
                },
                CommandExample {
                    command: "cargo-wrt check --output json --filter-severity error",
                    description: "JSON output showing only errors",
                    output_sample: None,
                },
            ],
            see_also: vec!["build", "verify", "autofix"],
            category: CommandCategory::Build,
        });
    }

    /// Register a new command
    pub fn register_command(&mut self, doc: CommandDoc) {
        self.commands.insert(doc.name, doc);
    }

    /// Get documentation for a specific command
    pub fn get_command_doc(&self, name: &str) -> Option<&CommandDoc> {
        self.commands.get(name)
    }

    /// Generate formatted help for a command
    pub fn format_command_help(&self, name: &str, use_colors: bool) -> Option<String> {
        let doc = self.get_command_doc(name)?;
        let mut output = String::new();

        // Command header
        if use_colors {
            output.push_str(&format!(
                "{} {}\n",
                doc.category.emoji(),
                doc.name.bright_blue().bold()
            ));
            output.push_str(&format!("  {}\n\n", doc.brief.bright_white()));
        } else {
            output.push_str(&format!("{} {}\n", doc.category.emoji(), doc.name));
            output.push_str(&format!("  {}\n\n", doc.brief));
        }

        // Description
        if use_colors {
            output.push_str(&format!("{}\n", "DESCRIPTION".bright_yellow().bold()));
        } else {
            output.push_str("DESCRIPTION\n");
        }
        output.push_str(&format!("  {}\n\n", doc.description));

        // Examples
        if !doc.examples.is_empty() {
            if use_colors {
                output.push_str(&format!("{}\n", "EXAMPLES".bright_yellow().bold()));
            } else {
                output.push_str("EXAMPLES\n");
            }

            for example in &doc.examples {
                if use_colors {
                    output.push_str(&format!("  {}\n", example.command.green()));
                } else {
                    output.push_str(&format!("  {}\n", example.command));
                }
                output.push_str(&format!("    {}\n", example.description));

                if let Some(sample) = example.output_sample {
                    output.push_str("    Output:\n");
                    for line in sample.lines() {
                        output.push_str(&format!("      {}\n", line));
                    }
                }
                output.push('\n');
            }
        }

        // See also
        if !doc.see_also.is_empty() {
            if use_colors {
                output.push_str(&format!("{}\n", "SEE ALSO".bright_yellow().bold()));
            } else {
                output.push_str("SEE ALSO\n");
            }
            output.push_str("  ");
            if use_colors {
                output.push_str(
                    &doc.see_also
                        .iter()
                        .map(|cmd| cmd.cyan().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            } else {
                output.push_str(&doc.see_also.join(", "));
            }
            output.push_str("\n\n");
        }

        Some(output)
    }

    /// Generate overview help showing all commands by category
    pub fn format_overview_help(&self, use_colors: bool) -> String {
        let mut output = String::new();

        // Header
        if use_colors {
            output.push_str(&format!(
                "{} {}\n\n",
                "🚀".bright_blue(),
                "cargo-wrt - WebAssembly Runtime Build System".bright_blue().bold()
            ));
        } else {
            output.push_str("🚀 cargo-wrt - WebAssembly Runtime Build System\n\n");
        }

        // Group commands by category
        let mut by_category: HashMap<CommandCategory, Vec<&CommandDoc>> = HashMap::new();
        for doc in self.commands.values() {
            by_category.entry(doc.category).or_insert_with(Vec::new).push(doc);
        }

        // Sort categories for consistent output
        let mut categories: Vec<_> = by_category.keys().cloned().collect();
        categories.sort_by_key(|cat| match cat {
            CommandCategory::Build => 0,
            CommandCategory::Test => 1,
            CommandCategory::Verification => 2,
            CommandCategory::Documentation => 3,
            CommandCategory::Utility => 4,
            CommandCategory::Advanced => 5,
        });

        for category in categories {
            if let Some(commands) = by_category.get(&category) {
                // Category header
                if use_colors {
                    output.push_str(&format!(
                        "{} {}\n",
                        category.emoji(),
                        category.name().bright_green().bold()
                    ));
                } else {
                    output.push_str(&format!("{} {}\n", category.emoji(), category.name()));
                }

                // Commands in category
                let mut sorted_commands = commands.clone();
                sorted_commands.sort_by_key(|doc| doc.name);

                for doc in sorted_commands {
                    if use_colors {
                        output.push_str(&format!("  {:<12} {}\n", doc.name.cyan(), doc.brief));
                    } else {
                        output.push_str(&format!("  {:<12} {}\n", doc.name, doc.brief));
                    }
                }
                output.push('\n');
            }
        }

        // Footer with global options hint
        if use_colors {
            output.push_str(&format!(
                "{}\n",
                "Run 'cargo-wrt help <command>' for detailed help on a specific command."
                    .bright_black()
            ));
            output.push_str(&format!(
                "{}\n",
                "Run 'cargo-wrt help diagnostics' for comprehensive diagnostic system guide."
                    .bright_black()
            ));
        } else {
            output.push_str(
                "Run 'cargo-wrt help <command>' for detailed help on a specific command.\n",
            );
            output.push_str(
                "Run 'cargo-wrt help diagnostics' for comprehensive diagnostic system guide.\n",
            );
        }

        output
    }
}

impl Default for HelpSystem {
    fn default() -> Self {
        Self::new()
    }
}
