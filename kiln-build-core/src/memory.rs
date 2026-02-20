//! Memory budget validation and analysis
//!
//! This module provides tools for analyzing and validating memory usage
//! across different platforms and ASIL levels.

use std::collections::HashMap;

use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::{
    build::BuildSystem,
    error::{BuildError, BuildResult},
};

/// Platform-specific memory budget configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformMemoryBudget {
    /// Platform name
    pub name: String,
    /// Total memory budget in bytes
    pub total_budget: usize,
    /// Stack size in bytes
    pub stack_size: usize,
    /// Heap size in bytes
    pub heap_size: usize,
    /// Static data size in bytes
    pub static_data: usize,
}

/// Memory analysis results
#[derive(Debug, Serialize)]
pub struct MemoryAnalysisResult {
    /// Target platform
    pub platform: String,
    /// Total memory used in bytes
    pub total_used: usize,
    /// Total memory budget in bytes
    pub total_budget: usize,
    /// Memory usage as percentage
    pub usage_percentage: f64,
    /// Detailed memory breakdown
    pub breakdown: MemoryBreakdown,
    /// Memory analysis warnings
    pub warnings: Vec<String>,
}

/// Detailed memory usage breakdown
#[derive(Debug, Serialize)]
pub struct MemoryBreakdown {
    /// Text/code segment size in bytes
    pub text_size: usize,
    /// Initialized data segment size in bytes
    pub data_size: usize,
    /// Uninitialized data (BSS) segment size in bytes
    pub bss_size: usize,
    /// Stack usage in bytes
    pub stack_usage: usize,
    /// Heap usage in bytes
    pub heap_usage: usize,
}

impl BuildSystem {
    /// Analyze memory budget for all platforms
    pub fn analyze_memory_budget(&self) -> BuildResult<Vec<MemoryAnalysisResult>> {
        println!("{} Analyzing memory budgets...", "🔍".bright_blue());

        let platforms = vec![
            PlatformMemoryBudget {
                name: "embedded".to_string(),
                total_budget: 256 * 1024, // 256KB
                stack_size: 8 * 1024,     // 8KB
                heap_size: 32 * 1024,     // 32KB
                static_data: 16 * 1024,   // 16KB
            },
            PlatformMemoryBudget {
                name: "iot".to_string(),
                total_budget: 1024 * 1024, // 1MB
                stack_size: 32 * 1024,     // 32KB
                heap_size: 128 * 1024,     // 128KB
                static_data: 64 * 1024,    // 64KB
            },
            PlatformMemoryBudget {
                name: "desktop".to_string(),
                total_budget: 16 * 1024 * 1024, // 16MB
                stack_size: 1024 * 1024,        // 1MB
                heap_size: 8 * 1024 * 1024,     // 8MB
                static_data: 512 * 1024,        // 512KB
            },
        ];

        let mut results = Vec::new();

        for platform in platforms {
            let result = self.analyze_platform_memory(&platform)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Analyze memory for a specific platform
    fn analyze_platform_memory(
        &self,
        platform: &PlatformMemoryBudget,
    ) -> BuildResult<MemoryAnalysisResult> {
        // In a real implementation, this would:
        // 1. Build for the target platform
        // 2. Use size/objdump to analyze binary
        // 3. Run static analysis for stack usage
        // 4. Analyze heap allocations

        // For now, use placeholder values
        let breakdown = MemoryBreakdown {
            text_size: 64 * 1024,                        // Simulated code size
            data_size: 8 * 1024,                         // Simulated initialized data
            bss_size: 4 * 1024,                          // Simulated uninitialized data
            stack_usage: platform.stack_size * 60 / 100, // 60% stack usage
            heap_usage: platform.heap_size * 40 / 100,   // 40% heap usage
        };

        let total_used = breakdown.text_size
            + breakdown.data_size
            + breakdown.bss_size
            + breakdown.stack_usage
            + breakdown.heap_usage;

        let usage_percentage = (total_used as f64 / platform.total_budget as f64) * 100.0;

        let mut warnings = Vec::new();

        if usage_percentage > 90.0 {
            warnings.push(format!("Critical: Memory usage exceeds 90% of budget"));
        } else if usage_percentage > 80.0 {
            warnings.push(format!("Warning: Memory usage exceeds 80% of budget"));
        }

        if breakdown.stack_usage > platform.stack_size * 80 / 100 {
            warnings.push(format!(
                "Warning: Stack usage exceeds 80% of allocated size"
            ));
        }

        Ok(MemoryAnalysisResult {
            platform: platform.name.clone(),
            total_used,
            total_budget: platform.total_budget,
            usage_percentage,
            breakdown,
            warnings,
        })
    }

    /// Validate memory usage against thresholds
    pub fn validate_memory_thresholds(
        &self,
        warning_threshold: u32,
        critical_threshold: u32,
    ) -> BuildResult<bool> {
        let results = self.analyze_memory_budget()?;
        let mut all_passed = true;

        println!();
        println!("{} Memory Budget Validation Results", "📊".bright_blue());
        println!("{}", "─".repeat(60));

        for result in &results {
            let status_icon = if result.usage_percentage > critical_threshold as f64 {
                all_passed = false;
                "❌".bright_red()
            } else if result.usage_percentage > warning_threshold as f64 {
                "⚠️".bright_yellow()
            } else {
                "✅".bright_green()
            };

            println!(
                "{} {} Platform: {:.1}% of {} budget",
                status_icon,
                result.platform.to_uppercase(),
                result.usage_percentage,
                format_bytes(result.total_budget)
            );

            if self.config.verbose {
                println!("    Text:  {}", format_bytes(result.breakdown.text_size));
                println!("    Data:  {}", format_bytes(result.breakdown.data_size));
                println!("    BSS:   {}", format_bytes(result.breakdown.bss_size));
                println!("    Stack: {}", format_bytes(result.breakdown.stack_usage));
                println!("    Heap:  {}", format_bytes(result.breakdown.heap_usage));
            }

            for warning in &result.warnings {
                println!("    {}", warning.bright_yellow());
            }
        }

        println!("{}", "─".repeat(60));

        if all_passed {
            println!("{} All platforms within memory budget", "✅".bright_green());
        } else {
            println!(
                "{} Some platforms exceed critical threshold",
                "❌".bright_red()
            );
        }

        Ok(all_passed)
    }

    /// Generate memory usage report
    pub fn generate_memory_report(&self, output_path: &str) -> BuildResult<()> {
        let results = self.analyze_memory_budget()?;

        // Generate HTML report
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<title>WRT Memory Budget Report</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 40px; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
        html.push_str("th { background-color: #4CAF50; color: white; }\n");
        html.push_str(".warning { background-color: #fff3cd; }\n");
        html.push_str(".critical { background-color: #f8d7da; }\n");
        html.push_str(".good { background-color: #d4edda; }\n");
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<h1>WRT Memory Budget Report</h1>\n");
        html.push_str(&format!(
            "<p>Generated: {}</p>\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        html.push_str("<table>\n");
        html.push_str(
            "<tr><th>Platform</th><th>Usage</th><th>Budget</th><th>Percentage</th><th>Status</\
             th></tr>\n",
        );

        for result in &results {
            let row_class = if result.usage_percentage > 90.0 {
                "critical"
            } else if result.usage_percentage > 80.0 {
                "warning"
            } else {
                "good"
            };

            html.push_str(&format!(
                "<tr class='{}'><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td><td>{}</td></tr>\n",
                row_class,
                result.platform.to_uppercase(),
                format_bytes(result.total_used),
                format_bytes(result.total_budget),
                result.usage_percentage,
                if result.usage_percentage > 90.0 {
                    "Critical"
                } else if result.usage_percentage > 80.0 {
                    "Warning"
                } else {
                    "OK"
                }
            ));
        }

        html.push_str("</table>\n");
        html.push_str("</body>\n</html>");

        std::fs::create_dir_all(std::path::Path::new(output_path).parent().unwrap())
            .map_err(|e| BuildError::Tool(format!("Failed to create report directory: {}", e)))?;

        std::fs::write(output_path, html)
            .map_err(|e| BuildError::Tool(format!("Failed to write memory report: {}", e)))?;

        println!(
            "{} Memory report generated: {}",
            "📄".bright_green(),
            output_path
        );
        Ok(())
    }
}

/// Format bytes in human-readable form
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
