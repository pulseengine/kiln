//! Component import linker
//!
//! Resolves component imports to providers and creates runtime instances.
//!
//! The component model requires imports to be satisfied by providing instances
//! that match the imported interface types. This module handles the resolution
//! and linking of imports to concrete providers.
//!
//! **Note**: The unified `ComponentLinker` is now in `crate::components::component_linker`.

pub mod wasi_provider;
pub mod wasi_stdout;

// Re-export unified ComponentLinker from its new location for convenience
pub use crate::components::component_linker::ComponentLinker;
pub use wasi_provider::WasiInstanceProvider;
pub use wasi_stdout::{wasi_blocking_write_and_flush, wasi_get_stdout, write_stdout};
