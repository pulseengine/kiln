// WRT - wrt-foundation
// Module: Validation Utilities
// SW-REQ-ID: REQ_VERIFY_002
// SW-REQ-ID: REQ_VERIFY_003
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Validation infrastructure for bounded collections
//!
//! This module provides traits and utilities for implementing validation
//! in bounded collections and other safety-critical data structures.

// Conditionally import from std or core

#[cfg(feature = "std")]
extern crate alloc;

// Added BoundedVec for tests
use wrt_error::{
    ErrorCategory,
    Result,
};

