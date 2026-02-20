//! Bridge between Component Model async and Rust async/await
//!
//! This module provides the glue between Component Model async primitives
//! (stream, future, error-context) and Rust's Future trait when using
//! the pluggable executor system.

use core::{
    future::Future,
    marker::Unpin,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

#[cfg(feature = "component-model-async")]
use crate::async_types::{
    ComponentFuture,
    ComponentFutureStatus,
    ComponentStream,
    FutureHandle,
    StreamHandle,
};
use crate::{
    async_executor_simple::{
        with_async as block_on,
        ExecutorError,
    },
    types::ValueType as ValType,
    values::Value,
};

#[cfg(feature = "component-model-async")]
/// Bridge a Component Model future to a Rust future
pub struct ComponentFutureBridge<T> {
    component_future: ComponentFuture<T>,
}

#[cfg(feature = "component-model-async")]
impl<T> ComponentFutureBridge<T> {
    pub fn new(component_future: ComponentFuture<T>) -> Self {
        Self { component_future }
    }
}

#[cfg(feature = "component-model-async")]
impl<T: Clone + Send + 'static + Unpin> Future for ComponentFutureBridge<T> {
    type Output = Result<T, ExecutorError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Get mutable reference to the inner data
        let this = self.get_mut();

        // Check Component Model future status
        match this.component_future.poll_status() {
            Ok(ComponentFutureStatus::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(ComponentFutureStatus::Pending) => {
                // Register waker to be notified when Component Model future completes
                this.component_future.set_waker(cx.waker().clone());
                Poll::Pending
            },
            Err(_) => Poll::Ready(Err(ExecutorError::TaskPanicked)),
        }
    }
}

#[cfg(feature = "component-model-async")]
/// Bridge a Component Model stream to a Rust Stream (when available)
pub struct ComponentStreamBridge<T> {
    component_stream: ComponentStream<T>,
}

#[cfg(feature = "component-model-async")]
impl<T> ComponentStreamBridge<T> {
    pub fn new(component_stream: ComponentStream<T>) -> Self {
        Self { component_stream }
    }

    /// Poll for the next value from the stream
    pub fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<T>> {
        match self.component_stream.try_read() {
            Ok(Some(value)) => Poll::Ready(Some(value)),
            Ok(None) if self.component_stream.is_closed() => Poll::Ready(None),
            _ => Poll::Pending,
        }
    }
}

/// Re-export AsyncRuntime from the simple executor
pub use crate::async_executor_simple::AsyncRuntime;

#[cfg(feature = "component-model-async")]
/// Extension trait for Component Model values to work with async
pub trait ComponentAsyncExt {
    /// Convert a Component Model async value to a Rust future
    fn into_future<T>(self) -> Result<ComponentFutureBridge<T>, ExecutorError>
    where
        T: Clone + Send + 'static;

    /// Convert a Component Model stream to a bridged stream
    fn into_stream<T>(self) -> Result<ComponentStreamBridge<T>, ExecutorError>
    where
        T: Clone + Send + 'static;
}

#[cfg(feature = "component-model-async")]
impl ComponentAsyncExt for Value {
    fn into_future<T>(self) -> Result<ComponentFutureBridge<T>, ExecutorError>
    where
        T: Clone + Send + 'static,
    {
        // Note: In a real implementation, Value would have Future and Stream variants
        // For now, we simulate with U32 values
        match self {
            Value::I32(future_handle) => {
                // In a real implementation, we'd look up the Component Model future
                // For now, we create a placeholder
                let component_future = ComponentFuture::new(
                    FutureHandle(future_handle as u32),
                    ValType::I32, // Placeholder type
                );
                Ok(ComponentFutureBridge::new(component_future))
            },
            _ => Err(ExecutorError::Custom("Value is not a future")),
        }
    }

    fn into_stream<T>(self) -> Result<ComponentStreamBridge<T>, ExecutorError>
    where
        T: Clone + Send + 'static,
    {
        match self {
            Value::I32(stream_handle) => {
                // In a real implementation, we'd look up the Component Model stream
                // For now, we create a placeholder
                let component_stream = ComponentStream::new(
                    StreamHandle(stream_handle as u32),
                    ValType::I32, // Placeholder type
                );
                Ok(ComponentStreamBridge::new(component_stream))
            },
            _ => Err(ExecutorError::Custom("Value is not a stream")),
        }
    }
}

/// Helper to run async code in a Component Model context
pub fn with_async<F, T>(f: F) -> Result<T, ExecutorError>
where
    F: Future<Output = T> + core::marker::Unpin,
{
    block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_runtime_creation() {
        let runtime = AsyncRuntime::new();
        assert!(runtime.executor.is_running());
    }
}
