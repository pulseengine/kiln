//! Production telemetry and logging infrastructure
//!
//! Provides structured logging, metrics collection, and
//! diagnostic information for production deployments.
//!
//! # Safety Requirements
//! - SW-REQ-ID: ASIL-A-TEL-001
//! - Provides structured telemetry for production diagnostics
//! - Zero-overhead when disabled
//! - Lock-free operation for real-time systems

// Note: no_std is configured at the crate level

use core::sync::atomic::{
    AtomicBool,
    AtomicU64,
    AtomicU8,
    Ordering,
};

/// Telemetry event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Severity {
    /// Detailed tracing information
    Trace    = 0,
    /// Debug-level information
    Debug    = 1,
    /// Informational messages
    Info     = 2,
    /// Warning conditions
    Warning  = 3,
    /// Error conditions
    Error    = 4,
    /// Critical failures
    Critical = 5,
}

/// Telemetry event category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Memory allocation events
    Memory,
    /// Capability system events
    Capability,
    /// Error handling events
    Error,
    /// Performance metrics
    Performance,
    /// Safety violations
    Safety,
    /// System lifecycle
    Lifecycle,
}

/// Telemetry event
#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    /// Event timestamp (milliseconds since start)
    pub timestamp_ms: u64,
    /// Event severity
    pub severity:     Severity,
    /// Event category
    pub category:     Category,
    /// Event code for categorization
    pub event_code:   u32,
    /// Context value 1 (event-specific)
    pub context1:     u64,
    /// Context value 2 (event-specific)
    pub context2:     u64,
}

/// Event codes for different telemetry events
pub mod event_codes {
    /// Memory allocation successful
    pub const MEM_ALLOC_SUCCESS: u32 = 0x1000;
    /// Memory allocation failed
    pub const MEM_ALLOC_FAILURE: u32 = 0x1001;
    /// Memory deallocation
    pub const MEM_DEALLOC: u32 = 0x1002;
    /// Budget violation
    pub const MEM_BUDGET_VIOLATION: u32 = 0x1003;

    /// Capability created
    pub const CAP_CREATED: u32 = 0x2000;
    /// Capability violation
    pub const CAP_VIOLATION: u32 = 0x2001;
    /// Capability exhausted
    pub const CAP_EXHAUSTED: u32 = 0x2002;

    /// Error occurred
    pub const ERROR_OCCURRED: u32 = 0x3000;
    /// Error recovered
    pub const ERROR_RECOVERED: u32 = 0x3001;
    /// Fatal error
    pub const ERROR_FATAL: u32 = 0x3002;

    /// Slow operation detected
    pub const PERF_SLOW_OP: u32 = 0x4000;
    /// Memory pressure
    pub const PERF_MEM_PRESSURE: u32 = 0x4001;

    /// Safety violation detected
    pub const SAFETY_VIOLATION: u32 = 0x5000;
    /// Double free detected
    pub const SAFETY_DOUBLE_FREE: u32 = 0x5001;
    /// Safety health degraded
    pub const SAFETY_HEALTH_DEGRADED: u32 = 0x5002;
    /// Memory deallocation
    pub const MEMORY_DEALLOCATION: u32 = 0x1004;

    /// System initialized
    pub const LIFECYCLE_INIT: u32 = 0x6000;
    /// System shutdown
    pub const LIFECYCLE_SHUTDOWN: u32 = 0x6001;
}

/// Simple ring buffer for telemetry events
///
/// Lock-free implementation suitable for real-time systems
pub struct TelemetryBuffer<const N: usize> {
    /// Event storage
    events:      [AtomicU64; N],
    /// Write position
    write_pos:   AtomicU64,
    /// Number of events written
    event_count: AtomicU64,
}

impl<const N: usize> Default for TelemetryBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TelemetryBuffer<N> {
    /// Create a new telemetry buffer
    pub const fn new() -> Self {
        Self {
            events:      [const { AtomicU64::new(0) }; N],
            write_pos:   AtomicU64::new(0),
            event_count: AtomicU64::new(0),
        }
    }

    /// Record a telemetry event
    pub fn record(&self, event: &TelemetryEvent) {
        // Pack event into u64 for atomic storage
        // Format: [severity:8][category:8][code:16][context1:16][context2:16]
        let packed = ((event.severity as u64) << 56)
            | ((event.category as u64) << 48)
            | ((event.event_code as u64) << 32)
            | ((event.context1 & 0xFFFF) << 16)
            | (event.context2 & 0xFFFF);

        // Get write position
        let pos = self.write_pos.fetch_add(1, Ordering::Relaxed) % N as u64;

        // Store event atomically
        self.events[pos as usize].store(packed, Ordering::Relaxed);
        self.event_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get number of events recorded
    pub fn event_count(&self) -> u64 {
        self.event_count.load(Ordering::Relaxed)
    }
}

/// Global telemetry system configuration
pub struct TelemetryConfig {
    /// Whether telemetry is enabled
    enabled:           AtomicBool,
    /// Minimum severity level to record
    min_severity:      AtomicU8,
    /// Timestamp counter
    timestamp_counter: AtomicU64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryConfig {
    /// Create new telemetry configuration
    pub const fn new() -> Self {
        Self {
            enabled:           AtomicBool::new(true),
            min_severity:      AtomicU8::new(Severity::Info as u8),
            timestamp_counter: AtomicU64::new(0),
        }
    }

    /// Enable or disable telemetry
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set minimum severity level
    pub fn set_min_severity(&self, severity: Severity) {
        self.min_severity.store(severity as u8, Ordering::Relaxed);
    }

    /// Check if event should be recorded
    pub fn should_record(&self, severity: Severity) -> bool {
        self.is_enabled() && severity as u8 >= self.min_severity.load(Ordering::Relaxed)
    }

    /// Get current timestamp
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp_counter.fetch_add(1, Ordering::Relaxed)
    }
}

/// Global telemetry buffer (1024 events)
static TELEMETRY_BUFFER: TelemetryBuffer<1024> = TelemetryBuffer::new();

/// Global telemetry configuration
static TELEMETRY_CONFIG: TelemetryConfig = TelemetryConfig::new();

/// Record a telemetry event
pub fn record_event(
    severity: Severity,
    category: Category,
    event_code: u32,
    context1: u64,
    context2: u64,
) {
    if TELEMETRY_CONFIG.should_record(severity) {
        let event = TelemetryEvent {
            timestamp_ms: TELEMETRY_CONFIG.get_timestamp(),
            severity,
            category,
            event_code,
            context1,
            context2,
        };
        TELEMETRY_BUFFER.record(&event);
    }
}

/// Convenience macros for telemetry
#[macro_export]
macro_rules! telemetry_info {
    ($category:expr, $code:expr, $ctx1:expr, $ctx2:expr) => {
        $crate::telemetry::record_event(
            $crate::telemetry::Severity::Info,
            $category,
            $code,
            $ctx1 as u64,
            $ctx2 as u64,
        )
    };
}

#[macro_export]
macro_rules! telemetry_error {
    ($category:expr, $code:expr, $ctx1:expr, $ctx2:expr) => {
        $crate::telemetry::record_event(
            $crate::telemetry::Severity::Error,
            $category,
            $code,
            $ctx1 as u64,
            $ctx2 as u64,
        )
    };
}

#[macro_export]
macro_rules! telemetry_critical {
    ($category:expr, $code:expr, $ctx1:expr, $ctx2:expr) => {
        $crate::telemetry::record_event(
            $crate::telemetry::Severity::Critical,
            $category,
            $code,
            $ctx1 as u64,
            $ctx2 as u64,
        )
    };
}

/// Initialize telemetry system
pub fn init_telemetry(enabled: bool, min_severity: Severity) {
    TELEMETRY_CONFIG.set_enabled(enabled);
    TELEMETRY_CONFIG.set_min_severity(min_severity);

    // Record initialization event
    record_event(
        Severity::Info,
        Category::Lifecycle,
        event_codes::LIFECYCLE_INIT,
        enabled as u64,
        min_severity as u64,
    );
}

/// Get telemetry statistics
pub fn get_telemetry_stats() -> TelemetryStats {
    TelemetryStats {
        events_recorded:   TELEMETRY_BUFFER.event_count(),
        telemetry_enabled: TELEMETRY_CONFIG.is_enabled(),
        min_severity:      TELEMETRY_CONFIG.min_severity.load(Ordering::Relaxed),
    }
}

/// Telemetry statistics
#[derive(Debug, Clone)]
pub struct TelemetryStats {
    /// Total events recorded
    pub events_recorded:   u64,
    /// Whether telemetry is enabled
    pub telemetry_enabled: bool,
    /// Minimum severity level
    pub min_severity:      u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_recording() {
        init_telemetry(true, Severity::Debug);

        let initial_count = get_telemetry_stats().events_recorded;

        // Record some events
        record_event(
            Severity::Info,
            Category::Memory,
            event_codes::MEM_ALLOC_SUCCESS,
            1024,
            0,
        );

        record_event(
            Severity::Error,
            Category::Memory,
            event_codes::MEM_ALLOC_FAILURE,
            2048,
            12, // error code
        );

        let stats = get_telemetry_stats();
        assert!(stats.events_recorded > initial_count);
        assert!(stats.telemetry_enabled);
    }

    #[test]
    fn test_severity_filtering() {
        init_telemetry(true, Severity::Warning);

        let initial_count = get_telemetry_stats().events_recorded;

        // This should not be recorded (Debug < Warning)
        record_event(
            Severity::Debug,
            Category::Performance,
            event_codes::PERF_SLOW_OP,
            100,
            0,
        );

        // This should be recorded (Error > Warning)
        record_event(
            Severity::Error,
            Category::Safety,
            event_codes::SAFETY_VIOLATION,
            0,
            0,
        );

        let stats = get_telemetry_stats();
        assert_eq!(stats.events_recorded, initial_count + 1);
    }

    #[test]
    fn test_telemetry_disabled() {
        init_telemetry(false, Severity::Trace);

        let initial_count = get_telemetry_stats().events_recorded;

        // Nothing should be recorded when disabled
        record_event(
            Severity::Critical,
            Category::Error,
            event_codes::ERROR_FATAL,
            0,
            0,
        );

        let stats = get_telemetry_stats();
        assert_eq!(stats.events_recorded, initial_count);
        assert!(!stats.telemetry_enabled);
    }
}
