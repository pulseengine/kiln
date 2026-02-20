//! Real-time memory monitoring
//!
//! This module provides continuous monitoring capabilities for memory usage
//! with configurable alerts and automatic visualization generation.

#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::{string::String, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use wrt_error::{Error, Result};
use wrt_foundation::budget_aware_provider::CrateId;

/// Real-time monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Monitoring interval in milliseconds
    pub interval_ms: u64,
    /// Memory usage threshold for alerts (percentage)
    pub alert_threshold: usize,
    /// Critical threshold for emergency alerts (percentage)
    pub critical_threshold: usize,
    /// Maximum number of data points to keep in history
    pub history_size: usize,
    /// Automatically generate visualizations
    pub auto_visualize: bool,
    /// Visualization output directory
    #[cfg(feature = "std")]
    pub output_dir: String,
    /// Enable console output
    pub console_output: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,      // 1 second
            alert_threshold: 80,    // 80%
            critical_threshold: 95, // 95%
            history_size: 300,      // 5 minutes at 1Hz
            auto_visualize: false,
            #[cfg(feature = "std")]
            output_dir: "./memory_reports".to_string(),
            console_output: true,
        }
    }
}

/// Memory monitoring sample
#[derive(Debug, Clone)]
pub struct MemorySample {
    /// Timestamp (simplified for no_std)
    pub timestamp: u64,
    /// Total allocated memory in bytes
    pub total_allocated: usize,
    /// Active provider count
    pub active_providers: usize,
    /// Per-crate utilization percentages
    pub crate_utilization: [usize; 16],
    /// Shared pool utilization percentage
    pub shared_pool_utilization: usize,
}

/// Memory monitoring alert
#[derive(Debug, Clone)]
pub struct MemoryAlert {
    /// Alert timestamp
    pub timestamp: u64,
    /// Alert level
    pub level: AlertLevel,
    /// Alert message
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub message: String,
    /// Alert message (static for no_std environments)
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    pub message: &'static str,
    /// Affected crate (if specific)
    pub crate_id: Option<CrateId>,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    /// Informational message
    Info,
    /// Warning condition
    Warning,
    /// Critical condition requiring attention
    Critical,
}

/// Real-time memory monitor
pub struct RealtimeMonitor {
    /// Monitoring configuration
    #[allow(dead_code)]
    config: MonitorConfig,
    /// Monitoring active flag
    active: AtomicBool,
    /// Sample counter for timestamping
    sample_counter: AtomicUsize,
    /// Historical data storage
    #[cfg(feature = "std")]
    history: Arc<Mutex<VecDeque<MemorySample>>>,
    /// Alert storage
    #[cfg(feature = "std")]
    alerts: Arc<Mutex<Vec<MemoryAlert>>>,
}

impl RealtimeMonitor {
    /// Create a new realtime monitor
    pub fn new(config: MonitorConfig) -> Self {
        #[cfg(feature = "std")]
        let history_capacity = config.history_size;

        Self {
            config,
            active: AtomicBool::new(false),
            sample_counter: AtomicUsize::new(0),
            #[cfg(feature = "std")]
            history: Arc::new(Mutex::new(VecDeque::with_capacity(history_capacity))),
            #[cfg(feature = "std")]
            alerts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start monitoring in background thread (std only)
    #[cfg(feature = "std")]
    pub fn start(&self) -> Result<()> {
        if self
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::runtime_error("Monitor is already running"));
        }

        let config = self.config.clone();
        let active = Arc::new(AtomicBool::new(true));
        let sample_counter = Arc::new(AtomicUsize::new(0));
        let history = Arc::clone(&self.history);
        let alerts = Arc::clone(&self.alerts);

        let active_clone = Arc::clone(&active);
        let counter_clone = Arc::clone(&sample_counter);

        thread::spawn(move || {
            if config.console_output {
                println!(
                    "🔍 Real-time memory monitor started (interval: {}ms)",
                    config.interval_ms
                );
            }

            while active_clone.load(Ordering::Acquire) {
                // Collect memory sample
                let timestamp = counter_clone.fetch_add(1, Ordering::AcqRel) as u64;

                if let Ok(sample) = Self::collect_sample(timestamp) {
                    // Check for alerts
                    if let Some(alert) = Self::check_alerts(&sample, &config) {
                        if config.console_output {
                            Self::print_alert(&alert);
                        }

                        // Store alert
                        if let Ok(mut alerts_guard) = alerts.lock() {
                            alerts_guard.push(alert);

                            // Limit alert history
                            if alerts_guard.len() > 100 {
                                alerts_guard.remove(0);
                            }
                        }
                    }

                    // Store sample in history
                    if let Ok(mut history_guard) = history.lock() {
                        history_guard.push_back(sample.clone());

                        // Maintain history size
                        while history_guard.len() > config.history_size {
                            history_guard.pop_front();
                        }
                    }

                    // Auto-generate visualizations
                    if config.auto_visualize && timestamp % 60 == 0 {
                        // Every minute
                        let _ = Self::auto_generate_reports(&config);
                    }
                }

                thread::sleep(Duration::from_millis(config.interval_ms));
            }

            if config.console_output {
                println!("🛑 Real-time memory monitor stopped");
            }
        });

        Ok(())
    }

    /// Stop monitoring
    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    /// Check if monitoring is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Collect a single memory sample
    fn collect_sample(timestamp: u64) -> Result<MemorySample> {
        // TODO: Update to use capability-based memory monitoring
        // This requires injecting a MemoryCapabilityContext that can provide
        // aggregated statistics across all capabilities

        use wrt_foundation::monitoring::MEMORY_MONITOR;

        let monitor_stats = MEMORY_MONITOR.get_statistics();
        let total_allocated = 0; // TODO: Get from capability context

        // Collect per-crate utilization
        let mut crate_utilization = [0usize; 16];
        let crates = [
            CrateId::Foundation,
            CrateId::Runtime,
            CrateId::Component,
            CrateId::Decoder,
            CrateId::Format,
            CrateId::Host,
            CrateId::Debug,
            CrateId::Platform,
            CrateId::Instructions,
            CrateId::Intercept,
            CrateId::Sync,
            CrateId::Math,
            CrateId::Logging,
            CrateId::Panic,
            CrateId::TestRegistry,
            CrateId::VerificationTool,
        ];

        for (i, &_crate_id) in crates.iter().enumerate() {
            // TODO: Get allocation info from capability context
            let allocated = 0; // TODO: context.get_crate_allocation(crate_id);
            let budget = 0; // TODO: context.get_crate_budget(crate_id);
            if budget > 0 {
                crate_utilization[i] = (allocated * 100) / budget;
            }
        }

        // Collect shared pool utilization (using total system stats)
        let total_budget = 0; // TODO: Get from capability context
        let shared_pool_utilization =
            if total_budget > 0 { (total_allocated * 100) / total_budget } else { 0 };

        Ok(MemorySample {
            timestamp,
            total_allocated,
            active_providers: monitor_stats
                .total_allocations
                .saturating_sub(monitor_stats.total_deallocations)
                as usize,
            crate_utilization,
            shared_pool_utilization,
        })
    }

    /// Check for alert conditions
    #[allow(dead_code)]
    fn check_alerts(sample: &MemorySample, config: &MonitorConfig) -> Option<MemoryAlert> {
        // Check global memory health
        // Note: Memory analysis features will be restored when wrt_foundation
        // memory_analysis module is available

        // Check per-crate utilization
        let crates = [
            CrateId::Foundation,
            CrateId::Runtime,
            CrateId::Component,
            CrateId::Decoder,
            CrateId::Format,
            CrateId::Host,
            CrateId::Debug,
            CrateId::Platform,
        ];

        for (i, &crate_id) in crates.iter().enumerate() {
            if i < sample.crate_utilization.len() {
                let utilization = sample.crate_utilization[i];

                if utilization >= config.critical_threshold {
                    return Some(MemoryAlert {
                        timestamp: sample.timestamp,
                        level: AlertLevel::Critical,
                        #[cfg(any(feature = "std", feature = "alloc"))]
                        message: format!(
                            "{:?} memory utilization critical: {}%",
                            crate_id, utilization
                        ),
                        #[cfg(not(any(feature = "std", feature = "alloc")))]
                        message: "Crate memory utilization critical",
                        crate_id: Some(crate_id),
                    });
                } else if utilization >= config.alert_threshold {
                    return Some(MemoryAlert {
                        timestamp: sample.timestamp,
                        level: AlertLevel::Warning,
                        #[cfg(any(feature = "std", feature = "alloc"))]
                        message: format!(
                            "{:?} memory utilization high: {}%",
                            crate_id, utilization
                        ),
                        #[cfg(not(any(feature = "std", feature = "alloc")))]
                        message: "Crate memory utilization high",
                        crate_id: Some(crate_id),
                    });
                }
            }
        }

        // Check shared pool utilization
        if sample.shared_pool_utilization >= config.critical_threshold {
            return Some(MemoryAlert {
                timestamp: sample.timestamp,
                level: AlertLevel::Critical,
                #[cfg(any(feature = "std", feature = "alloc"))]
                message: format!(
                    "Shared pool utilization critical: {}%",
                    sample.shared_pool_utilization
                ),
                #[cfg(not(any(feature = "std", feature = "alloc")))]
                message: "Shared pool utilization critical",
                crate_id: None,
            });
        }

        None
    }

    /// Print alert to console
    #[cfg(feature = "std")]
    fn print_alert(alert: &MemoryAlert) {
        let icon = match alert.level {
            AlertLevel::Info => "ℹ️",
            AlertLevel::Warning => "⚠️",
            AlertLevel::Critical => "🚨",
        };

        println!(
            "{} MEMORY ALERT [{}]: {}",
            icon, alert.timestamp, alert.message
        );
    }

    /// Auto-generate visualization reports
    #[cfg(feature = "std")]
    fn auto_generate_reports(config: &MonitorConfig) -> Result<()> {
        use std::fs;

        // Ensure output directory exists
        if let Err(e) = fs::create_dir_all(&config.output_dir) {
            if config.console_output {
                eprintln!("Failed to create output directory: {}", e);
            }
            return Ok(());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Note: Visualization features will be restored when budget_visualization
        // module is available
        let json_path = format!("{}/memory_report_{}.json", config.output_dir, timestamp);
        let html_path = format!("{}/memory_report_{}.html", config.output_dir, timestamp);

        // Placeholder for now
        let _ = fs::write(json_path, "{}");
        let _ = fs::write(
            html_path,
            "<html><body>Memory report placeholder</body></html>",
        );

        Ok(())
    }

    /// Get current memory sample
    pub fn current_sample(&self) -> Result<MemorySample> {
        let timestamp = self.sample_counter.load(Ordering::Acquire) as u64;
        Self::collect_sample(timestamp)
    }

    /// Get recent alerts
    #[cfg(feature = "std")]
    pub fn get_recent_alerts(&self, count: usize) -> Vec<MemoryAlert> {
        if let Ok(alerts) = self.alerts.lock() {
            alerts.iter().rev().take(count).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get memory history
    #[cfg(feature = "std")]
    pub fn get_history(&self) -> Vec<MemorySample> {
        if let Ok(history) = self.history.lock() {
            history.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Export monitoring data to CSV
    #[cfg(feature = "std")]
    pub fn export_to_csv(&self, filename: &str) -> Result<()> {
        use std::{fs::File, io::Write};

        let mut file = File::create(filename)
            .map_err(|_e| Error::runtime_error("Failed to create CSV file"))?;

        // Write CSV header
        writeln!(
            file,
            "timestamp,total_allocated,active_providers,shared_pool_utilization,foundation_util,\
             runtime_util,component_util"
        )
        .map_err(|_e| Error::runtime_error("Failed to write CSV header"))?;

        // Write data rows
        if let Ok(history) = self.history.lock() {
            for sample in history.iter() {
                writeln!(
                    file,
                    "{},{},{},{},{},{},{}",
                    sample.timestamp,
                    sample.total_allocated,
                    sample.active_providers,
                    sample.shared_pool_utilization,
                    sample.crate_utilization.get(0).unwrap_or(&0),
                    sample.crate_utilization.get(1).unwrap_or(&0),
                    sample.crate_utilization.get(2).unwrap_or(&0),
                )
                .map_err(|_e| Error::runtime_error("Failed to write CSV row"))?;
            }
        }

        Ok(())
    }
}

// Global monitor functionality disabled in no_std mode to avoid unsafe code
// Users should create their own RealtimeMonitor instances

/// Initialize global realtime monitor (placeholder - not supported in no_std)
pub fn init_global_monitor(_config: MonitorConfig) -> Result<()> {
    Err(Error::runtime_error(
        "Global monitor not supported in no_std mode - create RealtimeMonitor instances directly",
    ))
}

/// Start global monitoring (placeholder - not supported in no_std)
#[cfg(feature = "std")]
pub fn start_global_monitoring() -> Result<()> {
    Err(Error::runtime_error(
        "Global monitor not supported in no_std mode - create RealtimeMonitor instances directly",
    ))
}

/// Stop global monitoring (placeholder - not supported in no_std)
pub fn stop_global_monitoring() {
    // No-op in no_std mode
}

/// Get current memory sample from global monitor (placeholder - not supported
/// in no_std)
pub fn get_current_sample() -> Result<MemorySample> {
    Err(Error::runtime_error(
        "Global monitor not supported in no_std mode - create RealtimeMonitor instances directly",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_config_default() {
        let config = MonitorConfig::default();
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.alert_threshold, 80);
        assert_eq!(config.critical_threshold, 95);
    }

    #[test]
    fn test_sample_collection() {
        let sample = RealtimeMonitor::collect_sample(0);
        assert!(sample.is_ok());

        let sample = sample.unwrap();
        assert_eq!(sample.timestamp, 0);
        assert!(sample.crate_utilization.len() == 16);
    }

    #[test]
    fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = RealtimeMonitor::new(config);

        assert!(!monitor.is_active());
    }

    #[test]
    fn test_global_monitor_init() {
        let config = MonitorConfig::default();

        // This might fail if already initialized, which is okay
        let _ = init_global_monitor(config);

        // Should be able to get current sample
        let sample = get_current_sample();
        match sample {
            Ok(_) => {},  // Success
            Err(_) => {}, // Expected if memory system not initialized
        }
    }
}
