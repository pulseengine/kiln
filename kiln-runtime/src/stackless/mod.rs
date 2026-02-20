//! Stackless WebAssembly execution engine
//!
//! This module implements a stackless version of the WebAssembly execution
//! engine that doesn't rely on the host language's call stack, making it
//! suitable for environments with limited stack space and for no_std contexts.
//!
//! The stackless engine uses a state machine approach to track execution state
//! and allows for pausing and resuming execution at any point.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;
#[cfg(feature = "std")]
use alloc::string::String;
#[cfg(not(any(feature = "std", feature = "alloc")))]
type String =
    wrt_foundation::bounded::BoundedString<256>;

pub mod engine;
pub mod extensions;
pub mod frame;

#[cfg(feature = "std")]
pub mod tail_call;

pub use engine::{
    StacklessCallbackRegistry,
    StacklessEngine,
    StacklessStack,
};

/// Result of stackless execution containing completion status and return values.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Execution completed successfully with return values.
    Completed(wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 16, wrt_foundation::safe_memory::NoStdProvider<1024>>),
    /// Execution yielded and can be resumed.
    Yielded(YieldInfo),
    /// Execution is waiting for an external resource.
    Waiting(u32),
    /// Fuel was exhausted.
    FuelExhausted,
}

/// Information about a yielded execution that can be resumed later.
#[derive(Debug, Clone, PartialEq)]
pub struct YieldInfo {
    /// Current instruction pointer position.
    pub instruction_pointer: u32,
    /// Current operand stack state.
    pub operand_stack:       wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 256, wrt_foundation::safe_memory::NoStdProvider<4096>>,
    /// Current local variables.
    pub locals:              wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 128, wrt_foundation::safe_memory::NoStdProvider<2048>>,
    /// Current call stack.
    pub call_stack:          wrt_foundation::bounded::BoundedVec<u32, 64, wrt_foundation::safe_memory::NoStdProvider<512>>,
}

/// Internal state of the stackless execution engine.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineState {
    /// Current instruction pointer position.
    pub instruction_pointer: u32,
    /// Current operand stack state.
    pub operand_stack:       wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 256, wrt_foundation::safe_memory::NoStdProvider<4096>>,
    /// Current local variables.
    pub locals:              wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 128, wrt_foundation::safe_memory::NoStdProvider<2048>>,
    /// Current call stack.
    pub call_stack:          wrt_foundation::bounded::BoundedVec<u32, 64, wrt_foundation::safe_memory::NoStdProvider<512>>,
}

/// Type alias for stackless execution state.
pub type StacklessExecutionState = EngineState;
pub use frame::StacklessFrame;
