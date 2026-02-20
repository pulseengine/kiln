//! Generic Provider Factory
//!
//! This module provides a generic factory pattern for creating memory providers
//! with automatic budget enforcement. It can work with any provider type and
//! coordinator implementation.
//!
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement

use core::marker::PhantomData;

use crate::{
    codes,
    generic_memory_guard::{
        GenericMemoryGuard,
        ManagedMemoryProvider,
        MemoryCoordinator,
    },
    Error,
    ErrorCategory,
    Result,
};

/// Trait for types that can create memory providers
pub trait ProviderFactory: Sized {
    /// The provider type this factory creates
    type Provider: ManagedMemoryProvider;

    /// Create a new provider with the specified size
    fn create_provider(&self, size: usize) -> Result<Self::Provider>;
}

/// Generic budget-aware provider factory
///
/// This factory ensures all provider creation goes through budget checks
/// and returns RAII guards for automatic cleanup.
///
/// # Type Parameters
///
/// * `F` - The underlying factory type
/// * `C` - The coordinator type
/// * `I` - The crate identifier type
pub struct GenericBudgetAwareFactory<F, C, I>
where
    F: ProviderFactory,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    /// The underlying factory
    factory:     F,
    /// The memory coordinator
    coordinator: &'static C,
    /// Phantom data for type parameters
    _phantom:    PhantomData<I>,
}

impl<F, C, I> GenericBudgetAwareFactory<F, C, I>
where
    F: ProviderFactory,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    /// Create a new budget-aware factory
    pub fn new(factory: F, coordinator: &'static C) -> Self {
        Self {
            factory,
            coordinator,
            _phantom: PhantomData,
        }
    }

    /// Create a provider with budget checking
    pub fn create_provider(
        &self,
        size: usize,
        crate_id: I,
    ) -> Result<GenericMemoryGuard<F::Provider, C, I>> {
        // Create the provider
        let provider = self.factory.create_provider(size)?;

        // Wrap in RAII guard
        GenericMemoryGuard::new(provider, self.coordinator, crate_id)
    }

    /// Get reference to the coordinator
    pub fn coordinator(&self) -> &'static C {
        self.coordinator
    }
}

/// Configuration for factory creation
pub struct FactoryConfig<F, C, I>
where
    C: 'static,
{
    pub factory:     F,
    pub coordinator: &'static C,
    _phantom:        PhantomData<I>,
}

impl<F, C, I> FactoryConfig<F, C, I>
where
    F: ProviderFactory,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    pub fn new(factory: F, coordinator: &'static C) -> Self {
        Self {
            factory,
            coordinator,
            _phantom: PhantomData,
        }
    }

    pub fn build(self) -> GenericBudgetAwareFactory<F, C, I> {
        GenericBudgetAwareFactory::new(self.factory, self.coordinator)
    }
}

/// Macro to simplify factory creation
#[macro_export]
macro_rules! create_budget_factory {
    ($factory:expr, $coordinator:expr) => {
        $crate::generic_provider_factory::GenericBudgetAwareFactory::new($factory, $coordinator)
    };
}

/// Example implementation for a simple provider factory
#[cfg(feature = "examples")]
pub mod examples {
    use super::*;

    /// Simple memory provider for examples
    pub struct SimpleProvider {
        buffer: Vec<u8>,
    }

    impl ManagedMemoryProvider for SimpleProvider {
        fn allocation_size(&self) -> usize {
            self.buffer.capacity()
        }
    }

    /// Factory for simple providers
    pub struct SimpleProviderFactory;

    impl ProviderFactory for SimpleProviderFactory {
        type Provider = SimpleProvider;

        fn create_provider(&self, size: usize) -> Result<Self::Provider> {
            Ok(SimpleProvider {
                buffer: Vec::with_capacity(size),
            })
        }
    }
}

