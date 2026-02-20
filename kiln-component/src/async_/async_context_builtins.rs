// WRT - wrt-component
// Module: Async Context Management Built-ins
// SW-REQ-ID: REQ_ASYNC_CONTEXT_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Async Context Management Built-ins
//!
//! This module provides implementation of the `context.get` and `context.set`
//! built-in functions required by the WebAssembly Component Model for managing
//! async execution contexts.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::BTreeMap as HashMap, vec::Vec};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    collections::{BTreeMap, HashMap},
    vec::Vec,
};

// use wrt_decoder::prelude::DecoderVecExt; // TODO: Re-enable when wrt_decoder is available
use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    // atomic_memory::AtomicRefCell, // Not available in wrt-foundation
    BoundedMap,
    BoundedString,
    BoundedVec,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::FromBytes,
    types::ValueType,
};

// Temporary AtomicRefCell substitute for no_std compilation
// TODO: Replace with proper atomic implementation
use crate::bounded_component_infra::ComponentProvider;
#[cfg(not(feature = "std"))]
use crate::prelude::Mutex as AtomicRefCell;
use crate::prelude::WrtComponentValue;
#[cfg(feature = "std")]
use std::cell::RefCell as AtomicRefCell;

// Constants for no_std environments
#[cfg(not(any(feature = "std",)))]
const MAX_CONTEXT_ENTRIES: usize = 32;
#[cfg(not(any(feature = "std",)))]
const MAX_CONTEXT_VALUE_SIZE: usize = 256;
#[cfg(not(any(feature = "std",)))]
const MAX_CONTEXT_KEY_SIZE: usize = 64;

/// Context key identifier for async contexts
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg(feature = "std")]
pub struct ContextKey(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg(not(any(feature = "std",)))]
pub struct ContextKey(BoundedString<MAX_CONTEXT_KEY_SIZE>);

impl Default for ContextKey {
    fn default() -> Self {
        #[cfg(feature = "std")]
        return Self(String::new());
        #[cfg(not(any(feature = "std",)))]
        return Self(BoundedString::from_str_truncate("").unwrap_or_else(|_| {
            // Fallback: This should never happen, but we need to handle it gracefully
            panic!("Failed to create empty BoundedString");
        }));
    }
}

impl wrt_foundation::traits::Checksummable for ContextKey {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl wrt_runtime::ToBytes for ContextKey {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_runtime::FromBytes for ContextKey {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        return Ok(Self(String::from_bytes_with_provider(reader, provider)?));
        #[cfg(not(any(feature = "std",)))]
        return Ok(Self(BoundedString::from_bytes_with_provider(
            reader, provider,
        )?));
    }
}

impl ContextKey {
    #[cfg(feature = "std")]
    pub fn new(key: String) -> Self {
        Self(key)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn new(key: &str) -> Result<Self> {
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_key = BoundedString::try_from_str(key)
            .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
        Ok(Self(bounded_key))
    }

    pub fn as_str(&self) -> &str {
        #[cfg(feature = "std")]
        return &self.0;
        #[cfg(not(any(feature = "std",)))]
        // Safe to unwrap: string was successfully created in `new()`
        return self.0.as_str().unwrap();
    }
}

/// Context value that can be stored in an async context
#[derive(Debug, Clone)]
pub enum ContextValue {
    /// Simple value types
    Simple(WrtComponentValue<ComponentProvider>),
    /// Binary data (for serialized complex types)
    #[cfg(feature = "std")]
    Binary(Vec<u8>),
    #[cfg(not(any(feature = "std",)))]
    Binary(BoundedVec<u8, MAX_CONTEXT_VALUE_SIZE, NoStdProvider<4096>>),
}

impl Default for ContextValue {
    fn default() -> Self {
        Self::Simple(WrtComponentValue::default())
    }
}

impl PartialEq for ContextValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Simple(a), Self::Simple(b)) => a == b,
            (Self::Binary(a), Self::Binary(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ContextValue {}

impl wrt_foundation::traits::Checksummable for ContextValue {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::Simple(v) => v.update_checksum(checksum),
            Self::Binary(b) => {
                // Manual checksum update for Vec<u8> (DecoderVecExt not available)
                for byte in b.iter() {
                    byte.update_checksum(checksum);
                }
            },
        }
    }
}

impl wrt_runtime::ToBytes for ContextValue {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::Simple(v) => {
                0u8.to_bytes_with_provider(writer, provider)?;
                v.to_bytes_with_provider(writer, provider)
            },
            Self::Binary(b) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                // Manual serialization for Vec<u8> (DecoderVecExt not available)
                (b.len() as u32).to_bytes_with_provider(writer, provider)?;
                for byte in b.iter() {
                    byte.to_bytes_with_provider(writer, provider)?;
                }
                Ok(())
            },
        }
    }
}

impl wrt_runtime::FromBytes for ContextValue {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let tag = u8::from_bytes_with_provider(reader, provider)?;
        match tag {
            0 => Ok(Self::Simple(WrtComponentValue::from_bytes_with_provider(
                reader, provider,
            )?)),
            1 => {
                #[cfg(feature = "std")]
                return Ok(Self::Binary(
                    <Vec<u8> as FromBytes>::from_bytes_with_provider(reader, provider)?,
                ));
                #[cfg(not(any(feature = "std",)))]
                return Ok(Self::Binary(BoundedVec::from_bytes_with_provider(
                    reader, provider,
                )?));
            },
            _ => Err(Error::validation_error(
                "Invalid context value discriminant",
            )),
        }
    }
}

impl ContextValue {
    pub fn from_component_value(value: WrtComponentValue<ComponentProvider>) -> Self {
        Self::Simple(value)
    }

    #[cfg(feature = "std")]
    pub fn from_binary(data: Vec<u8>) -> Self {
        Self::Binary(data)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        let bounded_data = BoundedVec::new_from_slice(provider, data)
            .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
        Ok(Self::Binary(bounded_data))
    }

    pub fn as_component_value(&self) -> Option<&WrtComponentValue<ComponentProvider>> {
        match self {
            Self::Simple(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            #[cfg(feature = "std")]
            Self::Binary(data) => Some(data),
            #[cfg(not(any(feature = "std",)))]
            // Safe to unwrap: data was successfully stored in the context
            Self::Binary(data) => Some(data.as_slice().unwrap()),
            _ => None,
        }
    }
}

/// Async execution context that stores key-value pairs
#[derive(Debug, Clone)]
pub struct AsyncContext {
    #[cfg(feature = "std")]
    data: BTreeMap<ContextKey, ContextValue>,
    #[cfg(not(any(feature = "std",)))]
    data: BoundedMap<ContextKey, ContextValue, MAX_CONTEXT_ENTRIES, NoStdProvider<4096>>,
}

impl AsyncContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            data: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            data: {
                let provider = safe_managed_alloc!(4096, CrateId::Component)?;
                BoundedMap::new(provider)?
            },
        })
    }

    pub fn get(&self, key: &ContextKey) -> Option<ContextValue> {
        #[cfg(feature = "std")]
        {
            self.data.get(key).map(|v| v.clone())
        }
        #[cfg(not(feature = "std"))]
        {
            self.data.get(key).ok().flatten().map(|v| v.clone())
        }
    }

    pub fn set(&mut self, key: ContextKey, value: ContextValue) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.data.insert(key, value);
            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.data
                .insert(key, value)
                .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
            Ok(())
        }
    }

    pub fn remove(&mut self, key: &ContextKey) -> Option<ContextValue> {
        #[cfg(feature = "std")]
        {
            self.data.remove(key)
        }
        #[cfg(not(feature = "std"))]
        {
            self.data.remove(key).ok().flatten()
        }
    }

    pub fn contains_key(&self, key: &ContextKey) -> bool {
        #[cfg(feature = "std")]
        {
            self.data.contains_key(key)
        }
        #[cfg(not(any(feature = "std",)))]
        self.data.contains_key(key).unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        let _ = self.data.clear();
    }
}

impl Default for AsyncContext {
    fn default() -> Self {
        // Safe to unwrap: new() only fails on allocation errors which should be rare
        Self::new().unwrap()
    }
}

// Thread-local storage for async contexts in each execution thread
#[cfg(feature = "std")]
thread_local! {
    static ASYNC_CONTEXT_STACK: AtomicRefCell<Vec<AsyncContext>> =
        AtomicRefCell::new(Vec::new());
}

/// Global context storage for no_std environments
#[cfg(not(feature = "std"))]
static GLOBAL_ASYNC_CONTEXT: AtomicRefCell<Option<AsyncContext>> = AtomicRefCell::new(None);

/// Context manager that provides the canonical built-in functions
pub struct AsyncContextManager;

impl AsyncContextManager {
    /// Get the current async context
    /// Implements the `context.get` canonical built-in
    #[cfg(feature = "std")]
    pub fn context_get() -> Result<Option<AsyncContext>> {
        ASYNC_CONTEXT_STACK.with(|stack| {
            let stack_ref = stack
                .try_borrow()
                .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
            Ok(stack_ref.last().cloned())
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn context_get() -> Result<Option<AsyncContext>> {
        let context_ref = GLOBAL_ASYNC_CONTEXT.lock();
        Ok((*context_ref).clone())
    }

    /// Set the current async context
    /// Implements the `context.set` canonical built-in
    #[cfg(feature = "std")]
    pub fn context_set(context: AsyncContext) -> Result<()> {
        ASYNC_CONTEXT_STACK.with(|stack| {
            let mut stack_ref = stack
                .try_borrow_mut()
                .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
            stack_ref.push(context);
            Ok(())
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn context_set(context: AsyncContext) -> Result<()> {
        let mut context_ref = GLOBAL_ASYNC_CONTEXT.lock();
        *context_ref = Some(context);
        Ok(())
    }

    /// Push a new context onto the stack (for nested async operations)
    #[cfg(feature = "std")]
    pub fn context_push(context: AsyncContext) -> Result<()> {
        Self::context_set(context)
    }

    #[cfg(not(feature = "std"))]
    pub fn context_push(context: AsyncContext) -> Result<()> {
        Self::context_set(context)
    }

    /// Pop the current context from the stack
    #[cfg(feature = "std")]
    pub fn context_pop() -> Result<Option<AsyncContext>> {
        ASYNC_CONTEXT_STACK.with(|stack| {
            let mut stack_ref = stack
                .try_borrow_mut()
                .map_err(|_| Error::runtime_execution_error("Context access failed"))?;
            Ok(stack_ref.pop())
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn context_pop() -> Result<Option<AsyncContext>> {
        let mut context_ref = GLOBAL_ASYNC_CONTEXT.lock();
        Ok(context_ref.take())
    }

    /// Get a value from the current context by key
    pub fn get_context_value(key: &ContextKey) -> Result<Option<ContextValue>> {
        let context = Self::context_get()?;
        Ok(context.and_then(|ctx| ctx.get(key)))
    }

    /// Set a value in the current context by key
    pub fn set_context_value(key: ContextKey, value: ContextValue) -> Result<()> {
        let mut context = Self::context_get()?.unwrap_or_default();
        context.set(key, value)?;
        Self::context_set(context)
    }

    /// Remove a value from the current context by key
    pub fn remove_context_value(key: &ContextKey) -> Result<Option<ContextValue>> {
        if let Some(mut context) = Self::context_get()? {
            let removed = context.remove(key);
            Self::context_set(context)?;
            Ok(removed)
        } else {
            Ok(None)
        }
    }

    /// Clear all values from the current context
    pub fn clear_context() -> Result<()> {
        if let Some(mut context) = Self::context_get()? {
            context.clear();
            Self::context_set(context)?;
        }
        Ok(())
    }
}

/// Built-in function implementations for the canonical ABI
pub mod canonical_builtins {
    use super::*;

    /// `context.get` canonical built-in
    /// Returns the current async context as a component value
    pub fn canon_context_get() -> Result<WrtComponentValue<ComponentProvider>> {
        let context = AsyncContextManager::context_get()?;
        match context {
            Some(ctx) => {
                // Serialize context to component value
                // For now, return a simple boolean indicating presence
                Ok(WrtComponentValue::Bool(true))
            },
            None => Ok(WrtComponentValue::Bool(false)),
        }
    }

    /// `context.set` canonical built-in  
    /// Sets the current async context from a component value
    pub fn canon_context_set(value: WrtComponentValue<ComponentProvider>) -> Result<()> {
        match value {
            WrtComponentValue::Bool(true) => {
                // Create a new empty context
                let context = AsyncContext::new()?;
                AsyncContextManager::context_set(context)
            },
            WrtComponentValue::Bool(false) => {
                // Clear the current context
                AsyncContextManager::context_pop()?;
                Ok(())
            },
            _ => Err(Error::new(
                ErrorCategory::Type,
                wrt_error::codes::TYPE_MISMATCH,
                "Invalid context value type - expected boolean",
            )),
        }
    }

    /// Helper function to get a typed value from context
    pub fn get_typed_context_value<T>(key: &str, value_type: ValueType) -> Result<Option<T>>
    where
        T: TryFrom<WrtComponentValue<ComponentProvider>>,
        T::Error: Into<Error>,
    {
        #[cfg(feature = "std")]
        let context_key = ContextKey::new(key.to_string());
        #[cfg(not(any(feature = "std",)))]
        let context_key = ContextKey::new(key)?;

        if let Some(context_value) = AsyncContextManager::get_context_value(&context_key)? {
            if let Some(component_value) = context_value.as_component_value() {
                let typed_value = T::try_from(component_value.clone()).map_err(|e| e.into())?;
                Ok(Some(typed_value))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Helper function to set a typed value in context
    pub fn set_typed_context_value<T>(key: &str, value: T) -> Result<()>
    where
        T: Into<WrtComponentValue<ComponentProvider>>,
    {
        #[cfg(feature = "std")]
        let context_key = ContextKey::new(key.to_string());
        #[cfg(not(any(feature = "std",)))]
        let context_key = ContextKey::new(key)?;

        let component_value = value.into();
        let context_value = ContextValue::from_component_value(component_value);
        AsyncContextManager::set_context_value(context_key, context_value)
    }
}

/// Scope guard for automatic context management
pub struct AsyncContextScope {
    _marker: core::marker::PhantomData<()>,
}

impl AsyncContextScope {
    /// Enter a new async context scope
    pub fn enter(context: AsyncContext) -> Result<Self> {
        AsyncContextManager::context_push(context)?;
        Ok(Self {
            _marker: core::marker::PhantomData,
        })
    }

    /// Enter a new empty async context scope
    pub fn enter_empty() -> Result<Self> {
        Self::enter(AsyncContext::new()?)
    }
}

impl Drop for AsyncContextScope {
    fn drop(&mut self) {
        // Automatically pop context when scope ends
        let _ = AsyncContextManager::context_pop();
    }
}

/// Convenience macro for executing code within an async context scope
#[macro_export]
macro_rules! with_async_context {
    ($context:expr, $body:expr) => {{
        let _scope = $crate::async_context_builtins::AsyncContextScope::enter($context)?;
        $body
    }};
}
