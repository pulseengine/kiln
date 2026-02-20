// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Verification module for wrt-host using Kani.
//!
//! This module contains verification harnesses for the wrt-host crate.
//! It is only included when the `kani` feature is enabled.

use wrt_error::Result;

use super::*;

#[cfg(kani)]
#[kani::proof]
fn verify_cloneable_fn() {
    // Create a simple CloneableFn
    let f = CloneableFn::new(|_args| Ok(vec![]));

    // Clone the function
    let f2 = f.clone();

    // Verify that both functions work the same
    let target: Box<dyn std::any::Any> = Box::new(());
    let result1 = f.call(&mut *target, vec![]);
    let result2 = f2.call(&mut *target, vec![]);

    // Both should succeed
    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[cfg(kani)]
#[kani::proof]
fn verify_callback_registry() {
    // Create a new registry
    let mut registry = CallbackRegistry::new();

    // Register a host function
    registry.register_host_function(
        "test_module",
        "test_function",
        CloneableFn::new(|_args| Ok(vec![])),
    );

    // Verify that the function can be called
    assert!(registry.has_host_function("test_module", "test_function"));

    // Verify that a non-existent function is not found
    assert!(!registry.has_host_function("test_module", "nonexistent"));
}
