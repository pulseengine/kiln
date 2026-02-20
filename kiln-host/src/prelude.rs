// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Prelude module for wrt-host
//!
//! This module provides a unified set of imports for both std and `no_std`
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Binary std/no_std choice - conditional imports only

// Binary std/no_std choice
// Additional imports for pure no_std
#[cfg(not(feature = "std"))]
pub use core::fmt::Write as FmtWrite;

#[cfg(not(feature = "std"))]
pub use wrt_foundation::{
    bounded::{
        BoundedString as String,
        BoundedVec as Vec,
    },
    BoundedMap as HashMap,
    BoundedSet as HashSet,
};

// Arc is not available in pure no_std, use a reference wrapper
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone)]
/// Arc-like wrapper for `no_std` environments
pub struct Arc<T> {
    inner: T,
}

#[cfg(not(feature = "std"))]
impl<T> Arc<T> {
    /// Create a new Arc-like wrapper
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }
}

#[cfg(not(feature = "std"))]
impl<T> core::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// In pure no_std mode, we need a minimal Box implementation for trait objects
/// Simple Box implementation for `no_std` environments
///
/// This provides API compatibility with `std::boxed::Box` in `no_std`
/// environments. Unlike the standard Box, this does not allocate on the heap
/// but provides the same interface for trait object storage.
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct Box<T> {
    inner: T,
}

#[cfg(not(feature = "std"))]
impl<T> Box<T> {
    /// Create a new Box containing the given value
    ///
    /// This is a simplified Box implementation for `no_std` environments.
    /// In `no_std` mode, this doesn't actually allocate on the heap but
    /// provides API compatibility with `std::boxed::Box`.
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }
}

#[cfg(not(feature = "std"))]
impl<T> core::ops::Deref for Box<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(not(feature = "std"))]
impl<T> core::ops::DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// Drop and Debug are automatically derived for our simple Box implementation
pub use core::{
    any::Any,
    cmp::{
        Eq,
        Ord,
        PartialEq,
        PartialOrd,
    },
    convert::{
        TryFrom,
        TryInto,
    },
    fmt,
    fmt::{
        Debug,
        Display,
    },
    marker::PhantomData,
    mem,
    ops::{
        Deref,
        DerefMut,
    },
    slice,
    str,
};
// Re-export from std when the std feature is enabled
#[cfg(feature = "std")]
pub use alloc::{
    boxed::Box,
    string::{
        String,
        ToString,
    },
    sync::Arc,
    vec,
    vec::Vec,
};

#[cfg(feature = "std")]
pub use core::fmt::Write as FmtWrite;

#[cfg(feature = "std")]
pub use std::{
    collections::{
        HashMap,
        HashSet,
    },
    format,
    sync::{
        Mutex,
        RwLock,
    },
};

// Re-export from wrt-error
pub use wrt_error::{
    codes,
    kinds,
    Error,
    ErrorCategory,
    Result,
};
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::ComponentValue;
// Re-export from wrt-foundation
pub use wrt_foundation::{
    // Builtin types
    builtin::BuiltinType,
    // Memory allocation
    safe_managed_alloc,
    // SafeMemory types
    safe_memory::{
        SafeMemoryHandler,
        SafeSlice,
        SafeStack,
    },
    // Common types
    types::{
        BlockType,
        FuncType,
        GlobalType,
        MemoryType,
        TableType,
        ValueType,
    },
    values::Value,
    // Verification types
    verification::VerificationLevel,
    // CrateId for budget allocation
    CrateId,
};
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use wrt_intercept::{
    BeforeBuiltinResult,
    BuiltinInterceptor,
    InterceptContext,
    LinkInterceptor,
    LinkInterceptorStrategy,
};
// Import synchronization primitives for no_std
#[cfg(not(feature = "std"))]
pub use wrt_sync::{
    Mutex,
    RwLock,
};

/// Memory size for host function allocations in no_std mode
///
/// This constant defines the memory budget for host functions when running
/// in no_std environments. It provides 64KB of memory for host function
/// operations including callback storage and temporary data structures.
pub const HOST_MEMORY_SIZE: usize = 65536; // 64KB for host functions

// Re-export from this crate
pub use crate::{
    builder::HostBuilder,
    callback::{
        CallbackRegistry,
        CallbackType,
    },
    function::{
        CloneableFn,
        HostFunctionHandler,
    },
    host::BuiltinHost,
};
