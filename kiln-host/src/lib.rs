// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)] // Rule 2

//! Host function infrastructure for the WebAssembly Runtime (WRT).
//!
//! This crate provides the core infrastructure for registering and managing
//! host functions that can be called from WebAssembly components. It follows
//! the Component Model specification for host functions and the Canonical ABI.
//!
//! ## Features
//!
//! - Host function registration and callback management
//! - Built-in function handling for the WebAssembly Component Model
//! - Interception and introspection of function calls
//! - No-std compatible with the `no_std` and `alloc` features
//!
//! ## Usage
//!
//! ```rust,no_run
//! # use wrt_host::prelude::*;
//! # use wrt_foundation::Value;
//!
//! // Create a host builder
//! let builder = HostBuilder::new()
//!     .with_host_function("my_module", "my_function",
//!         HostFunctionHandler::new(|_| Ok(vec![Value::I32(42)])))
//!     .with_component_name("my_component")
//!     .with_host_id("my_host";
//!
//! // Build the host
//! let registry = builder.build().expect(".expect("Failed to build host"));")
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::missing_panics_doc)]

// Import std when available
#[cfg(feature = "std")]
extern crate std;

// Binary std/no_std choice
#[cfg(feature = "std")]
#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

// Binary std/no_std choice
// from wrt-foundation

// Bounded infrastructure for static memory allocation
#[cfg(not(feature = "std"))]
pub mod bounded_host_infra;

// Safety-critical memory limits
#[cfg(feature = "safety-critical")]
pub mod memory_limits;

// Export modules
pub mod builder;
pub mod callback;
pub mod function;
pub mod host;
pub mod prelude;

// Agent C deliverables - Enhanced Host Integration
/// Bounded host integration with memory constraints
pub mod bounded_host_integration;

// Include verification module conditionally, but exclude during coverage builds
#[cfg(all(not(coverage), any(doc, kani)))]
pub mod verify;

// Reexport types for convenience (use fully qualified paths)
// Re-export Agent C deliverables
pub use bounded_host_integration::{
    create_echo_function,
    create_memory_info_function,
    create_safety_check_function,
    BoundedCallContext,
    BoundedCallResult,
    BoundedHostFunction,
    BoundedHostIntegrationManager,
    ComponentInstanceId,
    HostFunctionId,
    HostIntegrationLimits,
    HostIntegrationStatistics,
    SimpleBoundedHostFunction,
};
pub use builder::HostBuilder;
pub use callback::{
    CallbackRegistry,
    CallbackType,
};
pub use function::{
    CloneableFn,
    HostFunctionHandler,
};
pub use host::BuiltinHost;
// Re-export prelude for convenience
pub use prelude::*;

// Panic handler disabled in library crates to avoid conflicts during workspace
// builds The main wrt crate or final binary should provide the panic handler
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     // For safety-critical systems, enter infinite loop to maintain known
// safe state     loop {
//         core::hint::spin_loop);
//     }
// }
