//! Built-in strategies for intercepting component function calls
//!
//! This module provides implementations of common interceptor strategies
//! that can be used out of the box or as examples for creating custom
//! strategies.

mod firewall;
mod logging;
mod stats;

pub use firewall::{
    FirewallConfig,
    FirewallRule,
    FirewallStrategy,
};
pub use logging::LoggingStrategy;
#[cfg(not(feature = "std"))]
pub use stats::FunctionStats;
#[cfg(feature = "std")]
pub use stats::{
    FunctionStats,
    StatisticsStrategy,
};
