//! Allocation tracing for understanding memory patterns.
//!
//! This module provides feature-gated tracing infrastructure to observe
//! allocation patterns during WebAssembly module processing. It's designed
//! for development/profiling use, not production.
//!
//! # Motivation
//!
//! Before constraining allocations with bounded collections, we need to
//! understand WHERE memory is allocated, WHEN (which phase), and HOW MUCH.
//! This tracing helps build an allocation inventory.
//!
//! # Usage
//!
//! Enable with `--features allocation-tracing`:
//!
//! ```rust,ignore
//! use wrt_foundation::allocation_tracing::{trace_alloc, Phase};
//!
//! // In your allocation site:
//! trace_alloc!(Phase::Decode, "streaming_decoder:405", "func_params", param_count);
//! let mut params = Vec::with_capacity(param_count);
//! ```
//!
//! # Output
//!
//! When enabled, traces are written to stderr:
//! ```text
//! [ALLOC] phase=Decode loc=streaming_decoder:405 what=func_params size=32
//! ```

/// Allocation phase during WebAssembly processing.
///
/// WebAssembly module processing is inherently phase-oriented:
/// - Decode: Reading and parsing the binary format
/// - Link: Resolving imports, creating instances
/// - Execute: Running WebAssembly code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Decoding/parsing phase - reading module sections
    Decode,
    /// Linking phase - resolving imports, creating instances
    Link,
    /// Execution phase - running WebAssembly code
    Execute,
    /// Validation phase - checking module validity
    Validate,
    /// Unknown/other phase
    Other,
}

impl core::fmt::Display for Phase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Phase::Decode => write!(f, "Decode"),
            Phase::Link => write!(f, "Link"),
            Phase::Execute => write!(f, "Execute"),
            Phase::Validate => write!(f, "Validate"),
            Phase::Other => write!(f, "Other"),
        }
    }
}

/// Trace an allocation event (only when feature is enabled).
///
/// This macro logs allocation information to help understand memory patterns
/// before implementing bounded allocation constraints.
///
/// # Arguments
///
/// * `$phase` - The processing phase (Decode, Link, Execute, etc.)
/// * `$location` - Source location string (e.g., "streaming_decoder:405")
/// * `$what` - Description of what's being allocated
/// * `$size` - Size/count of the allocation
#[cfg(feature = "allocation-tracing")]
#[macro_export]
macro_rules! trace_alloc {
    ($phase:expr, $location:expr, $what:expr, $size:expr) => {{
        #[cfg(feature = "std")]
        {
            eprintln!(
                "[ALLOC] phase={} loc={} what={} size={}",
                $phase, $location, $what, $size
            );
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std, we'd need a different output mechanism
            // For now, this is a no-op but the call sites are marked
            let _ = ($phase, $location, $what, $size);
        }
    }};
}

/// No-op version when allocation tracing is disabled.
#[cfg(not(feature = "allocation-tracing"))]
#[macro_export]
macro_rules! trace_alloc {
    ($phase:expr, $location:expr, $what:expr, $size:expr) => {
        // Completely removed in release builds
    };
}

/// Allocation record for programmatic analysis.
///
/// This struct captures allocation metadata for later analysis.
/// Use `AllocationLog` to collect these.
#[derive(Debug, Clone)]
pub struct AllocationRecord {
    /// Processing phase when allocation occurred
    pub phase: Phase,
    /// Source location (file:line)
    pub location: &'static str,
    /// Description of what was allocated
    pub what: &'static str,
    /// Size or count of the allocation
    pub size: usize,
}

/// Thread-local allocation log for collecting records.
///
/// This is only available with std feature and allocation-tracing.
#[cfg(all(feature = "std", feature = "allocation-tracing"))]
pub mod log {
    use super::{AllocationRecord, Phase};
    use std::cell::RefCell;

    thread_local! {
        static ALLOCATION_LOG: RefCell<Vec<AllocationRecord>> = RefCell::new(Vec::new());
    }

    /// Record an allocation for later analysis.
    pub fn record(phase: Phase, location: &'static str, what: &'static str, size: usize) {
        ALLOCATION_LOG.with(|log| {
            log.borrow_mut().push(AllocationRecord {
                phase,
                location,
                what,
                size,
            });
        });
    }

    /// Get all recorded allocations.
    pub fn get_records() -> Vec<AllocationRecord> {
        ALLOCATION_LOG.with(|log| log.borrow().clone())
    }

    /// Clear all recorded allocations.
    pub fn clear() {
        ALLOCATION_LOG.with(|log| log.borrow_mut().clear());
    }

    /// Print a summary of allocations by phase.
    pub fn print_summary() {
        let records = get_records();

        let mut decode_count = 0usize;
        let mut decode_size = 0usize;
        let mut link_count = 0usize;
        let mut link_size = 0usize;
        let mut execute_count = 0usize;
        let mut execute_size = 0usize;
        let mut other_count = 0usize;
        let mut other_size = 0usize;

        for record in &records {
            match record.phase {
                Phase::Decode => {
                    decode_count += 1;
                    decode_size += record.size;
                }
                Phase::Link => {
                    link_count += 1;
                    link_size += record.size;
                }
                Phase::Execute => {
                    execute_count += 1;
                    execute_size += record.size;
                }
                _ => {
                    other_count += 1;
                    other_size += record.size;
                }
            }
        }

        eprintln!("=== Allocation Summary ===");
        eprintln!(
            "Decode:  {} allocations, total size {}",
            decode_count, decode_size
        );
        eprintln!(
            "Link:    {} allocations, total size {}",
            link_count, link_size
        );
        eprintln!(
            "Execute: {} allocations, total size {}",
            execute_count, execute_size
        );
        eprintln!(
            "Other:   {} allocations, total size {}",
            other_count, other_size
        );
        eprintln!("==========================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_phase_display() {
        assert_eq!(format!("{}", Phase::Decode), "Decode");
        assert_eq!(format!("{}", Phase::Link), "Link");
        assert_eq!(format!("{}", Phase::Execute), "Execute");
    }

    #[test]
    fn test_phase_equality() {
        // Test Phase comparison without needing format!
        assert_eq!(Phase::Decode, Phase::Decode);
        assert_eq!(Phase::Link, Phase::Link);
        assert_eq!(Phase::Execute, Phase::Execute);
        assert_ne!(Phase::Decode, Phase::Link);
    }

    #[test]
    fn test_trace_alloc_macro_compiles() {
        // This just verifies the macro compiles in both enabled/disabled states
        trace_alloc!(Phase::Decode, "test:1", "test_alloc", 100);
    }

    #[cfg(all(feature = "std", feature = "allocation-tracing"))]
    #[test]
    fn test_allocation_log() {
        log::clear();
        log::record(Phase::Decode, "test:1", "vec", 10);
        log::record(Phase::Link, "test:2", "map", 20);

        let records = log::get_records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].phase, Phase::Decode);
        assert_eq!(records[1].phase, Phase::Link);

        log::clear();
        assert!(log::get_records().is_empty());
    }
}
