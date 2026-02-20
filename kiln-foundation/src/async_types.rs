//! Component Model async types for no_std environments
//!
//! This module provides the core async types from the WebAssembly Component
//! Model:
//! - `future<T>`: Single-value async computation
//! - `stream<T>`: Multi-value async sequence
//! - `error-context`: Error propagation for async operations

use core::task::Waker;

use crate::{
    types::ValueType as ValType,
    values::Value,
};

/// Maximum number of buffered values in a stream
pub const MAX_STREAM_BUFFER: usize = 64;

/// Handle to a Component Model future
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FutureHandle(pub u32);

/// Handle to a Component Model stream  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamHandle(pub u32);

/// Status of a Component Model future
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentFutureStatus<T> {
    /// Future is still pending
    Pending,
    /// Future has completed with a value
    Ready(T),
}

/// Component Model future type
pub struct ComponentFuture<T> {
    handle:     FutureHandle,
    value_type: ValType,
    status:     ComponentFutureStatus<T>,
    waker:      Option<Waker>,
}

impl<T> ComponentFuture<T> {
    /// Create a new Component Model future
    pub fn new(handle: FutureHandle, value_type: ValType) -> Self {
        Self {
            handle,
            value_type,
            status: ComponentFutureStatus::Pending,
            waker: None,
        }
    }

    /// Get the future's handle
    pub fn id(&self) -> u32 {
        self.handle.0
    }

    /// Poll the future's status
    pub fn poll_status(&mut self) -> Result<ComponentFutureStatus<T>, &'static str>
    where
        T: Clone,
    {
        // In a real implementation, this would check with the Component Model runtime
        Ok(self.status.clone())
    }

    /// Set a waker to be notified when the future completes
    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }

    /// Complete the future with a value
    pub fn complete(&mut self, value: T) {
        self.status = ComponentFutureStatus::Ready(value);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

/// State of a Component Model stream
#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    /// Stream is open for reading and writing
    Open,
    /// Stream is closed for writing but may have buffered values
    WriteClosed,
    /// Stream is fully closed
    Closed,
}

/// Component Model stream type (simplified for basic functionality)
pub struct ComponentStream<T> {
    handle:       StreamHandle,
    element_type: ValType,
    state:        StreamState,
    // Simplified: single item buffer for basic functionality
    buffer_item:  Option<T>,
    read_waker:   Option<Waker>,
    write_waker:  Option<Waker>,
}

impl<T> ComponentStream<T> {
    /// Create a new Component Model stream
    pub fn new(handle: StreamHandle, element_type: ValType) -> Self {
        Self {
            handle,
            element_type,
            state: StreamState::Open,
            buffer_item: None,
            read_waker: None,
            write_waker: None,
        }
    }

    /// Try to read a value from the stream
    pub fn try_read(&mut self) -> Result<Option<T>, &'static str> {
        if let Some(value) = self.buffer_item.take() {
            Ok(Some(value))
        } else if self.state == StreamState::Closed {
            Ok(None)
        } else {
            Err("No values available")
        }
    }

    /// Try to write a value to the stream
    pub fn try_write(&mut self, value: T) -> Result<(), &'static str> {
        if self.state != StreamState::Open {
            return Err("Stream closed for writing");
        }

        if self.buffer_item.is_some() {
            return Err("Stream buffer full");
        }

        self.buffer_item = Some(value);

        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }

        Ok(())
    }

    /// Check if the stream is closed
    pub fn is_closed(&self) -> bool {
        self.state == StreamState::Closed && self.buffer_item.is_none()
    }

    /// Close the stream for writing
    pub fn close_write(&mut self) {
        self.state = StreamState::WriteClosed;
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    /// Set a waker for read availability
    pub fn set_read_waker(&mut self, waker: Waker) {
        self.read_waker = Some(waker);
    }

    /// Set a waker for write availability
    pub fn set_write_waker(&mut self, waker: Waker) {
        self.write_waker = Some(waker);
    }
}

/// Error context for Component Model async operations
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error message
    pub message: &'static str,
    /// Optional error code
    pub code:    Option<u32>,
    /// Stack trace or additional context
    pub trace:   Option<&'static str>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(message: &'static str) -> Self {
        Self {
            message,
            code: None,
            trace: None,
        }
    }

    /// Add an error code
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    /// Add a trace or additional context
    pub fn with_trace(mut self, trace: &'static str) -> Self {
        self.trace = Some(trace);
        self
    }
}

/// Extension trait to add future/stream handles to `Value` `enum`
impl Value {
    /// Create a future value
    pub fn future(handle: u32) -> Self {
        // In a real implementation, this would be a new Value variant
        // For now, we use I32 as a placeholder
        Value::I32(handle as i32)
    }

    /// Create a stream value
    pub fn stream(handle: u32) -> Self {
        // In a real implementation, this would be a new Value variant
        // For now, we use I32 as a placeholder
        Value::I32(handle as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_future() {
        let mut future: ComponentFuture<u32> = ComponentFuture::new(FutureHandle(1), ValType::I32);

        // Initially pending
        assert!(matches!(
            future.poll_status().unwrap(),
            ComponentFutureStatus::Pending
        ));

        // Complete the future
        future.complete(42);
        assert!(matches!(
            future.poll_status().unwrap(),
            ComponentFutureStatus::Ready(42)
        ));
    }

    #[test]
    fn test_component_stream() {
        let mut stream: ComponentStream<u32> = ComponentStream::new(StreamHandle(1), ValType::I32);

        // Write and read a value
        stream.try_write(1).unwrap();
        assert_eq!(stream.try_read().unwrap(), Some(1));

        // Write another value
        stream.try_write(2).unwrap();
        assert_eq!(stream.try_read().unwrap(), Some(2));

        // No more values
        assert!(stream.try_read().is_err());

        // Close and check
        stream.close_write();
        assert!(!stream.is_closed()); // Still have buffered values

        stream.state = StreamState::Closed;
        assert!(stream.is_closed()); // Now fully closed
    }
}
