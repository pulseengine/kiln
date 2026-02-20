//! Prelude module for wrt-intercept
//!
//! This module provides a unified set of imports for both std and `no_std`
//! environments. It re-exports commonly used types and traits to ensure
//! consistency across all crates in the WRT project and simplify imports in
//! individual modules.

// Binary std/no_std choice
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
pub use std::{
    boxed::Box,
    collections::{
        HashMap,
        HashSet,
    },
    format,
    string::{
        String,
        ToString,
    },
    sync::{
        Arc,
        Mutex,
        RwLock,
    },
    vec,
    vec::Vec,
};

// Re-export from wrt-error
pub use wrt_error::{
    codes,
    kinds,
    Error,
    ErrorCategory,
    Result,
};
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::ComponentValue;
// Re-export from wrt-foundation (for component model)
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::ValType;
// Binary std/no_std choice
// BoundedVec already imported above
#[cfg(not(feature = "std"))]
pub use wrt_foundation::BoundedMap as BoundedHashMap;
// Re-export from wrt-foundation
pub use wrt_foundation::{
    builtin::BuiltinType,
    resource::ResourceCanonicalOperation,
    // SafeMemory types
    safe_memory::{
        SafeMemoryHandler,
        SafeSlice,
        SafeStack,
    },
    // Core types
    values::Value,
};
// no_std alternatives using bounded collections
#[cfg(not(feature = "std"))]
pub use wrt_foundation::{
    BoundedCapacity,
    BoundedString,
    BoundedVec,
};
// Import synchronization primitives for no_std
#[cfg(not(feature = "std"))]
pub use wrt_sync::{
    Mutex,
    RwLock,
};

// Export BeforeBuiltinResult for both std and no_std
pub use crate::builtins::BeforeBuiltinResult;
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use crate::builtins::{
    BuiltinInterceptor,
    BuiltinSerialization,
};
// Conditional imports
#[cfg(feature = "std")]
pub use crate::strategies::StatisticsStrategy;
// Re-export from this crate
pub use crate::{
    // Builtin interceptors
    builtins::InterceptContext,
    // Strategies
    strategies::{
        FirewallConfig,
        FirewallRule,
        FirewallStrategy,
        LoggingStrategy,
    },
    InterceptionResult,

    // Core interception types
    LinkInterceptor,
    LinkInterceptorStrategy,
    Modification,
};
