// WRT - wrt-decoder
// Module: Memory-Optimized Parsing Utilities
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Memory-optimized parsing utilities for WebAssembly binary format
//!
//! This module provides zero-allocation and minimal-allocation parsing
//! functions that work across std, no_std+alloc, and pure no_std environments.

use core::str;

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::safe_memory::{MemoryProvider, SafeSlice};

use crate::prelude::read_leb128_u32;

/// Memory pool for reusing vectors during parsing
pub struct MemoryPool<P: MemoryProvider> {
    /// Pool of instruction vectors for reuse
    #[cfg(feature = "std")]
    instruction_pools: crate::prelude::Vec<crate::prelude::Vec<u8>>,
    /// Pool of string buffers for reuse
    #[cfg(feature = "std")]
    string_pools: crate::prelude::Vec<crate::prelude::Vec<u8>>,
    /// Memory provider for no_std environments
    #[allow(dead_code)]
    provider: P,
}

impl<P: MemoryProvider + Default> Default for MemoryPool<P> {
    fn default() -> Self {
        Self::new(P::default())
    }
}

impl<P: MemoryProvider> MemoryPool<P> {
    /// Create a new memory pool
    pub fn new(provider: P) -> Self {
        Self {
            #[cfg(feature = "std")]
            instruction_pools: alloc::vec::Vec::with_capacity(0),
            #[cfg(feature = "std")]
            string_pools: alloc::vec::Vec::with_capacity(0),
            provider,
        }
    }

    /// Get a reusable vector for instructions
    #[cfg(feature = "std")]
    pub fn get_instruction_vector(&mut self) -> alloc::vec::Vec<u8> {
        self.instruction_pools
            .pop()
            .unwrap_or_else(|| alloc::vec::Vec::with_capacity(0))
    }

    /// Return a vector to the instruction pool
    #[cfg(feature = "std")]
    pub fn return_instruction_vector(&mut self, mut vec: alloc::vec::Vec<u8>) {
        vec.clear();
        if vec.capacity() <= 1024 {
            // Don't pool overly large vectors
            self.instruction_pools.push(vec);
        }
    }

    /// Get a reusable vector for string operations
    #[cfg(feature = "std")]
    pub fn get_string_buffer(&mut self) -> alloc::vec::Vec<u8> {
        self.string_pools.pop().unwrap_or_default()
    }

    /// Return a vector to the string pool
    #[cfg(feature = "std")]
    pub fn return_string_buffer(&mut self, mut vec: alloc::vec::Vec<u8>) {
        vec.clear();
        if vec.capacity() <= 256 {
            // Don't pool overly large vectors
            self.string_pools.push(vec);
        }
    }
}

/// Binary std/no_std choice
pub fn validate_utf8_slice(slice: &SafeSlice) -> Result<()> {
    let data = slice
        .data()
        .map_err(|_| Error::runtime_execution_error("Failed to access slice data"))?;

    str::from_utf8(data).map_err(|_| {
        Error::new(
            ErrorCategory::Parse,
            codes::INVALID_UTF8_ENCODING,
            "Invalid UTF8 encoding",
        )
    })?;
    Ok(())
}

/// Binary std/no_std choice
pub fn parse_string_inplace<'a>(
    slice: &'a SafeSlice<'a>,
    offset: usize,
) -> Result<(&'a str, usize)> {
    let data = slice.data().map_err(|_| Error::parse_error("Failed to access slice data"))?;

    if offset >= data.len() {
        return Err(Error::parse_error("Offset beyond slice boundary"));
    }

    let (length, new_offset) = read_leb128_u32(data, offset)?;

    if new_offset + length as usize > data.len() {
        return Err(Error::parse_error("String length exceeds available data"));
    }

    let string_bytes = &data[new_offset..new_offset + length as usize];
    let string_str = str::from_utf8(string_bytes)
        .map_err(|_| Error::runtime_execution_error("Invalid UTF8 in string"))?;

    Ok((string_str, new_offset + length as usize))
}

/// Copy string to target buffer only when necessary
pub fn copy_string_to_buffer(source: &str, buffer: &mut [u8]) -> Result<usize> {
    let bytes = source.as_bytes();
    if bytes.len() > buffer.len() {
        return Err(Error::parse_error("Buffer too small for string"));
    }

    buffer[..bytes.len()].copy_from_slice(bytes);
    Ok(bytes.len())
}

/// Binary std/no_std choice
pub struct StreamingCollectionParser<'a> {
    #[allow(dead_code)]
    slice: &'a SafeSlice<'a>,
    offset: usize,
    count: u32,
    processed: u32,
}

impl<'a> StreamingCollectionParser<'a> {
    /// Create a new streaming parser for a collection
    pub fn new(slice: &'a SafeSlice<'a>, offset: usize) -> Result<Self> {
        let data = slice.data().map_err(|_| Error::parse_error("Failed to access slice data"))?;

        let (count, new_offset) = read_leb128_u32(data, offset)?;

        Ok(Self {
            slice,
            offset: new_offset,
            count,
            processed: 0,
        })
    }

    /// Get the total count of items
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get the current offset
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Advance the offset
    pub fn advance_offset(&mut self, new_offset: usize) {
        self.offset = new_offset;
        self.processed += 1;
    }

    /// Check if there are more items to process
    pub fn has_more(&self) -> bool {
        self.processed < self.count
    }

    /// Get the remaining item count
    pub fn remaining(&self) -> u32 {
        self.count - self.processed
    }
}

/// Binary std/no_std choice
#[cfg(feature = "std")]
pub struct ModuleArena {
    buffer: crate::prelude::Vec<u8>,
    offset: usize,
}

#[cfg(feature = "std")]
impl ModuleArena {
    /// Create a new arena with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: {
                #[cfg(feature = "std")]
                {
                    alloc::vec::Vec::with_capacity(capacity)
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider =
                        crate::prelude::create_decoder_provider::<4096>().unwrap_or_default();
                    let mut vec = crate::prelude::DecoderVec::new(provider).unwrap_or_default();
                    // Pre-allocate by pushing zeros up to capacity
                    for _ in 0..capacity.min(4096) {
                        let _ = vec.push(0);
                    }
                    vec
                }
            },
            offset: 0,
        }
    }

    /// Allocate space in the arena
    pub fn allocate(&mut self, size: usize) -> Option<&mut [u8]> {
        if self.offset + size > self.buffer.capacity() {
            return None;
        }

        // Ensure buffer has enough actual length
        if self.buffer.len() < self.offset + size {
            self.buffer.resize(self.offset + size, 0);
        }

        let slice = &mut self.buffer[self.offset..self.offset + size];
        self.offset += size;
        Some(slice)
    }

    /// Reset the arena for reuse
    pub fn reset(&mut self) {
        self.offset = 0;
        self.buffer.clear();
    }
}

/// Bounded iterator for safe collection processing
pub struct BoundedIterator<'a, T> {
    items: &'a [T],
    index: usize,
    max_items: usize,
}

impl<'a, T> BoundedIterator<'a, T> {
    /// Create a new bounded iterator
    pub fn new(items: &'a [T], max_items: usize) -> Self {
        Self {
            items,
            index: 0,
            max_items,
        }
    }
}

impl<'a, T> Iterator for BoundedIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.items.len() || self.index >= self.max_items {
            None
        } else {
            let item = &self.items[self.index];
            self.index += 1;
            Some(item)
        }
    }
}

/// Memory-efficient bounds checking
pub fn check_bounds_u32(value: u32, max_value: u32, _context: &str) -> Result<()> {
    if value > max_value {
        Err(Error::parse_error("Bounds check failed"))
    } else {
        Ok(())
    }
}

/// Safe usize conversion with bounds checking
pub fn safe_usize_conversion(value: u32, _context: &str) -> Result<usize> {
    if value as usize as u32 != value {
        Err(Error::parse_error("Integer overflow in usize conversion"))
    } else {
        Ok(value as usize)
    }
}
