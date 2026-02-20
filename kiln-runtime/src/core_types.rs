//! Core type definitions for wrt-runtime
//!
//! This module provides essential type definitions that are used throughout
//! the runtime. These types are designed to work in both std and `no_std`
//! environments.

use wrt_error::Result;
use wrt_foundation::{
    bounded::BoundedVec,
    prelude::{
        codes,
        BoundedCapacity,
        Clone,
        Debug,
        Default,
        Eq,
        Error,
        ErrorCategory,
        PartialEq,
    },
    safe_memory::NoStdProvider,
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
};
use wrt_instructions::Value;

use crate::{
    bounded_runtime_infra::RuntimeProvider,
    prelude::ToString,
    simple_types::{
        LocalsVec,
        ParameterVec,
        ValueStackVec,
    },
};

/// Call frame for function execution tracking
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallFrame {
    /// Function index being executed
    pub function_index:      u32,
    /// Current instruction pointer
    pub instruction_pointer: u32,
    /// Local variables for this frame
    pub locals:              LocalsVec,
    /// Return address (for stackless execution)
    pub return_address:      Option<u32>,
}

impl Checksummable for CallFrame {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.function_index.to_le_bytes());
        checksum.update_slice(&self.instruction_pointer.to_le_bytes());
        checksum.update_slice(&(self.locals.len() as u32).to_le_bytes());
    }
}

impl ToBytes for CallFrame {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> Result<()> {
        writer.write_all(&self.function_index.to_le_bytes())?;
        writer.write_all(&self.instruction_pointer.to_le_bytes())?;
        Ok(())
    }
}

impl FromBytes for CallFrame {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &PStream,
    ) -> Result<Self> {
        let mut func_bytes = [0u8; 4];
        reader.read_exact(&mut func_bytes)?;
        let function_index = u32::from_le_bytes(func_bytes);

        let mut ip_bytes = [0u8; 4];
        reader.read_exact(&mut ip_bytes)?;
        let instruction_pointer = u32::from_le_bytes(ip_bytes);

        let provider_clone = RuntimeProvider::default();
        let locals = BoundedVec::new(provider_clone)?;

        Ok(CallFrame {
            function_index,
            instruction_pointer,
            locals,
            return_address: None,
        })
    }
}

/// Component execution state
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentExecutionState {
    /// Whether the component is currently running
    pub is_running:          bool,
    /// Current instruction pointer (if running)
    pub instruction_pointer: u32,
    /// Stack depth
    pub stack_depth:         usize,
    /// Gas remaining for execution
    pub gas_remaining:       u64,
}

impl Checksummable for ComponentExecutionState {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[if self.is_running { 1u8 } else { 0u8 }]);
        checksum.update_slice(&self.instruction_pointer.to_le_bytes());
        checksum.update_slice(&(self.stack_depth as u32).to_le_bytes());
        checksum.update_slice(&(self.gas_remaining as u32).to_le_bytes());
    }
}

impl ToBytes for ComponentExecutionState {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> Result<()> {
        writer.write_all(&[if self.is_running { 1 } else { 0 }])?;
        writer.write_all(&self.instruction_pointer.to_le_bytes())?;
        writer.write_all(&(self.stack_depth as u32).to_le_bytes())?;
        writer.write_all(&(self.gas_remaining as u32).to_le_bytes())?;
        Ok(())
    }
}

impl FromBytes for ComponentExecutionState {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &PStream,
    ) -> Result<Self> {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        let is_running = byte[0] != 0;

        let mut ip_bytes = [0u8; 4];
        reader.read_exact(&mut ip_bytes)?;
        let instruction_pointer = u32::from_le_bytes(ip_bytes);

        let mut depth_bytes = [0u8; 4];
        reader.read_exact(&mut depth_bytes)?;
        let stack_depth = u32::from_le_bytes(depth_bytes) as usize;

        let mut gas_bytes = [0u8; 4];
        reader.read_exact(&mut gas_bytes)?;
        let gas_remaining = u64::from(u32::from_le_bytes(gas_bytes));

        Ok(ComponentExecutionState {
            is_running,
            instruction_pointer,
            stack_depth,
            gas_remaining,
        })
    }
}

/// Execution context for runtime operations
#[derive(Debug, Default)]
pub struct ExecutionContext {
    /// Value stack for WebAssembly execution
    pub value_stack: ValueStackVec,
    /// Call stack for function tracking
    pub call_stack:  ParameterVec, // Reuse parameter vec for simplicity
    /// Current execution statistics
    pub stats:       crate::execution::ExecutionStats,
    /// Whether execution is currently active
    pub is_active:   bool,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new() -> Result<Self> {
        let provider = RuntimeProvider::default();
        Ok(ExecutionContext {
            value_stack: BoundedVec::new(provider.clone())?,
            call_stack:  BoundedVec::new(provider)?,
            stats:       crate::execution::ExecutionStats::new(),
            is_active:   false,
        })
    }

    /// Push a value onto the value stack
    pub fn push_value(&mut self, value: Value) -> Result<()> {
        self.value_stack
            .push(value)
            .map_err(|_| Error::runtime_execution_error("Value stack capacity exceeded"))
    }

    /// Pop a value from the value stack
    pub fn pop_value(&mut self) -> Option<Value> {
        self.value_stack.pop().ok().flatten()
    }

    /// Get the current stack depth
    pub fn stack_depth(&self) -> usize {
        self.value_stack.len()
    }
}
