//! Async types for WebAssembly Component Model
//!
//! This module implements the async types (stream, future, error-context)
//! required by the Component Model MVP specification for concurrent operations.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

#[cfg(feature = "std")]
use crate::bounded_component_infra::ComponentProvider;
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    MemoryProvider, NoStdProvider,
    bounded::BoundedString,
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};
#[cfg(feature = "std")]
use wrt_foundation::{
    collections::StaticVec as BoundedVec,
    component_value::ComponentValue,
    prelude::*,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

// Import prelude for no_std to get Vec, Box, etc.
#[cfg(not(feature = "std"))]
use crate::prelude::*;
#[cfg(not(feature = "std"))]
// For no_std, use a simpler ComponentValue representation
use crate::types::Value as ComponentValue;
use crate::types::{ValType, Value};

/// Maximum number of pending values in a stream for no_std environments
const MAX_STREAM_BUFFER: usize = 1024;

/// Maximum number of waitables in a set for no_std environments  
const MAX_WAITABLES: usize = 64;

/// Handle to a stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct StreamHandle(pub u32);

impl StreamHandle {
    /// Create a new stream handle
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl Checksummable for StreamHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for StreamHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for StreamHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Handle to a future
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FutureHandle(pub u32);

impl FutureHandle {
    /// Create a new future handle
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl Checksummable for FutureHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for FutureHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for FutureHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Handle to an error context
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErrorContextHandle(pub u32);

/// Handle to a waitable set
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WaitableSetHandle(pub u32);

impl WaitableSetHandle {
    /// Create a new waitable set handle
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

impl Checksummable for WaitableSetHandle {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for WaitableSetHandle {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for WaitableSetHandle {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Stream type for incremental value passing
#[derive(Debug, Clone)]
pub struct Stream<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    /// Stream handle
    pub handle: StreamHandle,
    /// Element type
    pub element_type: ValType,
    /// Stream state
    pub state: StreamState,
    /// Buffered values
    #[cfg(feature = "std")]
    pub buffer: Vec<T>,
    #[cfg(not(any(feature = "std",)))]
    pub buffer: BoundedVec<T, MAX_STREAM_BUFFER>,
    /// Readable end closed
    pub readable_closed: bool,
    /// Writable end closed  
    pub writable_closed: bool,
}

/// Future type for deferred values
#[derive(Debug, Clone)]
pub struct Future<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    /// Future handle
    pub handle: FutureHandle,
    /// Value type
    pub value_type: ValType,
    /// Future state
    pub state: FutureState,
    /// Stored value (once available)
    pub value: Option<T>,
    /// Readable end closed
    pub readable_closed: bool,
    /// Writable end closed
    pub writable_closed: bool,
}

/// Error context for detailed error information
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error context handle
    pub handle: ErrorContextHandle,
    /// Error message
    #[cfg(feature = "std")]
    pub message: String,
    #[cfg(not(any(feature = "std",)))]
    pub message: BoundedString<1024>,
    /// Stack trace if available
    #[cfg(feature = "std")]
    pub stack_trace: Option<Vec<StackFrame>>,
    #[cfg(not(any(feature = "std",)))]
    pub stack_trace: Option<BoundedVec<StackFrame, 32>>,
    /// Additional debug information
    pub debug_info: DebugInfo,
}

/// Stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is open and can transfer values
    Open,
    /// Stream has pending values to read
    Ready,
    /// Stream is closed (no more values)
    Closed,
    /// Stream encountered an error
    Error,
}

/// Future state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureState {
    /// Future is pending (no value yet)
    Pending,
    /// Future has a value ready
    Ready,
    /// Future was cancelled
    Cancelled,
    /// Future encountered an error
    Error,
    /// Future failed with error
    Failed,
}

/// Stack frame information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    /// Function name
    #[cfg(feature = "std")]
    pub function: String,
    #[cfg(not(any(feature = "std",)))]
    pub function: BoundedString<128>,
    /// Component instance
    pub component_instance: Option<u32>,
    /// Instruction offset
    pub offset: Option<u32>,
}

impl Default for StackFrame {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            function: String::new(),
            #[cfg(not(any(feature = "std",)))]
            function: BoundedString::from_str_truncate("")
                .unwrap_or_else(|_| panic!("Failed to create default StackFrame function name")),
            component_instance: None,
            offset: None,
        }
    }
}

impl Checksummable for StackFrame {
    fn update_checksum(&self, checksum: &mut Checksum) {
        #[cfg(feature = "std")]
        self.function.update_checksum(checksum);
        #[cfg(not(any(feature = "std",)))]
        {
            if let Ok(s) = self.function.as_str() {
                checksum.update_slice(s.as_bytes());
            }
        }

        self.component_instance.update_checksum(checksum);
        self.offset.update_checksum(checksum);
    }
}

impl ToBytes for StackFrame {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        self.function.to_bytes_with_provider(writer, provider)?;
        #[cfg(not(any(feature = "std",)))]
        self.function.to_bytes_with_provider(writer, provider)?;

        self.component_instance.to_bytes_with_provider(writer, provider)?;
        self.offset.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for StackFrame {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        #[cfg(feature = "std")]
        let function = String::from_bytes_with_provider(reader, provider)?;
        #[cfg(not(any(feature = "std",)))]
        let function = BoundedString::<128>::from_bytes_with_provider(reader, provider)?;

        let component_instance = Option::<u32>::from_bytes_with_provider(reader, provider)?;
        let offset = Option::<u32>::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            function,
            component_instance,
            offset,
        })
    }
}

/// Debug information for error contexts
#[derive(Debug, Clone)]
pub struct DebugInfo {
    /// Component that created the error
    pub source_component: Option<u32>,
    /// Error code if available
    pub error_code: Option<u32>,
    /// Additional properties
    #[cfg(feature = "std")]
    pub properties: Vec<(String, ComponentValue<ComponentProvider>)>,
    #[cfg(not(any(feature = "std",)))]
    pub properties: BoundedVec<(BoundedString<64>, ComponentValue), 16>,
}

/// Async read result
#[derive(Debug, Clone)]
pub enum AsyncReadResult {
    /// Value(s) available
    Values(Vec<Value>),
    /// Stream/future closed
    Closed,
    /// Would block (no data available)
    Blocked,
    /// Error occurred
    Error(ErrorContextHandle),
}

/// Waitable resource for task synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waitable {
    /// Stream readable
    StreamReadable(StreamHandle),
    /// Stream writable
    StreamWritable(StreamHandle),
    /// Future readable
    FutureReadable(FutureHandle),
    /// Future writable
    FutureWritable(FutureHandle),
}

impl Default for Waitable {
    fn default() -> Self {
        Self::StreamReadable(StreamHandle(0))
    }
}

impl Checksummable for Waitable {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            Self::StreamReadable(h) => {
                0u8.update_checksum(checksum);
                h.0.update_checksum(checksum);
            },
            Self::StreamWritable(h) => {
                1u8.update_checksum(checksum);
                h.0.update_checksum(checksum);
            },
            Self::FutureReadable(h) => {
                2u8.update_checksum(checksum);
                h.0.update_checksum(checksum);
            },
            Self::FutureWritable(h) => {
                3u8.update_checksum(checksum);
                h.0.update_checksum(checksum);
            },
        }
    }
}

impl ToBytes for Waitable {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Self::StreamReadable(h) => {
                0u8.to_bytes_with_provider(writer, provider)?;
                h.0.to_bytes_with_provider(writer, provider)
            },
            Self::StreamWritable(h) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                h.0.to_bytes_with_provider(writer, provider)
            },
            Self::FutureReadable(h) => {
                2u8.to_bytes_with_provider(writer, provider)?;
                h.0.to_bytes_with_provider(writer, provider)
            },
            Self::FutureWritable(h) => {
                3u8.to_bytes_with_provider(writer, provider)?;
                h.0.to_bytes_with_provider(writer, provider)
            },
        }
    }
}

impl FromBytes for Waitable {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let tag = u8::from_bytes_with_provider(reader, provider)?;
        let value = u32::from_bytes_with_provider(reader, provider)?;
        match tag {
            0 => Ok(Self::StreamReadable(StreamHandle(value))),
            1 => Ok(Self::StreamWritable(StreamHandle(value))),
            2 => Ok(Self::FutureReadable(FutureHandle(value))),
            3 => Ok(Self::FutureWritable(FutureHandle(value))),
            _ => Err(wrt_error::Error::validation_invalid_type(
                "Invalid Waitable tag",
            )),
        }
    }
}

/// Set of waitables for task synchronization
#[derive(Debug, Clone)]
pub struct WaitableSet {
    /// Waitables in the set
    #[cfg(feature = "std")]
    pub waitables: Vec<Waitable>,
    #[cfg(not(any(feature = "std",)))]
    pub waitables: BoundedVec<Waitable, MAX_WAITABLES>,
    /// Ready mask (bit per waitable)
    pub ready_mask: u64,
}

impl<T> Stream<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    /// Create a new stream
    pub fn new(handle: StreamHandle, element_type: ValType) -> wrt_error::Result<Self> {
        Ok(Self {
            handle,
            element_type,
            state: StreamState::Open,
            #[cfg(feature = "std")]
            buffer: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            buffer: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            readable_closed: false,
            writable_closed: false,
        })
    }

    /// Check if stream is readable
    pub fn is_readable(&self) -> bool {
        !self.readable_closed && (self.state == StreamState::Ready || !self.buffer.is_empty())
    }

    /// Check if stream is writable
    pub fn is_writable(&self) -> bool {
        !self.writable_closed && self.state != StreamState::Closed
    }

    /// Close readable end
    pub fn close_readable(&mut self) {
        self.readable_closed = true;
        if self.writable_closed {
            self.state = StreamState::Closed;
        }
    }

    /// Close writable end
    pub fn close_writable(&mut self) {
        self.writable_closed = true;
        if self.readable_closed {
            self.state = StreamState::Closed;
        }
    }
}

impl<T> Default for Stream<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    fn default() -> Self {
        #[cfg(feature = "std")]
        let buffer = Vec::new();
        #[cfg(not(any(feature = "std",)))]
        let buffer = Default::default();

        Self {
            handle: StreamHandle::default(),
            element_type: ValType::Bool,
            state: StreamState::Open,
            buffer,
            readable_closed: false,
            writable_closed: false,
        }
    }
}

impl<T> Future<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    /// Create a new future
    pub fn new(handle: FutureHandle, value_type: ValType) -> Self {
        Self {
            handle,
            value_type,
            state: FutureState::Pending,
            value: None,
            readable_closed: false,
            writable_closed: false,
        }
    }

    /// Check if future is readable
    pub fn is_readable(&self) -> bool {
        !self.readable_closed && self.state == FutureState::Ready
    }

    /// Check if future is writable
    pub fn is_writable(&self) -> bool {
        !self.writable_closed && self.state == FutureState::Pending
    }

    /// Set the future value
    pub fn set_value(&mut self, value: T) -> wrt_error::Result<()> {
        if self.state != FutureState::Pending {
            return Err(wrt_error::Error::runtime_execution_error(
                "Future already completed",
            ));
        }
        self.value = Some(value);
        self.state = FutureState::Ready;
        Ok(())
    }

    /// Cancel the future
    pub fn cancel(&mut self) {
        if self.state == FutureState::Pending {
            self.state = FutureState::Cancelled;
        }
    }
}

impl<T> Default for Future<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    fn default() -> Self {
        Self {
            handle: FutureHandle::default(),
            value_type: ValType::Bool,
            state: FutureState::Pending,
            value: None,
            readable_closed: false,
            writable_closed: false,
        }
    }
}

impl ErrorContext {
    /// Create a new error context
    #[cfg(feature = "std")]
    pub fn new(handle: ErrorContextHandle, message: String) -> Self {
        Self {
            handle,
            message,
            stack_trace: None,
            debug_info: DebugInfo::new().unwrap_or_default(),
        }
    }

    /// Create a new error context (no_std)
    #[cfg(not(any(feature = "std",)))]
    pub fn new(
        handle: ErrorContextHandle,
        message: BoundedString<1024>,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            handle,
            message,
            stack_trace: None,
            debug_info: DebugInfo::new()?,
        })
    }

    /// Get debug string representation
    #[cfg(feature = "std")]
    pub fn debug_string(&self) -> String {
        let mut result = self.message.clone();
        if let Some(trace) = &self.stack_trace {
            result.push_str("\nStack trace:\n");
            for frame in trace {
                result.push_str(&format!("  {}\n", frame.function));
            }
        }
        result
    }

    /// Get debug string representation
    #[cfg(not(any(feature = "std",)))]
    pub fn debug_string(&self) -> BoundedString<1024> {
        // In no_std, just return the message
        self.message.clone()
    }
}

impl DebugInfo {
    /// Create new debug info
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            source_component: None,
            error_code: None,
            #[cfg(feature = "std")]
            properties: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            properties: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
        })
    }

    /// Add a property
    #[cfg(feature = "std")]
    pub fn add_property(&mut self, key: String, value: ComponentValue<ComponentProvider>) {
        self.properties.push((key, value));
    }

    /// Add a property (no_std)
    #[cfg(not(any(feature = "std",)))]
    pub fn add_property(
        &mut self,
        key: BoundedString<64>,
        value: ComponentValue,
    ) -> wrt_error::Result<()> {
        self.properties
            .push((key, value))
            .map_err(|_| wrt_error::Error::runtime_execution_error("Failed to add property"))
    }
}

impl WaitableSet {
    /// Create a new waitable set
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            waitables: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            waitables: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            ready_mask: 0,
        })
    }

    /// Add a waitable to the set
    pub fn add(&mut self, waitable: Waitable) -> wrt_error::Result<u32> {
        let index = self.waitables.len();
        if index >= 64 {
            return Err(wrt_error::Error::runtime_execution_error(
                "Too many waitables",
            ));
        }

        #[cfg(feature = "std")]
        {
            self.waitables.push(waitable);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.waitables
                .push(waitable)
                .map_err(|_| wrt_error::Error::runtime_execution_error("Error occurred"))?;
        }

        Ok(index as u32)
    }

    /// Mark a waitable as ready
    pub fn mark_ready(&mut self, index: u32) {
        if index < 64 {
            self.ready_mask |= 1u64 << index;
        }
    }

    /// Check if any waitables are ready
    pub fn has_ready(&self) -> bool {
        self.ready_mask != 0
    }

    /// Get the first ready waitable index
    pub fn first_ready(&self) -> Option<u32> {
        if self.ready_mask == 0 { None } else { Some(self.ready_mask.trailing_zeros()) }
    }

    /// Clear ready state for a waitable
    pub fn clear_ready(&mut self, index: u32) {
        if index < 64 {
            self.ready_mask &= !(1u64 << index);
        }
    }
}

impl Default for StreamState {
    fn default() -> Self {
        Self::Open
    }
}

impl Default for FutureState {
    fn default() -> Self {
        Self::Pending
    }
}

impl Default for DebugInfo {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            source_component: None,
            error_code: None,
            #[cfg(feature = "std")]
            properties: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            properties: BoundedVec::new(),
        })
    }
}

impl Default for WaitableSet {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            #[cfg(feature = "std")]
            waitables: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            waitables: BoundedVec::new(),
            ready_mask: 0,
        })
    }
}

impl fmt::Display for StreamState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamState::Open => write!(f, "open"),
            StreamState::Ready => write!(f, "ready"),
            StreamState::Closed => write!(f, "closed"),
            StreamState::Error => write!(f, "error"),
        }
    }
}

impl fmt::Display for FutureState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FutureState::Pending => write!(f, "pending"),
            FutureState::Ready => write!(f, "ready"),
            FutureState::Cancelled => write!(f, "cancelled"),
            FutureState::Error => write!(f, "error"),
            FutureState::Failed => write!(f, "failed"),
        }
    }
}

impl fmt::Display for Waitable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Waitable::StreamReadable(h) => write!(f, "stream-readable({})", h.0),
            Waitable::StreamWritable(h) => write!(f, "stream-writable({})", h.0),
            Waitable::FutureReadable(h) => write!(f, "future-readable({})", h.0),
            Waitable::FutureWritable(h) => write!(f, "future-writable({})", h.0),
        }
    }
}
