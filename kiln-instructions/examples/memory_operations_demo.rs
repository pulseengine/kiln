// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Demonstration of memory operations bridging between wrt-instructions and
//! wrt-runtime
//!
//! This example shows how the MemoryOperations trait implementation allows
//! WebAssembly memory instructions to work directly with the runtime Memory
//! implementation.
//!
//! This example requires std/alloc features.

#![cfg(any(feature = "std"))]

use wrt_error::Result;
use wrt_instructions::{
    memory_ops::{
        MemoryCopy,
        MemoryFill,
        MemoryLoad,
        MemoryOperations,
        MemoryStore,
    },
    prelude::Value,
};

// Mock memory implementation for demonstration
#[derive(Debug)]
pub struct MockMemory {
    data: Vec<u8>,
}

impl MockMemory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }
}

impl MemoryOperations for MockMemory {
    fn read_bytes(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        let start = offset as usize;
        let end = start + len as usize;
        if end > self.data.len() {
            return Err(wrt_error::Error::memory_error("Read out of bounds"));
        }
        Ok(self.data[start..end].to_vec())
    }

    fn write_bytes(&mut self, offset: u64, bytes: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + bytes.len();

        // Extend data if necessary
        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        // Copy bytes
        self.data[start..end].copy_from_slice(bytes);
        Ok(())
    }

    fn size_in_bytes(&self) -> Result<u64> {
        Ok(self.data.len() as u64)
    }

    fn grow(&mut self, bytes: u64) -> Result<()> {
        let new_size = self.data.len() + bytes as usize;
        self.data.resize(new_size, 0);
        Ok(())
    }

    fn fill(&mut self, offset: u64, value: u8, size: u64) -> Result<()> {
        let start = offset as usize;
        let end = start + size as usize;

        // Extend data if necessary
        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        // Fill with value
        for i in start..end {
            self.data[i] = value;
        }
        Ok(())
    }

    fn copy(&mut self, dest: u64, src: u64, size: u64) -> Result<()> {
        if dest == src || size == 0 {
            return Ok(());
        }

        let dest_start = dest as usize;
        let src_start = src as usize;
        let copy_size = size as usize;

        // Extend data if necessary
        let max_end = core::cmp::max(dest_start + copy_size, src_start + copy_size);
        if max_end > self.data.len() {
            self.data.resize(max_end, 0);
        }

        // Use Vec's copy_within for safe overlapping copy
        if dest_start < src_start {
            // Copy forward
            for i in 0..copy_size {
                self.data[dest_start + i] = self.data[src_start + i];
            }
        } else {
            // Copy backward
            for i in (0..copy_size).rev() {
                self.data[dest_start + i] = self.data[src_start + i];
            }
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    // Initialize global memory system first
    wrt_foundation::memory_init::MemoryInitializer::initialize()
        .expect("Failed to initialize memory system");

    println!("WebAssembly Memory Operations Demo");
    println!("==================================");

    // Create a mock memory instance
    let mut memory = MockMemory::new(1024);

    // Demonstrate i32 store and load operations
    println!("\n1. Testing i32 store and load:");

    // Create a store operation: store i32 value at offset 0
    let store_op = MemoryStore::i32(0, 4); // offset=0, align=4

    // Store the value 0x12345678 at address 0
    store_op.execute(&mut memory, &Value::I32(0), &Value::I32(0x12345678))?;
    println!("  Stored 0x12345678 at address 0");

    // Create a load operation: load i32 value from offset 0
    let load_op = MemoryLoad::i32_legacy(0, 4); // offset=0, align=4

    // Load the value from address 0
    let loaded_value = load_op.execute(&memory, &Value::I32(0))?;
    println!("  Loaded value: {:?}", loaded_value);

    // Demonstrate memory fill operation
    println!("\n2. Testing memory fill:");

    let fill_op = MemoryFill::new(0); // memory_index=0
    fill_op.execute(
        &mut memory,
        &Value::I32(100),
        &Value::I32(0xAB),
        &Value::I32(10),
    )?;
    println!("  Filled 10 bytes with 0xAB starting at address 100");

    // Verify the fill by reading back
    let read_result = memory.read_bytes(100, 10)?;
    println!("  Read back: {:02x?}", read_result);

    // Demonstrate memory copy operation
    println!("\n3. Testing memory copy:");

    let copy_op = MemoryCopy::new(0, 0); // same memory_index for source and destination
    copy_op.execute(
        &mut memory,
        &Value::I32(200),
        &Value::I32(100),
        &Value::I32(5),
    )?;
    println!("  Copied 5 bytes from address 100 to address 200");

    // Verify the copy by reading back
    let copy_result = memory.read_bytes(200, 5)?;
    println!("  Copied data: {:02x?}", copy_result);

    // Demonstrate different data types
    println!("\n4. Testing different data types:");

    // f32 operations
    let f32_store = MemoryStore::f32(300, 4);
    f32_store.execute(
        &mut memory,
        &Value::I32(0),
        &Value::F32(wrt_foundation::FloatBits32::from_float(3.14159)),
    )?;

    let f32_load = MemoryLoad::f32(300, 4);
    let f32_value = f32_load.execute(&memory, &Value::I32(0))?;
    println!("  f32 value: {:?}", f32_value);

    // i64 operations
    let i64_store = MemoryStore::i64(400, 8);
    i64_store.execute(&mut memory, &Value::I32(0), &Value::I64(0x123456789ABCDEF0))?;

    let i64_load = MemoryLoad::i64(400, 8);
    let i64_value = i64_load.execute(&memory, &Value::I32(0))?;
    println!("  i64 value: {:?}", i64_value);

    // Demonstrate memory growth
    println!("\n5. Testing memory growth:");
    let old_size = memory.size_in_bytes()?;
    println!("  Original size: {} bytes", old_size);

    memory.grow(512)?; // Grow by 512 bytes
    let new_size = memory.size_in_bytes()?;
    println!("  New size after growth: {} bytes", new_size);

    println!("\n✓ All memory operations completed successfully!");
    println!("✓ The MemoryOperations trait successfully bridges wrt-instructions and wrt-runtime!");

    // Memory cleanup happens automatically via RAII
    println!("\nMemory operations demo completed successfully!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_operations_integration() -> Result<()> {
        let mut memory = MockMemory::new(1024);

        // Test basic store/load cycle
        let store = MemoryStore::i32(0, 4);
        store.execute(&mut memory, &Value::I32(0), &Value::I32(42))?;

        let load = MemoryLoad::i32_legacy(0, 4);
        let result = load.execute(&memory, &Value::I32(0))?;

        assert_eq!(result, Value::I32(42));

        Ok(())
    }

    #[test]
    fn test_memory_fill_and_copy() -> Result<()> {
        let mut memory = MockMemory::new(1024);

        // Fill a region
        let fill = MemoryFill::new(0);
        fill.execute(
            &mut memory,
            &Value::I32(0),
            &Value::I32(0xFF),
            &Value::I32(10),
        )?;

        // Copy to another region
        let copy = MemoryCopy::new(0, 0);
        copy.execute(&mut memory, &Value::I32(20), &Value::I32(0), &Value::I32(5))?;

        // Verify the copy
        let copied_data = memory.read_bytes(20, 5)?;
        assert_eq!(copied_data, vec![0xFF; 5]);

        Ok(())
    }

    #[test]
    fn test_different_data_types() -> Result<()> {
        let mut memory = MockMemory::new(1024);

        // Test f64
        let f64_store = MemoryStore::f64(0, 8);
        f64_store.execute(
            &mut memory,
            &Value::I32(0),
            &Value::F64(wrt_foundation::FloatBits64::from_float(2.71828)),
        )?;

        let f64_load = MemoryLoad::f64(0, 8);
        let result = f64_load.execute(&memory, &Value::I32(0))?;

        if let Value::F64(bits) = result {
            assert!((bits.to_float() - 2.71828).abs() < 1e-10);
        } else {
            panic!("Expected F64 value");
        }

        Ok(())
    }
}
