//! Streaming Canonical ABI implementation for WebAssembly Component Model
//!
//! This module implements streaming operations for the canonical ABI, enabling
//! incremental processing of large data through streams with backpressure control.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{fmt, mem};

#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};

// Enable vec! macro for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{vec, boxed::Box};

#[cfg(not(feature = "std"))]
use wrt_foundation::{
    BoundedVec as Vec, 
    safe_memory::NoStdProvider,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use wrt_foundation::{
    bounded::{ BoundedString},
    prelude::*,
};

use crate::{
    async_types::{Stream, StreamHandle, StreamState, AsyncReadResult},
    canonical_options::CanonicalOptions,
    types::{ValType, Value},
};

use wrt_error::{Error, ErrorCategory, Result};

/// Maximum buffer size for streaming operations in no_std environments
const MAX_STREAM_BUFFER_SIZE: usize = 8192;

/// Maximum number of concurrent streams for no_std environments
const MAX_CONCURRENT_STREAMS: usize = 64;

/// Streaming canonical ABI implementation
#[derive(Debug)]
pub struct StreamingCanonicalAbi {
    /// Active streams
    #[cfg(feature = "std")]
    streams: Vec<StreamingContext>,
    #[cfg(not(any(feature = "std", )))]
    streams: BoundedVec<StreamingContext, MAX_CONCURRENT_STREAMS>,
    
    /// Buffer pool for reusing memory
    #[cfg(feature = "std")]
    buffer_pool: Vec<Vec<u8>>,
    #[cfg(not(any(feature = "std", )))]
    buffer_pool: BoundedVec<BoundedVec<u8, MAX_STREAM_BUFFER_SIZE>, 16>,
    
    /// Next stream ID
    next_stream_id: u32,
    
    /// Global backpressure configuration
    backpressure_config: BackpressureConfig,
}

/// Context for a streaming operation
#[derive(Debug, Clone)]
pub struct StreamingContext {
    /// Stream handle
    pub handle: StreamHandle,
    /// Element type being streamed
    pub element_type: ValType,
    /// Current buffer
    #[cfg(feature = "std")]
    pub buffer: Vec<u8>,
    #[cfg(not(any(feature = "std", )))]
    pub buffer: BoundedVec<u8, MAX_STREAM_BUFFER_SIZE>,
    /// Bytes read/written so far
    pub bytes_processed: u64,
    /// Stream direction
    pub direction: StreamDirection,
    /// Backpressure state
    pub backpressure: BackpressureState,
    /// Canonical options for this stream
    pub options: CanonicalOptions,
}

/// Stream direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    /// Reading from core WebAssembly to component
    Lifting,
    /// Writing from component to core WebAssembly
    Lowering,
    /// Bidirectional stream
    Bidirectional,
}

/// Backpressure state for flow control
#[derive(Debug, Clone)]
pub struct BackpressureState {
    /// Current buffer usage as percentage (0-100)
    pub buffer_usage_percent: u8,
    /// Whether backpressure is active
    pub is_active: bool,
    /// Number of bytes that can be processed before triggering backpressure
    pub available_capacity: usize,
    /// High water mark (trigger backpressure)
    pub high_water_mark: usize,
    /// Low water mark (release backpressure)
    pub low_water_mark: usize,
}

/// Global backpressure configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Default high water mark percentage (0-100)
    pub default_high_water_percent: u8,
    /// Default low water mark percentage (0-100)
    pub default_low_water_percent: u8,
    /// Maximum buffer size per stream
    pub max_buffer_size: usize,
    /// Enable adaptive backpressure
    pub adaptive_enabled: bool,
}

/// Result of a streaming operation
#[derive(Debug, Clone)]
pub enum StreamingResult {
    /// Operation completed successfully with data
    Success { 
        data: Vec<u8>, 
        bytes_processed: usize 
    },
    /// Operation is pending, more data needed
    Pending { 
        bytes_available: usize 
    },
    /// Backpressure active, consumer should slow down
    Backpressure { 
        retry_after_ms: u32 
    },
    /// Stream ended normally
    EndOfStream,
    /// Error occurred
    Error(Error),
}

/// Streaming lift operation result
#[derive(Debug, Clone)]
pub struct StreamingLiftResult {
    /// Lifted values (partial or complete)
    pub values: Vec<Value>,
    /// Bytes consumed from input
    pub bytes_consumed: usize,
    /// Whether more input is needed
    pub needs_more_input: bool,
    /// Backpressure recommendation
    pub backpressure_active: bool,
}

/// Streaming lower operation result
#[derive(Debug, Clone)]
pub struct StreamingLowerResult {
    /// Lowered bytes (partial or complete)
    pub bytes: Vec<u8>,
    /// Values consumed from input
    pub values_consumed: usize,
    /// Whether more input is needed
    pub needs_more_input: bool,
    /// Backpressure recommendation
    pub backpressure_active: bool,
}

impl StreamingCanonicalAbi {
    /// Create new streaming canonical ABI
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            streams: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            streams: BoundedVec::new(),
            
            #[cfg(feature = "std")]
            buffer_pool: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            buffer_pool: BoundedVec::new(),
            
            next_stream_id: 1,
            backpressure_config: BackpressureConfig::default(),
        })
    }

    /// Create a new streaming context
    pub fn create_stream(
        &mut self,
        element_type: ValType,
        direction: StreamDirection,
        options: CanonicalOptions,
    ) -> Result<StreamHandle> {
        let handle = StreamHandle(self.next_stream_id);
        self.next_stream_id += 1;

        let context = StreamingContext {
            handle,
            element_type,
            #[cfg(feature = "std")]
            buffer: self.get_buffer_from_pool(),
            #[cfg(not(any(feature = "std", )))]
            buffer: BoundedVec::new(),
            bytes_processed: 0,
            direction,
            backpressure: BackpressureState::new(&self.backpressure_config),
            options,
        };

        self.streams.push(context).map_err(|_| {
            Error::resource_exhausted("Failed to add new stream context, maximum concurrent streams reached")
            )
        })?;

        Ok(handle)
    }

    /// Perform streaming lift operation (core bytes -> component values)
    pub fn streaming_lift(
        &mut self,
        stream_handle: StreamHandle,
        input_bytes: &[u8],
    ) -> Result<StreamingLiftResult> {
        let stream_index = self.find_stream_index(stream_handle)?;
        let context = &mut self.streams[stream_index];

        // Check backpressure
        if context.backpressure.is_active {
            return Ok(StreamingLiftResult {
                values: Vec::new(),
                bytes_consumed: 0,
                needs_more_input: false,
                backpressure_active: true,
            };
        }

        // Add input to buffer
        let available_capacity = context.backpressure.available_capacity;
        let bytes_to_consume = input_bytes.len().min(available_capacity;
        
        #[cfg(feature = "std")]
        {
            context.buffer.extend_from_slice(&input_bytes[..bytes_to_consume];
        }
        #[cfg(not(any(feature = "std", )))]
        {
            for &byte in &input_bytes[..bytes_to_consume] {
                if context.buffer.push(byte).is_err() {
                    break;
                }
            }
        }

        // Try to parse values from buffer
        let (values, bytes_consumed) = self.parse_values_from_buffer(stream_index)?;
        
        // Update backpressure state
        context.update_backpressure_state);
        context.bytes_processed += bytes_consumed as u64;

        Ok(StreamingLiftResult {
            values,
            bytes_consumed,
            needs_more_input: context.buffer.len() < self.get_minimum_parse_size(&context.element_type),
            backpressure_active: context.backpressure.is_active,
        })
    }

    /// Perform streaming lower operation (component values -> core bytes)
    pub fn streaming_lower(
        &mut self,
        stream_handle: StreamHandle,
        input_values: &[Value],
    ) -> Result<StreamingLowerResult> {
        let stream_index = self.find_stream_index(stream_handle)?;
        let context = &mut self.streams[stream_index];

        // Check backpressure
        if context.backpressure.is_active {
            return Ok(StreamingLowerResult {
                bytes: Vec::new(),
                values_consumed: 0,
                needs_more_input: false,
                backpressure_active: true,
            };
        }

        // Serialize values to bytes
        let (bytes, values_consumed) = self.serialize_values_to_buffer(stream_index, input_values)?;
        
        // Update backpressure state
        context.update_backpressure_state);

        Ok(StreamingLowerResult {
            bytes,
            values_consumed,
            needs_more_input: false, // For now, assume all values can be processed
            backpressure_active: context.backpressure.is_active,
        })
    }

    /// Close a stream and release resources
    pub fn close_stream(&mut self, stream_handle: StreamHandle) -> Result<()> {
        let stream_index = self.find_stream_index(stream_handle)?;
        let context = self.streams.remove(stream_index;

        // Return buffer to pool if possible
        #[cfg(feature = "std")]
        {
            self.return_buffer_to_pool(context.buffer;
        }

        Ok(())
    }

    /// Get stream statistics
    pub fn get_stream_stats(&self, stream_handle: StreamHandle) -> Result<StreamStats> {
        let stream_index = self.find_stream_index(stream_handle)?;
        let context = &self.streams[stream_index];

        Ok(StreamStats {
            handle: stream_handle,
            bytes_processed: context.bytes_processed,
            buffer_size: context.buffer.len(),
            backpressure_active: context.backpressure.is_active,
            buffer_usage_percent: context.backpressure.buffer_usage_percent,
        })
    }

    /// Update backpressure configuration
    pub fn update_backpressure_config(&mut self, config: BackpressureConfig) {
        self.backpressure_config = config;
        
        // Update existing streams
        for context in self.streams.iter_mut() {
            context.backpressure.update_config(&self.backpressure_config;
        }
    }

    // Private helper methods

    fn find_stream_index(&self, handle: StreamHandle) -> Result<usize> {
        self.streams
            .iter()
            .position(|ctx| ctx.handle == handle)
            .ok_or_else(|| {
                Error::runtime_execution_error("Stream handle not found")
            })?
    }

    #[cfg(feature = "std")]
    fn get_buffer_from_pool(&mut self) -> Vec<u8> {
        self.buffer_pool.pop().unwrap_or_else(|| Vec::with_capacity(MAX_STREAM_BUFFER_SIZE))
    }

    #[cfg(feature = "std")]
    fn return_buffer_to_pool(&mut self, mut buffer: Vec<u8>) {
        buffer.clear);
        if buffer.capacity() <= MAX_STREAM_BUFFER_SIZE * 2 {
            self.buffer_pool.push(buffer);
        }
    }

    fn parse_values_from_buffer(&mut self, stream_index: usize) -> core::result::Result<(Vec<Value>, usize)> {
        let context = &self.streams[stream_index];
        
        // Simplified parsing - in real implementation would parse according to element type
        if context.buffer.len() >= 4 {
            let value = match context.element_type {
                ValType::U32 => {
                    let bytes = [context.buffer[0], context.buffer[1], context.buffer[2], context.buffer[3]];
                    Value::U32(u32::from_le_bytes(bytes))
                }
                ValType::String => {
                    // Simplified string parsing
                    if context.buffer.len() >= 8 {
                        let len = u32::from_le_bytes([context.buffer[0], context.buffer[1], context.buffer[2], context.buffer[3]]) as usize;
                        if context.buffer.len() >= 4 + len {
                            let string_bytes = &context.buffer[4..4 + len];
                            let string_content = core::str::from_utf8(string_bytes)
                                .map_err(|_| Error::parse_error("Invalid UTF-8"))?;
                            Value::String(BoundedString::try_from_str(string_content).unwrap_or_default())
                        } else {
                            return Ok((Vec::new(), 0)); // Need more data
                        }
                    } else {
                        return Ok((Vec::new(), 0)); // Need more data
                    }
                }
                _ => {
                    // Default case
                    Value::U32(42)
                }
            };
            
            // Remove parsed bytes from buffer
            let bytes_consumed = match context.element_type {
                ValType::String => {
                    if context.buffer.len() >= 8 {
                        let len = u32::from_le_bytes([context.buffer[0], context.buffer[1], context.buffer[2], context.buffer[3]]) as usize;
                        4 + len
                    } else {
                        0
                    }
                }
                _ => 4
            };
            
            if bytes_consumed > 0 {
                let values = vec![value];
                Ok((values, bytes_consumed))
            } else {
                Ok((Vec::new(), 0))
            }
        } else {
            Ok((Vec::new(), 0)) // Need more data
        }
    }

    fn serialize_values_to_buffer(&mut self, _stream_index: usize, values: &[Value]) -> core::result::Result<(Vec<u8>, usize)> {
        let mut result_bytes = Vec::new();
        let mut values_consumed = 0;

        for value in values {
            match value {
                Value::U32(n) => {
                    result_bytes.extend_from_slice(&n.to_le_bytes);
                    values_consumed += 1;
                }
                Value::String(s) => {
                    let string_bytes = s.as_str().as_bytes);
                    result_bytes.extend_from_slice(&(string_bytes.len() as u32).to_le_bytes);
                    result_bytes.extend_from_slice(string_bytes;
                    values_consumed += 1;
                }
                _ => {
                    // Simplified - just serialize as u32
                    result_bytes.extend_from_slice(&42u32.to_le_bytes);
                    values_consumed += 1;
                }
            }
        }

        Ok((result_bytes, values_consumed))
    }

    fn get_minimum_parse_size(&self, element_type: &ValType) -> usize {
        match element_type {
            ValType::U32 | ValType::S32 => 4,
            ValType::U64 | ValType::S64 => 8,
            ValType::String => 4, // At least length prefix
            _ => 4, // Default minimum
        }
    }
}

impl StreamingContext {
    /// Update backpressure state based on current buffer usage
    pub fn update_backpressure_state(&mut self) {
        let buffer_usage = (self.buffer.len() * 100) / self.backpressure.high_water_mark;
        self.backpressure.buffer_usage_percent = buffer_usage.min(100) as u8;
        
        if buffer_usage >= 100 && !self.backpressure.is_active {
            self.backpressure.is_active = true;
        } else if buffer_usage <= (self.backpressure.low_water_mark * 100 / self.backpressure.high_water_mark) && self.backpressure.is_active {
            self.backpressure.is_active = false;
        }

        self.backpressure.available_capacity = self.backpressure.high_water_mark.saturating_sub(self.buffer.len();
    }
}

impl BackpressureState {
    /// Create new backpressure state
    pub fn new(config: &BackpressureConfig) -> Self {
        let high_water_mark = (config.max_buffer_size * config.default_high_water_percent as usize) / 100;
        let low_water_mark = (config.max_buffer_size * config.default_low_water_percent as usize) / 100;

        Self {
            buffer_usage_percent: 0,
            is_active: false,
            available_capacity: high_water_mark,
            high_water_mark,
            low_water_mark,
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: &BackpressureConfig) {
        self.high_water_mark = (config.max_buffer_size * config.default_high_water_percent as usize) / 100;
        self.low_water_mark = (config.max_buffer_size * config.default_low_water_percent as usize) / 100;
    }
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            default_high_water_percent: 80,
            default_low_water_percent: 20,
            max_buffer_size: MAX_STREAM_BUFFER_SIZE,
            adaptive_enabled: true,
        }
    }
}

impl Default for StreamingCanonicalAbi {
    fn default() -> Self {
        // Use new() which properly handles allocation or panic in development
        // This ensures consistent memory management patterns
        Self::new().expect("StreamingCanonicalAbi allocation should not fail in default construction")
    }
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Stream handle
    pub handle: StreamHandle,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Current buffer size
    pub buffer_size: usize,
    /// Whether backpressure is active
    pub backpressure_active: bool,
    /// Buffer usage percentage
    pub buffer_usage_percent: u8,
}

impl fmt::Display for StreamDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamDirection::Lifting => write!(f, "lifting"),
            StreamDirection::Lowering => write!(f, "lowering"),
            StreamDirection::Bidirectional => write!(f, "bidirectional"),
        }
    }
}

impl fmt::Display for StreamStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Stream({}): {} bytes, buffer: {} bytes ({}%), backpressure: {}",
            self.handle.0,
            self.bytes_processed,
            self.buffer_size,
            self.buffer_usage_percent,
            if self.backpressure_active { "active" } else { "inactive" }
        )
    }
