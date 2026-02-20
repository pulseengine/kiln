//! Capability-based WebAssembly execution engine
//!
//! This module provides a unified engine abstraction that uses capabilities
//! to enforce different safety levels (QM, ASIL-A, ASIL-B).

pub mod builder;
pub mod capability_engine;
pub mod presets;

pub use builder::EngineBuilder;
pub use capability_engine::{
    CapabilityAwareEngine,
    CapabilityEngine,
    EnginePreset,
    InstanceHandle,
    ModuleHandle,
};
pub use presets::{
    asil_a,
    asil_b,
    asil_c,
    asil_d,
    qm,
};
