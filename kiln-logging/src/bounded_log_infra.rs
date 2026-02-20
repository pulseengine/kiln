//! Bounded infrastructure for logging system
//!
//! This module provides bounded alternatives for logging collections
//! ensuring static memory allocation.

#[cfg(not(feature = "std"))]
extern crate alloc;

use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

/// Budget-aware memory provider for logging (32KB)
pub type LogProvider = NoStdProvider<8192>;

/// Maximum number of log entries in buffer
pub const MAX_LOG_ENTRIES: usize = 1024;

/// Maximum number of loggers
pub const MAX_LOGGERS: usize = 32;

/// Maximum log message length
pub const MAX_LOG_MESSAGE_LEN: usize = 512;

/// Maximum module name length
pub const MAX_MODULE_NAME_LEN: usize = 128;

/// Bounded vector for log entries
pub type BoundedLogEntryVec = BoundedVec<crate::BoundedLogEntry, MAX_LOG_ENTRIES, LogProvider>;

/// Bounded vector for loggers
pub type BoundedLoggerVec<T> = BoundedVec<T, MAX_LOGGERS, LogProvider>;

/// Bounded string for log messages
pub type BoundedLogMessage = BoundedString<MAX_LOG_MESSAGE_LEN>;

/// Bounded string for module names
pub type BoundedModuleName = BoundedString<MAX_MODULE_NAME_LEN>;

/// Create a new bounded log entry vector
pub fn new_log_entry_vec() -> wrt_error::Result<BoundedLogEntryVec> {
    let provider = safe_managed_alloc!(8192, CrateId::Logging)?;
    BoundedVec::new(provider)
}

/// Create a new bounded logger vector
pub fn new_logger_vec<T>() -> wrt_error::Result<BoundedLoggerVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq
        + Sized,
{
    let provider = safe_managed_alloc!(8192, CrateId::Logging)?;
    BoundedVec::new(provider)
}
