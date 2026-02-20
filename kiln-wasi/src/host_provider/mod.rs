//! Host provider implementations for WASI integration
//!
//! This module provides the integration layer between WASI interfaces and
//! the WRT component model, built on proven patterns from wrt-host and wrt-component.

pub mod component_model_provider;
pub mod resource_manager;

// Re-export main types
pub use component_model_provider::ComponentModelProvider;
pub use resource_manager::WasiResourceManager;