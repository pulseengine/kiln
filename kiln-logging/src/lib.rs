//! # WRT Logging
//!
//! Logging infrastructure for the WebAssembly Runtime (WRT).
//!
//! This crate provides logging functionality for WebAssembly components,
//! allowing components to log messages to the host environment. It extends
//! the wrt-host crate with logging-specific capabilities and works in both
//! standard and `no_std` environments.

// WRT - wrt-logging
// Module: Logging Infrastructure
// SW-REQ-ID: REQ_017
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)] // Rule 2
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Binary std/no_std choice
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Conditional imports based on feature flags
// Currently not needed at crate level - specific modules import as needed

// Reexports for convenience
pub use wrt_error::{
    Error,
    Result,
};
pub use wrt_host::CallbackRegistry;

/// Logging handlers for processing log messages.
///
/// This module contains the trait and implementations for log handlers,
/// which process log messages from WebAssembly components.
pub mod handler;

/// Bounded infrastructure for logging collections
pub mod bounded_log_infra;

/// Log level definitions for categorizing message severity.
///
/// This module defines the different log levels supported by the logging
/// infrastructure, from Debug (lowest severity) to Error (highest severity).
pub mod level;

/// Log operation data structures and utilities.
///
/// This module contains the types and functions for creating and working
/// with log operations, which encapsulate log messages and their metadata.
pub mod operation;

/// Minimal logging handler for pure no_std environments.
///
/// This module provides a minimal implementation of logging functionality
/// Binary std/no_std choice
pub mod minimal_handler;

/// Bounded logging infrastructure with configurable limits (Agent C).
///
/// This module provides enhanced logging functionality with bounded buffers
/// and platform-aware resource limits for deterministic operation.
pub mod bounded_logging;

// Reexport types
// Re-export Agent C deliverables
pub use bounded_logging::{
    BoundedLogBuffer,
    BoundedLogEntry,
    BoundedLogger,
    BoundedLoggingLimits,
    BoundedLoggingManager,
    BoundedLoggingStatistics,
    ComponentLoggingId,
    LogMetadata,
    LoggerId,
};
pub use handler::{
    LogHandler,
    LoggingExt,
};
pub use level::LogLevel;
// Reexport minimal_handler types for pure no_std environments
#[cfg(not(any(feature = "std",)))]
pub use minimal_handler::{
    MinimalLogHandler,
    MinimalLogMessage,
};
// Binary std/no_std choice
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std",))))]
pub use minimal_handler::{
    MinimalLogHandler,
    MinimalLogMessage,
};
pub use operation::LogOperation;

// Include verification module when the kani feature is enabled
#[cfg(feature = "kani")]
#[cfg_attr(docsrs, doc(cfg(feature = "kani")))]
pub mod verify;

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-logging is being tested in isolation
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
