//! Debug infrastructure tests
//!
//! Tests for the DebuggableRuntime and RuntimeState trait implementations
//! in StacklessEngine.

#![cfg(all(feature = "debug-runtime-traits", feature = "std"))]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use wrt_debug::runtime_traits::{
    Breakpoint, BreakpointCondition, BreakpointId, DebugAction, DebugError,
    DebuggableRuntime, RuntimeDebugger, RuntimeState,
};
use wrt_runtime::stackless::StacklessEngine;

/// Simple test debugger that counts events
struct TestDebugger {
    breakpoint_hits: AtomicU32,
    instruction_count: AtomicU32,
    function_entries: AtomicU32,
    function_exits: AtomicU32,
    trap_count: AtomicU32,
}

impl TestDebugger {
    fn new() -> Self {
        Self {
            breakpoint_hits: AtomicU32::new(0),
            instruction_count: AtomicU32::new(0),
            function_entries: AtomicU32::new(0),
            function_exits: AtomicU32::new(0),
            trap_count: AtomicU32::new(0),
        }
    }

    fn breakpoint_hits(&self) -> u32 {
        self.breakpoint_hits.load(Ordering::Acquire)
    }

    fn instruction_count(&self) -> u32 {
        self.instruction_count.load(Ordering::Acquire)
    }

    fn function_entries(&self) -> u32 {
        self.function_entries.load(Ordering::Acquire)
    }

    fn function_exits(&self) -> u32 {
        self.function_exits.load(Ordering::Acquire)
    }

    fn trap_count(&self) -> u32 {
        self.trap_count.load(Ordering::Acquire)
    }
}

impl RuntimeDebugger for TestDebugger {
    fn on_breakpoint(
        &mut self,
        _bp: &Breakpoint,
        _state: &dyn RuntimeState,
    ) -> DebugAction {
        self.breakpoint_hits.fetch_add(1, Ordering::AcqRel);
        DebugAction::Continue
    }

    fn on_instruction(&mut self, _pc: u32, _state: &dyn RuntimeState) -> DebugAction {
        self.instruction_count.fetch_add(1, Ordering::AcqRel);
        DebugAction::Continue
    }

    fn on_function_entry(&mut self, _func_idx: u32, _state: &dyn RuntimeState) {
        self.function_entries.fetch_add(1, Ordering::AcqRel);
    }

    fn on_function_exit(&mut self, _func_idx: u32, _state: &dyn RuntimeState) {
        self.function_exits.fetch_add(1, Ordering::AcqRel);
    }

    fn on_trap(&mut self, _trap_code: u32, _state: &dyn RuntimeState) {
        self.trap_count.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn test_debug_mode_toggle() {
    let mut engine = StacklessEngine::new();

    // Initially debug mode should be off
    assert!(!engine.is_debug_mode());

    // Enable debug mode
    engine.set_debug_mode(true);
    assert!(engine.is_debug_mode());

    // Disable debug mode
    engine.set_debug_mode(false);
    assert!(!engine.is_debug_mode());
}

#[test]
fn test_debugger_attachment() {
    let mut engine = StacklessEngine::new();

    // Initially no debugger attached
    assert!(!engine.has_debugger());
    assert!(!engine.is_debug_mode());

    // Attach debugger
    let debugger = Box::new(TestDebugger::new());
    engine.attach_debugger(debugger);

    // Should have debugger and debug mode should be auto-enabled
    assert!(engine.has_debugger());
    assert!(engine.is_debug_mode());

    // Detach debugger
    engine.detach_debugger();
    assert!(!engine.has_debugger());
    // Debug mode stays enabled (user may still want breakpoints)
    assert!(engine.is_debug_mode());
}

#[test]
fn test_breakpoint_management() {
    let mut engine = StacklessEngine::new();

    // Add a breakpoint
    let bp1 = Breakpoint::new(BreakpointId(1), 0x100);
    assert!(engine.add_breakpoint(bp1).is_ok());

    // Add another breakpoint at different address
    let bp2 = Breakpoint::new(BreakpointId(2), 0x200);
    assert!(engine.add_breakpoint(bp2).is_ok());

    // Try to add duplicate breakpoint at same address
    let bp_dup = Breakpoint::new(BreakpointId(3), 0x100);
    let result = engine.add_breakpoint(bp_dup);
    assert!(matches!(result, Err(DebugError::DuplicateBreakpoint)));

    // Disable first breakpoint
    assert!(engine.disable_breakpoint(BreakpointId(1)).is_ok());

    // Enable first breakpoint
    assert!(engine.enable_breakpoint(BreakpointId(1)).is_ok());

    // Remove first breakpoint
    assert!(engine.remove_breakpoint(BreakpointId(1)).is_ok());

    // Try to remove non-existent breakpoint
    let result = engine.remove_breakpoint(BreakpointId(99));
    assert!(matches!(result, Err(DebugError::BreakpointNotFound)));

    // Now we can add a breakpoint at address 0x100 again
    let bp_new = Breakpoint::new(BreakpointId(4), 0x100);
    assert!(engine.add_breakpoint(bp_new).is_ok());
}

#[test]
fn test_function_breakpoint() {
    let mut engine = StacklessEngine::new();

    // Create a breakpoint at function entry
    let bp = Breakpoint::at_function(BreakpointId(1), 5);
    assert!(engine.add_breakpoint(bp).is_ok());
}

#[test]
fn test_runtime_state_snapshot() {
    let engine = StacklessEngine::new();

    // Get state snapshot (engine not executing, so values will be default)
    let state = engine.get_state();

    // Verify we can read state without panicking
    let _pc = state.pc();
    let _sp = state.sp();
    let _fp = state.fp();
    let _func = state.current_function();

    // Stack access should return None when nothing on stack
    assert!(state.read_stack(0).is_none());

    // Local access should return None when no locals
    assert!(state.read_local(0).is_none());
}

#[test]
fn test_memory_accessor() {
    let engine = StacklessEngine::new();

    // Memory accessor returns None when no instance is loaded
    let memory = engine.get_memory();
    assert!(memory.is_none());
}

#[test]
fn test_breakpoint_conditions() {
    let mut engine = StacklessEngine::new();

    // Test breakpoint with hit count condition
    let mut bp_hitcount = Breakpoint::new(BreakpointId(1), 0x100);
    bp_hitcount.condition = BreakpointCondition::HitCount(5);
    assert!(engine.add_breakpoint(bp_hitcount).is_ok());

    // Test breakpoint with local equals condition
    let mut bp_local = Breakpoint::new(BreakpointId(2), 0x200);
    bp_local.condition = BreakpointCondition::LocalEquals { index: 0, value: 42 };
    assert!(engine.add_breakpoint(bp_local).is_ok());

    // Test unconditional breakpoint
    let bp_always = Breakpoint::new(BreakpointId(3), 0x300);
    assert!(engine.add_breakpoint(bp_always).is_ok());
}
