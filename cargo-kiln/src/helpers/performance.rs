//! Performance optimization utilities for cargo-wrt
//!
//! Provides tools for optimizing build performance, caching strategies,
//! and resource usage monitoring.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Performance metrics collector
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub command_times: HashMap<String, Duration>,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub memory_peak: usize,
    pub disk_reads: usize,
    pub disk_writes: usize,
    pub network_requests: usize,
}

/// Performance optimization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable parallel execution where possible
    pub parallel_execution: bool,

    /// Maximum number of parallel jobs
    pub max_parallel_jobs: usize,

    /// Enable aggressive caching
    pub aggressive_caching: bool,

    /// Cache TTL in seconds
    pub cache_ttl: u64,

    /// Enable build artifact reuse
    pub artifact_reuse: bool,

    /// Memory usage limit in MB
    pub memory_limit_mb: Option<usize>,

    /// Enable incremental builds
    pub incremental_builds: bool,

    /// Enable compiler cache (sccache/ccache)
    pub compiler_cache: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            parallel_execution: true,
            max_parallel_jobs: num_cpus::get().max(1),
            aggressive_caching: true,
            cache_ttl: 3600, // 1 hour
            artifact_reuse: true,
            memory_limit_mb: None,
            incremental_builds: true,
            compiler_cache: true,
        }
    }
}

/// Performance optimizer
pub struct PerformanceOptimizer {
    config: PerformanceConfig,
    metrics: PerformanceMetrics,
    command_timers: HashMap<String, Instant>,
}

impl PerformanceOptimizer {
    /// Create a new performance optimizer
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            metrics: PerformanceMetrics::default(),
            command_timers: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(PerformanceConfig::default())
    }

    /// Start timing a command
    pub fn start_timer(&mut self, command: &str) {
        self.command_timers.insert(command.to_string(), Instant::now());
    }

    /// Stop timing a command and record duration
    pub fn stop_timer(&mut self, command: &str) {
        if let Some(start_time) = self.command_timers.remove(command) {
            let duration = start_time.elapsed();
            self.metrics.command_times.insert(command.to_string(), duration);
        }
    }

    /// Record cache hit
    pub fn record_cache_hit(&mut self) {
        self.metrics.cache_hits += 1;
    }

    /// Record cache miss
    pub fn record_cache_miss(&mut self) {
        self.metrics.cache_misses += 1;
    }

    /// Get cache hit ratio
    pub fn cache_hit_ratio(&self) -> f64 {
        let total = self.metrics.cache_hits + self.metrics.cache_misses;
        if total == 0 { 0.0 } else { self.metrics.cache_hits as f64 / total as f64 }
    }

    /// Get performance recommendations
    pub fn get_recommendations(&self) -> Vec<PerformanceRecommendation> {
        let mut recommendations = Vec::new();

        // Cache performance
        let cache_ratio = self.cache_hit_ratio();
        if cache_ratio < 0.5 && self.metrics.cache_hits + self.metrics.cache_misses > 10 {
            recommendations.push(PerformanceRecommendation {
                category: RecommendationCategory::Caching,
                title: "Low Cache Hit Ratio".to_string(),
                description: format!(
                    "Cache hit ratio is {:.1}%. Consider enabling more aggressive caching.",
                    cache_ratio * 100.0
                ),
                impact: ImpactLevel::Medium,
                action: "Enable aggressive caching with --cache --aggressive".to_string(),
            });
        }

        // Parallel execution
        if !self.config.parallel_execution {
            recommendations.push(PerformanceRecommendation {
                category: RecommendationCategory::Parallelization,
                title: "Parallel Execution Disabled".to_string(),
                description: "Parallel execution could significantly improve build times"
                    .to_string(),
                impact: ImpactLevel::High,
                action: "Enable parallel execution in configuration".to_string(),
            });
        }

        // Command performance analysis
        for (command, duration) in &self.metrics.command_times {
            if duration.as_secs() > 30 {
                recommendations.push(PerformanceRecommendation {
                    category: RecommendationCategory::CommandOptimization,
                    title: format!("Slow {} Command", command),
                    description: format!(
                        "Command '{}' took {:.1}s. Consider optimization.",
                        command,
                        duration.as_secs_f64()
                    ),
                    impact: ImpactLevel::Medium,
                    action: format!("Profile {} command for bottlenecks", command),
                });
            }
        }

        // Memory usage (if available)
        if let Some(limit) = self.config.memory_limit_mb {
            if self.metrics.memory_peak > limit {
                recommendations.push(PerformanceRecommendation {
                    category: RecommendationCategory::Memory,
                    title: "Memory Limit Exceeded".to_string(),
                    description: format!(
                        "Peak memory usage ({} MB) exceeded limit ({} MB)",
                        self.metrics.memory_peak, limit
                    ),
                    impact: ImpactLevel::High,
                    action: "Increase memory limit or optimize memory usage".to_string(),
                });
            }
        }

        recommendations.sort_by_key(|r| r.impact.clone());
        recommendations
    }

    /// Generate performance report
    pub fn generate_report(&self) -> PerformanceReport {
        PerformanceReport {
            metrics: self.metrics.clone(),
            config: self.config.clone(),
            recommendations: self.get_recommendations(),
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Optimize configuration based on system
    pub fn optimize_for_system(&mut self) -> Result<()> {
        // Detect system capabilities
        let cpu_count = num_cpus::get();
        let available_memory = self.get_available_memory_mb()?;

        // Adjust parallel jobs
        if self.config.max_parallel_jobs > cpu_count * 2 {
            self.config.max_parallel_jobs = cpu_count;
        }

        // Adjust memory settings
        if self.config.memory_limit_mb.is_none() && available_memory > 1024 {
            self.config.memory_limit_mb = Some(available_memory / 2);
        }

        // Enable compiler cache if available
        if self.is_compiler_cache_available() {
            self.config.compiler_cache = true;
        }

        Ok(())
    }

    /// Get available memory in MB (simplified)
    fn get_available_memory_mb(&self) -> Result<usize> {
        // Simplified implementation - would use system APIs in practice
        Ok(8192) // Default to 8GB
    }

    /// Check if compiler cache is available
    fn is_compiler_cache_available(&self) -> bool {
        // Check for sccache or ccache
        which::which("sccache").is_ok() || which::which("ccache").is_ok()
    }
}

/// Performance recommendation
#[derive(Debug, Clone)]
pub struct PerformanceRecommendation {
    pub category: RecommendationCategory,
    pub title: String,
    pub description: String,
    pub impact: ImpactLevel,
    pub action: String,
}

/// Recommendation categories
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendationCategory {
    Caching,
    Parallelization,
    Memory,
    Disk,
    Network,
    CommandOptimization,
    Configuration,
}

/// Impact levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactLevel {
    Critical,
    High,
    Medium,
    Low,
}

impl ImpactLevel {
    pub fn emoji(&self) -> &'static str {
        match self {
            ImpactLevel::Critical => "🔴",
            ImpactLevel::High => "🟠",
            ImpactLevel::Medium => "🟡",
            ImpactLevel::Low => "🟢",
        }
    }
}

/// Performance report
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub metrics: PerformanceMetrics,
    pub config: PerformanceConfig,
    pub recommendations: Vec<PerformanceRecommendation>,
    pub timestamp: std::time::SystemTime,
}

impl PerformanceReport {
    /// Format report for human consumption
    pub fn format_human(&self, use_colors: bool) -> String {
        use colored::Colorize;

        let mut output = String::new();

        // Header
        if use_colors {
            output.push_str(&format!(
                "{} {}\n\n",
                "📊".bright_blue(),
                "Performance Report".bright_blue().bold()
            ));
        } else {
            output.push_str("📊 Performance Report\n\n");
        }

        // Metrics summary
        if use_colors {
            output.push_str(&format!("{}\n", "Metrics:".bright_yellow().bold()));
        } else {
            output.push_str("Metrics:\n");
        }

        if !self.metrics.command_times.is_empty() {
            output.push_str("  Command Times:\n");
            for (command, duration) in &self.metrics.command_times {
                output.push_str(&format!(
                    "    {}: {:.2}s\n",
                    command,
                    duration.as_secs_f64()
                ));
            }
        }

        let cache_total = self.metrics.cache_hits + self.metrics.cache_misses;
        if cache_total > 0 {
            let cache_ratio = self.metrics.cache_hits as f64 / cache_total as f64 * 100.0;
            output.push_str(&format!(
                "  Cache Hit Ratio: {:.1}% ({}/{} hits)\n",
                cache_ratio, self.metrics.cache_hits, cache_total
            ));
        }

        if self.metrics.memory_peak > 0 {
            output.push_str(&format!("  Peak Memory: {} MB\n", self.metrics.memory_peak));
        }

        // Recommendations
        if !self.recommendations.is_empty() {
            output.push('\n');
            if use_colors {
                output.push_str(&format!("{}\n", "Recommendations:".bright_yellow().bold()));
            } else {
                output.push_str("Recommendations:\n");
            }

            for rec in &self.recommendations {
                if use_colors {
                    output.push_str(&format!(
                        "  {} {} {}\n",
                        rec.impact.emoji(),
                        rec.title.bright_white().bold(),
                        format!("({})", rec.impact.emoji()).bright_black()
                    ));
                    output.push_str(&format!("    {}\n", rec.description.bright_white()));
                    output.push_str(&format!("    Action: {}\n\n", rec.action.bright_cyan()));
                } else {
                    output.push_str(&format!("  {} {}\n", rec.impact.emoji(), rec.title));
                    output.push_str(&format!("    {}\n", rec.description));
                    output.push_str(&format!("    Action: {}\n\n", rec.action));
                }
            }
        }

        output
    }

    /// Format report as JSON
    pub fn format_json(&self) -> Result<String> {
        #[derive(Serialize)]
        struct JsonReport {
            timestamp: String,
            metrics: JsonMetrics,
            recommendations: Vec<JsonRecommendation>,
        }

        #[derive(Serialize)]
        struct JsonMetrics {
            command_times: HashMap<String, f64>,
            cache_hits: usize,
            cache_misses: usize,
            cache_hit_ratio: f64,
            memory_peak_mb: usize,
        }

        #[derive(Serialize)]
        struct JsonRecommendation {
            category: String,
            title: String,
            description: String,
            impact: String,
            action: String,
        }

        let cache_total = self.metrics.cache_hits + self.metrics.cache_misses;
        let cache_ratio = if cache_total > 0 {
            self.metrics.cache_hits as f64 / cache_total as f64
        } else {
            0.0
        };

        let report = JsonReport {
            timestamp: format!("{:?}", self.timestamp),
            metrics: JsonMetrics {
                command_times: self
                    .metrics
                    .command_times
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_secs_f64()))
                    .collect(),
                cache_hits: self.metrics.cache_hits,
                cache_misses: self.metrics.cache_misses,
                cache_hit_ratio: cache_ratio,
                memory_peak_mb: self.metrics.memory_peak,
            },
            recommendations: self
                .recommendations
                .iter()
                .map(|r| JsonRecommendation {
                    category: format!("{:?}", r.category),
                    title: r.title.clone(),
                    description: r.description.clone(),
                    impact: format!("{:?}", r.impact),
                    action: r.action.clone(),
                })
                .collect(),
        };

        Ok(serde_json::to_string_pretty(&report)?)
    }
}
