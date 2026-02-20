// WRT - wrt-sync
// Module: Synchronization Primitives
// SW-REQ-ID: REQ_006
// SW-REQ-ID: REQ_CONCURRENCY_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::missing_panics_doc)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Binary std/no_std choice
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Binary std/no_std choice
#[cfg(feature = "std")]
extern crate std;

/// Synchronization primitives for WRT, supporting both std and `no_std`
/// environments.
///
/// This module provides essential synchronization tools like `WrtMutex` and
/// `WrtRwLock`, designed to work seamlessly in WebAssembly runtimes. They are
/// optimized for performance and safety, offering robust solutions for managing
/// concurrent access to shared resources.
///
/// ## Modules
///
/// - `mutex`: Provides `WrtMutex` and `WrtMutexGuard`.
/// - `rwlock`: Provides `WrtRwLock` and `WrtParkingRwLock`.
///
/// Each module is designed with an emphasis on ergonomics, performance, and
/// safety, offering robust solutions for managing concurrent access to shared
/// resources. The `WrtMutex` provides a mutual exclusion primitive.
/// It ensures that only one thread or task can access the protected data at any
/// given time. This implementation is designed for WebAssembly runtimes and
/// transparently adapts to both standard library and `no_std` environments
/// based on feature flags.
///
/// # Features
pub mod mutex;

/// OnceCell implementation for one-time initialization.
///
/// This module provides a synchronization primitive that allows for safe,
/// one-time initialization of a value.
///
/// # Examples
///
/// ```
/// use wrt_sync::{prelude::*, WrtOnce};
/// static SOME_STATIC: WrtOnce<Vec<i32>> = WrtOnce::new();
/// fn main() {
///     let data = SOME_STATIC.get_or_init(|| vec![1,2,3];
///     assert_eq!(data, &vec![1,2,3];
/// }
/// ```
pub mod once;

/// Prelude module for `wrt_sync`.
///
/// This module re-exports commonly used items for convenience.
pub mod prelude {
    // Binary std/no_std choice
    #[cfg(not(feature = "std"))]
    pub use core::{
        cell::UnsafeCell,
        fmt,
        ops::{
            Deref,
            DerefMut,
        },
        sync::atomic::{
            AtomicBool,
            AtomicUsize,
            Ordering,
        },
    };
    #[cfg(feature = "std")]
    pub use std::{
        boxed::Box,
        cell::UnsafeCell,
        fmt,
        ops::{
            Deref,
            DerefMut,
        },
        sync::{
            atomic::{
                AtomicBool,
                AtomicUsize,
                Ordering,
            },
            Arc,
            // Note: Mutex, RwLock from std::sync are not re-exported here
            // to avoid conflict with wrt_sync's own types.
        },
        thread::{
            self,
            park as park_thread,
            sleep,
            yield_now,
            JoinHandle,
        },
        time::Duration,
        vec::Vec,
    };
}

/// `ReadWrite` lock implementation for both std and `no_std` environments.
///
/// The `WrtRwLock` allows multiple readers or a single writer at any point in
/// time. This is useful for data structures that are read frequently but
/// modified infrequently. Like `WrtMutex`, this implementation is tailored for
/// WebAssembly and adjusts its behavior for `std` and `no_std` contexts.
///
/// # Features
pub mod rwlock;

/// Unified synchronization primitives that integrate with WRT foundation types.
///
/// This module provides enhanced synchronization primitives that work with:
/// - ASIL-aware safety contexts
/// - Bounded collections and memory providers
/// - Platform-configurable behavior
/// - Built-in verification for safety-critical applications
///
/// # Features
///
/// - `SafeMutex`: Mutex with integrated safety verification
/// - `BoundedChannel`: Bounded MPSC communication channel
/// - `SafeAtomicCounter`: Atomic counter with bounds checking
///
/// These primitives are designed for safety-critical applications where
/// predictable behavior and verification are required.
pub mod unified_sync;

// Include verification module conditionally, but exclude during coverage builds
#[cfg(all(not(coverage), doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "kani")))]
pub mod verify;

// Re-export types for convenience
pub use mutex::{
    WrtMutex,
    WrtMutexGuard,
};
pub use once::WrtOnce;
// Publicly exported synchronization primitives when the `std` feature is
// enabled. These are the parking versions.
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use rwlock::parking_impl::{
    WrtParkingRwLock,
    WrtParkingRwLockReadGuard,
    WrtParkingRwLockWriteGuard,
};
// Re-export the basic (spin-lock) RwLock and its guards.
// These are always available as they don't depend on std for parking.
pub use rwlock::{
    WrtRwLock,
    WrtRwLockReadGuard,
    WrtRwLockWriteGuard,
};
// Re-export unified synchronization primitives
pub use unified_sync::{
    AsilLevel,
    BoundedChannel,
    BoundedReceiver,
    BoundedSender,
    SafeAtomicCounter,
    SafetyContext,
    SafeMutex,
    SafeMutexGuard,
};

// Convenience aliases for easier importing
/// Type alias for WrtMutex to provide a familiar interface
pub type Mutex<T> = WrtMutex<T>;
/// Type alias for WrtRwLock to provide a familiar interface
pub type RwLock<T> = WrtRwLock<T>;

// Panic handler disabled to avoid conflicts with other crates
// The main wrt crate should provide the panic handler
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
