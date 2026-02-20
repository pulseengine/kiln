//! Comprehensive logging and monitoring for WASI-NN
//!
//! This module provides structured logging, security event tracking, and
//! performance monitoring for neural network operations. It supports both
//! development debugging and production monitoring with ASIL-compliant logging
//! levels.

// Import tracing utilities for structured logging
#[cfg(feature = "tracing")]
use wrt_foundation::tracing::info;

use core::fmt;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{
            AtomicU64,
            Ordering,
        },
        Arc,
        Mutex,
    },
};

use super::{
    NNOperation,
    ResourceUsageStats,
    VerificationLevel,
};
use crate::prelude::*;

/// Log levels for WASI-NN operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Critical security events or system failures
    Critical = 0,
    /// Security warnings or significant errors
    Warning  = 1,
    /// Informational events (operations, resource usage)
    Info     = 2,
    /// Debug information for development
    Debug    = 3,
    /// Detailed trace information
    Trace    = 4,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Critical => write!(f, "CRITICAL"),
            LogLevel::Warning => write!(f, "WARNING"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Trace => write!(f, "TRACE"),
        }
    }
}

/// Types of events that can be logged
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    /// Security-related events
    Security(SecurityEvent),
    /// Performance monitoring events
    Performance(PerformanceEvent),
    /// Resource usage events
    Resource(ResourceEvent),
    /// Operation lifecycle events
    Operation(OperationEvent),
    /// Error events
    Error(ErrorEvent),
}

/// Security events for audit logging
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityEvent {
    /// Rate limit exceeded
    RateLimitExceeded {
        operation:  String,
        limit_type: String,
    },
    /// Resource quota exceeded
    QuotaExceeded {
        resource_type: String,
        current:       usize,
        limit:         usize,
    },
    /// Unauthorized operation attempted
    UnauthorizedOperation {
        operation: String,
        reason:    String,
    },
    /// Model validation failed
    ModelValidationFailed {
        reason:     String,
        model_size: usize,
    },
    /// Capability verification failed
    CapabilityVerificationFailed {
        operation:        String,
        capability_level: String,
    },
    /// Suspicious activity detected
    SuspiciousActivity { pattern: String, details: String },
}

/// Performance monitoring events
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceEvent {
    /// Operation timing
    OperationTiming {
        operation:   String,
        duration_us: u64,
        success:     bool,
    },
    /// Memory usage snapshot
    MemorySnapshot {
        total_used: usize,
        peak_usage: usize,
    },
    /// Throughput measurement
    Throughput {
        operations_per_second: f64,
        window_size_ms:        u64,
    },
    /// Latency measurement
    Latency {
        operation: String,
        p50_us:    u64,
        p95_us:    u64,
        p99_us:    u64,
    },
    /// Resource efficiency
    Efficiency {
        cpu_utilization:   f64,
        memory_efficiency: f64,
    },
}

/// Resource usage events
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceEvent {
    /// Resource allocated
    Allocated {
        resource_type: String,
        amount:        usize,
        total_used:    usize,
    },
    /// Resource deallocated
    Deallocated {
        resource_type: String,
        amount:        usize,
        total_used:    usize,
    },
    /// Resource usage warning
    UsageWarning {
        resource_type:     String,
        usage_percent:     u8,
        threshold_percent: u8,
    },
    /// Resource leak detected
    LeakDetected {
        resource_type: String,
        leaked_amount: usize,
    },
}

/// Operation lifecycle events
#[derive(Debug, Clone, PartialEq)]
pub enum OperationEvent {
    /// Operation started
    Started {
        operation:    String,
        operation_id: u64,
        context:      String,
    },
    /// Operation completed successfully
    Completed {
        operation:    String,
        operation_id: u64,
        duration_us:  u64,
    },
    /// Operation failed
    Failed {
        operation:    String,
        operation_id: u64,
        error:        String,
        duration_us:  u64,
    },
    /// Operation cancelled
    Cancelled {
        operation:    String,
        operation_id: u64,
        reason:       String,
    },
}

/// Error events for debugging and monitoring
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorEvent {
    /// Validation error
    ValidationError {
        field:    String,
        value:    String,
        expected: String,
    },
    /// Internal error
    InternalError {
        component:       String,
        error:           String,
        recovery_action: String,
    },
    /// Configuration error
    ConfigurationError { setting: String, issue: String },
    /// External dependency error
    DependencyError {
        dependency: String,
        error:      String,
        impact:     String,
    },
}

/// Structured log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp in microseconds since epoch
    pub timestamp:          u64,
    /// Log level
    pub level:              LogLevel,
    /// Event type and details
    pub event:              EventType,
    /// Component that generated the log
    pub component:          String,
    /// Verification level context
    pub verification_level: Option<VerificationLevel>,
    /// Additional context data
    pub context:            Vec<(String, String)>,
    /// Correlation ID for request tracing
    pub correlation_id:     Option<String>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(level: LogLevel, event: EventType, component: &str) -> Self {
        Self {
            timestamp: get_current_time_us(),
            level,
            event,
            component: component.to_string(),
            verification_level: None,
            context: Vec::new(),
            correlation_id: None,
        }
    }

    /// Add verification level context
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = Some(level);
        self
    }

    /// Add context key-value pair
    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.push((key.to_string(), value.to_string()));
        self
    }

    /// Add correlation ID for request tracing
    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }

    /// Format as JSON for structured logging
    pub fn to_json(&self) -> String {
        let mut json = format!(
            r#"{{"timestamp":{},"level":"{}","component":"{}""#,
            self.timestamp, self.level, self.component
        );

        if let Some(ref level) = self.verification_level {
            json.push_str(&format!(r#","verification_level":"{:?}""#, level));
        }

        if let Some(ref correlation_id) = self.correlation_id {
            json.push_str(&format!(r#","correlation_id":"{}""#, correlation_id));
        }

        // Add event details
        json.push_str(&format!(r#","event":{}"#, self.event_to_json()));

        // Add context if present
        if !self.context.is_empty() {
            json.push_str(r#","context":{"#);
            for (i, (key, value)) in self.context.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&format!(r#""{}":"{}""#, key, value));
            }
            json.push('}');
        }

        json.push('}');
        json
    }

    /// Format as human-readable string
    pub fn to_human_readable(&self) -> String {
        let timestamp = format_timestamp(self.timestamp);
        let event_desc = self.event_description();

        let mut result = format!(
            "[{}] {} [{}] {}",
            timestamp, self.level, self.component, event_desc
        );

        if let Some(ref correlation_id) = self.correlation_id {
            result.push_str(&format!(" [correlation_id={}]", correlation_id));
        }

        if !self.context.is_empty() {
            result.push_str(" [");
            for (i, (key, value)) in self.context.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("{}={}", key, value));
            }
            result.push(']');
        }

        result
    }

    fn event_to_json(&self) -> String {
        match &self.event {
            EventType::Security(event) => format!(
                r#"{{"type":"security","details":{}}}"#,
                security_event_to_json(event)
            ),
            EventType::Performance(event) => format!(
                r#"{{"type":"performance","details":{}}}"#,
                performance_event_to_json(event)
            ),
            EventType::Resource(event) => format!(
                r#"{{"type":"resource","details":{}}}"#,
                resource_event_to_json(event)
            ),
            EventType::Operation(event) => format!(
                r#"{{"type":"operation","details":{}}}"#,
                operation_event_to_json(event)
            ),
            EventType::Error(event) => format!(
                r#"{{"type":"error","details":{}}}"#,
                error_event_to_json(event)
            ),
        }
    }

    fn event_description(&self) -> String {
        match &self.event {
            EventType::Security(event) => {
                format!("Security: {}", security_event_description(event))
            },
            EventType::Performance(event) => {
                format!("Performance: {}", performance_event_description(event))
            },
            EventType::Resource(event) => {
                format!("Resource: {}", resource_event_description(event))
            },
            EventType::Operation(event) => {
                format!("Operation: {}", operation_event_description(event))
            },
            EventType::Error(event) => format!("Error: {}", error_event_description(event)),
        }
    }
}

/// Logger configuration
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Minimum log level to output
    pub min_level:               LogLevel,
    /// Maximum number of log entries to keep in memory
    pub max_entries:             usize,
    /// Whether to output JSON format
    pub json_format:             bool,
    /// Whether to include timestamps
    pub include_timestamps:      bool,
    /// Whether to include correlation IDs
    pub include_correlation_ids: bool,
    /// Buffer size for batched logging
    pub buffer_size:             usize,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level:               LogLevel::Info,
            max_entries:             10_000,
            json_format:             false,
            include_timestamps:      true,
            include_correlation_ids: true,
            buffer_size:             100,
        }
    }
}

/// WASI-NN Logger implementation
#[derive(Debug)]
pub struct Logger {
    config:            LoggerConfig,
    entries:           Mutex<VecDeque<LogEntry>>,
    operation_counter: AtomicU64,
    // Performance metrics
    total_operations:  AtomicU64,
    total_errors:      AtomicU64,
    start_time:        u64,
}

impl Logger {
    /// Create a new logger with default configuration
    pub fn new() -> Self {
        Self::with_config(LoggerConfig::default())
    }

    /// Create a new logger with custom configuration
    pub fn with_config(config: LoggerConfig) -> Self {
        Self {
            config,
            entries: Mutex::new(VecDeque::new()),
            operation_counter: AtomicU64::new(1),
            total_operations: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            start_time: get_current_time_us(),
        }
    }

    /// Log an entry
    pub fn log(&self, entry: LogEntry) {
        if entry.level <= self.config.min_level {
            if let Ok(mut entries) = self.entries.lock() {
                // Maintain maximum entry count
                while entries.len() >= self.config.max_entries {
                    entries.pop_front();
                }

                entries.push_back(entry.clone());

                // Update metrics
                self.total_operations.fetch_add(1, Ordering::Relaxed);
                if matches!(entry.event, EventType::Error(_)) {
                    self.total_errors.fetch_add(1, Ordering::Relaxed);
                }
            }

            // Output to console/file based on configuration
            self.output_entry(&entry);
        }
    }

    /// Generate a unique operation ID
    pub fn next_operation_id(&self) -> u64 {
        self.operation_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Log security event
    pub fn log_security(&self, event: SecurityEvent, component: &str) {
        let entry = LogEntry::new(LogLevel::Warning, EventType::Security(event), component);
        self.log(entry);
    }

    /// Log performance event
    pub fn log_performance(&self, event: PerformanceEvent, component: &str) {
        let entry = LogEntry::new(LogLevel::Info, EventType::Performance(event), component);
        self.log(entry);
    }

    /// Log resource event
    pub fn log_resource(&self, event: ResourceEvent, component: &str) {
        let level = match &event {
            ResourceEvent::UsageWarning { .. } | ResourceEvent::LeakDetected { .. } => {
                LogLevel::Warning
            },
            _ => LogLevel::Info,
        };
        let entry = LogEntry::new(level, EventType::Resource(event), component);
        self.log(entry);
    }

    /// Log operation event
    pub fn log_operation(&self, event: OperationEvent, component: &str) {
        let level = match &event {
            OperationEvent::Failed { .. } => LogLevel::Warning,
            _ => LogLevel::Info,
        };
        let entry = LogEntry::new(level, EventType::Operation(event), component);
        self.log(entry);
    }

    /// Log error event
    pub fn log_error(&self, event: ErrorEvent, component: &str) {
        let entry = LogEntry::new(LogLevel::Critical, EventType::Error(event), component);
        self.log(entry);
    }

    /// Get recent log entries
    pub fn get_recent_entries(&self, count: usize) -> Vec<LogEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries.iter().rev().take(count).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get logging statistics
    pub fn get_stats(&self) -> LoggingStats {
        let uptime = get_current_time_us() - self.start_time;
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let total_errors = self.total_errors.load(Ordering::Relaxed);

        LoggingStats {
            total_operations: total_ops,
            total_errors,
            error_rate: if total_ops > 0 {
                (total_errors as f64 / total_ops as f64) * 100.0
            } else {
                0.0
            },
            uptime_us: uptime,
            entries_in_buffer: self.entries.lock().map(|e| e.len()).unwrap_or(0),
        }
    }

    /// Clear all log entries
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    fn output_entry(&self, entry: &LogEntry) {
        let output = if self.config.json_format {
            entry.to_json()
        } else {
            entry.to_human_readable()
        };

        // Output to tracing framework
        // In production, this is directed to proper logging infrastructure
        #[cfg(feature = "tracing")]
        info!(entry = %output, "WASI-NN log entry");

        #[cfg(all(feature = "std", not(feature = "tracing")))]
        {
            // Fallback to stderr when tracing is not enabled
            std::eprintln!("{}", output);
        }

        #[cfg(not(any(feature = "std", feature = "tracing")))]
        {
            // For no_std environments without tracing, we might use a different output
            // mechanism or simply store in the buffer for later
            // retrieval
            let _ = output; // Suppress unused variable warning
        }
    }
}

/// Logging statistics
#[derive(Debug, Clone)]
pub struct LoggingStats {
    pub total_operations:  u64,
    pub total_errors:      u64,
    pub error_rate:        f64,
    pub uptime_us:         u64,
    pub entries_in_buffer: usize,
}

/// Global logger instance
static LOGGER: std::sync::OnceLock<Arc<Logger>> = std::sync::OnceLock::new();

/// Initialize the global logger
pub fn initialize_logger(config: LoggerConfig) -> Result<()> {
    let logger = Arc::new(Logger::with_config(config));
    LOGGER
        .set(logger)
        .map_err(|_| Error::wasi_runtime_error("Logger already initialized"))?;
    Ok(())
}

/// Get the global logger
pub fn get_logger() -> Option<Arc<Logger>> {
    LOGGER.get().cloned()
}

/// Convenience macro for logging
#[macro_export]
macro_rules! log_nn {
    (security, $event:expr, $component:expr) => {
        if let Some(logger) = $crate::nn::monitoring::get_logger() {
            logger.log_security($event, $component);
        }
    };
    (performance, $event:expr, $component:expr) => {
        if let Some(logger) = $crate::nn::monitoring::get_logger() {
            logger.log_performance($event, $component);
        }
    };
    (resource, $event:expr, $component:expr) => {
        if let Some(logger) = $crate::nn::monitoring::get_logger() {
            logger.log_resource($event, $component);
        }
    };
    (operation, $event:expr, $component:expr) => {
        if let Some(logger) = $crate::nn::monitoring::get_logger() {
            logger.log_operation($event, $component);
        }
    };
    (error, $event:expr, $component:expr) => {
        if let Some(logger) = $crate::nn::monitoring::get_logger() {
            logger.log_error($event, $component);
        }
    };
}

// Helper functions for JSON serialization
fn security_event_to_json(event: &SecurityEvent) -> String {
    match event {
        SecurityEvent::RateLimitExceeded {
            operation,
            limit_type,
        } => format!(
            r#"{{"type":"rate_limit_exceeded","operation":"{}","limit_type":"{}"}}"#,
            operation, limit_type
        ),
        SecurityEvent::QuotaExceeded {
            resource_type,
            current,
            limit,
        } => format!(
            r#"{{"type":"quota_exceeded","resource_type":"{}","current":{},"limit":{}}}"#,
            resource_type, current, limit
        ),
        SecurityEvent::UnauthorizedOperation { operation, reason } => format!(
            r#"{{"type":"unauthorized_operation","operation":"{}","reason":"{}"}}"#,
            operation, reason
        ),
        SecurityEvent::ModelValidationFailed { reason, model_size } => format!(
            r#"{{"type":"model_validation_failed","reason":"{}","model_size":{}}}"#,
            reason, model_size
        ),
        SecurityEvent::CapabilityVerificationFailed {
            operation,
            capability_level,
        } => format!(
            r#"{{"type":"capability_verification_failed","operation":"{}","capability_level":"{}"}}"#,
            operation, capability_level
        ),
        SecurityEvent::SuspiciousActivity { pattern, details } => format!(
            r#"{{"type":"suspicious_activity","pattern":"{}","details":"{}"}}"#,
            pattern, details
        ),
    }
}

fn performance_event_to_json(event: &PerformanceEvent) -> String {
    match event {
        PerformanceEvent::OperationTiming {
            operation,
            duration_us,
            success,
        } => format!(
            r#"{{"type":"operation_timing","operation":"{}","duration_us":{},"success":{}}}"#,
            operation, duration_us, success
        ),
        PerformanceEvent::MemorySnapshot {
            total_used,
            peak_usage,
        } => format!(
            r#"{{"type":"memory_snapshot","total_used":{},"peak_usage":{}}}"#,
            total_used, peak_usage
        ),
        PerformanceEvent::Throughput {
            operations_per_second,
            window_size_ms,
        } => format!(
            r#"{{"type":"throughput","operations_per_second":{},"window_size_ms":{}}}"#,
            operations_per_second, window_size_ms
        ),
        PerformanceEvent::Latency {
            operation,
            p50_us,
            p95_us,
            p99_us,
        } => format!(
            r#"{{"type":"latency","operation":"{}","p50_us":{},"p95_us":{},"p99_us":{}}}"#,
            operation, p50_us, p95_us, p99_us
        ),
        PerformanceEvent::Efficiency {
            cpu_utilization,
            memory_efficiency,
        } => format!(
            r#"{{"type":"efficiency","cpu_utilization":{},"memory_efficiency":{}}}"#,
            cpu_utilization, memory_efficiency
        ),
    }
}

fn resource_event_to_json(event: &ResourceEvent) -> String {
    match event {
        ResourceEvent::Allocated {
            resource_type,
            amount,
            total_used,
        } => format!(
            r#"{{"type":"allocated","resource_type":"{}","amount":{},"total_used":{}}}"#,
            resource_type, amount, total_used
        ),
        ResourceEvent::Deallocated {
            resource_type,
            amount,
            total_used,
        } => format!(
            r#"{{"type":"deallocated","resource_type":"{}","amount":{},"total_used":{}}}"#,
            resource_type, amount, total_used
        ),
        ResourceEvent::UsageWarning {
            resource_type,
            usage_percent,
            threshold_percent,
        } => format!(
            r#"{{"type":"usage_warning","resource_type":"{}","usage_percent":{},"threshold_percent":{}}}"#,
            resource_type, usage_percent, threshold_percent
        ),
        ResourceEvent::LeakDetected {
            resource_type,
            leaked_amount,
        } => format!(
            r#"{{"type":"leak_detected","resource_type":"{}","leaked_amount":{}}}"#,
            resource_type, leaked_amount
        ),
    }
}

fn operation_event_to_json(event: &OperationEvent) -> String {
    match event {
        OperationEvent::Started {
            operation,
            operation_id,
            context,
        } => format!(
            r#"{{"type":"started","operation":"{}","operation_id":{},"context":"{}"}}"#,
            operation, operation_id, context
        ),
        OperationEvent::Completed {
            operation,
            operation_id,
            duration_us,
        } => format!(
            r#"{{"type":"completed","operation":"{}","operation_id":{},"duration_us":{}}}"#,
            operation, operation_id, duration_us
        ),
        OperationEvent::Failed {
            operation,
            operation_id,
            error,
            duration_us,
        } => format!(
            r#"{{"type":"failed","operation":"{}","operation_id":{},"error":"{}","duration_us":{}}}"#,
            operation, operation_id, error, duration_us
        ),
        OperationEvent::Cancelled {
            operation,
            operation_id,
            reason,
        } => format!(
            r#"{{"type":"cancelled","operation":"{}","operation_id":{},"reason":"{}"}}"#,
            operation, operation_id, reason
        ),
    }
}

fn error_event_to_json(event: &ErrorEvent) -> String {
    match event {
        ErrorEvent::ValidationError {
            field,
            value,
            expected,
        } => format!(
            r#"{{"type":"validation_error","field":"{}","value":"{}","expected":"{}"}}"#,
            field, value, expected
        ),
        ErrorEvent::InternalError {
            component,
            error,
            recovery_action,
        } => format!(
            r#"{{"type":"internal_error","component":"{}","error":"{}","recovery_action":"{}"}}"#,
            component, error, recovery_action
        ),
        ErrorEvent::ConfigurationError { setting, issue } => format!(
            r#"{{"type":"configuration_error","setting":"{}","issue":"{}"}}"#,
            setting, issue
        ),
        ErrorEvent::DependencyError {
            dependency,
            error,
            impact,
        } => format!(
            r#"{{"type":"dependency_error","dependency":"{}","error":"{}","impact":"{}"}}"#,
            dependency, error, impact
        ),
    }
}

// Helper functions for human-readable descriptions
fn security_event_description(event: &SecurityEvent) -> String {
    match event {
        SecurityEvent::RateLimitExceeded {
            operation,
            limit_type,
        } => format!(
            "Rate limit exceeded for {} operation ({})",
            operation, limit_type
        ),
        SecurityEvent::QuotaExceeded {
            resource_type,
            current,
            limit,
        } => format!(
            "Quota exceeded for {} ({}/{})",
            resource_type, current, limit
        ),
        SecurityEvent::UnauthorizedOperation { operation, reason } => {
            format!("Unauthorized {} operation: {}", operation, reason)
        },
        SecurityEvent::ModelValidationFailed { reason, model_size } => format!(
            "Model validation failed: {} (size: {} bytes)",
            reason, model_size
        ),
        SecurityEvent::CapabilityVerificationFailed {
            operation,
            capability_level,
        } => format!(
            "Capability verification failed for {} at {} level",
            operation, capability_level
        ),
        SecurityEvent::SuspiciousActivity { pattern, details } => {
            format!("Suspicious activity detected: {} ({})", pattern, details)
        },
    }
}

fn performance_event_description(event: &PerformanceEvent) -> String {
    match event {
        PerformanceEvent::OperationTiming {
            operation,
            duration_us,
            success,
        } => format!(
            "{} operation took {}μs ({})",
            operation,
            duration_us,
            if *success { "success" } else { "failed" }
        ),
        PerformanceEvent::MemorySnapshot {
            total_used,
            peak_usage,
        } => format!("Memory usage: {} bytes (peak: {})", total_used, peak_usage),
        PerformanceEvent::Throughput {
            operations_per_second,
            window_size_ms,
        } => format!(
            "Throughput: {:.2} ops/sec ({}ms window)",
            operations_per_second, window_size_ms
        ),
        PerformanceEvent::Latency {
            operation,
            p50_us,
            p95_us,
            p99_us,
        } => format!(
            "{} latency: p50={}μs, p95={}μs, p99={}μs",
            operation, p50_us, p95_us, p99_us
        ),
        PerformanceEvent::Efficiency {
            cpu_utilization,
            memory_efficiency,
        } => format!(
            "Efficiency: CPU={:.1}%, Memory={:.1}%",
            cpu_utilization * 100.0,
            memory_efficiency * 100.0
        ),
    }
}

fn resource_event_description(event: &ResourceEvent) -> String {
    match event {
        ResourceEvent::Allocated {
            resource_type,
            amount,
            total_used,
        } => format!(
            "Allocated {} {} (total: {})",
            amount, resource_type, total_used
        ),
        ResourceEvent::Deallocated {
            resource_type,
            amount,
            total_used,
        } => format!(
            "Deallocated {} {} (total: {})",
            amount, resource_type, total_used
        ),
        ResourceEvent::UsageWarning {
            resource_type,
            usage_percent,
            threshold_percent,
        } => format!(
            "{} usage at {}% (threshold: {}%)",
            resource_type, usage_percent, threshold_percent
        ),
        ResourceEvent::LeakDetected {
            resource_type,
            leaked_amount,
        } => format!("Detected {} leak: {} units", resource_type, leaked_amount),
    }
}

fn operation_event_description(event: &OperationEvent) -> String {
    match event {
        OperationEvent::Started {
            operation,
            operation_id,
            context,
        } => format!(
            "Started {} operation #{} ({})",
            operation, operation_id, context
        ),
        OperationEvent::Completed {
            operation,
            operation_id,
            duration_us,
        } => format!(
            "Completed {} operation #{} in {}μs",
            operation, operation_id, duration_us
        ),
        OperationEvent::Failed {
            operation,
            operation_id,
            error,
            duration_us,
        } => format!(
            "Failed {} operation #{} after {}μs: {}",
            operation, operation_id, duration_us, error
        ),
        OperationEvent::Cancelled {
            operation,
            operation_id,
            reason,
        } => format!(
            "Cancelled {} operation #{}: {}",
            operation, operation_id, reason
        ),
    }
}

fn error_event_description(event: &ErrorEvent) -> String {
    match event {
        ErrorEvent::ValidationError {
            field,
            value,
            expected,
        } => format!(
            "Validation error in {}: got '{}', expected '{}'",
            field, value, expected
        ),
        ErrorEvent::InternalError {
            component,
            error,
            recovery_action,
        } => format!(
            "Internal error in {}: {} (recovery: {})",
            component, error, recovery_action
        ),
        ErrorEvent::ConfigurationError { setting, issue } => {
            format!("Configuration error in {}: {}", setting, issue)
        },
        ErrorEvent::DependencyError {
            dependency,
            error,
            impact,
        } => format!(
            "Dependency error in {}: {} (impact: {})",
            dependency, error, impact
        ),
    }
}

/// Get current time in microseconds
fn get_current_time_us() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
    }
    #[cfg(not(feature = "std"))]
    {
        wrt_platform::time::PlatformTime::get_monotonic_time_us()
    }
}

/// Format timestamp for human-readable output
fn format_timestamp(timestamp_us: u64) -> String {
    #[cfg(feature = "std")]
    {
        use std::time::{
            Duration,
            SystemTime,
            UNIX_EPOCH,
        };
        let datetime = SystemTime::UNIX_EPOCH + Duration::from_micros(timestamp_us);
        // For simplicity, just show microseconds since epoch
        // In production, you'd want proper datetime formatting
        format!("{}", timestamp_us)
    }
    #[cfg(not(feature = "std"))]
    {
        format!("{}", timestamp_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let event = SecurityEvent::RateLimitExceeded {
            operation:  "load".to_string(),
            limit_type: "per_minute".to_string(),
        };

        let entry = LogEntry::new(LogLevel::Warning, EventType::Security(event), "wasi-nn")
            .with_context("user_id", "test_user")
            .with_correlation_id("req_123");

        assert_eq!(entry.level, LogLevel::Warning);
        assert_eq!(entry.component, "wasi-nn");
        assert_eq!(entry.correlation_id, Some("req_123".to_string()));
        assert_eq!(entry.context.len(), 1);
    }

    #[test]
    fn test_json_formatting() {
        let event = PerformanceEvent::OperationTiming {
            operation:   "inference".to_string(),
            duration_us: 1500,
            success:     true,
        };

        let entry = LogEntry::new(LogLevel::Info, EventType::Performance(event), "wasi-nn");
        let json = entry.to_json();

        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"component\":\"wasi-nn\""));
        assert!(json.contains("\"type\":\"performance\""));
        assert!(json.contains("\"duration_us\":1500"));
    }

    #[test]
    fn test_logger_stats() {
        let logger = Logger::new();

        let error_event = ErrorEvent::ValidationError {
            field:    "model_size".to_string(),
            value:    "too_large".to_string(),
            expected: "< 100MB".to_string(),
        };

        let entry = LogEntry::new(
            LogLevel::Critical,
            EventType::Error(error_event),
            "validation",
        );
        logger.log(entry);

        let stats = logger.get_stats();
        assert_eq!(stats.total_operations, 1);
        assert_eq!(stats.total_errors, 1);
        assert_eq!(stats.error_rate, 100.0);
    }
}
