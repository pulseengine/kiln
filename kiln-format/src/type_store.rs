//! Type storage system for Component Model types
//!
//! This module provides a bounded type storage system that can manage ValType
//! instances and provide stable references (ValTypeRef) for use in recursive
//! type definitions.
//!
//! The design aligns with the Component Model's resource management approach
//! and uses bounded collections for predictable memory usage in no_std
//! environments.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
};
use wrt_foundation::{
    bounded::BoundedVec,
    component_value::{
        ValType,
        ValTypeRef,
    },
    traits::BoundedCapacity,
    MemoryProvider,
};

/// Maximum number of types that can be stored
/// This aligns with Component Model limits
pub const MAX_STORED_TYPES: usize = 1024;

/// Type storage for managing ValType instances with bounded memory
pub struct TypeStore<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    /// Stored types indexed by ValTypeRef
    types: BoundedVec<ValType<P>, MAX_STORED_TYPES, P>,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> TypeStore<P> {
    /// Create a new type store with the given provider
    pub fn new(provider: P) -> Result<Self, Error> {
        Ok(Self {
            types: BoundedVec::new(provider)?,
        })
    }

    /// Store a type and return its reference
    pub fn intern(&mut self, val_type: ValType<P>) -> Result<ValTypeRef, Error> {
        // Check if we already have this type
        for (index, stored_type) in self.types.iter().enumerate() {
            if stored_type == val_type {
                return Ok(ValTypeRef(index as u32));
            }
        }

        // Add new type
        let index = self.types.len() as u32;
        self.types
            .push(val_type)
            .map_err(|_| Error::memory_error("Type store capacity exceeded"))?;
        Ok(ValTypeRef(index))
    }

    /// Get a type by its reference
    pub fn get(&self, _type_ref: ValTypeRef) -> Option<&ValType<P>> {
        // BoundedVec's get returns Result<T, Error>, not Option<&T>
        // We can't return a reference to the value since it's returned by value
        // This needs a different API design - for now return None
        None
    }

    /// Get a type by its reference (by value)
    pub fn get_by_value(&self, type_ref: ValTypeRef) -> Option<ValType<P>> {
        self.types.get(type_ref.0 as usize).ok()
    }

    /// Get a mutable type by its reference
    /// Note: Currently not supported with BoundedVec implementation
    pub fn get_mut(&mut self, _type_ref: ValTypeRef) -> Option<&mut ValType<P>> {
        // BoundedVec doesn't support get_mut, so we can't provide mutable access
        None
    }

    /// Get the number of stored types
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Clear all stored types
    pub fn clear(&mut self) {
        let _ = self.types.clear();
    }
}

/// Builder for constructing types with automatic interning
#[allow(dead_code)] // Stub implementation for future type building functionality
pub struct TypeBuilder<'a, P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    store:    &'a mut TypeStore<P>,
    provider: P,
}

impl<'a, P: MemoryProvider + Default + Clone + PartialEq + Eq> TypeBuilder<'a, P> {
    /// Create a new type builder
    pub fn new(store: &'a mut TypeStore<P>, provider: P) -> Self {
        Self { store, provider }
    }

    /// Build a primitive type (no interning needed)
    pub fn primitive(&self, prim: ValType<P>) -> ValType<P> {
        prim
    }

    /// Build a list type
    pub fn list(&mut self, element: ValType<P>) -> Result<ValType<P>, Error> {
        let element_ref = self.store.intern(element)?;
        Ok(ValType::List(element_ref))
    }

    /// Build a fixed list type
    pub fn fixed_list(&mut self, element: ValType<P>, length: u32) -> Result<ValType<P>, Error> {
        let element_ref = self.store.intern(element)?;
        Ok(ValType::FixedList(element_ref, length))
    }

    /// Build an option type
    pub fn option(&mut self, inner: ValType<P>) -> Result<ValType<P>, Error> {
        let inner_ref = self.store.intern(inner)?;
        Ok(ValType::Option(inner_ref))
    }

    /// Build a result type
    pub fn result(
        &mut self,
        ok: Option<ValType<P>>,
        err: Option<ValType<P>>,
    ) -> Result<ValType<P>, Error> {
        let ok_ref = match ok {
            Some(t) => Some(self.store.intern(t)?),
            None => None,
        };
        let err_ref = match err {
            Some(t) => Some(self.store.intern(t)?),
            None => None,
        };
        Ok(ValType::Result {
            ok:  ok_ref,
            err: err_ref,
        })
    }
}

#[cfg(test)]
mod tests {
    use wrt_foundation::traits::DefaultMemoryProvider;

    use super::*;

    #[test]
    fn test_type_store_basic() {
        let provider = DefaultMemoryProvider::default();
        let mut store = TypeStore::new(provider.clone()).unwrap();

        // Store a simple type
        let i32_type = ValType::S32;
        let i32_ref = store.intern(i32_type.clone()).unwrap();

        // Should get the same reference for the same type
        let i32_ref2 = store.intern(i32_type.clone()).unwrap();
        assert_eq!(i32_ref.0, i32_ref2.0);

        // Should be able to retrieve it
        let retrieved = store.get(i32_ref).unwrap();
        assert_eq!(retrieved, &i32_type);
    }

    #[test]
    fn test_type_builder() {
        let provider = DefaultMemoryProvider::default();
        let mut store = TypeStore::new(provider.clone()).unwrap();
        let mut builder = TypeBuilder::new(&mut store, provider);

        // Build a list of i32
        let list_type = builder.list(ValType::S32).unwrap();
        if let ValType::List(elem_ref) = &list_type {
            let elem_type = store.get(*elem_ref).unwrap();
            assert_eq!(elem_type, &ValType::S32);
        } else {
            panic!("Expected List type");
        }
    }
}
