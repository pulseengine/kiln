// WRT - wrt-foundation
// Module: Component Value Store Builder
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides a builder pattern for ComponentValueStore.
//!
//! This module implements a fluent API for constructing and configuring
//! ComponentValueStore instances, ensuring proper initialization and
//! resource management in both std and no_std environments.

use core::fmt::Debug;

use wrt_error::Result;

// Internal imports
use crate::{
    component_value_store::{
        ComponentValueStore,
        MAX_STORE_TYPES,
        MAX_STORE_VALUES,
        MAX_TYPE_TO_REF_MAP_ENTRIES,
    },
    verification::VerificationLevel,
    MemoryProvider,
};

/// Builder for `ComponentValueStore` instances.
///
/// This builder provides a fluent API for configuring and constructing
/// ComponentValueStore objects with various settings, including memory
/// provider, verification level, and capacity hints.
#[derive(Debug, Clone)]
pub struct ComponentValueStoreBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    /// The memory provider to use
    provider:                P,
    /// The verification level for safety checks
    verification_level:      VerificationLevel,
    /// Optional hint for initial values capacity
    initial_values_capacity: Option<usize>,
    /// Optional hint for initial types capacity
    initial_types_capacity:  Option<usize>,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default
    for ComponentValueStoreBuilder<P>
{
    fn default() -> Self {
        Self {
            provider:                P::default(),
            verification_level:      VerificationLevel::default(),
            initial_values_capacity: None,
            initial_types_capacity:  None,
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ComponentValueStoreBuilder<P> {
    /// Creates a new ComponentValueStoreBuilder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    #[must_use]
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the verification level for safety checks.
    #[must_use]
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Sets a hint for the initial capacity for stored values.
    ///
    /// This is a hint only - the actual capacity is limited by
    /// `MAX_STORE_VALUES`.
    #[must_use]
    pub fn with_initial_values_capacity(mut self, capacity: usize) -> Self {
        self.initial_values_capacity = Some(capacity.min(MAX_STORE_VALUES));
        self
    }

    /// Sets a hint for the initial capacity for stored types.
    ///
    /// This is a hint only - the actual capacity is limited by
    /// `MAX_STORE_TYPES`.
    #[must_use]
    pub fn with_initial_types_capacity(mut self, capacity: usize) -> Self {
        self.initial_types_capacity = Some(capacity.min(MAX_STORE_TYPES));
        self
    }

    /// Builds and returns a configured `ComponentValueStore`.
    ///
    /// # Errors
    ///
    /// Returns an error if the component value store cannot be created with the
    /// given provider.
    pub fn build(self) -> wrt_error::Result<ComponentValueStore<P>> {
        // First configure the provider with the specified verification level
        let mut provider = self.provider.clone();
        provider.set_verification_level(self.verification_level);

        // Create the store with the configured provider
        let mut store = ComponentValueStore::new(provider)?;

        // Binary std/no_std choice
        // However, since BoundedVec doesn't have a with_capacity constructor,
        // we'll leave this as a placeholder for future enhancement

        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_sizing::SmallProvider;

    #[test]
    fn test_component_value_store_builder() {
        // Create a builder with default settings
        let builder = ComponentValueStoreBuilder::<SmallProvider>::new();

        // Build the store
        let store = builder.build().unwrap();

        // Verify the store has been properly initialized
        assert_eq!(
            store.get_provider().verification_level(),
            VerificationLevel::default()
        );
    }

    #[test]
    fn test_component_value_store_builder_with_options() {
        // Create a builder with custom settings
        let provider = SmallProvider::with_verification_level(VerificationLevel::Full);

        let builder = ComponentValueStoreBuilder::<SmallProvider>::new()
            .with_provider(provider.clone())
            .with_verification_level(VerificationLevel::Full)
            .with_initial_values_capacity(100)
            .with_initial_types_capacity(50);

        // Build the store
        let store = builder.build().unwrap();

        // Verify the store has been properly initialized with our settings
        assert_eq!(
            store.get_provider().verification_level(),
            VerificationLevel::Full
        );
    }
}
