//! Prelude module for kiln-sync
//!
//! This module provides a unified set of imports for both std and no_std environments.
//! It re-exports commonly used types and traits to ensure consistency across all crates
//! in the Kiln project and simplify imports in individual modules.

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

// Re-export from kiln-error if enabled
#[cfg(feature = "error")]
pub use kiln_error::{codes, kinds, Error, ErrorCategory, Result};

// Re-export from this crate
pub use crate::mutex::{KilnMutex, KilnMutexGuard};
pub use crate::rwlock::{KilnRwLock, KilnRwLockReadGuard, KilnRwLockWriteGuard};

// Re-alias for convenience if not using std's versions
#[cfg(not(feature = "std"))]
pub use KilnMutex as Mutex;
#[cfg(not(feature = "std"))]
pub use KilnMutexGuard as MutexGuard;
#[cfg(not(feature = "std"))]
pub use KilnRwLock as RwLock;
#[cfg(not(feature = "std"))]
pub use KilnRwLockReadGuard as RwLockReadGuard;
#[cfg(not(feature = "std"))]
pub use KilnRwLockWriteGuard as RwLockWriteGuard;
