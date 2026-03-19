//! WebAssembly runtime state management.
//!
//! This module provides utilities for managing and serializing WebAssembly
//! runtime state including stack frames, globals, and memory.

pub mod serialization;

// Re-export functions
pub use serialization::{
    create_state_section,
    extract_state_section,
    has_state_sections,
    is_state_section_name,
};
pub use serialization::{
    StateHeader,
    StateSection,
    STATE_SECTION_PREFIX,
};
