// WRT - wrt-platform
// Module: Crate Prelude
// SW-REQ-ID: REQ_PLATFORM_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Public prelude for the `wrt-platform` crate.
//!
//! This module re-exports the core traits and types for convenient use.

// Re-export core error type (already re-exported in lib.rs, but good practice)
pub use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};

// Platform-specific re-exports based on features and targets
#[cfg(all(feature = "platform-macos", target_os = "macos"))]
pub use crate::macos_memory::{
    MacOsAllocator,
    MacOsAllocatorBuilder,
};
#[cfg(all(feature = "platform-macos", target_os = "macos"))]
pub use crate::macos_sync::{
    MacOsFutex,
    MacOsFutexBuilder,
};
// Binary std/no_std choice
// Re-export sync trait
pub use crate::{
    memory::{
        NoStdProvider,
        PageAllocator,
        VerificationLevel,
        WASM_PAGE_SIZE,
    },
    sync::FutexLike,
};
