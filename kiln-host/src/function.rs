// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Host function infrastructure.
//!
//! This module provides the core infrastructure for host functions
//! that can be called from WebAssembly components.

// Use the prelude for consistent imports
use crate::prelude::{
    Any,
    Eq,
    Error,
    ErrorCategory,
    PartialEq,
    Result,
    Value,
};

// Value vectors for function parameters/returns
#[cfg(feature = "std")]
type ValueVec = Vec<Value>;

#[cfg(not(feature = "std"))]
use crate::bounded_host_infra::HostProvider;

#[cfg(not(feature = "std"))]
type ValueVec = wrt_foundation::BoundedVec<Value, 16, HostProvider>;

/// A trait for functions that can be cloned and operate on value vectors.
/// This is used for storing host functions that can be called by the Wasm
/// engine.
#[cfg(feature = "std")]
pub trait FnWithVecValue: Send + Sync {
    /// Calls the function with the given target and arguments.
    fn call(&self, target: &mut dyn Any, args: ValueVec) -> Result<ValueVec>;

    /// Clones the function into a `Box`.
    fn clone_box(&self) -> Box<dyn FnWithVecValue>;
}

/// Simplified trait for `no_std` environments without dynamic dispatch
#[cfg(not(feature = "std"))]
pub trait FnWithVecValue: Send + Sync {
    /// Calls the function with the given target and arguments.
    fn call(&self, target: &mut dyn Any, args: ValueVec) -> Result<ValueVec>;
}

/// Wrapper for functions that take both target and arguments
#[cfg(feature = "std")]
#[derive(Clone)]
struct ArgsWrapper<F>
where
    F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    func: F,
}

#[cfg(feature = "std")]
impl<F> FnWithVecValue for ArgsWrapper<F>
where
    F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    fn call(&self, target: &mut dyn Any, args: ValueVec) -> Result<ValueVec> {
        (self.func)(target, args)
    }

    fn clone_box(&self) -> Box<dyn FnWithVecValue> {
        Box::new(self.clone())
    }
}

#[cfg(feature = "std")]
impl<F> FnWithVecValue for F
where
    F: Fn(&mut dyn Any) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    fn call(&self, target: &mut dyn Any, _args: ValueVec) -> Result<ValueVec> {
        // Using target but ignoring args since the function only takes target
        // This could be extended in the future to support functions that take args
        self(target)
    }

    fn clone_box(&self) -> Box<dyn FnWithVecValue> {
        Box::new(self.clone())
    }
}

/// Wrapper for functions that take both target and arguments (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Clone)]
struct ArgsWrapper<F>
where
    F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    func: F,
}

#[cfg(not(feature = "std"))]
impl<F> FnWithVecValue for ArgsWrapper<F>
where
    F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    fn call(&self, target: &mut dyn Any, args: ValueVec) -> Result<ValueVec> {
        (self.func)(target, args)
    }
}

#[cfg(not(feature = "std"))]
impl<F> FnWithVecValue for F
where
    F: Fn(&mut dyn Any) -> Result<ValueVec> + Send + Sync + Clone + 'static,
{
    fn call(&self, target: &mut dyn Any, _args: ValueVec) -> Result<ValueVec> {
        // Using target but ignoring args since the function only takes target
        // This could be extended in the future to support functions that take args
        self(target)
    }
}

/// A wrapper struct that makes a closure implementing `Fn` cloneable
/// by boxing it and handling the cloning via the `FnWithVecValue` trait.
#[cfg(feature = "std")]
pub struct CloneableFn(Box<dyn FnWithVecValue>);

/// Simplified function wrapper for `no_std` environments
#[cfg(not(feature = "std"))]
pub struct CloneableFn;

#[cfg(feature = "std")]
impl CloneableFn {
    /// Creates a new `CloneableFn` from a closure.
    ///
    /// The closure must be `Send`, `Sync`, `Clone`, and `'static`.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&mut dyn Any) -> Result<ValueVec> + Send + Sync + Clone + 'static,
    {
        Self(Box::new(f))
    }

    /// Creates a new `CloneableFn` from a closure that takes both target and
    /// arguments.
    ///
    /// The closure must be `Send`, `Sync`, `Clone`, and `'static`.
    pub fn new_with_args<F>(f: F) -> Self
    where
        F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
    {
        Self(Box::new(ArgsWrapper { func: f }))
    }

    /// Calls the wrapped function.
    pub fn call(&self, target: &mut dyn Any, args: ValueVec) -> Result<ValueVec> {
        self.0.call(target, args)
    }
}

#[cfg(not(feature = "std"))]
impl CloneableFn {
    /// Creates a new `CloneableFn` from a closure.
    ///
    /// In `no_std` mode, this is a no-op since we can't store dynamic
    /// functions.
    pub fn new<F>(_f: F) -> Self
    where
        F: Fn(&mut dyn Any) -> Result<ValueVec> + Send + Sync + Clone + 'static,
    {
        Self
    }

    /// Creates a new `CloneableFn` from a closure that takes both target and
    /// arguments.
    ///
    /// In `no_std` mode, this is a no-op since we can't store dynamic
    /// functions.
    pub fn new_with_args<F>(_f: F) -> Self
    where
        F: Fn(&mut dyn Any, ValueVec) -> Result<ValueVec> + Send + Sync + Clone + 'static,
    {
        Self
    }

    /// Calls the wrapped function.
    ///
    /// In `no_std` mode, this always returns an error since we can't store
    /// dynamic functions.
    pub fn call(&self, _target: &mut dyn Any, _args: ValueVec) -> Result<ValueVec> {
        Err(Error::new(
            ErrorCategory::Runtime,
            wrt_error::codes::NOT_IMPLEMENTED,
            "Dynamic function calls not supported in pure no_std mode",
        ))
    }
}

impl Clone for CloneableFn {
    fn clone(&self) -> Self {
        #[cfg(feature = "std")]
        {
            Self(self.0.clone_box())
        }

        #[cfg(not(feature = "std"))]
        {
            // In no_std mode, create a default function
            CloneableFn
        }
    }
}

impl PartialEq for CloneableFn {
    fn eq(&self, _other: &Self) -> bool {
        // Function pointers can't be meaningfully compared
        false
    }
}

impl Eq for CloneableFn {}

/// Host function handler type for implementing WebAssembly imports
pub type HostFunctionHandler = CloneableFn;

// Implement required traits for CloneableFn to work with BoundedMap in no_std
// mode
#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for CloneableFn {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Function pointers can't be meaningfully checksummed, use a placeholder
        checksum.update_slice(b"cloneable_fn");
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for CloneableFn {
    fn serialized_size(&self) -> usize {
        // Function pointers can't be serialized, return 0
        0
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        _writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Function pointers can't be serialized
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for CloneableFn {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // Function pointers can't be deserialized, return a dummy function
        Ok(CloneableFn::new(|_| {
            Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Runtime,
                wrt_error::codes::RUNTIME_ERROR,
                "Deserialized function not implemented",
            ))
        }))
    }
}

#[cfg(not(feature = "std"))]
impl Default for CloneableFn {
    fn default() -> Self {
        CloneableFn::new(|_| {
            Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Runtime,
                wrt_error::codes::RUNTIME_ERROR,
                "Default function not implemented",
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::vec;

    use wrt_foundation::{
        allocator::CrateId,
        safe_managed_alloc,
        safe_memory::NoStdProvider,
    };

    use super::*;

    #[test]
    fn test_cloneable_fn() {
        let f = CloneableFn::new(|_| {
            #[cfg(feature = "std")]
            return Ok(vec![Value::I32(42)]);

            #[cfg(not(feature = "std"))]
            {
                // Use capability-aware allocation for safety-critical code
                let provider = crate::bounded_host_infra::create_host_provider()?;
                let mut vec = ValueVec::new(provider).unwrap();
                vec.push(Value::I32(42)).unwrap();
                Ok(vec)
            }
        });
        let f2 = f.clone();

        let mut target = ();

        #[cfg(feature = "std")]
        let empty_args = vec![];
        #[cfg(not(feature = "std"))]
        let empty_args = {
            let provider = safe_managed_alloc!(8192, CrateId::Host).unwrap();
            ValueVec::new(provider).unwrap()
        };

        let result = f.call(&mut target, empty_args.clone());
        let result2 = f2.call(&mut target, empty_args);

        assert!(result.is_ok());
        let result_vec = result.unwrap();
        assert_eq!(result_vec.len(), 1);
        assert!(matches!(result_vec.get(0).unwrap(), Value::I32(42)));

        assert!(result2.is_ok());
        let result2_vec = result2.unwrap();
        assert_eq!(result2_vec.len(), 1);
        assert!(matches!(result2_vec.get(0).unwrap(), Value::I32(42)));
    }

    #[test]
    fn test_host_function_handler() {
        let handler = HostFunctionHandler::new(|_| {
            #[cfg(feature = "std")]
            return Ok(vec![Value::I32(42)]);

            #[cfg(not(feature = "std"))]
            {
                // Use capability-aware allocation for safety-critical code
                let provider = crate::bounded_host_infra::create_host_provider()?;
                let mut vec = ValueVec::new(provider).unwrap();
                vec.push(Value::I32(42)).unwrap();
                Ok(vec)
            }
        });

        let mut target = ();

        #[cfg(feature = "std")]
        let empty_args = vec![];
        #[cfg(not(feature = "std"))]
        let empty_args = {
            let provider = safe_managed_alloc!(8192, CrateId::Host).unwrap();
            ValueVec::new(provider).unwrap()
        };

        let result = handler.call(&mut target, empty_args);

        assert!(result.is_ok());
        let result_vec = result.unwrap();
        assert_eq!(result_vec.len(), 1);
        assert!(matches!(result_vec.get(0).unwrap(), Value::I32(42)));
    }
}
