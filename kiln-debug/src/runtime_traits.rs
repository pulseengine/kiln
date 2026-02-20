// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Runtime debugging trait definitions
//!
//! This module provides the core traits for runtime introspection and debugger
//! attachment. These traits are independent of DWARF parsing - they define
//! what information a debugger can access from the execution engine, not how
//! to correlate it with source code.
//!
//! # Architecture
//!
//! - [`RuntimeState`]: Read execution state (PC, locals, stack)
//! - [`DebugMemory`]: Read WebAssembly linear memory
//! - [`RuntimeDebugger`]: Callback interface for debugger events
//! - [`DebuggableRuntime`]: Attach/detach debugger, manage breakpoints
//!
//! These traits are designed to be implementable without any DWARF or source
//! debugging dependencies, enabling basic debugging even for stripped binaries.

#![cfg(feature = "runtime-traits")]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

/// Runtime state accessible during debugging
///
/// This trait provides read-only access to the execution engine's current state.
/// Implementations should provide live values that reflect the actual engine state.
pub trait RuntimeState {
    /// Get current program counter (instruction offset within current function)
    fn pc(&self) -> u32;

    /// Get stack pointer (number of values on operand stack)
    fn sp(&self) -> u32;

    /// Get frame pointer (if available)
    ///
    /// WebAssembly doesn't have a traditional frame pointer, but this can
    /// be used to track the base of locals for the current frame.
    fn fp(&self) -> Option<u32>;

    /// Read local variable by index
    ///
    /// Returns the raw u64 representation of the local variable value.
    /// The caller is responsible for interpreting the value based on type.
    fn read_local(&self, index: u32) -> Option<u64>;

    /// Read from operand stack at offset from top
    ///
    /// Offset 0 is the top of stack, offset 1 is one below, etc.
    fn read_stack(&self, offset: u32) -> Option<u64>;

    /// Get current function index
    fn current_function(&self) -> Option<u32>;
}

/// Memory access for debugging
///
/// This trait provides read-only access to WebAssembly linear memory
/// for inspection during debugging.
pub trait DebugMemory {
    /// Read bytes from memory at the given address
    fn read_bytes(&self, addr: u32, len: usize) -> Option<&[u8]>;

    /// Read a u32 from memory (little-endian)
    fn read_u32(&self, addr: u32) -> Option<u32> {
        self.read_bytes(addr, 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a u64 from memory (little-endian)
    fn read_u64(&self, addr: u32) -> Option<u64> {
        self.read_bytes(addr, 8).map(|bytes| {
            u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        })
    }

    /// Check if address is valid (within memory bounds)
    fn is_valid_address(&self, addr: u32) -> bool;
}

/// Breakpoint identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BreakpointId(pub u32);

/// Simple breakpoint conditions
#[derive(Debug, Clone)]
pub enum BreakpointCondition {
    /// Break when hit count reaches value
    HitCount(u32),
    /// Break when local variable equals value
    LocalEquals { index: u32, value: u64 },
    /// Always break (unconditional)
    Always,
}

/// Breakpoint information
#[derive(Debug)]
pub struct Breakpoint {
    /// Unique ID for this breakpoint
    pub id: BreakpointId,
    /// Address to break at (PC value / instruction offset)
    pub address: u32,
    /// Function index (if known, for function entry breakpoints)
    pub func_idx: Option<u32>,
    /// Condition for breaking (e.g., hit count, local value)
    pub condition: BreakpointCondition,
    /// Hit count (incremented each time breakpoint is reached)
    pub hit_count: u32,
    /// Whether the breakpoint is enabled
    pub enabled: bool,
}

impl Breakpoint {
    /// Create a new unconditional breakpoint
    pub fn new(id: BreakpointId, address: u32) -> Self {
        Self {
            id,
            address,
            func_idx: None,
            condition: BreakpointCondition::Always,
            hit_count: 0,
            enabled: true,
        }
    }

    /// Create a breakpoint at a function entry
    pub fn at_function(id: BreakpointId, func_idx: u32) -> Self {
        Self {
            id,
            address: 0, // Entry point
            func_idx: Some(func_idx),
            condition: BreakpointCondition::Always,
            hit_count: 0,
            enabled: true,
        }
    }
}

/// Debug action to take after a debug event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    /// Continue execution normally
    Continue,
    /// Step to next instruction
    StepInstruction,
    /// Step to next source line (requires DWARF info)
    StepLine,
    /// Step over function call
    StepOver,
    /// Step out of current function
    StepOut,
    /// Stop execution (break)
    Break,
}

/// Runtime debugger interface
///
/// This trait defines callbacks that the debugger receives during execution.
/// The runtime calls these methods at appropriate points during execution.
///
/// Requires `Send + Sync` for use in multi-threaded runtimes.
pub trait RuntimeDebugger: Send + Sync {
    /// Called when a breakpoint is hit
    ///
    /// Returns the action to take after the breakpoint.
    fn on_breakpoint(&mut self, bp: &Breakpoint, state: &dyn RuntimeState) -> DebugAction;

    /// Called on each instruction (if single-stepping is enabled)
    ///
    /// This can be expensive - implementations should only enable this
    /// when actively stepping.
    fn on_instruction(&mut self, pc: u32, state: &dyn RuntimeState) -> DebugAction;

    /// Called on function entry
    fn on_function_entry(&mut self, func_idx: u32, state: &dyn RuntimeState);

    /// Called on function exit
    fn on_function_exit(&mut self, func_idx: u32, state: &dyn RuntimeState);

    /// Called on trap/panic
    fn on_trap(&mut self, trap_code: u32, state: &dyn RuntimeState);
}

/// Debug error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugError {
    /// Breakpoint already exists at this address
    DuplicateBreakpoint,
    /// Breakpoint not found
    BreakpointNotFound,
    /// Invalid address
    InvalidAddress,
    /// No debugger attached
    NoDebugger,
    /// Feature not supported
    NotSupported,
    /// Maximum breakpoints reached
    BreakpointLimitReached,
}

/// Debugger attachment point for runtime
///
/// This trait allows a debugger to be attached to and detached from a runtime.
/// The runtime implementation is responsible for calling the debugger callbacks
/// at appropriate points during execution.
#[cfg(any(feature = "std", feature = "alloc"))]
pub trait DebuggableRuntime {
    /// Attach a debugger to this runtime
    fn attach_debugger(&mut self, debugger: Box<dyn RuntimeDebugger>);

    /// Detach the current debugger
    fn detach_debugger(&mut self);

    /// Check if a debugger is attached
    fn has_debugger(&self) -> bool;

    /// Enable or disable debug mode
    ///
    /// When debug mode is disabled, the runtime can skip debug checks
    /// for better performance.
    fn set_debug_mode(&mut self, enabled: bool);

    /// Check if debug mode is enabled
    fn is_debug_mode(&self) -> bool;

    /// Add a breakpoint
    fn add_breakpoint(&mut self, bp: Breakpoint) -> Result<(), DebugError>;

    /// Remove a breakpoint by ID
    fn remove_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError>;

    /// Enable a breakpoint
    fn enable_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError>;

    /// Disable a breakpoint (without removing it)
    fn disable_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError>;

    /// Get current runtime state snapshot
    fn get_state(&self) -> Box<dyn RuntimeState>;

    /// Get memory accessor
    fn get_memory(&self) -> Option<Box<dyn DebugMemory>>;
}

/// A simpler debuggable runtime for no_std without alloc
///
/// This version uses static callbacks instead of boxed trait objects.
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub trait DebuggableRuntime {
    /// Enable or disable debug mode
    fn set_debug_mode(&mut self, enabled: bool);

    /// Check if debug mode is enabled
    fn is_debug_mode(&self) -> bool;

    /// Add a breakpoint at an address
    fn add_breakpoint_at(&mut self, address: u32) -> Result<BreakpointId, DebugError>;

    /// Remove a breakpoint by ID
    fn remove_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError>;

    /// Get current PC
    fn current_pc(&self) -> u32;

    /// Get current function index
    fn current_function(&self) -> Option<u32>;
}
