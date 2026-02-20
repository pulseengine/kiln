// WRT - wrt-error
// Module: WRT Error Prelude
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Prelude module for wrt-error
//!
//! This module provides a unified set of imports for both std and `no_std`
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Core imports for both std and no_std environments
// Binary std/no_std choice
// Binary std/no_std choice
// pub use std::{
//     boxed::Box,
//     collections::{BTreeMap as HashMap, BTreeSet as HashSet},
//     format,
//     string::{String, ToString},
//     vec,
//     vec::Vec,
// };
pub use core::{
    any::Any,
    cmp::{Eq, Ord, PartialEq, PartialOrd},
    convert::{TryFrom, TryInto},
    fmt,
    fmt::{Debug, Display},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    slice, str,
};

// Re-export from std when the std feature is enabled
// #[cfg(feature = "std")]
// pub use std::{
//     boxed::Box,
//     collections::{HashMap, HashSet},
//     format,
//     string::{String, ToString},
//     vec,
//     vec::Vec,
// };

// Re-export helper functions for creating errors
pub use crate::helpers::*;
// Re-export error types from this crate
pub use crate::{
    Error, ErrorCategory, ErrorSource, FromError, Result, ToErrorCategory, codes,
    kinds::{
        self, ComponentError, InvalidType, OutOfBoundsError, ParseError, PoisonedLockError,
        ResourceError, RuntimeError, ValidationError,
    },
};
// Re-export error factory functions
pub use crate::{
    component_error, invalid_type, out_of_bounds_error, parse_error, poisoned_lock_error,
    resource_error, runtime_error, validation_error,
};
