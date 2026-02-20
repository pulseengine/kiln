//! Namespace implementation for WebAssembly Component Model.
//!
//! This module provides the Namespace type for organizing imports and exports.

use crate::prelude::*;

/// Represents a namespace for component imports and exports
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace {
    /// Parts of the namespace separated by dots
    parts: Vec<String>,
}

impl Namespace {
    /// Creates a new namespace from a string representation
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_component::namespace::Namespace;
    ///
    /// let ns = Namespace::from_string("wasi.io";
    /// assert_eq!(ns.to_string(), "wasi.io";
    /// ```
    pub fn from_string(s: &str) -> Self {
        if s.is_empty() {
            return Self { parts: Vec::new() };
        }

        let parts = s.split('.').map(ToString::to_string).collect();

        Self { parts }
    }

    /// Creates a new namespace from a vector of parts
    pub fn from_parts(parts: Vec<String>) -> Self {
        Self { parts }
    }

    /// Returns the parts of the namespace
    pub fn parts(&self) -> &[String] {
        &self.parts
    }

    /// Returns true if the namespace is empty
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Returns the length (number of parts) of the namespace
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Joins this namespace with another namespace or name
    pub fn join(&self, other: &str) -> Self {
        let mut new_parts = self.parts.clone();

        for part in other.split('.') {
            if !part.is_empty() {
                new_parts.push(part.to_string());
            }
        }

        Self { parts: new_parts }
    }

    /// Returns a parent namespace (removes the last part)
    pub fn parent(&self) -> Option<Self> {
        if self.parts.is_empty() {
            None
        } else {
            let mut new_parts = self.parts.clone();
            new_parts.pop);
            Some(Self { parts: new_parts })
        }
    }
}

impl ToString for Namespace {
    fn to_string(&self) -> String {
        self.parts.join(".")
    }
