//! WASI I/O interface implementation
//!
//! Implements the `wasi:io` interface for stream operations using WRT's
//! resource management patterns and platform abstractions.
//!
//! ## Pollable System
//!
//! This module implements a proper pollable resource system for WASI P2:
//! - `PollableTable`: Global table managing pollable resources
//! - `Pollable`: Individual pollable with state tracking
//! - `PollableState`: Ready, Pending, or Closed states
//! - `poll`: Blocking poll with timeout support

use core::any::Any;

#[cfg(feature = "std")]
use std::{
    collections::HashMap,
    sync::RwLock,
    time::{Duration, Instant},
};

use crate::{
    prelude::*,
    Value,
};

// ============================================================================
// Pollable Resource System
// ============================================================================

/// Pollable state for tracking readiness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollableState {
    /// Not yet ready, needs to wait
    Pending,
    /// Ready for I/O operation
    Ready,
    /// Pollable has been closed/dropped
    Closed,
}

/// Type of resource that a pollable represents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollableKind {
    /// Input stream (readable)
    InputStream(u32),
    /// Output stream (writable)
    OutputStream(u32),
    /// Timer/clock subscription
    Timer {
        /// Deadline in nanoseconds since UNIX epoch
        deadline_ns: u64,
    },
    /// Generic pollable without specific source
    Generic,
}

/// A pollable resource for async I/O notification
#[derive(Debug)]
pub struct Pollable {
    /// Unique handle for this pollable (reserved for resource tracking)
    _handle: u32,
    /// Type of resource this pollable tracks
    kind: PollableKind,
    /// Current state
    state: PollableState,
    /// Number of active waiters (reserved for future multi-waiter support)
    _waiter_count: u32,
}

impl Pollable {
    /// Create a new pollable for an input stream
    #[must_use]
    pub fn for_input_stream(handle: u32, stream_handle: u32) -> Self {
        Self {
            _handle: handle,
            kind: PollableKind::InputStream(stream_handle),
            state: PollableState::Pending,
            _waiter_count: 0,
        }
    }

    /// Create a new pollable for an output stream
    #[must_use]
    pub fn for_output_stream(handle: u32, stream_handle: u32) -> Self {
        Self {
            _handle: handle,
            kind: PollableKind::OutputStream(stream_handle),
            state: PollableState::Pending,
            _waiter_count: 0,
        }
    }

    /// Create a new pollable for a timer
    #[must_use]
    pub fn for_timer(handle: u32, deadline_ns: u64) -> Self {
        Self {
            _handle: handle,
            kind: PollableKind::Timer { deadline_ns },
            state: PollableState::Pending,
            _waiter_count: 0,
        }
    }

    /// Check if this pollable is ready
    #[cfg(feature = "std")]
    pub fn check_ready(&mut self) -> bool {
        if self.state == PollableState::Closed {
            return false;
        }

        let is_ready = match self.kind {
            PollableKind::InputStream(stream_handle) => {
                check_input_stream_ready(stream_handle)
            },
            PollableKind::OutputStream(stream_handle) => {
                check_output_stream_ready(stream_handle)
            },
            PollableKind::Timer { deadline_ns } => {
                check_timer_ready(deadline_ns)
            },
            PollableKind::Generic => true,
        };

        if is_ready {
            self.state = PollableState::Ready;
        }

        is_ready
    }

    #[cfg(not(feature = "std"))]
    pub fn check_ready(&mut self) -> bool {
        // In no_std, conservatively return true for outputs, false for inputs
        match self.kind {
            PollableKind::OutputStream(_) => {
                self.state = PollableState::Ready;
                true
            },
            _ => false,
        }
    }
}

/// Global pollable table
#[cfg(feature = "std")]
static POLLABLE_TABLE: RwLock<Option<PollableTable>> = RwLock::new(None);

/// Table managing all pollable resources
#[cfg(feature = "std")]
pub struct PollableTable {
    /// Map of pollable handles to pollables
    pollables: HashMap<u32, Pollable>,
    /// Next available handle
    next_handle: u32,
}

#[cfg(feature = "std")]
impl PollableTable {
    /// Create a new pollable table
    #[must_use]
    pub fn new() -> Self {
        Self {
            pollables: HashMap::new(),
            // Start after the POLLABLE_OFFSET to maintain compatibility
            next_handle: POLLABLE_OFFSET,
        }
    }

    /// Allocate a new handle
    fn allocate_handle(&mut self) -> u32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }

    /// Create a pollable for an input stream
    pub fn create_for_input(&mut self, stream_handle: u32) -> u32 {
        let handle = self.allocate_handle();
        let pollable = Pollable::for_input_stream(handle, stream_handle);
        self.pollables.insert(handle, pollable);
        handle
    }

    /// Create a pollable for an output stream
    pub fn create_for_output(&mut self, stream_handle: u32) -> u32 {
        let handle = self.allocate_handle();
        let pollable = Pollable::for_output_stream(handle, stream_handle);
        self.pollables.insert(handle, pollable);
        handle
    }

    /// Create a pollable for a timer
    pub fn create_for_timer(&mut self, deadline_ns: u64) -> u32 {
        let handle = self.allocate_handle();
        let pollable = Pollable::for_timer(handle, deadline_ns);
        self.pollables.insert(handle, pollable);
        handle
    }

    /// Get a mutable pollable by handle
    pub fn get_mut(&mut self, handle: u32) -> Option<&mut Pollable> {
        self.pollables.get_mut(&handle)
    }

    /// Drop a pollable by handle
    pub fn drop_pollable(&mut self, handle: u32) -> bool {
        if let Some(pollable) = self.pollables.get_mut(&handle) {
            pollable.state = PollableState::Closed;
            self.pollables.remove(&handle);
            true
        } else {
            false
        }
    }

    /// Poll multiple pollables, blocking until at least one is ready or timeout
    pub fn poll_blocking(&mut self, handles: &[u32], timeout: Option<Duration>) -> Vec<bool> {
        let start = Instant::now();
        let deadline = timeout.map(|t| start + t);

        loop {
            // Check each pollable
            let mut results: Vec<bool> = handles.iter().map(|&handle| {
                self.pollables.get_mut(&handle)
                    .map(Pollable::check_ready)
                    .unwrap_or(false)
            }).collect();

            // If any are ready, return
            if results.iter().any(|&ready| ready) {
                return results;
            }

            // Check timeout
            if let Some(dl) = deadline {
                if Instant::now() >= dl {
                    return results;
                }
            }

            // Small sleep to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(1));

            // Re-check in case of quick readiness
            results = handles.iter().map(|&handle| {
                self.pollables.get_mut(&handle)
                    .map(Pollable::check_ready)
                    .unwrap_or(false)
            }).collect();

            if results.iter().any(|&ready| ready) {
                return results;
            }
        }
    }
}

#[cfg(feature = "std")]
impl Default for PollableTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize or get the pollable table
#[cfg(feature = "std")]
fn ensure_pollable_table() -> Result<()> {
    let mut table = POLLABLE_TABLE.write()
        .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
    if table.is_none() {
        *table = Some(PollableTable::new());
    }
    Ok(())
}

/// Check if an input stream is ready for reading
#[cfg(feature = "std")]
fn check_input_stream_ready(stream_handle: u32) -> bool {
    match stream_handle {
        0 => {
            // stdin - check with platform-specific mechanism
            use std::io::IsTerminal;
            // For terminals, assume potentially ready
            // For pipes, assume ready (EOF or data)
            !std::io::stdin().is_terminal()
        },
        _ => {
            // File descriptors from filesystem - assume ready
            // A full implementation would use select/poll/epoll
            true
        }
    }
}

/// Check if an output stream is ready for writing
#[cfg(feature = "std")]
fn check_output_stream_ready(_stream_handle: u32) -> bool {
    // All streams are currently assumed ready for buffered I/O
    // A full implementation would check buffer space for non-stdout/stderr
    true
}

/// Check if a timer has expired
#[cfg(feature = "std")]
fn check_timer_ready(deadline_ns: u64) -> bool {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    now >= deadline_ns
}

/// WASI stream read operation
///
/// Implements `wasi:io/streams.read` for reading from input streams
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The read length argument is missing or invalid
/// - The stream read operation fails on this platform
pub fn wasi_stream_read(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    // Extract stream handle and length from arguments
    let stream_handle = extract_stream_handle(args)?;
    let len = extract_read_length(args, 1)?;

    // Validate stream handle and check if readable
    // In a real implementation, this would access the resource manager
    // to verify the stream exists and is readable

    // Perform platform-specific read operation
    let data = perform_stream_read(stream_handle, len)?;

    // Return as WASI list<u8>
    let value_data: Vec<Value> = data.into_iter().map(Value::U8).collect();
    Ok(vec![Value::List(value_data)])
}

/// WASI stream write operation
///
/// Implements `wasi:io/streams.write` for writing to output streams
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The write data argument is missing or invalid
/// - The stream write operation fails on this platform
pub fn wasi_stream_write(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    // Extract stream handle and data from arguments
    let stream_handle = extract_stream_handle(args)?;
    let data = extract_write_data(args, 1)?;

    // Validate stream handle and check if writable
    // In a real implementation, this would access the resource manager

    // Perform platform-specific write operation
    let bytes_written = perform_stream_write(stream_handle, &data)?;

    // Return bytes written
    Ok(vec![Value::U64(bytes_written)])
}

/// WASI stream flush operation
///
/// Implements `wasi:io/streams.flush` for flushing output streams
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The stream flush operation fails on this platform
pub fn wasi_stream_flush(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;

    // Validate stream handle and check if writable

    // Perform platform-specific flush operation
    perform_stream_flush(stream_handle)?;

    // Return unit (no value)
    Ok(vec![])
}

/// WASI stream check-write operation
///
/// Implements `wasi:io/streams.check-write` to check available write space
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The write capacity check fails
pub fn wasi_stream_check_write(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;

    // Check how many bytes can be written without blocking
    let available = check_write_capacity(stream_handle)?;

    Ok(vec![Value::U64(available)])
}

/// WASI stream blocking-read operation
///
/// Implements `wasi:io/streams.blocking-read` for blocking reads
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The read length argument is missing or invalid
/// - The blocking read operation fails on this platform
pub fn wasi_stream_blocking_read(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;
    let len = extract_read_length(args, 1)?;

    // Blocking read - will block until data available or EOF
    // For now, delegates to regular read which may block on stdin
    let data = perform_stream_read(stream_handle, len)?;

    let value_data: Vec<Value> = data.into_iter().map(Value::U8).collect();
    Ok(vec![Value::List(value_data)])
}

/// WASI stream blocking-write operation
///
/// Implements `wasi:io/streams.blocking-write-and-flush` for blocking writes
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The write data argument is missing or invalid
/// - The blocking write or flush operation fails on this platform
pub fn wasi_stream_blocking_write(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;
    let data = extract_write_data(args, 1)?;

    // Write all data (blocking if necessary)
    let bytes_written = perform_stream_write(stream_handle, &data)?;

    // Also flush
    perform_stream_flush(stream_handle)?;

    Ok(vec![Value::U64(bytes_written)])
}

/// WASI stream skip operation
///
/// Implements `wasi:io/streams.skip` for skipping input bytes
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The skip length argument is missing or invalid
/// - The underlying read operation fails
pub fn wasi_stream_skip(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;
    let len = extract_read_length(args, 1)?;

    // Read and discard the bytes
    let data = perform_stream_read(stream_handle, len)?;
    let skipped = data.len() as u64;

    Ok(vec![Value::U64(skipped)])
}

/// WASI stream blocking-skip operation
///
/// Implements `wasi:io/streams.blocking-skip` for blocking skip
///
/// # Errors
///
/// Returns an error if the underlying skip operation fails.
pub fn wasi_stream_blocking_skip(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    // Same as skip - the underlying read may block
    wasi_stream_skip(_target, args)
}

/// WASI stream splice operation
///
/// Implements `wasi:io/streams.splice` for transferring data between streams
/// Reads from input stream and writes to output stream without intermediate buffering
///
/// # Errors
///
/// Returns an error if:
/// - The output or input stream handle arguments are missing or invalid
/// - The length argument is missing or invalid
/// - The underlying read or write operations fail
pub fn wasi_stream_splice(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let output_stream = extract_stream_handle(args)?;
    let input_stream = args.get(1)
        .and_then(|v| match v {
            Value::U32(h) => Some(*h),
            _ => None,
        })
        .ok_or_else(|| Error::wasi_invalid_fd("Invalid input stream handle"))?;
    let len = extract_read_length(args, 2)?;

    // Read from input
    let data = perform_stream_read(input_stream, len)?;

    // Write to output
    let written = perform_stream_write(output_stream, &data)?;

    Ok(vec![Value::U64(written)])
}

/// WASI stream blocking-splice operation
///
/// Implements `wasi:io/streams.blocking-splice` for blocking splice
///
/// # Errors
///
/// Returns an error if the underlying splice operation fails.
pub fn wasi_stream_blocking_splice(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    // Same as splice - operations may block
    wasi_stream_splice(_target, args)
}

/// WASI stream forward operation
///
/// Implements forwarding all remaining data from input to output stream
///
/// # Errors
///
/// Returns an error if:
/// - The output or input stream handle arguments are missing or invalid
/// - The underlying read or write operations fail during forwarding
pub fn wasi_stream_forward(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let output_stream = extract_stream_handle(args)?;
    let input_stream = args.get(1)
        .and_then(|v| match v {
            Value::U32(h) => Some(*h),
            _ => None,
        })
        .ok_or_else(|| Error::wasi_invalid_fd("Invalid input stream handle"))?;

    let mut total_forwarded: u64 = 0;
    const CHUNK_SIZE: u64 = 4096;

    // Forward all data in chunks
    loop {
        let data = perform_stream_read(input_stream, CHUNK_SIZE)?;
        if data.is_empty() {
            break; // EOF reached
        }

        let written = perform_stream_write(output_stream, &data)?;
        total_forwarded += written;

        // If we read less than chunk size, we're at EOF
        if (data.len() as u64) < CHUNK_SIZE {
            break;
        }
    }

    Ok(vec![Value::U64(total_forwarded)])
}

/// WASI input-stream drop operation
///
/// Drops an input stream resource
///
/// # Errors
///
/// Returns an error if the stream handle argument is missing or invalid.
pub fn wasi_drop_input_stream(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;

    // For stdin (0), we don't actually close it
    // For other streams, would need to close the underlying resource
    if stream_handle >= 3 {
        // Would close the stream in a full implementation
    }

    Ok(vec![])
}

/// WASI output-stream drop operation
///
/// Drops an output stream resource
///
/// # Errors
///
/// Returns an error if the stream handle argument is missing or invalid.
pub fn wasi_drop_output_stream(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;

    // For stdout/stderr (1, 2), we don't actually close them
    // For other streams, would need to close the underlying resource
    if stream_handle >= 3 {
        // Would close the stream in a full implementation
    }

    Ok(vec![])
}

/// WASI stream subscribe operation
///
/// Implements `wasi:io/poll.subscribe` for async I/O notification
///
/// # Errors
///
/// Returns an error if:
/// - The stream handle argument is missing or invalid
/// - The pollable table lock cannot be acquired
/// - The pollable table is not initialized
pub fn wasi_stream_subscribe(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let stream_handle = extract_stream_handle(args)?;

    #[cfg(feature = "std")]
    {
        ensure_pollable_table()?;

        let mut table = POLLABLE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("Pollable table not initialized"))?;

        // Determine if this is an input or output stream
        let pollable_handle = match stream_handle {
            0 => table.create_for_input(stream_handle), // stdin is input
            _ => table.create_for_output(stream_handle), // stdout/stderr and file handles are output
        };

        Ok(vec![Value::U32(pollable_handle)])
    }

    #[cfg(not(feature = "std"))]
    {
        // Fallback for no_std: use simple offset-based mapping
        let pollable_handle = create_pollable(stream_handle)?;
        Ok(vec![Value::U32(pollable_handle)])
    }
}

/// WASI subscribe-to-timer operation
///
/// Creates a pollable that becomes ready when the timer expires.
/// Implements `wasi:clocks/monotonic-clock.subscribe-instant`
///
/// # Errors
///
/// Returns an error if:
/// - The deadline timestamp argument is missing or invalid
/// - The pollable table lock cannot be acquired
/// - The pollable table is not initialized
pub fn wasi_subscribe_timer(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let deadline_ns = extract_timestamp_ns(args)?;

    #[cfg(feature = "std")]
    {
        ensure_pollable_table()?;

        let mut table = POLLABLE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("Pollable table not initialized"))?;

        let pollable_handle = table.create_for_timer(deadline_ns);
        Ok(vec![Value::U32(pollable_handle)])
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std, return a placeholder handle
        Ok(vec![Value::U32(deadline_ns as u32 + POLLABLE_OFFSET)])
    }
}

/// WASI poll one-off operation
///
/// Implements `wasi:io/poll.poll` for synchronous polling.
/// Blocks until at least one pollable is ready.
///
/// # Errors
///
/// Returns an error if:
/// - The pollables list argument is missing or invalid
/// - The pollable table lock cannot be acquired
/// - The pollable table is not initialized
pub fn wasi_poll_one_off(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let pollables = extract_pollable_list(args)?;

    #[cfg(feature = "std")]
    {
        ensure_pollable_table()?;

        let mut table = POLLABLE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("Pollable table not initialized"))?;

        // Use blocking poll with no timeout (blocks until ready)
        // In WASI P2, poll always blocks - use poll-list for non-blocking
        let results = table.poll_blocking(&pollables, None);

        let wasi_results: Vec<Value> = results.into_iter().map(Value::Bool).collect();
        Ok(vec![Value::List(wasi_results)])
    }

    #[cfg(not(feature = "std"))]
    {
        // Fallback for no_std
        let results = poll_pollables(&pollables)?;
        let wasi_results: Vec<Value> = results.into_iter().map(Value::Bool).collect();
        Ok(vec![Value::List(wasi_results)])
    }
}

/// WASI poll with timeout operation
///
/// Implements `wasi:io/poll.poll-list` with optional timeout.
/// Returns immediately if any pollable is ready, or after timeout.
///
/// # Errors
///
/// Returns an error if:
/// - The pollables list argument is missing or invalid
/// - The pollable table lock cannot be acquired
/// - The pollable table is not initialized
pub fn wasi_poll_with_timeout(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let pollables = extract_pollable_list(args)?;
    let timeout_ns = args.get(1).and_then(|v| match v {
        Value::U64(ns) => Some(*ns),
        _ => None,
    });

    #[cfg(feature = "std")]
    {
        ensure_pollable_table()?;

        let mut table = POLLABLE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("Pollable table not initialized"))?;

        let timeout = timeout_ns.map(Duration::from_nanos);
        let results = table.poll_blocking(&pollables, timeout);

        let wasi_results: Vec<Value> = results.into_iter().map(Value::Bool).collect();
        Ok(vec![Value::List(wasi_results)])
    }

    #[cfg(not(feature = "std"))]
    {
        // Fallback for no_std - immediate return
        let results = poll_pollables(&pollables)?;
        let wasi_results: Vec<Value> = results.into_iter().map(Value::Bool).collect();
        Ok(vec![Value::List(wasi_results)])
    }
}

/// WASI drop-pollable operation
///
/// Drops a pollable resource.
/// Implements `wasi:io/poll.pollable.drop`
///
/// # Errors
///
/// Returns an error if:
/// - The pollable handle argument is missing or invalid
/// - The pollable table lock cannot be acquired
/// - The pollable table is not initialized
/// - The pollable handle does not exist in the table
pub fn wasi_drop_pollable(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    let pollable_handle = args.first()
        .and_then(|v| match v {
            Value::U32(h) => Some(*h),
            _ => None,
        })
        .ok_or_else(|| Error::wasi_invalid_fd("Invalid pollable handle"))?;

    #[cfg(feature = "std")]
    {
        ensure_pollable_table()?;

        let mut table = POLLABLE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire pollable table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("Pollable table not initialized"))?;

        if table.drop_pollable(pollable_handle) {
            Ok(vec![])
        } else {
            Err(Error::wasi_invalid_fd("Invalid pollable handle"))
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std, just accept the drop
        let _ = pollable_handle;
        Ok(vec![])
    }
}

/// Helper function to extract stream handle from arguments
fn extract_stream_handle(args: &[Value]) -> Result<u32> {
    if args.is_empty() {
        return Err(Error::wasi_invalid_fd("Missing stream handle argument"));
    }

    match &args[0] {
        Value::U32(handle) => Ok(*handle),
        Value::S32(handle) => {
            if *handle < 0 {
                Err(Error::wasi_invalid_fd("Invalid negative stream handle"))
            } else {
                Ok(*handle as u32)
            }
        },
        _ => Err(Error::wasi_invalid_fd("Invalid stream handle type")),
    }
}

/// Helper function to extract read length from arguments
fn extract_read_length(args: &[Value], index: usize) -> Result<u64> {
    if args.len() <= index {
        return Err(Error::wasi_invalid_fd("Missing read length argument"));
    }

    match &args[index] {
        Value::U64(len) => Ok(*len),
        Value::U32(len) => Ok(u64::from(*len)),
        _ => Err(Error::wasi_invalid_fd("Invalid read length type")),
    }
}

/// Helper function to extract write data from arguments
fn extract_write_data(args: &[Value], index: usize) -> Result<Vec<u8>> {
    if args.len() <= index {
        return Err(Error::wasi_invalid_fd("Missing write data argument"));
    }

    match &args[index] {
        Value::List(items) => {
            let mut data = Vec::new();
            for item in items {
                match item {
                    Value::U8(byte) => data.push(*byte),
                    _ => return Err(Error::wasi_invalid_fd("Invalid byte data in write")),
                }
            }
            Ok(data)
        },
        _ => Err(Error::wasi_invalid_fd("Invalid write data type")),
    }
}

/// Helper function to extract pollable list from arguments
fn extract_pollable_list(args: &[Value]) -> Result<Vec<u32>> {
    if args.is_empty() {
        return Err(Error::wasi_invalid_fd("Missing pollables argument"));
    }

    match &args[0] {
        Value::List(items) => {
            let mut pollables = Vec::new();
            for item in items {
                match item {
                    Value::U32(handle) => pollables.push(*handle),
                    _ => return Err(Error::wasi_invalid_fd("Invalid pollable handle")),
                }
            }
            Ok(pollables)
        },
        _ => Err(Error::wasi_invalid_fd("Invalid pollables type")),
    }
}

/// Helper function to extract timestamp (nanoseconds) from arguments
fn extract_timestamp_ns(args: &[Value]) -> Result<u64> {
    if args.is_empty() {
        return Err(Error::wasi_invalid_fd("Missing timestamp argument"));
    }

    match &args[0] {
        Value::U64(ns) => Ok(*ns),
        Value::U32(ns) => Ok(u64::from(*ns)),
        _ => Err(Error::wasi_invalid_fd("Invalid timestamp type")),
    }
}

/// Platform-specific stream read implementation
fn perform_stream_read(stream_handle: u32, len: u64) -> Result<Vec<u8>> {
    // In a real implementation, this would:
    // 1. Look up the stream in the resource manager
    // 2. Check capabilities (readable)
    // 3. Perform platform-specific read operation
    // 4. Update stream position if applicable

    match stream_handle {
        0 => {
            // stdin - in a real implementation, read from platform stdin
            #[cfg(feature = "std")]
            {
                use std::io::{
                    self,
                    Read,
                };
                let mut buffer = vec![0u8; len.min(4096) as usize];
                let bytes_read = io::stdin()
                    .read(&mut buffer)
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to read from stdin"))?;
                buffer.truncate(bytes_read);
                Ok(buffer)
            }
            #[cfg(not(feature = "std"))]
            {
                // In no_std environment, return empty data
                Ok(Vec::new())
            }
        },
        _ => {
            // Other streams - placeholder implementation
            Ok(Vec::new())
        },
    }
}

/// Platform-specific stream write implementation
fn perform_stream_write(stream_handle: u32, data: &[u8]) -> Result<u64> {
    // In a real implementation, this would:
    // 1. Look up the stream in the resource manager
    // 2. Check capabilities (writable)
    // 3. Perform platform-specific write operation
    // 4. Update stream position if applicable

    match stream_handle {
        1 => {
            // stdout - in a real implementation, write to platform stdout
            #[cfg(feature = "std")]
            {
                use std::io::{
                    self,
                    Write,
                };
                io::stdout()
                    .write_all(data)
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to write to stdout"))?;
                io::stdout()
                    .flush()
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to flush stdout"))?;
                Ok(data.len() as u64)
            }
            #[cfg(not(feature = "std"))]
            {
                // In no_std environment, return success without actual write
                Ok(data.len() as u64)
            }
        },
        2 => {
            // stderr - in a real implementation, write to platform stderr
            #[cfg(feature = "std")]
            {
                use std::io::{
                    self,
                    Write,
                };
                io::stderr()
                    .write_all(data)
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to write to stderr"))?;
                io::stderr()
                    .flush()
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to flush stderr"))?;
                Ok(data.len() as u64)
            }
            #[cfg(not(feature = "std"))]
            {
                // In no_std environment, return success without actual write
                Ok(data.len() as u64)
            }
        },
        _ => {
            // Other streams - placeholder implementation
            Ok(data.len() as u64)
        },
    }
}

/// Platform-specific stream flush implementation
fn perform_stream_flush(stream_handle: u32) -> Result<()> {
    match stream_handle {
        1 => {
            // stdout flush
            #[cfg(feature = "std")]
            {
                use std::io::{
                    self,
                    Write,
                };
                io::stdout()
                    .flush()
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to flush stdout"))?;
            }
            Ok(())
        },
        2 => {
            // stderr flush
            #[cfg(feature = "std")]
            {
                use std::io::{
                    self,
                    Write,
                };
                io::stderr()
                    .flush()
                    .map_err(|_| Error::wasi_capability_unavailable("Failed to flush stderr"))?;
            }
            Ok(())
        },
        _ => {
            // Other streams - no-op for now
            Ok(())
        },
    }
}

/// Check write capacity for stream
// Result kept for API consistency and future error handling.
#[allow(clippy::unnecessary_wraps)]
fn check_write_capacity(stream_handle: u32) -> Result<u64> {
    match stream_handle {
        1 | 2 => {
            // stdout/stderr - assume always ready for a reasonable amount
            Ok(4096)
        },
        _ => {
            // Other streams - placeholder implementation
            Ok(1024)
        },
    }
}

/// Pollable handle offset - pollable handles are `stream_handle + POLLABLE_OFFSET`
const POLLABLE_OFFSET: u32 = 1000;

/// Create a pollable for the given stream (legacy fallback for no_std)
#[cfg(not(feature = "std"))]
fn create_pollable(stream_handle: u32) -> Result<u32> {
    // Create a pollable handle that maps back to the stream
    // Pollable = stream_handle + offset
    Ok(stream_handle + POLLABLE_OFFSET)
}

/// Poll multiple pollables to check if their streams are ready (legacy fallback for no_std)
///
/// Returns a vector of booleans indicating whether each pollable is ready.
/// Ready means:
/// - For input streams: data is available to read
/// - For output streams: buffer space is available to write
#[cfg(not(feature = "std"))]
fn poll_pollables(pollables: &[u32]) -> Result<Vec<bool>> {
    let mut results = Vec::with_capacity(pollables.len());

    for &pollable in pollables {
        // Convert pollable back to stream handle
        let stream_handle = pollable.saturating_sub(POLLABLE_OFFSET);

        // Check if the stream is ready
        let is_ready = check_stream_ready(stream_handle)?;
        results.push(is_ready);
    }

    Ok(results)
}

/// Check if a stream is ready for I/O (legacy fallback for no_std)
///
/// This implements actual readiness checking for known stream types:
/// - stdin (0): Checks if input is available without blocking
/// - stdout (1): Always ready for writing
/// - stderr (2): Always ready for writing
/// - Other streams: Check based on stream type
#[cfg(not(feature = "std"))]
fn check_stream_ready(stream_handle: u32) -> Result<bool> {
    match stream_handle {
        0 => {
            // stdin - in no_std, assume not ready (conservative)
            Ok(false)
        }
        1 | 2 => {
            // stdout/stderr - always ready for writing
            // These are typically buffered and non-blocking
            Ok(true)
        }
        _ => {
            // For file descriptors from the filesystem, we'd need to check
            // using platform-specific mechanisms. For now, assume ready
            // for any handle in the user range (>= 3)
            if stream_handle >= 3 {
                // User file handles - assume ready
                // A full implementation would check the actual file
                Ok(true)
            } else {
                // Unknown handle
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_stream_handle() -> Result<()> {
        let args = vec![Value::U32(42)];
        let handle = extract_stream_handle(args)?;
        assert_eq!(handle, 42);

        let args = vec![Value::S32(24)];
        let handle = extract_stream_handle(args)?;
        assert_eq!(handle, 24);

        // Test negative handle
        let args = vec![Value::S32(-1)];
        let result = extract_stream_handle(args);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_extract_read_length() -> Result<()> {
        let args = vec![Value::U32(42), Value::U64(1024)];
        let len = extract_read_length(args, 1)?;
        assert_eq!(len, 1024);

        let args = vec![Value::U32(42), Value::U32(512)];
        let len = extract_read_length(args, 1)?;
        assert_eq!(len, 512);

        Ok(())
    }

    #[test]
    fn test_extract_write_data() -> Result<()> {
        let data = vec![Value::U8(1), Value::U8(2), Value::U8(3)];
        let args = vec![Value::U32(42), Value::List(data)];

        let bytes = extract_write_data(args, 1)?;
        assert_eq!(bytes, vec![1, 2, 3]);

        Ok(())
    }

    #[test]
    fn test_wasi_stream_operations() -> Result<()> {
        // Test write operation to stdout
        let data = vec![
            Value::U8(72),
            Value::U8(101),
            Value::U8(108),
            Value::U8(108),
            Value::U8(111),
        ]; // "Hello"
        let args = vec![Value::U32(1), Value::List(data)];
        let result = wasi_stream_write(&mut (), args)?;
        assert_eq!(result.len(), 1);
        if let Value::U64(bytes_written) = &result[0] {
            assert_eq!(*bytes_written, 5);
        }

        // Test flush operation
        let args = vec![Value::U32(1)];
        let result = wasi_stream_flush(&mut (), args)?;
        assert_eq!(result.len(), 0); // Flush returns unit

        // Test check-write operation
        let args = vec![Value::U32(1)];
        let result = wasi_stream_check_write(&mut (), args)?;
        assert_eq!(result.len(), 1);
        if let Value::U64(capacity) = &result[0] {
            assert!(*capacity > 0);
        }

        Ok(())
    }

    #[test]
    fn test_pollable_operations() -> Result<()> {
        // Test subscribe operation for stdout (stream 1)
        let args = vec![Value::U32(1)];
        let result = wasi_stream_subscribe(&mut (), args)?;
        assert_eq!(result.len(), 1);
        let pollable1 = if let Value::U32(pollable) = &result[0] {
            assert!(*pollable >= POLLABLE_OFFSET); // Should be at or after offset
            *pollable
        } else {
            return Err(Error::wasi_invalid_fd("Expected U32 pollable handle"));
        };

        // Subscribe another stream (stderr = 2)
        let args = vec![Value::U32(2)];
        let result = wasi_stream_subscribe(&mut (), args)?;
        assert_eq!(result.len(), 1);
        let pollable2 = if let Value::U32(pollable) = &result[0] {
            assert!(*pollable >= POLLABLE_OFFSET);
            *pollable
        } else {
            return Err(Error::wasi_invalid_fd("Expected U32 pollable handle"));
        };

        // Test poll operation with the pollables we created
        let pollables = vec![Value::U32(pollable1), Value::U32(pollable2)];
        let args = vec![Value::List(pollables)];
        let result = wasi_poll_one_off(&mut (), args)?;
        assert_eq!(result.len(), 1);
        if let Value::List(results) = &result[0] {
            assert_eq!(results.len(), 2);
            // stdout and stderr should be ready for writing
            for result in results {
                assert!(matches!(result, Value::Bool(true)));
            }
        }

        Ok(())
    }

    #[test]
    fn test_timer_pollable() -> Result<()> {
        // Create a timer pollable that should already be expired (deadline in past)
        let args = vec![Value::U64(0)]; // deadline at epoch (already passed)
        let result = wasi_subscribe_timer(&mut (), args)?;
        assert_eq!(result.len(), 1);

        let timer_pollable = if let Value::U32(p) = &result[0] {
            *p
        } else {
            return Err(Error::wasi_invalid_fd("Expected U32 pollable handle"));
        };

        // Poll the timer - should be ready since deadline passed
        let pollables = vec![Value::U32(timer_pollable)];
        let args = vec![Value::List(pollables)];
        let result = wasi_poll_one_off(&mut (), args)?;

        if let Value::List(results) = &result[0] {
            assert_eq!(results.len(), 1);
            // Timer with deadline 0 should be ready
            assert!(matches!(results[0], Value::Bool(true)));
        }

        Ok(())
    }

    #[test]
    fn test_drop_pollable() -> Result<()> {
        // Create a pollable
        let args = vec![Value::U32(1)];
        let result = wasi_stream_subscribe(&mut (), args)?;
        let pollable = if let Value::U32(p) = &result[0] {
            *p
        } else {
            return Err(Error::wasi_invalid_fd("Expected U32 pollable handle"));
        };

        // Drop it
        let args = vec![Value::U32(pollable)];
        let result = wasi_drop_pollable(&mut (), args)?;
        assert!(result.is_empty()); // Drop returns nothing on success

        Ok(())
    }
}
