//! Async Canonical ABI implementation for WebAssembly Component Model
//!
//! This module implements async lifting and lowering operations for the
//! Component Model's canonical ABI, enabling asynchronous component calls
//! with streams, futures, and error contexts.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::BTreeMap, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

// Enable vec! macro for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::collections::StaticVec as BoundedVec;
#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
#[cfg(not(feature = "std"))]
use wrt_foundation::{BoundedMap as BTreeMap, budget_aware_provider::CrateId, safe_managed_alloc};
use wrt_runtime::{Checksummable, FromBytes, ToBytes};

use crate::{
    async_::async_types::{
        AsyncReadResult, ErrorContext, ErrorContextHandle, Future, FutureHandle, FutureState,
        Stream, StreamHandle, StreamState, Waitable, WaitableSet, WaitableSetHandle,
    },
    prelude::{ResourceHandle, *},
    types::{ValType, Value},
};

// Temporary stubs for missing types
#[derive(Debug, Clone, Default)]
pub struct CanonicalAbi;

impl CanonicalAbi {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Default)]
pub struct CanonicalOptions;

#[derive(Debug, Clone)]
pub struct CanonicalLiftContext {
    pub options: CanonicalOptions,
}

impl Default for CanonicalLiftContext {
    fn default() -> Self {
        Self {
            options: CanonicalOptions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalLowerContext {
    pub options: CanonicalOptions,
}

impl Default for CanonicalLowerContext {
    fn default() -> Self {
        Self {
            options: CanonicalOptions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskManager;

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self
    }

    pub fn task_yield(&mut self) -> Result<()> {
        // Placeholder implementation - yield current task execution
        Err(Error::runtime_execution_error("No active task to yield"))
    }

    pub fn task_wait(&mut self, waitables: WaitableSet) -> Result<u32> {
        // Placeholder implementation - wait for waitable resources
        Err(Error::runtime_execution_error("Task wait not implemented"))
    }

    pub fn task_return(&mut self, values: ComponentVec<Value>) -> Result<()> {
        // Placeholder implementation - return values from task
        Ok(())
    }

    pub fn task_poll(&self, waitables: &WaitableSet) -> Result<Option<u32>> {
        // Placeholder implementation - poll waitable resources
        Ok(None)
    }

    pub fn task_cancel(&mut self, task_id: TaskId) -> Result<()> {
        // Placeholder implementation - cancel task
        Ok(())
    }

    pub fn task_backpressure(&mut self) -> Result<()> {
        // Placeholder implementation - apply backpressure to current task
        Err(Error::runtime_execution_error(
            "No active task for backpressure",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub u32);

impl TaskId {
    /// Create a new task identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Component,
    Async,
}

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task was cancelled
    Cancelled,
    /// Task failed with an error
    Failed,
}

// Stub module for missing async_canonical_lifting functions
pub mod async_canonical_lifting {
    use super::*;

    pub fn async_canonical_lift(
        _values: &[u8],
        _target_types: &[ValType],
        _options: &CanonicalOptions,
    ) -> Result<ComponentVec<Value>> {
        #[cfg(feature = "std")]
        {
            Ok(vec![])
        }
        #[cfg(not(feature = "std"))]
        {
            Ok(ComponentVec::new())
        }
    }

    pub fn async_canonical_lower(
        _values: &[Value],
        _options: &CanonicalOptions,
    ) -> Result<ComponentVec<u8>> {
        #[cfg(feature = "std")]
        {
            Ok(vec![])
        }
        #[cfg(not(feature = "std"))]
        {
            Ok(ComponentVec::new())
        }
    }
}

/// Maximum number of streams/futures in no_std environments
const MAX_ASYNC_RESOURCES: usize = 256;

/// Maximum number of async operations in flight for no_std environments
const MAX_ASYNC_OPS: usize = 256;

/// Maximum size for async call contexts in no_std environments
const MAX_ASYNC_CONTEXT_SIZE: usize = 64;

/// Async operation tracking
#[derive(Debug, Clone)]
pub struct AsyncOperation {
    /// Operation ID
    pub id: u32,
    /// Operation type
    pub op_type: AsyncOperationType,
    /// Current state
    pub state: AsyncOperationState,
    /// Associated context
    #[cfg(feature = "std")]
    pub context: ComponentVec<u8>,
    #[cfg(not(any(feature = "std",)))]
    pub context: BoundedVec<u8, 4096>,
    /// Task handle for cancellation
    pub task_handle: Option<u32>,
}

/// Type of async operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncOperationType {
    /// Async call to a component function
    AsyncCall,
    /// Stream read operation
    StreamRead,
    /// Stream write operation
    StreamWrite,
    /// Future get operation
    FutureGet,
    /// Future set operation
    FutureSet,
    /// Waitable poll operation
    WaitablePoll,
}

/// State of an async operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncOperationState {
    /// Operation is starting
    Starting,
    /// Operation is in progress
    InProgress,
    /// Operation is waiting for resources
    Waiting,
    /// Operation has completed successfully
    Completed,
    /// Operation was cancelled
    Cancelled,
    /// Operation failed with error
    Failed,
}

/// Results of async lifting operations
#[derive(Debug, Clone)]
pub enum AsyncLiftResult {
    /// Values are immediately available
    Immediate(ComponentVec<Value>),
    /// Operation needs to wait for async completion
    Pending(AsyncOperation),
    /// Stream for incremental reading
    Stream(StreamHandle),
    /// Future for deferred value
    Future(FutureHandle),
    /// Error occurred during lifting
    Error(ErrorContextHandle),
}

/// Results of async lowering operations
#[derive(Debug, Clone)]
pub enum AsyncLowerResult {
    /// Values were immediately lowered
    Immediate(ComponentVec<u8>),
    /// Operation needs async completion
    Pending(AsyncOperation),
    /// Stream for incremental writing
    Stream(StreamHandle),
    /// Future for deferred lowering
    Future(FutureHandle),
    /// Error occurred during lowering
    Error(ErrorContextHandle),
}

/// Async canonical ABI implementation
pub struct AsyncCanonicalAbi {
    /// Base canonical ABI
    canonical_abi: CanonicalAbi,

    /// Task manager
    task_manager: TaskManager,

    /// Stream registry
    #[cfg(feature = "std")]
    streams: BTreeMap<StreamHandle, Box<dyn StreamValue>>,
    #[cfg(not(any(feature = "std",)))]
    streams: BoundedVec<(StreamHandle, StreamValueEnum), 64>,

    /// Future registry
    #[cfg(feature = "std")]
    futures: BTreeMap<FutureHandle, Box<dyn FutureValue>>,
    #[cfg(not(any(feature = "std",)))]
    futures: BoundedVec<(FutureHandle, FutureValueEnum), 64>,

    /// Error context registry
    #[cfg(feature = "std")]
    error_contexts: BTreeMap<ErrorContextHandle, ErrorContext>,
    #[cfg(not(any(feature = "std",)))]
    error_contexts: BoundedVec<(ErrorContextHandle, ErrorContext), 32>,

    /// Waitable set registry
    #[cfg(feature = "std")]
    waitable_sets: BTreeMap<WaitableSetHandle, WaitableSet>,
    #[cfg(not(any(feature = "std",)))]
    waitable_sets: BoundedVec<(WaitableSetHandle, WaitableSet), 32>,

    /// Thread/task context storage (per-component implicit parameters)
    /// Keys are context slot indices, values are the stored context values
    #[cfg(feature = "std")]
    context_slots: BTreeMap<u32, Value>,
    #[cfg(not(any(feature = "std",)))]
    context_slots: BoundedVec<(u32, Value), 64>,

    /// Backpressure state for the current component
    /// When true, the component is signaling it cannot accept more work
    backpressure_enabled: bool,

    /// Backpressure refcount - incremented by callers, decremented when work completes
    backpressure_count: u32,

    /// Next handle IDs
    next_stream_handle: u32,
    next_future_handle: u32,
    next_error_context_handle: u32,
    next_waitable_set_handle: u32,
}

/// Stream value trait for type erasure
#[cfg(feature = "std")]
pub trait StreamValue: fmt::Debug {
    fn read(&mut self) -> Result<AsyncReadResult>;
    fn write(&mut self, values: &[Value]) -> Result<()>;
    fn cancel_read(&mut self) -> Result<()>;
    fn cancel_write(&mut self) -> Result<()>;
    fn close_readable(&mut self) -> Result<()>;
    fn close_writable(&mut self) -> Result<()>;
    fn element_type(&self) -> &ValType;
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
}

/// Future value trait for type erasure
#[cfg(feature = "std")]
pub trait FutureValue: fmt::Debug {
    fn read(&mut self) -> Result<AsyncReadResult>;
    fn write(&mut self, value: &Value) -> Result<()>;
    fn cancel_read(&mut self) -> Result<()>;
    fn cancel_write(&mut self) -> Result<()>;
    fn close_readable(&mut self) -> Result<()>;
    fn close_writable(&mut self) -> Result<()>;
    fn value_type(&self) -> &ValType;
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
}

/// Enum for stream values in no_std environments
#[cfg(not(any(feature = "std",)))]
#[derive(Debug)]
pub enum StreamValueEnum {
    Values(Stream<Value>),
    // Add more variants as needed for different types
}

/// Enum for future values in no_std environments
#[cfg(not(any(feature = "std",)))]
#[derive(Debug)]
pub enum FutureValueEnum {
    Value(Future<Value>),
    // Add more variants as needed for different types
}

/// Concrete stream implementation
#[derive(Debug)]
pub struct ConcreteStream<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    inner: Stream<T>,
}

/// Concrete future implementation
#[derive(Debug)]
pub struct ConcreteFuture<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    inner: Future<T>,
}

impl AsyncCanonicalAbi {
    /// Create a new async canonical ABI
    pub fn new() -> Result<Self> {
        Ok(Self {
            canonical_abi: CanonicalAbi::new(),
            task_manager: TaskManager::new(),
            #[cfg(feature = "std")]
            streams: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            streams: { BoundedVec::new() },
            #[cfg(feature = "std")]
            futures: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            futures: { BoundedVec::new() },
            #[cfg(feature = "std")]
            error_contexts: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            error_contexts: { BoundedVec::new() },
            #[cfg(feature = "std")]
            waitable_sets: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            waitable_sets: { BoundedVec::new() },
            #[cfg(feature = "std")]
            context_slots: BTreeMap::new(),
            #[cfg(not(any(feature = "std",)))]
            context_slots: { BoundedVec::new() },
            backpressure_enabled: false,
            backpressure_count: 0,
            next_stream_handle: 0,
            next_future_handle: 0,
            next_error_context_handle: 0,
            next_waitable_set_handle: 0,
        })
    }

    /// Create a new stream
    pub fn stream_new(&mut self, element_type: &ValType) -> Result<StreamHandle> {
        let handle = StreamHandle(self.next_stream_handle);
        self.next_stream_handle += 1;

        let stream = Stream::<Value>::new(handle, element_type.clone())?;

        #[cfg(feature = "std")]
        {
            let concrete = ConcreteStream { inner: stream };
            self.streams.insert(handle, Box::new(concrete));
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let stream_enum = StreamValueEnum::Values(stream);
            self.streams
                .push((handle, stream_enum))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many streams"))?;
        }

        Ok(handle)
    }

    /// Read from a stream
    pub fn stream_read(&mut self, stream_handle: StreamHandle) -> Result<AsyncReadResult> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.read()
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid stream handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            if s.buffer.is_empty() {
                                if s.writable_closed {
                                    Ok(AsyncReadResult::Closed)
                                } else {
                                    Ok(AsyncReadResult::Blocked)
                                }
                            } else {
                                // Read one value
                                let value = s.buffer.remove(0);
                                Ok(AsyncReadResult::Values(vec![value]))
                            }
                        },
                    };
                }
            }
            Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::INVALID_INPUT,
                "Invalid stream handle",
            ))
        }
    }

    /// Write to a stream
    pub fn stream_write(&mut self, stream_handle: StreamHandle, values: &[Value]) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.write(values)
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            if s.writable_closed {
                                return Err(wrt_error::Error::runtime_execution_error(
                                    "Stream is closed",
                                ));
                            }
                            for value in values {
                                s.buffer.push(value.clone()).map_err(|_| {
                                    wrt_error::Error::resource_exhausted("Buffer full")
                                })?;
                            }
                            s.state = StreamState::Ready;
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Cancel read operation on a stream
    pub fn stream_cancel_read(&mut self, stream_handle: StreamHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.cancel_read()
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            s.close_readable();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Cancel write operation on a stream
    pub fn stream_cancel_write(&mut self, stream_handle: StreamHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.cancel_write()
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            s.close_writable();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Close readable end of a stream
    pub fn stream_close_readable(&mut self, stream_handle: StreamHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.close_readable()
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            s.close_readable();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Close writable end of a stream
    pub fn stream_close_writable(&mut self, stream_handle: StreamHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(stream) = self.streams.get_mut(&stream_handle) {
                stream.close_writable()
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, stream) in &mut self.streams {
                if *handle == stream_handle {
                    return match stream {
                        StreamValueEnum::Values(s) => {
                            s.close_writable();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Create a new future
    pub fn future_new(&mut self, value_type: &ValType) -> Result<FutureHandle> {
        let handle = FutureHandle(self.next_future_handle);
        self.next_future_handle += 1;

        let future = Future::<Value>::new(handle, value_type.clone());

        #[cfg(feature = "std")]
        {
            let concrete = ConcreteFuture { inner: future };
            self.futures.insert(handle, Box::new(concrete));
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let future_enum = FutureValueEnum::Value(future);
            self.futures
                .push((handle, future_enum))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many futures"))?;
        }

        Ok(handle)
    }

    /// Read from a future
    pub fn future_read(&mut self, future_handle: FutureHandle) -> Result<AsyncReadResult> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.read()
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => match f.state {
                            FutureState::Ready => {
                                if let Some(value) = f.value.take() {
                                    Ok(AsyncReadResult::Values(vec![value]))
                                } else {
                                    Ok(AsyncReadResult::Closed)
                                }
                            },
                            FutureState::Cancelled => Ok(AsyncReadResult::Closed),
                            FutureState::Error => Ok(AsyncReadResult::Closed),
                            FutureState::Failed => Ok(AsyncReadResult::Closed),
                            FutureState::Pending => Ok(AsyncReadResult::Blocked),
                        },
                    };
                }
            }
            Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::INVALID_INPUT,
                "Invalid stream handle",
            ))
        }
    }

    /// Write to a future
    pub fn future_write(&mut self, future_handle: FutureHandle, value: &Value) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.write(value)
            } else {
                Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => f.set_value(value.clone()),
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Cancel read operation on a future
    ///
    /// This cancels any pending read operation on the future. After this call,
    /// the future handle is no longer valid for reading.
    pub fn future_cancel_read(&mut self, future_handle: FutureHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.cancel_read()
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid future handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => {
                            f.cancel();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid future handle",
            ))
        }
    }

    /// Cancel write operation on a future
    ///
    /// This cancels any pending write operation on the future. After this call,
    /// the future handle is no longer valid for writing.
    pub fn future_cancel_write(&mut self, future_handle: FutureHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.cancel_write()
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid future handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => {
                            f.cancel();
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid future handle",
            ))
        }
    }

    /// Drop the readable end of a future
    ///
    /// This releases the readable side of the future. The writer may still complete
    /// but the result will be discarded.
    pub fn future_drop_readable(&mut self, future_handle: FutureHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.close_readable()
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid future handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => {
                            f.readable_closed = true;
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid future handle",
            ))
        }
    }

    /// Drop the writable end of a future
    ///
    /// This releases the writable side of the future. Any pending reader will
    /// receive an error or empty result.
    pub fn future_drop_writable(&mut self, future_handle: FutureHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(future) = self.futures.get_mut(&future_handle) {
                future.close_writable()
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid future handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, future) in &mut self.futures {
                if *handle == future_handle {
                    return match future {
                        FutureValueEnum::Value(f) => {
                            f.writable_closed = true;
                            Ok(())
                        },
                    };
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid future handle",
            ))
        }
    }

    /// Create a new error context
    pub fn error_context_new(&mut self, message: &str) -> Result<ErrorContextHandle> {
        let handle = ErrorContextHandle(self.next_error_context_handle);
        self.next_error_context_handle += 1;

        #[cfg(feature = "std")]
        let error_context = ErrorContext::new(handle, message.to_string());
        #[cfg(not(any(feature = "std",)))]
        let error_context = {
            let provider = safe_managed_alloc!(2048, CrateId::Component)?;
            ErrorContext::new(
                handle,
                BoundedString::try_from_str(message).unwrap_or_default(),
            )?
        };

        #[cfg(feature = "std")]
        {
            self.error_contexts.insert(handle, error_context);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.error_contexts
                .push((handle, error_context))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many error contexts"))?;
        }

        Ok(handle)
    }

    /// Get debug string from error context
    #[cfg(feature = "std")]
    pub fn error_context_debug_string(&self, handle: ErrorContextHandle) -> Result<String> {
        if let Some(error_context) = self.error_contexts.get(&handle) {
            Ok(error_context.debug_string())
        } else {
            Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
        }
    }

    /// Get debug string from error context
    #[cfg(not(any(feature = "std",)))]
    pub fn error_context_debug_string(
        &self,
        handle: ErrorContextHandle,
    ) -> Result<BoundedString<1024>> {
        for (ctx_handle, error_context) in &self.error_contexts {
            if *ctx_handle == handle {
                return Ok(error_context.debug_string());
            }
        }
        Err(wrt_error::Error::runtime_execution_error("Invalid handle"))
    }

    /// Drop an error context
    pub fn error_context_drop(&mut self, handle: ErrorContextHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.error_contexts.remove(&handle);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.error_contexts.retain(|(h, _)| *h != handle);
        }
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Waitable Set Operations (canon.waitable-set.*)
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a new waitable set (canon waitable-set.new)
    ///
    /// Creates an empty waitable set that can be used to wait on multiple
    /// async resources (streams, futures) simultaneously.
    ///
    /// Returns a handle to the newly created waitable set.
    pub fn waitable_set_new(&mut self) -> Result<WaitableSetHandle> {
        let handle = WaitableSetHandle::new(self.next_waitable_set_handle);
        self.next_waitable_set_handle += 1;

        let waitable_set = WaitableSet::new()?;

        #[cfg(feature = "std")]
        {
            self.waitable_sets.insert(handle, waitable_set);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.waitable_sets
                .push((handle, waitable_set))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many waitable sets"))?;
        }

        Ok(handle)
    }

    /// Add a waitable to a waitable set (canon waitable.join)
    ///
    /// Adds the specified waitable (stream or future endpoint) to the set.
    /// Returns the index of the waitable within the set, which can be used
    /// to identify which waitable became ready after wait/poll.
    pub fn waitable_join(
        &mut self,
        set_handle: WaitableSetHandle,
        waitable: Waitable,
    ) -> Result<u32> {
        #[cfg(feature = "std")]
        {
            // Check readiness before borrowing waitable_sets mutably
            let is_ready = self.check_waitable_ready(&waitable)?;

            if let Some(set) = self.waitable_sets.get_mut(&set_handle) {
                let index = set.add(waitable)?;
                if is_ready {
                    set.mark_ready(index);
                }
                Ok(index)
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // Check readiness before borrowing waitable_sets mutably
            let is_ready = self.check_waitable_ready_no_std(&waitable);

            for (handle, set) in &mut self.waitable_sets {
                if *handle == set_handle {
                    let index = set.add(waitable)?;
                    if is_ready {
                        set.mark_ready(index);
                    }
                    return Ok(index);
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid waitable set handle",
            ))
        }
    }

    /// Wait for any waitable in the set to become ready (canon waitable-set.wait)
    ///
    /// Blocks until at least one waitable in the set is ready. Returns the
    /// index of a ready waitable. If multiple waitables are ready, any one
    /// of them may be returned.
    ///
    /// This is the blocking version - use `waitable_set_poll` for non-blocking.
    pub fn waitable_set_wait(&mut self, set_handle: WaitableSetHandle) -> Result<u32> {
        // First, update the ready state of all waitables in the set
        self.update_waitable_set_ready_state(set_handle)?;

        #[cfg(feature = "std")]
        {
            if let Some(set) = self.waitable_sets.get(&set_handle) {
                if let Some(ready_index) = set.first_ready() {
                    Ok(ready_index)
                } else {
                    // In a real implementation, this would suspend the current task
                    // and resume when a waitable becomes ready. For now, return an error.
                    Err(wrt_error::Error::runtime_execution_error(
                        "No waitables ready and blocking not yet implemented",
                    ))
                }
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, set) in &self.waitable_sets {
                if *handle == set_handle {
                    if let Some(ready_index) = set.first_ready() {
                        return Ok(ready_index);
                    } else {
                        return Err(wrt_error::Error::runtime_execution_error(
                            "No waitables ready and blocking not yet implemented",
                        ));
                    }
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid waitable set handle",
            ))
        }
    }

    /// Poll a waitable set for ready waitables (canon waitable-set.poll)
    ///
    /// Non-blocking check for ready waitables. Returns `Some(index)` if a
    /// waitable is ready, `None` if no waitables are currently ready.
    pub fn waitable_set_poll(&mut self, set_handle: WaitableSetHandle) -> Result<Option<u32>> {
        // Update ready state before checking
        self.update_waitable_set_ready_state(set_handle)?;

        #[cfg(feature = "std")]
        {
            if let Some(set) = self.waitable_sets.get(&set_handle) {
                Ok(set.first_ready())
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (handle, set) in &self.waitable_sets {
                if *handle == set_handle {
                    return Ok(set.first_ready());
                }
            }
            Err(wrt_error::Error::runtime_execution_error(
                "Invalid waitable set handle",
            ))
        }
    }

    /// Drop a waitable set (canon waitable-set.drop)
    ///
    /// Releases the waitable set and all its resources. The waitables
    /// themselves are not affected - only the set's tracking of them.
    pub fn waitable_set_drop(&mut self, set_handle: WaitableSetHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if self.waitable_sets.remove(&set_handle).is_some() {
                Ok(())
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let initial_len = self.waitable_sets.len();
            self.waitable_sets.retain(|(h, _)| *h != set_handle);
            if self.waitable_sets.len() < initial_len {
                Ok(())
            } else {
                Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ))
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Waitable Set Helper Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Check if a waitable is currently ready (std version)
    #[cfg(feature = "std")]
    fn check_waitable_ready(&self, waitable: &Waitable) -> Result<bool> {
        match waitable {
            Waitable::StreamReadable(handle) => {
                if let Some(stream) = self.streams.get(handle) {
                    Ok(stream.is_readable())
                } else {
                    Ok(false)
                }
            },
            Waitable::StreamWritable(handle) => {
                if let Some(stream) = self.streams.get(handle) {
                    Ok(stream.is_writable())
                } else {
                    Ok(false)
                }
            },
            Waitable::FutureReadable(handle) => {
                if let Some(future) = self.futures.get(handle) {
                    Ok(future.is_readable())
                } else {
                    Ok(false)
                }
            },
            Waitable::FutureWritable(handle) => {
                if let Some(future) = self.futures.get(handle) {
                    Ok(future.is_writable())
                } else {
                    Ok(false)
                }
            },
        }
    }

    /// Check if a waitable is currently ready (no_std version)
    #[cfg(not(any(feature = "std",)))]
    fn check_waitable_ready_no_std(&self, waitable: &Waitable) -> bool {
        match waitable {
            Waitable::StreamReadable(handle) => {
                for (h, stream) in &self.streams {
                    if h == handle {
                        return match stream {
                            StreamValueEnum::Values(s) => !s.buffer.is_empty() || s.writable_closed,
                        };
                    }
                }
                false
            },
            Waitable::StreamWritable(handle) => {
                for (h, stream) in &self.streams {
                    if h == handle {
                        return match stream {
                            StreamValueEnum::Values(s) => !s.writable_closed,
                        };
                    }
                }
                false
            },
            Waitable::FutureReadable(handle) => {
                for (h, future) in &self.futures {
                    if h == handle {
                        return match future {
                            FutureValueEnum::Value(f) => f.state == FutureState::Ready,
                        };
                    }
                }
                false
            },
            Waitable::FutureWritable(handle) => {
                for (h, future) in &self.futures {
                    if h == handle {
                        return match future {
                            FutureValueEnum::Value(f) => {
                                f.state == FutureState::Pending && !f.writable_closed
                            },
                        };
                    }
                }
                false
            },
        }
    }

    /// Update the ready state of all waitables in a set
    fn update_waitable_set_ready_state(&mut self, set_handle: WaitableSetHandle) -> Result<()> {
        #[cfg(feature = "std")]
        {
            // Clone waitables to avoid borrow issues
            let waitables: Vec<Waitable> = if let Some(set) = self.waitable_sets.get(&set_handle) {
                set.waitables.clone()
            } else {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ));
            };

            // Check each waitable and update ready mask
            let mut new_ready_mask: u64 = 0;
            for (index, waitable) in waitables.iter().enumerate() {
                if self.check_waitable_ready(waitable)? {
                    new_ready_mask |= 1u64 << index;
                }
            }

            // Update the set's ready mask
            if let Some(set) = self.waitable_sets.get_mut(&set_handle) {
                set.ready_mask = new_ready_mask;
            }

            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // For no_std, we need to be more careful with borrows
            let mut waitables_snapshot: BoundedVec<Waitable, 64> = BoundedVec::new();
            let mut found = false;

            for (handle, set) in &self.waitable_sets {
                if *handle == set_handle {
                    found = true;
                    for w in &set.waitables {
                        let _ = waitables_snapshot.push(w.clone());
                    }
                    break;
                }
            }

            if !found {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Invalid waitable set handle",
                ));
            }

            // Check each waitable
            let mut new_ready_mask: u64 = 0;
            for (index, waitable) in waitables_snapshot.iter().enumerate() {
                if self.check_waitable_ready_no_std(waitable) {
                    new_ready_mask |= 1u64 << index;
                }
            }

            // Update ready mask
            for (handle, set) in &mut self.waitable_sets {
                if *handle == set_handle {
                    set.ready_mask = new_ready_mask;
                    break;
                }
            }

            Ok(())
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Context Operations (canon.context.*)
    // ─────────────────────────────────────────────────────────────────────────

    /// Get a value from the thread/task context (canon context.get)
    ///
    /// Retrieves the value stored in the specified context slot. Context slots
    /// provide implicit parameter passing through the call stack without
    /// explicitly threading values through every function call.
    ///
    /// # Arguments
    /// * `slot` - The context slot index to retrieve from
    ///
    /// # Returns
    /// * `Ok(Some(value))` - If a value is stored in the slot
    /// * `Ok(None)` - If the slot is empty (no value set)
    /// * `Err(...)` - If an error occurs
    pub fn context_get(&self, slot: u32) -> Result<Option<Value>> {
        #[cfg(feature = "std")]
        {
            Ok(self.context_slots.get(&slot).cloned())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (s, value) in &self.context_slots {
                if *s == slot {
                    return Ok(Some(value.clone()));
                }
            }
            Ok(None)
        }
    }

    /// Set a value in the thread/task context (canon context.set)
    ///
    /// Stores a value in the specified context slot. Setting `None` removes
    /// any existing value from the slot. Context values are inherited by
    /// child tasks and can be overridden by callees.
    ///
    /// # Arguments
    /// * `slot` - The context slot index to store into
    /// * `value` - The value to store, or `None` to clear the slot
    ///
    /// # Returns
    /// * `Ok(())` - On success
    /// * `Err(...)` - If an error occurs (e.g., too many slots in no_std)
    pub fn context_set(&mut self, slot: u32, value: Option<Value>) -> Result<()> {
        #[cfg(feature = "std")]
        {
            match value {
                Some(v) => {
                    self.context_slots.insert(slot, v);
                },
                None => {
                    self.context_slots.remove(&slot);
                },
            }
            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            match value {
                Some(v) => {
                    // Try to update existing slot first
                    for (s, existing) in &mut self.context_slots {
                        if *s == slot {
                            *existing = v;
                            return Ok(());
                        }
                    }
                    // Not found, add new
                    self.context_slots.push((slot, v)).map_err(|_| {
                        wrt_error::Error::resource_exhausted("Too many context slots")
                    })?;
                    Ok(())
                },
                None => {
                    self.context_slots.retain(|(s, _)| *s != slot);
                    Ok(())
                },
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Backpressure Operations (canon.backpressure.*)
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable or disable backpressure (canon backpressure.set)
    ///
    /// When backpressure is enabled, callers should avoid sending more work
    /// to this component until backpressure is released. This is used for
    /// flow control in async component compositions.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable (true) or disable (false) backpressure
    pub fn backpressure_set(&mut self, enabled: bool) -> Result<()> {
        self.backpressure_enabled = enabled;
        Ok(())
    }

    /// Check if backpressure is currently enabled
    ///
    /// Returns true if the component is signaling that it cannot accept
    /// more work at the moment.
    pub fn backpressure_get(&self) -> bool {
        self.backpressure_enabled
    }

    /// Increment the backpressure count (canon backpressure.inc)
    ///
    /// Used by callers to indicate they have pending work for this component.
    /// The component uses this count to decide when to enable/disable
    /// backpressure based on its capacity.
    pub fn backpressure_inc(&mut self) -> Result<u32> {
        self.backpressure_count = self.backpressure_count.saturating_add(1);
        Ok(self.backpressure_count)
    }

    /// Decrement the backpressure count (canon backpressure.dec)
    ///
    /// Used by callers when work completes. When the count reaches zero,
    /// the component may choose to disable backpressure.
    pub fn backpressure_dec(&mut self) -> Result<u32> {
        self.backpressure_count = self.backpressure_count.saturating_sub(1);
        Ok(self.backpressure_count)
    }

    /// Get the current backpressure count
    pub fn backpressure_count(&self) -> u32 {
        self.backpressure_count
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Task Operations (wrappers around TaskManager)
    // ─────────────────────────────────────────────────────────────────────────

    /// Task return operation
    pub fn task_return(&mut self, values: ComponentVec<Value>) -> Result<()> {
        self.task_manager.task_return(values)
    }

    /// Task wait operation
    pub fn task_wait(&mut self, waitables: WaitableSet) -> Result<u32> {
        self.task_manager.task_wait(waitables)
    }

    /// Task poll operation
    pub fn task_poll(&self, waitables: &WaitableSet) -> Result<Option<u32>> {
        self.task_manager.task_poll(waitables)
    }

    /// Task yield operation
    pub fn task_yield(&mut self) -> Result<()> {
        self.task_manager.task_yield()
    }

    /// Task cancel operation
    pub fn task_cancel(&mut self, task_id: TaskId) -> Result<()> {
        self.task_manager.task_cancel(task_id)
    }

    /// Task backpressure operation
    pub fn task_backpressure(&mut self) -> Result<()> {
        self.task_manager.task_backpressure()
    }

    /// Get the underlying task manager
    pub fn task_manager(&self) -> &TaskManager {
        &self.task_manager
    }

    /// Get mutable task manager
    pub fn task_manager_mut(&mut self) -> &mut TaskManager {
        &mut self.task_manager
    }

    /// Get the underlying canonical ABI
    pub fn canonical_abi(&self) -> &CanonicalAbi {
        &self.canonical_abi
    }

    /// Get mutable canonical ABI
    pub fn canonical_abi_mut(&mut self) -> &mut CanonicalAbi {
        &mut self.canonical_abi
    }

    /// Perform async lifting of values from core representation
    pub fn async_lift(
        &mut self,
        values: &[u8],
        target_types: &[ValType],
        context: &CanonicalLiftContext,
    ) -> Result<AsyncLiftResult> {
        // Check for immediate values first
        if self.can_lift_immediately(values, target_types)? {
            let lifted_values = self.lift_immediate(values, target_types, &context.options)?;
            return Ok(AsyncLiftResult::Immediate(lifted_values));
        }

        // Check for stream types
        if target_types.len() == 1 {
            if let ValType::Stream(_) = &target_types[0] {
                let stream_handle = self.stream_new(&target_types[0])?;
                return Ok(AsyncLiftResult::Stream(stream_handle));
            }
            if let ValType::Future(_) = &target_types[0] {
                let future_handle = self.future_new(&target_types[0])?;
                return Ok(AsyncLiftResult::Future(future_handle));
            }
        }

        // Create pending async operation for complex lifting
        let operation = AsyncOperation {
            id: self.next_error_context_handle, // Reuse counter
            op_type: AsyncOperationType::AsyncCall,
            state: AsyncOperationState::Starting,
            #[cfg(feature = "std")]
            context: values.to_vec(),
            #[cfg(not(any(feature = "std",)))]
            context: BoundedVec::from_slice(values)
                .map_err(|_| Error::runtime_execution_error("Context too large"))?,
            task_handle: None,
        };

        self.next_error_context_handle += 1;
        Ok(AsyncLiftResult::Pending(operation))
    }

    /// Perform async lowering of values to core representation
    pub fn async_lower(
        &mut self,
        values: &[Value],
        context: &CanonicalLowerContext,
    ) -> Result<AsyncLowerResult> {
        // Check for immediate lowering
        if self.can_lower_immediately(values)? {
            let lowered_bytes = self.lower_immediate(values, &context.options)?;
            return Ok(AsyncLowerResult::Immediate(lowered_bytes));
        }

        // Check for stream/future values
        if values.len() == 1 {
            match &values[0] {
                Value::Stream(handle) => {
                    return Ok(AsyncLowerResult::Stream(
                        crate::async_::async_types::StreamHandle::new(handle.0),
                    ));
                },
                Value::Future(handle) => {
                    return Ok(AsyncLowerResult::Future(
                        crate::async_::async_types::FutureHandle::new(handle.0),
                    ));
                },
                _ => {},
            }
        }

        // Create pending async operation for complex lowering
        let operation = AsyncOperation {
            id: self.next_error_context_handle,
            op_type: AsyncOperationType::AsyncCall,
            state: AsyncOperationState::Starting,
            #[cfg(feature = "std")]
            context: Vec::new(), // Values will be serialized separately
            #[cfg(not(any(feature = "std",)))]
            context: { BoundedVec::new() },
            task_handle: None,
        };

        self.next_error_context_handle += 1;
        Ok(AsyncLowerResult::Pending(operation))
    }

    // Private helper methods for async operations
    fn can_lift_immediately(&self, _values: &[u8], target_types: &[ValType]) -> Result<bool> {
        // Check if all target types are immediately liftable (not async types)
        for ty in target_types {
            match ty {
                ValType::Stream(_) | ValType::Future(_) => return Ok(false),
                _ => {},
            }
        }
        Ok(true)
    }

    fn can_lower_immediately(&self, values: &[Value]) -> Result<bool> {
        // Check if all values are immediately lowerable (not async values)
        for value in values {
            match value {
                Value::Stream(_) | Value::Future(_) => return Ok(false),
                _ => {},
            }
        }
        Ok(true)
    }

    fn lift_immediate(
        &self,
        values: &[u8],
        target_types: &[ValType],
        options: &CanonicalOptions,
    ) -> Result<ComponentVec<Value>> {
        // Use the stub canonical ABI lifting
        async_canonical_lifting::async_canonical_lift(values, target_types, options)
    }

    fn lower_immediate(
        &self,
        values: &[Value],
        options: &CanonicalOptions,
    ) -> Result<ComponentVec<u8>> {
        // Use the stub canonical ABI lowering
        async_canonical_lifting::async_canonical_lower(values, options)
    }
}

// Trait implementations for std environment
#[cfg(feature = "std")]
impl<T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq + fmt::Debug>
    StreamValue for ConcreteStream<T>
where
    Value: From<T>,
    T: TryFrom<Value>,
{
    fn read(&mut self) -> Result<AsyncReadResult> {
        if self.inner.buffer.is_empty() {
            if self.inner.writable_closed {
                Ok(AsyncReadResult::Closed)
            } else {
                Ok(AsyncReadResult::Blocked)
            }
        } else {
            let value = self.inner.buffer.remove(0);
            Ok(AsyncReadResult::Values(vec![Value::from(value)]))
        }
    }

    fn write(&mut self, values: &[Value]) -> Result<()> {
        if self.inner.writable_closed {
            return Err(wrt_error::Error::runtime_execution_error(
                "Stream is closed",
            ));
        }

        for value in values {
            if let Ok(typed_value) = T::try_from(value.clone()) {
                self.inner.buffer.push(typed_value);
            } else {
                return Err(wrt_error::Error::type_mismatch_error("Value type mismatch"));
            }
        }
        self.inner.state = StreamState::Ready;
        Ok(())
    }

    fn cancel_read(&mut self) -> Result<()> {
        self.inner.close_readable();
        Ok(())
    }

    fn cancel_write(&mut self) -> Result<()> {
        self.inner.close_writable();
        Ok(())
    }

    fn close_readable(&mut self) -> Result<()> {
        self.inner.close_readable();
        Ok(())
    }

    fn close_writable(&mut self) -> Result<()> {
        self.inner.close_writable();
        Ok(())
    }

    fn element_type(&self) -> &ValType {
        &self.inner.element_type
    }

    fn is_readable(&self) -> bool {
        self.inner.is_readable()
    }

    fn is_writable(&self) -> bool {
        self.inner.is_writable()
    }
}

#[cfg(feature = "std")]
impl<T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq + fmt::Debug>
    FutureValue for ConcreteFuture<T>
where
    Value: From<T>,
    T: TryFrom<Value>,
{
    fn read(&mut self) -> Result<AsyncReadResult> {
        match self.inner.state {
            FutureState::Ready => {
                if let Some(value) = self.inner.value.take() {
                    Ok(AsyncReadResult::Values(vec![Value::from(value)]))
                } else {
                    Ok(AsyncReadResult::Closed)
                }
            },
            FutureState::Cancelled | FutureState::Error | FutureState::Failed => {
                Ok(AsyncReadResult::Closed)
            },
            FutureState::Pending => Ok(AsyncReadResult::Blocked),
        }
    }

    fn write(&mut self, value: &Value) -> Result<()> {
        if let Ok(typed_value) = T::try_from(value.clone()) {
            self.inner.set_value(typed_value)
        } else {
            Err(wrt_error::Error::type_mismatch_error(
                "Value type mismatch for future",
            ))
        }
    }

    fn cancel_read(&mut self) -> Result<()> {
        self.inner.cancel();
        Ok(())
    }

    fn cancel_write(&mut self) -> Result<()> {
        self.inner.cancel();
        Ok(())
    }

    fn close_readable(&mut self) -> Result<()> {
        self.inner.readable_closed = true;
        Ok(())
    }

    fn close_writable(&mut self) -> Result<()> {
        self.inner.writable_closed = true;
        Ok(())
    }

    fn value_type(&self) -> &ValType {
        &self.inner.value_type
    }

    fn is_readable(&self) -> bool {
        self.inner.is_readable()
    }

    fn is_writable(&self) -> bool {
        self.inner.is_writable()
    }
}

impl Default for AsyncCanonicalAbi {
    fn default() -> Self {
        Self::new().expect("Failed to create AsyncCanonicalAbi")
    }
}

impl fmt::Display for AsyncOperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsyncOperationType::AsyncCall => write!(f, "async-call"),
            AsyncOperationType::StreamRead => write!(f, "stream-read"),
            AsyncOperationType::StreamWrite => write!(f, "stream-write"),
            AsyncOperationType::FutureGet => write!(f, "future-get"),
            AsyncOperationType::FutureSet => write!(f, "future-set"),
            AsyncOperationType::WaitablePoll => write!(f, "waitable-poll"),
        }
    }
}

impl fmt::Display for AsyncOperationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsyncOperationState::Starting => write!(f, "starting"),
            AsyncOperationState::InProgress => write!(f, "in-progress"),
            AsyncOperationState::Waiting => write!(f, "waiting"),
            AsyncOperationState::Completed => write!(f, "completed"),
            AsyncOperationState::Cancelled => write!(f, "cancelled"),
            AsyncOperationState::Failed => write!(f, "failed"),
        }
    }
}
