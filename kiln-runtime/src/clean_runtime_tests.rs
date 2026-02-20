//! Test module for clean architecture migration
//!
//! This module demonstrates how to use clean types from wrt-foundation
//! instead of provider-embedded types, serving as a prototype for the
//! full wrt-runtime migration.

#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::{
    string::String,
    vec::Vec,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
#[cfg(any(feature = "std", feature = "alloc"))]
use wrt_foundation::{
    CleanFuncType,
    CleanMemoryType,
    CleanTableType,
    CleanValType,
    CleanValue,
    RuntimeTypeFactory,
    TypeFactory,
};

/// Clean runtime module using provider-free types
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct CleanRuntime {
    /// Runtime type factory for allocation
    factory:   RuntimeTypeFactory<65536>,
    /// Functions in this runtime
    functions: Vec<CleanFunction>,
    /// Memory instances
    memories:  Vec<CleanMemory>,
    /// Table instances  
    tables:    Vec<CleanTable>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl CleanRuntime {
    /// Create a new clean runtime
    pub fn new() -> Self {
        Self {
            factory:   RuntimeTypeFactory::new(),
            functions: Vec::new(),
            memories:  Vec::new(),
            tables:    Vec::new(),
        }
    }

    /// Add a function to the runtime
    pub fn add_function(&mut self, name: String, func_type: CleanFuncType) -> Result<u32> {
        let function = CleanFunction {
            name,
            func_type,
            id: self.functions.len() as u32,
        };

        self.functions.push(function);
        Ok((self.functions.len() - 1) as u32)
    }

    /// Add a memory to the runtime
    pub fn add_memory(&mut self, name: String, memory_type: CleanMemoryType) -> Result<u32> {
        let memory = CleanMemory {
            name,
            memory_type,
            id: self.memories.len() as u32,
            data: Vec::new(), // In real implementation, this would be managed by factory
        };

        self.memories.push(memory);
        Ok((self.memories.len() - 1) as u32)
    }

    /// Get function by ID
    pub fn get_function(&self, id: u32) -> Option<&CleanFunction> {
        self.functions.get(id as usize)
    }

    /// Get memory by ID
    pub fn get_memory(&self, id: u32) -> Option<&CleanMemory> {
        self.memories.get(id as usize)
    }

    /// Execute a function (simplified implementation)
    pub fn execute_function(&self, id: u32, args: Vec<CleanValue>) -> Result<Vec<CleanValue>> {
        let function = self
            .get_function(id)
            .ok_or_else(|| Error::runtime_function_not_found("Function not found"))?;

        // Validate argument count
        if args.len() != function.func_type.params.len() {
            return Err(Error::validation_error("Argument count mismatch"));
        }

        // For this test, just return dummy results matching the function signature
        let results = function
            .func_type
            .results
            .iter()
            .map(|result_type| match result_type {
                CleanValType::Bool => CleanValue::Bool(false),
                CleanValType::S32 => CleanValue::S32(42),
                CleanValType::U32 => CleanValue::U32(42),
                CleanValType::S64 => CleanValue::S64(42),
                CleanValType::U64 => CleanValue::U64(42),
                CleanValType::F32 => CleanValue::F32(42.0),
                CleanValType::F64 => CleanValue::F64(42.0),
                _ => CleanValue::S32(0), // Default for complex types
            })
            .collect();

        Ok(results)
    }

    /// Get factory reference (for internal allocation needs)
    pub fn factory(&self) -> &RuntimeTypeFactory<65536> {
        &self.factory
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl Default for CleanRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Clean function representation without provider embedding
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone)]
pub struct CleanFunction {
    /// Function name
    pub name:      String,
    /// Function type signature
    pub func_type: CleanFuncType,
    /// Function ID
    pub id:        u32,
}

/// Clean memory representation without provider embedding  
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone)]
pub struct CleanMemory {
    /// Memory name
    pub name:        String,
    /// Memory type
    pub memory_type: CleanMemoryType,
    /// Memory ID
    pub id:          u32,
    /// Memory data (simplified - in real impl would use factory for bounded
    /// allocation)
    pub data:        Vec<u8>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl CleanMemory {
    /// Read from memory
    pub fn read(&self, offset: u32, size: u32) -> Result<Vec<u8>> {
        let start = offset as usize;
        let end = start + size as usize;

        if end > self.data.len() {
            return Err(Error::memory_error("Memory access out of bounds"));
        }

        Ok(self.data[start..end].to_vec())
    }

    /// Write to memory
    pub fn write(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + data.len();

        // Grow memory if needed (simplified)
        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        self.data[start..end].copy_from_slice(data);
        Ok(())
    }
}

/// Clean table representation without provider embedding
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone)]
pub struct CleanTable {
    /// Table name
    pub name:       String,
    /// Table type
    pub table_type: CleanTableType,
    /// Table ID
    pub id:         u32,
    /// Table elements (simplified)
    pub elements:   Vec<Option<u32>>, // Function references
}

// Provide empty implementations for no-alloc environments
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct CleanRuntime;

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl CleanRuntime {
    pub fn new() -> Self {
        Self
    }
}

