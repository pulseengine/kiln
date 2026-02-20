// WRT - wrt-component
// Module: Advanced Threading Built-ins
// SW-REQ-ID: REQ_ADVANCED_THREADING_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Advanced Threading Built-ins
//!
//! This module provides implementation of advanced threading functions for the
//! WebAssembly Component Model, including `thread.spawn_ref`,
//! `thread.spawn_indirect`, and `thread.join` operations.

extern crate alloc;

use core::cell::RefCell as AtomicRefCell;
#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result};
#[cfg(not(feature = "std"))]
use wrt_foundation::BoundedString;
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    BoundedMap as HashMap, BoundedVec as Vec, budget_aware_provider::CrateId, safe_managed_alloc,
};
use wrt_foundation::{bounded::BoundedVec, bounded_collections::BoundedMap, types::ValueType};

// Type aliases for no_std compatibility
#[cfg(not(feature = "std"))]
type ThreadingString = BoundedString<256>;

use crate::threading::{
    task_cancellation::{CancellationToken, with_cancellation_scope},
    thread_builtins::{
        ComponentFunction, FunctionSignature, ParallelismInfo, ThreadBuiltins, ThreadError,
        ThreadJoinResult, ThreadSpawnConfig, ValueType as ThreadValueType,
    },
};
#[cfg(not(feature = "std"))]
// For no_std, use a simpler ComponentValue representation
use crate::types::Value as ComponentValue;

// Constants for no_std environments
#[cfg(not(any(feature = "std",)))]
const MAX_THREADS: usize = 32;
#[cfg(not(any(feature = "std",)))]
const MAX_THREAD_LOCALS: usize = 16;
#[cfg(not(any(feature = "std",)))]
const MAX_FUNCTION_NAME_SIZE: usize = 128;

/// Thread identifier for advanced threading operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdvancedThreadId(pub u64);

impl AdvancedThreadId {
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for AdvancedThreadId {
    fn default() -> Self {
        Self::new()
    }
}

/// Function reference for thread.spawn_ref
#[derive(Debug, Clone)]
pub struct FunctionReference {
    #[cfg(feature = "std")]
    pub name: String,
    #[cfg(not(any(feature = "std",)))]
    pub name: BoundedString<MAX_FUNCTION_NAME_SIZE>,

    pub signature: FunctionSignature,
    pub module_index: u32,
    pub function_index: u32,
}

impl FunctionReference {
    #[cfg(feature = "std")]
    pub fn new(
        name: String,
        signature: FunctionSignature,
        module_index: u32,
        function_index: u32,
    ) -> Self {
        Self {
            name,
            signature,
            module_index,
            function_index,
        }
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn new(
        name: &str,
        signature: FunctionSignature,
        module_index: u32,
        function_index: u32,
    ) -> Result<Self> {
        let bounded_name = BoundedString::try_from_str(name)
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        Ok(Self {
            name: bounded_name,
            signature,
            module_index,
            function_index,
        })
    }

    pub fn name(&self) -> &str {
        #[cfg(feature = "std")]
        return &self.name;
        #[cfg(not(any(feature = "std",)))]
        return self.name.as_str();
    }
}

/// Indirect function call descriptor for thread.spawn_indirect
#[derive(Debug, Clone)]
pub struct IndirectCall {
    pub table_index: u32,
    pub function_index: u32,
    pub type_index: u32,
    #[cfg(feature = "std")]
    pub arguments: Vec<ComponentValue>,
    #[cfg(not(any(feature = "std",)))]
    pub arguments: BoundedVec<ComponentValue<ComponentProvider>, 16>,
}

impl IndirectCall {
    #[cfg(feature = "std")]
    pub fn new(
        table_index: u32,
        function_index: u32,
        type_index: u32,
        arguments: Vec<ComponentValue>,
    ) -> Self {
        Self {
            table_index,
            function_index,
            type_index,
            arguments,
        }
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn new(
        table_index: u32,
        function_index: u32,
        type_index: u32,
        arguments: &[ComponentValue],
    ) -> Result<Self> {
        let bounded_args = BoundedVec::new_from_slice(arguments)
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        Ok(Self {
            table_index,
            function_index,
            type_index,
            arguments: bounded_args,
        })
    }

    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn get_argument(&self, index: usize) -> Option<&ComponentValue> {
        self.arguments.get(index)
    }
}

/// Thread execution state for advanced threading
#[derive(Debug, Clone, PartialEq)]
pub enum AdvancedThreadState {
    /// Thread is starting up
    Starting,
    /// Thread is running
    Running,
    /// Thread completed successfully
    Completed,
    /// Thread was cancelled
    Cancelled,
    /// Thread failed with an error
    Failed,
    /// Thread is being joined
    Joining,
}

impl AdvancedThreadState {
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }

    pub fn can_join(&self) -> bool {
        self.is_finished()
    }
}

/// Thread local storage entry
#[derive(Debug, Clone)]
pub struct ThreadLocalEntry {
    pub key: u32,
    pub value: ComponentValue,
    pub destructor: Option<u32>, // Function index for destructor
}

/// Advanced thread context
#[derive(Debug, Clone)]
pub struct AdvancedThread {
    pub id: AdvancedThreadId,
    pub state: AdvancedThreadState,
    pub config: ThreadSpawnConfig,
    pub cancellation_token: CancellationToken,

    #[cfg(feature = "std")]
    pub thread_locals: HashMap<u32, ThreadLocalEntry>,
    #[cfg(not(any(feature = "std",)))]
    pub thread_locals: BoundedMap<u32, ThreadLocalEntry, MAX_THREAD_LOCALS>,

    pub result: Option<ComponentValue>,
    pub error: Option<ThreadError>,
    pub parent_thread: Option<AdvancedThreadId>,

    #[cfg(feature = "std")]
    pub child_threads: Vec<AdvancedThreadId>,
    #[cfg(not(any(feature = "std",)))]
    pub child_threads: BoundedVec<AdvancedThreadId, MAX_THREADS>,
}

impl AdvancedThread {
    pub fn new(config: ThreadSpawnConfig) -> Result<Self> {
        Ok(Self {
            id: AdvancedThreadId::new(),
            state: AdvancedThreadState::Starting,
            config,
            cancellation_token: CancellationToken::new(),
            #[cfg(feature = "std")]
            thread_locals: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            thread_locals: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedMap::new()
            },
            result: None,
            error: None,
            parent_thread: None,
            #[cfg(feature = "std")]
            child_threads: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            child_threads: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
        })
    }

    pub fn with_parent(config: ThreadSpawnConfig, parent_id: AdvancedThreadId) -> Result<Self> {
        let mut thread = Self::new(config)?;
        thread.parent_thread = Some(parent_id);
        Ok(thread)
    }

    #[cfg(feature = "std")]
    pub fn add_child(&mut self, child_id: AdvancedThreadId) {
        self.child_threads.push(child_id);
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn add_child(&mut self, child_id: AdvancedThreadId) -> Result<()> {
        self.child_threads
            .push(child_id)
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        Ok(())
    }

    pub fn start(&mut self) {
        if self.state == AdvancedThreadState::Starting {
            self.state = AdvancedThreadState::Running;
        }
    }

    pub fn complete(&mut self, result: ComponentValue) {
        if self.state == AdvancedThreadState::Running {
            self.state = AdvancedThreadState::Completed;
            self.result = Some(result);
        }
    }

    pub fn fail(&mut self, error: ThreadError) {
        if self.state.is_active() {
            self.state = AdvancedThreadState::Failed;
            self.error = Some(error);
        }
    }

    pub fn cancel(&mut self) {
        if self.state.is_active() {
            self.state = AdvancedThreadState::Cancelled;
            self.cancellation_token.cancel();
        }
    }

    pub fn set_thread_local(
        &mut self,
        key: u32,
        value: ComponentValue,
        destructor: Option<u32>,
    ) -> Result<()> {
        let entry = ThreadLocalEntry {
            key,
            value,
            destructor,
        };

        #[cfg(feature = "std")]
        {
            self.thread_locals.insert(key, entry);
            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.thread_locals
                .insert(key, entry)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            Ok(())
        }
    }

    pub fn get_thread_local(&self, key: u32) -> Option<&ThreadLocalEntry> {
        self.thread_locals.get(&key)
    }

    pub fn remove_thread_local(&mut self, key: u32) -> Option<ThreadLocalEntry> {
        self.thread_locals.remove(&key)
    }

    pub fn child_count(&self) -> usize {
        self.child_threads.len()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

/// Global registry for advanced threads
static ADVANCED_THREAD_REGISTRY: AtomicRefCell<Option<AdvancedThreadRegistry>> =
    AtomicRefCell::new(None);

/// Registry for managing advanced threading operations
#[derive(Debug)]
pub struct AdvancedThreadRegistry {
    #[cfg(feature = "std")]
    threads: HashMap<AdvancedThreadId, AdvancedThread>,
    #[cfg(not(any(feature = "std",)))]
    threads: BoundedMap<AdvancedThreadId, AdvancedThread, MAX_THREADS>,
}

impl AdvancedThreadRegistry {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            threads: HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            threads: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedMap::new()
            },
        }
    }

    pub fn register_thread(&mut self, thread: AdvancedThread) -> Result<AdvancedThreadId> {
        let id = thread.id;
        #[cfg(feature = "std")]
        {
            self.threads.insert(id, thread);
            Ok(id)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.threads
                .insert(id, thread)
                .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
            Ok(id)
        }
    }

    pub fn get_thread(&self, id: AdvancedThreadId) -> Option<&AdvancedThread> {
        self.threads.get(&id)
    }

    pub fn get_thread_mut(&mut self, id: AdvancedThreadId) -> Option<&mut AdvancedThread> {
        self.threads.get_mut(&id)
    }

    pub fn remove_thread(&mut self, id: AdvancedThreadId) -> Option<AdvancedThread> {
        self.threads.remove(&id)
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub fn cleanup_finished_threads(&mut self) {
        #[cfg(feature = "std")]
        {
            self.threads.retain(|_, thread| !thread.state.is_finished());
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let provider = safe_managed_alloc!(
                MAX_THREADS * core::mem::size_of::<AdvancedThreadId>(),
                CrateId::Component
            )
            .unwrap();
            let mut finished_ids =
                BoundedVec::<AdvancedThreadId, MAX_THREADS>::new(provider).unwrap();
            for (id, thread) in self.threads.iter() {
                if thread.state.is_finished() {
                    let _ = finished_ids.push(*id);
                }
            }
            for id in finished_ids.iter() {
                self.threads.remove(id);
            }
        }
    }
}

impl Default for AdvancedThreadRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Advanced threading built-ins manager
pub struct AdvancedThreadingBuiltins;

impl AdvancedThreadingBuiltins {
    /// Initialize the global advanced thread registry
    pub fn initialize() -> Result<()> {
        let mut registry_ref = ADVANCED_THREAD_REGISTRY
            .try_borrow_mut()
            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
        *registry_ref = Some(AdvancedThreadRegistry::new());
        Ok(())
    }

    /// Get the global registry
    fn with_registry<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&AdvancedThreadRegistry) -> R,
    {
        let registry_ref = ADVANCED_THREAD_REGISTRY.try_borrow().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        let registry = registry_ref
            .as_ref()
            .ok_or_else(|| Error::runtime_execution_error("Error occurred"))?;
        Ok(f(registry))
    }

    /// Get the global registry mutably
    fn with_registry_mut<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut AdvancedThreadRegistry) -> Result<R>,
    {
        let mut registry_ref = ADVANCED_THREAD_REGISTRY.try_borrow_mut().map_err(|_| {
            Error::new(
                ErrorCategory::Runtime,
                wrt_error::codes::INVALID_STATE,
                "Error message needed",
            )
        })?;
        let registry = registry_ref
            .as_mut()
            .ok_or_else(|| Error::runtime_execution_error("Error occurred"))?;
        f(registry)
    }

    /// `thread.spawn_ref` canonical built-in
    /// Spawns a thread using a function reference
    pub fn thread_spawn_ref(
        func_ref: FunctionReference,
        config: ThreadSpawnConfig,
        parent_id: Option<AdvancedThreadId>,
    ) -> Result<AdvancedThreadId> {
        let thread = if let Some(parent) = parent_id {
            AdvancedThread::with_parent(config, parent)?
        } else {
            AdvancedThread::new(config)?
        };

        let thread_id = thread.id;

        Self::with_registry_mut(|registry| {
            let id = registry.register_thread(thread)?;

            // Start the thread
            if let Some(thread) = registry.get_thread_mut(id) {
                thread.start();
            }

            // Add to parent's child list if applicable
            if let Some(parent) = parent_id {
                if let Some(parent_thread) = registry.get_thread_mut(parent) {
                    #[cfg(feature = "std")]
                    parent_thread.add_child(id);
                    #[cfg(not(any(feature = "std",)))]
                    parent_thread.add_child(id)?;
                }
            }

            Ok(id)
        })
    }

    /// `thread.spawn_indirect` canonical built-in
    /// Spawns a thread using an indirect function call
    pub fn thread_spawn_indirect(
        indirect_call: IndirectCall,
        config: ThreadSpawnConfig,
        parent_id: Option<AdvancedThreadId>,
    ) -> Result<AdvancedThreadId> {
        let thread = if let Some(parent) = parent_id {
            AdvancedThread::with_parent(config, parent)?
        } else {
            AdvancedThread::new(config)?
        };

        let thread_id = thread.id;

        Self::with_registry_mut(|registry| {
            let id = registry.register_thread(thread)?;

            // Start the thread
            if let Some(thread) = registry.get_thread_mut(id) {
                thread.start();
            }

            // Add to parent's child list if applicable
            if let Some(parent) = parent_id {
                if let Some(parent_thread) = registry.get_thread_mut(parent) {
                    #[cfg(feature = "std")]
                    parent_thread.add_child(id);
                    #[cfg(not(any(feature = "std",)))]
                    parent_thread.add_child(id)?;
                }
            }

            Ok(id)
        })
    }

    /// `thread.join` canonical built-in
    /// Waits for a thread to complete and returns its result
    pub fn thread_join(thread_id: AdvancedThreadId) -> Result<ThreadJoinResult> {
        Self::with_registry_mut(|registry| {
            if let Some(thread) = registry.get_thread_mut(thread_id) {
                if !thread.state.can_join() {
                    return Ok(ThreadJoinResult::NotReady);
                }

                match thread.state {
                    AdvancedThreadState::Completed => {
                        if let Some(result) = thread.result.take() {
                            Ok(ThreadJoinResult::Success(result))
                        } else {
                            Ok(ThreadJoinResult::Success(ComponentValue::I32(0)))
                            // Default success
                        }
                    },
                    AdvancedThreadState::Failed => {
                        if let Some(error) = thread.error.take() {
                            Ok(ThreadJoinResult::Error(error))
                        } else {
                            Ok(ThreadJoinResult::Error(ThreadError::ExecutionFailed))
                        }
                    },
                    AdvancedThreadState::Cancelled => Ok(ThreadJoinResult::Cancelled),
                    _ => Ok(ThreadJoinResult::NotReady),
                }
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// Get thread state
    pub fn thread_state(thread_id: AdvancedThreadId) -> Result<AdvancedThreadState> {
        Self::with_registry(|registry| {
            if let Some(thread) = registry.get_thread(thread_id) {
                thread.state.clone()
            } else {
                AdvancedThreadState::Failed
            }
        })
    }

    /// Cancel a thread
    pub fn thread_cancel(thread_id: AdvancedThreadId) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(thread) = registry.get_thread_mut(thread_id) {
                thread.cancel();
                Ok(())
            } else {
                Err(Error::new(
                    ErrorCategory::Runtime,
                    wrt_error::codes::RESOURCE_INVALID_HANDLE,
                    "Error message needed",
                ))
            }
        })
    }

    /// Set thread-local value
    pub fn thread_local_set(
        thread_id: AdvancedThreadId,
        key: u32,
        value: ComponentValue,
        destructor: Option<u32>,
    ) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(thread) = registry.get_thread_mut(thread_id) {
                thread.set_thread_local(key, value, destructor)
            } else {
                Err(Error::runtime_execution_error("Error occurred"))
            }
        })
    }

    /// Get thread-local value
    pub fn thread_local_get(
        thread_id: AdvancedThreadId,
        key: u32,
    ) -> Result<Option<ComponentValue>> {
        Self::with_registry(|registry| {
            if let Some(thread) = registry.get_thread(thread_id) {
                thread.get_thread_local(key).map(|entry| entry.value.clone())
            } else {
                None
            }
        })
    }

    /// Get thread parallelism info
    pub fn thread_parallelism_info() -> Result<ParallelismInfo> {
        // Delegate to basic thread builtins
        ThreadBuiltins::available_parallelism()
    }

    /// Cleanup finished threads
    pub fn cleanup_finished_threads() -> Result<()> {
        Self::with_registry_mut(|registry| {
            registry.cleanup_finished_threads();
            Ok(())
        })
    }

    /// Get thread count
    pub fn thread_count() -> Result<usize> {
        Self::with_registry(|registry| registry.thread_count())
    }
}

/// Helper functions for advanced threading
pub mod advanced_threading_helpers {
    use super::*;

    /// Spawn a thread with function reference and wait for completion
    pub fn spawn_ref_and_join(
        func_ref: FunctionReference,
        config: ThreadSpawnConfig,
    ) -> Result<ThreadJoinResult> {
        let thread_id = AdvancedThreadingBuiltins::thread_spawn_ref(func_ref, config, None)?;

        // In a real implementation, this would block until completion
        // For demonstration, we simulate immediate completion
        AdvancedThreadingBuiltins::thread_join(thread_id)
    }

    /// Spawn a thread with indirect call and wait for completion
    pub fn spawn_indirect_and_join(
        indirect_call: IndirectCall,
        config: ThreadSpawnConfig,
    ) -> Result<ThreadJoinResult> {
        let thread_id =
            AdvancedThreadingBuiltins::thread_spawn_indirect(indirect_call, config, None)?;

        // In a real implementation, this would block until completion
        // For demonstration, we simulate immediate completion
        AdvancedThreadingBuiltins::thread_join(thread_id)
    }

    /// Cancel all child threads of a parent
    #[cfg(feature = "std")]
    pub fn cancel_child_threads(parent_id: AdvancedThreadId) -> Result<Vec<AdvancedThreadId>> {
        let mut cancelled = Vec::new();

        AdvancedThreadingBuiltins::with_registry_mut(|registry| {
            if let Some(parent) = registry.get_thread(parent_id) {
                for &child_id in &parent.child_threads {
                    if let Some(child) = registry.get_thread_mut(child_id) {
                        child.cancel();
                        cancelled.push(child_id);
                    }
                }
            }
            Ok(())
        })?;

        Ok(cancelled)
    }

    #[cfg(not(any(feature = "std",)))]
    pub fn cancel_child_threads(
        parent_id: AdvancedThreadId,
    ) -> Result<BoundedVec<AdvancedThreadId, MAX_THREADS>> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut cancelled = BoundedVec::new().unwrap();

        AdvancedThreadingBuiltins::with_registry_mut(|registry| {
            if let Some(parent) = registry.get_thread(parent_id) {
                for &child_id in parent.child_threads.iter() {
                    if let Some(child) = registry.get_thread_mut(child_id) {
                        child.cancel();
                        cancelled
                            .push(child_id)
                            .map_err(|_| Error::runtime_execution_error("Error occurred"))?;
                    }
                }
            }
            Ok(())
        })?;

        Ok(cancelled)
    }

    /// Execute a function within a cancellation scope
    pub fn with_cancellation<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(CancellationToken) -> Result<R>,
    {
        let token = CancellationToken::new();
        with_cancellation_scope(token.clone(), || f(token))
    }
}
