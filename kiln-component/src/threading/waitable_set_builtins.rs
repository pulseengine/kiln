// WRT - wrt-component
// Module: Waitable Set Canonical Operations
// SW-REQ-ID: REQ_WAITABLE_SET_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Waitable Set Canonical Operations
//!
//! This module provides implementation of the `waitable-set.*` built-in
//! functions required by the WebAssembly Component Model for managing sets of
//! waitable objects.

extern crate alloc;

#[cfg(feature = "std")]
use std::{
    boxed::Box,
    cell::RefCell as AtomicRefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    vec::Vec,
};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    bounded::BoundedVec,
    bounded_collections::{BoundedMap, BoundedSet},
    budget_aware_provider::CrateId,
    component_value::ComponentValue,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

use crate::{
    async_::async_types::{Future, FutureHandle, Stream, StreamHandle, Waitable, WaitableSet},
    threading::task_builtins::{TaskId as TaskBuiltinId, TaskStatus},
};

// Constants for no_std environments
#[cfg(not(any(feature = "std",)))]
const MAX_WAITABLE_SETS: usize = 32;
#[cfg(not(any(feature = "std",)))]
const MAX_WAITABLES_PER_SET: usize = 64;
#[cfg(not(any(feature = "std",)))]
const MAX_WAIT_RESULTS: usize = 64;

/// Waitable set identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaitableSetId(pub u64);

impl WaitableSetId {
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for WaitableSetId {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a wait operation
#[derive(Debug, Clone, PartialEq)]
pub enum WaitResult {
    /// A waitable became ready
    Ready(WaitableEntry),
    /// Wait operation timed out
    Timeout,
    /// Wait operation was cancelled
    Cancelled,
    /// An error occurred during waiting
    Error(Error),
}

impl WaitResult {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn as_ready(&self) -> Option<&WaitableEntry> {
        match self {
            Self::Ready(entry) => Some(entry),
            _ => None,
        }
    }

    pub fn into_ready(self) -> Option<WaitableEntry> {
        match self {
            Self::Ready(entry) => Some(entry),
            _ => None,
        }
    }
}

/// Entry in a waitable set
#[derive(Debug, Clone, PartialEq)]
pub struct WaitableEntry {
    pub id: WaitableId,
    pub waitable: Waitable,
    pub ready: bool,
}

impl WaitableEntry {
    pub fn new(id: WaitableId, waitable: Waitable) -> Self {
        Self {
            id,
            waitable,
            ready: false,
        }
    }

    pub fn mark_ready(&mut self) {
        self.ready = true;
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn check_ready(&mut self) -> bool {
        self.ready = match &self.waitable {
            Waitable::Future(future) => {
                matches!(
                    future.state,
                    crate::async_::async_types::FutureState::Ready
                        | crate::async_::async_types::FutureState::Error
                )
            },
            Waitable::Stream(stream) => {
                match stream.state {
                    crate::async_::async_types::StreamState::Open => true, // Data available to
                    // read
                    crate::async_::async_types::StreamState::Closed => true, // EOF condition
                    _ => false,
                }
            },
            Waitable::WaitableSet(_) => {
                // Nested waitable sets are ready if any of their contents are ready
                // This would require recursive checking in a full implementation
                false
            },
        };
        self.ready
    }
}

/// Waitable identifier within a set
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaitableId(pub u64);

impl WaitableId {
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for WaitableId {
    fn default() -> Self {
        Self::new()
    }
}

/// A set of waitable objects that can be waited on collectively
#[derive(Debug, Clone)]
pub struct WaitableSetImpl {
    pub id: WaitableSetId,
    #[cfg(feature = "std")]
    pub waitables: BTreeMap<WaitableId, WaitableEntry>,
    #[cfg(not(any(feature = "std",)))]
    pub waitables: BoundedMap<WaitableId, WaitableEntry, MAX_WAITABLES_PER_SET>,
    pub closed: bool,
}

impl WaitableSetImpl {
    pub fn new() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            Ok(Self {
                id: WaitableSetId::new(),
                waitables: BTreeMap::new(),
                closed: false,
            })
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            Ok(Self {
                id: WaitableSetId::new(),
                waitables: BoundedMap::new(),
                closed: false,
            })
        }
    }

    pub fn add_waitable(&mut self, waitable: Waitable) -> Result<WaitableId> {
        if self.closed {
            return Err(Error::runtime_execution_error("Error occurred"));
        }

        let id = WaitableId::new();
        let entry = WaitableEntry::new(id, waitable);

        #[cfg(feature = "std")]
        {
            self.waitables.insert(id, entry);
            Ok(id)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.waitables
                .insert(id, entry)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            Ok(id)
        }
    }

    pub fn remove_waitable(&mut self, id: WaitableId) -> Option<WaitableEntry> {
        self.waitables.remove(&id)
    }

    pub fn contains_waitable(&self, id: WaitableId) -> bool {
        self.waitables.contains_key(&id)
    }

    pub fn waitable_count(&self) -> usize {
        self.waitables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.waitables.is_empty()
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Check all waitables and return those that are ready
    #[cfg(feature = "std")]
    pub fn check_ready(&mut self) -> Vec<WaitableEntry> {
        let mut ready = Vec::new();
        for (_, entry) in self.waitables.iter_mut() {
            if entry.check_ready() {
                ready.push(entry.clone());
            }
        }
        ready
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn check_ready(&mut self) -> Result<BoundedVec<WaitableEntry, MAX_WAIT_RESULTS>> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut ready =
            BoundedVec::new().map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        for (_, entry) in self.waitables.iter_mut() {
            if entry.check_ready() {
                ready
                    .push(entry.clone())
                    .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            }
        }
        Ok(ready)
    }

    /// Get the first ready waitable if any
    pub fn get_first_ready(&mut self) -> Option<WaitableEntry> {
        for (_, entry) in self.waitables.iter_mut() {
            if entry.check_ready() {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Wait for any waitable to become ready (non-blocking check)
    pub fn poll(&mut self) -> Option<WaitResult> {
        if let Some(ready_entry) = self.get_first_ready() {
            Some(WaitResult::Ready(ready_entry))
        } else {
            None
        }
    }
}

impl Default for WaitableSetImpl {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to create WaitableSetImpl"))
    }
}

/// Global registry for waitable sets
static WAITABLE_SET_REGISTRY: AtomicRefCell<Option<WaitableSetRegistry>> = AtomicRefCell::new(None);

/// Registry that manages all waitable sets
#[derive(Debug)]
pub struct WaitableSetRegistry {
    #[cfg(feature = "std")]
    sets: HashMap<WaitableSetId, WaitableSetImpl>,
    #[cfg(not(any(feature = "std",)))]
    sets: BoundedMap<WaitableSetId, WaitableSetImpl, MAX_WAITABLE_SETS>,
}

impl WaitableSetRegistry {
    pub fn new() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            Ok(Self {
                sets: HashMap::new(),
            })
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let provider = safe_managed_alloc!(65536, CrateId::Component)?;
            Ok(Self {
                sets: BoundedMap::new(),
            })
        }
    }

    pub fn register_set(&mut self, set: WaitableSetImpl) -> Result<WaitableSetId> {
        let id = set.id;
        #[cfg(feature = "std")]
        {
            self.sets.insert(id, set);
            Ok(id)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.sets
                .insert(id, set)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            Ok(id)
        }
    }

    pub fn get_set(&self, id: WaitableSetId) -> Option<&WaitableSetImpl> {
        self.sets.get(&id)
    }

    pub fn get_set_mut(&mut self, id: WaitableSetId) -> Option<&mut WaitableSetImpl> {
        self.sets.get_mut(&id)
    }

    pub fn remove_set(&mut self, id: WaitableSetId) -> Option<WaitableSetImpl> {
        self.sets.remove(&id)
    }

    pub fn set_count(&self) -> usize {
        self.sets.len()
    }
}

impl Default for WaitableSetRegistry {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to create WaitableSetRegistry"))
    }
}

/// Waitable set built-ins providing canonical functions
pub struct WaitableSetBuiltins;

impl WaitableSetBuiltins {
    /// Initialize the global waitable set registry
    pub fn initialize() -> Result<()> {
        let mut registry_ref = WAITABLE_SET_REGISTRY.try_borrow_mut().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        *registry_ref = Some(WaitableSetRegistry::new()?);
        Ok(())
    }

    /// Get the global registry
    fn with_registry<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&WaitableSetRegistry) -> R,
    {
        let registry_ref = WAITABLE_SET_REGISTRY
            .try_borrow()
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        let registry = registry_ref.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        Ok(f(registry))
    }

    /// Get the global registry mutably
    fn with_registry_mut<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut WaitableSetRegistry) -> Result<R>,
    {
        let mut registry_ref = WAITABLE_SET_REGISTRY
            .try_borrow_mut()
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        let registry = registry_ref.as_mut().ok_or_else(|| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        f(registry)
    }

    /// `waitable-set.new` canonical built-in
    /// Creates a new waitable set
    pub fn waitable_set_new() -> Result<WaitableSetId> {
        let set = WaitableSetImpl::new()?;
        Self::with_registry_mut(|registry| registry.register_set(set))?
    }

    /// `waitable-set.add` canonical built-in
    /// Adds a waitable to a set
    pub fn waitable_set_add(set_id: WaitableSetId, waitable: Waitable) -> Result<WaitableId> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                set.add_waitable(waitable)
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// `waitable-set.remove` canonical built-in
    /// Removes a waitable from a set
    pub fn waitable_set_remove(set_id: WaitableSetId, waitable_id: WaitableId) -> Result<bool> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                Ok(set.remove_waitable(waitable_id).is_some())
            } else {
                Err(Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::RESOURCE_INVALID_HANDLE,
                    "Error message needed",
                ))
            }
        })
    }

    /// `waitable-set.wait` canonical built-in
    /// Waits for any waitable in the set to become ready
    pub fn waitable_set_wait(set_id: WaitableSetId) -> Result<WaitResult> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                Ok(set.poll().unwrap_or(WaitResult::Timeout))
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// Check if a waitable set contains a specific waitable
    pub fn waitable_set_contains(set_id: WaitableSetId, waitable_id: WaitableId) -> Result<bool> {
        Self::with_registry(|registry| {
            if let Some(set) = registry.get_set(set_id) {
                set.contains_waitable(waitable_id)
            } else {
                false
            }
        })
    }

    /// Get the number of waitables in a set
    pub fn waitable_set_count(set_id: WaitableSetId) -> Result<usize> {
        Self::with_registry(|registry| {
            if let Some(set) = registry.get_set(set_id) { set.waitable_count() } else { 0 }
        })
    }

    /// Close a waitable set (no more waitables can be added)
    pub fn waitable_set_close(set_id: WaitableSetId) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                set.close();
                Ok(())
            } else {
                Err(Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::RESOURCE_INVALID_HANDLE,
                    "Error message needed",
                ))
            }
        })?
    }

    /// Remove a waitable set from the registry
    pub fn waitable_set_drop(set_id: WaitableSetId) -> Result<()> {
        Self::with_registry_mut(|registry| {
            registry.remove_set(set_id);
            Ok(())
        })
    }

    /// Get all ready waitables from a set
    #[cfg(feature = "std")]
    pub fn waitable_set_poll_all(set_id: WaitableSetId) -> Result<Vec<WaitableEntry>> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                Ok(set.check_ready())
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    #[cfg(not(any(feature = "std")))]
    pub fn waitable_set_poll_all(
        set_id: WaitableSetId,
    ) -> core::result::Result<BoundedVec<WaitableEntry, MAX_WAIT_RESULTS>, wrt_error::Error> {
        Self::with_registry_mut(|registry| {
            if let Some(set) = registry.get_set_mut(set_id) {
                set.check_ready()
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }
}

/// Convenience functions for working with waitable sets
pub mod waitable_set_helpers {
    use super::*;

    /// Create a waitable set with initial waitables
    #[cfg(feature = "std")]
    pub fn create_waitable_set_with(waitables: Vec<Waitable>) -> Result<WaitableSetId> {
        let set_id = WaitableSetBuiltins::waitable_set_new()?;
        for waitable in waitables {
            WaitableSetBuiltins::waitable_set_add(set_id, waitable)?;
        }
        Ok(set_id)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn create_waitable_set_with(waitables: &[Waitable]) -> Result<WaitableSetId> {
        let set_id = WaitableSetBuiltins::waitable_set_new()?;
        for waitable in waitables {
            WaitableSetBuiltins::waitable_set_add(set_id, waitable.clone())?;
        }
        Ok(set_id)
    }

    /// Wait for any of multiple futures to complete
    #[cfg(feature = "std")]
    pub fn wait_for_any_future(futures: Vec<Future>) -> Result<WaitResult> {
        let waitables: Vec<Waitable> = futures.into_iter().map(Waitable::Future).collect();
        let set_id = create_waitable_set_with(waitables)?;
        WaitableSetBuiltins::waitable_set_wait(set_id)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn wait_for_any_future(futures: &[Future]) -> Result<WaitResult> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut waitables = BoundedVec::new().unwrap();
        for future in futures {
            waitables
                .push(Waitable::Future(future.clone()))
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        }
        let set_id = create_waitable_set_with(waitables.as_slice())?;
        WaitableSetBuiltins::waitable_set_wait(set_id)
    }

    /// Wait for any of multiple streams to have data available
    #[cfg(feature = "std")]
    pub fn wait_for_any_stream(streams: Vec<Stream>) -> Result<WaitResult> {
        let waitables: Vec<Waitable> = streams.into_iter().map(Waitable::Stream).collect();
        let set_id = create_waitable_set_with(waitables)?;
        WaitableSetBuiltins::waitable_set_wait(set_id)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn wait_for_any_stream(streams: &[Stream]) -> Result<WaitResult> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut waitables = BoundedVec::new().unwrap();
        for stream in streams {
            waitables
                .push(Waitable::Stream(stream.clone()))
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        }
        let set_id = create_waitable_set_with(waitables.as_slice())?;
        WaitableSetBuiltins::waitable_set_wait(set_id)
    }

    /// Create a waitable from a future handle
    pub fn waitable_from_future_handle(handle: FutureHandle) -> Waitable {
        Waitable::Future(Future {
            handle,
            state: crate::async_::async_types::FutureState::Pending,
        })
    }

    /// Create a waitable from a stream handle
    pub fn waitable_from_stream_handle(handle: StreamHandle) -> Waitable {
        Waitable::Stream(Stream {
            handle,
            state: crate::async_::async_types::StreamState::Open,
        })
    }
}
