//! Tracing support for WRT runtime
//!
//! This module provides structured tracing capabilities that work in both
//! std and no_std environments. It wraps the `tracing` crate and provides
//! WRT-specific utilities for debugging complex execution flows.

#![cfg(feature = "tracing")]

pub use tracing::{debug, error, info, trace, warn};
pub use tracing::{debug_span, error_span, info_span, span, trace_span, warn_span};
pub use tracing::{event, instrument, Level, Span};

/// Re-export tracing macros for convenience
pub use tracing::{field, span_enabled};

/// Trace events for module lifecycle
#[derive(Debug, Clone)]
pub struct ModuleTrace;

impl ModuleTrace {
    /// Create a span for module loading
    #[inline]
    pub fn loading(module_id: usize, size: usize) -> Span {
        debug_span!("module_load", module_id = %module_id, binary_size = %size)
    }

    /// Create a span for module conversion
    #[inline]
    pub fn converting(module_id: usize) -> Span {
        debug_span!("module_convert", module_id = %module_id)
    }

    /// Create a span for module instantiation
    #[inline]
    pub fn instantiating(module_id: usize) -> Span {
        debug_span!("module_instantiate", module_id = %module_id)
    }
}

/// Trace events for import operations
#[derive(Debug, Clone)]
pub struct ImportTrace;

impl ImportTrace {
    /// Create a span for import registration
    #[inline]
    pub fn registering(module: &str, field: &str, idx: usize) -> Span {
        trace_span!("import_register", module = %module, field = %field, index = %idx)
    }

    /// Create a span for import resolution
    #[inline]
    pub fn resolving(idx: usize) -> Span {
        trace_span!("import_resolve", index = %idx)
    }

    /// Create a span for import lookup
    #[inline]
    pub fn lookup(module: &str, field: &str) -> Span {
        trace_span!("import_lookup", module = %module, field = %field)
    }
}

/// Trace events for function execution
#[derive(Debug, Clone)]
pub struct ExecutionTrace;

impl ExecutionTrace {
    /// Create a span for function execution
    #[inline]
    pub fn function(func_idx: usize, instance_id: usize) -> Span {
        debug_span!("execute_function", func_idx = %func_idx, instance = %instance_id)
    }

    /// Create a span for instruction execution
    #[inline]
    pub fn instruction(pc: usize, opcode: &str) -> Span {
        trace_span!("execute_instruction", pc = %pc, opcode = %opcode)
    }

    /// Create a span for host function calls
    #[inline]
    pub fn host_call(module: &str, function: &str) -> Span {
        info_span!("host_call", module = %module, function = %function)
    }
}

/// Trace events for memory operations
#[derive(Debug, Clone)]
pub struct MemoryTrace;

impl MemoryTrace {
    /// Create a span for memory allocation
    #[inline]
    pub fn allocating(size: usize, crate_id: &str) -> Span {
        trace_span!("memory_alloc", size = %size, crate_id = %crate_id)
    }

    /// Create a span for memory access
    #[inline]
    pub fn accessing(offset: usize, len: usize) -> Span {
        trace_span!("memory_access", offset = %offset, len = %len)
    }
}

/// Macro to conditionally compile tracing code
#[macro_export]
macro_rules! trace_event {
    ($level:expr, $($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            use $crate::tracing::event;
            event!($level, $($arg)*);
        }
        #[cfg(not(feature = "tracing"))]
        {
            // No-op when tracing is disabled
            let _ = ($level, $($arg)*);
        }
    };
}

/// Macro to create spans conditionally
#[macro_export]
macro_rules! trace_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            $crate::tracing::span!($($arg)*)
        }
        #[cfg(not(feature = "tracing"))]
        {
            // Return a dummy span when tracing is disabled
            ()
        }
    };
}

/// Enter a span conditionally
#[macro_export]
macro_rules! enter_span {
    ($span:expr) => {
        #[cfg(feature = "tracing")]
        let _guard = $span.enter();
        #[cfg(not(feature = "tracing"))]
        let _guard = ();
    };
}

// No-std collector for embedded environments
#[cfg(not(feature = "std"))]
pub mod collector {
    use super::*;

    /// A simple ring buffer collector for no_std environments
    /// This is a placeholder - implement based on specific needs
    pub struct RingBufferCollector {
        // Implementation would go here
        // For now, this is a placeholder
    }

    impl RingBufferCollector {
        pub const fn new() -> Self {
            RingBufferCollector {}
        }
    }
}