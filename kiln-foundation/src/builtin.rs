// WRT - wrt-foundation
// Module: WebAssembly Component Model Built-in Types
// SW-REQ-ID: REQ_019
// SW-REQ-ID: REQ_020
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use core::{
    fmt,
    str::FromStr,
};

// Error types are imported through crate root
use crate::{
    bounded::BoundedVec,
    prelude::*,
    safe_memory::NoStdProvider,
    traits::{
        Checksummable,
        FromBytes,
        ReadStream,
        SerializationError,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
};

/// Maximum number of `BuiltinType` variants, used for `BoundedVec` capacity.
const MAX_BUILTIN_TYPES: usize = 35;

// Calculate a suitable capacity for the NoStdProvider.
// Each BuiltinType takes 1 byte (serialized_size).
// BoundedVec itself might have a small overhead, but provider capacity is for
// raw bytes.
const ALL_AVAILABLE_PROVIDER_CAPACITY: usize = MAX_BUILTIN_TYPES * 1; // BuiltinType::default().serialized_size() is 1

/// Enumeration of all supported WebAssembly Component Model built-in functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiltinType {
    // Resource built-ins (always available)
    /// Create a new resource
    ResourceCreate,
    /// Drop (destroy) a resource
    ResourceDrop,
    /// Get the representation of a resource
    ResourceRep,
    /// Get a handle to a resource
    ResourceGet,
    /// Create a new resource instance
    #[cfg(feature = "component-model-async")]
    ResourceNew,

    // Task built-ins (feature-gated)
    /// Yield execution of current task
    #[cfg(feature = "component-model-async")]
    TaskYield,
    /// Wait for a task to complete
    #[cfg(feature = "component-model-async")]
    TaskWait,
    /// Return a value from a task
    #[cfg(feature = "component-model-async")]
    TaskReturn,
    /// Poll a task for completion
    #[cfg(feature = "component-model-async")]
    TaskPoll,
    /// Apply backpressure to a task
    #[cfg(feature = "component-model-async")]
    TaskBackpressure,

    // Subtask built-ins (feature-gated)
    /// Drop a subtask
    #[cfg(feature = "component-model-async")]
    SubtaskDrop,

    // Stream built-ins (feature-gated)
    /// Create a new stream
    #[cfg(feature = "component-model-async")]
    StreamNew,
    /// Read from a stream
    #[cfg(feature = "component-model-async")]
    StreamRead,
    /// Write to a stream
    #[cfg(feature = "component-model-async")]
    StreamWrite,
    /// Cancel a read operation on a stream
    #[cfg(feature = "component-model-async")]
    StreamCancelRead,
    /// Cancel a write operation on a stream
    #[cfg(feature = "component-model-async")]
    StreamCancelWrite,
    /// Close the readable end of a stream
    #[cfg(feature = "component-model-async")]
    StreamCloseReadable,
    /// Close the writable end of a stream
    #[cfg(feature = "component-model-async")]
    StreamCloseWritable,

    // Future built-ins (feature-gated)
    /// Create a new future
    #[cfg(feature = "component-model-async")]
    FutureNew,
    /// Cancel a read operation on a future
    #[cfg(feature = "component-model-async")]
    FutureCancelRead,
    /// Cancel a write operation on a future
    #[cfg(feature = "component-model-async")]
    FutureCancelWrite,
    /// Close the readable end of a future
    #[cfg(feature = "component-model-async")]
    FutureCloseReadable,
    /// Close the writable end of a future
    #[cfg(feature = "component-model-async")]
    FutureCloseWritable,

    // Async built-ins (feature-gated)
    /// Create a new async value
    #[cfg(feature = "component-model-async")]
    AsyncNew,
    /// Get the value from an async value once resolved
    #[cfg(feature = "component-model-async")]
    AsyncGet,
    /// Poll an async value for completion
    #[cfg(feature = "component-model-async")]
    AsyncPoll,
    /// Wait for an async value to complete
    #[cfg(feature = "component-model-async")]
    AsyncWait,

    // Error Context built-ins (feature-gated)
    /// Create a new error context
    #[cfg(feature = "component-model-error-context")]
    ErrorNew,
    /// Get the trace from an error context
    #[cfg(feature = "component-model-error-context")]
    ErrorTrace,
    /// Create a new error context instance
    #[cfg(feature = "component-model-error-context")]
    ErrorContextNew,
    /// Drop an error context
    #[cfg(feature = "component-model-error-context")]
    ErrorContextDrop,
    /// Get debug message from error context
    #[cfg(feature = "component-model-error-context")]
    ErrorContextDebugMessage,

    // Threading built-ins (feature-gated)
    /// Spawn a new thread
    #[cfg(feature = "component-model-threading")]
    ThreadingSpawn,
    /// Join a thread (wait for its completion)
    #[cfg(feature = "component-model-threading")]
    ThreadingJoin,
    /// Create a synchronization primitive
    #[cfg(feature = "component-model-threading")]
    ThreadingSync,
}

impl Default for BuiltinType {
    fn default() -> Self {
        BuiltinType::ResourceCreate
    }
}

impl Checksummable for BuiltinType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        let discriminant_byte = match self {
            // Assign distinct, stable byte values for each variant
            BuiltinType::ResourceCreate => 0x01,
            BuiltinType::ResourceDrop => 0x02,
            BuiltinType::ResourceRep => 0x03,
            BuiltinType::ResourceGet => 0x04,
            #[cfg(feature = "component-model-async")]
            BuiltinType::ResourceNew => 0x05,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskYield => 0x06,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskWait => 0x07,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskReturn => 0x08,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskPoll => 0x09,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskBackpressure => 0x0A,
            #[cfg(feature = "component-model-async")]
            BuiltinType::SubtaskDrop => 0x0B,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamNew => 0x0C,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamRead => 0x0D,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamWrite => 0x0E,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCancelRead => 0x0F,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCancelWrite => 0x10,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCloseReadable => 0x11,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCloseWritable => 0x12,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureNew => 0x13,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCancelRead => 0x14,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCancelWrite => 0x15,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCloseReadable => 0x16,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCloseWritable => 0x17,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncNew => 0x18,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncGet => 0x19,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncPoll => 0x1A,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncWait => 0x1B,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorNew => 0x1C,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorTrace => 0x1D,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextNew => 0x1E,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextDrop => 0x1F,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextDebugMessage => 0x20,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingSpawn => 0x21,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingJoin => 0x22,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingSync => 0x23,
        };
        checksum.update_slice(&[discriminant_byte]); // Use update_slice for
                                                     // &[u8]
    }
}

impl ToBytes for BuiltinType {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        let byte_val = match self {
            BuiltinType::ResourceCreate => 0,
            BuiltinType::ResourceDrop => 1,
            BuiltinType::ResourceRep => 2,
            BuiltinType::ResourceGet => 3,
            #[cfg(feature = "component-model-async")]
            BuiltinType::ResourceNew => 4,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskYield => 5,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskWait => 6,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskReturn => 7,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskPoll => 8,
            #[cfg(feature = "component-model-async")]
            BuiltinType::TaskBackpressure => 9,
            #[cfg(feature = "component-model-async")]
            BuiltinType::SubtaskDrop => 10,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamNew => 11,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamRead => 12,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamWrite => 13,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCancelRead => 14,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCancelWrite => 15,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCloseReadable => 16,
            #[cfg(feature = "component-model-async")]
            BuiltinType::StreamCloseWritable => 17,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureNew => 18,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCancelRead => 19,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCancelWrite => 20,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCloseReadable => 21,
            #[cfg(feature = "component-model-async")]
            BuiltinType::FutureCloseWritable => 22,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncNew => 23,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncGet => 24,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncPoll => 25,
            #[cfg(feature = "component-model-async")]
            BuiltinType::AsyncWait => 26,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorNew => 27,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorTrace => 28,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextNew => 29,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextDrop => 30,
            #[cfg(feature = "component-model-error-context")]
            BuiltinType::ErrorContextDebugMessage => 31,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingSpawn => 32,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingJoin => 33,
            #[cfg(feature = "component-model-threading")]
            BuiltinType::ThreadingSync => 34,
        };
        writer.write_u8(byte_val).map_err(|e| e)
    }
}

impl FromBytes for BuiltinType {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let val = reader.read_u8()?;
        match val {
            0 => Ok(BuiltinType::ResourceCreate),
            1 => Ok(BuiltinType::ResourceDrop),
            2 => Ok(BuiltinType::ResourceRep),
            3 => Ok(BuiltinType::ResourceGet),
            #[cfg(feature = "component-model-async")]
            4 => Ok(BuiltinType::ResourceNew),
            #[cfg(feature = "component-model-async")]
            5 => Ok(BuiltinType::TaskYield),
            #[cfg(feature = "component-model-async")]
            6 => Ok(BuiltinType::TaskWait),
            #[cfg(feature = "component-model-async")]
            7 => Ok(BuiltinType::TaskReturn),
            #[cfg(feature = "component-model-async")]
            8 => Ok(BuiltinType::TaskPoll),
            #[cfg(feature = "component-model-async")]
            9 => Ok(BuiltinType::TaskBackpressure),
            #[cfg(feature = "component-model-async")]
            10 => Ok(BuiltinType::SubtaskDrop),
            #[cfg(feature = "component-model-async")]
            11 => Ok(BuiltinType::StreamNew),
            #[cfg(feature = "component-model-async")]
            12 => Ok(BuiltinType::StreamRead),
            #[cfg(feature = "component-model-async")]
            13 => Ok(BuiltinType::StreamWrite),
            #[cfg(feature = "component-model-async")]
            14 => Ok(BuiltinType::StreamCancelRead),
            #[cfg(feature = "component-model-async")]
            15 => Ok(BuiltinType::StreamCancelWrite),
            #[cfg(feature = "component-model-async")]
            16 => Ok(BuiltinType::StreamCloseReadable),
            #[cfg(feature = "component-model-async")]
            17 => Ok(BuiltinType::StreamCloseWritable),
            #[cfg(feature = "component-model-async")]
            18 => Ok(BuiltinType::FutureNew),
            #[cfg(feature = "component-model-async")]
            19 => Ok(BuiltinType::FutureCancelRead),
            #[cfg(feature = "component-model-async")]
            20 => Ok(BuiltinType::FutureCancelWrite),
            #[cfg(feature = "component-model-async")]
            21 => Ok(BuiltinType::FutureCloseReadable),
            #[cfg(feature = "component-model-async")]
            22 => Ok(BuiltinType::FutureCloseWritable),
            #[cfg(feature = "component-model-async")]
            23 => Ok(BuiltinType::AsyncNew),
            #[cfg(feature = "component-model-async")]
            24 => Ok(BuiltinType::AsyncGet),
            #[cfg(feature = "component-model-async")]
            25 => Ok(BuiltinType::AsyncPoll),
            #[cfg(feature = "component-model-async")]
            26 => Ok(BuiltinType::AsyncWait),
            #[cfg(feature = "component-model-error-context")]
            27 => Ok(BuiltinType::ErrorNew),
            #[cfg(feature = "component-model-error-context")]
            28 => Ok(BuiltinType::ErrorTrace),
            #[cfg(feature = "component-model-error-context")]
            29 => Ok(BuiltinType::ErrorContextNew),
            #[cfg(feature = "component-model-error-context")]
            30 => Ok(BuiltinType::ErrorContextDrop),
            #[cfg(feature = "component-model-error-context")]
            31 => Ok(BuiltinType::ErrorContextDebugMessage),
            #[cfg(feature = "component-model-threading")]
            32 => Ok(BuiltinType::ThreadingSpawn),
            #[cfg(feature = "component-model-threading")]
            33 => Ok(BuiltinType::ThreadingJoin),
            #[cfg(feature = "component-model-threading")]
            34 => Ok(BuiltinType::ThreadingSync),
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

/// Error returned when parsing a built-in type fails
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBuiltinError;

impl fmt::Display for ParseBuiltinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid built-in type name")
    }
}

impl FromStr for BuiltinType {
    type Err = ParseBuiltinError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            // Resource built-ins
            "resource.create" => Ok(Self::ResourceCreate),
            "resource.drop" => Ok(Self::ResourceDrop),
            "resource.rep" => Ok(Self::ResourceRep),
            "resource.get" => Ok(Self::ResourceGet),
            #[cfg(feature = "component-model-async")]
            "resource.new" => Ok(Self::ResourceNew),

            // Task built-ins
            #[cfg(feature = "component-model-async")]
            "task.yield" => Ok(Self::TaskYield),
            #[cfg(feature = "component-model-async")]
            "task.wait" => Ok(Self::TaskWait),
            #[cfg(feature = "component-model-async")]
            "task.return" => Ok(Self::TaskReturn),
            #[cfg(feature = "component-model-async")]
            "task.poll" => Ok(Self::TaskPoll),
            #[cfg(feature = "component-model-async")]
            "task.backpressure" => Ok(Self::TaskBackpressure),

            // Subtask built-ins
            #[cfg(feature = "component-model-async")]
            "subtask.drop" => Ok(Self::SubtaskDrop),

            // Stream built-ins
            #[cfg(feature = "component-model-async")]
            "stream.new" => Ok(Self::StreamNew),
            #[cfg(feature = "component-model-async")]
            "stream.read" => Ok(Self::StreamRead),
            #[cfg(feature = "component-model-async")]
            "stream.write" => Ok(Self::StreamWrite),
            #[cfg(feature = "component-model-async")]
            "stream.cancel-read" => Ok(Self::StreamCancelRead),
            #[cfg(feature = "component-model-async")]
            "stream.cancel-write" => Ok(Self::StreamCancelWrite),
            #[cfg(feature = "component-model-async")]
            "stream.close-readable" => Ok(Self::StreamCloseReadable),
            #[cfg(feature = "component-model-async")]
            "stream.close-writable" => Ok(Self::StreamCloseWritable),

            // Future built-ins
            #[cfg(feature = "component-model-async")]
            "future.new" => Ok(Self::FutureNew),
            #[cfg(feature = "component-model-async")]
            "future.cancel-read" => Ok(Self::FutureCancelRead),
            #[cfg(feature = "component-model-async")]
            "future.cancel-write" => Ok(Self::FutureCancelWrite),
            #[cfg(feature = "component-model-async")]
            "future.close-readable" => Ok(Self::FutureCloseReadable),
            #[cfg(feature = "component-model-async")]
            "future.close-writable" => Ok(Self::FutureCloseWritable),

            // Async built-ins
            #[cfg(feature = "component-model-async")]
            "async.new" => Ok(Self::AsyncNew),
            #[cfg(feature = "component-model-async")]
            "async.get" => Ok(Self::AsyncGet),
            #[cfg(feature = "component-model-async")]
            "async.poll" => Ok(Self::AsyncPoll),
            #[cfg(feature = "component-model-async")]
            "async.wait" => Ok(Self::AsyncWait),

            // Error Context built-ins
            #[cfg(feature = "component-model-error-context")]
            "error.new" => Ok(Self::ErrorNew),
            #[cfg(feature = "component-model-error-context")]
            "error.trace" => Ok(Self::ErrorTrace),
            #[cfg(feature = "component-model-error-context")]
            "error-context.new" => Ok(Self::ErrorContextNew),
            #[cfg(feature = "component-model-error-context")]
            "error-context.drop" => Ok(Self::ErrorContextDrop),
            #[cfg(feature = "component-model-error-context")]
            "error-context.debug-message" => Ok(Self::ErrorContextDebugMessage),

            // Threading built-ins
            #[cfg(feature = "component-model-threading")]
            "threading.spawn" => Ok(Self::ThreadingSpawn),
            #[cfg(feature = "component-model-threading")]
            "threading.join" => Ok(Self::ThreadingJoin),
            #[cfg(feature = "component-model-threading")]
            "threading.sync" => Ok(Self::ThreadingSync),

            // Unknown built-in
            _ => Err(ParseBuiltinError),
        }
    }
}

impl BuiltinType {
    /// Get the name of the built-in as a string
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            // Resource built-ins
            Self::ResourceCreate => "resource.create",
            Self::ResourceDrop => "resource.drop",
            Self::ResourceRep => "resource.rep",
            Self::ResourceGet => "resource.get",
            #[cfg(feature = "component-model-async")]
            Self::ResourceNew => "resource.new",

            // Task built-ins
            #[cfg(feature = "component-model-async")]
            Self::TaskYield => "task.yield",
            #[cfg(feature = "component-model-async")]
            Self::TaskWait => "task.wait",
            #[cfg(feature = "component-model-async")]
            Self::TaskReturn => "task.return",
            #[cfg(feature = "component-model-async")]
            Self::TaskPoll => "task.poll",
            #[cfg(feature = "component-model-async")]
            Self::TaskBackpressure => "task.backpressure",

            // Subtask built-ins
            #[cfg(feature = "component-model-async")]
            Self::SubtaskDrop => "subtask.drop",

            // Stream built-ins
            #[cfg(feature = "component-model-async")]
            Self::StreamNew => "stream.new",
            #[cfg(feature = "component-model-async")]
            Self::StreamRead => "stream.read",
            #[cfg(feature = "component-model-async")]
            Self::StreamWrite => "stream.write",
            #[cfg(feature = "component-model-async")]
            Self::StreamCancelRead => "stream.cancel-read",
            #[cfg(feature = "component-model-async")]
            Self::StreamCancelWrite => "stream.cancel-write",
            #[cfg(feature = "component-model-async")]
            Self::StreamCloseReadable => "stream.close-readable",
            #[cfg(feature = "component-model-async")]
            Self::StreamCloseWritable => "stream.close-writable",

            // Future built-ins
            #[cfg(feature = "component-model-async")]
            Self::FutureNew => "future.new",
            #[cfg(feature = "component-model-async")]
            Self::FutureCancelRead => "future.cancel-read",
            #[cfg(feature = "component-model-async")]
            Self::FutureCancelWrite => "future.cancel-write",
            #[cfg(feature = "component-model-async")]
            Self::FutureCloseReadable => "future.close-readable",
            #[cfg(feature = "component-model-async")]
            Self::FutureCloseWritable => "future.close-writable",

            // Async built-ins
            #[cfg(feature = "component-model-async")]
            Self::AsyncNew => "async.new",
            #[cfg(feature = "component-model-async")]
            Self::AsyncGet => "async.get",
            #[cfg(feature = "component-model-async")]
            Self::AsyncPoll => "async.poll",
            #[cfg(feature = "component-model-async")]
            Self::AsyncWait => "async.wait",

            // Error Context built-ins
            #[cfg(feature = "component-model-error-context")]
            Self::ErrorNew => "error.new",
            #[cfg(feature = "component-model-error-context")]
            Self::ErrorTrace => "error.trace",
            #[cfg(feature = "component-model-error-context")]
            Self::ErrorContextNew => "error-context.new",
            #[cfg(feature = "component-model-error-context")]
            Self::ErrorContextDrop => "error-context.drop",
            #[cfg(feature = "component-model-error-context")]
            Self::ErrorContextDebugMessage => "error-context.debug-message",

            // Threading built-ins
            #[cfg(feature = "component-model-threading")]
            Self::ThreadingSpawn => "threading.spawn",
            #[cfg(feature = "component-model-threading")]
            Self::ThreadingJoin => "threading.join",
            #[cfg(feature = "component-model-threading")]
            Self::ThreadingSync => "threading.sync",
        }
    }

    /// Parse a string into a built-in type (convenience method)
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }

    /// Check if this built-in is available in the current configuration
    #[must_use]
    pub fn is_available(&self) -> bool {
        match self {
            // Resource built-ins are always available
            Self::ResourceCreate | Self::ResourceDrop | Self::ResourceRep | Self::ResourceGet => {
                true
            },

            // Feature-gated built-ins
            #[cfg(feature = "component-model-async")]
            Self::ResourceNew
            | Self::TaskYield
            | Self::TaskWait
            | Self::TaskReturn
            | Self::TaskPoll
            | Self::TaskBackpressure
            | Self::SubtaskDrop
            | Self::StreamNew
            | Self::StreamRead
            | Self::StreamWrite
            | Self::StreamCancelRead
            | Self::StreamCancelWrite
            | Self::StreamCloseReadable
            | Self::StreamCloseWritable
            | Self::FutureNew
            | Self::FutureCancelRead
            | Self::FutureCancelWrite
            | Self::FutureCloseReadable
            | Self::FutureCloseWritable
            | Self::AsyncNew
            | Self::AsyncGet
            | Self::AsyncPoll
            | Self::AsyncWait => true,

            #[cfg(feature = "component-model-error-context")]
            Self::ErrorNew
            | Self::ErrorTrace
            | Self::ErrorContextNew
            | Self::ErrorContextDrop
            | Self::ErrorContextDebugMessage => true,

            #[cfg(feature = "component-model-threading")]
            Self::ThreadingSpawn | Self::ThreadingJoin | Self::ThreadingSync => true,

            // Built-ins that are conditionally compiled out are not available
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }

    /// Get all available built-in types in the current configuration
    #[must_use]
    pub fn all_available(
    ) -> BoundedVec<Self, MAX_BUILTIN_TYPES, NoStdProvider<ALL_AVAILABLE_PROVIDER_CAPACITY>> {
        let provider = NoStdProvider::<ALL_AVAILABLE_PROVIDER_CAPACITY>::default();
        let mut result: BoundedVec<Self, MAX_BUILTIN_TYPES, _> = BoundedVec::new(provider)
            .expect("Static BoundedVec init failed for BuiltinType::all_available");

        // These pushes are infallible as MAX_BUILTIN_TYPES is calculated to hold all
        // possible variants, and item_serialized_size (1) * MAX_BUILTIN_TYPES fits
        // provider capacity. BoundedVec::push returns Result<(), BoundedError>,
        // so use expect or proper handling.
        result.push(Self::ResourceCreate).expect("Static capacity push failed");
        result.push(Self::ResourceDrop).expect("Static capacity push failed");
        result.push(Self::ResourceRep).expect("Static capacity push failed");
        result.push(Self::ResourceGet).expect("Static capacity push failed");

        // Feature-gated built-ins
        #[cfg(feature = "component-model-async")]
        {
            result.push(Self::ResourceNew).expect("Static capacity push failed");
            result.push(Self::TaskYield).expect("Static capacity push failed");
            result.push(Self::TaskWait).expect("Static capacity push failed");
            result.push(Self::TaskReturn).expect("Static capacity push failed");
            result.push(Self::TaskPoll).expect("Static capacity push failed");
            result.push(Self::TaskBackpressure).expect("Static capacity push failed");
            result.push(Self::SubtaskDrop).expect("Static capacity push failed");
            result.push(Self::StreamNew).expect("Static capacity push failed");
            result.push(Self::StreamRead).expect("Static capacity push failed");
            result.push(Self::StreamWrite).expect("Static capacity push failed");
            result.push(Self::StreamCancelRead).expect("Static capacity push failed");
            result.push(Self::StreamCancelWrite).expect("Static capacity push failed");
            result.push(Self::StreamCloseReadable).expect("Static capacity push failed");
            result.push(Self::StreamCloseWritable).expect("Static capacity push failed");
            result.push(Self::FutureNew).expect("Static capacity push failed");
            result.push(Self::FutureCancelRead).expect("Static capacity push failed");
            result.push(Self::FutureCancelWrite).expect("Static capacity push failed");
            result.push(Self::FutureCloseReadable).expect("Static capacity push failed");
            result.push(Self::FutureCloseWritable).expect("Static capacity push failed");
            result.push(Self::AsyncNew).expect("Static capacity push failed");
            result.push(Self::AsyncGet).expect("Static capacity push failed");
            result.push(Self::AsyncPoll).expect("Static capacity push failed");
            result.push(Self::AsyncWait).expect("Static capacity push failed");
        }

        // Error Context built-ins
        #[cfg(feature = "component-model-error-context")]
        {
            result.push(Self::ErrorNew).expect("Static capacity push failed");
            result.push(Self::ErrorTrace).expect("Static capacity push failed");
            result.push(Self::ErrorContextNew).expect("Static capacity push failed");
            result.push(Self::ErrorContextDrop).expect("Static capacity push failed");
            result.push(Self::ErrorContextDebugMessage).expect("Static capacity push failed");
        }

        // Threading built-ins
        #[cfg(feature = "component-model-threading")]
        {
            result.push(Self::ThreadingSpawn).expect("Static capacity push failed");
            result.push(Self::ThreadingJoin).expect("Static capacity push failed");
            result.push(Self::ThreadingSync).expect("Static capacity push failed");
        }

        result
    }
}

impl fmt::Display for BuiltinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_name() {
        assert_eq!(BuiltinType::ResourceCreate.name(), "resource.create");
        assert_eq!(BuiltinType::ResourceDrop.name(), "resource.drop");
        assert_eq!(BuiltinType::ResourceRep.name(), "resource.rep");
        assert_eq!(BuiltinType::ResourceGet.name(), "resource.get");
    }

    #[test]
    fn test_builtin_from_str() {
        assert_eq!(
            BuiltinType::from_str("resource.create"),
            Ok(BuiltinType::ResourceCreate)
        );
        assert_eq!(
            BuiltinType::from_str("resource.drop"),
            Ok(BuiltinType::ResourceDrop)
        );
        assert_eq!(
            BuiltinType::from_str("resource.rep"),
            Ok(BuiltinType::ResourceRep)
        );
        assert_eq!(
            BuiltinType::from_str("resource.get"),
            Ok(BuiltinType::ResourceGet)
        );
        assert_eq!(
            BuiltinType::from_str("unknown.builtin"),
            Err(ParseBuiltinError)
        );

        // Also test the parse convenience method
        assert_eq!(
            BuiltinType::parse("resource.create"),
            Some(BuiltinType::ResourceCreate)
        );
        assert_eq!(BuiltinType::parse("unknown.builtin"), None);
    }

    #[test]
    fn test_builtin_is_available() {
        // Resource built-ins should always be available
        assert!(BuiltinType::ResourceCreate.is_available());
        assert!(BuiltinType::ResourceDrop.is_available());
        assert!(BuiltinType::ResourceRep.is_available());
        assert!(BuiltinType::ResourceGet.is_available());
    }

    #[test]
    fn test_all_available() {
        // Should at least contain the resource built-ins
        let available = BuiltinType::all_available();
        assert!(available.contains(&BuiltinType::ResourceCreate).unwrap());
        assert!(available.contains(&BuiltinType::ResourceDrop).unwrap());
        assert!(available.contains(&BuiltinType::ResourceRep).unwrap());
        assert!(available.contains(&BuiltinType::ResourceGet).unwrap());
    }
}
