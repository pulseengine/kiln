//! WASI Preview2 interface implementations
//!
//! This module contains implementations of all WASI Preview2 interfaces,
//! built on WRT's proven patterns for maximum code reuse and reliability.

#[cfg(feature = "wasi-filesystem")]
pub mod filesystem;

#[cfg(feature = "wasi-cli")]
pub mod cli;

#[cfg(feature = "wasi-clocks")]
pub mod clocks;

#[cfg(feature = "wasi-io")]
pub mod io;

#[cfg(feature = "wasi-random")]
pub mod random;

#[cfg(feature = "wasi-sockets")]
pub mod sockets;

// Re-export main functions for convenience
// Note: Filesystem operations are implemented directly in component_model_provider.rs
// #[cfg(feature = "wasi-filesystem")]
// pub use filesystem::{...};

#[cfg(feature = "wasi-cli")]
pub use cli::{wasi_cli_get_arguments, wasi_cli_get_environment};

#[cfg(feature = "wasi-clocks")]
pub use clocks::{wasi_monotonic_clock_now, wasi_wall_clock_now};

#[cfg(feature = "wasi-io")]
pub use io::{wasi_stream_read, wasi_stream_write};

#[cfg(feature = "wasi-random")]
pub use random::{wasi_get_random_bytes, wasi_get_insecure_random_bytes};

#[cfg(feature = "wasi-sockets")]
pub use sockets::WasiSocketCapabilities;