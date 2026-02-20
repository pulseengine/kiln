#![cfg(feature = "runtime-inspection")]

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

use wrt_foundation::{
    NoStdProvider,
    bounded::{BoundedVec, MAX_DWARF_FILE_TABLE},
};

use crate::bounded_debug_infra;
/// Runtime debugging API definitions
/// This module defines the interface between the debug information
/// and the runtime execution engine for advanced debugging features
use crate::{BasicType, DebugString};

/// Runtime state accessible during debugging
pub trait RuntimeState {
    /// Get current program counter
    fn pc(&self) -> u32;

    /// Get stack pointer
    fn sp(&self) -> u32;

    /// Get frame pointer (if available)
    fn fp(&self) -> Option<u32>;

    /// Read local variable by index
    fn read_local(&self, index: u32) -> Option<u64>;

    /// Read from operand stack
    fn read_stack(&self, offset: u32) -> Option<u64>;

    /// Get current function index
    fn current_function(&self) -> Option<u32>;
}

/// Memory access for debugging
pub trait DebugMemory {
    /// Read bytes from memory
    fn read_bytes(&self, addr: u32, len: usize) -> Option<&[u8]>;

    /// Read a u32 from memory
    fn read_u32(&self, addr: u32) -> Option<u32> {
        self.read_bytes(addr, 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a u64 from memory
    fn read_u64(&self, addr: u32) -> Option<u64> {
        self.read_bytes(addr, 8).map(|bytes| {
            u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        })
    }

    /// Check if address is valid
    fn is_valid_address(&self, addr: u32) -> bool;
}

/// Variable value read from runtime
#[derive(Debug, Clone)]
pub struct VariableValue {
    /// Raw bytes of the value
    pub bytes: [u8; 8],
    /// Size in bytes
    pub size: u8,
    /// Type information
    pub var_type: BasicType,
    /// Memory address (if applicable)
    pub address: Option<u32>,
}

impl VariableValue {
    /// Interpret as i32
    pub fn as_i32(&self) -> Option<i32> {
        if self.size >= 4 {
            Some(i32::from_le_bytes([
                self.bytes[0],
                self.bytes[1],
                self.bytes[2],
                self.bytes[3],
            ]))
        } else {
            None
        }
    }

    /// Interpret as u32
    pub fn as_u32(&self) -> Option<u32> {
        if self.size >= 4 {
            Some(u32::from_le_bytes([
                self.bytes[0],
                self.bytes[1],
                self.bytes[2],
                self.bytes[3],
            ]))
        } else {
            None
        }
    }

    /// Interpret as f32
    pub fn as_f32(&self) -> Option<f32> {
        self.as_u32().map(f32::from_bits)
    }

    /// Interpret as f64
    pub fn as_f64(&self) -> Option<f64> {
        if self.size >= 8 {
            Some(f64::from_bits(u64::from_le_bytes(self.bytes)))
        } else {
            None
        }
    }
}

/// DWARF location description (simplified)
#[derive(Debug, Clone)]
pub enum DwarfLocation {
    /// In register/local
    Register(u32),
    /// At memory address
    Memory(u32),
    /// On stack at offset from frame pointer
    FrameOffset(i32),
    /// Complex expression (not yet supported)
    Expression(BoundedVec<u8, 64, crate::bounded_debug_infra::DebugProvider>),
}

/// Variable information with runtime value
#[derive(Debug)]
pub struct LiveVariable<'a> {
    /// Variable name
    pub name: Option<DebugString<'a>>,
    /// Variable type
    pub var_type: BasicType,
    /// DWARF location
    pub location: DwarfLocation,
    /// Current value (if available)
    pub value: Option<VariableValue>,
    /// Scope (PC range where valid)
    pub scope_start: u32,
    pub scope_end: u32,
}

/// Breakpoint identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakpointId(pub u32);

/// Breakpoint information
#[derive(Debug)]
pub struct Breakpoint {
    /// Unique ID
    pub id: BreakpointId,
    /// Address to break at
    pub address: u32,
    /// Source file
    pub file_index: Option<u16>,
    /// Source line
    pub line: Option<u32>,
    /// Condition (simplified - would need expression evaluator)
    pub condition: Option<BreakpointCondition>,
    /// Hit count
    pub hit_count: u32,
    /// Enabled state
    pub enabled: bool,
}

/// Simple breakpoint conditions
#[derive(Debug, Clone)]
pub enum BreakpointCondition {
    /// Break when hit count reaches value
    HitCount(u32),
    /// Break when local variable equals value
    LocalEquals { index: u32, value: u64 },
    /// Always break
    Always,
}

/// Debug action to take
#[derive(Debug, Clone, Copy)]
pub enum DebugAction {
    /// Continue execution
    Continue,
    /// Step to next instruction
    StepInstruction,
    /// Step to next line
    StepLine,
    /// Step over function call
    StepOver,
    /// Step out of current function
    StepOut,
    /// Stop execution
    Break,
}

/// Runtime debugger interface
pub trait RuntimeDebugger {
    /// Called when breakpoint is hit
    fn on_breakpoint(
        &mut self,
        bp: &Breakpoint,
        state: &(dyn RuntimeState + 'static),
    ) -> DebugAction;

    /// Called on each instruction (if enabled)
    fn on_instruction(&mut self, pc: u32, state: &(dyn RuntimeState + 'static)) -> DebugAction;

    /// Called on function entry
    fn on_function_entry(&mut self, func_idx: u32, state: &(dyn RuntimeState + 'static));

    /// Called on function exit
    fn on_function_exit(&mut self, func_idx: u32, state: &(dyn RuntimeState + 'static));

    /// Called on trap/panic
    fn on_trap(&mut self, trap_code: u32, state: &(dyn RuntimeState + 'static));
}

/// Debugger attachment point for runtime
pub trait DebuggableRuntime {
    /// Attach a debugger
    fn attach_debugger(&mut self, debugger: Box<dyn RuntimeDebugger>);

    /// Detach current debugger
    fn detach_debugger(&mut self);

    /// Check if debugger is attached
    fn has_debugger(&self) -> bool;

    /// Set execution mode
    fn set_debug_mode(&mut self, enabled: bool);

    /// Add breakpoint
    fn add_breakpoint(&mut self, bp: Breakpoint) -> Result<(), DebugError>;

    /// Remove breakpoint
    fn remove_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError>;

    /// Get runtime state
    fn get_state(&self) -> Box<dyn RuntimeState>;

    /// Get memory accessor
    fn get_memory(&self) -> Box<dyn DebugMemory>;
}

/// Debug error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugError {
    /// Breakpoint already exists
    DuplicateBreakpoint,
    /// Breakpoint not found
    BreakpointNotFound,
    /// Invalid address
    InvalidAddress,
    /// No debugger attached
    NoDebugger,
    /// Feature not supported
    NotSupported,
}

/// Example integration for WRT interpreter
#[cfg(feature = "example-integration")]
mod integration_example {
    use super::*;

    /// Example: How WRT interpreter would implement RuntimeState
    struct InterpreterState<'a> {
        pc: u32,
        sp: u32,
        locals: &'a [u64],
        stack: &'a [u64],
        func_idx: u32,
    }

    impl<'a> RuntimeState for InterpreterState<'a> {
        fn pc(&self) -> u32 {
            self.pc
        }

        fn sp(&self) -> u32 {
            self.sp
        }

        fn fp(&self) -> Option<u32> {
            None
        }

        // WASM has no frame pointer

        fn read_local(&self, index: u32) -> Option<u64> {
            self.locals.get(index as usize).copied()
        }

        fn read_stack(&self, offset: u32) -> Option<u64> {
            self.stack.get(offset as usize).copied()
        }

        fn current_function(&self) -> Option<u32> {
            Some(self.func_idx)
        }
    }
}
