//! WebAssembly module handling for components
//!
//! This module provides functionality to handle WebAssembly modules within
//! components.

use crate::prelude::*;

/// Represents a WebAssembly module within a component
#[derive(Debug)]
pub struct Module {
    /// Module binary data
    binary: Vec<u8>,
    /// Module name (optional)
    name: Option<String>,
}

impl Module {
    /// Create a new module from binary data
    pub fn new(binary: Vec<u8>) -> Self {
        Self { binary, name: None }
    }

    /// Create a new named module from binary data
    pub fn with_name(binary: Vec<u8>, name: &str) -> Self {
        Self { binary, name: Some(name.to_string()) }
    }

    /// Get the module binary data
    pub fn binary(&self) -> &[u8] {
        &self.binary
    }

    /// Get the module name
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set the module name
    pub fn set_name(&mut self, name: &str) {
        self.name = Some(name.to_string());

}
