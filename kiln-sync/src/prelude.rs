//! Prelude module for wrt-sync
//!
//! This module provides a unified set of imports for both std and no_std environments.
//! It re-exports commonly used types and traits to ensure consistency across all crates
//! in the WRT project and simplify imports in individual modules.

// Core imports for both std and no_std environments
pub use core::{
    any::Any,
    cell::UnsafeCell,
    cmp::{Eq, Ord, PartialEq, PartialOrd},
    convert::{TryFrom, TryInto},
    fmt,
    fmt::Debug,
    fmt::Display,
    hint::spin_loop,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    slice, str,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

// Re-export from std when the std feature is enabled
#[cfg(feature = "std")]
pub use std::{
    boxed::Box,
    collections::{HashMap, HashSet},
    format, println,
    string::{String, ToString},
    sync::{Arc, Barrier},
    thread,
    time::Duration,
    vec,
    vec::Vec,
};

// Re-export from alloc when available (no_std + alloc)
#[cfg(all(not(feature = "std"), any(feature = "alloc", feature = "dynamic-allocation")))]
extern crate alloc;
#[cfg(all(not(feature = "std"), any(feature = "alloc", feature = "dynamic-allocation")))]
pub use alloc::{
    boxed::Box,
    collections::{BTreeMap as HashMap, BTreeSet as HashSet},
    format,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};

// Stub types for pure no_std environments
#[cfg(all(not(feature = "std"), not(feature = "alloc"), not(feature = "dynamic-allocation")))]
pub type Arc<T> = core::marker::PhantomData<T>;
#[cfg(all(not(feature = "std"), not(feature = "alloc"), not(feature = "dynamic-allocation")))]
pub type Box<T> = core::marker::PhantomData<T>;

// Re-export from wrt-error if enabled
#[cfg(feature = "error")]
pub use wrt_error::{codes, kinds, Error, ErrorCategory, Result};

// Re-export from this crate
pub use crate::mutex::{WrtMutex, WrtMutexGuard};
pub use crate::rwlock::{WrtRwLock, WrtRwLockReadGuard, WrtRwLockWriteGuard};

// Re-alias for convenience if not using std's versions
#[cfg(not(feature = "std"))]
pub use WrtMutex as Mutex;
#[cfg(not(feature = "std"))]
pub use WrtMutexGuard as MutexGuard;
#[cfg(not(feature = "std"))]
pub use WrtRwLock as RwLock;
#[cfg(not(feature = "std"))]
pub use WrtRwLockReadGuard as RwLockReadGuard;
#[cfg(not(feature = "std"))]
pub use WrtRwLockWriteGuard as RwLockWriteGuard;
