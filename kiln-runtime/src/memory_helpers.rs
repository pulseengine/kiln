//! Helper extensions for working with `Arc<Memory>` in the WRT runtime
//!
//! This module provides extension traits to simplify working with `Arc<Memory>`
//! instances, reducing the need for explicit dereferencing and borrowing.

// Import Arc from appropriate source based on feature flags
// alloc is imported in lib.rs with proper feature gates

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    safe_memory::SafeStack,
    values::Value,
};

use crate::{
    prelude::*,
    Memory,
};

/// Extension trait for `Arc<Memory>` to simplify access to memory operations
pub trait ArcMemoryExt {
    /// Get the size of memory in pages
    fn size(&self) -> u32;

    /// Get the size of memory in bytes
    fn size_in_bytes(&self) -> usize;

    /// Get the peak memory usage in bytes
    fn peak_usage(&self) -> usize;

    /// Get the number of memory accesses performed
    fn access_count(&self) -> u64;

    /// Get a debug name for this memory, if any
    fn debug_name(&self) -> Option<&str>;

    /// Read bytes from memory using SafeStack - safer alternative to read_bytes
    fn read_bytes_safe(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<
        wrt_foundation::safe_memory::SafeStack<
            u8,
            1024,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    >;

    /// Read bytes from memory (legacy method, prefer read_bytes_safe)
    #[deprecated(
        since = "0.2.0",
        note = "Use read_bytes_safe instead for enhanced memory safety"
    )]
    fn read_exact(&self, offset: u32, len: u32) -> Result<Vec<u8>>;

    /// Write bytes to memory
    fn write_all(&self, offset: u32, bytes: &[u8]) -> Result<()>;

    /// Grow memory by a number of pages
    fn grow(&self, pages: u32) -> Result<u32>;

    /// Read a 32-bit integer from memory
    fn read_i32(&self, addr: u32) -> Result<i32>;

    /// Read a 64-bit integer from memory
    fn read_i64(&self, addr: u32) -> Result<i64>;

    /// Read a 32-bit float from memory
    fn read_f32(&self, addr: u32) -> Result<f32>;

    /// Read a 64-bit float from memory
    fn read_f64(&self, addr: u32) -> Result<f64>;

    /// Read an 8-bit signed integer from memory
    fn read_i8(&self, addr: u32) -> Result<i8>;

    /// Read an 8-bit unsigned integer from memory
    fn read_u8(&self, addr: u32) -> Result<u8>;

    /// Read a 16-bit signed integer from memory
    fn read_i16(&self, addr: u32) -> Result<i16>;

    /// Read a 16-bit unsigned integer from memory
    fn read_u16(&self, addr: u32) -> Result<u16>;

    /// Read a 32-bit unsigned integer from memory
    fn read_u32(&self, addr: u32) -> Result<u32>;

    /// Read a 64-bit unsigned integer from memory
    fn read_u64(&self, addr: u32) -> Result<u64>;

    /// Read a 128-bit vector from memory
    fn read_v128(&self, addr: u32) -> Result<[u8; 16]>;

    /// Write a 32-bit integer to memory
    fn write_i32(&self, addr: u32, value: i32) -> Result<()>;

    /// Write a 64-bit integer to memory
    fn write_i64(&self, addr: u32, value: i64) -> Result<()>;

    /// Write a 32-bit float to memory
    fn write_f32(&self, addr: u32, value: f32) -> Result<()>;

    /// Write a 64-bit float to memory
    fn write_f64(&self, addr: u32, value: f64) -> Result<()>;

    /// Write an 8-bit integer to memory
    fn write_i8(&self, addr: u32, value: i8) -> Result<()>;

    /// Write an 8-bit unsigned integer to memory
    fn write_u8(&self, addr: u32, value: u8) -> Result<()>;

    /// Write a 16-bit signed integer to memory
    fn write_i16(&self, addr: u32, value: i16) -> Result<()>;

    /// Write a 16-bit unsigned integer to memory
    fn write_u16(&self, addr: u32, value: u16) -> Result<()>;

    /// Write a 32-bit unsigned integer to memory
    fn write_u32(&self, addr: u32, value: u32) -> Result<()>;

    /// Write a 64-bit unsigned integer to memory
    fn write_u64(&self, addr: u32, value: u64) -> Result<()>;

    /// Write a 128-bit vector to memory
    fn write_v128(&self, addr: u32, value: [u8; 16]) -> Result<()>;

    /// Check alignment for memory access
    fn check_alignment(&self, offset: u32, access_size: u32, align: u32) -> Result<()>;

    /// Read standard WebAssembly value
    fn read_value(&self, addr: u32, value_type: wrt_foundation::types::ValueType) -> Result<Value>;

    /// Write standard WebAssembly value
    fn write_value(&self, addr: u32, value: Value) -> Result<()>;

    /// Initialize a region of memory from a data segment
    fn init(&self, dst: usize, data: &[u8], src: usize, size: usize) -> Result<()>;

    /// Read multiple WebAssembly values into a SafeStack
    ///
    /// This method provides a safer alternative to reading values into a `Vec`
    /// by using SafeStack, which includes memory verification.
    ///
    /// # Arguments
    ///
    /// * `addr` - The starting address to read from
    /// * `value_type` - The type of values to read
    /// * `count` - Number of values to read
    ///
    /// # Returns
    ///
    /// A SafeStack containing the read values
    ///
    /// # Errors
    ///
    /// Returns an error if the memory access is invalid
    fn read_values_as_safe_stack(
        &self,
        addr: u32,
        value_type: wrt_foundation::types::ValueType,
        count: usize,
    ) -> Result<
        wrt_foundation::safe_memory::SafeStack<
            Value,
            256,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    >;

    /// Write bytes to memory at the given offset
    fn write_via_callback(&self, offset: u32, buffer: &[u8]) -> Result<()>;

    /// Grow memory by the given number of pages
    fn grow_via_callback(&self, pages: u32) -> Result<u32>;
}

impl ArcMemoryExt for Arc<Memory> {
    fn size(&self) -> u32 {
        self.as_ref().size()
    }

    fn size_in_bytes(&self) -> usize {
        self.as_ref().size_in_bytes()
    }

    fn peak_usage(&self) -> usize {
        self.as_ref().peak_memory()
    }

    fn access_count(&self) -> u64 {
        self.as_ref().access_count()
    }

    fn debug_name(&self) -> Option<&str> {
        self.as_ref().debug_name()
    }

    fn read_bytes_safe(
        &self,
        offset: u32,
        len: u32,
    ) -> Result<
        wrt_foundation::safe_memory::SafeStack<
            u8,
            1024,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    > {
        // Early return for zero-length reads
        if len == 0 {
            let provider = wrt_foundation::safe_managed_alloc!(
                1024,
                wrt_foundation::budget_aware_provider::CrateId::Runtime
            )?;
            return wrt_foundation::safe_memory::SafeStack::new(provider);
        }

        // Get a memory-safe slice directly instead of creating a temporary buffer
        let safe_slice = self.as_ref().get_safe_slice(offset, len as usize)?;

        // Create a SafeStack from the verified slice data with appropriate verification
        // level
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;
        let mut safe_stack = wrt_foundation::safe_memory::SafeStack::new(provider)?;

        // Set verification level to match memory's level
        let verification_level = self.as_ref().verification_level();
        safe_stack.set_verification_level(verification_level);

        // Get data from the safe slice with integrity verification built in
        let data = safe_slice.data()?;

        // Add all bytes to the SafeStack
        {
            // Use push for each byte since extend_from_slice is not available
            for &byte in data {
                safe_stack.push(byte)?;
            }
        }

        // Perform final validation if high verification level is enabled
        if verification_level.should_verify(200) {
            // This would perform a redundant integrity check in a real
            // implementation but we'll let the SafeStack handle
            // that internally
        }

        // Return the verified buffer
        Ok(safe_stack)
    }

    fn read_exact(&self, offset: u32, len: u32) -> Result<Vec<u8>> {
        // Early return for zero-length reads
        if len == 0 {
            return Ok(Vec::new());
        }

        // Get a memory-safe slice directly instead of creating a temporary buffer
        let safe_slice = self.as_ref().get_safe_slice(offset, len as usize)?;

        // Get data from the safe slice with integrity verification built in
        let data = safe_slice.data()?;

        // Create a Vec from the verified slice data
        let mut buffer = Vec::new();
        for &byte in data {
            buffer.push(byte);
        }
        Ok(buffer)
    }

    fn write_all(&self, offset: u32, bytes: &[u8]) -> Result<()> {
        // Use the new thread-safe write method
        self.as_ref().write_shared(offset, bytes)
    }

    fn grow(&self, pages: u32) -> Result<u32> {
        // TODO: This is a design issue - the trait expects &self but grow_shared needs
        // &mut self For now, return an error indicating this operation is not
        // supported with Arc
        Err(Error::not_supported_unsupported_operation(
            "Memory growth not supported for Arc<Memory>, use direct Memory instance",
        ))
    }

    fn read_i32(&self, addr: u32) -> Result<i32> {
        self.as_ref().read_i32(addr)
    }

    fn read_i64(&self, addr: u32) -> Result<i64> {
        self.as_ref().read_i64(addr)
    }

    fn read_f32(&self, addr: u32) -> Result<f32> {
        self.as_ref().read_f32(addr)
    }

    fn read_f64(&self, addr: u32) -> Result<f64> {
        self.as_ref().read_f64(addr)
    }

    fn read_i8(&self, addr: u32) -> Result<i8> {
        self.as_ref().read_i8(addr)
    }

    fn read_u8(&self, addr: u32) -> Result<u8> {
        self.as_ref().read_u8(addr)
    }

    fn read_i16(&self, addr: u32) -> Result<i16> {
        self.as_ref().read_i16(addr)
    }

    fn read_u16(&self, addr: u32) -> Result<u16> {
        self.as_ref().read_u16(addr)
    }

    fn read_u32(&self, addr: u32) -> Result<u32> {
        self.as_ref().read_u32(addr)
    }

    fn read_u64(&self, addr: u32) -> Result<u64> {
        self.as_ref().read_u64(addr)
    }

    fn read_v128(&self, addr: u32) -> Result<[u8; 16]> {
        self.as_ref().read_v128(addr)
    }

    fn write_i32(&self, addr: u32, value: i32) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_i64(&self, addr: u32, value: i64) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_f32(&self, addr: u32, value: f32) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_bits().to_le_bytes())
    }

    fn write_f64(&self, addr: u32, value: f64) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_bits().to_le_bytes())
    }

    fn write_i8(&self, addr: u32, value: i8) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_u8(&self, addr: u32, value: u8) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_i16(&self, addr: u32, value: i16) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_u16(&self, addr: u32, value: u16) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_u32(&self, addr: u32, value: u32) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_u64(&self, addr: u32, value: u64) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value.to_le_bytes())
    }

    fn write_v128(&self, addr: u32, value: [u8; 16]) -> Result<()> {
        // Use thread-safe write method
        self.as_ref().write_shared(addr, &value)
    }

    fn check_alignment(&self, offset: u32, access_size: u32, align: u32) -> Result<()> {
        self.as_ref().check_alignment(offset, access_size, align)
    }

    fn read_value(&self, addr: u32, value_type: wrt_foundation::types::ValueType) -> Result<Value> {
        match value_type {
            wrt_foundation::types::ValueType::I32 => self.read_i32(addr).map(Value::I32),
            wrt_foundation::types::ValueType::I64 => self.read_i64(addr).map(Value::I64),
            wrt_foundation::types::ValueType::F32 => self
                .read_f32(addr)
                .map(|f| Value::F32(wrt_foundation::values::FloatBits32::from_float(f))),
            wrt_foundation::types::ValueType::F64 => self
                .read_f64(addr)
                .map(|f| Value::F64(wrt_foundation::values::FloatBits64::from_float(f))),
            // V128 doesn't exist in ValueType enum, so we'll handle it separately
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Unsupported value type",
            )),
        }
    }

    fn write_value(&self, addr: u32, value: Value) -> Result<()> {
        // Use thread-safe write method for different value types
        match value {
            Value::I32(v) => self.write_i32(addr, v),
            Value::I64(v) => self.write_i64(addr, v),
            Value::F32(v) => self.write_f32(addr, f32::from_bits(v.to_bits())),
            Value::F64(v) => self.write_f64(addr, f64::from_bits(v.to_bits())),
            Value::V128(v) => self.write_v128(addr, v.bytes),
            _ => Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Type,
                wrt_error::codes::TYPE_MISMATCH_ERROR,
                "Unsupported value type for memory write",
            )),
        }
    }

    fn init(&self, dst: usize, data: &[u8], src: usize, size: usize) -> Result<()> {
        // Create a safe slice of the source data for verification
        let src_data = if src < data.len() {
            let end = src.checked_add(size).ok_or_else(|| {
                wrt_error::Error::runtime_execution_error("Source bounds overflow")
            })?;

            if end <= data.len() {
                &data[src..end]
            } else {
                return Err(wrt_error::Error::new(
                    wrt_error::ErrorCategory::Memory,
                    wrt_error::codes::MEMORY_OUT_OF_BOUNDS,
                    "End bounds overflow",
                ));
            }
        } else if size == 0 {
            // Zero-sized init is always valid
            &[]
        } else {
            return Err(wrt_error::Error::runtime_execution_error(
                "Invalid source offset",
            ));
        };

        // Convert dst to u32 and write the data directly
        let dst_u32 = u32::try_from(dst).map_err(|_| {
            wrt_error::Error::new(
                wrt_error::ErrorCategory::Memory,
                wrt_error::codes::MEMORY_OUT_OF_BOUNDS,
                "Destination offset conversion failed",
            )
        })?;

        self.write_all(dst_u32, src_data)
    }

    /// Read multiple WebAssembly values into a SafeStack
    ///
    /// This method provides a safer alternative to reading values into a `Vec`
    /// by using SafeStack, which includes memory verification.
    ///
    /// # Arguments
    ///
    /// * `addr` - The starting address to read from
    /// * `value_type` - The type of values to read
    /// * `count` - Number of values to read
    ///
    /// # Returns
    ///
    /// A SafeStack containing the read values
    ///
    /// # Errors
    ///
    /// Returns an error if the memory access is invalid
    fn read_values_as_safe_stack(
        &self,
        addr: u32,
        value_type: wrt_foundation::types::ValueType,
        count: usize,
    ) -> Result<
        wrt_foundation::safe_memory::SafeStack<
            Value,
            256,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    > {
        // Create a SafeStack to store the values
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;
        let mut result = wrt_foundation::safe_memory::SafeStack::new(provider)?;

        // Set verification level to match memory's level
        let verification_level = self.as_ref().verification_level();
        result.set_verification_level(verification_level);

        // Calculate size of each value in bytes
        let value_size = match value_type {
            wrt_foundation::types::ValueType::I32 => 4,
            wrt_foundation::types::ValueType::I64 => 8,
            wrt_foundation::types::ValueType::F32 => 4,
            wrt_foundation::types::ValueType::F64 => 8,
            _ => {
                return Err(wrt_error::Error::runtime_execution_error(
                    "Unsupported value type",
                ))
            },
        };

        // Read each value safely
        for i in 0..count {
            let offset = addr.checked_add((i * value_size) as u32).ok_or_else(|| {
                wrt_error::Error::new(
                    wrt_error::ErrorCategory::Memory,
                    wrt_error::codes::MEMORY_OUT_OF_BOUNDS,
                    "Address overflow in read_values",
                )
            })?;

            let value = self.read_value(offset, value_type)?;
            result.push(value)?;
        }

        // Verify the final stack if high verification is enabled
        if verification_level.should_verify(200) {
            // This would perform a redundant integrity check in a real
            // implementation
        }

        Ok(result)
    }

    fn write_via_callback(&self, offset: u32, buffer: &[u8]) -> Result<()> {
        #[cfg(feature = "std")]
        {
            // Use internal Mutex or RwLock to provide thread-safe mutation
            // Clone and modify through interior mutability
            let mut current_buffer = self.buffer()?;
            let start = offset as usize;
            let end = start + buffer.len();

            if end > current_buffer.len() {
                return Err(Error::memory_error("Memory access out of bounds"));
            }

            // Update the memory through the mutex/lock mechanism in the Memory
            // implementation
            self.update_buffer(|mem_buffer| {
                for (i, &byte) in buffer.iter().enumerate() {
                    mem_buffer[start + i] = byte;
                }
                Ok(())
            })
        }

        #[cfg(not(feature = "std"))]
        {
            // For no_std, Arc<Memory> cannot provide mutable access without interior
            // mutability
            Err(Error::runtime_execution_error(
                "Arc<Memory> mutable access not available in no_std",
            ))
        }
    }

    fn grow_via_callback(&self, _pages: u32) -> Result<u32> {
        // Memory::grow_memory requires &mut self.
        // Arc<Memory> cannot provide &mut Memory without interior mutability
        // or Arc::get_mut, which this trait signature doesn't allow.
        Err(Error::new(
            ErrorCategory::Runtime,
            wrt_error::codes::UNSUPPORTED_OPERATION,
            "Memory growth not supported for Arc<Memory>",
        ))
    }
}

