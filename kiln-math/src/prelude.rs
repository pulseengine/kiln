// WRT - wrt-math
// Module: Prelude
// SW-REQ-ID: N/A
//
// Copyright (c) 2024 Your Name/Organization
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Crate prelude for `wrt-math`

// Re-export commonly used items from this crate
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use alloc::{
    boxed::Box,
    format,
    string::{
        String,
        ToString,
    },
    vec,
    vec::Vec,
};
#[cfg(not(feature = "std"))]
pub use core::format_args;
// No specific core-only imports needed here for #[cfg(not(feature = "std"))]
// Project: WRT
// Module: wrt-math::prelude (SW-REQ-ID-TBD)
// Prelude module for wrt-math
//
// This module provides a unified set of imports for both std and no_std environments.
// Binary std/no_std choice
/// wrt-error, and this crate's own modules.
// Core imports for both std and no_std environments
pub use core::{
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
        Add,
        Div,
        Mul,
        Neg,
        Rem,
        Shl,
        Shr,
        Sub,
    }, /* Common math ops
        * Add any other core imports needed by this
        * specific crate */
};

// Re-export relevant error types or result aliases if any specific to math ops
// For now, users will use wrt_error::Result directly

// Re-export fundamental math operations if desired for a flat import structure
// Example (if ops module contains public functions like i32_add):
// pub use crate::ops::i32_add;
// pub use crate::ops::f32_add;
// ... (add other key operations)

// Consider re-exporting core/std items if commonly used within this crate's context
// and not already covered by a workspace-level prelude.
// #[cfg(feature = "std")]  // This empty import was causing a warning
// pub use std::{};

// No duplicate std imports needed - already defined above

// Re-export from wrt-error using its prelude
pub use wrt_error::prelude::*;

// It's often useful to have a `crate_alias` for macro usage or clarity
#[doc(hidden)]
pub use crate as wrt_math;
// pub use crate::float_bits::{FloatBits32, FloatBits64}; // This is duplicated below

// Re-export from this crate's modules
pub use crate::{
    float_bits::{
        FloatBits32,
        FloatBits64,
    },
    ops, // Re-export the whole ops module
    safety::{
        RoundingMode,
        SafeArithmetic,
        SafeFloat,
        SafeRounding,
    },
    traits::LittleEndian, /* Re-export the trait from its new location
                           * Add other re-exports specific to this crate's modules */
};
