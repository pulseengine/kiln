// Kiln - kiln-async :: config
// SW-REQ-ID: REQ_ASYNC_SCHED
// SPDX-License-Identifier: MIT

//! Compile-time and runtime scheduler configuration.

/// Default fixed capacities for a small embedded target (Cortex-M class).
///
/// These are the `const` generic arguments callers pass to
/// [`crate::Scheduler`]; collected here as a documented default profile.
pub mod defaults {
    /// Maximum concurrent tasks.
    pub const NTASK: usize = 16;
    /// Ready-queue capacity (size `== NTASK`; never overflows by invariant).
    pub const NREADY: usize = 16;
    /// Maximum tracked waitables (futures + streams + sets).
    pub const NWAIT: usize = 16;
}

/// Runtime-tunable scheduler configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedConfig {
    /// Fuel charged per task poll slice — the deterministic time quantum.
    pub fuel_slice: u64,
    /// Number of priority levels (`1` = FIFO; `> 1` = fixed-priority bitmap).
    pub priorities: u8,
}

impl SchedConfig {
    /// A sensible default: 10k fuel per slice, single-level FIFO.
    pub const DEFAULT: Self = Self {
        fuel_slice: 10_000,
        priorities: 1,
    };
}

impl Default for SchedConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}
