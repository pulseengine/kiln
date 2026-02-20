//! Type factory pattern for allocation boundary management
//!
//! This module provides factories that bridge between clean, provider-agnostic
//! types and provider-aware implementations. The factory pattern allows us to
//! keep public APIs free from provider parameters while still supporting
//! bounded allocation internally.
//!
//! Note: This module requires allocation capabilities (std or alloc feature).

// Only compile this module when allocation is available
#[cfg(any(feature = "std", feature = "alloc"))]
pub use factory::*;

#[cfg(any(feature = "std", feature = "alloc"))]
mod factory {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::{
        boxed::Box,
        string::String,
        vec::Vec,
    };
    #[cfg(not(feature = "std"))]
    use core::marker::PhantomData;
    #[cfg(feature = "std")]
    use std::marker::PhantomData;
    #[cfg(feature = "std")]
    use std::{
        boxed::Box,
        string::String,
        vec::Vec,
    };

    use crate::{
        bounded::{
            BoundedString,
            BoundedVec,
        },
        clean_types::{
            Case as CleanCase,
            ComponentType as CleanComponentType,
            Enum as CleanEnum,
            ExternType as CleanExternType,
            Field as CleanField,
            Flags as CleanFlags,
            FuncType as CleanFuncType,
            GlobalType as CleanGlobalType,
            MemoryType as CleanMemoryType,
            Record as CleanRecord,
            Result_ as CleanResult,
            TableType as CleanTableType,
            Tuple as CleanTuple,
            ValType as CleanValType,
            Value as CleanValue,
            Variant as CleanVariant,
        },
        codes,
        safe_memory::{
            MemoryProvider,
            NoStdProvider,
        },
        Error,
        ErrorCategory,
        Result,
    };

    /// Factory trait for creating provider-aware types from clean types
    pub trait TypeFactory {
        /// Provider type used by this factory
        type Provider: MemoryProvider + Clone + PartialEq + Eq + Default;

        /// Create a bounded string from a clean string
        fn create_bounded_string<const N: usize>(
            &self,
            s: &str,
        ) -> Result<BoundedString<N>>;

        /// Create a bounded vector from a clean vector
        fn create_bounded_vec<T, const N: usize>(
            &self,
            items: Vec<T>,
        ) -> Result<BoundedVec<T, N, Self::Provider>>
        where
            T: Clone
                + Default
                + PartialEq
                + Eq
                + crate::traits::Checksummable
                + crate::traits::ToBytes
                + crate::traits::FromBytes;

        /// Convert clean ValType to provider-aware ValType (if needed by legacy
        /// code)
        fn convert_valtype(&self, clean_type: &CleanValType) -> Result<CleanValType> {
            // For now, return the same type since we're moving away from provider-embedded
            // types
            Ok(clean_type.clone())
        }

        /// Convert clean Value to provider-aware Value (if needed by legacy
        /// code)
        fn convert_value(&self, clean_value: &CleanValue) -> Result<CleanValue> {
            // For now, return the same value since we're moving away from provider-embedded
            // types
            Ok(clean_value.clone())
        }
    }

    /// Runtime type factory using NoStdProvider
    pub struct RuntimeTypeFactory<const BUFFER_SIZE: usize> {
        provider: NoStdProvider<BUFFER_SIZE>,
    }

    impl<const BUFFER_SIZE: usize> RuntimeTypeFactory<BUFFER_SIZE> {
        /// Create a new runtime type factory
        pub fn new() -> Self {
            let provider = crate::safe_managed_alloc!(
                BUFFER_SIZE,
                crate::budget_aware_provider::CrateId::Foundation
            )
            .unwrap_or_else(|_| NoStdProvider::<BUFFER_SIZE>::default());
            Self { provider }
        }

        /// Create with custom provider
        pub fn with_provider(provider: NoStdProvider<BUFFER_SIZE>) -> Self {
            Self { provider }
        }

        /// Get a reference to the internal provider
        pub fn provider(&self) -> &NoStdProvider<BUFFER_SIZE> {
            &self.provider
        }
    }

    impl<const BUFFER_SIZE: usize> Default for RuntimeTypeFactory<BUFFER_SIZE> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<const BUFFER_SIZE: usize> TypeFactory for RuntimeTypeFactory<BUFFER_SIZE> {
        type Provider = NoStdProvider<BUFFER_SIZE>;

        fn create_bounded_string<const N: usize>(
            &self,
            s: &str,
        ) -> Result<BoundedString<N>> {
            BoundedString::try_from_str(s)
                .map_err(|_| Error::memory_error("String too long for bounded string"))
        }

        fn create_bounded_vec<T, const N: usize>(
            &self,
            items: Vec<T>,
        ) -> Result<BoundedVec<T, N, Self::Provider>>
        where
            T: Clone
                + Default
                + PartialEq
                + Eq
                + crate::traits::Checksummable
                + crate::traits::ToBytes
                + crate::traits::FromBytes,
        {
            let mut bounded_vec = BoundedVec::new(self.provider.clone())
                .map_err(|e| Error::memory_error("Failed to create bounded vector"))?;

            for item in items {
                bounded_vec
                    .push(item)
                    .map_err(|_| Error::memory_error("Bounded vector capacity exceeded"))?;
            }

            Ok(bounded_vec)
        }
    }

    /// Component type factory using NoStdProvider
    pub struct ComponentTypeFactory<const BUFFER_SIZE: usize> {
        provider: NoStdProvider<BUFFER_SIZE>,
    }

    impl<const BUFFER_SIZE: usize> ComponentTypeFactory<BUFFER_SIZE> {
        /// Create a new component type factory
        pub fn new() -> Self {
            let provider = crate::safe_managed_alloc!(
                BUFFER_SIZE,
                crate::budget_aware_provider::CrateId::Foundation
            )
            .unwrap_or_else(|_| NoStdProvider::<BUFFER_SIZE>::default());
            Self { provider }
        }

        /// Create with custom provider
        pub fn with_provider(provider: NoStdProvider<BUFFER_SIZE>) -> Self {
            Self { provider }
        }

        /// Get a reference to the internal provider
        pub fn provider(&self) -> &NoStdProvider<BUFFER_SIZE> {
            &self.provider
        }
    }

    impl<const BUFFER_SIZE: usize> Default for ComponentTypeFactory<BUFFER_SIZE> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<const BUFFER_SIZE: usize> TypeFactory for ComponentTypeFactory<BUFFER_SIZE> {
        type Provider = NoStdProvider<BUFFER_SIZE>;

        fn create_bounded_string<const N: usize>(
            &self,
            s: &str,
        ) -> Result<BoundedString<N>> {
            BoundedString::try_from_str(s)
                .map_err(|_| Error::memory_error("String too long for bounded string"))
        }

        fn create_bounded_vec<T, const N: usize>(
            &self,
            items: Vec<T>,
        ) -> Result<BoundedVec<T, N, Self::Provider>>
        where
            T: Clone
                + Default
                + PartialEq
                + Eq
                + crate::traits::Checksummable
                + crate::traits::ToBytes
                + crate::traits::FromBytes,
        {
            let mut bounded_vec = BoundedVec::new(self.provider.clone())
                .map_err(|e| Error::memory_error("Failed to create bounded vector"))?;

            for item in items {
                bounded_vec
                    .push(item)
                    .map_err(|_| Error::memory_error("Bounded vector capacity exceeded"))?;
            }

            Ok(bounded_vec)
        }
    }

    /// Type conversion utilities between clean and provider-aware types
    pub struct TypeConverter;

    impl TypeConverter {
        /// Convert clean field to provider-aware field (for legacy
        /// compatibility)
        pub fn convert_field_to_bounded<F, const BUFFER_SIZE: usize>(
            field: &CleanField,
            _factory: &F,
        ) -> Result<CleanField>
        where
            F: TypeFactory<Provider = NoStdProvider<BUFFER_SIZE>>,
        {
            // For now, just clone since we're moving away from provider embedding
            Ok(field.clone())
        }

        /// Convert clean record to provider-aware record (for legacy
        /// compatibility)
        pub fn convert_record_to_bounded<F, const BUFFER_SIZE: usize>(
            record: &CleanRecord,
            _factory: &F,
        ) -> Result<CleanRecord>
        where
            F: TypeFactory<Provider = NoStdProvider<BUFFER_SIZE>>,
        {
            // For now, just clone since we're moving away from provider embedding
            Ok(record.clone())
        }

        /// Convert provider-aware field back to clean field
        pub fn convert_field_from_bounded(field: &CleanField) -> CleanField {
            field.clone()
        }

        /// Convert provider-aware record back to clean record
        pub fn convert_record_from_bounded(record: &CleanRecord) -> CleanRecord {
            record.clone()
        }
    }

    /// Standard factory type aliases for common buffer sizes
    pub type RuntimeFactory8K = RuntimeTypeFactory<8192>;
    pub type RuntimeFactory64K = RuntimeTypeFactory<65536>;
    pub type RuntimeFactory1M = RuntimeTypeFactory<1048576>;

    pub type ComponentFactory8K = ComponentTypeFactory<8192>;
    pub type ComponentFactory64K = ComponentTypeFactory<65536>;
    pub type ComponentFactory1M = ComponentTypeFactory<1048576>;

    /// Factory builder for creating factories with specific configurations
    pub struct FactoryBuilder<const BUFFER_SIZE: usize> {
        _phantom: PhantomData<[u8; BUFFER_SIZE]>,
    }

    impl<const BUFFER_SIZE: usize> FactoryBuilder<BUFFER_SIZE> {
        /// Create a new factory builder
        pub fn new() -> Self {
            Self {
                _phantom: PhantomData,
            }
        }

        /// Build a runtime factory
        pub fn build_runtime_factory(self) -> RuntimeTypeFactory<BUFFER_SIZE> {
            RuntimeTypeFactory::new()
        }

        /// Build a component factory
        pub fn build_component_factory(self) -> ComponentTypeFactory<BUFFER_SIZE> {
            ComponentTypeFactory::new()
        }
    }

    impl<const BUFFER_SIZE: usize> Default for FactoryBuilder<BUFFER_SIZE> {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_runtime_factory_creation() {
            let factory = RuntimeTypeFactory::<1024>::new();
            // Verify that factory was created successfully
            assert!(factory.provider().size() > 0);
        }

        #[test]
        fn test_component_factory_creation() {
            let factory = ComponentTypeFactory::<1024>::new();
            // Verify that factory was created successfully
            assert!(factory.provider().size() > 0);
        }

        #[test]
        fn test_bounded_string_creation() {
            let factory = RuntimeTypeFactory::<1024>::new();
            let bounded_str = factory.create_bounded_string::<64>("test").unwrap();
            assert_eq!(bounded_str.as_str().unwrap(), "test");
        }

        #[test]
        fn test_factory_builder() {
            let builder = FactoryBuilder::<1024>::new();
            let runtime_factory = builder.build_runtime_factory();

            let builder2 = FactoryBuilder::<1024>::new();
            let component_factory = builder2.build_component_factory();

            // Just ensure they can be created
            assert!(!core::ptr::eq(
                runtime_factory.provider(),
                component_factory.provider()
            ));
        }

        #[test]
        fn test_type_converter() {
            let field = CleanField {
                name: "test_field".to_string(),
                ty:   CleanValType::S32,
            };

            let converted = TypeConverter::convert_field_from_bounded(&field);
            assert_eq!(converted.name, field.name);
            assert_eq!(converted.ty, field.ty);
        }
    }
}
